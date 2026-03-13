use crate::event::DomainEvent;
use crate::event::RunEvent;
use crate::reducer::{reduce, reduce_run_status};
use crate::state::{AgentState, RunStatus};

pub fn project(events: &[DomainEvent]) -> AgentState {
    project_from(&AgentState::default(), events)
}

pub fn project_from(initial: &AgentState, events: &[DomainEvent]) -> AgentState {
    events
        .iter()
        .fold(initial.clone(), |state, event| reduce(&state, event))
}

pub fn project_run(events: &[RunEvent]) -> Option<RunStatus> {
    let first = events.first()?;
    let RunEvent::RunCreated { run_id, goal } = first else {
        return None;
    };
    Some(project_run_from(
        &RunStatus::new(run_id.clone(), goal.clone()),
        &events[1..],
    ))
}

pub fn project_run_from(initial: &RunStatus, events: &[RunEvent]) -> RunStatus {
    events.iter().fold(initial.clone(), |status, event| {
        reduce_run_status(&status, event)
    })
}
