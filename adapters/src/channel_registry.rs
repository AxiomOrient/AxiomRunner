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
    pub aliases: &'static [&'static str],
    pub label: &'static str,
    pub kind: ChannelKind,
}

const NO_ALIASES: &[&str] = &[];
const SLACK_ALIASES: &[&str] = &["slack-bot"];
const WHATSAPP_ALIASES: &[&str] = &["whatsapp", "wa"];

static CHANNEL_REGISTRY: &[ChannelRegistryEntry] = &[
    ChannelRegistryEntry {
        id: "discord",
        aliases: NO_ALIASES,
        label: "Discord bot channel",
        kind: ChannelKind::Discord,
    },
    ChannelRegistryEntry {
        id: "slack",
        aliases: SLACK_ALIASES,
        label: "Slack bot channel",
        kind: ChannelKind::Slack,
    },
    ChannelRegistryEntry {
        id: "telegram",
        aliases: NO_ALIASES,
        label: "Telegram bot channel",
        kind: ChannelKind::Telegram,
    },
    ChannelRegistryEntry {
        id: "irc",
        aliases: NO_ALIASES,
        label: "IRC channel",
        kind: ChannelKind::Irc,
    },
    ChannelRegistryEntry {
        id: "matrix",
        aliases: NO_ALIASES,
        label: "Matrix room channel",
        kind: ChannelKind::Matrix,
    },
    ChannelRegistryEntry {
        id: "whatsapp",
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
                let bot_token = read_env_required("AXIOM_DISCORD_BOT_TOKEN", "discord.bot_token")?;
                let guild_id = read_env_optional("AXIOM_DISCORD_GUILD_ID");
                let webhook_url = read_env_optional("AXIOM_CHANNEL_DISCORD_WEBHOOK");
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
                let bot_token = read_env_required("AXIOM_SLACK_BOT_TOKEN", "slack.bot_token")?;
                let channel_id = read_env_optional("AXIOM_SLACK_CHANNEL_ID");
                let webhook_url = read_env_optional("AXIOM_CHANNEL_SLACK_WEBHOOK");
                let config = SlackConfig::new(bot_token, channel_id, Vec::new())
                    .map_err(|e| format!("slack config error: {e}"))?
                    .with_webhook(webhook_url)
                    .map_err(|e| format!("slack webhook config error: {e}"))?;
                let adapter = SlackChannelAdapter::live(config)
                    .map_err(|e| format!("slack adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-slack"))]
            return Err(
                "slack channel not compiled (enable channel-slack feature)".to_string(),
            );
        }
        ChannelKind::Telegram => {
            let bot_token = read_env_required("AXIOM_TELEGRAM_BOT_TOKEN", "telegram.bot_token")?;
            let config = TelegramConfig::new(bot_token, Vec::new())
                .map_err(|e| format!("telegram config error: {e}"))?;
            Box::new(TelegramChannelAdapter::new(config))
        }
        ChannelKind::Irc => {
            #[cfg(feature = "channel-irc")]
            {
                let server = read_env_required("AXIOM_IRC_SERVER", "irc.server")?;
                let channel = read_env_optional("AXIOM_IRC_CHANNEL");
                let nick = read_env_trimmed("AXIOM_IRC_NICK")
                    .unwrap_or_else(|| String::from("axiom-bot"));
                let config = IrcConfig::new(server, channel, nick, Vec::new())
                    .map_err(|e| format!("irc config error: {e}"))?;
                let adapter = IrcChannelAdapter::live(config)
                    .map_err(|e| format!("irc adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-irc"))]
            return Err(
                "irc channel not compiled (enable channel-irc feature)".to_string(),
            );
        }
        ChannelKind::Matrix => {
            #[cfg(feature = "channel-matrix")]
            {
                let access_token =
                    read_env_required("AXIOM_MATRIX_ACCESS_TOKEN", "matrix.access_token")?;
                let room_id = read_env_optional("AXIOM_MATRIX_ROOM_ID");
                let homeserver = read_env_optional("AXIOM_MATRIX_HOMESERVER");
                let config = MatrixConfig::new(access_token, room_id, homeserver, Vec::new())
                    .map_err(|e| format!("matrix config error: {e}"))?;
                let adapter = MatrixChannelAdapter::live(config)
                    .map_err(|e| format!("matrix adapter init failed: {e}"))?;
                Box::new(adapter)
            }
            #[cfg(not(feature = "channel-matrix"))]
            return Err(
                "matrix channel not compiled (enable channel-matrix feature)".to_string(),
            );
        }
        ChannelKind::WhatsApp => {
            #[cfg(feature = "channel-whatsapp")]
            {
                let api_token =
                    read_env_required("AXIOM_WHATSAPP_API_TOKEN", "whatsapp.api_token")?;
                let phone_number_id = read_env_optional("AXIOM_WHATSAPP_PHONE_NUMBER_ID");
                let business_account_id = read_env_optional("AXIOM_WHATSAPP_BUSINESS_ACCOUNT_ID");
                let config =
                    WhatsAppConfig::new(api_token, phone_number_id, business_account_id, Vec::new())
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

#[allow(dead_code)]
fn read_env_optional(key: &str) -> Option<String> {
    read_env_trimmed(key)
}

fn read_env_trimmed(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
