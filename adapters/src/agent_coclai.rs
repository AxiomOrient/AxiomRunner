use coclai::{Client, ClientError, PromptRunError};
use tokio::runtime::Runtime;

use crate::contracts::{AdapterHealth, AgentAdapter, AgentRequest, AgentResponse};
use crate::error::{AdapterError, AdapterResult, RetryClass};

pub struct CoclaiAgentAdapter {
    rt: Runtime,
}

impl CoclaiAgentAdapter {
    pub fn new() -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to create tokio runtime: {e}"))?;
        Ok(Self { rt })
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
        self.rt.block_on(async move {
            let client = Client::connect_default().await.map_err(|e: ClientError| {
                AdapterError::failed("coclai.connect", e.to_string(), RetryClass::Retryable)
            })?;

            let result = client.run(request.cwd, request.prompt).await.map_err(|e: PromptRunError| {
                AdapterError::failed("coclai.run", e.to_string(), RetryClass::NonRetryable)
            });

            let _ = client.shutdown().await;

            Ok(AgentResponse {
                content: result?.assistant_text,
            })
        })
    }
}
