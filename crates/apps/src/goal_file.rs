use axiomrunner_adapters::WorkflowPackContract;
use axiomrunner_core::{
    DoneCondition, RunApprovalMode, RunBudget, RunConstraint, RunGoal, VerificationCheck,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct GoalFileInput {
    summary: String,
    workspace_root: String,
    #[serde(default)]
    constraints: Vec<ConstraintInput>,
    done_conditions: Vec<DoneConditionInput>,
    verification_checks: Vec<VerificationCheckInput>,
    budget: BudgetInput,
    approval_mode: String,
    #[serde(default)]
    workflow_pack: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConstraintInput {
    label: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
struct DoneConditionInput {
    label: String,
    evidence: String,
}

#[derive(Debug, Deserialize)]
struct VerificationCheckInput {
    label: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
struct BudgetInput {
    max_steps: u64,
    max_minutes: u64,
    max_tokens: u64,
}

pub fn parse_goal_file_template(
    path: &str,
) -> Result<crate::cli_command::GoalFileTemplate, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("read goal file '{path}' failed: {error}"))?;
    let input: GoalFileInput = serde_json::from_str(&raw)
        .map_err(|error| format!("parse goal file '{path}' failed: {error}"))?;

    let approval_mode = match input.approval_mode.trim().to_ascii_lowercase().as_str() {
        "never" => RunApprovalMode::Never,
        "on-risk" => RunApprovalMode::OnRisk,
        "always" => RunApprovalMode::Always,
        other => {
            return Err(format!(
                "invalid goal file '{path}': unsupported approval_mode '{other}'"
            ));
        }
    };

    let goal = RunGoal {
        summary: input.summary,
        workspace_root: input.workspace_root,
        constraints: input
            .constraints
            .into_iter()
            .map(|constraint| RunConstraint {
                label: constraint.label,
                detail: constraint.detail,
            })
            .collect(),
        done_conditions: input
            .done_conditions
            .into_iter()
            .map(|condition| DoneCondition {
                label: condition.label,
                evidence: condition.evidence,
            })
            .collect(),
        verification_checks: input
            .verification_checks
            .into_iter()
            .map(|check| VerificationCheck {
                label: check.label,
                detail: check.detail,
            })
            .collect(),
        budget: RunBudget::bounded(
            input.budget.max_steps,
            input.budget.max_minutes,
            input.budget.max_tokens,
        ),
        approval_mode,
    };

    goal.validate()
        .map_err(|error| format!("invalid goal file '{path}': {error:?}"))?;

    let workflow_pack = match input.workflow_pack {
        Some(pack_path) => Some(load_workflow_pack(path, &pack_path)?),
        None => None,
    };

    Ok(crate::cli_command::GoalFileTemplate {
        path: path.to_owned(),
        goal,
        workflow_pack,
    })
}

fn load_workflow_pack(
    goal_file_path: &str,
    pack_path: &str,
) -> Result<WorkflowPackContract, String> {
    let resolved = resolve_pack_path(goal_file_path, pack_path);
    let raw = fs::read_to_string(&resolved).map_err(|error| {
        format!(
            "read workflow pack '{}' for goal file '{}' failed: {error}",
            resolved.display(),
            goal_file_path
        )
    })?;
    let pack: WorkflowPackContract = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "parse workflow pack '{}' for goal file '{}' failed: {error}",
            resolved.display(),
            goal_file_path
        )
    })?;
    pack.validate().map_err(|field| {
        format!(
            "invalid workflow pack '{}' for goal file '{}': missing_or_invalid={field}",
            resolved.display(),
            goal_file_path
        )
    })?;
    Ok(pack)
}

fn resolve_pack_path(goal_file_path: &str, pack_path: &str) -> PathBuf {
    let candidate = PathBuf::from(pack_path);
    if candidate.is_absolute() {
        return candidate;
    }
    Path::new(goal_file_path)
        .parent()
        .map(|parent| parent.join(&candidate))
        .unwrap_or(candidate)
}

#[cfg(test)]
mod tests {
    use super::parse_goal_file_template;
    use std::fs;
    use std::path::PathBuf;

    fn unique_path(label: &str, ext: &str) -> PathBuf {
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiomrunner-goal-file-{label}-{}-{tick}.{ext}",
            std::process::id()
        ))
    }

    #[test]
    fn goal_file_loads_external_workflow_pack_manifest() {
        let goal_path = unique_path("with-pack-goal", "json");
        let pack_path = unique_path("with-pack-manifest", "json");
        fs::write(
            &pack_path,
            r#"{
  "pack_id": "rust-service-basic",
  "version": "1",
  "description": "pack",
  "entry_goal": "goal",
  "planner_hints": ["prefer bounded verification"],
  "recommended_verifier_flow": ["build", "test", "lint"],
  "allowed_tools": [{"operation": "run_command", "scope": "workspace"}],
  "verifier_rules": [{
    "label": "build",
    "profile": "build",
    "command_example": "cargo build",
    "artifact_expectation": "build passes",
    "required": true
  }],
  "risk_policy": {"approval_mode": "never", "max_mutating_steps": 8}
}"#,
        )
        .expect("pack manifest should be written");
        fs::write(
            &goal_path,
            format!(
                r#"{{
  "summary": "Goal with pack",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [{{ "label": "report", "evidence": "report exists" }}],
  "verification_checks": [{{ "label": "build", "detail": "cargo build" }}],
  "budget": {{ "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 }},
  "approval_mode": "never",
  "workflow_pack": "{}"
}}"#,
                pack_path.display()
            ),
        )
        .expect("goal file should be written");

        let parsed = parse_goal_file_template(goal_path.to_str().expect("utf8 path"))
            .expect("goal file should parse");
        assert_eq!(
            parsed
                .workflow_pack
                .as_ref()
                .map(|pack| pack.pack_id.as_str()),
            Some("rust-service-basic")
        );

        let _ = fs::remove_file(goal_path);
        let _ = fs::remove_file(pack_path);
    }

    #[test]
    fn goal_file_rejects_invalid_workflow_pack_manifest() {
        let goal_path = unique_path("bad-pack-goal", "json");
        let pack_path = unique_path("bad-pack-manifest", "json");
        fs::write(&pack_path, r#"{"pack_id": ""}"#).expect("pack manifest should be written");
        fs::write(
            &goal_path,
            format!(
                r#"{{
  "summary": "Goal with invalid pack",
  "workspace_root": "/workspace",
  "constraints": [],
  "done_conditions": [{{ "label": "report", "evidence": "report exists" }}],
  "verification_checks": [{{ "label": "build", "detail": "cargo build" }}],
  "budget": {{ "max_steps": 5, "max_minutes": 10, "max_tokens": 8000 }},
  "approval_mode": "never",
  "workflow_pack": "{}"
}}"#,
                pack_path.display()
            ),
        )
        .expect("goal file should be written");

        let error = parse_goal_file_template(goal_path.to_str().expect("utf8 path"))
            .expect_err("invalid pack should fail closed");
        assert!(
            error.contains("parse workflow pack") || error.contains("invalid workflow pack"),
            "error was: {error}"
        );

        let _ = fs::remove_file(goal_path);
        let _ = fs::remove_file(pack_path);
    }
}
