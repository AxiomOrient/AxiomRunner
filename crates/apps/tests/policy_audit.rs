use std::fs;

use axonrunner_core::audit::{
    format_policy_decision_audit_line, policy_reason_code, policy_risk_effect_path,
};
use axonrunner_core::{AgentState, Intent, PolicyCode, build_policy_audit, evaluate_policy};

#[test]
fn policy_audit_logs_reason_code_and_risk_effect_path_for_denied_policy() {
    let state = AgentState {
        revision: 9,
        ..AgentState::default()
    };
    let intent = Intent::freeze_writes("policy-audit-denied", Some("gateway".to_owned()));
    let verdict = evaluate_policy(&state, &intent);
    let audit = build_policy_audit(&state, &intent, &verdict);

    assert!(!audit.allowed);
    assert_eq!(audit.code, PolicyCode::UnauthorizedControl);
    assert_eq!(policy_reason_code(&audit), "unauthorized_control");
    assert_eq!(
        policy_risk_effect_path(&intent, &audit),
        "risk/high/effect/blocked"
    );

    let line = format_policy_decision_audit_line(&intent, &audit);
    assert!(line.contains("reason_code=unauthorized_control"));
    assert!(line.contains("risk_effect_path=risk/high/effect/blocked"));
}

#[test]
fn policy_audit_writes_sample_artifact_under_target() {
    let denied_intent =
        Intent::freeze_writes("policy-audit-sample-denied", Some("gateway".to_owned()));
    let denied_verdict = evaluate_policy(&AgentState::default(), &denied_intent);
    let denied_audit = build_policy_audit(&AgentState::default(), &denied_intent, &denied_verdict);
    let denied_line = format_policy_decision_audit_line(&denied_intent, &denied_audit);

    let allowed_intent = Intent::write(
        "policy-audit-sample-allowed",
        Some("alice".to_owned()),
        "alpha",
        "1",
    );
    let allowed_verdict = evaluate_policy(&AgentState::default(), &allowed_intent);
    let allowed_audit =
        build_policy_audit(&AgentState::default(), &allowed_intent, &allowed_verdict);
    let allowed_line = format_policy_decision_audit_line(&allowed_intent, &allowed_audit);

    assert!(allowed_audit.allowed);
    assert_eq!(policy_reason_code(&allowed_audit), "allowed");
    assert_eq!(
        policy_risk_effect_path(&allowed_intent, &allowed_audit),
        "risk/none/effect/write_fact"
    );

    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target")
        .join("policy_audit");
    fs::create_dir_all(&artifact_dir).expect("artifact directory should be created");

    let artifact_path = artifact_dir.join("sample_audit.log");
    let artifact_text = format!("{denied_line}\n{allowed_line}\n");
    fs::write(&artifact_path, artifact_text)
        .expect("sample policy audit artifact should be written");

    let written = fs::read_to_string(&artifact_path).expect("written artifact should be readable");
    assert!(written.contains("reason_code=unauthorized_control"));
    assert!(written.contains("risk_effect_path=risk/high/effect/blocked"));
    assert!(written.contains("reason_code=allowed"));
    assert!(written.contains("risk_effect_path=risk/none/effect/write_fact"));
}
