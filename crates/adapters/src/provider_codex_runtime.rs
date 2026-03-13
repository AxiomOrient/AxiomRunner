use crate::contracts::{
    AdapterFuture, ProviderAdapter, ProviderHealthReport, ProviderRequest, ProviderResponse,
};
use crate::error::{AdapterError, RetryClass};
use codex_runtime::runtime::{Client, ClientConfig, SessionConfig};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::sync::Mutex;

pub const ENV_CODEX_BIN: &str = "AXONRUNNER_CODEX_BIN";
const DEFAULT_CODEX_BIN: &str = "codex";
const SESSION_TIMEOUT_SECS: u64 = 120;
const MIN_SUPPORTED_CODEX_CLI: &str = "0.104.0";

struct ActiveSession {
    client: Client,
    session: codex_runtime::runtime::Session,
    cwd: String,
    model: String,
}

pub struct CodexRuntimeProvider {
    id_str: &'static str,
    cli_bin: PathBuf,
    active_session: Mutex<Option<ActiveSession>>,
}

impl CodexRuntimeProvider {
    pub fn new(id_str: &'static str) -> Self {
        Self {
            id_str,
            cli_bin: cli_bin_from_env(),
            active_session: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn new_with_cli_bin(id_str: &'static str, cli_bin: impl Into<PathBuf>) -> Self {
        Self {
            id_str,
            cli_bin: cli_bin.into(),
            active_session: Mutex::new(None),
        }
    }

    async fn session_for_request(
        &self,
        request: &ProviderRequest,
    ) -> Result<codex_runtime::runtime::Session, AdapterError> {
        let stale_session = {
            let mut guard = self.active_session.lock().await;
            if let Some(active) = guard.as_ref()
                && can_reuse_session(active.cwd.as_str(), active.model.as_str(), request)
                && !active.session.is_closed()
            {
                return Ok(active.session.clone());
            }
            guard.take()
        };

        if let Some(stale) = stale_session {
            close_active_session(stale).await?;
        }

        let client = Client::connect(ClientConfig::new().with_cli_bin(self.cli_bin.clone()))
            .await
            .map_err(|error| {
                AdapterError::failed(
                    "codex_runtime.connect",
                    error.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;

        let session = client
            .start_session(
                SessionConfig::new(request.cwd.clone())
                    .with_model(request.model.clone())
                    .with_timeout(Duration::from_secs(SESSION_TIMEOUT_SECS)),
            )
            .await
            .map_err(|error| {
                AdapterError::failed(
                    "codex_runtime.start_session",
                    error.to_string(),
                    RetryClass::NonRetryable,
                )
            })?;

        let reusable = session.clone();
        let mut guard = self.active_session.lock().await;
        *guard = Some(ActiveSession {
            client,
            session,
            cwd: request.cwd.clone(),
            model: request.model.clone(),
        });
        Ok(reusable)
    }

    async fn drop_cached_session(&self) -> Result<(), AdapterError> {
        let stale = {
            let mut guard = self.active_session.lock().await;
            guard.take()
        };
        if let Some(stale) = stale {
            close_active_session(stale).await?;
        }
        Ok(())
    }
}

fn can_reuse_session(active_cwd: &str, active_model: &str, request: &ProviderRequest) -> bool {
    active_cwd == request.cwd && active_model == request.model
}

fn cli_bin_from_env() -> PathBuf {
    std::env::var_os(ENV_CODEX_BIN)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CODEX_BIN))
}

impl ProviderAdapter for CodexRuntimeProvider {
    fn id(&self) -> &str {
        self.id_str
    }

    fn health(&self) -> AdapterFuture<'_, ProviderHealthReport> {
        let cli_bin = self.cli_bin.clone();
        Box::pin(async move { probe_codex_runtime(cli_bin).await })
    }

    fn complete(&self, request: ProviderRequest) -> AdapterFuture<'_, ProviderResponse> {
        Box::pin(async move {
            if request.prompt.trim().is_empty() {
                return Err(AdapterError::invalid_input("prompt", "must not be empty"));
            }
            if request.max_tokens == 0 {
                return Err(AdapterError::invalid_input(
                    "max_tokens",
                    "must be greater than zero",
                ));
            }
            if request.cwd.trim().is_empty() {
                return Err(AdapterError::invalid_input("cwd", "must not be empty"));
            }

            let session = self.session_for_request(&request).await?;
            let result = match session.ask(request.prompt).await {
                Ok(result) => result,
                Err(error) => {
                    let _ = self.drop_cached_session().await;
                    return Err(AdapterError::failed(
                        "codex_runtime.ask",
                        error.to_string(),
                        RetryClass::NonRetryable,
                    ));
                }
            };

            Ok(ProviderResponse {
                content: result.assistant_text,
            })
        })
    }

    fn shutdown(&self) -> AdapterFuture<'_, ()> {
        Box::pin(async move { self.drop_cached_session().await })
    }
}

