use crate::config_loader::AppConfig;
use crate::display::mode_name;
use crate::env_util::read_env_trimmed;
use axiomrunner_core::{AgentState, DecisionOutcome, ExecutionMode, PolicyCode};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

pub const ENV_RUNTIME_STATE_PATH: &str = "AXIOMRUNNER_RUNTIME_STATE_PATH";

const FORMAT_VERSION: &str = "axiomrunner-state-v2";
const NONE_SENTINEL: &str = "-";
const PENDING_RUN_REQUIRED_FIELDS: [&str; 9] = [
    "run_id",
    "intent_id",
    "goal_file_path",
    "phase",
    "reason_code",
    "reason_detail",
    "approval_state",
    "verifier_state",
    "advisory_constraints",
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeStateSnapshot {
    pub state: AgentState,
    pub next_intent_seq: u64,
    pub next_run_seq: u64,
    pub pending_run: Option<PendingRunSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRunSnapshot {
    pub run_id: String,
    pub intent_id: String,
    pub goal_file_path: String,
    pub phase: String,
    pub reason: String,
    pub reason_code: String,
    pub reason_detail: String,
    pub approval_state: String,
    pub verifier_state: String,
    /// Comma-separated advisory constraint labels, or "none". Advisory
    /// constraints are goal-file constraints that do not match an enforced
    /// policy key (path_scope, destructive_commands, external_commands,
    /// approval_escalation). Populated at approval-wait time so operators can
    /// see which constraints are advisory before resuming.
    pub advisory_constraints: String,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn from_app_config(config: &AppConfig) -> Result<Self, String> {
        let path = read_env_trimmed(ENV_RUNTIME_STATE_PATH)
            .ok()
            .flatten()
            .map(PathBuf::from)
            .or_else(|| config.state_path.clone())
            .or_else(|| {
                std::env::var("HOME").ok().map(|home| {
                    PathBuf::from(home)
                        .join(".axiomrunner")
                        .join("state.snapshot")
                })
            })
            .ok_or_else(|| String::from("runtime state path is not available"))?;

        Ok(Self { path })
    }

    pub fn load_snapshot(&self) -> Result<RuntimeStateSnapshot, String> {
        match read_snapshot_file(&self.path) {
            Ok(snapshot) => Ok(snapshot),
            Err(primary_error) if primary_error.kind() == ErrorKind::NotFound => {
                match read_snapshot_file(&self.temp_path()) {
                    Ok(snapshot) => Ok(snapshot),
                    Err(temp_error) if temp_error.kind() == ErrorKind::NotFound => {
                        Ok(RuntimeStateSnapshot::default())
                    }
                    Err(temp_error) => Err(format!(
                        "read state snapshot '{}' fallback '{}' failed: {}",
                        self.path.display(),
                        self.temp_path().display(),
                        render_snapshot_error(&temp_error)
                    )),
                }
            }
            Err(primary_error) => Err(format!(
                "read state snapshot '{}' failed: {}",
                self.path.display(),
                render_snapshot_error(&primary_error)
            )),
        }
    }

    pub fn save_snapshot(&self, snapshot: &RuntimeStateSnapshot) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "create state snapshot directory '{}' failed: {error}",
                    parent.display()
                )
            })?;
        }

        let temp_path = self.path.with_extension("tmp");
        fs::write(&temp_path, serialize_snapshot(snapshot)).map_err(|error| {
            format!(
                "write state snapshot '{}' failed: {error}",
                temp_path.display()
            )
        })?;

        match fs::rename(&temp_path, &self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                fs::remove_file(&self.path).map_err(|remove_error| {
                    format!(
                        "replace state snapshot '{}' failed while removing previous file: {remove_error}",
                        self.path.display()
                    )
                })?;
                fs::rename(&temp_path, &self.path).map_err(|rename_error| {
                    format!(
                        "replace state snapshot '{}' failed: {rename_error}",
                        self.path.display()
                    )
                })
            }
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                Err(format!(
                    "move state snapshot '{}' into place failed: {error}",
                    self.path.display()
                ))
            }
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn temp_path(&self) -> PathBuf {
        self.path.with_extension("tmp")
    }
}

fn read_snapshot_file(path: &PathBuf) -> Result<RuntimeStateSnapshot, StateSnapshotError> {
    let raw = fs::read_to_string(path).map_err(|source| StateSnapshotError::Read {
        path: path.clone(),
        source,
    })?;
    parse_snapshot(&raw).map_err(|detail| StateSnapshotError::Parse {
        path: path.clone(),
        detail,
    })
}

#[derive(Debug)]
enum StateSnapshotError {
    Read { path: PathBuf, source: std::io::Error },
    Parse { path: PathBuf, detail: String },
}

