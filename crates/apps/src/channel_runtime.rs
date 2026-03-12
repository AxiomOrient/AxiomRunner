use axonrunner_adapters::{channel_registry, resolve_channel_id};

use crate::env_util::read_env_trimmed;

pub(crate) const ENV_RUNTIME_CHANNEL: &str = "AXONRUNNER_RUNTIME_CHANNEL";

fn available_runtime_channels() -> String {
    channel_registry()
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn resolve_optional_runtime_channel_id_from_env() -> Result<Option<String>, String> {
    resolve_optional_runtime_channel_id(|key| read_env_trimmed(key).ok().flatten())
}

pub(crate) fn resolve_runtime_channel_id(
    mut read_env: impl FnMut(&str) -> Option<String>,
) -> Result<String, String> {
    match resolve_optional_runtime_channel_id(|key| read_env(key))? {
        Some(channel_id) => Ok(channel_id),
        None => Err(format!(
            "missing {ENV_RUNTIME_CHANNEL}; set it to one of: {}",
            available_runtime_channels()
        )),
    }
}

pub(crate) fn resolve_optional_runtime_channel_id(
    mut read_env: impl FnMut(&str) -> Option<String>,
) -> Result<Option<String>, String> {
    let Some(raw) = read_env(ENV_RUNTIME_CHANNEL)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    resolve_channel_id(&raw)
        .map(str::to_owned)
        .map(Some)
        .ok_or_else(|| {
            format!(
                "unsupported {ENV_RUNTIME_CHANNEL} '{raw}'. supported channels: {}",
                available_runtime_channels()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ENV_RUNTIME_CHANNEL, resolve_optional_runtime_channel_id, resolve_runtime_channel_id,
    };

    #[test]
    fn runtime_channel_resolver_accepts_namespaced_alias() {
        let resolved = resolve_runtime_channel_id(|key| {
            if key == ENV_RUNTIME_CHANNEL {
                Some(String::from("channel.slack"))
            } else {
                None
            }
        })
        .expect("alias should resolve");

        assert_eq!(resolved, "slack");
    }

    #[test]
    fn runtime_channel_resolver_rejects_missing_env() {
        let error = resolve_runtime_channel_id(|_| None).expect_err("missing env should fail");
        assert!(
            error.contains(ENV_RUNTIME_CHANNEL),
            "error should mention env key, got: {error}"
        );
        assert!(
            error.contains("set it to one of"),
            "error should include supported channels hint, got: {error}"
        );
    }

    #[test]
    fn optional_runtime_channel_resolver_returns_none_when_absent() {
        let resolved = resolve_optional_runtime_channel_id(|_| None)
            .expect("absent env should not be an error");
        assert_eq!(resolved, None);
    }

    #[test]
    fn optional_runtime_channel_resolver_rejects_unknown_channel() {
        let error = resolve_optional_runtime_channel_id(|key| {
            if key == ENV_RUNTIME_CHANNEL {
                Some(String::from("unknown-channel"))
            } else {
                None
            }
        })
        .expect_err("unknown channel should fail");

        assert!(
            error.contains("unsupported"),
            "error should mention unsupported value, got: {error}"
        );
        assert!(
            error.contains("telegram"),
            "error should include supported channels list, got: {error}"
        );
    }
}
