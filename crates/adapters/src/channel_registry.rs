#[cfg(feature = "channel-discord")]
use crate::channel_discord::{DiscordChannelAdapter, DiscordConfig};
#[cfg(feature = "channel-irc")]
use crate::channel_irc::{IrcChannelAdapter, IrcConfig};
#[cfg(feature = "channel-matrix")]
use crate::channel_matrix::{MatrixChannelAdapter, MatrixConfig};
#[cfg(feature = "channel-slack")]
use crate::channel_slack::{SlackChannelAdapter, SlackConfig};
use crate::channel_telegram::{TelegramChannelAdapter, TelegramConfig};
#[cfg(feature = "channel-whatsapp")]
use crate::channel_whatsapp::{WhatsAppChannelAdapter, WhatsAppConfig};
use crate::contracts::ChannelAdapter;

pub const DEFAULT_CHANNEL_ID: &str = "discord";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    Discord,
    Slack,
    Telegram,
    Irc,
    Matrix,
    WhatsApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelRegistryEntry {
    pub id: &'static str,
    pub adapter_id: &'static str,
    pub aliases: &'static [&'static str],
    pub label: &'static str,
    pub kind: ChannelKind,
}

const DISCORD_ALIASES: &[&str] = &["channel.discord"];
const SLACK_ALIASES: &[&str] = &["slack-bot", "channel.slack"];
const TELEGRAM_ALIASES: &[&str] = &["channel.telegram"];
const IRC_ALIASES: &[&str] = &["channel.irc"];
const MATRIX_ALIASES: &[&str] = &["channel.matrix"];
const WHATSAPP_ALIASES: &[&str] = &["wa", "channel.whatsapp"];

static CHANNEL_REGISTRY: &[ChannelRegistryEntry] = &[
    ChannelRegistryEntry {
        id: "discord",
        adapter_id: "channel.discord",
        aliases: DISCORD_ALIASES,
        label: "Discord bot channel",
        kind: ChannelKind::Discord,
    },
    ChannelRegistryEntry {
        id: "slack",
        adapter_id: "channel.slack",
        aliases: SLACK_ALIASES,
        label: "Slack bot channel",
        kind: ChannelKind::Slack,
    },
    ChannelRegistryEntry {
        id: "telegram",
        adapter_id: "channel.telegram",
        aliases: TELEGRAM_ALIASES,
        label: "Telegram bot channel",
        kind: ChannelKind::Telegram,
    },
    ChannelRegistryEntry {
        id: "irc",
        adapter_id: "channel.irc",
        aliases: IRC_ALIASES,
        label: "IRC channel",
        kind: ChannelKind::Irc,
    },
    ChannelRegistryEntry {
        id: "matrix",
        adapter_id: "channel.matrix",
        aliases: MATRIX_ALIASES,
        label: "Matrix room channel",
        kind: ChannelKind::Matrix,
    },
    ChannelRegistryEntry {
        id: "whatsapp",
        adapter_id: "channel.whatsapp",
        aliases: WHATSAPP_ALIASES,
        label: "WhatsApp Business channel",
        kind: ChannelKind::WhatsApp,
    },
];

pub fn channel_registry() -> &'static [ChannelRegistryEntry] {
    CHANNEL_REGISTRY
}

pub fn resolve_channel_entry(name: &str) -> Option<&'static ChannelRegistryEntry> {
    let key = name.trim();
    if key.is_empty() {
        return None;
    }

    channel_registry().iter().find(|entry| {
        entry.id.eq_ignore_ascii_case(key)
            || entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(key))
    })
}

pub fn resolve_channel_id(name: &str) -> Option<&'static str> {
    resolve_channel_entry(name).map(|entry| entry.id)
}

pub fn resolve_channel_adapter_id(name: &str) -> Option<&'static str> {
    resolve_channel_entry(name).map(|entry| entry.adapter_id)
}

/// Returns whether the channel kind is compiled into this binary.
/// This probe is side-effect free: it does not read runtime credentials or build adapters.
pub fn channel_is_compiled(name: &str) -> bool {
    let Some(entry) = resolve_channel_entry(name) else {
        return false;
    };

    match entry.kind {
        ChannelKind::Discord => cfg!(feature = "channel-discord"),
        ChannelKind::Slack => cfg!(feature = "channel-slack"),
        ChannelKind::Telegram => true,
        ChannelKind::Irc => cfg!(feature = "channel-irc"),
        ChannelKind::Matrix => cfg!(feature = "channel-matrix"),
        ChannelKind::WhatsApp => cfg!(feature = "channel-whatsapp"),
    }
}

