use axonrunner_apps::gateway;
use axonrunner_core::{DecisionOutcome, DomainEvent, ExecutionMode, PolicyCode};

fn make_request(body: &str, source_ip: &str) -> gateway::HttpBoundaryRequest {
    gateway::HttpBoundaryRequest::new(
        gateway::GATEWAY_METHOD,
        gateway::GATEWAY_PATH,
        body,
        source_ip,
    )
}

#[test]
fn gateway_e2e_input_validation_failure_path() {
    let mut runtime = gateway::GatewayRuntime::new();
    let mut request = make_request("write:alpha=1", "127.0.0.1");
    request.method = String::from("GET");

    let response = runtime.handle(request);

    assert_eq!(response.status_code, 400);
    assert!(matches!(
        response.reject_reason,
        Some(gateway::GatewayRejectReason::MethodNotAllowed { .. })
    ));
    assert!(!response.processed());
    assert!(response.records.is_empty());
    assert!(response.events.is_empty());
    assert_eq!(runtime.state.revision, 0);
}

#[test]
fn gateway_e2e_policy_rejection_path() {
    let mut runtime = gateway::GatewayRuntime::new();
    let response = runtime.handle(make_request("freeze", "10.0.0.8"));

    assert!(response.processed());
    assert_eq!(response.status_code, 403);
    assert_eq!(response.decision, Some(DecisionOutcome::Rejected));
    assert_eq!(response.policy_code, Some(PolicyCode::UnauthorizedControl));
    assert_eq!(response.state.mode, ExecutionMode::Active);
    assert_eq!(response.state.denied_count, 1);
    assert_eq!(response.state.audit_count, 1);
    assert_eq!(response.events.len(), 4);

    let converted_intent_id = response
        .records
        .iter()
        .find_map(|record| match record {
            gateway::GatewayRecord::IntentConverted { intent_id, .. } => Some(intent_id.as_str()),
            _ => None,
        })
        .expect("processed input should emit an intent conversion record");
    let audit = response
        .records
        .iter()
        .find_map(|record| match record {
            gateway::GatewayRecord::PolicyAudited { audit, .. } => Some(audit),
            _ => None,
        })
        .expect("processed input should emit a policy audit record");

    assert_eq!(audit.intent_id, converted_intent_id);
    assert_eq!(audit.actor_id.as_deref(), Some("gateway"));
    assert_eq!(audit.state_revision, 0);
    assert!(!audit.allowed);
    assert_eq!(audit.code, PolicyCode::UnauthorizedControl);
    assert_eq!(audit.reason, "control actions require actor `system`");
    assert_eq!(response.policy_code, Some(audit.code));
    assert_eq!(response.message, audit.reason);
    assert!(response.events.iter().any(|event| matches!(
        event,
        DomainEvent::PolicyEvaluated { audit: event_audit } if event_audit == audit
    )));
}

#[test]
fn gateway_e2e_success_path_mutates_state() {
    let mut runtime = gateway::GatewayRuntime::new();
    let response = runtime.handle(make_request("write:alpha=42", "10.0.0.8"));

    assert!(response.processed());
    assert_eq!(response.status_code, 202);
    assert_eq!(response.decision, Some(DecisionOutcome::Accepted));
    assert_eq!(response.policy_code, Some(PolicyCode::Allowed));
    assert_eq!(response.state.facts.get("alpha"), Some(&String::from("42")));
    assert_eq!(response.state.revision, 4);
    assert_eq!(response.state.audit_count, 1);
    assert_eq!(response.state.denied_count, 0);
    assert_eq!(runtime.state.facts.get("alpha"), Some(&String::from("42")));

    let converted_intent_id = response
        .records
        .iter()
        .find_map(|record| match record {
            gateway::GatewayRecord::IntentConverted { intent_id, .. } => Some(intent_id.as_str()),
            _ => None,
        })
        .expect("processed input should emit an intent conversion record");
    let audit = response
        .records
        .iter()
        .find_map(|record| match record {
            gateway::GatewayRecord::PolicyAudited { audit, .. } => Some(audit),
            _ => None,
        })
        .expect("processed input should emit a policy audit record");

    assert_eq!(audit.intent_id, converted_intent_id);
    assert_eq!(audit.actor_id.as_deref(), Some("gateway"));
    assert_eq!(audit.state_revision, 0);
    assert!(audit.allowed);
    assert_eq!(audit.code, PolicyCode::Allowed);
    assert_eq!(audit.reason, "allowed");
    assert_eq!(response.policy_code, Some(audit.code));
    assert!(response.events.iter().any(|event| matches!(
        event,
        DomainEvent::PolicyEvaluated { audit: event_audit } if event_audit == audit
    )));
}

#[test]
fn gateway_e2e_processed_input_has_intent_conversion_record() {
    let mut runtime = gateway::GatewayRuntime::new();
    let response = runtime.handle(make_request("read:alpha", "10.0.0.8"));

    assert!(response.processed());
    let converted_intent_id = response
        .records
        .iter()
        .find_map(|record| match record {
            gateway::GatewayRecord::IntentConverted { intent_id, .. } => Some(intent_id.clone()),
            _ => None,
        })
        .expect("processed input should have an intent conversion record");

    assert!(response.records.iter().any(|record| matches!(
        record,
        gateway::GatewayRecord::PolicyAudited { audit, .. }
            if audit.intent_id == converted_intent_id
    )));
    assert!(response.events.iter().any(|event| matches!(
        event,
        DomainEvent::IntentAccepted { intent } if intent.intent_id == converted_intent_id
    )));
}
