use crate::cli_command::{LegacyIntentTemplate, RunTemplate};
use axonrunner_adapters::{
    RunCommandProfile, WorkflowPackAllowedTool, WorkflowPackContract, WorkflowPackRiskPolicy,
    WorkflowPackVerifierRule,
};
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
pub(super) enum ToolPlan {
    WriteLog {
        path: String,
        line_prefix: String,
    },
    RunCommand {
        label: String,
        program: String,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeComposePlan {
    pub(super) workflow_pack: Option<WorkflowPackContract>,
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
    match template {
        RunTemplate::GoalFile(goal_file) => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: goal_file.goal.summary.clone(),
            summary: format!(
                "intent_id={intent_id} goal_file={} workspace_root={} workflow_pack={}",
                goal_file.path,
                goal_file.goal.workspace_root,
                default_goal_workflow_pack(goal_file).pack_id
            ),
            done_when: goal_file
                .goal
                .done_conditions
                .iter()
                .map(|condition| format!("{}:{}", condition.label, condition.evidence))
                .collect::<Vec<_>>()
                .join(" | "),
            planned_steps: 3,
            steps: vec![
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-1-planning"),
                    label: String::from("load goal file"),
                    phase: "planning",
                    done_when: format!("goal file `{}` is parsed into RunGoal", goal_file.path),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-2-verifying"),
                    label: String::from("validate goal contract"),
                    phase: "verifying",
                    done_when: String::from(
                        "goal summary, workspace, done conditions, verification checks, and budget validate",
                    ),
                },
                RuntimeRunPlanStep {
                    id: format!("{run_id}/step-3-executing"),
                    label: String::from("finalize goal run"),
                    phase: "executing_step",
                    done_when: String::from(
                        "goal run records a terminal outcome with a replayable run id",
                    ),
                },
            ],
        },
        RunTemplate::LegacyIntent(template) => {
            build_legacy_runtime_run_plan(template, run_id, intent_id, outcome)
        }
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
            workflow_pack: None,
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        };
    }

    match template {
        RunTemplate::GoalFile(goal_file) => RuntimeComposePlan {
            workflow_pack: Some(default_goal_workflow_pack(goal_file)),
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        },
        RunTemplate::LegacyIntent(template) => build_legacy_runtime_compose_plan(
            template,
            intent_id,
            provider_model,
            max_tokens,
            tool_log_path,
        ),
    }
}

fn build_legacy_runtime_run_plan(
    template: &LegacyIntentTemplate,
    run_id: &str,
    intent_id: &str,
    outcome: DecisionOutcome,
) -> RuntimeRunPlan {
    match template {
        LegacyIntentTemplate::Read { key } => RuntimeRunPlan {
            run_id: run_id.to_owned(),
            goal: format!("Read fact `{key}` from persisted runtime state"),
            summary: format!("intent_id={intent_id} legacy_read key={key}"),
            done_when: format!("query response for `{key}` is emitted with revision context"),
            planned_steps: 2,
            steps: vec![
                plan_step(
                    run_id,
                    1,
                    "planning",
                    "select persisted fact",
                    format!("read target `{key}` is identified"),
                ),
                plan_step(
                    run_id,
                    2,
                    "verifying",
                    "emit query result",
                    "stdout contains the resolved value",
                ),
            ],
        },
        LegacyIntentTemplate::Write { key, value } => legacy_mutation_run_plan(
            run_id,
            intent_id,
            outcome,
            key,
            format!("Write fact `{key}` into bounded runtime state"),
            format!("fact `{key}` persists with value `{value}` and mutation evidence is recorded"),
            "write",
            "verify persisted change",
            format!("verification confirms write `{key}` completed without hidden failure"),
        ),
        LegacyIntentTemplate::Remove { key } => legacy_mutation_run_plan(
            run_id,
            intent_id,
            outcome,
            key,
            format!("Remove fact `{key}` from bounded runtime state"),
            format!("fact `{key}` is removed and removal evidence is recorded"),
            "remove",
            "verify removed state",
            format!("verification confirms remove `{key}` completed without hidden failure"),
        ),
        LegacyIntentTemplate::Freeze => legacy_control_run_plan(
            run_id,
            intent_id,
            outcome,
            "freeze",
            "Freeze future fact writes",
            "runtime enters read_only mode with explicit policy evidence",
            "apply mode transition",
            "state mode becomes read_only when accepted",
            "verify control state",
            "verification records accepted or blocked control outcome",
        ),
        LegacyIntentTemplate::Halt => legacy_control_run_plan(
            run_id,
            intent_id,
            outcome,
            "halt",
            "Halt future runtime mutations",
            "runtime enters halted mode with explicit policy evidence",
            "apply halt transition",
            "state mode becomes halted when accepted",
            "verify halted state",
            "verification records accepted or blocked halt outcome",
        ),
    }
}

