use axiom_adapters::{MemoryAdapter, memory::MarkdownMemoryAdapter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLI_USAGE: &str = "\
usage:
  axiom_apps [global-options] <command> [command-args]

global-options:
  --config-file <path>
  --profile=<name>
  --endpoint=<url>
  --actor=<id>  (default: system)

commands:
  onboard [onboard-options]
  agent [agent-options]
  read <key>
  write <key> <value>
  remove <key>
  freeze
  halt
  status
  batch [--reset-state] <intent-spec>...
  health
  doctor
  cron <list|add|remove> [cron-args]
  service <install|start|stop|status|uninstall>
  channel <list|start|doctor|add|remove|serve> [channel-args]
  integrations info <name>
  skills <list|install|remove> [skills-args]
  migrate --legacy-root <path> --target-root <path> [migrate-options]
  serve --mode=<gateway|daemon>

intent-spec:
  read:<key>
  write:<key>=<value>
  remove:<key>
  freeze
  halt";

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .args(args)
        .output()
        .expect("axiom_apps binary should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn unique_path(label: &str, extension: &str) -> PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axiom-e2e-cli-{label}-{}-{tick}.{extension}",
        std::process::id()
    ))
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("path should be UTF-8")
}

fn parse_cron_id(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let rest = line.strip_prefix("cron added id=")?;
        rest.split_whitespace().next().map(ToOwned::to_owned)
    })
}

#[test]
fn e2e_cli_onboard_quick_initializes_workspace_and_profile() {
    let state_path = unique_path("onboard-state", "db");
    let workspace_path = unique_path("onboard-workspace", "dir");

    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_ONBOARD_STATE_PATH", &state_path)
        .env("AXIOM_ONBOARD_WORKSPACE_PATH", &workspace_path)
        .args([
            "--profile=dev",
            "onboard",
            "--provider=openai",
            "--memory=markdown",
            "--api-key=sk-test",
        ])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("onboard configured profile=dev"));
    assert!(stdout.contains("provider=openai"));
    assert!(stdout.contains("memory=markdown"));
    assert!(stdout.contains("api_key_set=true"));
    assert!(stdout.contains("interactive=false"));
    assert!(stdout.contains("channels_only=false"));
    assert!(state_path.exists(), "state file should be created");
    assert!(
        workspace_path
            .join("profiles")
            .join("dev")
            .join("profile.db")
            .exists(),
        "profile manifest should be created"
    );

    let _ = fs::remove_file(state_path);
    let _ = fs::remove_dir_all(workspace_path);
}

