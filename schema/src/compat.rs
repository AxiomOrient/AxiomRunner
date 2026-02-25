use crate::legacy::{ParseLegacySpecError, SchemaVersion, parse_legacy_spec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatLevel {
    Exact,
    Compatible,
    LegacyBridge,
    Incompatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompatibilityReport {
    pub expected: SchemaVersion,
    pub actual: SchemaVersion,
    pub level: CompatLevel,
}

impl CompatibilityReport {
    pub const fn is_compatible(self) -> bool {
        !matches!(self.level, CompatLevel::Incompatible)
    }
}

pub fn check_compatibility(expected: SchemaVersion, actual: SchemaVersion) -> CompatibilityReport {
    let level = if expected == actual {
        CompatLevel::Exact
    } else if expected.major == actual.major {
        if actual.minor < expected.minor {
            CompatLevel::Incompatible
        } else {
            CompatLevel::Compatible
        }
    } else if expected.major == 2 && actual.major == 1 {
        CompatLevel::LegacyBridge
    } else {
        CompatLevel::Incompatible
    };

    CompatibilityReport {
        expected,
        actual,
        level,
    }
}

pub fn check_compatibility_from_str(
    expected: &str,
    actual: &str,
) -> Result<CompatibilityReport, ParseLegacySpecError> {
    let expected = parse_legacy_spec(expected)?;
    let actual = parse_legacy_spec(actual)?;
    Ok(check_compatibility(expected, actual))
}
