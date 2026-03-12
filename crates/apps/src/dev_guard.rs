use std::fmt::{Display, Formatter};

use crate::config_loader::AppConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    DevProfileBlockedInRelease,
}

impl Display for GuardError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardError::DevProfileBlockedInRelease => {
                write!(f, "profile=dev is blocked in release builds")
            }
        }
    }
}

impl std::error::Error for GuardError {}

pub fn enforce_current_build(config: &AppConfig) -> Result<(), GuardError> {
    enforce_release_gate(config, !cfg!(debug_assertions))
}

pub fn enforce_release_gate(config: &AppConfig, is_release_build: bool) -> Result<(), GuardError> {
    let is_dev_profile = config.profile.eq_ignore_ascii_case("dev");

    if should_block_dev_profile_in_release(is_release_build, is_dev_profile) {
        return Err(GuardError::DevProfileBlockedInRelease);
    }

    Ok(())
}

pub const fn should_block_dev_profile_in_release(
    is_release_build: bool,
    is_dev_profile: bool,
) -> bool {
    is_release_build && is_dev_profile
}

#[cfg(test)]
mod tests {
    use super::{GuardError, enforce_release_gate, should_block_dev_profile_in_release};
    use crate::config_loader::AppConfig;

    fn dev_config() -> AppConfig {
        AppConfig {
            profile: String::from("dev"),
            provider: String::from("mock-local"),
        }
    }

    #[test]
    fn release_gate_blocks_dev_profile_in_release() {
        let result = enforce_release_gate(&dev_config(), true);

        assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
    }

    #[test]
    fn release_gate_allows_dev_profile_in_debug_build() {
        let result = enforce_release_gate(&dev_config(), false);

        assert!(result.is_ok());
    }

    #[test]
    fn release_gate_treats_profile_case_insensitively() {
        let config = AppConfig {
            profile: String::from("DeV"),
            provider: String::from("mock-local"),
        };

        let result = enforce_release_gate(&config, true);

        assert_eq!(result, Err(GuardError::DevProfileBlockedInRelease));
    }

    #[test]
    fn block_rule_remains_explicit() {
        assert!(should_block_dev_profile_in_release(true, true));
        assert!(!should_block_dev_profile_in_release(true, false));
        assert!(!should_block_dev_profile_in_release(false, true));
    }
}
