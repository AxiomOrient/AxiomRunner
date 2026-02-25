use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    Cli,
    Webhook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMessage {
    pub kind: ChannelKind,
    pub route: String,
    pub payload: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliChannel {
    max_payload_bytes: usize,
}

impl CliChannel {
    pub const fn new(max_payload_bytes: usize) -> Self {
        Self { max_payload_bytes }
    }

    pub fn accept(&self, input: &str) -> Result<ChannelMessage, ChannelValidationError> {
        validate_payload(input, self.max_payload_bytes)?;
        Ok(ChannelMessage {
            kind: ChannelKind::Cli,
            route: String::from("cli"),
            payload: input.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebhookChannel {
    require_signature: bool,
    max_body_bytes: usize,
}

impl WebhookChannel {
    pub const fn new(require_signature: bool, max_body_bytes: usize) -> Self {
        Self {
            require_signature,
            max_body_bytes,
        }
    }

    pub fn accept(
        &self,
        input: WebhookInput<'_>,
    ) -> Result<ChannelMessage, ChannelValidationError> {
        validate_method(input.method)?;
        validate_path(input.path)?;
        validate_payload(input.body, self.max_body_bytes)?;

        if self.require_signature {
            let signature = input
                .signature
                .ok_or(ChannelValidationError::MissingSignature)?;
            if signature.trim().is_empty() {
                return Err(ChannelValidationError::MissingSignature);
            }
        }

        Ok(ChannelMessage {
            kind: ChannelKind::Webhook,
            route: input.path.to_owned(),
            payload: input.body.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebhookInput<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a str,
    pub signature: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelValidationError {
    EmptyPayload,
    PayloadTooLarge { limit: usize, actual: usize },
    ContainsNul,
    InvalidMethod,
    InvalidPath,
    MissingSignature,
}

impl fmt::Display for ChannelValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelValidationError::EmptyPayload => write!(f, "payload must not be empty"),
            ChannelValidationError::PayloadTooLarge { limit, actual } => {
                write!(f, "payload exceeds limit ({actual} > {limit})")
            }
            ChannelValidationError::ContainsNul => write!(f, "payload contains NUL byte"),
            ChannelValidationError::InvalidMethod => write!(f, "webhook method is not allowed"),
            ChannelValidationError::InvalidPath => write!(f, "webhook path is invalid"),
            ChannelValidationError::MissingSignature => write!(f, "webhook signature is required"),
        }
    }
}

impl std::error::Error for ChannelValidationError {}

fn validate_payload(payload: &str, max_bytes: usize) -> Result<(), ChannelValidationError> {
    if payload.trim().is_empty() {
        return Err(ChannelValidationError::EmptyPayload);
    }

    if payload.contains('\0') {
        return Err(ChannelValidationError::ContainsNul);
    }

    let actual = payload.len();
    if actual > max_bytes {
        return Err(ChannelValidationError::PayloadTooLarge {
            limit: max_bytes,
            actual,
        });
    }

    Ok(())
}

fn validate_method(method: &str) -> Result<(), ChannelValidationError> {
    let method = method.trim().to_ascii_uppercase();
    match method.as_str() {
        "POST" | "PUT" => Ok(()),
        _ => Err(ChannelValidationError::InvalidMethod),
    }
}

fn validate_path(path: &str) -> Result<(), ChannelValidationError> {
    if path.trim().is_empty() {
        return Err(ChannelValidationError::InvalidPath);
    }

    if !path.starts_with('/') || path.contains('\0') || path.contains("..") {
        return Err(ChannelValidationError::InvalidPath);
    }

    Ok(())
}
