pub fn read_env_trimmed(env_var: &str) -> Result<Option<String>, String> {
    match std::env::var(env_var) {
        Ok(value) => {
            let value = value.trim().to_owned();
            if value.is_empty() {
                return Err(format!("environment '{}' must not be empty", env_var));
            }
            Ok(Some(value))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("environment '{}' is not valid unicode", env_var))
        }
    }
}
