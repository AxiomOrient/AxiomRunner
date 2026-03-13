use crate::effect::Effect;
use crate::event::{DomainEvent, RunEvent};
use crate::state::{AgentState, RunOutcome, RunPhase, RunStatus};

pub fn reduce(state: &AgentState, event: &DomainEvent) -> AgentState {
    let mut next = state.clone();
    next.revision = next.revision.saturating_add(1);

    match event {
        DomainEvent::IntentAccepted { intent } => {
            next.last_intent_id = Some(intent.intent_id.clone());
            next.last_actor_id = intent.actor_id.clone();
        }
        DomainEvent::PolicyEvaluated { audit } => {
            next.last_policy_code = Some(audit.code);
            next.audit_count = next.audit_count.saturating_add(1);
            if !audit.allowed {
                next.denied_count = next.denied_count.saturating_add(1);
            }
        }
        DomainEvent::DecisionCalculated { decision } => {
            next.last_decision = Some(decision.outcome);
        }
        DomainEvent::EffectsApplied { effects } => {
            for effect in effects {
                match effect {
                    Effect::PutFact { key, value } => {
                        next.facts.insert(key.clone(), value.clone());
                    }
                    Effect::RemoveFact { key } => {
                        next.facts.remove(key);
                    }
                    Effect::SetMode(mode) => {
                        next.mode = *mode;
                    }
                }
            }
        }
    }

    next
}

pub fn reduce_run_status(status: &RunStatus, event: &RunEvent) -> RunStatus {
    let mut next = status.clone();

    match event {
        RunEvent::RunCreated { .. } => {}
        RunEvent::PhaseUpdated { phase, .. } => {
            next.phase = *phase;
            if !matches!(phase, RunPhase::Blocked | RunPhase::WaitingApproval) {
                next.last_blocker = None;
            }
        }
        RunEvent::BudgetConsumed {
            consumed_steps,
            consumed_minutes,
            consumed_tokens,
            ..
        } => {
            next.completed_steps = next.completed_steps.saturating_add(*consumed_steps);
            next.budget.max_steps = next.budget.max_steps.saturating_sub(*consumed_steps);
            next.budget.max_minutes = next.budget.max_minutes.saturating_sub(*consumed_minutes);
            next.budget.max_tokens = next.budget.max_tokens.saturating_sub(*consumed_tokens);
        }
        RunEvent::ApprovalRequested { reason, .. } => {
            next.phase = RunPhase::WaitingApproval;
            next.last_blocker = Some(reason.clone());
        }
        RunEvent::OutcomeRecorded { outcome, .. } => {
            next.outcome = Some(*outcome);
            next.phase = phase_for_outcome(*outcome);
            if matches!(outcome, RunOutcome::Blocked | RunOutcome::ApprovalRequired) {
                next.last_blocker
                    .get_or_insert_with(|| String::from("operator attention required"));
            } else {
                next.last_blocker = None;
            }
        }
    }

    next
}

fn phase_for_outcome(outcome: RunOutcome) -> RunPhase {
    match outcome {
        RunOutcome::Success => RunPhase::Completed,
        RunOutcome::Blocked | RunOutcome::BudgetExhausted => RunPhase::Blocked,
        RunOutcome::ApprovalRequired => RunPhase::WaitingApproval,
        RunOutcome::Failed => RunPhase::Failed,
        RunOutcome::Aborted => RunPhase::Aborted,
    }
}
