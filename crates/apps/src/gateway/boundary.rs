use axonrunner_core::{Intent, IntentKind};
use std::net::IpAddr;

use super::{
    GATEWAY_METHOD, GATEWAY_PATH, GatewayRejectReason, HttpBoundaryRequest, MAX_BODY_BYTES,
};
use crate::cli_command::{IntentSpecVariant, parse_intent_spec_raw};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedBoundaryRequest {
    source_ip: IpAddr,
    template: GatewayIntentTemplate,
}

impl ValidatedBoundaryRequest {
    pub(super) fn source_ip(&self) -> IpAddr {
        self.source_ip
    }

    pub(super) fn into_intent(self, intent_id: String, actor_id: Option<String>) -> Intent {
        self.template.to_intent(intent_id, actor_id)
    }
}

pub(super) fn validate_boundary_request(
    request: &HttpBoundaryRequest,
) -> Result<ValidatedBoundaryRequest, GatewayRejectReason> {
    let method = request.method.trim().to_ascii_uppercase();
    if method != GATEWAY_METHOD {
        return Err(GatewayRejectReason::MethodNotAllowed { method });
    }

    let path = request.path.trim().to_owned();
    if path != GATEWAY_PATH {
        return Err(GatewayRejectReason::PathNotAllowed { path });
    }

    if request.body.trim().is_empty() {
        return Err(GatewayRejectReason::BodyEmpty);
    }

    if request.body.contains('\0') {
        return Err(GatewayRejectReason::BodyContainsNul);
    }

    let actual = request.body.len();
    if actual > MAX_BODY_BYTES {
        return Err(GatewayRejectReason::BodyTooLarge {
            limit: MAX_BODY_BYTES,
            actual,
        });
    }

    let source_ip_value = request.source_ip.trim();
    let source_ip: IpAddr =
        source_ip_value
            .parse()
            .map_err(|_| GatewayRejectReason::SourceIpInvalid {
                source_ip: request.source_ip.clone(),
            })?;

    if !source_ip_allowed(source_ip) {
        return Err(GatewayRejectReason::SourceIpNotAllowed {
            source_ip: source_ip.to_string(),
        });
    }

    let template = GatewayIntentTemplate::parse(&request.body)?;
    Ok(ValidatedBoundaryRequest {
        source_ip,
        template,
    })
}

fn source_ip_allowed(source_ip: IpAddr) -> bool {
    match source_ip {
        IpAddr::V4(ipv4) => ipv4.is_loopback() || ipv4.is_private(),
        IpAddr::V6(ipv6) => ipv6.is_loopback() || ipv6.is_unique_local(),
    }
}

pub(super) fn actor_id_for_source_ip(source_ip: IpAddr) -> &'static str {
    if source_ip.is_loopback() {
        "system"
    } else {
        "gateway"
    }
}

pub(super) fn intent_kind_name(kind: &IntentKind) -> &'static str {
    match kind {
        IntentKind::ReadFact { .. } => "read",
        IntentKind::WriteFact { .. } => "write",
        IntentKind::RemoveFact { .. } => "remove",
        IntentKind::FreezeWrites => "freeze",
        IntentKind::Halt => "halt",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GatewayIntentTemplate {
    Read { key: String },
    Write { key: String, value: String },
    Remove { key: String },
    Freeze,
    Halt,
}

impl GatewayIntentTemplate {
    fn parse(spec: &str) -> Result<Self, GatewayRejectReason> {
        parse_intent_spec_raw(spec)
            .map_err(|_| invalid_intent("unsupported intent spec"))
            .map(|variant| match variant {
                IntentSpecVariant::Read { key } => Self::Read { key },
                IntentSpecVariant::Write { key, value } => Self::Write { key, value },
                IntentSpecVariant::Remove { key } => Self::Remove { key },
                IntentSpecVariant::Freeze => Self::Freeze,
                IntentSpecVariant::Halt => Self::Halt,
            })
    }

    fn to_intent(&self, intent_id: String, actor_id: Option<String>) -> Intent {
        match self {
            GatewayIntentTemplate::Read { key } => Intent::read(intent_id, actor_id, key.clone()),
            GatewayIntentTemplate::Write { key, value } => {
                Intent::write(intent_id, actor_id, key.clone(), value.clone())
            }
            GatewayIntentTemplate::Remove { key } => {
                Intent::remove(intent_id, actor_id, key.clone())
            }
            GatewayIntentTemplate::Freeze => Intent::freeze_writes(intent_id, actor_id),
            GatewayIntentTemplate::Halt => Intent::halt(intent_id, actor_id),
        }
    }
}

fn invalid_intent(detail: &str) -> GatewayRejectReason {
    GatewayRejectReason::BodyInvalidIntent {
        detail: detail.to_owned(),
    }
}
