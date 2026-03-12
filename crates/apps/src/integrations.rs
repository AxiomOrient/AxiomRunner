use axonrunner_adapters::{channel_is_compiled, resolve_provider_id, resolve_tool_id};

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
    Partial,
    ComingSoon,
}

impl IntegrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrationStatus::Active => "active",
            IntegrationStatus::Available => "available",
            IntegrationStatus::Partial => "partial",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegrationDescriptor {
    name: &'static str,
    category: IntegrationCategory,
    capability: IntegrationCapability,
    transport: &'static str,
    summary: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegrationCapability {
    Static(IntegrationStatus),
    Local(IntegrationStatus),
    Channel {
        id: &'static str,
        executable_status: IntegrationStatus,
    },
    Provider {
        id: &'static str,
        executable_status: IntegrationStatus,
    },
    Tool {
        id: &'static str,
        executable_status: IntegrationStatus,
    },
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
    Info {
        entry: IntegrationEntry,
    },
    Installed {
        name: String,
        instructions: Vec<String>,
    },
    RemovalPlanned {
        name: String,
        instructions: Vec<String>,
    },
    Listed {
        entries: Vec<IntegrationEntry>,
    },
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
            let entry =
                find_integration(&name).ok_or_else(|| format!("unknown integration '{name}'"))?;
            let instructions = build_install_instructions(&entry);
            Ok(IntegrationsResult::Installed { name, instructions })
        }
        IntegrationsAction::Remove { name } => {
            let entry =
                find_integration(&name).ok_or_else(|| format!("unknown integration '{name}'"))?;
            let instructions = build_remove_instructions(&entry);
            Ok(IntegrationsResult::RemovalPlanned { name, instructions })
        }
        IntegrationsAction::List => {
            let entries = integration_catalog();
            Ok(IntegrationsResult::Listed { entries })
        }
    }
}

fn find_integration(name: &str) -> Option<IntegrationEntry> {
    INTEGRATION_CATALOG
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .map(build_integration_entry)
}

fn integration_catalog() -> Vec<IntegrationEntry> {
    INTEGRATION_CATALOG
        .iter()
        .map(build_integration_entry)
        .collect()
}

fn build_integration_entry(descriptor: &IntegrationDescriptor) -> IntegrationEntry {
    IntegrationEntry {
        name: descriptor.name,
        category: descriptor.category,
        status: resolve_integration_status(descriptor.capability),
        transport: descriptor.transport,
        summary: descriptor.summary,
    }
}

fn resolve_integration_status(capability: IntegrationCapability) -> IntegrationStatus {
    match capability {
        IntegrationCapability::Static(status) | IntegrationCapability::Local(status) => status,
        IntegrationCapability::Channel {
            id,
            executable_status,
        } => {
            if is_channel_executable(id) {
                executable_status
            } else {
                IntegrationStatus::ComingSoon
            }
        }
        IntegrationCapability::Provider {
            id,
            executable_status,
        } => {
            if is_provider_executable(id) {
                executable_status
            } else {
                IntegrationStatus::ComingSoon
            }
        }
        IntegrationCapability::Tool {
            id,
            executable_status,
        } => {
            if is_tool_executable(id) {
                executable_status
            } else {
                IntegrationStatus::ComingSoon
            }
        }
    }
}

fn is_channel_executable(channel_id: &str) -> bool {
    channel_is_compiled(channel_id)
}

fn is_provider_executable(provider_id: &str) -> bool {
    resolve_provider_id(provider_id).is_some()
}

fn is_tool_executable(tool_id: &str) -> bool {
    resolve_tool_id(tool_id).is_some()
}

fn build_install_instructions(entry: &IntegrationEntry) -> Vec<String> {
    match entry.category {
        IntegrationCategory::Chat => vec![
            format!("run: axonrunner channel add --type {}", entry.name),
            String::from("configure bot token in environment"),
        ],
        IntegrationCategory::AiModel => vec![
            format!("set AXONRUNNER_RUNTIME_PROVIDER={}", entry.name),
            String::from("set api key in environment"),
        ],
        _ => vec![format!("see documentation for {}", entry.name)],
    }
}

