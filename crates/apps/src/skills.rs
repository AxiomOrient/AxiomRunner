use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env_util::resolve_env_path;
use crate::skills_registry::{DEFAULT_REGISTRY_URL, find_in_registry};

const ENV_SKILLS_DIR: &str = "AXIOM_SKILLS_DIR";
const DEFAULT_SKILLS_DIR: &str = ".axiom/skills";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsAction {
    List,
    Install { source: String },
    Remove { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillInstallMode {
    Cloned,
    Linked,
    Copied,
}

impl SkillInstallMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SkillInstallMode::Cloned => "cloned",
            SkillInstallMode::Linked => "linked",
            SkillInstallMode::Copied => "copied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsResult {
    Listed {
        path: PathBuf,
        skills: Vec<SkillEntry>,
    },
    Installed {
        path: PathBuf,
        name: String,
        source: String,
        mode: SkillInstallMode,
    },
    Removed {
        path: PathBuf,
        name: String,
        removed: bool,
    },
}

pub fn execute_skills_action(action: SkillsAction) -> Result<SkillsResult, String> {
    let dir = resolve_env_path(ENV_SKILLS_DIR, Path::new(DEFAULT_SKILLS_DIR))?;
    execute_skills_action_at(action, &dir)
}

fn execute_skills_action_at(
    action: SkillsAction,
    skills_dir: &Path,
) -> Result<SkillsResult, String> {
    match action {
        SkillsAction::List => {
            let skills = load_skills(skills_dir)?;
            Ok(SkillsResult::Listed {
                path: skills_dir.to_path_buf(),
                skills,
            })
        }
        SkillsAction::Install { source } => install_skill(skills_dir, &source),
        SkillsAction::Remove { name } => remove_skill(skills_dir, &name),
    }
}

fn install_skill(skills_dir: &Path, source: &str) -> Result<SkillsResult, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err(String::from("skill source must not be empty"));
    }

    fs::create_dir_all(skills_dir).map_err(|error| {
        format!(
            "failed to create skills directory '{}': {error}",
            skills_dir.display()
        )
    })?;

    if source.starts_with("https://") || source.starts_with("http://") {
        let name = repo_name_from_source(source)?;
        validate_skill_name(&name)?;
        let destination = skills_dir.join(&name);
        if destination.exists() {
            return Err(format!("skill '{name}' already exists"));
        }

        let output = Command::new("git")
            .args(["clone", "--depth", "1", source])
            .arg(&destination)
            .output()
            .map_err(|error| format!("failed to execute git clone: {error}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!("git clone failed: {stderr}"));
        }

        return Ok(SkillsResult::Installed {
            path: skills_dir.to_path_buf(),
            name,
            source: source.to_string(),
            mode: SkillInstallMode::Cloned,
        });
    }

    // Registry name lookup: if source looks like a simple name (no path separators, no protocol)
    if !source.contains('/') && !source.contains('\\') && !source.contains('.') {
        let entry = find_in_registry(DEFAULT_REGISTRY_URL, source)
            .map_err(|e| format!("registry lookup failed: {e}"))?;
        let entry = entry.ok_or_else(|| format!("skill '{source}' not found in registry"))?;
        let repo_url = entry.repo_url.clone();
        return install_skill(skills_dir, &repo_url);
    }

    let source_path = PathBuf::from(source);
    if !source_path.exists() {
        return Err(format!("skill source does not exist: {source}"));
    }
    if !source_path.is_dir() {
        return Err(format!("skill source is not a directory: {source}"));
    }

    let name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| String::from("failed to derive skill name from source path"))?
        .to_string();
    validate_skill_name(&name)?;

    let destination = skills_dir.join(&name);
    if destination.exists() {
        return Err(format!("skill '{name}' already exists"));
    }

    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::symlink;
        if symlink(&source_path, &destination).is_ok() {
            SkillInstallMode::Linked
        } else {
            copy_dir_recursive(&source_path, &destination)?;
            SkillInstallMode::Copied
        }
    };

    #[cfg(not(unix))]
    let mode = {
        copy_dir_recursive(&source_path, &destination)?;
        SkillInstallMode::Copied
    };

    Ok(SkillsResult::Installed {
        path: skills_dir.to_path_buf(),
        name,
        source: source.to_string(),
        mode,
    })
}

