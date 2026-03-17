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
        if self
            .allowed_tools
            .iter()
            .any(|tool| tool.operation.trim().is_empty() || tool.scope.trim().is_empty())
        {
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
