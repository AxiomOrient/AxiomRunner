use crate::intent::Intent;
use crate::policy::PolicyVerdict;
use crate::policy_codes::PolicyCode;
use crate::state::AgentState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAuditRecord {
    pub intent_id: String,
    pub actor_id: Option<String>,
    pub state_revision: u64,
    pub allowed: bool,
    pub code: PolicyCode,
    pub reason: String,
}

pub fn build_policy_audit(
    state: &AgentState,
    intent: &Intent,
    verdict: &PolicyVerdict,
) -> PolicyAuditRecord {
    PolicyAuditRecord {
        intent_id: intent.intent_id.clone(),
        actor_id: intent.actor_id.clone(),
        state_revision: state.revision,
        allowed: verdict.allowed,
        code: verdict.code,
        reason: verdict.reason.to_owned(),
    }
}

pub fn policy_reason_code(audit: &PolicyAuditRecord) -> &'static str {
    audit.code.as_str()
}

pub fn policy_risk_effect_path(intent: &Intent, audit: &PolicyAuditRecord) -> String {
    format!(
        "risk/{}/effect/{}",
        risk_level_for_code(audit.code),
        effect_path_for_intent(intent, audit.allowed)
    )
}

pub fn format_policy_decision_audit_line(intent: &Intent, audit: &PolicyAuditRecord) -> String {
    format!(
        "policy_decision_audit intent_id={} actor_id={} state_revision={} allowed={} reason_code={} risk_effect_path={} reason={:?}",
        audit.intent_id,
        audit.actor_id.as_deref().unwrap_or("<none>"),
        audit.state_revision,
        audit.allowed,
        policy_reason_code(audit),
        policy_risk_effect_path(intent, audit),
        audit.reason
    )
}

fn risk_level_for_code(code: PolicyCode) -> &'static str {
    match code {
        PolicyCode::Allowed => "none",
        PolicyCode::ActorMissing | PolicyCode::PayloadTooLarge => "low",
        PolicyCode::ReadOnlyMutation => "medium",
        PolicyCode::RuntimeHalted | PolicyCode::UnauthorizedControl => "high",
    }
}

fn effect_path_for_intent(intent: &Intent, allowed: bool) -> &'static str {
    if !allowed {
        return "blocked";
    }

    match &intent.kind {
        crate::intent::IntentKind::ReadFact { .. } => "read_fact",
        crate::intent::IntentKind::WriteFact { .. } => "write_fact",
        crate::intent::IntentKind::RemoveFact { .. } => "remove_fact",
        crate::intent::IntentKind::FreezeWrites => "set_read_only",
        crate::intent::IntentKind::Halt => "set_halted",
    }
}
