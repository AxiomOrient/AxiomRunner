use crate::intent::{Intent, IntentKind};
use crate::policy_codes::PolicyCode;
use crate::state::{AgentState, ExecutionMode};

pub const MAX_KEY_LEN: usize = 128;
pub const MAX_VALUE_LEN: usize = 4096;
pub const LOCALHOST_BIND: &str = "127.0.0.1";
pub const ENV_DEV_MODE: &str = "DEV_MODE";
pub const ENV_BIND: &str = "BIND";
pub const ENV_ALLOW_REMOTE: &str = "ALLOW_REMOTE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevModeMitigationInput {
    pub dev_mode: Option<bool>,
    pub bind_is_localhost: Option<bool>,
    pub allow_remote: Option<bool>,
}

impl DevModeMitigationInput {
    pub const fn new(
        dev_mode: Option<bool>,
        bind_is_localhost: Option<bool>,
        allow_remote: Option<bool>,
    ) -> Self {
        Self {
            dev_mode,
            bind_is_localhost,
            allow_remote,
        }
    }

    pub fn from_env_values(dev_mode: bool, bind: &str, allow_remote: bool) -> Self {
        Self::new(
            Some(dev_mode),
            Some(bind == LOCALHOST_BIND),
            Some(allow_remote),
        )
    }
}

pub const fn dev_mode_mitigation_active(input: DevModeMitigationInput) -> bool {
    matches!(input.dev_mode, Some(true))
        && matches!(input.bind_is_localhost, Some(true))
        && matches!(input.allow_remote, Some(false))
}

pub const fn dev_mode_mitigation_enabled(
    mitigation_requested: bool,
    input: DevModeMitigationInput,
) -> bool {
    mitigation_requested && dev_mode_mitigation_active(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyVerdict {
    pub allowed: bool,
    pub code: PolicyCode,
    pub reason: &'static str,
}

impl PolicyVerdict {
    pub const fn allow() -> Self {
        Self {
            allowed: true,
            code: PolicyCode::Allowed,
            reason: "allowed",
        }
    }

    pub const fn deny(code: PolicyCode, reason: &'static str) -> Self {
        Self {
            allowed: false,
            code,
            reason,
        }
    }
}

pub fn evaluate_policy(state: &AgentState, intent: &Intent) -> PolicyVerdict {
    let actor = intent
        .actor_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if actor.is_empty() {
        return PolicyVerdict::deny(PolicyCode::ActorMissing, "actor is required");
    }

    if state.mode == ExecutionMode::Halted {
        return PolicyVerdict::deny(PolicyCode::RuntimeHalted, "runtime is halted");
    }

    if state.mode == ExecutionMode::ReadOnly && intent.mutates_facts() {
        return PolicyVerdict::deny(
            PolicyCode::ReadOnlyMutation,
            "fact mutations are blocked in read-only mode",
        );
    }

    if intent.is_control_action() && actor != "system" {
        return PolicyVerdict::deny(
            PolicyCode::UnauthorizedControl,
            "control actions require actor `system`",
        );
    }

    match &intent.kind {
        IntentKind::ReadFact { key } | IntentKind::RemoveFact { key } => {
            if key.len() > MAX_KEY_LEN {
                return PolicyVerdict::deny(PolicyCode::PayloadTooLarge, "key exceeds limit");
            }
        }
        IntentKind::WriteFact { key, value } => {
            if key.len() > MAX_KEY_LEN || value.len() > MAX_VALUE_LEN {
                return PolicyVerdict::deny(PolicyCode::PayloadTooLarge, "payload exceeds limit");
            }
        }
        IntentKind::FreezeWrites | IntentKind::Halt => {}
    }

    PolicyVerdict::allow()
}

#[cfg(test)]
mod tests {
    use super::{
        DevModeMitigationInput, ENV_ALLOW_REMOTE, ENV_BIND, ENV_DEV_MODE, LOCALHOST_BIND,
        dev_mode_mitigation_active, dev_mode_mitigation_enabled,
    };

    #[test]
    fn policy_dev_mode_mitigation_requires_explicit_local_tuple() {
        let active = DevModeMitigationInput::new(Some(true), Some(true), Some(false));
        assert!(dev_mode_mitigation_active(active));
        assert!(dev_mode_mitigation_enabled(true, active));

        let remote_allowed = DevModeMitigationInput::new(Some(true), Some(true), Some(true));
        assert!(!dev_mode_mitigation_active(remote_allowed));
        assert!(!dev_mode_mitigation_enabled(true, remote_allowed));

        let wildcard_bind = DevModeMitigationInput::new(Some(true), Some(false), Some(false));
        assert!(!dev_mode_mitigation_active(wildcard_bind));
    }

    #[test]
    fn policy_dev_mode_mitigation_rejects_missing_inputs() {
        let missing_dev_mode = DevModeMitigationInput::new(None, Some(true), Some(false));
        assert!(!dev_mode_mitigation_active(missing_dev_mode));

        let missing_bind = DevModeMitigationInput::new(Some(true), None, Some(false));
        assert!(!dev_mode_mitigation_active(missing_bind));

        let missing_allow_remote = DevModeMitigationInput::new(Some(true), Some(true), None);
        assert!(!dev_mode_mitigation_active(missing_allow_remote));

        assert_eq!(ENV_DEV_MODE, "DEV_MODE");
        assert_eq!(ENV_BIND, "BIND");
        assert_eq!(ENV_ALLOW_REMOTE, "ALLOW_REMOTE");
        assert_eq!(LOCALHOST_BIND, "127.0.0.1");
    }
}
