#[path = "../src/legacy.rs"]
#[allow(dead_code)]
mod legacy;

use legacy::{
    ParseLegacySpecError, SchemaVersion, is_legacy_spec, normalize_legacy_spec, parse_legacy_spec,
};

#[test]
fn parse_legacy_versions_with_prefix_and_partial_segments() {
    assert_eq!(
        parse_legacy_spec("v1").expect("v1 should parse"),
        SchemaVersion::new(1, 0, 0)
    );
    assert_eq!(
        parse_legacy_spec("legacy: 1.2").expect("legacy: 1.2 should parse"),
        SchemaVersion::new(1, 2, 0)
    );
    assert_eq!(
        parse_legacy_spec("V1.2.3").expect("V1.2.3 should parse"),
        SchemaVersion::new(1, 2, 3)
    );
}

#[test]
fn normalize_legacy_spec_returns_canonical_form() {
    let normalized = normalize_legacy_spec("1.2").expect("should normalize");
    assert_eq!(normalized, "v1.2.0");
}

#[test]
fn parse_legacy_spec_rejects_invalid_strings() {
    assert_eq!(
        parse_legacy_spec("").unwrap_err(),
        ParseLegacySpecError::Empty
    );
    assert_eq!(
        parse_legacy_spec("1..0").unwrap_err(),
        ParseLegacySpecError::InvalidFormat
    );
    assert_eq!(
        parse_legacy_spec("1.2.3.4").unwrap_err(),
        ParseLegacySpecError::TooManySegments
    );
    assert_eq!(
        parse_legacy_spec("v1.x").unwrap_err(),
        ParseLegacySpecError::InvalidNumber
    );
}

#[test]
fn legacy_detector_tracks_major_version() {
    assert!(is_legacy_spec("v1.9.0"));
    assert!(!is_legacy_spec("v2.0.0"));
}
