use crate::decision::DecisionOutcome;
use crate::policy_codes::PolicyCode;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Active,
    ReadOnly,
    Halted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    pub revision: u64,
    pub mode: ExecutionMode,
    pub facts: BTreeMap<String, String>,
    pub last_intent_id: Option<String>,
    pub last_actor_id: Option<String>,
    pub last_decision: Option<DecisionOutcome>,
    pub last_policy_code: Option<PolicyCode>,
    pub denied_count: u64,
    pub audit_count: u64,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            revision: 0,
            mode: ExecutionMode::Active,
            facts: BTreeMap::new(),
            last_intent_id: None,
            last_actor_id: None,
            last_decision: None,
            last_policy_code: None,
            denied_count: 0,
            audit_count: 0,
        }
    }
}

impl AgentState {
    pub fn with_fact(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.facts.insert(key.into(), value.into());
        self
    }

    pub fn invariants_hold(&self) -> bool {
        self.denied_count <= self.audit_count && self.audit_count <= self.revision
    }
}
