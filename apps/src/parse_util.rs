pub fn parse_non_empty(raw: &str, label: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    Ok(trimmed.to_string())
}

pub fn parse_bool(raw: &str, field: &str) -> Result<bool, String> {
    match raw.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("invalid {field} '{other}': expected true/false")),
    }
}

pub fn parse_number<T: std::str::FromStr>(raw: &str, label: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    raw.trim()
        .parse::<T>()
        .map_err(|error| format!("invalid {label} '{raw}': {error}"))
}

pub fn parse_tools_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
