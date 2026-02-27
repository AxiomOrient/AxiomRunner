#![forbid(unsafe_code)]

pub mod agent_coclai;
pub mod agent_registry;
mod async_http_bridge;
pub mod channel;
#[cfg(feature = "channel-discord")]
pub mod channel_discord;
#[cfg(feature = "channel-irc")]
pub mod channel_irc;
#[cfg(feature = "channel-matrix")]
pub mod channel_matrix;
pub mod channel_registry;
#[cfg(feature = "channel-slack")]
pub mod channel_slack;
pub mod channel_telegram;
pub mod channel_validate;
#[cfg(feature = "channel-whatsapp")]
pub mod channel_whatsapp;
pub mod context_axiomme;
pub mod contracts;
pub mod error;
#[path = "memory.rs"]
pub mod memory;
pub mod memory_axiomme;
pub mod provider_openai;
pub mod provider_registry;
pub mod runtime;
pub mod tool;
pub mod tool_browser;
pub mod tool_composio;
pub mod tool_delegate;
pub mod tool_memory;

pub use agent_registry::{DEFAULT_AGENT_ID, ENV_AGENT_ID, MockAgentAdapter, build_contract_agent};
#[cfg(feature = "channel-discord")]
pub use channel_discord::{DiscordChannelAdapter, DiscordConfig};
#[cfg(feature = "channel-irc")]
pub use channel_irc::{IrcChannelAdapter, IrcConfig};
#[cfg(feature = "channel-matrix")]
pub use channel_matrix::{MatrixChannelAdapter, MatrixConfig};
pub use channel_registry::{
    ChannelKind, ChannelRegistryEntry, DEFAULT_CHANNEL_ID, build_contract_channel,
    channel_registry, resolve_channel_adapter_id, resolve_channel_id,
};
#[cfg(feature = "channel-slack")]
pub use channel_slack::{SlackChannelAdapter, SlackConfig};
pub use channel_telegram::{TelegramChannelAdapter, TelegramConfig};
#[cfg(feature = "channel-whatsapp")]
pub use channel_whatsapp::{WhatsAppChannelAdapter, WhatsAppConfig};
pub use context_axiomme::AxiommeContextAdapter;
pub use contracts::*;
pub use error::*;
pub use memory::{build_contract_context, build_contract_memory};
pub use memory_axiomme::AxiommeMemoryAdapter;
pub use provider_registry::{
    DEFAULT_PROVIDER_ID, ProviderRegistryEntry, build_contract_provider, provider_registry,
    resolve_provider_adapter_id, resolve_provider_id,
};
pub use tool::{
    DEFAULT_TOOL_ID, ToolRegistryEntry, ToolRegistryKind, build_contract_tool,
    resolve_tool_adapter_id, resolve_tool_id, tool_registry,
};
pub use tool_composio::ComposioToolAdapter;
