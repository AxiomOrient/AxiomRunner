use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use axonrunner_adapters::{
    DiscordChannelAdapter, DiscordConfig, IrcChannelAdapter, IrcConfig, MatrixChannelAdapter,
    MatrixConfig, SlackChannelAdapter, SlackConfig, TelegramChannelAdapter, TelegramConfig,
    WhatsAppChannelAdapter, WhatsAppConfig, channel_registry, resolve_channel_id,
};

use crate::parse_util::{parse_bool, parse_number};

use super::{CHANNEL_STORE_FORMAT, ChannelHealthStatus, ChannelRecord, ChannelStore};

struct ChannelCapabilitySpec {
    required_keys: &'static [&'static str],
    optional_keys: &'static [&'static str],
}

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

pub(super) fn parse_channel_id(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    let Some(channel_id) = resolve_channel_id(value) else {
        let supported = channel_registry()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unsupported channel type '{value}': expected one of [{supported}]"
        ));
    };
    Ok(channel_id.to_string())
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
        channel.channel_type,
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

    let channel_type = parse_channel_id(parts[2])?;
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
pub(super) fn parse_channel_config(config_str: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
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

fn capability_spec(channel_id: &str) -> Option<ChannelCapabilitySpec> {
    match channel_id {
        "telegram" => Some(ChannelCapabilitySpec {
            required_keys: &["bot_token", "allowed_users"],
            optional_keys: &[],
        }),
        "discord" => Some(ChannelCapabilitySpec {
            required_keys: &["bot_token"],
            optional_keys: &["guild_id", "webhook_url", "allowed_users"],
        }),
        "slack" => Some(ChannelCapabilitySpec {
            required_keys: &["bot_token"],
            optional_keys: &["channel_id", "webhook_url", "allowed_users"],
        }),
        "matrix" => Some(ChannelCapabilitySpec {
            required_keys: &["access_token"],
            optional_keys: &["room_id", "homeserver", "allowed_users"],
        }),
        "irc" => Some(ChannelCapabilitySpec {
            required_keys: &["server"],
            optional_keys: &["channel", "nick", "allowed_users"],
        }),
        "whatsapp" => Some(ChannelCapabilitySpec {
            required_keys: &["api_token"],
            optional_keys: &["phone_number_id", "business_account_id", "allowed_users"],
        }),
        _ => None,
    }
}

fn validate_channel_capabilities(
    channel_id: &str,
    cfg: &BTreeMap<String, String>,
) -> Result<(), String> {
    let spec = capability_spec(channel_id)
        .ok_or_else(|| format!("no channel capability spec for '{channel_id}'"))?;

    for key in cfg.keys() {
        let known = spec.required_keys.contains(&key.as_str())
            || spec.optional_keys.contains(&key.as_str());
        if !known {
            return Err(format!(
                "unsupported config key '{key}' for channel '{channel_id}'"
            ));
        }
    }

    for key in spec.required_keys {
        let value = cfg.get(*key).map(String::as_str).unwrap_or("").trim();
        if value.is_empty() {
            return Err(format!(
                "missing required config key '{key}' for channel '{channel_id}'"
            ));
        }
    }

    for key in spec.optional_keys {
        if let Some(value) = cfg.get(*key)
            && value.trim().is_empty()
        {
            return Err(format!(
                "config key '{key}' for channel '{channel_id}' must not be empty"
            ));
        }
    }

    Ok(())
}

fn required_value(
    cfg: &BTreeMap<String, String>,
    channel_id: &str,
    key: &'static str,
) -> Result<String, String> {
    cfg.get(key)
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing required config key '{key}' for channel '{channel_id}'"))
}

fn optional_value(cfg: &BTreeMap<String, String>, key: &'static str) -> Option<String> {
    cfg.get(key)
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_allowed_users(cfg: &BTreeMap<String, String>) -> Vec<String> {
    optional_value(cfg, "allowed_users")
        .map(|users| {
            users
                .split(':')
                .map(str::trim)
                .filter(|user| !user.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// Attempts to construct a channel adapter from the stored config string.
/// Returns Ok(()) if the config is valid for the given channel id, Err with message otherwise.
/// Does NOT start polling or make network calls.
pub(super) fn try_build_channel_adapter(channel_id: &str, config_str: &str) -> Result<(), String> {
    let cfg = parse_channel_config(config_str);
    validate_channel_capabilities(channel_id, &cfg)?;

    match channel_id {
        "telegram" => {
            let bot_token = required_value(&cfg, channel_id, "bot_token")?;
            let allowed_users = parse_allowed_users(&cfg);
            TelegramConfig::new(bot_token, allowed_users)
                .map(|config| {
                    let _ = TelegramChannelAdapter::new(config);
                })
                .map_err(|e| e.to_string())
        }
        "discord" => {
            let bot_token = required_value(&cfg, channel_id, "bot_token")?;
            let guild_id = optional_value(&cfg, "guild_id");
            let webhook_url = optional_value(&cfg, "webhook_url");
            let allowed_users = parse_allowed_users(&cfg);
            DiscordConfig::new(bot_token, guild_id, allowed_users)
                .and_then(|config| config.with_webhook(webhook_url))
                .map(|config| {
                    let _ = DiscordChannelAdapter::new(config);
                })
                .map_err(|e| e.to_string())
        }
        "slack" => {
            let bot_token = required_value(&cfg, channel_id, "bot_token")?;
            let channel = optional_value(&cfg, "channel_id");
            let webhook_url = optional_value(&cfg, "webhook_url");
            let allowed_users = parse_allowed_users(&cfg);
            SlackConfig::new(bot_token, channel, allowed_users)
                .and_then(|config| config.with_webhook(webhook_url))
                .map(|config| {
                    let _ = SlackChannelAdapter::new(config);
                })
                .map_err(|e| e.to_string())
        }
        "matrix" => {
            let access_token = required_value(&cfg, channel_id, "access_token")?;
            let room_id = optional_value(&cfg, "room_id");
            let homeserver = optional_value(&cfg, "homeserver");
            let allowed_users = parse_allowed_users(&cfg);
            MatrixConfig::new(access_token, room_id, homeserver, allowed_users)
                .map(|config| {
                    let _ = MatrixChannelAdapter::new(config);
                })
                .map_err(|e| e.to_string())
        }
        "irc" => {
            let server = required_value(&cfg, channel_id, "server")?;
            let channel = optional_value(&cfg, "channel");
            let nick =
                optional_value(&cfg, "nick").unwrap_or_else(|| String::from("axonrunner-bot"));
            let allowed_users = parse_allowed_users(&cfg);
            IrcConfig::new(server, channel, nick, allowed_users)
                .map(|config| {
                    let _ = IrcChannelAdapter::new(config);
                })
                .map_err(|e| e.to_string())
        }
        "whatsapp" => {
            let api_token = required_value(&cfg, channel_id, "api_token")?;
            let phone_number_id = optional_value(&cfg, "phone_number_id");
            let business_account_id = optional_value(&cfg, "business_account_id");
            let allowed_users = parse_allowed_users(&cfg);
            WhatsAppConfig::new(
                api_token,
                phone_number_id,
                business_account_id,
                allowed_users,
            )
            .map(|config| {
                let _ = WhatsAppChannelAdapter::new(config);
            })
            .map_err(|e| e.to_string())
        }
        _ => Err(format!("no channel capability spec for '{channel_id}'")),
    }
}

pub(super) fn sort_channels(channels: &mut [ChannelRecord]) {
    channels.sort_by(|left, right| left.name.cmp(&right.name));
}