async fn close_active_session(active: ActiveSession) -> Result<(), AdapterError> {
    active.session.close().await.map_err(|error| {
        AdapterError::failed(
            "codex_runtime.close",
            error.to_string(),
            RetryClass::NonRetryable,
        )
    })?;

    active.client.shutdown().await.map_err(|error| {
        AdapterError::failed(
            "codex_runtime.shutdown",
            error.to_string(),
            RetryClass::NonRetryable,
        )
    })
}

async fn probe_codex_runtime(cli_bin: PathBuf) -> Result<ProviderHealthReport, AdapterError> {
    let resolved = match resolve_cli_bin(&cli_bin) {
        Ok(resolved) => resolved,
        Err(reason) => return Ok(ProviderHealthReport::blocked(reason)),
    };
    let version = match probe_codex_version(&resolved) {
        Ok(version) => version,
        Err(error) => {
            return Ok(ProviderHealthReport::blocked(format!(
                "cli_bin={},probe_error={}",
                resolved.display(),
                sanitize_detail(&error.to_string())
            )));
        }
    };
    let compatibility = compatibility_for_version(&version);
    match compatibility {
        CodexCliCompatibility::Blocked => {
            return Ok(ProviderHealthReport::blocked(format!(
                "cli_bin={},version={},compatibility=blocked,min_supported={},reason=below_minimum",
                resolved.display(),
                version,
                MIN_SUPPORTED_CODEX_CLI
            )));
        }
        CodexCliCompatibility::Unknown => {
            return Ok(ProviderHealthReport::degraded(format!(
                "cli_bin={},version={},compatibility=unknown,min_supported={},reason=unparseable_version",
                resolved.display(),
                version,
                MIN_SUPPORTED_CODEX_CLI
            )));
        }
        CodexCliCompatibility::Supported => {}
    }
    let client = match Client::connect(ClientConfig::new().with_cli_bin(&resolved)).await {
        Ok(client) => client,
        Err(error) => {
            return Ok(ProviderHealthReport::blocked(format!(
                "cli_bin={},version={},compatibility=supported,min_supported={},handshake_error={}",
                resolved.display(),
                version,
                MIN_SUPPORTED_CODEX_CLI,
                sanitize_detail(&error.to_string())
            )));
        }
    };

    match client.shutdown().await {
        Ok(()) => Ok(ProviderHealthReport::ready(format!(
            "cli_bin={},version={},compatibility=supported,min_supported={},handshake=ok",
            resolved.display(),
            version,
            MIN_SUPPORTED_CODEX_CLI
        ))),
        Err(error) => Ok(ProviderHealthReport::degraded(format!(
            "cli_bin={},version={},compatibility=supported,min_supported={},shutdown_error={}",
            resolved.display(),
            version,
            MIN_SUPPORTED_CODEX_CLI,
            sanitize_detail(&error.to_string())
        ))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexCliCompatibility {
    Supported,
    Unknown,
    Blocked,
}

fn compatibility_for_version(version: &str) -> CodexCliCompatibility {
    let Some(found) = parse_semver_triplet(version) else {
        return CodexCliCompatibility::Unknown;
    };
    let Some(minimum) = parse_semver_triplet(MIN_SUPPORTED_CODEX_CLI) else {
        return CodexCliCompatibility::Unknown;
    };
    if found >= minimum {
        CodexCliCompatibility::Supported
    } else {
        CodexCliCompatibility::Blocked
    }
}

fn parse_semver_triplet(raw: &str) -> Option<(u64, u64, u64)> {
    for token in raw.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.')) {
        if token.chars().filter(|ch| *ch == '.').count() < 2 {
            continue;
        }
        let mut parts = token.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        return Some((major, minor, patch));
    }
    None
}

fn resolve_cli_bin(path: &Path) -> Result<PathBuf, String> {
    if path.components().count() > 1 || path.is_absolute() {
        return path
            .try_exists()
            .map_err(|error| {
                format!(
                    "reason=path_probe_failed,error={}",
                    sanitize_detail(&error.to_string())
                )
            })?
            .then(|| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
            .ok_or_else(|| format!("reason=binary_not_found,path={}", path.display()));
    }

    let Some(paths) = std::env::var_os("PATH") else {
        return Err(format!("reason=binary_not_found,path={}", path.display()));
    };

    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(path);
        if candidate.try_exists().unwrap_or(false) {
            return Ok(candidate.canonicalize().unwrap_or(candidate));
        }
    }

    Err(format!("reason=binary_not_found,path={}", path.display()))
}

fn probe_codex_version(cli_bin: &Path) -> Result<String, AdapterError> {
    let output = Command::new(cli_bin)
        .arg("--version")
        .output()
        .map_err(|error| {
            AdapterError::failed(
                "codex_version",
                sanitize_detail(&error.to_string()),
                RetryClass::NonRetryable,
            )
        })?;

    if !output.status.success() {
        return Err(AdapterError::failed(
            "codex_version",
            format!("exit_status={}", output.status),
            RetryClass::NonRetryable,
        ));
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if raw.is_empty() {
        return Err(AdapterError::failed(
            "codex_version",
            "empty_version_output".to_owned(),
            RetryClass::NonRetryable,
        ));
    }

    Ok(sanitize_detail(&raw))
}

fn sanitize_detail(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | ',' | ':' | '/' | '_' | '-' | '=' => ch,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        CodexCliCompatibility, CodexRuntimeProvider, can_reuse_session, compatibility_for_version,
        parse_semver_triplet,
    };
    use crate::contracts::{ProviderAdapter, ProviderRequest};
    use crate::test_util::block_on;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn fake_cli(label: &str, stdout: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "axonrunner-codex-fake-{label}-{}-{tick}.sh",
            std::process::id()
        ));
        fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", stdout))
            .expect("fake cli should be written");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&path)
                .expect("metadata should exist")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).expect("permissions should be updated");
        }
        path
    }

    #[tokio::test]
    async fn codex_runtime_provider_rejects_empty_prompt() {
        let provider = CodexRuntimeProvider::new("codek");
        let err = provider
            .complete(ProviderRequest::new("gpt-5-codex", "", 100, "/tmp"))
            .await
            .expect_err("empty prompt should fail");
        assert!(matches!(
            err,
            crate::error::AdapterError::InvalidInput {
                field: "prompt",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn codex_runtime_provider_rejects_zero_max_tokens() {
        let provider = CodexRuntimeProvider::new("codek");
        let err = provider
            .complete(ProviderRequest::new("gpt-5-codex", "hello", 0, "/tmp"))
            .await
            .expect_err("zero max_tokens should fail");
        assert!(matches!(
            err,
            crate::error::AdapterError::InvalidInput {
                field: "max_tokens",
                ..
            }
        ));
    }

    #[test]
    fn codex_runtime_provider_uses_explicit_cli_bin_override() {
        let provider = CodexRuntimeProvider::new_with_cli_bin("codek", "/tmp/custom-codex");
        assert_eq!(provider.cli_bin, PathBuf::from("/tmp/custom-codex"));
    }

    #[test]
    fn codex_runtime_provider_health_is_blocked_when_binary_is_missing() {
        let provider =
            CodexRuntimeProvider::new_with_cli_bin("codek", "/definitely-missing-codex-binary");
        let report = block_on(provider.health()).expect("health probe should complete");
        assert_eq!(report.status.as_str(), "blocked");
        assert!(report.detail.contains("reason=binary_not_found"));
    }

    #[test]
    fn codex_runtime_provider_health_reports_blocked_old_version() {
        let cli = fake_cli("old-version", "codex 0.103.9");
        let provider = CodexRuntimeProvider::new_with_cli_bin("codek", &cli);

        let report = block_on(provider.health()).expect("health probe should complete");

        assert_eq!(report.status.as_str(), "blocked");
        assert!(
            report.detail.contains("version=codex_0.103.9")
                || report.detail.contains("version=codex 0.103.9")
        );
        assert!(report.detail.contains("compatibility=blocked"));

        let _ = fs::remove_file(cli);
    }

    #[test]
    fn codex_runtime_provider_health_reports_unknown_version_as_degraded() {
        let cli = fake_cli("unknown-version", "codex dev-build");
        let provider = CodexRuntimeProvider::new_with_cli_bin("codek", &cli);

        let report = block_on(provider.health()).expect("health probe should complete");

        assert_eq!(report.status.as_str(), "degraded");
        assert!(report.detail.contains("compatibility=unknown"));

        let _ = fs::remove_file(cli);
    }

    #[test]
    fn codex_runtime_version_parser_extracts_semver_triplet() {
        assert_eq!(parse_semver_triplet("codex 0.104.1"), Some((0, 104, 1)));
        assert_eq!(
            parse_semver_triplet("codex-cli version 0.99.0"),
            Some((0, 99, 0))
        );
        assert_eq!(parse_semver_triplet("unknown"), None);
    }

    #[test]
    fn codex_runtime_compatibility_uses_minimum_supported_version() {
        assert_eq!(
            compatibility_for_version("codex 0.104.0"),
            CodexCliCompatibility::Supported
        );
        assert_eq!(
            compatibility_for_version("codex 0.105.2"),
            CodexCliCompatibility::Supported
        );
        assert_eq!(
            compatibility_for_version("codex 0.103.9"),
            CodexCliCompatibility::Blocked
        );
        assert_eq!(
            compatibility_for_version("codex unknown"),
            CodexCliCompatibility::Unknown
        );
    }

    #[test]
    fn codex_runtime_session_reuse_requires_matching_cwd_and_model() {
        let request = ProviderRequest::new("gpt-5-codex", "hello", 100, "/tmp/work");

        assert!(can_reuse_session("/tmp/work", "gpt-5-codex", &request));
        assert!(!can_reuse_session("/tmp/other", "gpt-5-codex", &request));
        assert!(!can_reuse_session("/tmp/work", "gpt-4.1", &request));
    }
}
