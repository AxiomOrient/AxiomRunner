pub const LOCALHOST_BIND: &str = "127.0.0.1";
pub const ENV_DEV_MODE: &str = "DEV_MODE";
pub const ENV_BIND: &str = "BIND";
pub const ENV_ALLOW_REMOTE: &str = "ALLOW_REMOTE";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardMode {
    Strict,
    Relaxed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevModeEnvFlags {
    pub dev_mode: bool,
    pub bind_is_localhost: bool,
    pub allow_remote: bool,
}

impl DevModeEnvFlags {
    pub const fn new(dev_mode: bool, bind_is_localhost: bool, allow_remote: bool) -> Self {
        Self {
            dev_mode,
            bind_is_localhost,
            allow_remote,
        }
    }

    pub fn from_env(dev_mode: bool, bind: &str, allow_remote: bool) -> Self {
        Self::new(dev_mode, bind == LOCALHOST_BIND, allow_remote)
    }

    pub const fn local_only(dev_mode: bool) -> Self {
        Self::new(dev_mode, true, false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevModePolicy {
    pub enabled: bool,
    pub relax_schema_guard: bool,
    pub env_flags: DevModeEnvFlags,
}

impl DevModePolicy {
    pub const fn new(enabled: bool, relax_schema_guard: bool) -> Self {
        Self::with_env_flags(DevModeEnvFlags::local_only(enabled), relax_schema_guard)
    }

    pub const fn with_env_flags(env_flags: DevModeEnvFlags, relax_schema_guard: bool) -> Self {
        Self {
            enabled: env_flags.dev_mode,
            relax_schema_guard,
            env_flags,
        }
    }
    pub const fn strict() -> Self {
        Self::new(false, false)
    }

    pub const fn guard_mode(self) -> GuardMode {
        if self.relaxes_guard() {
            GuardMode::Relaxed
        } else {
            GuardMode::Strict
        }
    }

    pub const fn relaxes_guard(self) -> bool {
        self.relax_schema_guard
            && self.env_flags.dev_mode
            && self.env_flags.bind_is_localhost
            && !self.env_flags.allow_remote
    }
}

pub const fn dev_mode_mitigation_active(
    dev_mode: Option<bool>,
    bind_is_localhost: Option<bool>,
    allow_remote: Option<bool>,
) -> bool {
    matches!(dev_mode, Some(true))
        && matches!(bind_is_localhost, Some(true))
        && matches!(allow_remote, Some(false))
}

#[cfg(test)]
mod tests {
    use super::{
        ENV_ALLOW_REMOTE, ENV_BIND, ENV_DEV_MODE, LOCALHOST_BIND, dev_mode_mitigation_active,
    };

    #[test]
    fn dev_mode_mitigation_only_activates_for_explicit_local_tuple() {
        assert!(dev_mode_mitigation_active(
            Some(true),
            Some(true),
            Some(false)
        ));
        assert!(!dev_mode_mitigation_active(
            Some(false),
            Some(true),
            Some(false)
        ));
        assert!(!dev_mode_mitigation_active(
            Some(true),
            Some(false),
            Some(false)
        ));
        assert!(!dev_mode_mitigation_active(
            Some(true),
            Some(true),
            Some(true)
        ));
    }

    #[test]
    fn dev_mode_mitigation_does_not_activate_for_missing_env_values() {
        assert!(!dev_mode_mitigation_active(None, Some(true), Some(false)));
        assert!(!dev_mode_mitigation_active(Some(true), None, Some(false)));
        assert!(!dev_mode_mitigation_active(Some(true), Some(true), None));

        assert_eq!(ENV_DEV_MODE, "DEV_MODE");
        assert_eq!(ENV_BIND, "BIND");
        assert_eq!(ENV_ALLOW_REMOTE, "ALLOW_REMOTE");
        assert_eq!(LOCALHOST_BIND, "127.0.0.1");
    }
}
