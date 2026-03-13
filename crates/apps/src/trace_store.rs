use crate::runtime_compose::{RuntimeComposeExecution, RuntimeComposePatchArtifact};
use axonrunner_core::{AgentState, DecisionOutcome};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const TRACE_INTENT_SCHEMA_V1: &str = "axonrunner.trace.intent.v1";

#[derive(Debug, Clone)]
pub struct TraceStore {
    storage: JsonlTraceStorage,
}

#[derive(Debug, Clone)]
struct JsonlTraceStorage {
    events_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFailureBoundary {
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceArtifacts {
    pub plan: String,
    pub apply: String,
    pub verify: String,
    pub report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceVerificationSummary {
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracePatchArtifact {
    pub operation: String,
    pub target_path: String,
    pub artifact_path: String,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub before_excerpt: Option<String>,
    pub after_excerpt: Option<String>,
    pub unified_diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceIntentEvent {
    pub schema: String,
    pub timestamp_ms: u64,
    pub actor_id: String,
    pub intent_id: String,
    pub kind: String,
    pub outcome: String,
    pub policy_code: String,
    pub effect_count: usize,
    pub revision: u64,
    pub mode: String,
    pub provider: String,
    pub memory: String,
    pub tool: String,
    pub tool_outputs: Vec<String>,
    pub first_failure: Option<TraceFailureBoundary>,
    #[serde(default = "default_trace_verification")]
    pub verification: TraceVerificationSummary,
    #[serde(default)]
    pub patch_artifacts: Vec<TracePatchArtifact>,
    pub artifacts: TraceArtifacts,
    pub report_written: bool,
    pub report_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySummary {
    pub intent_count: usize,
    pub latest_revision: u64,
    pub latest_mode: String,
    pub failed_intents: usize,
    pub latest_intent_id: Option<String>,
    pub latest_failure: Option<TraceFailureBoundary>,
}

impl TraceStore {
    pub fn from_workspace_root(workspace_root: Option<PathBuf>) -> Result<Self, String> {
        Ok(Self {
            storage: JsonlTraceStorage::from_workspace_root(workspace_root)?,
        })
    }

    pub fn append_intent_event(
        &self,
        actor_id: &str,
        intent_id: &str,
        kind: &str,
        outcome: DecisionOutcome,
        policy_code: &str,
        effect_count: usize,
        state: &AgentState,
        execution: &RuntimeComposeExecution,
        report_written: bool,
        report_error: Option<&str>,
        patch_artifacts: &[RuntimeComposePatchArtifact],
    ) -> Result<(), String> {
        let event = json!({
            "schema": TRACE_INTENT_SCHEMA_V1,
            "timestamp_ms": now_millis(),
            "actor_id": actor_id,
            "intent_id": intent_id,
            "kind": kind,
            "outcome": outcome_name(outcome),
            "policy_code": policy_code,
            "effect_count": effect_count,
            "revision": state.revision,
            "mode": mode_name(state.mode),
            "provider": step_name(&execution.provider),
            "memory": step_name(&execution.memory),
            "tool": step_name(&execution.tool),
            "tool_outputs": execution.tool_outputs,
            "first_failure": execution.first_failure().map(|(stage, message)| json!({
                "stage": stage,
                "message": message,
            })),
            "verification": verification_summary(execution, report_written, report_error),
            "patch_artifacts": patch_artifacts.iter().map(trace_patch_artifact).collect::<Vec<_>>(),
            "artifacts": {
                "plan": format!(".axonrunner/artifacts/{intent_id}.plan.md"),
                "apply": format!(".axonrunner/artifacts/{intent_id}.apply.md"),
                "verify": format!(".axonrunner/artifacts/{intent_id}.verify.md"),
                "report": format!(".axonrunner/artifacts/{intent_id}.report.md"),
            },
            "report_written": report_written,
            "report_error": report_error,
        });

        self.storage.append_event(&event)
    }

    pub fn load_events(&self) -> Result<Vec<TraceIntentEvent>, String> {
        self.storage.load_events()
    }

    pub fn replay_summary(&self) -> Result<ReplaySummary, String> {
        summarize_events(self.load_events()?)
    }

    pub fn replay_summary_for_intent(
        &self,
        intent_id: &str,
    ) -> Result<Option<ReplaySummary>, String> {
        let filtered = self
            .load_events()?
            .into_iter()
            .filter(|event| event.intent_id == intent_id)
            .collect::<Vec<_>>();
        if filtered.is_empty() {
            return Ok(None);
        }
        summarize_events(filtered).map(Some)
    }

    pub fn latest_event(&self) -> Result<Option<TraceIntentEvent>, String> {
        Ok(self.load_events()?.into_iter().last())
    }

    pub fn latest_event_for_intent(
        &self,
        intent_id: &str,
    ) -> Result<Option<TraceIntentEvent>, String> {
        Ok(self
            .load_events()?
            .into_iter()
            .filter(|event| event.intent_id == intent_id)
            .last())
    }

    pub fn events_path(&self) -> &PathBuf {
        self.storage.events_path()
    }
}

impl JsonlTraceStorage {
    fn from_workspace_root(workspace_root: Option<PathBuf>) -> Result<Self, String> {
        let root = workspace_root
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| String::from("trace workspace root is not available"))?;
        Ok(Self {
            events_path: root.join(".axonrunner").join("trace").join("events.jsonl"),
        })
    }

    fn append_event(&self, event: &serde_json::Value) -> Result<(), String> {
        if let Some(parent) = self.events_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "create trace directory '{}' failed: {error}",
                    parent.display()
                )
            })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .map_err(|error| format!("open trace event log failed: {error}"))?;
        serde_json::to_writer(&mut file, event)
            .map_err(|error| format!("serialize trace event failed: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("append trace newline failed: {error}"))?;
        Ok(())
    }

    fn load_events(&self) -> Result<Vec<TraceIntentEvent>, String> {
        let raw = match fs::read_to_string(&self.events_path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("read trace event log failed: {error}")),
        };

        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(index, line)| {
                serde_json::from_str::<TraceIntentEvent>(line)
                    .map_err(|error| format!("parse trace line {} failed: {error}", index + 1))
            })
            .collect()
    }

    fn events_path(&self) -> &PathBuf {
        &self.events_path
    }
}

fn now_millis() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0));
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}

