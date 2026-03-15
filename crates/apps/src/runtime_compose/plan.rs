use crate::cli_command::RunTemplate;
use axonrunner_adapters::{
    RunCommandProfile, WorkflowPackAllowedTool, WorkflowPackContract, WorkflowPackRiskPolicy,
    WorkflowPackVerifierRule, WorkflowPackVerifierStrength,
};
use axonrunner_core::DecisionOutcome;

const MAX_GOAL_PLAN_STEPS: usize = 8;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderPlan {
    pub(super) model: String,
    pub(super) prompt: String,
    pub(super) max_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ToolPlan {
    RunCommands { commands: Vec<ToolCommandPlan> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ToolCommandPlan {
    pub(super) label: String,
    pub(super) program: String,
    pub(super) args: Vec<String>,
    pub(super) expectation: String,
    pub(super) strength: WorkflowPackVerifierStrength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeComposePlan {
    pub(super) workflow_pack: Option<WorkflowPackContract>,
    pub(super) provider: Option<ProviderPlan>,
    pub(super) memory: MemoryPlan,
    pub(super) tool: Option<ToolPlan>,
}

struct DerivedVerifierCommand {
    program: String,
    args: Vec<String>,
    expectation: String,
    strength: WorkflowPackVerifierStrength,
}

pub fn build_runtime_run_plan(
    template: &RunTemplate,
    run_id: &str,
    intent_id: &str,
) -> RuntimeRunPlan {
    let RunTemplate::GoalFile(goal_file) = template;
    build_goal_runtime_run_plan(goal_file, run_id, intent_id)
}

fn build_goal_runtime_run_plan(
    goal_file: &crate::cli_command::GoalFileTemplate,
    run_id: &str,
    intent_id: &str,
) -> RuntimeRunPlan {
    let workflow_pack = goal_workflow_pack(goal_file);
    let mut queue = goal_file
        .goal
        .done_conditions
        .iter()
        .map(|condition| {
            (
                "executing_step",
                format!("advance subgoal `{}`", condition.label),
                format!(
                    "done condition `{}` is advanced toward evidence `{}`",
                    condition.label, condition.evidence
                ),
            )
        })
        .chain(goal_file.goal.verification_checks.iter().map(|check| {
            (
                "verifying",
                format!("run verifier `{}`", check.label),
                format!("verification detail is queued: {}", check.detail),
            )
        }))
        .collect::<Vec<_>>();

    let reserved_tail = 1usize;
    let reserved_compaction = usize::from(queue.len() + 2 > MAX_GOAL_PLAN_STEPS);
    let queue_budget = MAX_GOAL_PLAN_STEPS.saturating_sub(2 + reserved_tail + reserved_compaction);
    let truncated = queue.len() > queue_budget;
    queue.truncate(queue_budget);

    let mut steps = vec![
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
    ];

    for (phase, label, done_when) in queue {
        let index = steps.len() + 1;
        steps.push(plan_step(run_id, index, phase, label, done_when));
    }

    if truncated {
        let index = steps.len() + 1;
        steps.push(plan_step(
            run_id,
            index,
            "planning",
            "compact remaining queue",
            format!(
                "remaining queued work is compacted to stay within {} planned steps",
                MAX_GOAL_PLAN_STEPS
            ),
        ));
    }

    let final_index = steps.len() + 1;
    steps.push(plan_step(
        run_id,
        final_index,
        "executing_step",
        "finalize goal run",
        "goal run records a terminal outcome with a replayable run id",
    ));

    RuntimeRunPlan {
        run_id: run_id.to_owned(),
        goal: goal_file.goal.summary.clone(),
        summary: format!(
            "intent_id={intent_id} goal_file={} workspace_root={} workflow_pack={} verifier_flow={} queued_done_conditions={} queued_verifiers={}",
            goal_file.path,
            goal_file.goal.workspace_root,
            workflow_pack.pack_id,
            render_verifier_flow(&workflow_pack.recommended_verifier_flow),
            goal_file.goal.done_conditions.len(),
            goal_file.goal.verification_checks.len()
        ),
        done_when: goal_file
            .goal
            .done_conditions
            .iter()
            .map(|condition| format!("{}:{}", condition.label, condition.evidence))
            .collect::<Vec<_>>()
            .join(" | "),
        planned_steps: steps.len(),
        steps,
    }
}

pub(super) fn build_runtime_compose_plan(
    template: &RunTemplate,
    outcome: DecisionOutcome,
) -> RuntimeComposePlan {
    if outcome == DecisionOutcome::Rejected {
        return RuntimeComposePlan {
            workflow_pack: None,
            provider: None,
            memory: MemoryPlan::None,
            tool: None,
        };
    }

    let RunTemplate::GoalFile(goal_file) = template;
    RuntimeComposePlan {
        workflow_pack: Some(goal_workflow_pack(goal_file)),
        provider: None,
        memory: MemoryPlan::None,
        tool: None,
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
        recommended_verifier_flow: recommended_goal_verifier_flow(goal_file),
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
        verifier_rules: goal_file
            .goal
            .verification_checks
            .iter()
            .map(|check| {
                let profile = verifier_profile_for_detail(&check.detail);
                let derived = derive_default_verifier_command(&check.label, &check.detail, profile);
                WorkflowPackVerifierRule {
                    label: check.label.clone(),
                    profile,
                    command_example: render_command_example(&derived.program, &derived.args),
                    artifact_expectation: derived.expectation,
                    strength: derived.strength,
                    required: true,
                }
            })
            .collect(),
        risk_policy: WorkflowPackRiskPolicy {
            approval_mode: approval_mode_name(goal_file.goal.approval_mode).to_owned(),
            max_mutating_steps: goal_file.goal.budget.max_steps,
        },
    }
}

fn recommended_goal_verifier_flow(
    goal_file: &crate::cli_command::GoalFileTemplate,
) -> Vec<RunCommandProfile> {
    let mut flow = Vec::new();
    for check in &goal_file.goal.verification_checks {
        let profile = verifier_profile_for_detail(&check.detail);
        if !flow.contains(&profile) {
            flow.push(profile);
        }
    }
    if flow.is_empty() {
        flow.push(RunCommandProfile::Generic);
    }
    flow
}

fn render_verifier_flow(flow: &[RunCommandProfile]) -> String {
    flow.iter()
        .map(|profile| profile.as_str())
        .collect::<Vec<_>>()
        .join(">")
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
    let mut commands = Vec::new();
    let mut used = vec![false; pack.verifier_rules.len()];

    for profile in &pack.recommended_verifier_flow {
        for (index, rule) in pack.verifier_rules.iter().enumerate() {
            if used[index] || rule.profile != *profile {
                continue;
            }
            if let Some(command) = command_plan_from_rule(rule) {
                commands.push(command);
                used[index] = true;
            }
        }
    }
    for (index, rule) in pack.verifier_rules.iter().enumerate() {
        if used[index] {
            continue;
        }
        if let Some(command) = command_plan_from_rule(rule) {
            commands.push(command);
        }
    }
    if commands.is_empty() {
        None
    } else {
        Some(ToolPlan::RunCommands { commands })
    }
}

fn goal_workflow_pack(goal_file: &crate::cli_command::GoalFileTemplate) -> WorkflowPackContract {
    goal_file
        .workflow_pack
        .clone()
        .unwrap_or_else(|| default_goal_workflow_pack(goal_file))
}

fn verifier_profile_for_detail(detail: &str) -> RunCommandProfile {
    let detail = detail.to_ascii_lowercase();
    if detail.contains("clippy") || detail.contains("eslint") || detail.contains("lint") {
        RunCommandProfile::Lint
    } else if detail.contains("build") {
        RunCommandProfile::Build
    } else if detail.contains("test") {
        RunCommandProfile::Test
    } else {
        RunCommandProfile::Generic
    }
}

fn derive_default_verifier_command(
    label: &str,
    detail: &str,
    profile: RunCommandProfile,
) -> DerivedVerifierCommand {
    if let Some((program, args)) = parse_command_detail(detail) {
        return DerivedVerifierCommand {
            program,
            args,
            expectation: format!("detail-derived verifier `{label}` exits 0"),
            strength: WorkflowPackVerifierStrength::Strong,
        };
    }

    let (strength, reason) = unresolved_verifier_strength(label, detail);
    let (program, args) = fallback_verifier_probe(profile);
    DerivedVerifierCommand {
        program: program.to_owned(),
        args: args.into_iter().map(str::to_owned).collect(),
        expectation: format!("{reason} for verifier `{label}`"),
        strength,
    }
}

fn parse_command_detail(detail: &str) -> Option<(String, Vec<String>)> {
    let mut parts = detail
        .split_whitespace()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let program = parts.first()?.clone();
    if !looks_like_command_program(&program) {
        return None;
    }
    let args = parts.split_off(1);
    Some((program, args))
}

fn looks_like_command_program(program: &str) -> bool {
    matches!(
        program,
        "cargo"
            | "npm"
            | "node"
            | "python"
            | "python3"
            | "pytest"
            | "pwd"
            | "rg"
            | "git"
            | "ls"
            | "cat"
            | "sh"
            | "bash"
            | "pnpm"
            | "yarn"
            | "uv"
            | "make"
    ) || program.starts_with("./")
        || program.starts_with("../")
}

fn unresolved_verifier_strength(
    label: &str,
    detail: &str,
) -> (WorkflowPackVerifierStrength, &'static str) {
    let normalized = format!("{label} {detail}").to_ascii_lowercase();
    if detail.trim().is_empty() {
        (
            WorkflowPackVerifierStrength::Unresolved,
            "verification_unresolved fallback probe",
        )
    } else if normalized.contains("pack")
        || normalized.contains("domain")
        || normalized.contains("representative")
    {
        (
            WorkflowPackVerifierStrength::PackRequired,
            "pack_required fallback probe",
        )
    } else {
        (
            WorkflowPackVerifierStrength::Weak,
            "verification_weak fallback probe",
        )
    }
}

fn fallback_verifier_probe(profile: RunCommandProfile) -> (&'static str, Vec<&'static str>) {
    match profile {
        RunCommandProfile::Build => ("rg", vec!["--files", "."]),
        RunCommandProfile::Test => ("rg", vec!["--files", "."]),
        RunCommandProfile::Lint => ("rg", vec!["--files", "."]),
        RunCommandProfile::Generic => ("ls", vec!["."]),
    }
}

fn render_command_example(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        return program.to_owned();
    }
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_plan_from_rule(rule: &WorkflowPackVerifierRule) -> Option<ToolCommandPlan> {
    let mut parts = rule
        .command_example
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let program = parts.first()?.clone();
    let args = parts.split_off(1);
    Some(ToolCommandPlan {
        label: rule.label.clone(),
        program,
        args,
        expectation: rule.artifact_expectation.clone(),
        strength: rule.strength,
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

#[cfg(test)]
mod tests {
    use super::{ToolPlan, build_runtime_compose_plan, build_runtime_run_plan, goal_verifier_tool_plan};
    use crate::cli_command::RunTemplate;
    use axonrunner_adapters::{RunCommandProfile, WorkflowPackVerifierStrength};
    use axonrunner_core::DecisionOutcome;

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
            workflow_pack: None,
        });

        let plan = build_runtime_compose_plan(
            &template,
            DecisionOutcome::Accepted,
        );

        let pack = plan.workflow_pack.expect("workflow pack should exist");
        assert_eq!(pack.pack_id, "goal-default-v1");
        assert_eq!(pack.risk_policy.approval_mode, "on-risk");
        assert_eq!(
            pack.recommended_verifier_flow,
            vec![RunCommandProfile::Test]
        );
        assert_eq!(pack.allowed_tools[0].operation, "read_file");
        assert!(pack.verifier_rules[0].required);
        assert_eq!(
            pack.verifier_rules[0].command_example,
            "cargo test -p axonrunner_apps --test release_security_gate"
        );
        assert_eq!(
            pack.verifier_rules[0].strength,
            WorkflowPackVerifierStrength::Strong
        );
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
            workflow_pack: None,
        });

        let plan = build_runtime_compose_plan(
            &template,
            DecisionOutcome::Accepted,
        );

        let Some(ToolPlan::RunCommands { commands }) = goal_verifier_tool_plan(&plan) else {
            panic!("expected verifier command tool plan");
        };
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].program, "cargo");
        assert_eq!(
            commands[0].args,
            vec![
                String::from("test"),
                String::from("-p"),
                String::from("axonrunner_apps"),
                String::from("--test"),
                String::from("release_security_gate"),
            ]
        );
    }

    #[test]
    fn planner_marks_non_command_default_goal_verifier_as_pack_required() {
        let goal = axonrunner_core::RunGoal {
            summary: String::from("Need domain-specific verification"),
            workspace_root: String::from("/workspace"),
            constraints: Vec::new(),
            done_conditions: vec![axonrunner_core::DoneCondition {
                label: String::from("report"),
                evidence: String::from("report artifact exists"),
            }],
            verification_checks: vec![axonrunner_core::VerificationCheck {
                label: String::from("domain verification"),
                detail: String::from("representative domain path"),
            }],
            budget: axonrunner_core::RunBudget::bounded(5, 10, 8000),
            approval_mode: axonrunner_core::RunApprovalMode::Never,
        };
        let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal,
            workflow_pack: None,
        });

        let plan = build_runtime_compose_plan(
            &template,
            DecisionOutcome::Accepted,
        );

        let pack = plan.workflow_pack.expect("workflow pack should exist");
        assert_eq!(pack.verifier_rules[0].command_example, "ls .");
        assert_eq!(
            pack.verifier_rules[0].strength,
            WorkflowPackVerifierStrength::PackRequired
        );
        assert!(
            pack.verifier_rules[0]
                .artifact_expectation
                .contains("pack_required fallback probe")
        );
    }

    #[test]
    fn planner_infers_pack_specific_verifier_flow_from_goal_checks() {
        let goal = axonrunner_core::RunGoal {
            summary: String::from("Infer verifier flow"),
            workspace_root: String::from("/workspace"),
            constraints: Vec::new(),
            done_conditions: vec![axonrunner_core::DoneCondition {
                label: String::from("report"),
                evidence: String::from("report artifact exists"),
            }],
            verification_checks: vec![
                axonrunner_core::VerificationCheck {
                    label: String::from("build"),
                    detail: String::from("cargo build"),
                },
                axonrunner_core::VerificationCheck {
                    label: String::from("test"),
                    detail: String::from("cargo test"),
                },
                axonrunner_core::VerificationCheck {
                    label: String::from("lint"),
                    detail: String::from("cargo clippy"),
                },
                axonrunner_core::VerificationCheck {
                    label: String::from("smoke"),
                    detail: String::from("python scripts/smoke.py"),
                },
            ],
            budget: axonrunner_core::RunBudget::bounded(8, 10, 8000),
            approval_mode: axonrunner_core::RunApprovalMode::Never,
        };
        let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal,
            workflow_pack: None,
        });

        let plan = build_runtime_compose_plan(&template, DecisionOutcome::Accepted);

        let pack = plan.workflow_pack.expect("workflow pack should exist");
        assert_eq!(
            pack.recommended_verifier_flow,
            vec![
                RunCommandProfile::Build,
                RunCommandProfile::Test,
                RunCommandProfile::Lint,
                RunCommandProfile::Generic,
            ]
        );
    }

    #[test]
    fn planner_builds_multi_step_goal_queue() {
        let goal = axonrunner_core::RunGoal {
            summary: String::from("Ship one bounded goal package"),
            workspace_root: String::from("/workspace"),
            constraints: Vec::new(),
            done_conditions: vec![
                axonrunner_core::DoneCondition {
                    label: String::from("report"),
                    evidence: String::from("report artifact exists"),
                },
                axonrunner_core::DoneCondition {
                    label: String::from("tests"),
                    evidence: String::from("test suite passes"),
                },
            ],
            verification_checks: vec![
                axonrunner_core::VerificationCheck {
                    label: String::from("release gate"),
                    detail: String::from(
                        "cargo test -p axonrunner_apps --test release_security_gate",
                    ),
                },
                axonrunner_core::VerificationCheck {
                    label: String::from("unit"),
                    detail: String::from("cargo test -p axonrunner_apps"),
                },
            ],
            budget: axonrunner_core::RunBudget::bounded(8, 10, 8000),
            approval_mode: axonrunner_core::RunApprovalMode::Never,
        };
        let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal,
            workflow_pack: None,
        });

        let plan = build_runtime_run_plan(&template, "run-1", "cli-1");

        assert_eq!(plan.planned_steps, 7);
        assert_eq!(plan.steps[0].label, "load goal file");
        assert_eq!(plan.steps[1].label, "validate goal contract");
        assert_eq!(plan.steps[2].label, "advance subgoal `report`");
        assert_eq!(plan.steps[3].label, "advance subgoal `tests`");
        assert_eq!(plan.steps[4].label, "run verifier `release gate`");
        assert_eq!(plan.steps[5].label, "run verifier `unit`");
        assert_eq!(plan.steps[6].label, "finalize goal run");
        assert!(plan.summary.contains("queued_done_conditions=2"));
        assert!(plan.summary.contains("queued_verifiers=2"));
    }

    #[test]
    fn planner_compacts_large_goal_queue_to_bounded_size() {
        let goal = axonrunner_core::RunGoal {
            summary: String::from("Compact a large goal queue"),
            workspace_root: String::from("/workspace"),
            constraints: Vec::new(),
            done_conditions: (0..6)
                .map(|index| axonrunner_core::DoneCondition {
                    label: format!("done-{index}"),
                    evidence: format!("evidence-{index}"),
                })
                .collect(),
            verification_checks: (0..6)
                .map(|index| axonrunner_core::VerificationCheck {
                    label: format!("verify-{index}"),
                    detail: format!("echo verify-{index}"),
                })
                .collect(),
            budget: axonrunner_core::RunBudget::bounded(20, 10, 8000),
            approval_mode: axonrunner_core::RunApprovalMode::Never,
        };
        let template = RunTemplate::GoalFile(crate::cli_command::GoalFileTemplate {
            path: String::from("GOAL.json"),
            goal,
            workflow_pack: None,
        });

        let plan = build_runtime_run_plan(&template, "run-2", "cli-2");

        assert_eq!(plan.planned_steps, super::MAX_GOAL_PLAN_STEPS);
        assert!(
            plan.steps
                .iter()
                .any(|step| step.label == "compact remaining queue")
        );
        assert_eq!(
            plan.steps.last().map(|step| step.label.as_str()),
            Some("finalize goal run")
        );
    }
}
