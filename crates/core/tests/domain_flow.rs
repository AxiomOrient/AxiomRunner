use axonrunner_core::{
    AgentState, DecisionOutcome, DomainEvent, DoneCondition, ExecutionMode, Intent, PolicyCode,
    RunApprovalMode, RunBudget, RunConstraint, RunEvent, RunGoal, RunGoalValidationError,
    RunOutcome, RunPhase,
    RunStatus, VerificationCheck,
    build_policy_audit, decide, evaluate_policy, project, project_from, project_run,
    reduce, reduce_run_status,
};

fn pipeline_events(state: &AgentState, intent: Intent) -> Vec<DomainEvent> {
    let verdict = evaluate_policy(state, &intent);
    let decision = decide(&intent, &verdict);
    let effects = decision.effects.clone();
    let audit = build_policy_audit(state, &intent, &verdict);

    vec![
        DomainEvent::IntentAccepted { intent },
        DomainEvent::PolicyEvaluated { audit },
        DomainEvent::DecisionCalculated { decision },
        DomainEvent::EffectsApplied { effects },
    ]
}

#[test]
fn policy_denial_audit_contains_required_fields() {
    let state = AgentState {
        revision: 7,
        ..AgentState::default()
    };
    let intent = Intent::freeze_writes("i-deny", Some("gateway".to_owned()));

    let verdict = evaluate_policy(&state, &intent);
    assert!(!verdict.allowed);
    assert_eq!(verdict.code, PolicyCode::UnauthorizedControl);

    let audit = build_policy_audit(&state, &intent, &verdict);
    assert_eq!(audit.intent_id, "i-deny");
    assert_eq!(audit.actor_id.as_deref(), Some("gateway"));
    assert_eq!(audit.state_revision, 7);
    assert!(!audit.allowed);
    assert_eq!(audit.code, PolicyCode::UnauthorizedControl);
    assert_eq!(audit.reason, "control actions require actor `system`");
}

#[test]
fn policy_allow_audit_contains_required_fields() {
    let state = AgentState {
        revision: 3,
        ..AgentState::default()
    };
    let intent = Intent::write("i-allow", Some("alice".to_owned()), "alpha", "1");

    let verdict = evaluate_policy(&state, &intent);
    assert!(verdict.allowed);
    assert_eq!(verdict.code, PolicyCode::Allowed);

    let audit = build_policy_audit(&state, &intent, &verdict);
    assert_eq!(audit.intent_id, "i-allow");
    assert_eq!(audit.actor_id.as_deref(), Some("alice"));
    assert_eq!(audit.state_revision, 3);
    assert!(audit.allowed);
    assert_eq!(audit.code, PolicyCode::Allowed);
    assert_eq!(audit.reason, "allowed");
}

#[test]
fn projection_applies_effects_without_mutating_input_state() {
    let initial = AgentState::default();
    let intent = Intent::write("i-2", Some("alice".to_owned()), "alpha", "42");
    let events = pipeline_events(&initial, intent);

    let next = project(&events);

    assert_eq!(next.facts.get("alpha"), Some(&"42".to_owned()));
    assert_eq!(next.revision, 4);
    assert_eq!(next.last_decision, Some(DecisionOutcome::Accepted));
    assert_eq!(initial.facts.get("alpha"), None);
    assert_eq!(initial.revision, 0);
}

#[test]
fn readonly_mode_blocks_fact_mutation() {
    let state = AgentState {
        mode: ExecutionMode::ReadOnly,
        ..AgentState::default()
    };
    let intent = Intent::write("i-3", Some("alice".to_owned()), "beta", "99");
    let events = pipeline_events(&state, intent);

    let next = project_from(&state, &events);

    assert_eq!(next.facts.get("beta"), None);
    assert_eq!(next.last_policy_code, Some(PolicyCode::ReadOnlyMutation));
    assert_eq!(next.last_decision, Some(DecisionOutcome::Rejected));
    assert_eq!(next.denied_count, 1);
    assert_eq!(next.audit_count, 1);
}