impl StateSnapshotError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Read { source, .. } => source.kind(),
            Self::Parse { .. } => ErrorKind::InvalidData,
        }
    }
}

fn render_snapshot_error(error: &StateSnapshotError) -> String {
    match error {
        StateSnapshotError::Read { path, source } => {
            format!("read '{}' failed: {source}", path.display())
        }
        StateSnapshotError::Parse { path, detail } => {
            format!("parse '{}' failed: {detail}", path.display())
        }
    }
}

fn serialize_snapshot(snapshot: &RuntimeStateSnapshot) -> String {
    let mut lines = vec![
        format!("version={FORMAT_VERSION}"),
        format!("next_intent_seq={}", snapshot.next_intent_seq),
        format!("next_run_seq={}", snapshot.next_run_seq),
        format!("revision={}", snapshot.state.revision),
        format!("mode={}", mode_name(snapshot.state.mode)),
        format!(
            "last_intent_id={}",
            encode_optional(snapshot.state.last_intent_id.as_deref())
        ),
        format!(
            "last_actor_id={}",
            encode_optional(snapshot.state.last_actor_id.as_deref())
        ),
        format!(
            "last_decision={}",
            snapshot
                .state
                .last_decision
                .map(|d| d.as_str())
                .unwrap_or(NONE_SENTINEL)
        ),
        format!(
            "last_policy_code={}",
            snapshot
                .state
                .last_policy_code
                .map(|c| c.as_str())
                .unwrap_or(NONE_SENTINEL)
        ),
    ];

    if let Some(pending_run) = &snapshot.pending_run {
        lines.push(format!(
            "pending_run.run_id={}",
            hex_encode(pending_run.run_id.as_bytes())
        ));
        lines.push(format!(
            "pending_run.intent_id={}",
            hex_encode(pending_run.intent_id.as_bytes())
        ));
        lines.push(format!(
            "pending_run.goal_file_path={}",
            hex_encode(pending_run.goal_file_path.as_bytes())
        ));
        lines.push(format!(
            "pending_run.phase={}",
            hex_encode(pending_run.phase.as_bytes())
        ));
        lines.push(format!(
            "pending_run.reason={}",
            hex_encode(pending_run.reason.as_bytes())
        ));
        lines.push(format!(
            "pending_run.reason_code={}",
            hex_encode(pending_run.reason_code.as_bytes())
        ));
        lines.push(format!(
            "pending_run.reason_detail={}",
            hex_encode(pending_run.reason_detail.as_bytes())
        ));
        lines.push(format!(
            "pending_run.approval_state={}",
            hex_encode(pending_run.approval_state.as_bytes())
        ));
        lines.push(format!(
            "pending_run.verifier_state={}",
            hex_encode(pending_run.verifier_state.as_bytes())
        ));
        lines.push(format!(
            "pending_run.advisory_constraints={}",
            hex_encode(pending_run.advisory_constraints.as_bytes())
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

fn parse_snapshot(raw: &str) -> Result<RuntimeStateSnapshot, String> {
    let mut snapshot = RuntimeStateSnapshot::default();
    let mut saw_version = false;
    let mut pending_run = PendingRunSnapshot {
        run_id: String::new(),
        intent_id: String::new(),
        goal_file_path: String::new(),
        phase: String::new(),
        reason: String::new(),
        reason_code: String::new(),
        reason_detail: String::new(),
        approval_state: String::new(),
        verifier_state: String::new(),
        advisory_constraints: String::new(),
    };
    let mut saw_pending_run = false;

    for (index, raw_line) in raw.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid state snapshot line {}: '{}'", index + 1, raw_line))?;

        if let Some(pending_key) = key.strip_prefix("pending_run.") {
            let decoded = decode_hex_utf8(value).ok_or_else(|| {
                format!("invalid pending run field encoding on line {}", index + 1)
            })?;
            saw_pending_run = true;
            match pending_key {
                "run_id" => pending_run.run_id = decoded,
                "intent_id" => pending_run.intent_id = decoded,
                "goal_file_path" => pending_run.goal_file_path = decoded,
                "phase" => pending_run.phase = decoded,
                "reason" => pending_run.reason = decoded,
                "reason_code" => pending_run.reason_code = decoded,
                "reason_detail" => pending_run.reason_detail = decoded,
                "approval_state" => pending_run.approval_state = decoded,
                "verifier_state" => pending_run.verifier_state = decoded,
                "advisory_constraints" => pending_run.advisory_constraints = decoded,
                _ => {
                    return Err(format!(
                        "unknown pending run key '{}' on line {}",
                        pending_key,
                        index + 1
                    ));
                }
            }
            continue;
        }

        match key {
            "version" => {
                if value != FORMAT_VERSION {
                    return Err(format!(
                        "unsupported state snapshot version '{value}' on line {}; no legacy migration path is supported",
                        index + 1
                    ));
                }
                saw_version = true;
            }
            "next_intent_seq" => snapshot.next_intent_seq = parse_u64(value, key, index + 1)?,
            "next_run_seq" => snapshot.next_run_seq = parse_u64(value, key, index + 1)?,
            "revision" => snapshot.state.revision = parse_u64(value, key, index + 1)?,
            "mode" => snapshot.state.mode = parse_mode(value, index + 1)?,
            "last_intent_id" => {
                snapshot.state.last_intent_id = decode_optional_hex_utf8(value, index + 1)?
            }
            "last_actor_id" => {
                snapshot.state.last_actor_id = decode_optional_hex_utf8(value, index + 1)?
            }
            "last_decision" => snapshot.state.last_decision = parse_decision(value, index + 1)?,
            "last_policy_code" => {
                snapshot.state.last_policy_code = parse_policy_code(value, index + 1)?
            }
            _ => {
                return Err(format!(
                    "unknown state snapshot key '{}' on line {}",
                    key,
                    index + 1
                ));
            }
        }
    }

    if !saw_version {
        return Err(String::from("state snapshot is missing version header"));
    }

    if saw_pending_run {
        if let Some(field) = missing_pending_run_field(&pending_run) {
            return Err(format!(
                "pending run snapshot missing required field '{field}' on load"
            ));
        }
        snapshot.pending_run = Some(pending_run);
    }
    Ok(snapshot)
}

fn missing_pending_run_field(pending_run: &PendingRunSnapshot) -> Option<&'static str> {
    for field in PENDING_RUN_REQUIRED_FIELDS {
        let missing = match field {
            "run_id" => pending_run.run_id.is_empty(),
            "intent_id" => pending_run.intent_id.is_empty(),
            "goal_file_path" => pending_run.goal_file_path.is_empty(),
            "phase" => pending_run.phase.is_empty(),
            "reason_code" => pending_run.reason_code.is_empty(),
            "reason_detail" => pending_run.reason_detail.is_empty(),
            "approval_state" => pending_run.approval_state.is_empty(),
            "verifier_state" => pending_run.verifier_state.is_empty(),
            "advisory_constraints" => pending_run.advisory_constraints.is_empty(),
            _ => false,
        };
        if missing {
            return Some(field);
        }
    }
    None
}

fn parse_u64(raw: &str, field: &str, line: usize) -> Result<u64, String> {
    raw.parse::<u64>()
        .map_err(|error| format!("invalid {field} on line {line}: {error}"))
}

fn parse_mode(raw: &str, line: usize) -> Result<ExecutionMode, String> {
    match raw {
        "active" => Ok(ExecutionMode::Active),
        _ => Err(format!("invalid mode '{raw}' on line {line}")),
    }
}

fn parse_decision(raw: &str, line: usize) -> Result<Option<DecisionOutcome>, String> {
    match raw {
        NONE_SENTINEL => Ok(None),
        "accepted" => Ok(Some(DecisionOutcome::Accepted)),
        "rejected" => Ok(Some(DecisionOutcome::Rejected)),
        _ => Err(format!("invalid decision '{raw}' on line {line}")),
    }
}

fn parse_policy_code(raw: &str, line: usize) -> Result<Option<PolicyCode>, String> {
    let code = match raw {
        NONE_SENTINEL => return Ok(None),
        "allowed" => PolicyCode::Allowed,
        "actor_missing" => PolicyCode::ActorMissing,
        "payload_too_large" => PolicyCode::PayloadTooLarge,
        "constraint_path_scope" => PolicyCode::ConstraintPathScope,
        "constraint_destructive_commands" => PolicyCode::ConstraintDestructiveCommands,
        "constraint_external_commands" => PolicyCode::ConstraintExternalCommands,
        "constraint_approval_escalation" => PolicyCode::ConstraintApprovalEscalation,
        _ => return Err(format!("invalid policy code '{raw}' on line {line}")),
    };
    Ok(Some(code))
}

fn encode_optional(value: Option<&str>) -> String {
    value
        .map(|value| hex_encode(value.as_bytes()))
        .unwrap_or_else(|| NONE_SENTINEL.to_owned())
}

fn decode_optional_hex_utf8(raw: &str, line: usize) -> Result<Option<String>, String> {
    if raw == NONE_SENTINEL {
        return Ok(None);
    }

    decode_hex_utf8(raw)
        .map(Some)
        .ok_or_else(|| format!("invalid optional utf8 field on line {line}"))
}

fn decode_hex_utf8(raw: &str) -> Option<String> {
    let bytes = hex_decode(raw)?;
    String::from_utf8(bytes).ok()
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(s, "{byte:02x}").expect("writing to String is infallible");
    }
    s
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }

    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        PendingRunSnapshot, RuntimeStateSnapshot, StateStore, parse_snapshot, serialize_snapshot,
    };
    use axiomrunner_core::{AgentState, DecisionOutcome, ExecutionMode, PolicyCode};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn state_snapshot_round_trips() {
        let snapshot = RuntimeStateSnapshot {
            state: AgentState {
                revision: 12,
                mode: ExecutionMode::Active,
                last_intent_id: Some(String::from("cli-3")),
                last_actor_id: Some(String::from("system")),
                last_decision: Some(DecisionOutcome::Accepted),
                last_policy_code: Some(PolicyCode::Allowed),
            },
            next_intent_seq: 3,
            next_run_seq: 3,
            pending_run: Some(PendingRunSnapshot {
                run_id: String::from("run-3"),
                intent_id: String::from("cli-3"),
                goal_file_path: String::from("/tmp/goal.json"),
                phase: String::from("waiting_approval"),
                reason: String::from("approval_required_before_execution"),
                reason_code: String::from("approval_required_before_execution"),
                reason_detail: String::from("none"),
                approval_state: String::from("required"),
                verifier_state: String::from("passed"),
                advisory_constraints: String::from("none"),
            }),
        };

        let encoded = serialize_snapshot(&snapshot);
        let decoded = parse_snapshot(&encoded).expect("snapshot should parse");

        assert_eq!(decoded, snapshot);
        assert!(encoded.contains("mode=active"));
    }

    #[test]
    fn state_snapshot_rejects_pending_run_missing_required_field() {
        let raw = "\
version=axiomrunner-state-v2
next_intent_seq=1
next_run_seq=1
revision=1
mode=active
last_intent_id=-
last_actor_id=-
last_decision=-
last_policy_code=-
pending_run.run_id=72756e2d31
pending_run.intent_id=636c692d31
pending_run.goal_file_path=2f746d702f676f616c2e6a736f6e
pending_run.phase=77616974696e675f617070726f76616c
pending_run.approval_state=7265717569726564
pending_run.verifier_state=736b6970706564
";

        let error =
            parse_snapshot(raw).expect_err("missing required pending run field should fail");
        assert!(error.contains("pending run snapshot missing required field 'reason_code' on load"));
    }

    #[test]
    fn state_snapshot_rejects_legacy_version_without_migration() {
        let raw = "\
version=axiomrunner-state-v1
next_intent_seq=1
next_run_seq=1
revision=1
mode=active
last_intent_id=-
last_actor_id=-
last_decision=-
last_policy_code=-
";

        let error = parse_snapshot(raw).expect_err("legacy version should fail");
        assert!(error.contains("unsupported state snapshot version 'axiomrunner-state-v1'"));
        assert!(error.contains("no legacy migration path is supported"));
    }

    fn unique_snapshot_path(label: &str) -> PathBuf {
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiomrunner-state-store-{label}-{}-{tick}.snapshot",
            std::process::id()
        ))
    }

    #[test]
    fn state_store_loads_tmp_snapshot_when_primary_is_missing() {
        let path = unique_snapshot_path("tmp-fallback-missing");
        let store = StateStore { path: path.clone() };
        let snapshot = RuntimeStateSnapshot {
            state: AgentState::default(),
            next_intent_seq: 7,
            next_run_seq: 9,
            pending_run: None,
        };

        fs::write(store.temp_path(), serialize_snapshot(&snapshot))
            .expect("tmp snapshot should exist");

        let loaded = store.load_snapshot().expect("tmp snapshot should load");
        assert_eq!(loaded, snapshot);

        let _ = fs::remove_file(store.temp_path());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn state_store_rejects_tmp_fallback_when_primary_is_corrupt() {
        let path = unique_snapshot_path("tmp-fallback-corrupt");
        let store = StateStore { path: path.clone() };
        let snapshot = RuntimeStateSnapshot {
            state: AgentState::default(),
            next_intent_seq: 3,
            next_run_seq: 4,
            pending_run: None,
        };

        fs::write(&path, "not-a-valid-snapshot").expect("corrupt primary should exist");
        fs::write(store.temp_path(), serialize_snapshot(&snapshot))
            .expect("tmp snapshot should exist");

        let error = store
            .load_snapshot()
            .expect_err("corrupt primary should hard fail");
        assert!(error.contains("parse"));

        let _ = fs::remove_file(store.temp_path());
        let _ = fs::remove_file(path);
    }
}