fn build_remove_instructions(entry: &IntegrationEntry) -> Vec<String> {
    match entry.category {
        IntegrationCategory::Chat => vec![
            format!("run: axonrunner channel remove --name {}", entry.name),
            String::from("delete channel token from environment"),
        ],
        IntegrationCategory::AiModel => vec![
            String::from("unset AXONRUNNER_RUNTIME_PROVIDER when this provider is active"),
            String::from("remove provider api key from environment"),
        ],
        _ => vec![format!(
            "remove {} integration via product-specific settings",
            entry.name
        )],
    }
}

const INTEGRATION_CATALOG: [IntegrationDescriptor; 23] = [
    IntegrationDescriptor {
        name: "telegram",
        category: IntegrationCategory::Chat,
        capability: IntegrationCapability::Channel {
            id: "telegram",
            executable_status: IntegrationStatus::Available,
        },
        transport: "telegram_polling",
        summary: "telegram bot api long-polling channel",
    },
    IntegrationDescriptor {
        name: "discord",
        category: IntegrationCategory::Chat,
        capability: IntegrationCapability::Channel {
            id: "discord",
            executable_status: IntegrationStatus::Partial,
        },
        transport: "discord_webhook_send_only",
        summary: "discord send-only webhook adapter (gateway receive not implemented)",
    },
    IntegrationDescriptor {
        name: "slack",
        category: IntegrationCategory::Chat,
        capability: IntegrationCapability::Channel {
            id: "slack",
            executable_status: IntegrationStatus::Partial,
        },
        transport: "slack_webhook_send_only",
        summary: "slack send-only incoming webhook adapter (event api receive not implemented)",
    },
    IntegrationDescriptor {
        name: "matrix",
        category: IntegrationCategory::Chat,
        capability: IntegrationCapability::Channel {
            id: "matrix",
            executable_status: IntegrationStatus::Available,
        },
        transport: "matrix_sync_api",
        summary: "matrix room messaging channel adapter",
    },
    IntegrationDescriptor {
        name: "whatsapp",
        category: IntegrationCategory::Chat,
        capability: IntegrationCapability::Channel {
            id: "whatsapp",
            executable_status: IntegrationStatus::Partial,
        },
        transport: "whatsapp_cloud_api_send_only",
        summary: "whatsapp business cloud send-only adapter (webhook receive not implemented)",
    },
    IntegrationDescriptor {
        name: "irc",
        category: IntegrationCategory::Chat,
        capability: IntegrationCapability::Channel {
            id: "irc",
            executable_status: IntegrationStatus::Available,
        },
        transport: "irc_socket_transport",
        summary: "irc channel transport adapter",
    },
    IntegrationDescriptor {
        name: "openai",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "openai",
            executable_status: IntegrationStatus::Active,
        },
        transport: "openai_compatible_http",
        summary: "openai-compatible provider backend",
    },
    IntegrationDescriptor {
        name: "openrouter",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "openrouter",
            executable_status: IntegrationStatus::Available,
        },
        transport: "openrouter_http",
        summary: "multi-model router provider endpoint",
    },
    IntegrationDescriptor {
        name: "anthropic",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "anthropic",
            executable_status: IntegrationStatus::Available,
        },
        transport: "anthropic_compatible_http",
        summary: "anthropic-compatible provider backend",
    },
    IntegrationDescriptor {
        name: "deepseek",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "deepseek",
            executable_status: IntegrationStatus::Available,
        },
        transport: "deepseek_openai_http",
        summary: "deepseek openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "groq",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "groq",
            executable_status: IntegrationStatus::Available,
        },
        transport: "groq_openai_http",
        summary: "groq openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "mistral",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "mistral",
            executable_status: IntegrationStatus::Available,
        },
        transport: "mistral_openai_http",
        summary: "mistral openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "fireworks",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "fireworks",
            executable_status: IntegrationStatus::Available,
        },
        transport: "fireworks_openai_http",
        summary: "fireworks openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "together",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "together",
            executable_status: IntegrationStatus::Available,
        },
        transport: "together_openai_http",
        summary: "together openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "perplexity",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "perplexity",
            executable_status: IntegrationStatus::Available,
        },
        transport: "perplexity_openai_http",
        summary: "perplexity openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "xai",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "xai",
            executable_status: IntegrationStatus::Available,
        },
        transport: "xai_openai_http",
        summary: "xai openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "moonshot",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "moonshot",
            executable_status: IntegrationStatus::Available,
        },
        transport: "moonshot_openai_http",
        summary: "moonshot openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "qwen",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "qwen",
            executable_status: IntegrationStatus::Available,
        },
        transport: "qwen_openai_http",
        summary: "qwen openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "openai-compatible",
        category: IntegrationCategory::AiModel,
        capability: IntegrationCapability::Provider {
            id: "openai-compatible",
            executable_status: IntegrationStatus::Available,
        },
        transport: "openai_compatible_http",
        summary: "custom openai-compatible endpoint",
    },
    IntegrationDescriptor {
        name: "github",
        category: IntegrationCategory::Productivity,
        capability: IntegrationCapability::Static(IntegrationStatus::ComingSoon),
        transport: "github_rest_api",
        summary: "repository and issue workflow automation",
    },
    IntegrationDescriptor {
        name: "browser",
        category: IntegrationCategory::Platform,
        capability: IntegrationCapability::Tool {
            id: "browser",
            executable_status: IntegrationStatus::Available,
        },
        transport: "headless_browser",
        summary: "browser automation tool integration",
    },
    IntegrationDescriptor {
        name: "composio",
        category: IntegrationCategory::Platform,
        capability: IntegrationCapability::Tool {
            id: "composio",
            executable_status: IntegrationStatus::Available,
        },
        transport: "composio_rest_api",
        summary: "composio tool execution and integration platform",
    },
    IntegrationDescriptor {
        name: "cron",
        category: IntegrationCategory::Platform,
        capability: IntegrationCapability::Local(IntegrationStatus::Active),
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

    const STATUS_SNAPSHOT_BEGIN: &str = "<!-- INTEGRATIONS_STATUS_SNAPSHOT:BEGIN -->";
    const STATUS_SNAPSHOT_END: &str = "<!-- INTEGRATIONS_STATUS_SNAPSHOT:END -->";

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
    fn integrations_info_marks_anthropic_available_when_provider_is_supported() {
        let result = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("anthropic"),
        })
        .expect("anthropic integration should exist");

        match result {
            IntegrationsResult::Info { entry } => {
                assert_eq!(entry.name, "anthropic");
                assert_eq!(entry.category, IntegrationCategory::AiModel);
                assert_eq!(entry.status, IntegrationStatus::Available);
            }
            _ => panic!("expected Info"),
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
    fn integrations_info_marks_discord_partial_for_send_only_capability() {
        let result = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("discord"),
        })
        .expect("discord integration should exist");

        match result {
            IntegrationsResult::Info { entry } => {
                assert_eq!(entry.name, "discord");
                assert_eq!(entry.category, IntegrationCategory::Chat);
                assert_eq!(entry.status, IntegrationStatus::Partial);
            }
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn integrations_info_marks_slack_partial_for_send_only_capability() {
        let result = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("slack"),
        })
        .expect("slack integration should exist");

        match result {
            IntegrationsResult::Info { entry } => {
                assert_eq!(entry.name, "slack");
                assert_eq!(entry.category, IntegrationCategory::Chat);
                assert_eq!(entry.status, IntegrationStatus::Partial);
            }
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn integrations_info_marks_whatsapp_partial_for_webhook_receive_gap() {
        let result = execute_integrations_action(IntegrationsAction::Info {
            name: String::from("whatsapp"),
        })
        .expect("whatsapp integration should exist");

        match result {
            IntegrationsResult::Info { entry } => {
                assert_eq!(entry.name, "whatsapp");
                assert_eq!(entry.category, IntegrationCategory::Chat);
                assert_eq!(entry.status, IntegrationStatus::Partial);
            }
            _ => panic!("expected Info"),
        }
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
    fn integrations_install_for_ai_model_uses_runtime_provider_env() {
        let result = execute_integrations_action(IntegrationsAction::Install {
            name: String::from("openai"),
        })
        .expect("openai install should succeed");

        match result {
            IntegrationsResult::Installed { instructions, .. } => {
                assert!(
                    instructions
                        .iter()
                        .any(|line| line.contains("AXONRUNNER_RUNTIME_PROVIDER=openai")),
                    "instructions should reference AXONRUNNER_RUNTIME_PROVIDER, got: {instructions:?}"
                );
            }
            _ => panic!("expected Installed"),
        }
    }

    #[test]
    fn integrations_remove_returns_removal_plan() {
        let result = execute_integrations_action(IntegrationsAction::Remove {
            name: String::from("telegram"),
        })
        .expect("telegram remove should succeed");
        match result {
            IntegrationsResult::RemovalPlanned { name, instructions } => {
                assert_eq!(name, "telegram");
                assert!(!instructions.is_empty());
            }
            _ => panic!("expected RemovalPlanned"),
        }
    }

    #[test]
    fn integrations_remove_for_ai_model_uses_runtime_provider_env() {
        let result = execute_integrations_action(IntegrationsAction::Remove {
            name: String::from("openrouter"),
        })
        .expect("openrouter remove should succeed");

        match result {
            IntegrationsResult::RemovalPlanned { instructions, .. } => {
                assert!(
                    instructions
                        .iter()
                        .any(|line| line.contains("AXONRUNNER_RUNTIME_PROVIDER")),
                    "instructions should reference AXONRUNNER_RUNTIME_PROVIDER, got: {instructions:?}"
                );
            }
            _ => panic!("expected RemovalPlanned"),
        }
    }

    #[test]
    fn integrations_list_returns_all_entries() {
        let result =
            execute_integrations_action(IntegrationsAction::List).expect("list should succeed");
        match result {
            IntegrationsResult::Listed { entries } => {
                assert!(!entries.is_empty());
                assert!(entries.iter().any(|e| e.name == "telegram"));
                assert_eq!(entries.len(), 23);
            }
            _ => panic!("expected Listed"),
        }
    }

    #[test]
    fn integrations_list_syncs_status_with_executable_capability() {
        let result =
            execute_integrations_action(IntegrationsAction::List).expect("list should succeed");

        let entries = match result {
            IntegrationsResult::Listed { entries } => entries,
            _ => panic!("expected Listed"),
        };

        let by_name = |name: &str| -> IntegrationStatus {
            entries
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| entry.status)
                .unwrap_or_else(|| panic!("missing integration {name}"))
        };

        assert_eq!(by_name("openai"), IntegrationStatus::Active);
        assert_eq!(by_name("discord"), IntegrationStatus::Partial);
        assert_eq!(by_name("slack"), IntegrationStatus::Partial);
        assert_eq!(by_name("whatsapp"), IntegrationStatus::Partial);
        assert_eq!(by_name("openrouter"), IntegrationStatus::Available);
        assert_eq!(by_name("anthropic"), IntegrationStatus::Available);
        assert_eq!(by_name("deepseek"), IntegrationStatus::ComingSoon);
        assert_eq!(by_name("openai-compatible"), IntegrationStatus::ComingSoon);
        assert_eq!(by_name("browser"), IntegrationStatus::Available);
    }

    #[test]
    fn integrations_install_rejects_unknown() {
        let err = execute_integrations_action(IntegrationsAction::Install {
            name: String::from("unknown-xyz"),
        })
        .expect_err("unknown install should fail");
        assert!(err.contains("unknown integration 'unknown-xyz'"));
    }

    #[test]
    fn integrations_readme_snapshot_is_synced_with_catalog() {
        let expected = expected_status_snapshot();
        let readme = include_str!("../../../README.md");
        let actual = extract_snapshot_block(readme).expect("README snapshot block should exist");
        assert_eq!(actual, expected);
        assert!(
            !readme.contains("browser adapter is a stub"),
            "README should not claim browser is a stub after TASK-C-008"
        );
    }

    #[test]
    fn integrations_deployment_snapshot_is_synced_with_catalog() {
        let expected = expected_status_snapshot();
        let deployment = include_str!("../../../docs/DEPLOYMENT.md");
        let actual =
            extract_snapshot_block(deployment).expect("DEPLOYMENT snapshot block should exist");
        assert_eq!(actual, expected);
        assert!(
            deployment.contains("ANTHROPIC_API_KEY"),
            "deployment guide should document ANTHROPIC_API_KEY"
        );
    }

    fn expected_status_snapshot() -> String {
        let entries = match execute_integrations_action(IntegrationsAction::List)
            .expect("list should succeed")
        {
            IntegrationsResult::Listed { entries } => entries,
            _ => panic!("expected Listed"),
        };

        entries
            .into_iter()
            .map(|entry| {
                format!(
                    "integrations list name={} category={} status={}",
                    entry.name,
                    entry.category.as_str(),
                    entry.status.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn extract_snapshot_block(doc: &str) -> Option<String> {
        let start = doc.find(STATUS_SNAPSHOT_BEGIN)?;
        let start = start + STATUS_SNAPSHOT_BEGIN.len();
        let rest = &doc[start..];
        let end = rest.find(STATUS_SNAPSHOT_END)?;
        Some(rest[..end].trim().to_string())
    }
}