#[test]
fn projection_matches_manual_reduction_fold() {
    let initial = AgentState::default();

    let mut events = pipeline_events(
        &initial,
        Intent::write("i-4", Some("alice".to_owned()), "key", "value"),
    );
    events.extend(pipeline_events(
        &initial,
        Intent::remove("i-5", Some("alice".to_owned()), "key"),
    ));

    let projected = project(&events);
    let folded = events
        .iter()
        .fold(initial.clone(), |state, event| reduce(&state, event));

    assert_eq!(projected, folded);
}

#[test]
fn run_goal_types_capture_autonomous_contract_basics() {
    let goal = RunGoal {
        summary: String::from("Ship goal-oriented autonomous loop"),
        workspace_root: String::from("/workspace"),
        constraints: vec![RunConstraint {
            label: String::from("non-goal"),
            detail: String::from("no multi-agent orchestration"),
        }],
        done_conditions: vec![DoneCondition {
            label: String::from("tests"),
            evidence: String::from("cargo test"),
        }],
        verification_checks: vec![VerificationCheck {
            label: String::from("release gate"),
            detail: String::from("cargo test -p axonrunner_apps --test release_security_gate"),
        }],
        budget: RunBudget::bounded(12, 30, 24_000),
        approval_mode: RunApprovalMode::OnRisk,
    };
    let status = RunStatus::new("run-1", goal.clone());
    let created = RunEvent::RunCreated {
        run_id: String::from("run-1"),
        goal: goal.clone(),
    };
    let completed = RunEvent::OutcomeRecorded {
        run_id: String::from("run-1"),
        outcome: RunOutcome::Success,
    };

    assert_eq!(status.run_id, "run-1");
    assert_eq!(status.phase, RunPhase::Planning);
    assert_eq!(status.outcome, None);
    assert_eq!(status.goal, goal);
    assert_eq!(status.goal.validate(), Ok(()));
    assert_eq!(status.budget.max_steps, 12);
    assert_eq!(created, RunEvent::RunCreated { run_id: String::from("run-1"), goal });
    assert_eq!(
        completed,
        RunEvent::OutcomeRecorded {
            run_id: String::from("run-1"),
            outcome: RunOutcome::Success,
        }
    );
}

#[test]
fn run_goal_validation_rejects_missing_done_conditions_and_zero_budget() {
    let goal = RunGoal {
        summary: String::from("Ship goal package"),
        workspace_root: String::from("/workspace"),
        constraints: vec![RunConstraint {
            label: String::from("non-goal"),
            detail: String::from("no multi-agent orchestration"),
        }],
        done_conditions: Vec::new(),
        verification_checks: vec![VerificationCheck {
            label: String::from("release gate"),
            detail: String::from("cargo test -p axonrunner_apps --test release_security_gate"),
        }],
        budget: RunBudget::bounded(1, 1, 1),
        approval_mode: RunApprovalMode::OnRisk,
    };

    assert_eq!(goal.validate(), Err(RunGoalValidationError::DoneConditionsEmpty));

    let zero_budget_goal = RunGoal {
        summary: String::from("Ship goal package"),
        workspace_root: String::from("/workspace"),
        constraints: Vec::new(),
        done_conditions: vec![DoneCondition {
            label: String::from("report"),
            evidence: String::from("report exists"),
        }],
        verification_checks: vec![VerificationCheck {
            label: String::from("release gate"),
            detail: String::from("cargo test -p axonrunner_apps --test release_security_gate"),
        }],
        budget: RunBudget::bounded(0, 1, 1),
        approval_mode: RunApprovalMode::Never,
    };

    assert_eq!(
        zero_budget_goal.validate(),
        Err(RunGoalValidationError::BudgetStepsZero)
    );
}

