fn main() -> std::process::ExitCode {
    h2_impl::run()
}

use axiom_apps::display::mode_name;
use axiom_apps::gateway;

mod h2_impl {
    use super::{gateway, mode_name};
    use axiom_core::DecisionOutcome;
    use gateway::{GatewayRejectReason, GatewayRuntime, HttpBoundaryRequest};
    use std::{
        fmt::Write as _,
        fs,
        path::{Path, PathBuf},
        process::{Command, ExitCode},
    };

    const USAGE: &str =
        "usage:\n  h2_verify --apps-bin <path> [--allowed-diff <u32>] [--report <path>]";
    struct Args {
        apps_bin: PathBuf,
        allowed_diff: u32,
        report: Option<PathBuf>,
    }
    enum ParsedArgs {
        Help,
        Run(Args),
    }
    struct Def {
        id: &'static str,
        scenario: Scenario,
    }
    enum Scenario {
        Cli(CliCase),
        Gateway(GwCase),
    }
    struct CliCase {
        args: Vec<&'static str>,
        exp: CliExp,
    }
    #[derive(Clone, Copy)]
    struct CliExp {
        code: i32,
        out_has: &'static [&'static str],
        err_has: &'static [&'static str],
        out_empty: bool,
        err_empty: bool,
    }
    struct GwCase {
        reqs: Vec<Req>,
        exp: GwExp,
    }
    #[derive(Clone, Copy)]
    struct Req {
        method: &'static str,
        path: &'static str,
        body: &'static str,
        source_ip: &'static str,
    }
    #[derive(Clone, Copy)]
    struct GwExp {
        status: u16,
        processed: bool,
        reject: Option<&'static str>,
        decision: Option<&'static str>,
        policy: Option<&'static str>,
        revision: u64,
        mode: &'static str,
        facts: usize,
        denied: u64,
        audit: u64,
        msg_has: Option<&'static str>,
    }
    struct CliNow {
        code: i32,
        out: String,
        err: String,
    }
    struct GwNow {
        status: u16,
        processed: bool,
        reject: Option<String>,
        decision: Option<String>,
        policy: Option<String>,
        revision: u64,
        mode: String,
        facts: usize,
        denied: u64,
        audit: u64,
        msg: String,
    }
    struct CaseResult {
        id: &'static str,
        kind: &'static str,
        matched: bool,
        diffs: Vec<String>,
    }

    pub(super) fn run() -> ExitCode {
        let args = match parse_args(std::env::args().skip(1).collect()) {
            Ok(ParsedArgs::Help) => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            Ok(ParsedArgs::Run(v)) => v,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        let defs = defs();
        let mut results = Vec::with_capacity(defs.len());
        let mut diff_count = 0_u32;
        for def in defs {
            let r = run_case(&args.apps_bin, def);
            if !r.matched {
                diff_count = diff_count.saturating_add(1);
            }
            results.push(r);
        }
        let gate = if diff_count <= args.allowed_diff {
            "pass"
        } else {
            "fail"
        };
        let json = to_json(
            "h2_parallel_validation_v1",
            &args.apps_bin.display().to_string(),
            args.allowed_diff,
            results.len() as u32,
            diff_count,
            gate,
            &results,
        );
        if let Some(path) = &args.report
            && let Err(e) = std::fs::write(path, &json)
        {
            eprintln!("failed to write report to '{}': {e}", path.display());
            return ExitCode::from(1);
        }
        println!("{json}");
        if gate == "pass" {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        }
    }

    fn parse_args(raw: Vec<String>) -> Result<ParsedArgs, String> {
        if raw.iter().any(|a| a == "-h" || a == "--help") {
            return Ok(ParsedArgs::Help);
        }
        let mut apps_bin: Option<PathBuf> = None;
        let mut allowed_diff = env_allowed_diff()?;
        let mut report = None;
        let mut i = 0;
        while i < raw.len() {
            let cur = &raw[i];
            match cur.as_str() {
                "--apps-bin" => {
                    i += 1;
                    let v = raw
                        .get(i)
                        .ok_or_else(|| String::from("--apps-bin requires a path"))?;
                    apps_bin = Some(PathBuf::from(v));
                }
                "--allowed-diff" => {
                    i += 1;
                    let v = raw
                        .get(i)
                        .ok_or_else(|| String::from("--allowed-diff requires a u32"))?;
                    allowed_diff = parse_u32(v, "--allowed-diff")?;
                }
                "--report" => {
                    i += 1;
                    let v = raw
                        .get(i)
                        .ok_or_else(|| String::from("--report requires a path"))?;
                    report = Some(PathBuf::from(v));
                }
                _ if cur.starts_with("--apps-bin=") => apps_bin = Some(PathBuf::from(&cur[11..])),
                _ if cur.starts_with("--allowed-diff=") => {
                    allowed_diff = parse_u32(&cur[15..], "--allowed-diff")?
                }
                _ if cur.starts_with("--report=") => report = Some(PathBuf::from(&cur[9..])),
                _ => return Err(format!("unknown option '{cur}'\n{USAGE}")),
            }
            i += 1;
        }
        let apps_bin = apps_bin.ok_or_else(|| format!("missing required --apps-bin\n{USAGE}"))?;
        if !apps_bin.exists() {
            return Err(format!("--apps-bin does not exist: {}", apps_bin.display()));
        }
        if !apps_bin.is_file() {
            return Err(format!("--apps-bin is not a file: {}", apps_bin.display()));
        }
        Ok(ParsedArgs::Run(Args {
            apps_bin,
            allowed_diff,
            report,
        }))
    }
    fn env_allowed_diff() -> Result<u32, String> {
        match std::env::var("H2_ALLOWED_DIFF") {
            Ok(v) => parse_u32(&v, "H2_ALLOWED_DIFF"),
            Err(std::env::VarError::NotPresent) => Ok(0),
            Err(e) => Err(format!("failed reading H2_ALLOWED_DIFF: {e}")),
        }
    }
    fn parse_u32(v: &str, name: &str) -> Result<u32, String> {
        v.parse::<u32>()
            .map_err(|e| format!("{name} expects u32, got '{v}': {e}"))
    }

    fn run_case(apps_bin: &Path, def: Def) -> CaseResult {
        match def.scenario {
            Scenario::Cli(case) => {
                let now = run_cli(apps_bin, &case.args).unwrap_or_else(|e| CliNow {
                    code: -1,
                    out: String::new(),
                    err: e,
                });
                let diffs = diff_cli(case.exp, &now);
                CaseResult {
                    id: def.id,
                    kind: "cli",
                    matched: diffs.is_empty(),
                    diffs,
                }
            }
            Scenario::Gateway(case) => {
                let now = run_gw(&case.reqs);
                let diffs = diff_gw(case.exp, &now);
                CaseResult {
                    id: def.id,
                    kind: "gateway",
                    matched: diffs.is_empty(),
                    diffs,
                }
            }
        }
    }

    fn run_cli(apps_bin: &Path, args: &[&str]) -> Result<CliNow, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_root =
            std::env::temp_dir().join(format!("axiom_h2_verify_{}_{}", std::process::id(), stamp));
        let workspace = tmp_root.join("workspace");
        fs::create_dir_all(&workspace)
            .map_err(|e| format!("failed to create h2 temp workspace: {e}"))?;
        let memory = tmp_root.join("memory.db");
        let out = Command::new(apps_bin)
            .env("AXIOM_RUNTIME_MEMORY_PATH", memory.as_os_str())
            .env("AXIOM_RUNTIME_TOOL_WORKSPACE", workspace.as_os_str())
            .args(args)
            .output()
            .map_err(|e| format!("failed to execute apps binary: {e}"))?;
        let _ = fs::remove_dir_all(&tmp_root);
        Ok(CliNow {
            code: out.status.code().unwrap_or(-1),
            out: String::from_utf8(out.stdout).map_err(|e| format!("stdout invalid utf-8: {e}"))?,
            err: String::from_utf8(out.stderr).map_err(|e| format!("stderr invalid utf-8: {e}"))?,
        })
    }
    fn run_gw(reqs: &[Req]) -> GwNow {
        let mut rt = GatewayRuntime::new();
        let mut resp = rt.handle(to_http(reqs[0]));
        for req in &reqs[1..] {
            resp = rt.handle(to_http(*req));
        }
        GwNow {
            status: resp.status_code,
            processed: resp.processed(),
            reject: resp.reject_reason.as_ref().map(reject_name),
            decision: resp.decision.map(decision_name),
            policy: resp.policy_code.map(|c| c.as_str().to_string()),
            revision: resp.state.revision,
            mode: mode_name(resp.state.mode).to_string(),
            facts: resp.state.facts.len(),
            denied: resp.state.denied_count,
            audit: resp.state.audit_count,
            msg: resp.message,
        }
    }
    fn to_http(req: Req) -> HttpBoundaryRequest {
        HttpBoundaryRequest::new(req.method, req.path, req.body, req.source_ip)
    }

    fn diff_cli(exp: CliExp, now: &CliNow) -> Vec<String> {
        let mut d = Vec::new();
        if exp.code != now.code {
            d.push(format!(
                "exit_code expected={} actual={}",
                exp.code, now.code
            ));
        }
        if exp.out_empty && !now.out.trim().is_empty() {
            d.push(String::from("stdout expected empty"));
        }
        if exp.err_empty && !now.err.trim().is_empty() {
            d.push(String::from("stderr expected empty"));
        }
        for s in exp.out_has {
            if !now.out.contains(s) {
                d.push(format!("stdout missing: {s}"));
            }
        }
        for s in exp.err_has {
            if !now.err.contains(s) {
                d.push(format!("stderr missing: {s}"));
            }
        }
        d
    }
    fn diff_gw(exp: GwExp, now: &GwNow) -> Vec<String> {
        let mut d = Vec::new();
        if exp.status != now.status {
            d.push(format!(
                "status expected={} actual={}",
                exp.status, now.status
            ));
        }
        if exp.processed != now.processed {
            d.push(format!(
                "processed expected={} actual={}",
                exp.processed, now.processed
            ));
        }
        if exp.reject.map(str::to_string) != now.reject {
            d.push(format!(
                "reject expected={:?} actual={:?}",
                exp.reject, now.reject
            ));
        }
        if exp.decision.map(str::to_string) != now.decision {
            d.push(format!(
                "decision expected={:?} actual={:?}",
                exp.decision, now.decision
            ));
        }
        if exp.policy.map(str::to_string) != now.policy {
            d.push(format!(
                "policy expected={:?} actual={:?}",
                exp.policy, now.policy
            ));
        }
        if exp.revision != now.revision {
            d.push(format!(
                "revision expected={} actual={}",
                exp.revision, now.revision
            ));
        }
        if exp.mode != now.mode {
            d.push(format!("mode expected={} actual={}", exp.mode, now.mode));
        }
        if exp.facts != now.facts {
            d.push(format!("facts expected={} actual={}", exp.facts, now.facts));
        }
        if exp.denied != now.denied {
            d.push(format!(
                "denied expected={} actual={}",
                exp.denied, now.denied
            ));
        }
        if exp.audit != now.audit {
            d.push(format!("audit expected={} actual={}", exp.audit, now.audit));
        }
        if let Some(s) = exp.msg_has
            && !now.msg.contains(s)
        {
            d.push(format!("message missing: {s}"));
        }
        d
    }

    fn defs() -> Vec<Def> {
        vec![
            cli(
                "cli_status_default",
                &["status"],
                CliExp {
                    code: 0,
                    out_has: &["status revision=0 mode=active facts=0 denied=0 audit=0"],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_health_default",
                &["health"],
                CliExp {
                    code: 0,
                    out_has: &[
                        "health ok=true profile=prod endpoint=http://127.0.0.1:8080 mode=active revision=0",
                    ],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            // ReadFact is short-circuited before the pipeline: no intent id line is emitted.
            cli(
                "cli_read_missing_fact",
                &["read", "alpha"],
                CliExp {
                    code: 0,
                    out_has: &["read key=alpha value=<none>"],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_write_success",
                &["write", "alpha", "42"],
                CliExp {
                    code: 0,
                    out_has: &[
                        "intent id=cli-1 kind=write outcome=accepted policy=allowed effects=1",
                    ],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_freeze_as_alice",
                &["--actor=alice", "freeze"],
                CliExp {
                    code: 0,
                    out_has: &[
                        "intent id=cli-1 kind=freeze outcome=rejected policy=unauthorized_control effects=0",
                    ],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            // ReadFact short-circuit: intent IDs shift down by 1, revision 24→20, audit 6→5.
            cli(
                "cli_batch_pipeline",
                &[
                    "--actor=system",
                    "batch",
                    "write:alpha=1",
                    "read:alpha",
                    "freeze",
                    "write:beta=2",
                    "remove:alpha",
                    "halt",
                ],
                CliExp {
                    code: 0,
                    out_has: &[
                        "intent id=cli-3 kind=write outcome=rejected policy=readonly_mutation effects=0",
                        "intent id=cli-5 kind=halt outcome=accepted policy=allowed effects=1",
                        "batch completed count=6 revision=20 mode=halted facts=1 denied=2 audit=5",
                    ],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_batch_flow",
                &["--actor=system", "batch", "write:key=value", "remove:key"],
                CliExp {
                    code: 0,
                    out_has: &[
                        "intent id=cli-1 kind=write outcome=accepted policy=allowed effects=1",
                        "intent id=cli-2 kind=remove outcome=accepted policy=allowed effects=1",
                        "batch completed count=2 revision=8 mode=active facts=0 denied=0 audit=2",
                    ],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_serve_gateway_mode",
                &["--endpoint=http://gateway.local", "serve", "--mode=gateway"],
                CliExp {
                    code: 0,
                    out_has: &["gateway started profile=prod endpoint=http://gateway.local"],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_serve_daemon_mode",
                &["serve", "--mode=daemon"],
                CliExp {
                    code: 0,
                    out_has: &["daemon started profile=prod endpoint=http://127.0.0.1:8080"],
                    err_has: &[],
                    out_empty: false,
                    err_empty: true,
                },
            ),
            cli(
                "cli_unknown_option",
                &["--allow-dev-in-release", "status"],
                CliExp {
                    code: 2,
                    out_has: &[],
                    err_has: &["unknown option '--allow-dev-in-release'"],
                    out_empty: true,
                    err_empty: false,
                },
            ),
            gw(
                "gateway_write_accepted",
                &[r(
                    gateway::GATEWAY_METHOD,
                    gateway::GATEWAY_PATH,
                    "write:alpha=42",
                    "10.0.0.8",
                )],
                GwExp {
                    status: 202,
                    processed: true,
                    reject: None,
                    decision: Some("accepted"),
                    policy: Some("allowed"),
                    revision: 4,
                    mode: "active",
                    facts: 1,
                    denied: 0,
                    audit: 1,
                    msg_has: None,
                },
            ),
            gw(
                "gateway_read_after_write",
                &[
                    r(
                        gateway::GATEWAY_METHOD,
                        gateway::GATEWAY_PATH,
                        "write:alpha=1",
                        "10.0.0.8",
                    ),
                    r(
                        gateway::GATEWAY_METHOD,
                        gateway::GATEWAY_PATH,
                        "read:alpha",
                        "10.0.0.8",
                    ),
                ],
                GwExp {
                    status: 202,
                    processed: true,
                    reject: None,
                    decision: Some("accepted"),
                    policy: Some("allowed"),
                    revision: 8,
                    mode: "active",
                    facts: 1,
                    denied: 0,
                    audit: 2,
                    msg_has: None,
                },
            ),
            gw(
                "gateway_freeze_unauthorized",
                &[r(
                    gateway::GATEWAY_METHOD,
                    gateway::GATEWAY_PATH,
                    "freeze",
                    "10.0.0.8",
                )],
                GwExp {
                    status: 403,
                    processed: true,
                    reject: None,
                    decision: Some("rejected"),
                    policy: Some("unauthorized_control"),
                    revision: 4,
                    mode: "active",
                    facts: 0,
                    denied: 1,
                    audit: 1,
                    msg_has: Some("control actions require actor `system`"),
                },
            ),
            gw(
                "gateway_freeze_then_write_rejected",
                &[
                    r(
                        gateway::GATEWAY_METHOD,
                        gateway::GATEWAY_PATH,
                        "freeze",
                        "127.0.0.1",
                    ),
                    r(
                        gateway::GATEWAY_METHOD,
                        gateway::GATEWAY_PATH,
                        "write:beta=2",
                        "127.0.0.1",
                    ),
                ],
                GwExp {
                    status: 403,
                    processed: true,
                    reject: None,
                    decision: Some("rejected"),
                    policy: Some("readonly_mutation"),
                    revision: 8,
                    mode: "read_only",
                    facts: 0,
                    denied: 1,
                    audit: 2,
                    msg_has: Some("fact mutations are blocked in read-only mode"),
                },
            ),
            gw(
                "gateway_method_not_allowed",
                &[r(
                    "GET",
                    gateway::GATEWAY_PATH,
                    "write:alpha=1",
                    "127.0.0.1",
                )],
                GwExp {
                    status: 400,
                    processed: false,
                    reject: Some("method_not_allowed"),
                    decision: None,
                    policy: None,
                    revision: 0,
                    mode: "active",
                    facts: 0,
                    denied: 0,
                    audit: 0,
                    msg_has: Some("method is not allowed"),
                },
            ),
            gw(
                "gateway_body_empty",
                &[r(
                    gateway::GATEWAY_METHOD,
                    gateway::GATEWAY_PATH,
                    "   ",
                    "127.0.0.1",
                )],
                GwExp {
                    status: 400,
                    processed: false,
                    reject: Some("body_empty"),
                    decision: None,
                    policy: None,
                    revision: 0,
                    mode: "active",
                    facts: 0,
                    denied: 0,
                    audit: 0,
                    msg_has: Some("body must not be empty"),
                },
            ),
            gw(
                "gateway_body_invalid_intent",
                &[r(
                    gateway::GATEWAY_METHOD,
                    gateway::GATEWAY_PATH,
                    "noop",
                    "127.0.0.1",
                )],
                GwExp {
                    status: 400,
                    processed: false,
                    reject: Some("body_invalid_intent"),
                    decision: None,
                    policy: None,
                    revision: 0,
                    mode: "active",
                    facts: 0,
                    denied: 0,
                    audit: 0,
                    msg_has: Some("body intent is invalid"),
                },
            ),
            gw(
                "gateway_source_ip_invalid",
                &[r(
                    gateway::GATEWAY_METHOD,
                    gateway::GATEWAY_PATH,
                    "write:alpha=1",
                    "bad-ip",
                )],
                GwExp {
                    status: 400,
                    processed: false,
                    reject: Some("source_ip_invalid"),
                    decision: None,
                    policy: None,
                    revision: 0,
                    mode: "active",
                    facts: 0,
                    denied: 0,
                    audit: 0,
                    msg_has: Some("source_ip is invalid"),
                },
            ),
            gw(
                "gateway_source_ip_not_allowed",
                &[r(
                    gateway::GATEWAY_METHOD,
                    gateway::GATEWAY_PATH,
                    "write:alpha=1",
                    "8.8.8.8",
                )],
                GwExp {
                    status: 400,
                    processed: false,
                    reject: Some("source_ip_not_allowed"),
                    decision: None,
                    policy: None,
                    revision: 0,
                    mode: "active",
                    facts: 0,
                    denied: 0,
                    audit: 0,
                    msg_has: Some("source_ip is not allowed"),
                },
            ),
        ]
    }

    fn cli(id: &'static str, args: &[&'static str], exp: CliExp) -> Def {
        Def {
            id,
            scenario: Scenario::Cli(CliCase {
                args: args.to_vec(),
                exp,
            }),
        }
    }
    fn gw(id: &'static str, reqs: &[Req], exp: GwExp) -> Def {
        Def {
            id,
            scenario: Scenario::Gateway(GwCase {
                reqs: reqs.to_vec(),
                exp,
            }),
        }
    }
    const fn r(
        method: &'static str,
        path: &'static str,
        body: &'static str,
        source_ip: &'static str,
    ) -> Req {
        Req {
            method,
            path,
            body,
            source_ip,
        }
    }
    fn reject_name(r: &GatewayRejectReason) -> String {
        String::from(match r {
            GatewayRejectReason::MethodNotAllowed { .. } => "method_not_allowed",
            GatewayRejectReason::PathNotAllowed { .. } => "path_not_allowed",
            GatewayRejectReason::BodyEmpty => "body_empty",
            GatewayRejectReason::BodyContainsNul => "body_contains_nul",
            GatewayRejectReason::BodyTooLarge { .. } => "body_too_large",
            GatewayRejectReason::BodyInvalidIntent { .. } => "body_invalid_intent",
            GatewayRejectReason::SourceIpInvalid { .. } => "source_ip_invalid",
            GatewayRejectReason::SourceIpNotAllowed { .. } => "source_ip_not_allowed",
            GatewayRejectReason::SignatureInvalid => "signature_invalid",
        })
    }
    fn decision_name(v: DecisionOutcome) -> String {
        String::from(match v {
            DecisionOutcome::Accepted => "accepted",
            DecisionOutcome::Rejected => "rejected",
        })
    }

    fn to_json(
        suite: &str,
        apps_bin: &str,
        allowed_diff: u32,
        scenario_count: u32,
        diff_count: u32,
        gate: &str,
        scenarios: &[CaseResult],
    ) -> String {
        let mut out = String::new();
        out.push('{');
        out.push_str("\"suite\":");
        push_json_str(&mut out, suite);
        out.push_str(",\"apps_bin\":");
        push_json_str(&mut out, apps_bin);
        out.push_str(",\"allowed_diff\":");
        out.push_str(&allowed_diff.to_string());
        out.push_str(",\"scenario_count\":");
        out.push_str(&scenario_count.to_string());
        out.push_str(",\"diff_count\":");
        out.push_str(&diff_count.to_string());
        out.push_str(",\"gate\":");
        push_json_str(&mut out, gate);
        out.push_str(",\"scenarios\":[");
        for (i, s) in scenarios.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push('{');
            out.push_str("\"id\":");
            push_json_str(&mut out, s.id);
            out.push_str(",\"kind\":");
            push_json_str(&mut out, s.kind);
            out.push_str(",\"match\":");
            out.push_str(if s.matched { "true" } else { "false" });
            out.push_str(",\"diffs\":[");
            for (j, d) in s.diffs.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                push_json_str(&mut out, d);
            }
            out.push(']');
            out.push('}');
        }
        out.push_str("]}");
        out
    }
    fn push_json_str(out: &mut String, v: &str) {
        out.push('"');
        for ch in v.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ if ch.is_control() => {
                    let _ = write!(out, "\\u{:04x}", ch as u32);
                }
                _ => out.push(ch),
            }
        }
        out.push('"');
    }
}