#[test]
fn e2e_cli_onboard_rejects_interactive_and_channels_only_together() {
    let output = run_cli(&["onboard", "--interactive", "--channels-only"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert!(
        stderr.contains("use either --interactive or --channels-only"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn e2e_cli_agent_single_message_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_AGENT_ID", "mock")
        .args(["agent", "--message=hello", "--cwd=/tmp", "--model=gpt-4o-mini"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("agent single agent=mock"));
    assert!(stdout.contains("model=gpt-4o-mini"));
    assert!(stdout.contains("input=hello"));
    assert!(stdout.contains("reason=single_message_completed"));
}

#[test]
fn e2e_cli_agent_interactive_mode_from_env_script() {
    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_AGENT_ID", "mock")
        .env("AXIOM_AGENT_SCRIPT", "hello|status|exit")
        .args(["agent", "--cwd=/tmp", "--model=gpt-4o"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("agent interactive agent=mock"));
    assert!(stdout.contains("model=gpt-4o"));
    assert!(stdout.contains("agent turn index=1 input=hello"));
    assert!(stdout.contains("agent turn index=2 input=status"));
    assert!(stdout.contains("agent turn index=3 input=exit"));
    assert!(stdout.contains("agent complete turns=3 reason=exit_command"));
}

#[test]
fn e2e_cli_agent_rejects_unknown_option() {
    let output = run_cli(&["agent", "--unknown-option=value"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert!(
        stderr.contains("unknown agent option"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn e2e_cli_batch_pipeline_flow() {
    let output = run_cli(&[
        "--actor=system",
        "batch",
        "write:alpha=1",
        "read:alpha",
        "freeze",
        "write:beta=2",
        "remove:alpha",
        "halt",
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("intent id=cli-1 kind=write outcome=accepted policy=allowed effects=1")
    );
    assert!(stdout.contains("read key=alpha value=1"));
    assert!(stdout.contains(
        "intent id=cli-4 kind=write outcome=rejected policy=readonly_mutation effects=0"
    ));
    assert!(stdout.contains("intent id=cli-6 kind=halt outcome=accepted policy=allowed effects=1"));
    assert!(
        stdout.contains("batch completed count=6 revision=24 mode=halted facts=1 denied=2 audit=6")
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
}

#[test]
fn e2e_cli_batch_single_intent_respects_control_policy() {
    let output = run_cli(&["--actor=alice", "batch", "freeze"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.contains(
        "intent id=cli-1 kind=freeze outcome=rejected policy=unauthorized_control effects=0"
    ));
    assert!(
        stdout.contains("batch completed count=1 revision=4 mode=active facts=0 denied=1 audit=1")
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
}

#[test]
fn e2e_cli_batch_reset_status_and_health_commands() {
    let batch = run_cli(&[
        "--actor=system",
        "batch",
        "--reset-state",
        "write:key=value",
        "remove:key",
    ]);
    let batch_stdout = stdout_of(&batch);
    let batch_stderr = stderr_of(&batch);
    assert!(
        batch.status.success(),
        "stdout:\n{batch_stdout}\n\nstderr:\n{batch_stderr}"
    );
    assert!(
        batch_stdout
            .contains("batch completed count=2 revision=8 mode=active facts=0 denied=0 audit=2")
    );
    assert!(batch_stderr.is_empty(), "stderr:\n{batch_stderr}");

    let status = run_cli(&["status"]);
    let status_stdout = stdout_of(&status);
    let status_stderr = stderr_of(&status);
    assert!(
        status.status.success(),
        "stdout:\n{status_stdout}\n\nstderr:\n{status_stderr}"
    );
    assert!(status_stdout.contains("status revision=0 mode=active facts=0 denied=0 audit=0"));
    assert!(status_stderr.is_empty(), "stderr:\n{status_stderr}");

    let health = run_cli(&["health"]);
    let health_stdout = stdout_of(&health);
    let health_stderr = stderr_of(&health);
    assert!(
        health.status.success(),
        "stdout:\n{health_stdout}\n\nstderr:\n{health_stderr}"
    );
    assert!(health_stdout.contains(
        "health ok=true profile=prod endpoint=http://127.0.0.1:8080 mode=active revision=0"
    ));
    assert!(health_stderr.is_empty(), "stderr:\n{health_stderr}");
}

#[test]
fn e2e_cli_status_includes_runtime_daemon_and_channel_summary() {
    let health_path = unique_path("status-health", "status");
    let channel_store_path = unique_path("status-channel-store", "db");
    fs::write(
        &health_path,
        "tick=7\nstate=running\nstate_detail=item=sync attempt=1\nreason=-\nin_flight=sync\nin_flight_attempt=1\nqueue_depth=0\ncompleted=3\nfailed=0\n",
    )
    .expect("daemon health fixture should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_RUNTIME_PROVIDER_MODEL", "status-e2e-model")
        .env("AXIOM_DAEMON_HEALTH_PATH", &health_path)
        .env("AXIOM_CHANNEL_STORE_PATH", &channel_store_path)
        .env_remove("AXIOM_RUNTIME_MEMORY_PATH")
        .env_remove("AXIOM_RUNTIME_TOOL_WORKSPACE")
        .args(["status"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("status revision=0 mode=active facts=0 denied=0 audit=0"));
    assert!(stdout.contains(
        "status runtime provider_id=mock-local provider_model=status-e2e-model memory_enabled=true memory_state=ready tool_enabled=true tool_state=ready bootstrap_state=disabled"
    ));
    assert!(stdout.contains(
        "status daemon state=ok tick=7 daemon_state=running in_flight=sync queue_depth=0 completed=3 failed=0"
    ));
    assert!(stdout.contains("status channels total=0 running=0"));

    let _ = fs::remove_file(health_path);
    let _ = fs::remove_file(channel_store_path);
}

#[test]
fn e2e_cli_status_reads_daemon_health_from_state_pointer_when_env_missing() {
    let health_path = unique_path("status-health-fallback", "status");
    let state_path = unique_path("status-health-pointer", "txt");
    let channel_store_path = unique_path("status-channel-store-fallback", "db");
    fs::write(
        &health_path,
        "tick=11\nstate=running\nstate_detail=item=sync attempt=2\nreason=-\nin_flight=sync\nin_flight_attempt=2\nqueue_depth=1\ncompleted=5\nfailed=1\n",
    )
    .expect("daemon health fixture should be writable");
    fs::write(&state_path, format!("{}\n", path_str(&health_path)))
        .expect("daemon health state pointer should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_RUNTIME_PROVIDER_MODEL", "status-fallback-model")
        .env("AXIOM_DAEMON_HEALTH_STATE_PATH", &state_path)
        .env_remove("AXIOM_DAEMON_HEALTH_PATH")
        .env("AXIOM_CHANNEL_STORE_PATH", &channel_store_path)
        .env_remove("AXIOM_RUNTIME_MEMORY_PATH")
        .env_remove("AXIOM_RUNTIME_TOOL_WORKSPACE")
        .args(["status"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains(
        "status runtime provider_id=mock-local provider_model=status-fallback-model memory_enabled=true memory_state=ready tool_enabled=true tool_state=ready bootstrap_state=disabled"
    ));
    assert!(stdout.contains(
        "status daemon state=ok tick=11 daemon_state=running in_flight=sync queue_depth=1 completed=5 failed=1"
    ));
    assert!(stdout.contains("status channels total=0 running=0"));

    let _ = fs::remove_file(health_path);
    let _ = fs::remove_file(state_path);
    let _ = fs::remove_file(channel_store_path);
}

#[test]
fn e2e_cli_doctor_reports_deterministic_summary() {
    let health_path = unique_path("doctor-health", "status");
    fs::write(
        &health_path,
        "tick=7\nstate=running\nstate_detail=item=sync attempt=1\nreason=-\nin_flight=sync\nin_flight_attempt=1\nqueue_depth=0\ncompleted=3\nfailed=0\n",
    )
    .expect("daemon health fixture should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_DAEMON_HEALTH_PATH", &health_path)
        .env("AXIOM_RUNTIME_PROVIDER_MODEL", "doctor-e2e-model")
        .env_remove("AXIOM_RUNTIME_MEMORY_PATH")
        .env_remove("AXIOM_RUNTIME_TOOL_WORKSPACE")
        .args(["--profile=dev", "--endpoint=http://doctor.local", "doctor"])
        .output()
        .expect("axiom_apps binary should run");

    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains(
        "doctor ok=true profile=dev endpoint=http://doctor.local mode=active revision=0 checks=6"
    ));
    assert!(
        stdout.contains(
            "doctor check=endpoint_scheme level=pass detail=endpoint=http://doctor.local"
        )
    );
    assert!(stdout.contains("doctor check=runtime_mode level=pass detail=mode=active"));
    assert!(
        stdout.contains(
            "doctor check=provider_model level=pass detail=provider_model=doctor-e2e-model"
        )
    );
    assert!(stdout.contains("doctor check=memory_adapter level=info detail=enabled=true"));
    assert!(stdout.contains("doctor check=tool_adapter level=info detail=enabled=true"));
    assert!(stdout.contains(&format!(
        "doctor check=daemon_health level=pass detail=status=ok env=AXIOM_DAEMON_HEALTH_PATH path={} tick=7 state=running state_detail=item=sync attempt=1 reason=- in_flight=sync in_flight_attempt=1 queue_depth=0 completed=3 failed=0",
        path_str(&health_path)
    )));

    let _ = fs::remove_file(health_path);
}

#[test]
fn e2e_cli_cron_add_list_remove_flow() {
    let store_path = unique_path("cron-store", "db");

    let add = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CRON_STORE_PATH", &store_path)
        .args(["cron", "add", "*/5 * * * *", "echo", "hello"])
        .output()
        .expect("axiom_apps binary should run");
    let add_stdout = stdout_of(&add);
    let add_stderr = stderr_of(&add);
    assert!(
        add.status.success(),
        "stdout:\n{add_stdout}\n\nstderr:\n{add_stderr}"
    );
    assert!(add_stderr.is_empty(), "stderr:\n{add_stderr}");
    assert!(add_stdout.contains("cron added id="));
    assert!(add_stdout.contains("expr=*/5 * * * *"));
    assert!(add_stdout.contains("cmd=echo hello"));
    let id = parse_cron_id(&add_stdout).expect("added cron id should be present");

    let list = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CRON_STORE_PATH", &store_path)
        .args(["cron", "list"])
        .output()
        .expect("axiom_apps binary should run");
    let list_stdout = stdout_of(&list);
    let list_stderr = stderr_of(&list);
    assert!(
        list.status.success(),
        "stdout:\n{list_stdout}\n\nstderr:\n{list_stderr}"
    );
    assert!(list_stderr.is_empty(), "stderr:\n{list_stderr}");
    assert!(list_stdout.contains("cron list count=1"));
    assert!(list_stdout.contains(&format!("cron job id={id}")));
    assert!(list_stdout.contains("expr=*/5 * * * *"));
    assert!(list_stdout.contains("cmd=echo hello"));

    let remove = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CRON_STORE_PATH", &store_path)
        .args(["cron", "remove", &id])
        .output()
        .expect("axiom_apps binary should run");
    let remove_stdout = stdout_of(&remove);
    let remove_stderr = stderr_of(&remove);
    assert!(
        remove.status.success(),
        "stdout:\n{remove_stdout}\n\nstderr:\n{remove_stderr}"
    );
    assert!(remove_stderr.is_empty(), "stderr:\n{remove_stderr}");
    assert!(remove_stdout.contains(&format!("cron removed id={id} remaining=0")));

    let list_after = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CRON_STORE_PATH", &store_path)
        .args(["cron", "list"])
        .output()
        .expect("axiom_apps binary should run");
    let list_after_stdout = stdout_of(&list_after);
    let list_after_stderr = stderr_of(&list_after);
    assert!(
        list_after.status.success(),
        "stdout:\n{list_after_stdout}\n\nstderr:\n{list_after_stderr}"
    );
    assert!(list_after_stderr.is_empty(), "stderr:\n{list_after_stderr}");
    assert!(list_after_stdout.contains("cron list count=0 due=0"));

    let _ = fs::remove_file(store_path);
}

#[test]
fn e2e_cli_service_lifecycle_smoke_flow() {
    let state_path = unique_path("service-state", "db");

    let install = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SERVICE_STATE_PATH", &state_path)
        .args(["service", "install"])
        .output()
        .expect("axiom_apps binary should run");
    let install_stdout = stdout_of(&install);
    let install_stderr = stderr_of(&install);
    assert!(
        install.status.success(),
        "stdout:\n{install_stdout}\n\nstderr:\n{install_stderr}"
    );
    assert!(install_stderr.is_empty(), "stderr:\n{install_stderr}");
    assert!(install_stdout.contains("service installed=true"));
    assert!(install_stdout.contains("running=false"));

    let start = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SERVICE_STATE_PATH", &state_path)
        .args(["service", "start"])
        .output()
        .expect("axiom_apps binary should run");
    let start_stdout = stdout_of(&start);
    let start_stderr = stderr_of(&start);
    assert!(
        start.status.success(),
        "stdout:\n{start_stdout}\n\nstderr:\n{start_stderr}"
    );
    assert!(start_stderr.is_empty(), "stderr:\n{start_stderr}");
    assert!(start_stdout.contains("service started=true"));
    assert!(start_stdout.contains("running=true"));

    let status = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SERVICE_STATE_PATH", &state_path)
        .args(["service", "status"])
        .output()
        .expect("axiom_apps binary should run");
    let status_stdout = stdout_of(&status);
    let status_stderr = stderr_of(&status);
    assert!(
        status.status.success(),
        "stdout:\n{status_stdout}\n\nstderr:\n{status_stderr}"
    );
    assert!(status_stderr.is_empty(), "stderr:\n{status_stderr}");
    assert!(status_stdout.contains("service status installed=true"));
    assert!(status_stdout.contains("running=true"));

    let stop = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SERVICE_STATE_PATH", &state_path)
        .args(["service", "stop"])
        .output()
        .expect("axiom_apps binary should run");
    let stop_stdout = stdout_of(&stop);
    let stop_stderr = stderr_of(&stop);
    assert!(
        stop.status.success(),
        "stdout:\n{stop_stdout}\n\nstderr:\n{stop_stderr}"
    );
    assert!(stop_stderr.is_empty(), "stderr:\n{stop_stderr}");
    assert!(stop_stdout.contains("service stopped=true"));
    assert!(stop_stdout.contains("running=false"));

    let uninstall = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SERVICE_STATE_PATH", &state_path)
        .args(["service", "uninstall"])
        .output()
        .expect("axiom_apps binary should run");
    let uninstall_stdout = stdout_of(&uninstall);
    let uninstall_stderr = stderr_of(&uninstall);
    assert!(
        uninstall.status.success(),
        "stdout:\n{uninstall_stdout}\n\nstderr:\n{uninstall_stderr}"
    );
    assert!(uninstall_stderr.is_empty(), "stderr:\n{uninstall_stderr}");
    assert!(uninstall_stdout.contains("service uninstalled=true removed=true"));

    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_service_start_before_install_fails() {
    let state_path = unique_path("service-start-before-install", "db");
    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SERVICE_STATE_PATH", &state_path)
        .args(["service", "start"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert!(
        stderr.contains("service is not installed"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_file(state_path);
}

#[test]
fn e2e_cli_channel_add_start_doctor_remove_flow() {
    let store_path = unique_path("channel-store", "db");

    let add = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "add", "telegram", "bot_token=abc"])
        .output()
        .expect("axiom_apps binary should run");
    let add_stdout = stdout_of(&add);
    let add_stderr = stderr_of(&add);
    assert!(
        add.status.success(),
        "stdout:\n{add_stdout}\n\nstderr:\n{add_stderr}"
    );
    assert!(add_stderr.is_empty(), "stderr:\n{add_stderr}");
    assert!(add_stdout.contains("channel added name=telegram type=telegram"));

    let list = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "list"])
        .output()
        .expect("axiom_apps binary should run");
    let list_stdout = stdout_of(&list);
    let list_stderr = stderr_of(&list);
    assert!(
        list.status.success(),
        "stdout:\n{list_stdout}\n\nstderr:\n{list_stderr}"
    );
    assert!(list_stderr.is_empty(), "stderr:\n{list_stderr}");
    assert!(list_stdout.contains("channel list count=1 running=0"));
    assert!(list_stdout.contains("channel entry name=telegram type=telegram running=false"));

    let start = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "start"])
        .output()
        .expect("axiom_apps binary should run");
    let start_stdout = stdout_of(&start);
    let start_stderr = stderr_of(&start);
    assert!(
        start.status.success(),
        "stdout:\n{start_stdout}\n\nstderr:\n{start_stderr}"
    );
    assert!(start_stderr.is_empty(), "stderr:\n{start_stderr}");
    assert!(start_stdout.contains("channel start started=1 running=1"));

    let doctor = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "doctor"])
        .output()
        .expect("axiom_apps binary should run");
    let doctor_stdout = stdout_of(&doctor);
    let doctor_stderr = stderr_of(&doctor);
    assert!(
        doctor.status.success(),
        "stdout:\n{doctor_stdout}\n\nstderr:\n{doctor_stderr}"
    );
    assert!(doctor_stderr.is_empty(), "stderr:\n{doctor_stderr}");
    assert!(doctor_stdout.contains("channel doctor count=1 healthy=1 unhealthy=0"));
    assert!(doctor_stdout.contains("channel check name=telegram type=telegram status=ok"));

    let remove = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "remove", "telegram"])
        .output()
        .expect("axiom_apps binary should run");
    let remove_stdout = stdout_of(&remove);
    let remove_stderr = stderr_of(&remove);
    assert!(
        remove.status.success(),
        "stdout:\n{remove_stdout}\n\nstderr:\n{remove_stderr}"
    );
    assert!(remove_stderr.is_empty(), "stderr:\n{remove_stderr}");
    assert!(remove_stdout.contains("channel removed name=telegram remaining=0"));

    let list_after = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "list"])
        .output()
        .expect("axiom_apps binary should run");
    let list_after_stdout = stdout_of(&list_after);
    let list_after_stderr = stderr_of(&list_after);
    assert!(
        list_after.status.success(),
        "stdout:\n{list_after_stdout}\n\nstderr:\n{list_after_stderr}"
    );
    assert!(list_after_stderr.is_empty(), "stderr:\n{list_after_stderr}");
    assert!(list_after_stdout.contains("channel list count=0 running=0"));

    let _ = fs::remove_file(store_path);
}

#[test]
fn e2e_cli_channel_remove_missing_fails() {
    let store_path = unique_path("channel-remove-missing", "db");
    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
        .args(["channel", "remove", "unknown"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert!(
        stderr.contains("channel 'unknown' not found"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_file(store_path);
}

#[test]
fn e2e_cli_channel_add_accepts_matrix_whatsapp_irc_types() {
    let store_path = unique_path("channel-extended-types", "db");

    for channel_type in ["matrix", "whatsapp", "irc"] {
        let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
            .env("AXIOM_CHANNEL_STORE_PATH", &store_path)
            .args(["channel", "add", channel_type, "token=demo"])
            .output()
            .expect("axiom_apps binary should run");
        let stdout = stdout_of(&output);
        let stderr = stderr_of(&output);

        assert!(
            output.status.success(),
            "channel_type={channel_type}\nstdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
        assert!(stderr.is_empty(), "stderr:\n{stderr}");
        assert!(stdout.contains(&format!(
            "channel added name={channel_type} type={channel_type}"
        )));
    }

    let _ = fs::remove_file(store_path);
}

#[test]
fn e2e_cli_integrations_info_returns_catalog_row() {
    let output = run_cli(&["integrations", "info", "telegram"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("integrations info name=telegram"));
    assert!(stdout.contains("category=chat"));
    assert!(stdout.contains("status=available"));
    assert!(stdout.contains("transport=telegram_polling"));
}

#[test]
fn e2e_cli_integrations_info_unknown_name_fails() {
    let output = run_cli(&["integrations", "info", "unknown"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert!(
        stderr.contains("unknown integration 'unknown'"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn e2e_cli_integrations_info_supports_case_insensitive_and_status_variants() {
    let openai = run_cli(&["integrations", "info", "OPENAI"]);
    let openai_stdout = stdout_of(&openai);
    let openai_stderr = stderr_of(&openai);
    assert!(
        openai.status.success(),
        "stdout:\n{openai_stdout}\n\nstderr:\n{openai_stderr}"
    );
    assert!(openai_stderr.is_empty(), "stderr:\n{openai_stderr}");
    assert!(openai_stdout.contains("integrations info name=openai"));
    assert!(openai_stdout.contains("category=ai_model"));
    assert!(openai_stdout.contains("status=active"));

    let github = run_cli(&["integrations", "info", "github"]);
    let github_stdout = stdout_of(&github);
    let github_stderr = stderr_of(&github);
    assert!(
        github.status.success(),
        "stdout:\n{github_stdout}\n\nstderr:\n{github_stderr}"
    );
    assert!(github_stderr.is_empty(), "stderr:\n{github_stderr}");
    assert!(github_stdout.contains("integrations info name=github"));
    assert!(github_stdout.contains("category=productivity"));
    assert!(github_stdout.contains("status=coming_soon"));
}

#[test]
fn e2e_cli_skills_list_install_remove_local_flow() {
    let skills_dir = unique_path("skills-dir", "dir");
    let source_root = unique_path("skills-source", "dir");
    let source_skill = source_root.join("demo_skill");
    fs::create_dir_all(&source_skill).expect("source skill should be creatable");
    fs::write(source_skill.join("SKILL.md"), "# Demo skill\n\nRun task\n")
        .expect("skill markdown should be writable");

    let install = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SKILLS_DIR", &skills_dir)
        .args(["skills", "install", path_str(&source_skill)])
        .output()
        .expect("axiom_apps binary should run");
    let install_stdout = stdout_of(&install);
    let install_stderr = stderr_of(&install);
    assert!(
        install.status.success(),
        "stdout:\n{install_stdout}\n\nstderr:\n{install_stderr}"
    );
    assert!(install_stderr.is_empty(), "stderr:\n{install_stderr}");
    assert!(install_stdout.contains("skills installed name=demo_skill"));
    assert!(
        install_stdout.contains("mode=linked") || install_stdout.contains("mode=copied"),
        "stdout:\n{install_stdout}"
    );

    let list = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SKILLS_DIR", &skills_dir)
        .args(["skills", "list"])
        .output()
        .expect("axiom_apps binary should run");
    let list_stdout = stdout_of(&list);
    let list_stderr = stderr_of(&list);
    assert!(
        list.status.success(),
        "stdout:\n{list_stdout}\n\nstderr:\n{list_stderr}"
    );
    assert!(list_stderr.is_empty(), "stderr:\n{list_stderr}");
    assert!(list_stdout.contains("skills list count=1"));
    assert!(list_stdout.contains("skills entry name=demo_skill"));
    assert!(list_stdout.contains("description=Demo skill"));

    let remove = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SKILLS_DIR", &skills_dir)
        .args(["skills", "remove", "demo_skill"])
        .output()
        .expect("axiom_apps binary should run");
    let remove_stdout = stdout_of(&remove);
    let remove_stderr = stderr_of(&remove);
    assert!(
        remove.status.success(),
        "stdout:\n{remove_stdout}\n\nstderr:\n{remove_stderr}"
    );
    assert!(remove_stderr.is_empty(), "stderr:\n{remove_stderr}");
    assert!(remove_stdout.contains("skills removed name=demo_skill removed=true"));

    let list_after = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SKILLS_DIR", &skills_dir)
        .args(["skills", "list"])
        .output()
        .expect("axiom_apps binary should run");
    let list_after_stdout = stdout_of(&list_after);
    let list_after_stderr = stderr_of(&list_after);
    assert!(
        list_after.status.success(),
        "stdout:\n{list_after_stdout}\n\nstderr:\n{list_after_stderr}"
    );
    assert!(list_after_stderr.is_empty(), "stderr:\n{list_after_stderr}");
    assert!(list_after_stdout.contains("skills list count=0"));

    let _ = fs::remove_dir_all(skills_dir);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn e2e_cli_skills_remove_path_traversal_fails() {
    let skills_dir = unique_path("skills-remove-invalid", "dir");
    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_SKILLS_DIR", &skills_dir)
        .args(["skills", "remove", "../escape"])
        .output()
        .expect("axiom_apps binary should run");
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert!(stderr.contains("invalid skill name"), "stderr:\n{stderr}");

    let _ = fs::remove_dir_all(skills_dir);
}

#[test]
fn e2e_cli_serve_modes() {
    let gateway = run_cli(&["--endpoint=http://gateway.local", "serve", "--mode=gateway"]);
    let gateway_stdout = stdout_of(&gateway);
    let gateway_stderr = stderr_of(&gateway);
    assert!(
        gateway.status.success(),
        "stdout:\n{gateway_stdout}\n\nstderr:\n{gateway_stderr}"
    );
    assert!(gateway_stdout.contains("gateway started profile=prod endpoint=http://gateway.local"));
    assert!(gateway_stderr.is_empty(), "stderr:\n{gateway_stderr}");

    let daemon = run_cli(&["--profile=dev", "serve", "--mode=daemon"]);
    let daemon_stdout = stdout_of(&daemon);
    let daemon_stderr = stderr_of(&daemon);
    if cfg!(debug_assertions) {
        assert!(
            daemon.status.success(),
            "stdout:\n{daemon_stdout}\n\nstderr:\n{daemon_stderr}"
        );
        assert!(
            daemon_stdout.contains("daemon started profile=dev endpoint=http://127.0.0.1:8080")
        );
        assert!(daemon_stderr.is_empty(), "stderr:\n{daemon_stderr}");
    } else {
        assert_eq!(
            daemon.status.code(),
            Some(2),
            "stdout:\n{daemon_stdout}\n\nstderr:\n{daemon_stderr}"
        );
        assert!(daemon_stderr.contains("release gate blocked startup"));
        assert!(daemon_stderr.contains("profile=dev is blocked in release builds"));
    }
}

#[test]
fn e2e_cli_migrate_dry_run_via_main_command() {
    let root = unique_path("migrate-command", "dir");
    let legacy_root = root.join("legacy");
    let target_root = root.join("target");
    fs::create_dir_all(&legacy_root).expect("legacy root should be creatable");
    fs::write(
        legacy_root.join("config.toml"),
        "profile = \"prod\"\nendpoint = \"http://legacy.local\"\n",
    )
    .expect("config should be writable");
    fs::write(legacy_root.join("workspace"), "2.0.0\n").expect("workspace hint should be writable");

    let output = run_cli(&[
        "migrate",
        "--legacy-root",
        path_str(&legacy_root),
        "--target-root",
        path_str(&target_root),
        "--dry-run",
    ]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert!(stdout.contains("\"dry_run\":true"));
    assert!(stdout.contains("\"fatal\":false"));
    assert!(stdout.contains("\"expected_schema\":\"2.0.0\""));
    assert!(!target_root.join("config.toml").exists());
    assert!(!target_root.join("memory").join("MEMORY.md").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn e2e_cli_write_composes_provider_memory_and_tool() {
    let memory_path = unique_path("compose-memory", "md");
    let workspace = unique_path("compose-workspace", "dir");

    let output = Command::new(env!("CARGO_BIN_EXE_axiom_apps"))
        .env("AXIOM_RUNTIME_MEMORY_PATH", &memory_path)
        .env("AXIOM_RUNTIME_TOOL_WORKSPACE", &workspace)
        .env("AXIOM_RUNTIME_TOOL_LOG_PATH", "runtime.log")
        .args(["--actor=system", "write", "alpha", "42"])
        .output()
        .expect("axiom_apps binary should run");

    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert!(
        output.status.success(),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("intent id=cli-1 kind=write outcome=accepted policy=allowed effects=1")
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");

    let reader = MarkdownMemoryAdapter::new(memory_path.clone()).expect("memory file should load");
    let record = reader
        .get("alpha")
        .expect("memory read should succeed")
        .expect("alpha should be persisted");
    assert_eq!(record.value, "42");

    let log_path = workspace.join("runtime.log");
    let log = fs::read_to_string(&log_path).expect("tool log should exist");
    assert!(log.contains("intent=cli-1 kind=write key=alpha"));
    assert!(log.contains("provider=intent=cli-1 kind=write key=alpha value=42"));

    let _ = fs::remove_file(memory_path);
    let _ = fs::remove_file(log_path);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn e2e_cli_unknown_command_stderr_stable() {
    let output = run_cli(&["gateway"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert_eq!(
        stderr,
        format!("unknown command 'gateway'\n{CLI_USAGE}\n"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn e2e_cli_removed_apply_command_stderr_stable() {
    let output = run_cli(&["apply", "write:key=value"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert_eq!(
        stderr,
        format!("unknown command 'apply'\n{CLI_USAGE}\n"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn e2e_cli_removed_replay_command_stderr_stable() {
    let output = run_cli(&["replay", "write:key=value"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert_eq!(
        stderr,
        format!("unknown command 'replay'\n{CLI_USAGE}\n"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn e2e_cli_unknown_serve_mode_stderr_stable() {
    let output = run_cli(&["serve", "--mode=worker"]);
    let stdout = stdout_of(&output);
    let stderr = stderr_of(&output);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "stdout:\n{stdout}");
    assert_eq!(
        stderr,
        format!("unknown serve mode 'worker'\n{CLI_USAGE}\n"),
        "stderr:\n{stderr}"
    );
}
