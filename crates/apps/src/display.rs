use axiomrunner_core::{DecisionOutcome, ExecutionMode};

pub fn mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Active => "active",
        _ => unreachable!("ExecutionMode has no retained operator-facing variant here"),
    }
}

pub fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}