pub fn build_contract_channel(name: &str) -> Result<Box<dyn ChannelAdapter>, String> {
    let entry = resolve_channel_entry(name).ok_or_else(|| {
        let available = channel_registry()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("unsupported channel '{name}'. supported channels: {available}")
    })?;

    let adapter: Box<dyn ChannelAdapter> = match entry.kind {
        ChannelKind::Discord => {
            #[cfg(feature = "channel-discord")]
            {
                let bot_token =
                    read_env_required("AXONRUNNER_DISCORD_BOT_TOKEN", "discord.bot_token")?;
                let guild_id = read_env_optional("AXONRUNNER_DISCORD_GUILD_ID");
                let webhook_url = read_env_optional("AXONRUNNER_CHANNEL_DISCORD_WEBHOOK");
                let config = DiscordConfig::new(bot_token, guild_id, Vec::new())
                    .map_err(|e| format!("discord config error: {e}"))?
                    .with_webhook(webhook_url)
                    .map_err(|e| format!("discord webhook config error: {e}"))?;
                let adapter = DiscordChannelAdapter::live(config)
                    .map_err(|e| format!("discord adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-discord"))]
            return Err(
                "discord channel not compiled (enable channel-discord feature)".to_string(),
            );
        }
        ChannelKind::Slack => {
            #[cfg(feature = "channel-slack")]
            {
                let bot_token = read_env_required("AXONRUNNER_SLACK_BOT_TOKEN", "slack.bot_token")?;
                let channel_id = read_env_optional("AXONRUNNER_SLACK_CHANNEL_ID");
                let webhook_url = read_env_optional("AXONRUNNER_CHANNEL_SLACK_WEBHOOK");
                let config = SlackConfig::new(bot_token, channel_id, Vec::new())
                    .map_err(|e| format!("slack config error: {e}"))?
                    .with_webhook(webhook_url)
                    .map_err(|e| format!("slack webhook config error: {e}"))?;
                let adapter = SlackChannelAdapter::live(config)
                    .map_err(|e| format!("slack adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-slack"))]
            return Err("slack channel not compiled (enable channel-slack feature)".to_string());
        }
        ChannelKind::Telegram => {
            let bot_token =
                read_env_required("AXONRUNNER_TELEGRAM_BOT_TOKEN", "telegram.bot_token")?;
            let allowed_users = read_env_list_required(
                "AXONRUNNER_TELEGRAM_ALLOWED_USERS",
                "telegram.allowed_users",
            )?;
            let config = TelegramConfig::new(bot_token, allowed_users)
                .map_err(|e| format!("telegram config error: {e}"))?;
            let adapter = TelegramChannelAdapter::live(config)
                .map_err(|e| format!("telegram adapter init failed: {e}"))?;
            Box::new(adapter)
        }
        ChannelKind::Irc => {
            #[cfg(feature = "channel-irc")]
            {
                let server = read_env_required("AXONRUNNER_IRC_SERVER", "irc.server")?;
                let channel = read_env_optional("AXONRUNNER_IRC_CHANNEL");
                let nick = read_env_trimmed("AXONRUNNER_IRC_NICK")
                    .unwrap_or_else(|| String::from("axonrunner-bot"));
                let config = IrcConfig::new(server, channel, nick, Vec::new())
                    .map_err(|e| format!("irc config error: {e}"))?;
                let adapter = IrcChannelAdapter::live(config)
                    .map_err(|e| format!("irc adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-irc"))]
            return Err("irc channel not compiled (enable channel-irc feature)".to_string());
        }
        ChannelKind::Matrix => {
            #[cfg(feature = "channel-matrix")]
            {
                let access_token =
                    read_env_required("AXONRUNNER_MATRIX_ACCESS_TOKEN", "matrix.access_token")?;
                let room_id = read_env_optional("AXONRUNNER_MATRIX_ROOM_ID");
                let homeserver = read_env_optional("AXONRUNNER_MATRIX_HOMESERVER");
                let config = MatrixConfig::new(access_token, room_id, homeserver, Vec::new())
                    .map_err(|e| format!("matrix config error: {e}"))?;
                let adapter = MatrixChannelAdapter::live(config)
                    .map_err(|e| format!("matrix adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-matrix"))]
            return Err("matrix channel not compiled (enable channel-matrix feature)".to_string());
        }
        ChannelKind::WhatsApp => {
            #[cfg(feature = "channel-whatsapp")]
            {
                let api_token =
                    read_env_required("AXONRUNNER_WHATSAPP_API_TOKEN", "whatsapp.api_token")?;
                let phone_number_id = read_env_optional("AXONRUNNER_WHATSAPP_PHONE_NUMBER_ID");
                let business_account_id =
                    read_env_optional("AXONRUNNER_WHATSAPP_BUSINESS_ACCOUNT_ID");
                let config = WhatsAppConfig::new(
                    api_token,
                    phone_number_id,
                    business_account_id,
                    Vec::new(),
                )
                .map_err(|e| format!("whatsapp config error: {e}"))?;
                let adapter = WhatsAppChannelAdapter::live(config)
                    .map_err(|e| format!("whatsapp adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-whatsapp"))]
            return Err(
                "whatsapp channel not compiled (enable channel-whatsapp feature)".to_string(),
            );
        }
    };

    Ok(adapter)
}