fn step_name(step: &crate::runtime_compose::RuntimeComposeStep) -> &'static str {
    match step {
        crate::runtime_compose::RuntimeComposeStep::Skipped => "skipped",
        crate::runtime_compose::RuntimeComposeStep::Applied => "applied",
        crate::runtime_compose::RuntimeComposeStep::Failed(_) => "failed",
    }
}

fn mode_name(mode: axonrunner_core::ExecutionMode) -> &'static str {
    match mode {
        axonrunner_core::ExecutionMode::Active => "active",
        axonrunner_core::ExecutionMode::ReadOnly => "read_only",
        axonrunner_core::ExecutionMode::Halted => "halted",
    }
}

fn summarize_events(events: Vec<TraceIntentEvent>) -> Result<ReplaySummary, String> {
    let latest = events
        .last()
        .ok_or_else(|| String::from("trace summary requires at least one event"))?;
    let failed_intents = events
        .iter()
        .filter(|event| event.first_failure.is_some())
        .count();

    Ok(ReplaySummary {
        intent_count: events.len(),
        latest_revision: latest.revision,
        latest_mode: latest.mode.clone(),
        failed_intents,
        latest_intent_id: Some(latest.intent_id.clone()),
        latest_failure: latest.first_failure.clone(),
    })
}

fn default_trace_verification() -> TraceVerificationSummary {
    TraceVerificationSummary {
        status: String::from("unknown"),
        summary: String::from("legacy_trace_without_verification"),
    }
}

fn verification_summary(
    execution: &RuntimeComposeExecution,
    report_written: bool,
    report_error: Option<&str>,
) -> TraceVerificationSummary {
    if let Some((stage, message)) = execution.first_failure() {
        return TraceVerificationSummary {
            status: String::from("failed"),
            summary: format!("stage={stage},message={message}"),
        };
    }
    if let Some(message) = report_error {
        return TraceVerificationSummary {
            status: String::from("failed"),
            summary: format!("stage=report,message={message}"),
        };
    }
    if report_written {
        return TraceVerificationSummary {
            status: String::from("passed"),
            summary: String::from("report_written=true"),
        };
    }
    TraceVerificationSummary {
        status: String::from("failed"),
        summary: String::from("report_written=false"),
    }
}

