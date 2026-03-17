use super::*;

pub(super) fn execute_intent(runtime: &mut CliRuntime, intent: &RunTemplate) -> Result<(), String> {
    let previous = runtime.runtime_snapshot();
    let started_at = Instant::now();
    let run_id = runtime.next_run_id();
    let mut applied = runtime.apply_template(intent)?;
    let plan = runtime
        .compose_state
        .plan_template(intent, &run_id, &applied.intent_id);
    let pre_execution_guard =
        lifecycle::goal_pre_execution_guard(intent, &plan, runtime.compose_state.max_tokens()).map(
            |guard| {
                if let Some(policy_code) = guard.policy_code {
                    runtime.state.last_policy_code = Some(policy_code);
                    applied.policy_code = policy_code;
                }
                (
                    runtime.compose_state.idle_execution(),
                    RuntimeRunVerification {
                        status: "skipped",
                        summary: guard.summary,
                        checks: vec![
                            format!("goal_file={}", intent.path),
                            format!("workspace_root={}", intent.goal.workspace_root),
                            format!("done_conditions={}", intent.goal.done_conditions.len()),
                            format!(
                                "verification_checks={}",
                                intent.goal.verification_checks.len()
                            ),
                        ],
                    },
                    RuntimeRunRepair::skipped("pre_execution_guard"),
                )
            },
        );
    let (execution, verification, repair, checkpoint) = if let Some(guard) = pre_execution_guard {
        (guard.0, guard.1, guard.2, None)
    } else {
        runtime.compose_state.prepare_execution_workspace(&run_id)?;
        let checkpoint = runtime
            .compose_state
            .write_checkpoint_metadata(&applied.intent_id, &run_id)?;
        let execution = runtime.persist_template_result(intent, &applied);
        let verification = lifecycle::verify_run(intent, &execution);
        let (execution, verification, repair) =
            lifecycle::run_repair_loop(runtime, intent, &plan, &applied, execution, verification);
        (execution, verification, repair, checkpoint)
    };
    let mut finalized = lifecycle::finalize_run(
        runtime.compose_state.health(),
        lifecycle::FinalizeRunInput {
            template: intent,
            plan: plan.clone(),
            applied: applied.clone(),
            execution,
            verification,
            repair,
            goal_approval_granted: false,
            elapsed_ms: lifecycle::elapsed_ms(started_at),
            requested_max_tokens: runtime.compose_state.max_tokens(),
        },
    );
    finalized.record.checkpoint = checkpoint;
    finalized.record = lifecycle::apply_goal_elapsed_budget(intent, finalized.record);
    let first_failure = finalized
        .execution
        .first_failure()
        .map(|(stage, message)| (stage.to_owned(), message.to_owned()));
    let persist_snapshot = if first_failure.is_some()
        && finalized.record.outcome != RuntimeRunOutcome::BudgetExhausted
    {
        None
    } else if matches!(
        finalized.record.outcome,
        RuntimeRunOutcome::Failed | RuntimeRunOutcome::Aborted
    ) {
        None
    } else {
        Some(RuntimeStateSnapshot {
            state: runtime.state.clone(),
            next_intent_seq: runtime.next_intent_seq,
            next_run_seq: runtime.next_run_seq,
            pending_run: pending_run_snapshot(intent, &applied, &finalized.record),
        })
    };
    let report_input = report_write_input(&applied);
    let committed = crate::run_commit::commit_prepared_run(
        &mut runtime.compose_state,
        &runtime.trace_store,
        &runtime.state_store,
        crate::run_commit::PreparedRunCommit {
            actor_id: &runtime.actor_id,
            intent_id: &applied.intent_id,
            kind: applied.kind,
            outcome: applied.outcome,
            policy_code: applied.policy_code.as_str(),
            template: intent,
            report_input,
            execution: finalized.execution,
            record: finalized.record,
            state: &runtime.state,
            snapshot: persist_snapshot.clone(),
            apply_done_conditions: true,
        },
    )?;
    finalized.record = committed.record;
    if let Some(warning) = committed.memory_warning {
        eprintln!("{warning}");
    }
    runtime.pending_run = persist_snapshot.and_then(|snapshot| snapshot.pending_run);
    if let Some((stage, message)) = first_failure
        && finalized.record.outcome != RuntimeRunOutcome::BudgetExhausted
    {
        runtime.restore_snapshot(previous);
        return Err(format!(
            "runtime execution failed intent_id={} stage={} error={}",
            applied.intent_id, stage, message
        ));
    }
    if matches!(
        finalized.record.outcome,
        RuntimeRunOutcome::Failed | RuntimeRunOutcome::Aborted
    ) {
        runtime.restore_snapshot(previous);
        return Err(format!(
            "runtime execution failed intent_id={} stage=run error={}",
            applied.intent_id, finalized.record.reason
        ));
    }

    print_intent_result(&applied);
    print_run_result(&applied.intent_id, &finalized.record);
    Ok(())
}

