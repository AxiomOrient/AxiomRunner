use std::path::{Path, PathBuf};

pub fn resolve_env_path(env_var: &'static str, default_relative: &Path) -> Result<PathBuf, String> {
    match std::env::var(env_var) {
        Ok(path) => {
            let path = path.trim().to_owned();
            if path.is_empty() {
                return Err(format!("environment '{env_var}' must not be empty"));
            }
            Ok(PathBuf::from(path))
        }
        Err(std::env::VarError::NotPresent) => {
            let cwd = std::env::current_dir()
                .map_err(|error| format!("failed to resolve current directory: {error}"))?;
            Ok(cwd.join(default_relative))
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("environment '{env_var}' is not valid unicode"))
        }
    }
}