fn trace_patch_artifact(artifact: &RuntimeComposePatchArtifact) -> TracePatchArtifact {
    TracePatchArtifact {
        operation: artifact.operation.clone(),
        target_path: artifact.target_path.clone(),
        artifact_path: artifact.artifact_path.clone(),
        before_digest: artifact.before_digest.clone(),
        after_digest: artifact.after_digest.clone(),
        before_excerpt: artifact.before_excerpt.clone(),
        after_excerpt: artifact.after_excerpt.clone(),
        unified_diff: artifact.unified_diff.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ReplaySummary, TRACE_INTENT_SCHEMA_V1, TraceArtifacts, TraceFailureBoundary,
        TraceIntentEvent, TracePatchArtifact, TraceStore, TraceVerificationSummary,
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
            "axonrunner-trace-store-{label}-{}-{tick}",
            std::process::id()
        ))
    }

    fn event(intent_id: &str, revision: u64, failed: bool) -> TraceIntentEvent {
        TraceIntentEvent {
            schema: String::from(TRACE_INTENT_SCHEMA_V1),
            timestamp_ms: revision,
            actor_id: String::from("system"),
            intent_id: intent_id.to_owned(),
            kind: String::from("write"),
            outcome: String::from("accepted"),
            policy_code: String::from("allowed"),
            effect_count: 1,
            revision,
            mode: String::from("active"),
            provider: if failed {
                String::from("failed")
            } else {
                String::from("applied")
            },
            memory: String::from("applied"),
            tool: String::from("applied"),
            tool_outputs: vec![String::from("log=runtime.log")],
            first_failure: failed.then(|| TraceFailureBoundary {
                stage: String::from("provider"),
                message: String::from("boom"),
            }),
            verification: TraceVerificationSummary {
                status: if failed {
                    String::from("failed")
                } else {
                    String::from("passed")
                },
                summary: if failed {
                    String::from("stage=provider,message=boom")
                } else {
                    String::from("report_written=true")
                },
            },
            patch_artifacts: vec![TracePatchArtifact {
                operation: String::from("overwrite"),
                target_path: String::from("runtime.log"),
                artifact_path: String::from(".axonrunner/patches/runtime-log.json"),
                before_digest: None,
                after_digest: Some(String::from("abcd")),
                before_excerpt: None,
                after_excerpt: Some(String::from("after")),
                unified_diff: None,
            }],
            artifacts: TraceArtifacts {
                plan: format!(".axonrunner/artifacts/{intent_id}.plan.md"),
                apply: format!(".axonrunner/artifacts/{intent_id}.apply.md"),
                verify: format!(".axonrunner/artifacts/{intent_id}.verify.md"),
                report: format!(".axonrunner/artifacts/{intent_id}.report.md"),
            },
            report_written: true,
            report_error: None,
        }
    }

    #[test]
    fn trace_store_loads_events_and_builds_summary() {
        let root = unique_dir("summary");
        fs::create_dir_all(&root).expect("workspace should exist");
        let store = TraceStore::from_workspace_root(Some(root.clone())).expect("store should init");
        let path = root.join(".axonrunner/trace/events.jsonl");
        fs::create_dir_all(path.parent().expect("trace parent should exist"))
            .expect("trace parent should be created");

        let lines = [event("cli-1", 1, false), event("cli-2", 2, true)]
            .into_iter()
            .map(|event| serde_json::to_string(&event).expect("event should serialize"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{lines}\n")).expect("trace log should be written");

        let events = store.load_events().expect("events should load");
        assert_eq!(events.len(), 2);

        let summary = store.replay_summary().expect("summary should build");
        assert_eq!(
            summary,
            ReplaySummary {
                intent_count: 2,
                latest_revision: 2,
                latest_mode: String::from("active"),
                failed_intents: 1,
                latest_intent_id: Some(String::from("cli-2")),
                latest_failure: Some(TraceFailureBoundary {
                    stage: String::from("provider"),
                    message: String::from("boom"),
                }),
            }
        );

        let intent_summary = store
            .replay_summary_for_intent("cli-1")
            .expect("intent summary should build")
            .expect("intent summary should exist");
        assert_eq!(intent_summary.intent_count, 1);
        assert_eq!(intent_summary.failed_intents, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn trace_store_loads_legacy_events_without_new_fields() {
        let root = unique_dir("legacy");
        fs::create_dir_all(&root).expect("workspace should exist");
        let store = TraceStore::from_workspace_root(Some(root.clone())).expect("store should init");
        let path = root.join(".axonrunner/trace/events.jsonl");
        fs::create_dir_all(path.parent().expect("trace parent should exist"))
            .expect("trace parent should be created");

        let legacy = serde_json::json!({
            "schema": TRACE_INTENT_SCHEMA_V1,
            "timestamp_ms": 1_u64,
            "actor_id": "system",
            "intent_id": "cli-legacy",
            "kind": "write",
            "outcome": "accepted",
            "policy_code": "allowed",
            "effect_count": 1,
            "revision": 1,
            "mode": "active",
            "provider": "applied",
            "memory": "applied",
            "tool": "applied",
            "tool_outputs": ["log=runtime.log"],
            "first_failure": serde_json::Value::Null,
            "artifacts": {
                "plan": ".axonrunner/artifacts/cli-legacy.plan.md",
                "apply": ".axonrunner/artifacts/cli-legacy.apply.md",
                "verify": ".axonrunner/artifacts/cli-legacy.verify.md",
                "report": ".axonrunner/artifacts/cli-legacy.report.md"
            },
            "report_written": true,
            "report_error": serde_json::Value::Null
        });
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::to_string(&legacy).expect("legacy trace should serialize")
            ),
        )
        .expect("trace log should be written");

        let events = store.load_events().expect("events should load");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].verification.status, "unknown");
        assert!(events[0].patch_artifacts.is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