pub(super) fn execute_resume(runtime: &mut CliRuntime, target: &str) -> Result<(), String> {
    let started_at = Instant::now();
    let pending = pending_resume_for_target(runtime, target)?.clone();
    runtime
        .compose_state
        .prepare_execution_workspace(&pending.run_id)?;
    let checkpoint = runtime
        .compose_state
        .write_checkpoint_metadata(&pending.intent_id, &pending.run_id)?;
    let template = load_pending_goal_template(&pending.goal_file_path)?;
    let plan = runtime
        .compose_state
        .plan_template(&template, &pending.run_id, &pending.intent_id);
    let execution = runtime
        .compose_state
        .apply_template(&template, DecisionOutcome::Accepted);
    let resume_applied = accepted_goal_applied(&pending.intent_id);
    let verification = lifecycle::verify_run(&template, &execution);
    let (execution, verification, repair) = lifecycle::run_repair_loop(
        runtime,
        &template,
        &plan,
        &resume_applied,
        execution,
        verification,
    );
    let mut finalized = lifecycle::finalize_run(
        runtime.compose_state.health(),
        lifecycle::FinalizeRunInput {
            template: &template,
            plan: plan.clone(),
            applied: resume_applied.clone(),
            execution,
            verification,
            repair,
            goal_approval_granted: true,
            elapsed_ms: lifecycle::elapsed_ms(started_at),
            requested_max_tokens: runtime.compose_state.max_tokens(),
        },
    );
    finalized.record.checkpoint = checkpoint;
    finalized.record = lifecycle::apply_goal_elapsed_budget(&template, finalized.record);
    let report_input = report_write_input(&resume_applied);
    let committed = crate::run_commit::commit_prepared_run(
        &mut runtime.compose_state,
        &runtime.trace_store,
        &runtime.state_store,
        crate::run_commit::PreparedRunCommit {
            actor_id: &runtime.actor_id,
            intent_id: &pending.intent_id,
            kind: "goal",
            outcome: DecisionOutcome::Accepted,
            policy_code: PolicyCode::Allowed.as_str(),
            template: &template,
            report_input,
            execution: finalized.execution,
            record: finalized.record,
            state: &runtime.state,
            snapshot: Some(RuntimeStateSnapshot {
                state: runtime.state.clone(),
                next_intent_seq: runtime.next_intent_seq,
                next_run_seq: runtime.next_run_seq,
                pending_run: None,
            }),
            apply_done_conditions: true,
        },
    )?;
    finalized.record = committed.record;
    if let Some(warning) = committed.memory_warning {
        eprintln!("{warning}");
    }
    runtime.pending_run = None;
    println!(
        "resume run_id={} phase={} outcome={} reason={}",
        pending.run_id,
        run_phase_name(finalized.record.phase),
        run_outcome_name(finalized.record.outcome),
        finalized.record.reason
    );
    Ok(())
}

pub(super) fn execute_abort(runtime: &mut CliRuntime, target: &str) -> Result<(), String> {
    let started_at = Instant::now();
    let pending = pending_abort_for_target(runtime, target)?.clone();
    let template = load_pending_goal_template(&pending.goal_file_path)?;
    let plan = runtime
        .compose_state
        .plan_template(&template, &pending.run_id, &pending.intent_id);
    let execution = runtime.compose_state.idle_execution();
    let verification = RuntimeRunVerification {
        status: "passed",
        summary: String::from(RUN_REASON_OPERATOR_ABORT),
        checks: vec![String::from("abort=operator_requested")],
    };
    let repair = RuntimeRunRepair::skipped("abort");
    let record = RuntimeRunRecord {
        plan,
        step_journal: vec![RuntimeRunStepRecord {
            id: format!("{}/step-abort", pending.run_id),
            label: String::from("abort pending run"),
            phase: run_phase_name(RuntimeRunPhase::Aborted).to_owned(),
            status: run_phase_name(RuntimeRunPhase::Aborted).to_owned(),
            evidence: String::from(RUN_REASON_OPERATOR_ABORT),
            failure: None,
        }],
        verification,
        repair,
        checkpoint: None,
        rollback: None,
        elapsed_ms: lifecycle::elapsed_ms(started_at),
        phase: RuntimeRunPhase::Aborted,
        outcome: RuntimeRunOutcome::Aborted,
        reason: String::from(RUN_REASON_OPERATOR_ABORT),
        reason_code: String::from(RUN_REASON_OPERATOR_ABORT),
        reason_detail: String::from("none"),
    };
    let abort_applied = accepted_goal_applied(&pending.intent_id);
    let report_input = report_write_input(&abort_applied);
    let committed = crate::run_commit::commit_prepared_run(
        &mut runtime.compose_state,
        &runtime.trace_store,
        &runtime.state_store,
        crate::run_commit::PreparedRunCommit {
            actor_id: &runtime.actor_id,
            intent_id: &pending.intent_id,
            kind: "goal",
            outcome: DecisionOutcome::Accepted,
            policy_code: PolicyCode::Allowed.as_str(),
            template: &template,
            report_input,
            execution,
            record,
            state: &runtime.state,
            snapshot: Some(RuntimeStateSnapshot {
                state: runtime.state.clone(),
                next_intent_seq: runtime.next_intent_seq,
                next_run_seq: runtime.next_run_seq,
                pending_run: None,
            }),
            apply_done_conditions: false,
        },
    )?;
    if let Some(warning) = committed.memory_warning {
        eprintln!("{warning}");
    }
    runtime.pending_run = None;
    println!(
        "abort run_id={} phase={} outcome={} reason={}",
        pending.run_id,
        run_phase_name(committed.record.phase),
        run_outcome_name(committed.record.outcome),
        committed.record.reason
    );
    Ok(())
}