#[test]
fn run_lifecycle_projection_tracks_successful_step_flow() {
    let goal = RunGoal {
        summary: String::from("Finish one bounded workspace goal"),
        workspace_root: String::from("/workspace"),
        constraints: Vec::new(),
        done_conditions: vec![DoneCondition {
            label: String::from("verify"),
            evidence: String::from("verification passed"),
        }],
        verification_checks: vec![VerificationCheck {
            label: String::from("verify"),
            detail: String::from("run verifier"),
        }],
        budget: RunBudget::bounded(4, 15, 10_000),
        approval_mode: RunApprovalMode::OnRisk,
    };
    let events = vec![
        RunEvent::RunCreated {
            run_id: String::from("run-2"),
            goal,
        },
        RunEvent::PhaseUpdated {
            run_id: String::from("run-2"),
            phase: RunPhase::ExecutingStep,
        },
        RunEvent::BudgetConsumed {
            run_id: String::from("run-2"),
            consumed_steps: 1,
            consumed_minutes: 2,
            consumed_tokens: 1_500,
        },
        RunEvent::PhaseUpdated {
            run_id: String::from("run-2"),
            phase: RunPhase::Verifying,
        },
        RunEvent::OutcomeRecorded {
            run_id: String::from("run-2"),
            outcome: RunOutcome::Success,
        },
    ];

    let status = project_run(&events).expect("run projection should initialize from RunCreated");

    assert_eq!(status.phase, RunPhase::Completed);
    assert_eq!(status.outcome, Some(RunOutcome::Success));
    assert_eq!(status.completed_steps, 1);
    assert_eq!(status.budget.max_steps, 3);
    assert_eq!(status.budget.max_minutes, 13);
    assert_eq!(status.budget.max_tokens, 8_500);
}

#[test]
fn run_lifecycle_projection_tracks_repair_and_blocked_outcomes() {
    let goal = RunGoal {
        summary: String::from("Handle failed verification"),
        workspace_root: String::from("/workspace"),
        constraints: Vec::new(),
        done_conditions: vec![DoneCondition {
            label: String::from("repair"),
            evidence: String::from("repair attempted"),
        }],
        verification_checks: vec![VerificationCheck {
            label: String::from("fault"),
            detail: String::from("inject failure"),
        }],
        budget: RunBudget::bounded(3, 10, 5_000),
        approval_mode: RunApprovalMode::Always,
    };
    let events = vec![
        RunEvent::RunCreated {
            run_id: String::from("run-3"),
            goal,
        },
        RunEvent::PhaseUpdated {
            run_id: String::from("run-3"),
            phase: RunPhase::ExecutingStep,
        },
        RunEvent::PhaseUpdated {
            run_id: String::from("run-3"),
            phase: RunPhase::Verifying,
        },
        RunEvent::PhaseUpdated {
            run_id: String::from("run-3"),
            phase: RunPhase::Repairing,
        },
        RunEvent::ApprovalRequested {
            run_id: String::from("run-3"),
            reason: String::from("dangerous command requires approval"),
        },
        RunEvent::OutcomeRecorded {
            run_id: String::from("run-3"),
            outcome: RunOutcome::ApprovalRequired,
        },
    ];

    let status = project_run(&events).expect("run projection should initialize from RunCreated");

    assert_eq!(status.phase, RunPhase::WaitingApproval);
    assert_eq!(status.outcome, Some(RunOutcome::ApprovalRequired));
    assert_eq!(
        status.last_blocker.as_deref(),
        Some("dangerous command requires approval")
    );
}

#[test]
fn run_approval_request_keeps_operator_visible_blocker_until_resolution() {
    let goal = RunGoal {
        summary: String::from("Await approval"),
        workspace_root: String::from("/workspace"),
        constraints: Vec::new(),
        done_conditions: vec![DoneCondition {
            label: String::from("approval"),
            evidence: String::from("operator decision"),
        }],
        verification_checks: vec![VerificationCheck {
            label: String::from("risk"),
            detail: String::from("high-risk command"),
        }],
        budget: RunBudget::bounded(2, 5, 2_000),
        approval_mode: RunApprovalMode::Always,
    };
    let status = RunStatus::new("run-4", goal);
    let waiting = reduce_run_status(
        &status,
        &RunEvent::ApprovalRequested {
            run_id: String::from("run-4"),
            reason: String::from("remove_path requires approval"),
        },
    );

    assert_eq!(waiting.phase, RunPhase::WaitingApproval);
    assert_eq!(
        waiting.last_blocker.as_deref(),
        Some("remove_path requires approval")
    );
}