fn read_env_required(key: &str, field: &str) -> Result<String, String> {
    read_env_trimmed(key)
        .ok_or_else(|| format!("missing required environment variable '{key}' for {field}"))
}

fn read_env_list_required(key: &str, field: &str) -> Result<Vec<String>, String> {
    let raw = read_env_required(key, field)?;
    let values = parse_csv_list(&raw);
    if values.is_empty() {
        return Err(format!(
            "environment variable '{key}' for {field} must contain at least one value"
        ));
    }
    Ok(values)
}

fn parse_csv_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[allow(dead_code)]
fn read_env_optional(key: &str) -> Option<String> {
    read_env_trimmed(key)
}

pub(crate) fn read_env_trimmed(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    for candidate in env_candidates(key) {
        if let Ok(value) = std::env::var(candidate.as_str()) {
            let value = value.trim().to_owned();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn env_candidates(key: &str) -> [String; 2] {
    let legacy = key
        .strip_prefix("AXONRUNNER_")
        .map(|suffix| format!("AXIOM_{suffix}"))
        .unwrap_or_else(|| key.to_owned());
    [key.to_owned(), legacy]
}

#[cfg(test)]
mod tests {
    use super::{
        channel_is_compiled, parse_csv_list, resolve_channel_adapter_id, resolve_channel_id,
    };

    #[test]
    fn resolve_channel_id_accepts_namespaced_aliases() {
        assert_eq!(resolve_channel_id("channel.discord"), Some("discord"));
        assert_eq!(resolve_channel_id("channel.slack"), Some("slack"));
        assert_eq!(resolve_channel_id("channel.telegram"), Some("telegram"));
        assert_eq!(resolve_channel_id("channel.irc"), Some("irc"));
        assert_eq!(resolve_channel_id("channel.matrix"), Some("matrix"));
        assert_eq!(resolve_channel_id("channel.whatsapp"), Some("whatsapp"));
    }

    #[test]
    fn resolve_channel_adapter_id_matches_registry_entry() {
        assert_eq!(
            resolve_channel_adapter_id("discord"),
            Some("channel.discord")
        );
        assert_eq!(
            resolve_channel_adapter_id("slack-bot"),
            Some("channel.slack")
        );
        assert_eq!(resolve_channel_adapter_id("wa"), Some("channel.whatsapp"));
        assert_eq!(resolve_channel_adapter_id("unknown"), None);
    }

    #[test]
    fn parse_csv_list_trims_and_drops_empty_values() {
        assert_eq!(
            parse_csv_list(" 1001, 1002 ,, 1003 "),
            vec![
                String::from("1001"),
                String::from("1002"),
                String::from("1003")
            ]
        );
        assert!(parse_csv_list(" , , ").is_empty());
    }

    #[test]
    fn channel_is_compiled_tracks_registry_entries() {
        assert!(channel_is_compiled("telegram"));
        assert!(channel_is_compiled("channel.telegram"));
        assert!(!channel_is_compiled("unknown-channel"));
    }
}
