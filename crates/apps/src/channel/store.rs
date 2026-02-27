use std::fs;
use std::path::Path;

use axiom_adapters::{TelegramChannelAdapter, TelegramConfig};

use crate::parse_util::{parse_bool, parse_number};

use super::{CHANNEL_STORE_FORMAT, ChannelHealthStatus, ChannelKind, ChannelRecord, ChannelStore};

pub(super) fn validate_channel_config(config: &str) -> Result<(), String> {
    let config = config.trim();
    if config.is_empty() {
        return Err(String::from("channel config must not be empty"));
    }
    if config.contains('\n') || config.contains('\r') {
        return Err(String::from("channel config must be single-line text"));
    }
    if config.contains('|') {
        return Err(String::from("channel config must not contain '|'"));
    }
    Ok(())
}

pub(super) fn validate_channel_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(String::from("channel name must not be empty"));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("invalid channel name '{name}'"));
    }
    Ok(())
}

pub(super) fn parse_channel_kind(raw: &str) -> Result<ChannelKind, String> {
    let value = raw.trim();
    let kind = match value {
        "telegram" => ChannelKind::Telegram,
        "discord" => ChannelKind::Discord,
        "slack" => ChannelKind::Slack,
        "matrix" => ChannelKind::Matrix,
        "whatsapp" => ChannelKind::Whatsapp,
        "irc" => ChannelKind::Irc,
        "webhook" => ChannelKind::Webhook,
        "cli" => ChannelKind::Cli,
        _ => {
            return Err(format!(
                "unsupported channel type '{value}': expected one of [telegram, discord, slack, matrix, whatsapp, irc, webhook, cli]"
            ));
        }
    };
    Ok(kind)
}

pub(super) fn load_store(path: &Path) -> Result<ChannelStore, String> {
    if !path.exists() {
        return Ok(ChannelStore::default());
    }

    let content = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read channel store '{}': {error}",
            path.to_string_lossy()
        )
    })?;
    parse_store(&content).map_err(|error| {
        format!(
            "failed to parse channel store '{}': {error}",
            path.display()
        )
    })
}

pub(super) fn save_store(path: &Path, store: &ChannelStore) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create channel store directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    let mut body = String::new();
    body.push_str(CHANNEL_STORE_FORMAT);
    body.push('\n');
    for channel in &store.channels {
        body.push_str(&render_channel_line(channel));
        body.push('\n');
    }

    fs::write(path, body).map_err(|error| {
        format!(
            "failed to write channel store '{}': {error}",
            path.display()
        )
    })
}

fn parse_store(content: &str) -> Result<ChannelStore, String> {
    let mut channels = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line == CHANNEL_STORE_FORMAT {
            continue;
        }

        let channel =
            parse_channel_line(line).map_err(|error| format!("line {}: {error}", index + 1))?;
        channels.push(channel);
    }

    sort_channels(&mut channels);
    Ok(ChannelStore { channels })
}

fn render_channel_line(channel: &ChannelRecord) -> String {
    let last_health = channel
        .last_health
        .map(|status| status.as_str())
        .unwrap_or("-");
    let last_checked = channel
        .last_checked_at
        .map(|value| value.to_string())
        .unwrap_or_else(|| String::from("-"));

    format!(
        "channel|{}|{}|{}|{}|{}|{}|{}|{}",
        channel.name,
        channel.channel_type.as_str(),
        channel.config,
        channel.running,
        last_health,
        last_checked,
        channel.created_at,
        channel.updated_at
    )
}

fn parse_channel_line(line: &str) -> Result<ChannelRecord, String> {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() != 9 {
        return Err(format!(
            "invalid channel record '{line}': expected 9 fields"
        ));
    }
    if parts[0] != "channel" {
        return Err(format!("unsupported channel record prefix '{}'", parts[0]));
    }

    let name = parts[1].trim().to_string();
    validate_channel_name(&name)?;

    let channel_type = parse_channel_kind(parts[2])?;
    let config = parts[3].to_string();
    validate_channel_config(&config)?;
    let running = parse_bool(parts[4], "running")?;

    let last_health = if parts[5] == "-" {
        None
    } else {
        Some(parse_health_status(parts[5])?)
    };

    let last_checked_at = if parts[6] == "-" {
        None
    } else {
        Some(parse_number::<u64>(parts[6], "last_checked_at")?)
    };

    let created_at = parse_number::<u64>(parts[7], "created_at")?;
    let updated_at = parse_number::<u64>(parts[8], "updated_at")?;

    Ok(ChannelRecord {
        name,
        channel_type,
        config,
        running,
        last_health,
        last_checked_at,
        created_at,
        updated_at,
    })
}

fn parse_health_status(raw: &str) -> Result<ChannelHealthStatus, String> {
    match raw.trim() {
        "ok" => Ok(ChannelHealthStatus::Ok),
        "unhealthy" => Ok(ChannelHealthStatus::Unhealthy),
        other => Err(format!("invalid channel health status '{other}'")),
    }
}

/// Parses a `key=value,key=value` channel config string into a map.
/// Colon-separated values within a field (e.g. `allowed_users=alice:bob`) are preserved as-is.
pub(super) fn parse_channel_config(config_str: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for pair in config_str.split(',') {
        if let Some((key, value)) = pair.split_once('=') {
            let k = key.trim().to_string();
            let v = value.trim().to_string();
            if !k.is_empty() {
                map.insert(k, v);
            }
        }
    }
    map
}

/// Attempts to construct a channel adapter from the stored config string.
/// Returns Ok(()) if the config is valid for the given channel kind, Err with message otherwise.
/// Does NOT start polling or make network calls.
pub(super) fn try_build_channel_adapter(kind: ChannelKind, config_str: &str) -> Result<(), String> {
    let cfg = parse_channel_config(config_str);
    match kind {
        ChannelKind::Telegram => {
            let bot_token = cfg
                .get("bot_token")
                .filter(|v| !v.is_empty())
                .ok_or_else(|| String::from("missing bot_token in telegram config"))?
                .clone();
            let allowed_users = cfg
                .get("allowed_users")
                .map(|v| v.split(':').map(String::from).collect::<Vec<_>>())
                .unwrap_or_default();
            TelegramConfig::new(bot_token, allowed_users)
                .map(|config| {
                    let _ = TelegramChannelAdapter::new(config);
                })
                .map_err(|e| e.to_string())
        }
        // Other channel types: config presence is sufficient for now.
        // Each will be extended as their adapters gain proper config parsing.
        _ => Ok(()),
    }
}

pub(super) fn sort_channels(channels: &mut [ChannelRecord]) {
    channels.sort_by(|left, right| left.name.cmp(&right.name));
}
