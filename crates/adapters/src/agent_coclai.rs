use coclai::{Client, ClientError, PromptRunError};
use std::sync::Mutex;
use tokio::runtime::Runtime;

use crate::contracts::{AdapterHealth, AgentAdapter, AgentRequest, AgentResponse};
use crate::error::{AdapterError, AdapterResult, RetryClass};

pub struct CoclaiAgentAdapter {
    rt: Runtime,
    client: Mutex<Client>,
}

impl CoclaiAgentAdapter {
    pub fn new() -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to create tokio runtime: {e}"))?;

        let client = rt
            .block_on(Client::connect_default())
            .map_err(|e: ClientError| format!("failed to initialize coclai client: {e}"))?;

        Ok(Self {
            rt,
            client: Mutex::new(client),
        })
    }
}

const ID: &str = "coclai";

impl AgentAdapter for CoclaiAgentAdapter {
    fn id(&self) -> &str {
        ID
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy
    }

    fn run(&self, request: AgentRequest) -> AdapterResult<AgentResponse> {
        let client = self
            .client
            .lock()
            .map_err(|_| {
                AdapterError::failed(
                    "coclai.lock",
                    "client lock poisoned",
                    RetryClass::NonRetryable,
                )
            })?
            .clone();

        self.rt.block_on(async move {
            let result =
                client
                    .run(request.cwd, request.prompt)
                    .await
                    .map_err(|e: PromptRunError| {
                        AdapterError::failed("coclai.run", e.to_string(), RetryClass::NonRetryable)
                    });

            Ok(AgentResponse {
                content: result?.assistant_text,
            })
        })
    }
}
