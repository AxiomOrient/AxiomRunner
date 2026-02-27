use crate::cli_command::IntentTemplate;
use axiom_core::DecisionOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MemoryPlan {
    None,
    Put { key: String, value: String },
    Remove { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderPlan {
    pub(super) model: String,
    pub(super) prompt: String,
    pub(super) max_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ToolPlan {
    pub(super) path: String,
    pub(super) line_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeComposePlan {
    pub(super) provider: Option<ProviderPlan>,
    pub(super) memory: MemoryPlan,
    pub(super) tool: Option<ToolPlan>,
}

pub(super) fn build_runtime_compose_plan(
    template: &IntentTemplate,
    intent_id: &str,
    outcome: DecisionOutcome,
    provider_model: &str,
    max_tokens: usize,
    tool_log_path: &str,
    bootstrap_prompt: Option<&str>,
) -> RuntimeComposePlan {
    if outcome == DecisionOutcome::Rejected {
        return RuntimeComposePlan {
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        };
    }

    match template {
        IntentTemplate::Write { key, value } => RuntimeComposePlan {
            provider: Some(ProviderPlan {
                model: provider_model.to_owned(),
                prompt: compose_provider_prompt(
                    format!("intent={intent_id} kind=write key={key} value={value}"),
                    bootstrap_prompt,
                ),
                max_tokens,
            }),
            memory: MemoryPlan::Put {
                key: key.clone(),
                value: value.clone(),
            },
            tool: Some(ToolPlan {
                path: tool_log_path.to_owned(),
                line_prefix: format!("intent={intent_id} kind=write key={key}"),
            }),
        },
        IntentTemplate::Remove { key } => RuntimeComposePlan {
            provider: Some(ProviderPlan {
                model: provider_model.to_owned(),
                prompt: compose_provider_prompt(
                    format!("intent={intent_id} kind=remove key={key}"),
                    bootstrap_prompt,
                ),
                max_tokens,
            }),
            memory: MemoryPlan::Remove { key: key.clone() },
            tool: Some(ToolPlan {
                path: tool_log_path.to_owned(),
                line_prefix: format!("intent={intent_id} kind=remove key={key}"),
            }),
        },
        IntentTemplate::Read { .. } | IntentTemplate::Freeze | IntentTemplate::Halt => {
            RuntimeComposePlan {
                provider: None,
                memory: MemoryPlan::None,
                tool: None,
            }
        }
    }
}

fn compose_provider_prompt(base_prompt: String, bootstrap_prompt: Option<&str>) -> String {
    let Some(bootstrap_prompt) = bootstrap_prompt else {
        return base_prompt;
    };

    if bootstrap_prompt.is_empty() {
        return base_prompt;
    }

    format!("{base_prompt}\n\nbootstrap_context:\n{bootstrap_prompt}")
}
