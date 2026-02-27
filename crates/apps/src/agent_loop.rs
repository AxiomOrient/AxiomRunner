use std::env;
use std::fmt::Write as FmtWrite;

use axiom_adapters::contracts::ContextAdapter;
use axiom_adapters::{AgentAdapter, AgentRequest};

use crate::parse_util::parse_non_empty;

const ENV_AGENT_SCRIPT: &str = "AXIOM_AGENT_SCRIPT";
const DEFAULT_MODEL: &str = "anthropic/claude-sonnet-4-20250514";
const MAX_INTERACTIVE_TURNS: usize = 16;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentAction {
    pub cwd: Option<String>,
    pub message: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStopReason {
    SingleMessageCompleted,
    ExitCommand,
    IterationLimit,
    EStopActivated,
}

impl AgentStopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentStopReason::SingleMessageCompleted => "single_message_completed",
            AgentStopReason::ExitCommand => "exit_command",
            AgentStopReason::IterationLimit => "iteration_limit",
            AgentStopReason::EStopActivated => "estop_activated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurn {
    pub index: usize,
    pub input: String,
    pub output: String,
    pub stop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResultBase {
    pub agent_id: String,
    pub cwd: String,
    pub model: String,
    pub reason: AgentStopReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentResultKind {
    Single { input: String, output: String },
    Interactive { turns: Vec<AgentTurn> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResult {
    pub base: AgentResultBase,
    pub kind: AgentResultKind,
}

pub struct AgentExecutionContext<'a> {
    pub agent: &'a dyn AgentAdapter,
    pub estop: Option<&'a crate::estop::EStop>,
    pub context: Option<&'a dyn ContextAdapter>,
}

pub fn execute_agent_action(
    action: AgentAction,
    ctx: AgentExecutionContext<'_>,
) -> Result<AgentResult, String> {
    let AgentExecutionContext {
        agent,
        estop,
        context,
    } = ctx;
    let cwd = resolve_cwd(action.cwd)?;
    let model = action
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_MODEL)
        .to_owned();

    if let Some(e) = estop
        && e.is_stopped()
    {
        return Ok(AgentResult {
            base: AgentResultBase {
                agent_id: agent.id().to_owned(),
                cwd,
                model,
                reason: AgentStopReason::EStopActivated,
            },
            kind: AgentResultKind::Interactive { turns: vec![] },
        });
    }

    match action.message {
        Some(message) => {
            let input = parse_non_empty(&message, "agent message")?;
            // RAG: enrich message with semantic memory hits (explicit, isolated I/O)
            let effective_message = if let Some(ctx) = context {
                match ctx.semantic_search(&input, "axiom://agent/memory", 5) {
                    Ok(hits) if !hits.is_empty() => enrich_with_context(&input, &hits),
                    _ => input.clone(), // graceful degradation: proceed without context
                }
            } else {
                input.clone()
            };
            let output = call_agent(agent, &cwd, &effective_message).map_err(|e| e.to_string())?;
            Ok(AgentResult {
                base: AgentResultBase {
                    agent_id: agent.id().to_owned(),
                    cwd,
                    model,
                    reason: AgentStopReason::SingleMessageCompleted,
                },
                kind: AgentResultKind::Single { input, output },
            })
        }
        None => {
            let script = parse_interactive_script_from_env();
            execute_interactive(agent, cwd, model, &script, estop, context)
        }
    }
}

fn execute_interactive(
    agent: &dyn AgentAdapter,
    cwd: String,
    model: String,
    script: &[String],
    estop: Option<&crate::estop::EStop>,
    context: Option<&dyn ContextAdapter>,
) -> Result<AgentResult, String> {
    if script.is_empty() {
        return Err(String::from(
            "interactive agent script is empty; set AXIOM_AGENT_SCRIPT or provide --message",
        ));
    }

    let mut turns = Vec::new();
    let mut reason = AgentStopReason::IterationLimit;

    for (index, raw_input) in script.iter().enumerate() {
        if let Some(e) = estop
            && e.is_stopped()
        {
            reason = AgentStopReason::EStopActivated;
            break;
        }

        if index >= MAX_INTERACTIVE_TURNS {
            break;
        }

        let input = parse_non_empty(raw_input, "agent message")?;
        let stop = is_stop_command(&input);
        // RAG: enrich each turn with semantic memory hits (graceful degradation)
        let effective_message = if !stop {
            if let Some(ctx) = context {
                match ctx.semantic_search(&input, "axiom://agent/memory", 5) {
                    Ok(hits) if !hits.is_empty() => enrich_with_context(&input, &hits),
                    _ => input.clone(),
                }
            } else {
                input.clone()
            }
        } else {
            input.clone()
        };
        let output = if stop {
            String::from("agent loop stopping")
        } else {
            call_agent(agent, &cwd, &effective_message).map_err(|e| e.to_string())?
        };

        turns.push(AgentTurn {
            index: index + 1,
            input,
            output,
            stop,
        });

        if stop {
            reason = AgentStopReason::ExitCommand;
            break;
        }
    }

    Ok(AgentResult {
        base: AgentResultBase {
            agent_id: agent.id().to_owned(),
            cwd,
            model,
            reason,
        },
        kind: AgentResultKind::Interactive { turns },
    })
}

fn call_agent(
    agent: &dyn AgentAdapter,
    cwd: &str,
    prompt: &str,
) -> Result<String, axiom_adapters::AdapterError> {
    let request = AgentRequest::new(cwd, prompt);
    agent.run(request).map(|r| r.content)
}

fn resolve_cwd(cwd: Option<String>) -> Result<String, String> {
    match cwd {
        Some(path) => {
            let path = path.trim().to_owned();
            if path.is_empty() {
                return Err(String::from("agent cwd must not be empty"));
            }
            Ok(path)
        }
        None => std::env::current_dir()
            .map(|p| p.display().to_string())
            .map_err(|e| format!("failed to resolve current directory: {e}")),
    }
}

const DEFAULT_INTERACTIVE_SCRIPT: &[&str] = &["status", "exit"];

fn parse_interactive_script_from_env() -> Vec<String> {
    let from_env = env::var(ENV_AGENT_SCRIPT)
        .ok()
        .map(|value| {
            value
                .split('|')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    if from_env.is_empty() {
        DEFAULT_INTERACTIVE_SCRIPT
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        from_env
    }
}

/// Prepend retrieved context hits to the user message.
/// Pure function: same inputs always produce same output.
/// O(n) where n = sum of snippet lengths.
fn enrich_with_context(message: &str, hits: &[axiom_adapters::contracts::ContextHit]) -> String {
    if hits.is_empty() {
        return message.to_string();
    }
    let capacity = "[RETRIEVED CONTEXT]\n".len()
        + hits
            .iter()
            .map(|h| h.uri.len() + h.snippet.len() + 32)
            .sum::<usize>()
        + "\n[USER MESSAGE]\n".len()
        + message.len();
    let mut enriched = String::with_capacity(capacity);
    enriched.push_str("[RETRIEVED CONTEXT]\n");
    for (i, hit) in hits.iter().enumerate() {
        writeln!(
            enriched,
            "{}. [score={:.3}] {}: {}",
            i + 1,
            hit.score,
            hit.uri,
            hit.snippet
        )
        .ok();
    }
    enriched.push_str("\n[USER MESSAGE]\n");
    enriched.push_str(message);
    enriched
}

const STOP_COMMANDS: &[&str] = &["quit", "q", "halt", "stop", "exit"];

fn is_stop_command(input: &str) -> bool {
    STOP_COMMANDS.iter().any(|&s| input.eq_ignore_ascii_case(s))
}

#[cfg(test)]
mod tests {
    use super::{
        AgentAction, AgentExecutionContext, AgentResultKind, AgentStopReason, execute_agent_action,
        execute_interactive, is_stop_command,
    };
    use crate::estop::EStop;
    use axiom_adapters::contracts::{AdapterHealth, AgentAdapter, AgentRequest, AgentResponse};
    use axiom_adapters::error::AdapterResult;

    struct MockAgent;

    impl AgentAdapter for MockAgent {
        fn id(&self) -> &str {
            "mock-agent"
        }

        fn health(&self) -> AdapterHealth {
            AdapterHealth::Healthy
        }

        fn run(&self, request: AgentRequest) -> AdapterResult<AgentResponse> {
            Ok(AgentResponse {
                content: format!("echo:{}", request.prompt),
            })
        }
    }

    #[test]
    fn agent_single_message_path_calls_adapter() {
        let agent = MockAgent;
        let result = execute_agent_action(
            AgentAction {
                cwd: Some(String::from("/tmp")),
                message: Some(String::from("hello")),
                model: Some(String::from("test-model")),
            },
            AgentExecutionContext {
                agent: &agent,
                estop: None,
                context: None,
            },
        )
        .expect("single path should succeed");

        assert_eq!(result.base.agent_id, "mock-agent");
        assert_eq!(result.base.cwd, "/tmp");
        assert_eq!(result.base.model, "test-model");
        assert_eq!(result.base.reason, AgentStopReason::SingleMessageCompleted);
        match result.kind {
            AgentResultKind::Single { input, output } => {
                assert_eq!(input, "hello");
                assert_eq!(output, "echo:hello");
            }
            _ => panic!("expected single result"),
        }
    }

    #[test]
    fn agent_interactive_path_uses_script_and_stops_on_exit() {
        let agent = MockAgent;
        let script = vec![
            String::from("hello"),
            String::from("status"),
            String::from("exit"),
        ];
        let result = execute_interactive(
            &agent,
            String::from("/tmp"),
            String::from("model"),
            &script,
            None,
            None,
        )
        .expect("interactive path should succeed");

        assert_eq!(result.base.reason, AgentStopReason::ExitCommand);
        match result.kind {
            AgentResultKind::Interactive { turns } => {
                assert_eq!(turns.len(), 3);
                assert_eq!(turns[0].input, "hello");
                assert_eq!(turns[2].input, "exit");
                assert!(turns[2].stop);
            }
            _ => panic!("expected interactive result"),
        }
    }

    #[test]
    fn agent_defaults_cwd_to_current_directory() {
        let agent = MockAgent;
        let result = execute_agent_action(
            AgentAction {
                cwd: None,
                message: Some(String::from("hello")),
                model: None,
            },
            AgentExecutionContext {
                agent: &agent,
                estop: None,
                context: None,
            },
        )
        .expect("should succeed with default cwd");

        assert!(!result.base.cwd.is_empty(), "cwd should be non-empty");
        assert!(matches!(result.kind, AgentResultKind::Single { .. }));
    }

    #[test]
    fn helper_is_stop_command_is_stable() {
        assert!(is_stop_command("quit"));
        assert!(is_stop_command("EXIT"));
        assert!(!is_stop_command("status"));
        assert!(!is_stop_command("hello"));
    }

    #[test]
    fn estop_activated_halts_before_first_turn() {
        let agent = MockAgent;
        let estop = EStop::new();
        estop.halt();
        let result = execute_agent_action(
            AgentAction {
                cwd: Some(String::from("/tmp")),
                message: Some(String::from("hello")),
                model: None,
            },
            AgentExecutionContext {
                agent: &agent,
                estop: Some(&estop),
                context: None,
            },
        )
        .expect("estop path should return Ok");
        assert_eq!(result.base.reason, AgentStopReason::EStopActivated);
    }

    #[test]
    fn estop_activated_halts_interactive_loop() {
        let agent = MockAgent;
        let estop = EStop::new();
        estop.halt();
        let script = vec![String::from("hello"), String::from("status")];
        let result = execute_interactive(
            &agent,
            String::from("/tmp"),
            String::from("model"),
            &script,
            Some(&estop),
            None,
        )
        .expect("estop interactive should return Ok");
        assert_eq!(result.base.reason, AgentStopReason::EStopActivated);
        match result.kind {
            AgentResultKind::Interactive { turns } => {
                assert!(turns.is_empty(), "no turns should have executed");
            }
            _ => panic!("expected interactive result"),
        }
    }

    #[test]
    fn enrich_with_context_empty_hits_returns_original() {
        let msg = "what is the answer?";
        let result = super::enrich_with_context(msg, &[]);
        assert_eq!(result, msg);
    }

    #[test]
    fn enrich_with_context_prepends_hits() {
        use axiom_adapters::contracts::ContextHit;
        let hits = vec![ContextHit {
            uri: "axiom://agent/memory/test".to_string(),
            score: 0.95,
            snippet: "relevant fact".to_string(),
            content: String::new(),
        }];
        let result = super::enrich_with_context("my question", &hits);
        assert!(result.contains("RETRIEVED CONTEXT"));
        assert!(result.contains("0.950"));
        assert!(result.contains("relevant fact"));
        assert!(result.contains("my question"));
    }

    #[test]
    fn execute_agent_action_with_none_context_works() {
        let agent = MockAgent;
        let result = execute_agent_action(
            AgentAction {
                cwd: Some(String::from("/tmp")),
                message: Some(String::from("hello context none")),
                model: Some(String::from("test-model")),
            },
            AgentExecutionContext {
                agent: &agent,
                estop: None,
                context: None,
            },
        )
        .expect("none context should succeed");

        assert_eq!(result.base.reason, AgentStopReason::SingleMessageCompleted);
        match result.kind {
            AgentResultKind::Single { input, output } => {
                assert_eq!(input, "hello context none");
                assert_eq!(output, "echo:hello context none");
            }
            _ => panic!("expected single result"),
        }
    }
}
