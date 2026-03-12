use crate::error::{AdapterError, AdapterResult};

pub(crate) const DEFAULT_ERROR_BODY_PREVIEW_BYTES: usize = 200;
const ROUTED_TOPIC_PREFIX: &str = "axonrunner://channel/";
const LEGACY_ROUTED_TOPIC_PREFIX: &str = "axiom://channel/";

pub(crate) fn normalize_token(raw: String, field: &'static str) -> AdapterResult<String> {
    let token = raw.trim();
    if token.is_empty() {
        return Err(AdapterError::invalid_input(field, "must not be empty"));
    }
    if token.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            field,
            "must not contain whitespace",
        ));
    }
    Ok(token.to_string())
}

#[cfg(any(
    feature = "channel-discord",
    feature = "channel-slack",
    feature = "channel-irc",
    feature = "channel-matrix",
    feature = "channel-whatsapp"
))]
pub(crate) fn normalize_optional_value(
    raw: Option<String>,
    field: &'static str,
) -> AdapterResult<Option<String>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AdapterError::invalid_input(field, "must not be empty"));
    }
    if value.contains(char::is_whitespace) {
        return Err(AdapterError::invalid_input(
            field,
            "must not contain whitespace",
        ));
    }
    Ok(Some(value.to_string()))
}

pub(crate) fn normalize_allowed_users(
    users: Vec<String>,
    field: &'static str,
) -> AdapterResult<Vec<String>> {
    let mut normalized = Vec::new();
    for user in users {
        let user = user.trim();
        if user.is_empty() {
            return Err(AdapterError::invalid_input(field, "contains empty entry"));
        }
        if user.contains(char::is_whitespace) {
            return Err(AdapterError::invalid_input(
                field,
                "entries must not contain whitespace",
            ));
        }
        normalized.push(user.to_string());
    }
    normalized.sort_unstable();
    normalized.dedup();
    Ok(normalized)
}

#[cfg(any(feature = "channel-discord", feature = "channel-slack"))]
pub(crate) fn normalize_optional_https_url(
    raw: Option<String>,
    field: &'static str,
) -> AdapterResult<Option<String>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AdapterError::invalid_input(field, "must not be empty"));
    }
    if !value.starts_with("https://") {
        return Err(AdapterError::invalid_input(
            field,
            "must start with https://",
        ));
    }
    Ok(Some(value.to_string()))
}

pub(crate) fn truncate_utf8_preview(raw: &str, max_bytes: usize) -> &str {
    if raw.len() <= max_bytes {
        return raw;
    }
    let end = (0..=max_bytes)
        .rev()
        .find(|&i| raw.is_char_boundary(i))
        .unwrap_or(0);
    &raw[..end]
}

pub(crate) fn encode_routed_topic(adapter: &str, route: &str) -> String {
    format!("{ROUTED_TOPIC_PREFIX}{adapter}/{route}")
}

pub(crate) fn decode_routed_topic(topic: &str, adapter: &str) -> Option<String> {
    for base_prefix in [ROUTED_TOPIC_PREFIX, LEGACY_ROUTED_TOPIC_PREFIX] {
        let prefix = format!("{base_prefix}{adapter}/");
        if let Some(route) = topic.strip_prefix(prefix.as_str()) {
            let route = route.trim();
            if route.is_empty() {
                return None;
            }
            return Some(route.to_string());
        }
    }
    None
}

pub(crate) fn resolve_route_topic(topic: &str, adapter: &str) -> String {
    decode_routed_topic(topic, adapter).unwrap_or_else(|| topic.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_ERROR_BODY_PREVIEW_BYTES, decode_routed_topic, encode_routed_topic,
        resolve_route_topic, truncate_utf8_preview,
    };

    #[test]
    fn truncate_utf8_preview_caps_ascii_bytes() {
        let long_body = "x".repeat(500);
        let preview = truncate_utf8_preview(&long_body, DEFAULT_ERROR_BODY_PREVIEW_BYTES);
        assert_eq!(preview.len(), DEFAULT_ERROR_BODY_PREVIEW_BYTES);
    }

    #[test]
    fn truncate_utf8_preview_keeps_short_body() {
        let short = "error: bad request";
        let preview = truncate_utf8_preview(short, DEFAULT_ERROR_BODY_PREVIEW_BYTES);
        assert_eq!(preview, short);
    }

    #[test]
    fn truncate_utf8_preview_never_breaks_multibyte() {
        let sample = "가".repeat(80);
        let preview = truncate_utf8_preview(&sample, 200);
        assert!(preview.is_char_boundary(preview.len()));
        assert!(preview.len() <= 200);
    }

    #[test]
    fn routed_topic_roundtrip_encode_decode() {
        let topic = encode_routed_topic("telegram", "12345");
        assert_eq!(topic, "axonrunner://channel/telegram/12345");
        assert_eq!(
            decode_routed_topic(topic.as_str(), "telegram"),
            Some(String::from("12345"))
        );
    }

    #[test]
    fn resolve_route_topic_supports_legacy_and_canonical() {
        let canonical = "axonrunner://channel/matrix/!room:example.org";
        let legacy_canonical = "axiom://channel/matrix/!room:example.org";
        let legacy = "!room:example.org";

        assert_eq!(
            resolve_route_topic(canonical, "matrix"),
            String::from("!room:example.org")
        );
        assert_eq!(
            resolve_route_topic(legacy_canonical, "matrix"),
            String::from("!room:example.org")
        );
        assert_eq!(
            resolve_route_topic(legacy, "matrix"),
            String::from("!room:example.org")
        );
    }
}
