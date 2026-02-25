#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationCategory {
    Chat,
    AiModel,
    Productivity,
    Platform,
}

impl IntegrationCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrationCategory::Chat => "chat",
            IntegrationCategory::AiModel => "ai_model",
            IntegrationCategory::Productivity => "productivity",
            IntegrationCategory::Platform => "platform",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationStatus {
    Active,
    Available,
    ComingSoon,
}

impl IntegrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrationStatus::Active => "active",
            IntegrationStatus::Available => "available",
            IntegrationStatus::ComingSoon => "coming_soon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationEntry {
    pub name: &'static str,
    pub category: IntegrationCategory,
    pub status: IntegrationStatus,
    pub transport: &'static str,
    pub summary: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationsAction {
    Info { name: String },
    Install { name: String },
    Remove { name: String },
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationsResult {
    Info { entry: IntegrationEntry },
    Installed { name: String, instructions: Vec<String> },
    Removed { name: String },
    Listed { entries: Vec<IntegrationEntry> },
}

pub fn execute_integrations_action(
    action: IntegrationsAction,
) -> Result<IntegrationsResult, String> {
    match action {
        IntegrationsAction::Info { name } => {
            let Some(entry) = find_integration(&name) else {
                return Err(format!("unknown integration '{name}'"));
            };
            Ok(IntegrationsResult::Info { entry })
        }
        IntegrationsAction::Install { name } => {
            let entry = find_integration(&name)
                .ok_or_else(|| format!("unknown integration '{name}'"))?;
            let instructions = build_install_instructions(&entry);
            Ok(IntegrationsResult::Installed { name, instructions })
        }
        IntegrationsAction::Remove { name } => {
            find_integration(&name)
                .ok_or_else(|| format!("unknown integration '{name}'"))?;
            Ok(IntegrationsResult::Removed { name })
        }
        IntegrationsAction::List => {
            let entries = INTEGRATION_CATALOG.to_vec();
            Ok(IntegrationsResult::Listed { entries })
        }
    }
}

fn find_integration(name: &str) -> Option<IntegrationEntry> {
    INTEGRATION_CATALOG
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .copied()
}

fn build_install_instructions(entry: &IntegrationEntry) -> Vec<String> {
    match entry.category {
        IntegrationCategory::Chat => vec![
            format!("run: axiom channel add --type {}", entry.name),
            String::from("configure bot token in environment"),
        ],
        IntegrationCategory::AiModel => vec![
            format!("set AXIOM_PROVIDER={}", entry.name),
            String::from("set api key in environment"),
        ],
        _ => vec![format!("see documentation for {}", entry.name)],
    }
}

const INTEGRATION_CATALOG: [IntegrationEntry; 22] = [
    IntegrationEntry {
        name: "telegram",
        category: IntegrationCategory::Chat,
        status: IntegrationStatus::Available,
        transport: "telegram_polling",
        summary: "telegram bot api long-polling channel",
    },
    IntegrationEntry {
        name: "discord",
        category: IntegrationCategory::Chat,
        status: IntegrationStatus::Available,
        transport: "discord_gateway",
        summary: "discord server and dm channel adapter",
    },
    IntegrationEntry {
        name: "slack",
        category: IntegrationCategory::Chat,
        status: IntegrationStatus::Available,
        transport: "slack_web_api",
        summary: "slack workspace channel integration",
    },
    IntegrationEntry {
        name: "matrix",
        category: IntegrationCategory::Chat,
        status: IntegrationStatus::Available,
        transport: "matrix_sync_api",
        summary: "matrix room messaging channel adapter",
    },
    IntegrationEntry {
        name: "whatsapp",
        category: IntegrationCategory::Chat,
        status: IntegrationStatus::Available,
        transport: "whatsapp_cloud_api",
        summary: "whatsapp business cloud channel adapter",
    },
    IntegrationEntry {
        name: "irc",
        category: IntegrationCategory::Chat,
        status: IntegrationStatus::Available,
        transport: "irc_socket_transport",
        summary: "irc channel transport adapter",
    },
    IntegrationEntry {
        name: "openai",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Active,
        transport: "openai_compatible_http",
        summary: "openai-compatible provider backend",
    },
    IntegrationEntry {
        name: "openrouter",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "openrouter_http",
        summary: "multi-model router provider endpoint",
    },
    IntegrationEntry {
        name: "anthropic",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "anthropic_compatible_http",
        summary: "anthropic-compatible provider backend",
    },
    IntegrationEntry {
        name: "deepseek",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "deepseek_openai_http",
        summary: "deepseek openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "groq",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "groq_openai_http",
        summary: "groq openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "mistral",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "mistral_openai_http",
        summary: "mistral openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "fireworks",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "fireworks_openai_http",
        summary: "fireworks openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "together",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "together_openai_http",
        summary: "together openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "perplexity",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "perplexity_openai_http",
        summary: "perplexity openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "xai",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "xai_openai_http",
        summary: "xai openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "moonshot",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "moonshot_openai_http",
        summary: "moonshot openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "qwen",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "qwen_openai_http",
        summary: "qwen openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "openai-compatible",
        category: IntegrationCategory::AiModel,
        status: IntegrationStatus::Available,
        transport: "openai_compatible_http",
        summary: "custom openai-compatible endpoint",
    },
    IntegrationEntry {
        name: "github",
        category: IntegrationCategory::Productivity,
        status: IntegrationStatus::ComingSoon,
        transport: "github_rest_api",
        summary: "repository and issue workflow automation",
    },
    IntegrationEntry {
        name: "browser",
        category: IntegrationCategory::Platform,
        status: IntegrationStatus::Available,
        transport: "headless_browser",
        summary: "browser automation tool integration",
    },
    IntegrationEntry {
        name: "cron",
        category: IntegrationCategory::Platform,
        status: IntegrationStatus::Active,
        transport: "local_scheduler",
        summary: "local scheduled task execution engine",
    },
];

#[cfg(test)]
mod tests {
    use super::{
        IntegrationCategory, IntegrationStatus, IntegrationsAction, IntegrationsResult,
        execute_integrations_action,
    };

    #[test]
    fn integrations_info_returns_catalog_entry() {
        let result = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("telegram"),
        })
        .expect("telegram should exist");

        match result {
            IntegrationsResult::Info { entry } => {
                assert_eq!(entry.name, "telegram");
                assert_eq!(entry.category, IntegrationCategory::Chat);
                assert_eq!(entry.status, IntegrationStatus::Available);
            }
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn integrations_info_is_case_insensitive() {
        let result = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("OpenAI"),
        })
        .expect("openai should exist");

        match result {
            IntegrationsResult::Info { entry } => {
                assert_eq!(entry.name, "openai");
                assert_eq!(entry.status, IntegrationStatus::Active);
            }
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn integrations_info_includes_extended_ai_provider_catalog() {
        for name in [
            "anthropic",
            "deepseek",
            "groq",
            "mistral",
            "fireworks",
            "together",
            "perplexity",
            "xai",
            "moonshot",
            "qwen",
            "openai-compatible",
        ] {
            let result = execute_integrations_action(IntegrationsAction::Info {
                name: name.to_string(),
            })
            .expect("extended ai integration should exist");

            match result {
                IntegrationsResult::Info { entry } => {
                    assert_eq!(entry.name, name);
                    assert_eq!(entry.category, IntegrationCategory::AiModel);
                }
                _ => panic!("expected Info"),
            }
        }
    }

    #[test]
    fn integrations_info_rejects_unknown_name() {
        let error = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("unknown-service"),
        })
        .expect_err("unknown integration should fail");

        assert!(error.contains("unknown integration 'unknown-service'"));
    }

    #[test]
    fn integrations_install_returns_instructions() {
        let result = execute_integrations_action(IntegrationsAction::Install {
            name: String::from("telegram"),
        })
        .expect("telegram install should succeed");
        match result {
            IntegrationsResult::Installed { name, instructions } => {
                assert_eq!(name, "telegram");
                assert!(!instructions.is_empty());
            }
            _ => panic!("expected Installed"),
        }
    }

    #[test]
    fn integrations_remove_succeeds_for_known_integration() {
        let result = execute_integrations_action(IntegrationsAction::Remove {
            name: String::from("telegram"),
        })
        .expect("telegram remove should succeed");
        match result {
            IntegrationsResult::Removed { name } => assert_eq!(name, "telegram"),
            _ => panic!("expected Removed"),
        }
    }

    #[test]
    fn integrations_list_returns_all_entries() {
        let result = execute_integrations_action(IntegrationsAction::List)
            .expect("list should succeed");
        match result {
            IntegrationsResult::Listed { entries } => {
                assert!(!entries.is_empty());
                assert!(entries.iter().any(|e| e.name == "telegram"));
            }
            _ => panic!("expected Listed"),
        }
    }

    #[test]
    fn integrations_install_rejects_unknown() {
        let err = execute_integrations_action(IntegrationsAction::Install {
            name: String::from("unknown-xyz"),
        })
        .expect_err("unknown install should fail");
        assert!(err.contains("unknown integration 'unknown-xyz'"));
    }
}