fn remove_skill(skills_dir: &Path, name: &str) -> Result<SkillsResult, String> {
    validate_skill_name(name)?;
    let path = skills_dir.join(name);

    if !path.exists() {
        return Err(format!("skill '{name}' not found"));
    }

    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("failed to inspect skill '{}': {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        fs::remove_file(&path).map_err(|error| {
            format!("failed to remove skill link '{}': {error}", path.display())
        })?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(&path).map_err(|error| {
            format!(
                "failed to remove skill directory '{}': {error}",
                path.display()
            )
        })?;
    } else {
        fs::remove_file(&path).map_err(|error| {
            format!("failed to remove skill file '{}': {error}", path.display())
        })?;
    }

    Ok(SkillsResult::Removed {
        path: skills_dir.to_path_buf(),
        name: name.to_string(),
        removed: true,
    })
}

fn load_skills(skills_dir: &Path) -> Result<Vec<SkillEntry>, String> {
    if !skills_dir.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    let entries = fs::read_dir(skills_dir).map_err(|error| {
        format!(
            "failed to read skills directory '{}': {error}",
            skills_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read skill entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to read skill entry type: {error}"))?;
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        validate_skill_name(&name)?;
        let path = entry.path();
        let description = read_skill_description(&path)?;
        let source = if file_type.is_symlink() {
            fs::read_link(&path)
                .map(|value| value.display().to_string())
                .unwrap_or_else(|_| path.display().to_string())
        } else {
            path.display().to_string()
        };

        skills.push(SkillEntry {
            name,
            description,
            source,
        });
    }

    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(skills)
}

fn read_skill_description(path: &Path) -> Result<String, String> {
    let markdown = path.join("SKILL.md");
    if markdown.exists() {
        let contents = fs::read_to_string(&markdown).map_err(|error| {
            format!(
                "failed to read skill markdown '{}': {error}",
                markdown.display()
            )
        })?;

        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let cleaned = trimmed
                .trim_start_matches('#')
                .trim_start_matches('*')
                .trim();
            if !cleaned.is_empty() {
                return Ok(cleaned.to_string());
            }
        }
    }

    let manifest = path.join("SKILL.toml");
    if manifest.exists() {
        let contents = fs::read_to_string(&manifest).map_err(|error| {
            format!(
                "failed to read skill manifest '{}': {error}",
                manifest.display()
            )
        })?;

        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(raw) = trimmed.strip_prefix("description")
                && let Some((_, value)) = raw.split_once('=')
            {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Ok(value.to_string());
                }
            }
        }
    }

    Ok(String::from("no description"))
}

fn validate_skill_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(String::from("invalid skill name: empty"));
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(format!("invalid skill name: {name}"));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(format!("invalid skill name: {name} (allowed: a-zA-Z0-9-_)"));
    }
    Ok(())
}

