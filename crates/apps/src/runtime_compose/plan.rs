use crate::cli_command::{LegacyIntentTemplate, RunTemplate};
use axonrunner_core::DecisionOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunPlanStep {
    pub id: String,
    pub label: String,
    pub phase: &'static str,
    pub done_when: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRunPlan {
    pub run_id: String,
    pub goal: String,
    pub summary: String,
    pub done_when: String,
    pub planned_steps: usize,
    pub steps: Vec<RuntimeRunPlanStep>,
}

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

pub fn build_runtime_run_plan(
    template: &RunTemplate,
    run_id: &str,
    intent_id: &str,
    outcome: DecisionOutcome,
) -> RuntimeRunPlan {
    match template.legacy_intent() {
        LegacyIntentTemplate::Read { key } => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: format!("Read fact `{key}` from persisted runtime state"),
            summary: format!("intent_id={intent_id} legacy_read key={key}"),
            done_when: format!("query response for `{key}` is emitted with revision context"),
            planned_steps: 2,
            steps: vec![
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-1-planning"),
                    label: String::from("select persisted fact"),
                    phase: "planning",
                    done_when: format!("read target `{key}` is identified"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-2-verifying"),
                    label: String::from("emit query result"),
                    phase: "verifying",
                    done_when: String::from("stdout contains the resolved value"),
                },
            ],
        },
        LegacyIntentTemplate::Write { key, value } => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: format!("Write fact `{key}` into bounded runtime state"),
            summary: format!("intent_id={intent_id} legacy_write key={key} outcome={}", outcome_name(outcome)),
            done_when: format!("fact `{key}` persists with value `{value}` and mutation evidence is recorded"),
            planned_steps: 4,
            steps: vec![
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-1-planning"),
                    label: String::from("prepare bounded write"),
                    phase: "planning",
                    done_when: String::from("write target and evidence path are fixed"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-2-executing"),
                    label: String::from("execute provider step"),
                    phase: "executing_step",
                    done_when: String::from("provider response is captured or explicitly skipped"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-3-executing"),
                    label: String::from("apply memory and tool mutation"),
                    phase: "executing_step",
                    done_when: String::from("memory/tool evidence exists for the accepted write"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-4-verifying"),
                    label: String::from("verify persisted change"),
                    phase: "verifying",
                    done_when: format!("verification confirms write `{key}` completed without hidden failure"),
                },
            ],
        },
        LegacyIntentTemplate::Remove { key } => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: format!("Remove fact `{key}` from bounded runtime state"),
            summary: format!(
                "intent_id={intent_id} legacy_remove key={key} outcome={}",
                outcome_name(outcome)
            ),
            done_when: format!("fact `{key}` is removed and removal evidence is recorded"),
            planned_steps: 4,
            steps: vec![
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-1-planning"),
                    label: String::from("prepare bounded remove"),
                    phase: "planning",
                    done_when: String::from("remove target and evidence path are fixed"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-2-executing"),
                    label: String::from("execute provider step"),
                    phase: "executing_step",
                    done_when: String::from("provider response is captured or explicitly skipped"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-3-executing"),
                    label: String::from("apply memory and tool mutation"),
                    phase: "executing_step",
                    done_when: String::from("memory/tool evidence exists for the accepted remove"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-4-verifying"),
                    label: String::from("verify removed state"),
                    phase: "verifying",
                    done_when: format!("verification confirms remove `{key}` completed without hidden failure"),
                },
            ],
        },
        LegacyIntentTemplate::Freeze => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: String::from("Freeze future fact writes"),
            summary: format!("intent_id={intent_id} legacy_freeze outcome={}", outcome_name(outcome)),
            done_when: String::from("runtime enters read_only mode with explicit policy evidence"),
            planned_steps: 3,
            steps: vec![
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-1-planning"),
                    label: String::from("validate control action"),
                    phase: "planning",
                    done_when: String::from("control action actor is evaluated"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-2-executing"),
                    label: String::from("apply mode transition"),
                    phase: "executing_step",
                    done_when: String::from("state mode becomes read_only when accepted"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-3-verifying"),
                    label: String::from("verify control state"),
                    phase: "verifying",
                    done_when: String::from("verification records accepted or blocked control outcome"),
                },
            ],
        },
        LegacyIntentTemplate::Halt => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: String::from("Halt future runtime mutations"),
            summary: format!("intent_id={intent_id} legacy_halt outcome={}", outcome_name(outcome)),
            done_when: String::from("runtime enters halted mode with explicit policy evidence"),
            planned_steps: 3,
            steps: vec![
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-1-planning"),
                    label: String::from("validate control action"),
                    phase: "planning",
                    done_when: String::from("control action actor is evaluated"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-2-executing"),
                    label: String::from("apply halt transition"),
                    phase: "executing_step",
                    done_when: String::from("state mode becomes halted when accepted"),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-3-verifying"),
                    label: String::from("verify halted state"),
                    phase: "verifying",
                    done_when: String::from("verification records accepted or blocked halt outcome"),
                },
            ],
        },
    }
}

pub(super) fn build_runtime_compose_plan(
    template: &RunTemplate,
    intent_id: &str,
    outcome: DecisionOutcome,
    provider_model: &str,
    max_tokens: usize,
    tool_log_path: &str,
) -> RuntimeComposePlan {
    if outcome == DecisionOutcome::Rejected {
        return RuntimeComposePlan {
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        };
    }

    match template.legacy_intent() {
        LegacyIntentTemplate::Write { key, value } => RuntimeComposePlan {
            provider: Some(ProviderPlan {
                model: provider_model.to_owned(),
                prompt: format!("intent={intent_id} kind=write key={key} value={value}"),
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
        LegacyIntentTemplate::Remove { key } => RuntimeComposePlan {
            provider: Some(ProviderPlan {
                model: provider_model.to_owned(),
                prompt: format!("intent={intent_id} kind=remove key={key}"),
                max_tokens,
            }),
            memory: MemoryPlan::Remove { key: key.clone() },
            tool: Some(ToolPlan {
                path: tool_log_path.to_owned(),
                line_prefix: format!("intent={intent_id} kind=remove key={key}"),
            }),
        },
        LegacyIntentTemplate::Read { .. }
        | LegacyIntentTemplate::Freeze
        | LegacyIntentTemplate::Halt => {
            RuntimeComposePlan {
                provider: None,
                memory: MemoryPlan::None,
                tool: None,
            }
        }
    }
}

fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}

#[cfg(test)]
mod tests {
    use super::build_runtime_run_plan;
    use crate::cli_command::{LegacyIntentTemplate, RunTemplate};
    use axonrunner_core::DecisionOutcome;

    #[test]
    fn planner_builds_bounded_write_plan() {
        let plan = build_runtime_run_plan(
            &RunTemplate::LegacyIntent(LegacyIntentTemplate::Write {
                key: String::from("alpha"),
                value: String::from("42"),
            }),
            "run-7",
            "cli-7",
            DecisionOutcome::Accepted,
        );

        assert_eq!(plan.run_id, "run-7");
        assert_eq!(plan.goal, "Write fact `alpha` into bounded runtime state");
        assert_eq!(plan.planned_steps, 4);
        assert!(plan.summary.contains("legacy_write"));
        assert!(plan.done_when.contains("mutation evidence"));
        assert_eq!(plan.steps[0].id, "run-7/step-1-planning");
        assert_eq!(plan.steps[0].phase, "planning");
        assert_eq!(plan.steps.last().map(|step| step.phase), Some("verifying"));
    }

    #[test]
    fn planner_builds_control_plan_without_mutation_steps() {
        let plan = build_runtime_run_plan(
            &RunTemplate::LegacyIntent(LegacyIntentTemplate::Freeze),
            "run-8",
            "cli-8",
            DecisionOutcome::Accepted,
        );

        assert_eq!(plan.run_id, "run-8");
        assert_eq!(plan.planned_steps, 3);
        assert!(plan.goal.contains("Freeze"));
        assert!(plan.steps.iter().all(|step| step.phase != "repairing"));
        assert!(plan.done_when.contains("read_only"));
    }
}
