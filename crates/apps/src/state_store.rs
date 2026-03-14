use crate::config_loader::AppConfig;
use crate::env_util::read_env_trimmed;
use axonrunner_core::{AgentState, DecisionOutcome, ExecutionMode, PolicyCode};
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

pub const ENV_RUNTIME_STATE_PATH: &str = "AXONRUNNER_RUNTIME_STATE_PATH";

const FORMAT_VERSION: &str = "axonrunner-state-v1";
const NONE_SENTINEL: &str = "-";

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
    pub approval_state: String,
    pub verifier_state: String,
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
                        .join(".axonrunner")
                        .join("state.snapshot")
                })
            })
            .ok_or_else(|| String::from("runtime state path is not available"))?;

        Ok(Self { path })
    }

    pub fn load_snapshot(&self) -> Result<RuntimeStateSnapshot, String> {
        match read_snapshot_file(&self.path) {
            Ok(snapshot) => Ok(snapshot),
            Err(primary_error) => match read_snapshot_file(&self.temp_path()) {
                Ok(snapshot) => Ok(snapshot),
                Err(temp_error) if is_missing_snapshot_error(&primary_error) => {
                    if is_missing_snapshot_error(&temp_error) {
                        Ok(RuntimeStateSnapshot::default())
                    } else {
                        Err(format!(
                            "read state snapshot '{}' fallback '{}' failed: {temp_error}",
                            self.path.display(),
                            self.temp_path().display()
                        ))
                    }
                }
                Err(temp_error) => Err(format!(
                    "read state snapshot '{}' failed: {primary_error}; fallback '{}' failed: {temp_error}",
                    self.path.display(),
                    self.temp_path().display()
                )),
            },
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

fn read_snapshot_file(path: &PathBuf) -> Result<RuntimeStateSnapshot, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("read '{}' failed: {error}", path.display()))?;
    parse_snapshot(&raw).map_err(|error| format!("parse '{}' failed: {error}", path.display()))
}

