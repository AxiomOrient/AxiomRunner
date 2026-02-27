use crate::agent_coclai::CoclaiAgentAdapter;
use crate::contracts::{
    AdapterFuture, AdapterHealth, AgentAdapter, AgentRequest, ToolAdapter, ToolCall, ToolOutput,
};
use crate::error::{AdapterError, RetryClass};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const DEFAULT_MAX_DELEGATE_DEPTH: usize = 3;

pub struct DelegateToolAdapter {
    agent: Box<dyn AgentAdapter>,
    max_depth: usize,
    current_depth: AtomicUsize,
}

impl DelegateToolAdapter {
    /// Production constructor: uses real CoclaiAgentAdapter.
    pub fn new(max_depth: usize) -> Result<Self, String> {
        let agent = CoclaiAgentAdapter::new()?;
        Ok(Self {
            agent: Box::new(agent),
            max_depth,
            current_depth: AtomicUsize::new(0),
        })
    }

    /// Low-level constructor for explicit agent injection (e.g., depth chaining).
    pub fn with_agent(
        agent: Box<dyn AgentAdapter>,
        max_depth: usize,
        current_depth: usize,
    ) -> Self {
        Self {
            agent,
            max_depth,
            current_depth: AtomicUsize::new(current_depth),
        }
    }
}

