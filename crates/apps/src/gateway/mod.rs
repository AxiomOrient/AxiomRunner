use axiom_apps::metrics::{
    MetricsSnapshot, record_copy_bytes, record_lock_wait_ns, record_queue_depth,
};
use axiom_core::{
    AgentState, Decision, DecisionOutcome, DomainEvent, Intent, PolicyAuditRecord, PolicyCode,
    build_policy_audit, decide, evaluate_policy, project_from,
};
use std::env;

mod boundary;
pub mod signature;

use boundary::{
    ValidatedBoundaryRequest, actor_id_for_source_ip, intent_kind_name, validate_boundary_request,
};

pub const GATEWAY_METHOD: &str = "POST";
pub const GATEWAY_PATH: &str = "/v1/intents";
pub const MAX_BODY_BYTES: usize = 4096;

const ENV_GATEWAY_REQUESTS: &str = "AXIOM_GATEWAY_REQUESTS";
const DEFAULT_GATEWAY_SOURCE_IP: &str = "127.0.0.1";
const DEFAULT_GATEWAY_BODY: &str = "read:health";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayRunRequest {
    pub source_ip: String,
    pub body: String,
    pub signature: Option<String>,
}

impl GatewayRunRequest {
    pub fn new(source_ip: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            source_ip: source_ip.into(),
            body: body.into(),
            signature: None,
        }
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        let signature = signature.into();
        self.signature = if signature.trim().is_empty() {
            None
        } else {
            Some(signature)
        };
        self
    }

    fn to_http_request(&self) -> HttpBoundaryRequest {
        HttpBoundaryRequest::new(GATEWAY_METHOD, GATEWAY_PATH, &self.body, &self.source_ip)
            .with_signature_opt(self.signature.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayRunSummary {
    pub total_requests: u64,
    pub accepted_requests: u64,
    pub rejected_requests: u64,
    pub final_revision: u64,
    pub queue_peak_depth: u64,
}

pub fn run(profile: &str, endpoint: &str) {
    println!("gateway started profile={} endpoint={}", profile, endpoint);

    let requests = match load_gateway_requests() {
        Ok(requests) => requests,
        Err(error) => {
            eprintln!("gateway request config error: {error}; falling back to default request");
            default_gateway_requests()
        }
    };

    let summary = execute_gateway_run(requests);
    println!(
        "gateway summary total={} accepted={} rejected={} revision={} queue_peak={}",
        summary.total_requests,
        summary.accepted_requests,
        summary.rejected_requests,
        summary.final_revision,
        summary.queue_peak_depth
    );
}

pub fn execute_gateway_run(requests: Vec<GatewayRunRequest>) -> GatewayRunSummary {
    let mut runtime = GatewayRuntime::new();
    let mut accepted_requests = 0_u64;
    let mut rejected_requests = 0_u64;

    for request in requests {
        let response = runtime.handle(request.to_http_request());
        if response.processed() {
            accepted_requests = accepted_requests.saturating_add(1);
        } else {
            rejected_requests = rejected_requests.saturating_add(1);
        }
    }

    let metrics = runtime.metrics_snapshot();
    GatewayRunSummary {
        total_requests: accepted_requests.saturating_add(rejected_requests),
        accepted_requests,
        rejected_requests,
        final_revision: runtime.state.revision,
        queue_peak_depth: metrics.queue.peak_depth,
    }
}

pub fn parse_gateway_requests(raw: &str) -> Result<Vec<GatewayRunRequest>, String> {
    let mut requests = Vec::new();

    for (index, raw_line) in raw.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(3, '|');
        let source_ip = parts.next().unwrap_or_default();
        let body = parts.next().ok_or_else(|| {
            format!(
                "invalid gateway request line {}: expected '<source_ip>|<intent-spec>' or '<source_ip>|<intent-spec>|<signature-hex>'",
                index + 1
            )
        })?;
        let signature = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        let source_ip = source_ip.trim();
        let body = body.trim();

        if source_ip.is_empty() {
            return Err(format!(
                "invalid gateway request line {}: source_ip is empty",
                index + 1
            ));
        }

        if body.is_empty() {
            return Err(format!(
                "invalid gateway request line {}: intent-spec is empty",
                index + 1
            ));
        }

        let mut request = GatewayRunRequest::new(source_ip, body);
        request.signature = signature;
        requests.push(request);
    }

    if requests.is_empty() {
        return Err(String::from("gateway request list is empty"));
    }

    Ok(requests)
}

fn load_gateway_requests() -> Result<Vec<GatewayRunRequest>, String> {
    match env::var(ENV_GATEWAY_REQUESTS) {
        Ok(raw) => parse_gateway_requests(&raw),
        Err(_) => Ok(default_gateway_requests()),
    }
}

fn default_gateway_requests() -> Vec<GatewayRunRequest> {
    vec![GatewayRunRequest::new(
        DEFAULT_GATEWAY_SOURCE_IP,
        DEFAULT_GATEWAY_BODY,
    )]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpBoundaryRequest {
    pub method: String,
    pub path: String,
    pub body: String,
    pub source_ip: String,
    pub signature: Option<String>,
}

impl HttpBoundaryRequest {
    pub fn new(method: &str, path: &str, body: &str, source_ip: &str) -> Self {
        Self {
            method: method.to_owned(),
            path: path.to_owned(),
            body: body.to_owned(),
            source_ip: source_ip.to_owned(),
            signature: None,
        }
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        let signature = signature.into();
        self.signature = if signature.trim().is_empty() {
            None
        } else {
            Some(signature)
        };
        self
    }

    pub fn with_signature_opt(mut self, signature: Option<String>) -> Self {
        self.signature = signature.and_then(|value| {
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpBoundaryResponse {
    pub request_id: String,
    pub status_code: u16,
    pub message: String,
    pub reject_reason: Option<GatewayRejectReason>,
    pub decision: Option<DecisionOutcome>,
    pub policy_code: Option<PolicyCode>,
    pub events: Vec<DomainEvent>,
    pub records: Vec<GatewayRecord>,
    pub state: AgentState,
}

impl HttpBoundaryResponse {
    pub fn processed(&self) -> bool {
        self.reject_reason.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayRecord {
    IntentConverted {
        request_id: String,
        intent_id: String,
        actor_id: String,
        source_ip: String,
        kind: String,
    },
    PolicyAudited {
        request_id: String,
        audit: PolicyAuditRecord,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayRejectReason {
    MethodNotAllowed { method: String },
    PathNotAllowed { path: String },
    BodyEmpty,
    BodyContainsNul,
    BodyTooLarge { limit: usize, actual: usize },
    BodyInvalidIntent { detail: String },
    SourceIpInvalid { source_ip: String },
    SourceIpNotAllowed { source_ip: String },
    SignatureInvalid,
}

impl GatewayRejectReason {
    fn status_code(&self) -> u16 {
        match self {
            GatewayRejectReason::SignatureInvalid => 401,
            _ => 400,
        }
    }

    fn message(&self) -> String {
        match self {
            GatewayRejectReason::MethodNotAllowed { method } => {
                format!("method is not allowed: {method}")
            }
            GatewayRejectReason::PathNotAllowed { path } => {
                format!("path is not allowed: {path}")
            }
            GatewayRejectReason::BodyEmpty => String::from("body must not be empty"),
            GatewayRejectReason::BodyContainsNul => String::from("body contains NUL byte"),
            GatewayRejectReason::BodyTooLarge { limit, actual } => {
                format!("body exceeds limit ({actual} > {limit})")
            }
            GatewayRejectReason::BodyInvalidIntent { detail } => {
                format!("body intent is invalid: {detail}")
            }
            GatewayRejectReason::SourceIpInvalid { source_ip } => {
                format!("source_ip is invalid: {source_ip}")
            }
            GatewayRejectReason::SourceIpNotAllowed { source_ip } => {
                format!("source_ip is not allowed: {source_ip}")
            }
            GatewayRejectReason::SignatureInvalid => String::from("signature_invalid"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GatewayRuntime {
    pub state: AgentState,
    next_request_seq: u64,
    next_intent_seq: u64,
    metrics: MetricsSnapshot,
}

impl GatewayRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics
    }

    pub fn handle(&mut self, request: HttpBoundaryRequest) -> HttpBoundaryResponse {
        let request_id = self.next_request_id();
        self.record_input_metrics(&request);

        let response = match self.signature_reject_reason(&request) {
            Some(reason) => self.rejected_response(request_id, reason),
            None => match validate_boundary_request(&request) {
                Ok(validated) => self.process_validated_request(request_id, validated),
                Err(reason) => self.rejected_response(request_id, reason),
            },
        };

        self.record_output_metrics(&response);
        response
    }

    fn next_request_id(&mut self) -> String {
        self.next_request_seq = self.next_request_seq.saturating_add(1);
        format!("gw-req-{}", self.next_request_seq)
    }

    fn next_intent_id(&mut self) -> String {
        self.next_intent_seq = self.next_intent_seq.saturating_add(1);
        format!("gw-intent-{}", self.next_intent_seq)
    }

    fn record_input_metrics(&mut self, request: &HttpBoundaryRequest) {
        self.metrics = record_queue_depth(self.metrics, 1);
        self.metrics = record_copy_bytes(self.metrics, request.body.len() as u64, 0);
    }

    fn record_output_metrics(&mut self, response: &HttpBoundaryResponse) {
        self.metrics = record_lock_wait_ns(self.metrics, 0);
        self.metrics = record_queue_depth(self.metrics, 0);
        self.metrics = record_copy_bytes(self.metrics, 0, response_output_bytes(response));
    }

    fn signature_reject_reason(
        &self,
        request: &HttpBoundaryRequest,
    ) -> Option<GatewayRejectReason> {
        let secret = signature::load_gateway_secret()?;
        let provided_sig = request.signature.as_deref().unwrap_or_default();
        let verified = signature::verify_request_signature(
            request.body.as_bytes(),
            secret.as_bytes(),
            provided_sig,
        );
        if verified {
            None
        } else {
            Some(GatewayRejectReason::SignatureInvalid)
        }
    }

    fn rejected_response(
        &self,
        request_id: String,
        reason: GatewayRejectReason,
    ) -> HttpBoundaryResponse {
        HttpBoundaryResponse {
            request_id,
            status_code: reason.status_code(),
            message: reason.message(),
            reject_reason: Some(reason),
            decision: None,
            policy_code: None,
            events: Vec::new(),
            records: Vec::new(),
            state: self.state.clone(),
        }
    }

    fn process_validated_request(
        &mut self,
        request_id: String,
        validated: ValidatedBoundaryRequest,
    ) -> HttpBoundaryResponse {
        let source_ip = validated.source_ip();
        let actor_id_str = actor_id_for_source_ip(source_ip);
        let intent_id = self.next_intent_id();
        let intent = validated.into_intent(intent_id, Some(actor_id_str.to_owned()));

        let verdict = evaluate_policy(&self.state, &intent);
        let decision = decide(&intent, &verdict);
        let audit = build_policy_audit(&self.state, &intent, &verdict);
        let projection = build_decision_projection(
            &request_id,
            source_ip,
            actor_id_str,
            intent,
            audit,
            decision,
        );

        self.state = project_from(&self.state, &projection.events);

        HttpBoundaryResponse {
            request_id,
            status_code: status_code_for_decision(projection.outcome),
            message: projection.message,
            reject_reason: None,
            decision: Some(projection.outcome),
            policy_code: Some(projection.policy_code),
            events: projection.events,
            records: projection.records,
            state: self.state.clone(),
        }
    }
}

struct GatewayDecisionProjection {
    outcome: DecisionOutcome,
    message: String,
    policy_code: PolicyCode,
    events: Vec<DomainEvent>,
    records: Vec<GatewayRecord>,
}

fn build_decision_projection(
    request_id: &str,
    source_ip: std::net::IpAddr,
    actor_id: &str,
    intent: Intent,
    audit: PolicyAuditRecord,
    decision: Decision,
) -> GatewayDecisionProjection {
    let intent_id = intent.intent_id.clone();
    let kind = intent_kind_name(&intent.kind).to_owned();
    let source_ip = source_ip.to_string();
    let outcome = decision.outcome;
    let message = decision.reason.clone();
    let effects = decision.effects.clone();
    let policy_code = audit.code;
    let records = vec![
        GatewayRecord::IntentConverted {
            request_id: request_id.to_owned(),
            intent_id,
            actor_id: actor_id.to_owned(),
            source_ip,
            kind,
        },
        GatewayRecord::PolicyAudited {
            request_id: request_id.to_owned(),
            audit: audit.clone(),
        },
    ];
    let events = vec![
        DomainEvent::IntentAccepted { intent },
        DomainEvent::PolicyEvaluated { audit },
        DomainEvent::DecisionCalculated { decision },
        DomainEvent::EffectsApplied { effects },
    ];

    GatewayDecisionProjection {
        outcome,
        message,
        policy_code,
        events,
        records,
    }
}

fn status_code_for_decision(outcome: DecisionOutcome) -> u16 {
    match outcome {
        DecisionOutcome::Accepted => 202,
        DecisionOutcome::Rejected => 403,
    }
}

fn response_output_bytes(response: &HttpBoundaryResponse) -> u64 {
    response
        .request_id
        .len()
        .saturating_add(response.message.len()) as u64
}

#[cfg(test)]
mod tests {
    use super::{
        GatewayRunRequest, default_gateway_requests, execute_gateway_run, parse_gateway_requests,
    };

    #[test]
    fn parse_gateway_requests_reads_line_pairs() {
        let parsed = parse_gateway_requests("127.0.0.1|read:alpha\n10.0.0.8|write:key=value")
            .expect("request list should parse");

        assert_eq!(
            parsed,
            vec![
                GatewayRunRequest::new("127.0.0.1", "read:alpha"),
                GatewayRunRequest::new("10.0.0.8", "write:key=value")
            ]
        );
    }

    #[test]
    fn parse_gateway_requests_reads_optional_signature() {
        let parsed =
            parse_gateway_requests("127.0.0.1|read:alpha|abc123\n10.0.0.8|write:key=value|")
                .expect("request list with signatures should parse");

        assert_eq!(
            parsed,
            vec![
                GatewayRunRequest::new("127.0.0.1", "read:alpha").with_signature("abc123"),
                GatewayRunRequest::new("10.0.0.8", "write:key=value")
            ]
        );
    }

    #[test]
    fn parse_gateway_requests_rejects_malformed_line() {
        let error =
            parse_gateway_requests("127.0.0.1 read:alpha").expect_err("malformed line should fail");

        assert!(error.contains("expected '<source_ip>|<intent-spec>'"));
    }

    #[test]
    fn execute_gateway_run_uses_default_request_successfully() {
        let summary = execute_gateway_run(default_gateway_requests());

        assert_eq!(summary.total_requests, 1);
        assert_eq!(summary.accepted_requests, 1);
        assert_eq!(summary.rejected_requests, 0);
        assert_eq!(summary.final_revision, 4);
        assert!(summary.queue_peak_depth >= 1);
    }
}
