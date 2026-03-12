use std::path::{Path, PathBuf};

const NEW_ENV_PREFIX: &str = "AXONRUNNER_";
const LEGACY_ENV_PREFIX: &str = "AXIOM_";
const NEW_STATE_DIR: &str = ".axonrunner";
const LEGACY_STATE_DIR: &str = ".axiom";

pub fn read_env_trimmed(env_var: &str) -> Result<Option<String>, String> {
    for candidate in env_candidates(env_var) {
        match std::env::var(candidate.as_str()) {
            Ok(value) => {
                let value = value.trim().to_owned();
                if value.is_empty() {
                    return Err(format!("environment '{}' must not be empty", candidate));
                }
                return Ok(Some(value));
            }
            Err(std::env::VarError::NotPresent) => continue,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(format!("environment '{}' is not valid unicode", candidate));
            }
        }
    }
    Ok(None)
}

pub fn resolve_env_path(env_var: &'static str, default_relative: &Path) -> Result<PathBuf, String> {
    if let Some(path) = read_env_trimmed(env_var)? {
        return Ok(PathBuf::from(path));
    }

    let cwd = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    let primary = cwd.join(default_relative);
    Ok(prefer_existing_path(primary))
}

pub fn prefer_existing_path(primary: PathBuf) -> PathBuf {
    if primary.exists() {
        return primary;
    }

    if let Some(legacy) = legacy_path_for(primary.as_path())
        && legacy.exists()
    {
        return legacy;
    }

    primary
}

fn env_candidates(env_var: &str) -> Vec<String> {
    let mut keys = vec![env_var.to_owned()];
    if let Some(suffix) = env_var.strip_prefix(NEW_ENV_PREFIX) {
        keys.push(format!("{LEGACY_ENV_PREFIX}{suffix}"));
    }
    keys
}

fn legacy_path_for(path: &Path) -> Option<PathBuf> {
    let mut replaced = false;
    let mut legacy = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::Normal(value) if value == NEW_STATE_DIR => {
                legacy.push(LEGACY_STATE_DIR);
                replaced = true;
            }
            _ => legacy.push(component.as_os_str()),
        }
    }

    replaced.then_some(legacy)
}

#[cfg(test)]
mod tests {
    use super::{legacy_path_for, prefer_existing_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn legacy_path_for_rewrites_state_root() {
        let primary = Path::new("/tmp/example/.axonrunner/channel/store.db");
        let legacy = legacy_path_for(primary).expect("legacy path");
        assert_eq!(
            legacy,
            PathBuf::from("/tmp/example/.axiom/channel/store.db")
        );
    }

    #[test]
    fn prefer_existing_path_keeps_primary_when_legacy_missing() {
        let unique = format!(
            "axonrunner-env-util-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let primary = root.join(".axonrunner").join("memory.db");
        let preferred = prefer_existing_path(primary.clone());
        assert_eq!(preferred, primary);
    }
}
