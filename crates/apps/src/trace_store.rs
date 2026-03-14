use crate::display::outcome_name;
use crate::runtime_compose::{
    RuntimeComposeExecution, RuntimeComposePatchArtifact, RuntimeRunRecord,
    RuntimeRunRollbackMetadata, RuntimeRunStepRecord, run_outcome_name, run_phase_name,
};
use axonrunner_core::{AgentState, DecisionOutcome};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const TRACE_INTENT_SCHEMA_V1: &str = "axonrunner.trace.intent.v1";
const MAX_ARTIFACT_INDEX_PATCH_PATHS: usize = 6;

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
pub struct TraceRepairSummary {
    pub attempted: bool,
    #[serde(default)]
    pub attempts: usize,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRunSummary {
    pub run_id: String,
    pub step_ids: Vec<String>,
    #[serde(default)]
    pub step_journal: Vec<TraceRunStepSummary>,
    pub provider_cwd: String,
    pub phase: String,
    pub outcome: String,
    pub reason: String,
    #[serde(default = "default_trace_approval_state")]
    pub approval_state: String,
    #[serde(default = "default_trace_verifier_state")]
    pub verifier_state: String,
    #[serde(default)]
    pub elapsed_ms: u64,
    pub plan_summary: String,
    pub planned_steps: usize,
    pub repair: TraceRepairSummary,
    #[serde(default)]
    pub rollback: Option<TraceRollbackSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRollbackSummary {
    pub metadata_path: String,
    pub restore_path: String,
    #[serde(default)]
    pub cleanup_path: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRunStepSummary {
    pub id: String,
    pub label: String,
    pub phase: String,
    pub status: String,
    pub evidence: String,
    #[serde(default)]
    pub failure: Option<String>,
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
    #[serde(default)]
    pub run: Option<TraceRunSummary>,
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
    pub false_success_intents: usize,
    pub false_done_intents: usize,
    pub latest_intent_id: Option<String>,
    pub latest_failure: Option<TraceFailureBoundary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceArtifactEntry {
    pub intent_id: String,
    pub plan: String,
    pub apply: String,
    pub verify: String,
    pub report: String,
    pub patch_artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceArtifactIndex {
    pub entries: Vec<TraceArtifactEntry>,
}

pub struct TraceEventInput<'a> {
    pub actor_id: &'a str,
    pub intent_id: &'a str,
    pub kind: &'a str,
    pub outcome: DecisionOutcome,
    pub policy_code: &'a str,
    pub effect_count: usize,
    pub state: &'a AgentState,
    pub execution: &'a RuntimeComposeExecution,
    pub report_written: bool,
    pub report_error: Option<&'a str>,
    pub patch_artifacts: &'a [RuntimeComposePatchArtifact],
    pub run: &'a RuntimeRunRecord,
}

impl TraceStore {
    pub fn from_workspace_root(workspace_root: Option<PathBuf>) -> Result<Self, String> {
        Ok(Self {
            storage: JsonlTraceStorage::from_workspace_root(workspace_root)?,
        })
    }

    pub fn append_intent_event(&self, input: TraceEventInput<'_>) -> Result<(), String> {
        let event = json!({
            "schema": TRACE_INTENT_SCHEMA_V1,
            "timestamp_ms": now_millis(),
            "actor_id": input.actor_id,
            "intent_id": input.intent_id,
            "kind": input.kind,
            "outcome": outcome_name(input.outcome),
            "policy_code": input.policy_code,
            "effect_count": input.effect_count,
            "revision": input.state.revision,
            "mode": mode_name(input.state.mode),
            "provider": step_name(&input.execution.provider),
            "memory": step_name(&input.execution.memory),
            "tool": step_name(&input.execution.tool),
            "tool_outputs": input.execution.tool_outputs,
            "first_failure": input.execution.first_failure().map(|(stage, message)| json!({
                "stage": stage,
                "message": message,
            })),
            "verification": verification_summary(input.execution, input.run, input.report_error),
            "patch_artifacts": input.patch_artifacts.iter().map(trace_patch_artifact).collect::<Vec<_>>(),
            "run": {
                "run_id": input.run.plan.run_id,
                "step_ids": input.run.plan.steps.iter().map(|step| step.id.clone()).collect::<Vec<_>>(),
                "step_journal": input.run
                    .step_journal
                    .iter()
                    .map(trace_run_step_summary)
                    .collect::<Vec<_>>(),
                "provider_cwd": input.execution.provider_cwd,
                "phase": run_phase_name(input.run.phase),
                "outcome": run_outcome_name(input.run.outcome),
                "reason": input.run.reason,
                "approval_state": trace_approval_state(input.run),
                "verifier_state": input.run.verification.status,
                "elapsed_ms": input.run.elapsed_ms,
                "plan_summary": input.run.plan.summary,
                "planned_steps": input.run.plan.planned_steps,
                "repair": {
                    "attempted": input.run.repair.attempted,
                    "attempts": input.run.repair.attempts,
                    "status": input.run.repair.status,
                    "summary": input.run.repair.summary,
                },
                "rollback": input.run.rollback.as_ref().map(trace_rollback_summary),
            },
            "artifacts": {
                "plan": format!(".axonrunner/artifacts/{}.plan.md", input.intent_id),
                "apply": format!(".axonrunner/artifacts/{}.apply.md", input.intent_id),
                "verify": format!(".axonrunner/artifacts/{}.verify.md", input.intent_id),
                "report": format!(".axonrunner/artifacts/{}.report.md", input.intent_id),
            },
            "report_written": input.report_written,
            "report_error": input.report_error,
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

    pub fn replay_summary_for_run(&self, run_id: &str) -> Result<Option<ReplaySummary>, String> {
        let filtered = self
            .load_events()?
            .into_iter()
            .filter(|event| event_has_run_id(event, run_id))
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
            .rfind(|event| event.intent_id == intent_id))
    }

    pub fn latest_event_for_run(&self, run_id: &str) -> Result<Option<TraceIntentEvent>, String> {
        Ok(self
            .load_events()?
            .into_iter()
            .rfind(|event| event_has_run_id(event, run_id)))
    }

    pub fn events_path(&self) -> &PathBuf {
        self.storage.events_path()
    }

    pub fn artifact_index(&self) -> Result<TraceArtifactIndex, String> {
        Ok(build_artifact_index(self.load_events()?))
    }

    pub fn artifact_index_for_intent(
        &self,
        intent_id: &str,
    ) -> Result<Option<TraceArtifactEntry>, String> {
        Ok(self
            .load_events()?
            .into_iter()
            .find(|event| event.intent_id == intent_id)
            .map(artifact_entry_from_event))
    }

    pub fn artifact_index_for_run(
        &self,
        run_id: &str,
    ) -> Result<Option<TraceArtifactEntry>, String> {
        Ok(self
            .load_events()?
            .into_iter()
            .find(|event| event_has_run_id(event, run_id))
            .map(artifact_entry_from_event))
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
        let lines = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        let last_index = lines.len().saturating_sub(1);
        let mut events = Vec::with_capacity(lines.len());

        for (index, line) in lines.into_iter().enumerate() {
            match serde_json::from_str::<TraceIntentEvent>(line) {
                Ok(event) => events.push(event),
                Err(_) if index == last_index && !raw.ends_with('\n') => break,
                Err(error) => {
                    return Err(format!("parse trace line {} failed: {error}", index + 1));
                }
            }
        }

        Ok(events)
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
    let false_success_intents = events
        .iter()
        .filter(|event| {
            event.outcome == "accepted"
                && event
                    .run
                    .as_ref()
                    .map(|run| run.outcome.as_str() != "success")
                    .unwrap_or(false)
        })
        .count();
    let false_done_intents = events
        .iter()
        .filter(|event| {
            event.outcome == "accepted"
                && event
                    .run
                    .as_ref()
                    .map(|run| run.outcome.as_str() != "success")
                    .unwrap_or(false)
                && event.verification.summary.starts_with("done_condition_")
        })
        .count();

    Ok(ReplaySummary {
        intent_count: events.len(),
        latest_revision: latest.revision,
        latest_mode: latest.mode.clone(),
        failed_intents,
        false_success_intents,
        false_done_intents,
        latest_intent_id: Some(latest.intent_id.clone()),
        latest_failure: latest.first_failure.clone(),
    })
}

fn event_has_run_id(event: &TraceIntentEvent, run_id: &str) -> bool {
    event
        .run
        .as_ref()
        .map(|run| run.run_id == run_id)
        .unwrap_or(false)
}

fn build_artifact_index(events: Vec<TraceIntentEvent>) -> TraceArtifactIndex {
    TraceArtifactIndex {
        entries: events.into_iter().map(artifact_entry_from_event).collect(),
    }
}

fn artifact_entry_from_event(event: TraceIntentEvent) -> TraceArtifactEntry {
    let patch_artifacts = event
        .patch_artifacts
        .into_iter()
        .map(|artifact| artifact.artifact_path)
        .collect::<Vec<_>>();
    TraceArtifactEntry {
        intent_id: event.intent_id,
        plan: event.artifacts.plan,
        apply: event.artifacts.apply,
        verify: event.artifacts.verify,
        report: event.artifacts.report,
        patch_artifacts: compact_artifact_paths(patch_artifacts),
    }
}

fn compact_artifact_paths(paths: Vec<String>) -> Vec<String> {
    if paths.len() <= MAX_ARTIFACT_INDEX_PATCH_PATHS {
        return paths;
    }
    let omitted = paths.len() - MAX_ARTIFACT_INDEX_PATCH_PATHS;
    let mut compacted = paths
        .into_iter()
        .take(MAX_ARTIFACT_INDEX_PATCH_PATHS)
        .collect::<Vec<_>>();
    compacted.push(format!("...+{omitted}_more"));
    compacted
}

fn default_trace_verification() -> TraceVerificationSummary {
    TraceVerificationSummary {
        status: String::from("unknown"),
        summary: String::from("legacy_trace_without_verification"),
    }
}

fn default_trace_approval_state() -> String {
    String::from("unknown")
}

fn default_trace_verifier_state() -> String {
    String::from("unknown")
}

fn verification_summary(
    execution: &RuntimeComposeExecution,
    run: &RuntimeRunRecord,
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
    TraceVerificationSummary {
        status: run.verification.status.to_owned(),
        summary: run.verification.summary.clone(),
    }
}

fn trace_approval_state(run: &RuntimeRunRecord) -> &'static str {
    match run.outcome {
        crate::runtime_compose::RuntimeRunOutcome::ApprovalRequired => "required",
        _ => "not_required",
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

fn trace_rollback_summary(rollback: &RuntimeRunRollbackMetadata) -> serde_json::Value {
    json!({
        "metadata_path": rollback.metadata_path,
        "restore_path": rollback.restore_path,
        "cleanup_path": rollback.cleanup_path,
        "reason": rollback.reason,
    })
}

fn trace_run_step_summary(step: &RuntimeRunStepRecord) -> TraceRunStepSummary {
    TraceRunStepSummary {
        id: step.id.clone(),
        label: step.label.clone(),
        phase: step.phase.clone(),
        status: step.status.clone(),
        evidence: step.evidence.clone(),
        failure: step.failure.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ReplaySummary, TRACE_INTENT_SCHEMA_V1, TraceArtifactIndex, TraceArtifacts,
        TraceFailureBoundary, TraceIntentEvent, TracePatchArtifact, TraceRepairSummary,
        TraceRunStepSummary, TraceRunSummary, TraceStore, TraceVerificationSummary,
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
            run: Some(TraceRunSummary {
                run_id: format!("run-for-{intent_id}"),
                step_ids: vec![
                    format!("run-for-{intent_id}/step-1-planning"),
                    format!("run-for-{intent_id}/step-2-executing"),
                    format!("run-for-{intent_id}/step-3-executing"),
                    format!("run-for-{intent_id}/step-4-verifying"),
                ],
                step_journal: vec![TraceRunStepSummary {
                    id: format!("run-for-{intent_id}/step-1-planning"),
                    label: String::from("prepare bounded write"),
                    phase: String::from("planning"),
                    status: String::from("completed"),
                    evidence: String::from("intent_id=cli-1"),
                    failure: None,
                }],
                provider_cwd: String::from("/tmp/workspace"),
                phase: if failed {
                    String::from("failed")
                } else {
                    String::from("completed")
                },
                outcome: if failed {
                    String::from("failed")
                } else {
                    String::from("success")
                },
                reason: if failed {
                    String::from("stage=provider,message=boom")
                } else {
                    String::from("verification_passed")
                },
                approval_state: String::from("not_required"),
                verifier_state: if failed {
                    String::from("failed")
                } else {
                    String::from("passed")
                },
                plan_summary: String::from(
                    "intent_id=cli-1 legacy_write key=alpha outcome=accepted",
                ),
                planned_steps: 4,
                repair: TraceRepairSummary {
                    attempted: false,
                    attempts: 0,
                    status: String::from("skipped"),
                    summary: String::from("verification_passed"),
                },
                elapsed_ms: 0,
                rollback: None,
            }),
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
        assert_eq!(
            events[0].run.as_ref().map(|run| run.run_id.as_str()),
            Some("run-for-cli-1")
        );
        assert_eq!(
            events[0].run.as_ref().map(|run| run.step_ids.len()),
            Some(4)
        );
        assert_eq!(
            events[0].run.as_ref().map(|run| run.provider_cwd.as_str()),
            Some("/tmp/workspace")
        );

        let summary = store.replay_summary().expect("summary should build");
        assert_eq!(
            summary,
            ReplaySummary {
                intent_count: 2,
                latest_revision: 2,
                latest_mode: String::from("active"),
                failed_intents: 1,
                false_success_intents: 1,
                false_done_intents: 0,
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
        assert_eq!(intent_summary.false_success_intents, 0);
        assert_eq!(intent_summary.false_done_intents, 0);

        let artifact_index = store.artifact_index().expect("artifact index should build");
        assert_eq!(
            artifact_index,
            TraceArtifactIndex {
                entries: vec![
                    super::artifact_entry_from_event(event("cli-1", 1, false)),
                    super::artifact_entry_from_event(event("cli-2", 2, true)),
                ],
            }
        );

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

    #[test]
    fn trace_store_ignores_partial_final_line_without_newline() {
        let root = unique_dir("partial-tail");
        fs::create_dir_all(&root).expect("workspace should exist");
        let store = TraceStore::from_workspace_root(Some(root.clone())).expect("store should init");
        let path = root.join(".axonrunner/trace/events.jsonl");
        fs::create_dir_all(path.parent().expect("trace parent should exist"))
            .expect("trace parent should be created");

        let valid = serde_json::to_string(&event("cli-1", 1, false)).expect("event should serialize");
        fs::write(&path, format!("{valid}\n{{\"schema\":\"axonrunner.trace.intent.v1\""))
            .expect("trace log should be written");

        let events = store.load_events().expect("events should load");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].intent_id, "cli-1");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn trace_store_rejects_committed_malformed_line() {
        let root = unique_dir("malformed-line");
        fs::create_dir_all(&root).expect("workspace should exist");
        let store = TraceStore::from_workspace_root(Some(root.clone())).expect("store should init");
        let path = root.join(".axonrunner/trace/events.jsonl");
        fs::create_dir_all(path.parent().expect("trace parent should exist"))
            .expect("trace parent should be created");

        let valid = serde_json::to_string(&event("cli-1", 1, false)).expect("event should serialize");
        fs::write(&path, format!("{valid}\n{{bad json}}\n")).expect("trace log should be written");

        let error = store.load_events().expect_err("malformed committed line should fail");
        assert!(error.contains("parse trace line 2 failed"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn trace_store_artifact_index_compacts_long_patch_lists() {
        let mut long_event = event("cli-long", 1, false);
        long_event.patch_artifacts = (0..10)
            .map(|index| TracePatchArtifact {
                operation: String::from("overwrite"),
                target_path: format!("file-{index}.txt"),
                artifact_path: format!(".axonrunner/patches/file-{index}.json"),
                before_digest: None,
                after_digest: Some(String::from("abcd")),
                before_excerpt: None,
                after_excerpt: Some(String::from("after")),
                unified_diff: None,
            })
            .collect();

        let entry = super::artifact_entry_from_event(long_event);

        assert_eq!(
            entry.patch_artifacts.len(),
            super::MAX_ARTIFACT_INDEX_PATCH_PATHS + 1
        );
        assert_eq!(
            entry.patch_artifacts.last().map(|value| value.as_str()),
            Some("...+4_more")
        );
    }

    #[test]
    fn trace_store_counts_false_done_runs_separately() {
        let false_done = TraceIntentEvent {
            schema: TRACE_INTENT_SCHEMA_V1.to_owned(),
            timestamp_ms: 1,
            actor_id: String::from("system"),
            intent_id: String::from("cli-false-done"),
            kind: String::from("goal"),
            outcome: String::from("accepted"),
            policy_code: String::from("allowed"),
            effect_count: 0,
            revision: 1,
            mode: String::from("active"),
            provider: String::from("skipped"),
            memory: String::from("skipped"),
            tool: String::from("applied"),
            tool_outputs: vec![String::from("verifier=workspace")],
            first_failure: None,
            verification: TraceVerificationSummary {
                status: String::from("failed"),
                summary: String::from("done_condition_missing_report_artifact:report"),
            },
            patch_artifacts: Vec::new(),
            run: Some(TraceRunSummary {
                run_id: String::from("run-false-done"),
                step_ids: vec![String::from("run-false-done/step-1-planning")],
                step_journal: Vec::new(),
                provider_cwd: String::from("/tmp/workspace"),
                phase: String::from("failed"),
                outcome: String::from("failed"),
                reason: String::from("done_condition_missing_report_artifact:report"),
                approval_state: String::from("not_required"),
                verifier_state: String::from("failed"),
                plan_summary: String::from("intent_id=cli-false-done goal"),
                planned_steps: 4,
                repair: TraceRepairSummary {
                    attempted: false,
                    attempts: 0,
                    status: String::from("skipped"),
                    summary: String::from("verification_passed"),
                },
                elapsed_ms: 0,
                rollback: None,
            }),
            artifacts: TraceArtifacts {
                plan: String::from(".axonrunner/artifacts/cli-false-done.plan.md"),
                apply: String::from(".axonrunner/artifacts/cli-false-done.apply.md"),
                verify: String::from(".axonrunner/artifacts/cli-false-done.verify.md"),
                report: String::from(".axonrunner/artifacts/cli-false-done.report.md"),
            },
            report_written: true,
            report_error: None,
        };

        let summary = super::summarize_events(vec![false_done]).expect("summary should build");

        assert_eq!(summary.failed_intents, 0);
        assert_eq!(summary.false_success_intents, 1);
        assert_eq!(summary.false_done_intents, 1);
    }
}
