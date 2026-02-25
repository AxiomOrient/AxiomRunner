#[path = "../src/compat.rs"]
mod compat;
#[path = "../src/legacy.rs"]
#[allow(dead_code)]
mod legacy;

use compat::{CompatLevel, check_compatibility, check_compatibility_from_str};
use legacy::SchemaVersion;

#[test]
fn compatibility_accepts_exact_and_minor_upgrades_in_same_major() {
    let exact = check_compatibility(SchemaVersion::new(2, 1, 0), SchemaVersion::new(2, 1, 0));
    assert_eq!(exact.level, CompatLevel::Exact);
    assert!(exact.is_compatible());

    let upgraded = check_compatibility(SchemaVersion::new(2, 1, 0), SchemaVersion::new(2, 3, 4));
    assert_eq!(upgraded.level, CompatLevel::Compatible);
    assert!(upgraded.is_compatible());
}

#[test]
fn compatibility_rejects_required_minor_that_is_missing() {
    let report = check_compatibility(SchemaVersion::new(2, 3, 0), SchemaVersion::new(2, 2, 9));
    assert_eq!(report.level, CompatLevel::Incompatible);
    assert!(!report.is_compatible());
}

#[test]
fn compatibility_supports_legacy_bridge_for_v1_to_v2_boundary() {
    let report = check_compatibility(SchemaVersion::new(2, 0, 0), SchemaVersion::new(1, 9, 9));
    assert_eq!(report.level, CompatLevel::LegacyBridge);
    assert!(report.is_compatible());
}

#[test]
fn compatibility_parser_handles_string_inputs() {
    let report = check_compatibility_from_str("v2.1.0", "2.2.0").expect("should parse and compare");
    assert_eq!(report.level, CompatLevel::Compatible);
}

#[test]
fn compatibility_keeps_major_mismatch_incompatible() {
    let report = check_compatibility(SchemaVersion::new(3, 0, 0), SchemaVersion::new(2, 9, 9));
    assert_eq!(report.level, CompatLevel::Incompatible);
    assert!(!report.is_compatible());
}
