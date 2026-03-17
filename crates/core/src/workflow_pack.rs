use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunCommandProfile {
    Generic,
    Build,
    Test,
    Lint,
}

impl RunCommandProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Build => "build",
            Self::Test => "test",
            Self::Lint => "lint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackContract {
    pub pack_id: String,
    pub version: String,
    pub entry_goal: String,
    pub recommended_verifier_flow: Vec<RunCommandProfile>,
    pub allowed_tools: Vec<WorkflowPackAllowedTool>,
    pub verifier_rules: Vec<WorkflowPackVerifierRule>,
    pub approval_mode: String,
}

impl WorkflowPackContract {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.pack_id.trim().is_empty() {
            return Err("pack_id");
        }
        if self.version.trim().is_empty() {
            return Err("version");
        }
        if self.entry_goal.trim().is_empty() {
            return Err("entry_goal");
        }
        if self.recommended_verifier_flow.is_empty() {
            return Err("recommended_verifier_flow");
        }
        if self.allowed_tools.is_empty() {
            return Err("allowed_tools");
        }
        if self.verifier_rules.is_empty() {
            return Err("verifier_rules");
        }
        if self.allowed_tools.iter().any(|tool| {
            tool.operation.trim().is_empty()
                || tool.scope.trim().is_empty()
                || !matches!(
                    tool.operation.trim(),
                    "list_files"
                        | "read_file"
                        | "search_files"
                        | "file_write"
                        | "replace_in_file"
                        | "remove_path"
                        | "run_command"
                )
                || tool.scope.trim() != "workspace"
        }) {
            return Err("allowed_tools.entry");
        }
        if self.verifier_rules.iter().any(|rule| {
            rule.label.trim().is_empty()
                || rule.command.program.trim().is_empty()
                || rule.artifact_expectation.trim().is_empty()
        }) {
            return Err("verifier_rules.entry");
        }
        if !matches!(self.approval_mode.trim(), "never" | "always") {
            return Err("approval_mode");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackAllowedTool {
    pub operation: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackVerifierRule {
    pub label: String,
    pub profile: RunCommandProfile,
    pub command: WorkflowPackVerifierCommand,
    pub artifact_expectation: String,
    #[serde(default)]
    pub strength: WorkflowPackVerifierStrength,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackVerifierCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackVerifierStrength {
    #[default]
    Strong,
    Weak,
    Unresolved,
    PackRequired,
}

impl WorkflowPackVerifierStrength {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Weak => "weak",
            Self::Unresolved => "unresolved",
            Self::PackRequired => "pack_required",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RunCommandProfile, WorkflowPackAllowedTool, WorkflowPackContract,
        WorkflowPackVerifierCommand, WorkflowPackVerifierRule,
    };

    fn base_contract() -> WorkflowPackContract {
        WorkflowPackContract {
            pack_id: String::from("rust-service-basic"),
            version: String::from("1"),
            entry_goal: String::from("goal"),
            recommended_verifier_flow: vec![RunCommandProfile::Test],
            allowed_tools: vec![WorkflowPackAllowedTool {
                operation: String::from("run_command"),
                scope: String::from("workspace"),
            }],
            verifier_rules: vec![WorkflowPackVerifierRule {
                label: String::from("cargo test"),
                profile: RunCommandProfile::Test,
                command: WorkflowPackVerifierCommand {
                    program: String::from("cargo"),
                    args: vec![String::from("test")],
                },
                artifact_expectation: String::from("test exits 0"),
                strength: Default::default(),
                required: true,
            }],
            approval_mode: String::from("never"),
        }
    }

    #[test]
    fn workflow_pack_validate_rejects_unknown_allowed_tool_operation() {
        let mut contract = base_contract();
        contract.allowed_tools[0].operation = String::from("browser");
        assert_eq!(contract.validate(), Err("allowed_tools.entry"));
    }

    #[test]
    fn workflow_pack_validate_rejects_non_workspace_scope() {
        let mut contract = base_contract();
        contract.allowed_tools[0].scope = String::from("global");
        assert_eq!(contract.validate(), Err("allowed_tools.entry"));
    }
}