fn legacy_mutation_run_plan(
    run_id: &str,
    intent_id: &str,
    outcome: DecisionOutcome,
    key: &str,
    goal: String,
    done_when: String,
    kind: &str,
    verify_label: &str,
    verify_done_when: String,
) -> RuntimeRunPlan {
    RuntimeRunPlan {
        run_id: run_id.to_owned(),
        goal,
        summary: format!(
            "intent_id={intent_id} legacy_{kind} key={key} outcome={}",
            outcome_name(outcome)
        ),
        done_when,
        planned_steps: 4,
        steps: vec![
            plan_step(
                run_id,
                1,
                "planning",
                format!("prepare bounded {kind}"),
                format!("{kind} target and evidence path are fixed"),
            ),
            plan_step(
                run_id,
                2,
                "executing_step",
                "execute provider step",
                "provider response is captured or explicitly skipped",
            ),
            plan_step(
                run_id,
                3,
                "executing_step",
                "apply memory and tool mutation",
                format!("memory/tool evidence exists for the accepted {kind}"),
            ),
            plan_step(run_id, 4, "verifying", verify_label, verify_done_when),
        ],
    }
}

fn legacy_control_run_plan(
    run_id: &str,
    intent_id: &str,
    outcome: DecisionOutcome,
    kind: &str,
    goal: &str,
    done_when: &str,
    execute_label: &str,
    execute_done_when: &str,
    verify_label: &str,
    verify_done_when: &str,
) -> RuntimeRunPlan {
    RuntimeRunPlan {
        run_id: run_id.to_owned(),
        goal: String::from(goal),
        summary: format!(
            "intent_id={intent_id} legacy_{kind} outcome={}",
            outcome_name(outcome)
        ),
        done_when: String::from(done_when),
        planned_steps: 3,
        steps: vec![
            plan_step(
                run_id,
                1,
                "planning",
                "validate control action",
                "control action actor is evaluated",
            ),
            plan_step(
                run_id,
                2,
                "executing_step",
                execute_label,
                execute_done_when,
            ),
            plan_step(run_id, 3, "verifying", verify_label, verify_done_when),
        ],
    }
}

fn build_legacy_runtime_compose_plan(
    template: &LegacyIntentTemplate,
    intent_id: &str,
    provider_model: &str,
    max_tokens: usize,
    tool_log_path: &str,
) -> RuntimeComposePlan {
    match template {
        LegacyIntentTemplate::Write { key, value } => legacy_mutation_compose_plan(
            intent_id,
            provider_model,
            max_tokens,
            tool_log_path,
            "write",
            key,
            Some(value),
        ),
        LegacyIntentTemplate::Remove { key } => legacy_mutation_compose_plan(
            intent_id,
            provider_model,
            max_tokens,
            tool_log_path,
            "remove",
            key,
            None,
        ),
        LegacyIntentTemplate::Read { .. }
        | LegacyIntentTemplate::Freeze
        | LegacyIntentTemplate::Halt => RuntimeComposePlan {
            workflow_pack: None,
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        },
    }
}

fn legacy_mutation_compose_plan(
    intent_id: &str,
    provider_model: &str,
    max_tokens: usize,
    tool_log_path: &str,
    kind: &str,
    key: &str,
    value: Option<&str>,
) -> RuntimeComposePlan {
    let prompt = match value {
        Some(value) => format!("intent={intent_id} kind={kind} key={key} value={value}"),
        None => format!("intent={intent_id} kind={kind} key={key}"),
    };

    RuntimeComposePlan {
        workflow_pack: None,
        provider: Some(ProviderPlan {
            model: provider_model.to_owned(),
            prompt,
            max_tokens,
        }),
        memory: match value {
            Some(value) => MemoryPlan::Put {
                key: key.to_owned(),
                value: value.to_owned(),
            },
            None => MemoryPlan::Remove {
                key: key.to_owned(),
            },
        },
        tool: Some(ToolPlan::WriteLog {
            path: tool_log_path.to_owned(),
            line_prefix: format!("intent={intent_id} kind={kind} key={key}"),
        }),
    }
}

fn default_goal_workflow_pack(
    goal_file: &crate::cli_command::GoalFileTemplate,
) -> WorkflowPackContract {
    WorkflowPackContract {
        pack_id: String::from("goal-default-v1"),
        version: String::from("1"),
        description: String::from("default bounded goal execution pack"),
        entry_goal: goal_file.goal.summary.clone(),
        planner_hints: vec![
            String::from("prefer workspace-bounded verification"),
            String::from("preserve replayable evidence for every goal run"),
        ],
        allowed_tools: vec![
            WorkflowPackAllowedTool {
                operation: String::from("read_file"),
                scope: String::from("workspace"),
            },
            WorkflowPackAllowedTool {
                operation: String::from("search_files"),
                scope: String::from("workspace"),
            },
            WorkflowPackAllowedTool {
                operation: String::from("file_write"),
                scope: String::from("workspace"),
            },
            WorkflowPackAllowedTool {
                operation: String::from("replace_in_file"),
                scope: String::from("workspace"),
            },
            WorkflowPackAllowedTool {
                operation: String::from("run_command"),
                scope: String::from("workspace"),
            },
        ],
        verifier_rules: vec![WorkflowPackVerifierRule {
            label: String::from("workspace verifier"),
            profile: RunCommandProfile::Generic,
            command_example: String::from("pwd"),
            artifact_expectation: goal_file
                .goal
                .verification_checks
                .first()
                .map(|check| check.detail.clone())
                .unwrap_or_else(|| String::from("verify/report artifact exists")),
            required: true,
        }],
        risk_policy: WorkflowPackRiskPolicy {
            approval_mode: approval_mode_name(goal_file.goal.approval_mode).to_owned(),
            max_mutating_steps: goal_file.goal.budget.max_steps,
        },
    }
}