fn is_missing_snapshot_error(error: &str) -> bool {
    error.contains("No such file or directory") || error.contains("os error 2")
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
            decision_name(snapshot.state.last_decision)
        ),
        format!(
            "last_policy_code={}",
            policy_code_name(snapshot.state.last_policy_code)
        ),
        format!("denied_count={}", snapshot.state.denied_count),
        format!("audit_count={}", snapshot.state.audit_count),
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
            "pending_run.approval_state={}",
            hex_encode(pending_run.approval_state.as_bytes())
        ));
        lines.push(format!(
            "pending_run.verifier_state={}",
            hex_encode(pending_run.verifier_state.as_bytes())
        ));
    }

    for (key, value) in &snapshot.state.facts {
        lines.push(format!(
            "fact.{}={}",
            hex_encode(key.as_bytes()),
            hex_encode(value.as_bytes())
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

fn parse_snapshot(raw: &str) -> Result<RuntimeStateSnapshot, String> {
    let mut snapshot = RuntimeStateSnapshot::default();
    let mut saw_version = false;
    let mut facts = BTreeMap::new();
    let mut pending_run = PendingRunSnapshot {
        run_id: String::new(),
        intent_id: String::new(),
        goal_file_path: String::new(),
        phase: String::new(),
        reason: String::new(),
        approval_state: String::new(),
        verifier_state: String::new(),
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

        if let Some(encoded_key) = key.strip_prefix("fact.") {
            let key = decode_hex_utf8(encoded_key)
                .ok_or_else(|| format!("invalid fact key encoding on line {}", index + 1))?;
            let value = decode_hex_utf8(value)
                .ok_or_else(|| format!("invalid fact value encoding on line {}", index + 1))?;
            facts.insert(key, value);
            continue;
        }

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
                "approval_state" => pending_run.approval_state = decoded,
                "verifier_state" => pending_run.verifier_state = decoded,
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
                        "unsupported state snapshot version '{value}' on line {}",
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
            "denied_count" => snapshot.state.denied_count = parse_u64(value, key, index + 1)?,
            "audit_count" => snapshot.state.audit_count = parse_u64(value, key, index + 1)?,
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

    snapshot.state.facts = facts;
    if snapshot.next_run_seq == 0 {
        snapshot.next_run_seq = snapshot.next_intent_seq;
    }
    if saw_pending_run {
        if pending_run.approval_state.is_empty() {
            pending_run.approval_state = match pending_run.phase.as_str() {
                "waiting_approval" => String::from("required"),
                _ => String::from("unknown"),
            };
        }
        if pending_run.verifier_state.is_empty() {
            pending_run.verifier_state = String::from("unknown");
        }
        snapshot.pending_run = Some(pending_run);
    }
    Ok(snapshot)
}

fn parse_u64(raw: &str, field: &str, line: usize) -> Result<u64, String> {
    raw.parse::<u64>()
        .map_err(|error| format!("invalid {field} on line {line}: {error}"))
}

fn parse_mode(raw: &str, line: usize) -> Result<ExecutionMode, String> {
    match raw {
        "active" => Ok(ExecutionMode::Active),
        "read_only" | "readonly" => Ok(ExecutionMode::ReadOnly),
        "halted" => Ok(ExecutionMode::Halted),
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
        "runtime_halted" => PolicyCode::RuntimeHalted,
        "readonly_mutation" => PolicyCode::ReadOnlyMutation,
        "unauthorized_control" => PolicyCode::UnauthorizedControl,
        "payload_too_large" => PolicyCode::PayloadTooLarge,
        _ => return Err(format!("invalid policy code '{raw}' on line {line}")),
    };
    Ok(Some(code))
}

fn mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Active => "active",
        ExecutionMode::ReadOnly => "read_only",
        ExecutionMode::Halted => "halted",
    }
}

fn decision_name(decision: Option<DecisionOutcome>) -> &'static str {
    match decision {
        Some(DecisionOutcome::Accepted) => "accepted",
        Some(DecisionOutcome::Rejected) => "rejected",
        None => NONE_SENTINEL,
    }
}

fn policy_code_name(code: Option<PolicyCode>) -> &'static str {
    match code {
        Some(code) => code.as_str(),
        None => NONE_SENTINEL,
    }
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
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    use super::{PendingRunSnapshot, RuntimeStateSnapshot, StateStore, parse_snapshot, serialize_snapshot};
    use axonrunner_core::{AgentState, DecisionOutcome, ExecutionMode, PolicyCode};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn state_snapshot_round_trips() {
        let snapshot = RuntimeStateSnapshot {
            state: AgentState {
                revision: 12,
                mode: ExecutionMode::ReadOnly,
                facts: [("alpha".to_owned(), "42".to_owned())]
                    .into_iter()
                    .collect(),
                last_intent_id: Some(String::from("cli-3")),
                last_actor_id: Some(String::from("system")),
                last_decision: Some(DecisionOutcome::Accepted),
                last_policy_code: Some(PolicyCode::Allowed),
                denied_count: 1,
                audit_count: 4,
            },
            next_intent_seq: 3,
            next_run_seq: 3,
            pending_run: Some(PendingRunSnapshot {
                run_id: String::from("run-3"),
                intent_id: String::from("cli-3"),
                goal_file_path: String::from("/tmp/goal.json"),
                phase: String::from("waiting_approval"),
                reason: String::from("approval_required_before_execution"),
                approval_state: String::from("required"),
                verifier_state: String::from("passed"),
            }),
        };

        let encoded = serialize_snapshot(&snapshot);
        let decoded = parse_snapshot(&encoded).expect("snapshot should parse");

        assert_eq!(decoded, snapshot);
        assert!(encoded.contains("mode=read_only"));
    }

    #[test]
    fn state_snapshot_loads_legacy_readonly_mode() {
        let raw = "\
version=axonrunner-state-v1
next_intent_seq=3
next_run_seq=3
revision=12
mode=readonly
last_intent_id=636c692d33
last_actor_id=73797374656d
last_decision=accepted
last_policy_code=allowed
denied_count=1
audit_count=4
fact.616c706861=3432
";

        let decoded = parse_snapshot(raw).expect("legacy snapshot should parse");

        assert_eq!(decoded.next_intent_seq, 3);
        assert_eq!(decoded.next_run_seq, 3);
        assert_eq!(decoded.state.revision, 12);
        assert_eq!(decoded.state.mode, ExecutionMode::ReadOnly);
        assert_eq!(decoded.state.facts.get("alpha"), Some(&String::from("42")));
    }

    #[test]
    fn state_snapshot_legacy_payload_defaults_next_run_seq_from_intent_seq() {
        let raw = "\
version=axonrunner-state-v1
next_intent_seq=9
revision=1
mode=active
last_intent_id=-
last_actor_id=-
last_decision=-
last_policy_code=-
denied_count=0
audit_count=0
";

        let decoded = parse_snapshot(raw).expect("legacy snapshot should parse");
        assert_eq!(decoded.next_intent_seq, 9);
        assert_eq!(decoded.next_run_seq, 9);
    }

    #[test]
    fn state_snapshot_legacy_pending_run_defaults_new_fields() {
        let raw = "\
version=axonrunner-state-v1
next_intent_seq=2
next_run_seq=2
revision=1
mode=active
last_intent_id=-
last_actor_id=-
last_decision=-
last_policy_code=-
denied_count=0
audit_count=0
pending_run.run_id=72756e2d31
pending_run.intent_id=636c692d31
pending_run.goal_file_path=2f746d702f676f616c2e6a736f6e
pending_run.phase=77616974696e675f617070726f76616c
pending_run.reason=617070726f76616c5f72657175697265645f6265666f72655f657865637574696f6e
";

        let decoded = parse_snapshot(raw).expect("legacy pending snapshot should parse");
        let pending = decoded.pending_run.expect("pending run should exist");

        assert_eq!(pending.approval_state, "required");
        assert_eq!(pending.verifier_state, "unknown");
    }

    fn unique_snapshot_path(label: &str) -> PathBuf {
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axonrunner-state-store-{label}-{}-{tick}.snapshot",
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

        fs::write(store.temp_path(), serialize_snapshot(&snapshot)).expect("tmp snapshot should exist");

        let loaded = store.load_snapshot().expect("tmp snapshot should load");
        assert_eq!(loaded, snapshot);

        let _ = fs::remove_file(store.temp_path());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn state_store_loads_tmp_snapshot_when_primary_is_corrupt() {
        let path = unique_snapshot_path("tmp-fallback-corrupt");
        let store = StateStore { path: path.clone() };
        let snapshot = RuntimeStateSnapshot {
            state: AgentState::default(),
            next_intent_seq: 3,
            next_run_seq: 4,
            pending_run: None,
        };

        fs::write(&path, "not-a-valid-snapshot").expect("corrupt primary should exist");
        fs::write(store.temp_path(), serialize_snapshot(&snapshot)).expect("tmp snapshot should exist");

        let loaded = store.load_snapshot().expect("tmp snapshot should load");
        assert_eq!(loaded, snapshot);

        let _ = fs::remove_file(store.temp_path());
        let _ = fs::remove_file(path);
    }
}