fn repo_name_from_source(source: &str) -> Result<String, String> {
    let trimmed = source.trim().trim_end_matches('/');
    let tail = trimmed
        .rsplit('/')
        .next()
        .ok_or_else(|| format!("invalid skill source '{source}'"))?;
    let name = tail.trim_end_matches(".git").trim();
    if name.is_empty() {
        return Err(format!("invalid skill source '{source}'"));
    }
    Ok(name.to_string())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "failed to create destination directory '{}': {error}",
            destination.display()
        )
    })?;

    let entries = fs::read_dir(source).map_err(|error| {
        format!(
            "failed to read source directory '{}': {error}",
            source.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read source entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect source entry type: {error}"))?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if file_type.is_symlink() {
            let link_target = fs::read_link(&source_path).map_err(|error| {
                format!(
                    "failed to read source symlink '{}': {error}",
                    source_path.display()
                )
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                symlink(link_target, &destination_path).map_err(|error| {
                    format!(
                        "failed to copy symlink '{}' -> '{}': {error}",
                        source_path.display(),
                        destination_path.display()
                    )
                })?;
            }
            #[cfg(not(unix))]
            {
                let _ = link_target;
                fs::copy(&source_path, &destination_path).map_err(|error| {
                    format!(
                        "failed to copy file '{}' -> '{}': {error}",
                        source_path.display(),
                        destination_path.display()
                    )
                })?;
            }
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "failed to copy file '{}' -> '{}': {error}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        SkillInstallMode, SkillsAction, SkillsResult, execute_skills_action_at,
        repo_name_from_source,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_dir(label: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-skills-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    #[test]
    fn skills_list_empty_when_missing_directory() {
        let dir = unique_dir("list-empty");
        let result = execute_skills_action_at(SkillsAction::List, &dir).expect("list should work");
        match result {
            SkillsResult::Listed { skills, .. } => assert!(skills.is_empty()),
            _ => panic!("expected listed result"),
        }
    }

    #[test]
    fn skills_install_list_remove_local_path() {
        let skills_dir = unique_dir("workspace");
        let source_root = unique_dir("source");
        fs::create_dir_all(&source_root).expect("source root should exist");
        let source_skill = source_root.join("demo_skill");
        fs::create_dir_all(&source_skill).expect("source skill should exist");
        fs::write(source_skill.join("SKILL.md"), "# Demo skill\n\nDo work\n")
            .expect("skill markdown should be writable");

        let install = execute_skills_action_at(
            SkillsAction::Install {
                source: source_skill.display().to_string(),
            },
            &skills_dir,
        )
        .expect("install should succeed");
        match install {
            SkillsResult::Installed { name, mode, .. } => {
                assert_eq!(name, "demo_skill");
                assert!(
                    mode == SkillInstallMode::Linked || mode == SkillInstallMode::Copied,
                    "unexpected mode={mode:?}"
                );
            }
            _ => panic!("expected installed result"),
        }

        let list = execute_skills_action_at(SkillsAction::List, &skills_dir)
            .expect("list should succeed after install");
        match list {
            SkillsResult::Listed { skills, .. } => {
                assert_eq!(skills.len(), 1);
                assert_eq!(skills[0].name, "demo_skill");
                assert_eq!(skills[0].description, "Demo skill");
            }
            _ => panic!("expected listed result"),
        }

        let remove = execute_skills_action_at(
            SkillsAction::Remove {
                name: String::from("demo_skill"),
            },
            &skills_dir,
        )
        .expect("remove should succeed");
        match remove {
            SkillsResult::Removed { removed, .. } => assert!(removed),
            _ => panic!("expected removed result"),
        }

        let _ = fs::remove_dir_all(skills_dir);
        let _ = fs::remove_dir_all(source_root);
    }

    #[test]
    fn skills_remove_rejects_path_traversal() {
        let dir = unique_dir("remove-reject");
        let error = execute_skills_action_at(
            SkillsAction::Remove {
                name: String::from("../escape"),
            },
            &dir,
        )
        .expect_err("path traversal must fail");
        assert!(error.contains("invalid skill name"), "error={error}");
    }

    #[test]
    fn skills_install_rejects_missing_source() {
        let dir = unique_dir("missing-source");
        let error = execute_skills_action_at(
            SkillsAction::Install {
                source: String::from("/tmp/axiom-skills-this-path-should-not-exist"),
            },
            &dir,
        )
        .expect_err("missing source should fail");
        assert!(error.contains("does not exist"), "error={error}");
    }

    #[test]
    fn repo_name_from_source_trims_git_suffix() {
        let name = repo_name_from_source("https://example.com/my-skill.git")
            .expect("repo name should parse");
        assert_eq!(name, "my-skill");
    }
    #[test]
    fn install_skill_rejects_unknown_registry_name() {
        let dir = std::env::temp_dir().join("axiom_test_skills_registry");
        let result = execute_skills_action_at(
            SkillsAction::Install {
                source: String::from("nonexistent-skill-xyz123"),
            },
            &dir,
        );
        assert!(result.is_err(), "unknown skill name should return error");
    }
}