fn approval_mode_name(mode: axonrunner_core::RunApprovalMode) -> &'static str {
    match mode {
        axonrunner_core::RunApprovalMode::Never => "never",
        axonrunner_core::RunApprovalMode::OnRisk => "on-risk",
        axonrunner_core::RunApprovalMode::Always => "always",
    }
}

pub(super) fn goal_verifier_tool_plan(plan: &RuntimeComposePlan) -> Option<ToolPlan> {
    let pack = plan.workflow_pack.as_ref()?;
    let rule = pack.verifier_rules.iter().find(|rule| rule.required)?;
    let mut parts = rule
        .command_example
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let program = parts.first()?.clone();
    let args = parts.split_off(1);
    Some(ToolPlan::RunCommand {
        label: rule.label.clone(),
        program,
        args,
    })
}

fn plan_step(
    run_id: &str,
    index: usize,
    phase: &'static str,
    label: impl Into<String>,
    done_when: impl Into<String>,
) -> RuntimeRunPlanStep {
    let phase_id = match phase {
        "executing_step" => "executing",
        other => other,
    };
    RuntimeRunPlanStep {
        id: format!("{run_id}/step-{index}-{phase_id}"),
        label: label.into(),
        phase,
        done_when: done_when.into(),
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
    use super::{
        ToolPlan, build_runtime_compose_plan, build_runtime_run_plan, goal_verifier_tool_plan,
    };
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

    #[test]
    fn planner_resolves_default_goal_workflow_pack() {
        let goal = axonrunner_core::RunGoal {
            summary: String::from("Ship one bounded goal package"),
            workspace_root: String::from("/workspace"),
            constraints: Vec::new(),
            done_conditions: vec![axonrunner_core::DoneCondition {
                label: String::from("report"),
                evidence: String::from("report artifact exists"),
            }],
            verification_checks: vec![axonrunner_core::VerificationCheck {
                label: String::from("release gate"),
                detail: String::from("cargo test -p axonrunner_apps --test release_security_gate"),
            }],
            budget: axonrunner_core::RunBudget::bounded(5, 10, 8000),
            approval_mode: axonrunner_core::RunApprovalMode::OnRisk,
        };
        let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal,
        });

        let plan = build_runtime_compose_plan(
            &template,
            "cli-1",
            DecisionOutcome::Accepted,
            "mock-local",
            256,
            "runtime.log",
        );

        let pack = plan.workflow_pack.expect("workflow pack should exist");
        assert_eq!(pack.pack_id, "goal-default-v1");
        assert_eq!(pack.risk_policy.approval_mode, "on-risk");
        assert_eq!(pack.allowed_tools[0].operation, "read_file");
        assert_eq!(pack.verifier_rules[0].required, true);
        assert_eq!(pack.verifier_rules[0].command_example, "pwd");
    }

    #[test]
    fn planner_derives_goal_verifier_command_plan() {
        let goal = axonrunner_core::RunGoal {
            summary: String::from("Ship one bounded goal package"),
            workspace_root: String::from("/workspace"),
            constraints: Vec::new(),
            done_conditions: vec![axonrunner_core::DoneCondition {
                label: String::from("report"),
                evidence: String::from("report artifact exists"),
            }],
            verification_checks: vec![axonrunner_core::VerificationCheck {
                label: String::from("release gate"),
                detail: String::from("cargo test -p axonrunner_apps --test release_security_gate"),
            }],
            budget: axonrunner_core::RunBudget::bounded(5, 10, 8000),
            approval_mode: axonrunner_core::RunApprovalMode::Never,
        };
        let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal,
        });

        let plan = build_runtime_compose_plan(
            &template,
            "cli-1",
            DecisionOutcome::Accepted,
            "mock-local",
            256,
            "runtime.log",
        );

        let Some(ToolPlan::RunCommand { program, args, .. }) = goal_verifier_tool_plan(&plan)
        else {
            panic!("expected verifier command tool plan");
        };
        assert_eq!(program, "pwd");
        assert!(args.is_empty());
    }
}