fn pending_run_for_target<'a>(
    runtime: &'a CliRuntime,
    target: &str,
) -> Result<&'a PendingRunSnapshot, String> {
    let Some(pending) = runtime.pending_run.as_ref() else {
        return Err(String::from("no pending run is available"));
    };
    if target == "latest" || target == pending.run_id {
        return Ok(pending);
    }
    Err(format!("pending run not found: {target}"))
}

fn pending_resume_for_target<'a>(
    runtime: &'a CliRuntime,
    target: &str,
) -> Result<&'a PendingRunSnapshot, String> {
    let pending = pending_run_for_target(runtime, target).map_err(|_| {
        format!("{RESUME_PENDING_APPROVAL_ONLY}; no pending approval run is available")
    })?;
    if pending.phase != run_phase_name(RuntimeRunPhase::WaitingApproval)
        || pending.approval_state != APPROVAL_STATE_REQUIRED
    {
        return Err(format!(
            "{RESUME_PENDING_APPROVAL_ONLY}; found phase={} approval_state={}",
            pending.phase, pending.approval_state
        ));
    }
    Ok(pending)
}

fn pending_abort_for_target<'a>(
    runtime: &'a CliRuntime,
    target: &str,
) -> Result<&'a PendingRunSnapshot, String> {
    let pending = pending_run_for_target(runtime, target).map_err(|_| {
        format!("{ABORT_PENDING_CONTROL_ONLY}; no pending control run is available")
    })?;
    if pending.phase != run_phase_name(RuntimeRunPhase::WaitingApproval)
        || pending.approval_state != APPROVAL_STATE_REQUIRED
    {
        return Err(format!(
            "{ABORT_PENDING_CONTROL_ONLY}; found phase={} approval_state={}",
            pending.phase, pending.approval_state
        ));
    }
    Ok(pending)
}

fn pending_run_snapshot(
    intent: &RunTemplate,
    applied: &AppliedIntent,
    record: &RuntimeRunRecord,
) -> Option<PendingRunSnapshot> {
    if !matches!(record.outcome, RuntimeRunOutcome::ApprovalRequired) {
        return None;
    }
    Some(PendingRunSnapshot {
        run_id: record.plan.run_id.clone(),
        intent_id: applied.intent_id.clone(),
        goal_file_path: intent.path.clone(),
        phase: run_phase_name(record.phase).to_owned(),
        reason: record.reason.clone(),
        reason_code: record.reason_code.clone(),
        reason_detail: record.reason_detail.clone(),
        approval_state: String::from(APPROVAL_STATE_REQUIRED),
        verifier_state: record.verification.status.to_owned(),
        advisory_constraints: advisory_constraint_labels(&intent.goal.constraints),
    })
}

fn advisory_constraint_labels(constraints: &[RunConstraint]) -> String {
    let labels: Vec<&str> = constraints
        .iter()
        .filter(|c| c.mode() == RunConstraintMode::Advisory)
        .map(|c| c.label.as_str())
        .collect();
    if labels.is_empty() {
        String::from("none")
    } else {
        labels.join(",")
    }
}

fn load_pending_goal_template(goal_file_path: &str) -> Result<RunTemplate, String> {
    crate::goal_file::parse_goal_file_template(goal_file_path)
}

fn print_intent_result(applied: &AppliedIntent) {
    println!(
        "intent id={} kind={} outcome={} policy={} effects={}",
        applied.intent_id,
        applied.kind,
        outcome_name(applied.outcome),
        applied.policy_code.as_str(),
        applied.effect_count
    );
}

fn print_run_result(intent_id: &str, record: &RuntimeRunRecord) {
    println!(
        "run intent_id={} phase={} outcome={} reason={}",
        intent_id,
        run_phase_name(record.phase),
        run_outcome_name(record.outcome),
        record.reason
    );
}
