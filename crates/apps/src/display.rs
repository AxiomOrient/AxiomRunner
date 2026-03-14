use axonrunner_core::{DecisionOutcome, ExecutionMode};

pub fn mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Active => "active",
        ExecutionMode::ReadOnly => "read_only",
        ExecutionMode::Halted => "halted",
    }
}

pub fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}
