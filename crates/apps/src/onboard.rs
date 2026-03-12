use crate::env_util::resolve_env_path;
use crate::time_util::unix_now_seconds;
use std::fs;
use std::path::{Path, PathBuf};

const ENV_ONBOARD_STATE_PATH: &str = "AXONRUNNER_ONBOARD_STATE_PATH";
const ENV_ONBOARD_WORKSPACE_PATH: &str = "AXONRUNNER_ONBOARD_WORKSPACE_PATH";
const DEFAULT_ONBOARD_STATE_PATH: &str = ".axonrunner/onboard/state.db";
const DEFAULT_ONBOARD_WORKSPACE_PATH: &str = ".axonrunner/workspace";
const ONBOARD_STATE_FORMAT: &str = "format=axonrunner-onboard-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardMemory {
    Sqlite,
    Markdown,
    None,
}

impl OnboardMemory {
    pub fn as_str(self) -> &'static str {
        match self {
            OnboardMemory::Sqlite => "sqlite",
            OnboardMemory::Markdown => "markdown",
            OnboardMemory::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardState {
    pub profile: String,
    pub provider: String,
    pub memory: OnboardMemory,
    pub api_key_set: bool,
    pub interactive: bool,
    pub channels_only: bool,
    pub workspace_profile_path: PathBuf,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardAction {
    Configure {
        profile: String,
        interactive: bool,
        channels_only: bool,
        api_key: Option<String>,
        provider: Option<String>,
        memory: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardResult {
    Configured {
        state_path: PathBuf,
        workspace_path: PathBuf,
        state: OnboardState,
    },
}

pub fn execute_onboard_action(action: OnboardAction) -> Result<OnboardResult, String> {
    let state_path = resolve_env_path(
        ENV_ONBOARD_STATE_PATH,
        Path::new(DEFAULT_ONBOARD_STATE_PATH),
    )?;
    let workspace_path = resolve_env_path(
        ENV_ONBOARD_WORKSPACE_PATH,
        Path::new(DEFAULT_ONBOARD_WORKSPACE_PATH),
    )?;
    execute_onboard_action_at(action, &state_path, &workspace_path, unix_now_seconds())
}

fn execute_onboard_action_at(
    action: OnboardAction,
    state_path: &Path,
    workspace_path: &Path,
    now: u64,
) -> Result<OnboardResult, String> {
    match action {
        OnboardAction::Configure {
            profile,
            interactive,
            channels_only,
            api_key,
            provider,
            memory,
        } => {
            let profile = validate_profile(&profile)?;
            let input = OnboardInput {
                profile: profile.clone(),
                interactive,
                channels_only,
                api_key,
                provider,
                memory,
            };
            let state = apply_onboard_input(&input, workspace_path, now)?;
            write_onboard_artifacts(state_path, workspace_path, &state)?;
            Ok(OnboardResult::Configured {
                state_path: state_path.to_path_buf(),
                workspace_path: workspace_path.to_path_buf(),
                state,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OnboardInput {
    profile: String,
    interactive: bool,
    channels_only: bool,
    api_key: Option<String>,
    provider: Option<String>,
    memory: Option<String>,
}

fn apply_onboard_input(
    input: &OnboardInput,
    workspace_path: &Path,
    now: u64,
) -> Result<OnboardState, String> {
    if input.interactive && input.channels_only {
        return Err(String::from(
            "use either --interactive or --channels-only, not both",
        ));
    }

    if input.channels_only
        && (input.api_key.is_some() || input.provider.is_some() || input.memory.is_some())
    {
        return Err(String::from(
            "--channels-only does not accept --api-key, --provider, or --memory",
        ));
    }

    let provider = normalize_provider(input.provider.as_deref())?;
    let memory = normalize_memory(input.memory.as_deref())?;
    let workspace_profile_path = workspace_path.join("profiles").join(&input.profile);

    Ok(OnboardState {
        profile: input.profile.clone(),
        provider,
        memory,
        api_key_set: input
            .api_key
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        interactive: input.interactive,
        channels_only: input.channels_only,
        workspace_profile_path,
        updated_at: now,
    })
}

fn normalize_provider(provider: Option<&str>) -> Result<String, String> {
    let value = provider.unwrap_or("openrouter").trim();
    if value.is_empty() {
        return Err(String::from("provider must not be empty"));
    }
    if value.contains(char::is_whitespace) {
        return Err(format!("provider '{value}' must not contain whitespace"));
    }
    Ok(value.to_string())
}

fn normalize_memory(memory: Option<&str>) -> Result<OnboardMemory, String> {
    match memory.unwrap_or("sqlite").trim() {
        "sqlite" => Ok(OnboardMemory::Sqlite),
        "markdown" => Ok(OnboardMemory::Markdown),
        "none" => Ok(OnboardMemory::None),
        other => Err(format!(
            "unsupported memory backend '{other}': expected one of [sqlite, markdown, none]"
        )),
    }
}

fn validate_profile(profile: &str) -> Result<String, String> {
    let profile = profile.trim();
    if profile.is_empty() {
        return Err(String::from("profile must not be empty"));
    }
    if profile.contains('/') || profile.contains('\\') || profile.contains("..") {
        return Err(format!("invalid profile '{profile}'"));
    }
    Ok(profile.to_string())
}

fn write_onboard_artifacts(
    state_path: &Path,
    workspace_path: &Path,
    state: &OnboardState,
) -> Result<(), String> {
    fs::create_dir_all(workspace_path).map_err(|error| {
        format!(
            "failed to create workspace directory '{}': {error}",
            workspace_path.display()
        )
    })?;
    fs::create_dir_all(&state.workspace_profile_path).map_err(|error| {
        format!(
            "failed to create workspace profile directory '{}': {error}",
            state.workspace_profile_path.display()
        )
    })?;

    let profile_manifest = state.workspace_profile_path.join("profile.db");
    let profile_body = format!(
        "profile={}\nprovider={}\nmemory={}\nupdated_at={}\n",
        state.profile,
        state.provider,
        state.memory.as_str(),
        state.updated_at
    );
    fs::write(&profile_manifest, profile_body).map_err(|error| {
        format!(
            "failed to write workspace profile manifest '{}': {error}",
            profile_manifest.display()
        )
    })?;

    if let Some(parent) = state_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create onboard state directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    fs::write(state_path, render_state(state)).map_err(|error| {
        format!(
            "failed to write onboard state '{}': {error}",
            state_path.display()
        )
    })?;
    Ok(())
}

fn render_state(state: &OnboardState) -> String {
    format!(
        "{ONBOARD_STATE_FORMAT}\nprofile={}\nprovider={}\nmemory={}\napi_key_set={}\ninteractive={}\nchannels_only={}\nworkspace_profile_path={}\nupdated_at={}\n",
        state.profile,
        state.provider,
        state.memory.as_str(),
        state.api_key_set,
        state.interactive,
        state.channels_only,
        state.workspace_profile_path.display(),
        state.updated_at
    )
}

#[cfg(test)]
mod tests {
    use super::{OnboardAction, OnboardMemory, OnboardResult, execute_onboard_action_at};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axonrunner-onboard-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    fn unique_dir(label: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axonrunner-onboard-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    #[test]
    fn onboard_configure_writes_state_and_profile() {
        let state_path = unique_path("state", "db");
        let workspace = unique_dir("workspace");
        let result = execute_onboard_action_at(
            OnboardAction::Configure {
                profile: String::from("prod"),
                interactive: false,
                channels_only: false,
                api_key: Some(String::from("sk-test")),
                provider: Some(String::from("openai")),
                memory: Some(String::from("markdown")),
            },
            &state_path,
            &workspace,
            55,
        )
        .expect("onboard should succeed");

        match result {
            OnboardResult::Configured { state, .. } => {
                assert_eq!(state.profile, "prod");
                assert_eq!(state.provider, "openai");
                assert_eq!(state.memory, OnboardMemory::Markdown);
                assert!(state.api_key_set);
            }
        }

        assert!(state_path.exists(), "state file must exist");
        assert!(
            workspace
                .join("profiles")
                .join("prod")
                .join("profile.db")
                .exists(),
            "workspace profile manifest must exist"
        );

        let _ = fs::remove_file(state_path);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn onboard_rejects_interactive_and_channels_only_combination() {
        let state_path = unique_path("invalid-combo", "db");
        let workspace = unique_dir("invalid-combo-workspace");

        let error = execute_onboard_action_at(
            OnboardAction::Configure {
                profile: String::from("prod"),
                interactive: true,
                channels_only: true,
                api_key: None,
                provider: None,
                memory: None,
            },
            &state_path,
            &workspace,
            70,
        )
        .expect_err("invalid combination must fail");

        assert!(
            error.contains("use either --interactive or --channels-only"),
            "error={error}"
        );
    }

    #[test]
    fn onboard_rejects_channels_only_with_quick_options() {
        let state_path = unique_path("channels-only-invalid", "db");
        let workspace = unique_dir("channels-only-invalid-workspace");

        let error = execute_onboard_action_at(
            OnboardAction::Configure {
                profile: String::from("prod"),
                interactive: false,
                channels_only: true,
                api_key: None,
                provider: Some(String::from("openai")),
                memory: None,
            },
            &state_path,
            &workspace,
            71,
        )
        .expect_err("channels-only options combination must fail");

        assert!(
            error.contains("--channels-only does not accept"),
            "error={error}"
        );
    }

    #[test]
    fn onboard_rejects_unknown_memory_backend() {
        let state_path = unique_path("memory-invalid", "db");
        let workspace = unique_dir("memory-invalid-workspace");

        let error = execute_onboard_action_at(
            OnboardAction::Configure {
                profile: String::from("prod"),
                interactive: false,
                channels_only: false,
                api_key: None,
                provider: None,
                memory: Some(String::from("vector")),
            },
            &state_path,
            &workspace,
            72,
        )
        .expect_err("unsupported memory backend must fail");

        assert!(
            error.contains("unsupported memory backend"),
            "error={error}"
        );
    }
}