impl ToolAdapter for DelegateToolAdapter {
    fn id(&self) -> &str {
        "delegate"
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn execute(&self, call: ToolCall) -> AdapterFuture<'_, ToolOutput> {
        Box::pin(async move {
            let depth = self.current_depth.fetch_add(1, Ordering::SeqCst);
            if depth >= self.max_depth {
                self.current_depth.fetch_sub(1, Ordering::SeqCst);
                return Err(AdapterError::invalid_input(
                    "delegate",
                    "max depth exceeded",
                ));
            }

            let task = call
                .args
                .get("task")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AdapterError::invalid_input("task", "must not be empty"));

            let task = match task {
                Ok(t) => t,
                Err(e) => {
                    self.current_depth.fetch_sub(1, Ordering::SeqCst);
                    return Err(e);
                }
            };

            let request = AgentRequest::new(".", task);
            let result = self.agent.run(request).map_err(|e| {
                AdapterError::failed("delegate_run", e.to_string(), RetryClass::NonRetryable)
            });

            self.current_depth.fetch_sub(1, Ordering::SeqCst);

            Ok(ToolOutput {
                content: result?.content,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AdapterResult;
    use std::collections::BTreeMap;
    use std::future::Future;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should initialize")
            .block_on(future)
    }

    fn call_with_task(task: &str) -> ToolCall {
        let mut args = BTreeMap::new();
        if !task.is_empty() {
            args.insert("task".to_string(), task.to_string());
        }
        ToolCall::new("delegate", args)
    }

    #[test]
    fn delegate_rejects_when_max_depth_reached() {
        // Depth check happens before agent.run() — no network needed.
        // Use with_agent + a stub only for the depth test path.
        struct NeverCalledAgent;
        impl AgentAdapter for NeverCalledAgent {
            fn id(&self) -> &str {
                "never"
            }
            fn health(&self) -> crate::contracts::AdapterHealth {
                crate::contracts::AdapterHealth::Healthy
            }
            fn run(&self, _: AgentRequest) -> AdapterResult<crate::contracts::AgentResponse> {
                panic!("should not be called")
            }
        }
        let adapter = DelegateToolAdapter::with_agent(Box::new(NeverCalledAgent), 0, 0);
        let err = block_on(adapter.execute(call_with_task("do something")))
            .expect_err("should fail at max depth");
        assert!(matches!(
            err,
            AdapterError::InvalidInput {
                field: "delegate",
                ..
            }
        ));
    }

    #[test]
    fn delegate_rejects_empty_task() {
        // Validation happens before agent.run() — no network needed.
        struct NeverCalledAgent;
        impl AgentAdapter for NeverCalledAgent {
            fn id(&self) -> &str {
                "never"
            }
            fn health(&self) -> crate::contracts::AdapterHealth {
                crate::contracts::AdapterHealth::Healthy
            }
            fn run(&self, _: AgentRequest) -> AdapterResult<crate::contracts::AgentResponse> {
                panic!("should not be called")
            }
        }
        let adapter = DelegateToolAdapter::with_agent(Box::new(NeverCalledAgent), 3, 0);
        let err =
            block_on(adapter.execute(call_with_task(""))).expect_err("empty task should fail");
        assert!(matches!(
            err,
            AdapterError::InvalidInput { field: "task", .. }
        ));
    }

    #[test]
    fn delegate_rejects_missing_task_arg() {
        struct NeverCalledAgent;
        impl AgentAdapter for NeverCalledAgent {
            fn id(&self) -> &str {
                "never"
            }
            fn health(&self) -> crate::contracts::AdapterHealth {
                crate::contracts::AdapterHealth::Healthy
            }
            fn run(&self, _: AgentRequest) -> AdapterResult<crate::contracts::AgentResponse> {
                panic!("should not be called")
            }
        }
        let adapter = DelegateToolAdapter::with_agent(Box::new(NeverCalledAgent), 3, 0);
        let err = block_on(adapter.execute(ToolCall::new("delegate", BTreeMap::new())))
            .expect_err("missing task arg should fail");
        assert!(matches!(
            err,
            AdapterError::InvalidInput { field: "task", .. }
        ));
    }

    #[test]
    fn delegate_at_exact_max_depth_fails() {
        struct NeverCalledAgent;
        impl AgentAdapter for NeverCalledAgent {
            fn id(&self) -> &str {
                "never"
            }
            fn health(&self) -> crate::contracts::AdapterHealth {
                crate::contracts::AdapterHealth::Healthy
            }
            fn run(&self, _: AgentRequest) -> AdapterResult<crate::contracts::AgentResponse> {
                panic!("should not be called")
            }
        }
        let adapter = DelegateToolAdapter::with_agent(Box::new(NeverCalledAgent), 3, 3);
        let err = block_on(adapter.execute(call_with_task("some task")))
            .expect_err("at max depth should fail");
        assert!(matches!(
            err,
            AdapterError::InvalidInput {
                field: "delegate",
                ..
            }
        ));
    }

    #[test]
    fn delegate_depth_resets_after_error() {
        // After a depth-exceeded error, the counter must be decremented back.
        struct NeverCalledAgent;
        impl AgentAdapter for NeverCalledAgent {
            fn id(&self) -> &str {
                "never"
            }
            fn health(&self) -> crate::contracts::AdapterHealth {
                crate::contracts::AdapterHealth::Healthy
            }
            fn run(&self, _: AgentRequest) -> AdapterResult<crate::contracts::AgentResponse> {
                panic!("should not be called")
            }
        }
        let adapter = DelegateToolAdapter::with_agent(Box::new(NeverCalledAgent), 1, 1);
        let _ = block_on(adapter.execute(call_with_task("x")));
        // depth should be back to 1, not 2
        assert_eq!(adapter.current_depth.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn delegate_depth_resets_after_validation_error() {
        // After an empty-task error, the counter must be decremented back.
        struct NeverCalledAgent;
        impl AgentAdapter for NeverCalledAgent {
            fn id(&self) -> &str {
                "never"
            }
            fn health(&self) -> crate::contracts::AdapterHealth {
                crate::contracts::AdapterHealth::Healthy
            }
            fn run(&self, _: AgentRequest) -> AdapterResult<crate::contracts::AgentResponse> {
                panic!("should not be called")
            }
        }
        let adapter = DelegateToolAdapter::with_agent(Box::new(NeverCalledAgent), 3, 0);
        let _ = block_on(adapter.execute(call_with_task("")));
        assert_eq!(adapter.current_depth.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn delegate_id_is_delegate() {
        // Just check id — no agent call needed.
        // new() creates a tokio runtime but does not connect.
        let adapter = DelegateToolAdapter::new(3).expect("should build");
        assert_eq!(adapter.id(), "delegate");
    }

    /// Requires running codex CLI. Run with: cargo test -- --ignored
    #[test]
    #[ignore]
    fn delegate_live_run() {
        if std::env::var_os("AXIOM_RUN_DELEGATE_LIVE").is_none() {
            eprintln!("skipping delegate_live_run: set AXIOM_RUN_DELEGATE_LIVE=1 to enable");
            return;
        }
        let adapter = DelegateToolAdapter::new(3).expect("should build");
        let result = block_on(adapter.execute(call_with_task("echo hello")));
        assert!(result.is_ok());
    }
}
