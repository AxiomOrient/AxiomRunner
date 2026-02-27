#[path = "../src/dev_mode.rs"]
mod dev_mode;

use dev_mode::{DevModeEnvFlags, DevModePolicy, GuardMode, LOCALHOST_BIND};

#[test]
fn dev_mode_relax_guard_policy_follows_requested_and_safe_tuple() {
    let cases = [
        (
            "local_tuple_and_requested",
            DevModeEnvFlags::from_env(true, LOCALHOST_BIND, false),
            true,
            true,
            GuardMode::Relaxed,
        ),
        (
            "dev_mode_disabled",
            DevModeEnvFlags::from_env(false, LOCALHOST_BIND, false),
            true,
            false,
            GuardMode::Strict,
        ),
        (
            "wildcard_bind",
            DevModeEnvFlags::from_env(true, "0.0.0.0", false),
            true,
            false,
            GuardMode::Strict,
        ),
        (
            "hostname_alias_bind",
            DevModeEnvFlags::from_env(true, "localhost", false),
            true,
            false,
            GuardMode::Strict,
        ),
        (
            "remote_enabled",
            DevModeEnvFlags::from_env(true, LOCALHOST_BIND, true),
            true,
            false,
            GuardMode::Strict,
        ),
        (
            "relax_not_requested",
            DevModeEnvFlags::from_env(true, LOCALHOST_BIND, false),
            false,
            false,
            GuardMode::Strict,
        ),
        (
            "multiple_unsafe_flags",
            DevModeEnvFlags::from_env(false, "0.0.0.0", true),
            true,
            false,
            GuardMode::Strict,
        ),
    ];

    for (case_name, flags, relax_requested, expected_relaxes_guard, expected_guard_mode) in cases {
        let policy = DevModePolicy::with_env_flags(flags, relax_requested);
        assert_eq!(
            policy.relaxes_guard(),
            expected_relaxes_guard,
            "case `{case_name}` relaxes_guard mismatch"
        );
        assert_eq!(
            policy.guard_mode(),
            expected_guard_mode,
            "case `{case_name}` guard_mode mismatch"
        );
    }
}

#[test]
fn dev_mode_relax_guard_policy_with_requested_local_tuple_relaxes() {
    let policy =
        DevModePolicy::with_env_flags(DevModeEnvFlags::from_env(true, LOCALHOST_BIND, false), true);
    assert!(policy.relaxes_guard());
    assert_eq!(policy.guard_mode(), GuardMode::Relaxed);
}

#[test]
fn dev_mode_relax_guard_strict_policy_remains_strict() {
    let policy = DevModePolicy::strict();
    assert!(!policy.relaxes_guard());
    assert_eq!(policy.guard_mode(), GuardMode::Strict);
}
