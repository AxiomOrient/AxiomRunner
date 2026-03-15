use crate::decision::DecisionOutcome;
use crate::policy_codes::PolicyCode;

/// The operational mode of the agent runtime.
///
/// Currently only `Active` is supported. This enum is `#[non_exhaustive]`
/// to allow adding modes (e.g., `Maintenance`, `ReadOnly`) in future releases
/// without breaking downstream match expressions.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub last_intent_id: Option<String>,
    pub last_actor_id: Option<String>,
    pub last_decision: Option<DecisionOutcome>,
    pub last_policy_code: Option<PolicyCode>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            revision: 0,
            mode: ExecutionMode::Active,
            last_intent_id: None,
            last_actor_id: None,
            last_decision: None,
            last_policy_code: None,
        }
    }
}

impl AgentState {
    pub fn record_intent(
        &self,
        intent_id: impl Into<String>,
        actor_id: impl Into<String>,
        decision: DecisionOutcome,
        policy_code: PolicyCode,
    ) -> Self {
        Self {
            revision: self.revision.saturating_add(1),
            mode: self.mode,
            last_intent_id: Some(intent_id.into()),
            last_actor_id: Some(actor_id.into()),
            last_decision: Some(decision),
            last_policy_code: Some(policy_code),
        }
    }

    /// Returns `true` if the state satisfies all structural invariants.
    ///
    /// When `revision > 0` at least one intent has been recorded, so all
    /// tracking fields must be populated.
    pub fn invariants_hold(&self) -> bool {
        if self.revision > 0 {
            return self.last_intent_id.is_some()
                && self.last_actor_id.is_some()
                && self.last_decision.is_some()
                && self.last_policy_code.is_some();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::AgentState;
    use crate::decision::DecisionOutcome;
    use crate::policy_codes::PolicyCode;

    #[test]
    fn agent_state_records_intent_metadata_purely() {
        let state = AgentState::default();

        let next = state.record_intent(
            "cli-7",
            "operator",
            DecisionOutcome::Accepted,
            PolicyCode::Allowed,
        );

        assert_eq!(state.revision, 0);
        assert_eq!(next.revision, 1);
        assert_eq!(next.last_intent_id.as_deref(), Some("cli-7"));
        assert_eq!(next.last_actor_id.as_deref(), Some("operator"));
        assert_eq!(next.last_decision, Some(DecisionOutcome::Accepted));
        assert_eq!(next.last_policy_code, Some(PolicyCode::Allowed));
    }

    #[test]
    fn invariants_hold_passes_for_default_state() {
        assert!(AgentState::default().invariants_hold());
    }

    #[test]
    fn invariants_hold_passes_after_recording_intent() {
        let state = AgentState::default().record_intent(
            "cli-1",
            "operator",
            DecisionOutcome::Accepted,
            PolicyCode::Allowed,
        );
        assert!(state.invariants_hold());
    }
}
