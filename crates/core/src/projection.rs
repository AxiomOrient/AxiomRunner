use crate::event::DomainEvent;
use crate::reducer::reduce;
use crate::state::AgentState;

pub fn project(events: &[DomainEvent]) -> AgentState {
    project_from(&AgentState::default(), events)
}

pub fn project_from(initial: &AgentState, events: &[DomainEvent]) -> AgentState {
    events
        .iter()
        .fold(initial.clone(), |state, event| reduce(&state, event))
}
