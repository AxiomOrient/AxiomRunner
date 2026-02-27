pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/AxiomOrient/axiom-skills-registry/main/registry.json";
const REGISTRY_HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const REGISTRY_HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRegistryEntry {
    pub name: String,
    pub version: String,
    pub repo_url: String,
    pub description: String,
}

/// Parses a JSON string into registry entries.
/// Expected format: [{"name": "...", "version": "...", "repo_url": "...", "description": "..."}]
pub fn parse_registry_json(json: &str) -> Result<Vec<SkillRegistryEntry>, String> {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| format!("registry json parse error: {e}"))?;

    let mut entries = Vec::with_capacity(values.len());
    for (i, value) in values.iter().enumerate() {
        let name = value["name"]
            .as_str()
            .ok_or_else(|| format!("entry {i}: missing name"))?
            .to_string();
        let version = value["version"]
            .as_str()
            .ok_or_else(|| format!("entry {i}: missing version"))?
            .to_string();
        let repo_url = value["repo_url"]
            .as_str()
            .ok_or_else(|| format!("entry {i}: missing repo_url"))?
            .to_string();
        let description = value["description"].as_str().unwrap_or("").to_string();

        entries.push(SkillRegistryEntry {
            name,
            version,
            repo_url,
            description,
        });
    }
    Ok(entries)
}

/// Fetches the registry JSON from a URL and parses it.
/// Requires network access. Use `#[ignore]` tests for network calls.
pub fn fetch_registry(registry_url: &str) -> Result<Vec<SkillRegistryEntry>, String> {
    let http = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(
            REGISTRY_HTTP_CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            REGISTRY_HTTP_REQUEST_TIMEOUT_SECS,
        ))
        .build()
        .map_err(|e| format!("failed to initialize registry http client: {e}"))?;

    let response = http
        .get(registry_url)
        .send()
        .map_err(|e| format!("failed to fetch registry: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "registry fetch failed: status {}",
            response.status()
        ));
    }

    let json = response
        .text()
        .map_err(|e| format!("failed to read registry response: {e}"))?;

    parse_registry_json(&json)
}

/// Finds a skill entry by name in the fetched registry.
/// Name lookup is case-insensitive.
pub fn find_in_registry(
    registry_url: &str,
    name: &str,
) -> Result<Option<SkillRegistryEntry>, String> {
    let entries = fetch_registry(registry_url)?;
    Ok(entries
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"[
        {"name": "echo-skill", "version": "1.0.0", "repo_url": "https://github.com/AxiomOrient/axiom-skill-echo", "description": "Echo skill"},
        {"name": "timer-skill", "version": "0.2.1", "repo_url": "https://github.com/AxiomOrient/axiom-skill-timer", "description": "Timer skill"}
    ]"#;

    #[test]
    fn parse_registry_json_returns_all_entries() {
        let entries = parse_registry_json(SAMPLE_JSON).expect("valid json");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "echo-skill");
        assert_eq!(entries[0].version, "1.0.0");
        assert_eq!(
            entries[0].repo_url,
            "https://github.com/AxiomOrient/axiom-skill-echo"
        );
    }

    #[test]
    fn parse_registry_json_rejects_missing_fields() {
        let bad_json = r#"[{"name": "missing-version"}]"#;
        let err = parse_registry_json(bad_json).expect_err("should fail");
        assert!(err.contains("missing version"));
    }

    #[test]
    fn parse_registry_json_handles_empty_array() {
        let entries = parse_registry_json("[]").expect("empty array is valid");
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_registry_json_rejects_malformed_json() {
        let err = parse_registry_json("{not json}").expect_err("should fail");
        assert!(err.contains("parse error"));
    }

    /// Requires network. Run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn fetch_registry_live() {
        let entries = fetch_registry(DEFAULT_REGISTRY_URL);
        // Either succeeds with a list or fails gracefully
        match entries {
            Ok(list) => println!("fetched {} entries", list.len()),
            Err(e) => println!("registry not available: {e}"),
        }
    }
}
