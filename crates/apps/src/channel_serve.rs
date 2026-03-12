use axonrunner_adapters::contracts::{
    AgentAdapter, ChannelAdapter, ChannelMessage, ContextAdapter,
};
use std::sync::Arc;

use crate::agent_loop::{
    AgentAction, AgentExecutionContext, AgentResultKind, execute_agent_action,
};
use crate::env_util::read_env_trimmed;
use crate::estop::EStop;
use axonrunner_apps::async_runtime_host::global_async_runtime_host;

const MAX_SEND_RETRIES: u32 = 3;
const ENV_CHANNEL_SERVE_CONCURRENCY: &str = "AXONRUNNER_CHANNEL_SERVE_CONCURRENCY";
const DEFAULT_CHANNEL_SERVE_CONCURRENCY_FALLBACK: usize = 4;
const DEFAULT_CHANNEL_SERVE_CONCURRENCY_CAP: usize = 8;

type SharedAgent = Arc<dyn AgentAdapter>;
type SharedContext = Arc<dyn ContextAdapter>;

#[derive(Debug, Default)]
struct MessageProcessResult {
    processed: bool,
    reply: Option<ChannelMessage>,
    error: Option<String>,
}

struct BlockingBatchExecutor {
    runtime: tokio::runtime::Runtime,
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl BlockingBatchExecutor {
    fn new(max_in_flight: usize) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .map_err(|error| format!("channel serve runtime init failed: {error}"))?;
        Ok(Self {
            runtime,
            semaphore: Arc::new(tokio::sync::Semaphore::new(max_in_flight.max(1))),
        })
    }

    fn run_batch<T, F>(&self, tasks: Vec<F>) -> Result<Vec<T>, String>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let semaphore = Arc::clone(&self.semaphore);
        self.runtime.block_on(async move {
            let mut handles = Vec::with_capacity(tasks.len());
            for task in tasks {
                let semaphore = Arc::clone(&semaphore);
                let handle = tokio::spawn(async move {
                    let permit = semaphore
                        .acquire_owned()
                        .await
                        .map_err(|_| String::from("channel serve semaphore closed"))?;
                    let output = tokio::task::spawn_blocking(task).await.map_err(|error| {
                        format!("channel serve blocking worker join failed: {error}")
                    })?;
                    drop(permit);
                    Ok::<T, String>(output)
                });
                handles.push(handle);
            }

            let mut outputs = Vec::with_capacity(handles.len());
            for handle in handles {
                let output = handle
                    .await
                    .map_err(|error| format!("channel serve task join failed: {error}"))??;
                outputs.push(output);
            }
            Ok(outputs)
        })
    }
}

/// 채널 메시지를 폴링하여 에이전트에게 전달하고 응답을 채널로 전송.
///
/// estop이 활성화되거나 max_polls 횟수가 소진되면 종료.
/// 반환값: 성공적으로 처리된 메시지 수.
///
/// # 보안 참고 사항
/// OTP(TOTP) 게이팅은 CLI 진입점(`execute_agent()`)에만 적용됩니다.
/// 채널 데몬 경로는 채널 플랫폼의 인증(Telegram `allowed_users` 화이트리스트,
/// Discord webhook secret 등)으로 보호됩니다.
pub fn run_channel_serve_loop(
    channel: &mut dyn ChannelAdapter,
    agent: SharedAgent,
    estop: Option<Arc<EStop>>,
    poll_interval: std::time::Duration,
    max_polls: Option<u64>,
    context: Option<SharedContext>,
) -> Result<u64, String> {
    run_channel_serve_loop_with_parallelism(
        channel,
        agent,
        estop,
        poll_interval,
        max_polls,
        context,
        channel_serve_parallelism_from_env(),
    )
}

fn run_channel_serve_loop_with_parallelism(
    channel: &mut dyn ChannelAdapter,
    agent: SharedAgent,
    estop: Option<Arc<EStop>>,
    poll_interval: std::time::Duration,
    max_polls: Option<u64>,
    context: Option<SharedContext>,
    parallelism: usize,
) -> Result<u64, String> {
    let mut processed = 0u64;
    let mut poll_count = 0u64;
    let executor = if parallelism > 1 {
        Some(BlockingBatchExecutor::new(parallelism)?)
    } else {
        None
    };

    loop {
        if let Some(e) = estop.as_deref()
            && e.is_stopped()
        {
            break;
        }
        if let Some(max) = max_polls
            && poll_count >= max
        {
            break;
        }

        match channel_drain(channel) {
            Ok(messages) => {
                let outcomes = if let Some(executor) = executor.as_ref() {
                    process_polled_messages_parallel(
                        executor,
                        messages,
                        Arc::clone(&agent),
                        estop.clone(),
                        context.clone(),
                    )?
                } else {
                    process_polled_messages_serial(
                        messages,
                        agent.as_ref(),
                        estop.as_deref(),
                        context.as_deref(),
                    )
                };

                for outcome in outcomes {
                    if let Some(error) = outcome.error {
                        eprintln!("agent error: {error}");
                        continue;
                    }

                    if let Some(reply_msg) = outcome.reply {
                        send_with_retries(channel, reply_msg);
                    }
                    if outcome.processed {
                        processed = processed.saturating_add(1);
                    }
                }
            }
            Err(error) => eprintln!("channel drain error: {error}"),
        }

        std::thread::sleep(poll_interval);
        poll_count = poll_count.saturating_add(1);
    }

    Ok(processed)
}

fn channel_serve_parallelism_from_env() -> usize {
    read_env_trimmed(ENV_CHANNEL_SERVE_CONCURRENCY)
        .ok()
        .flatten()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or_else(default_channel_serve_concurrency)
}

fn default_channel_serve_concurrency() -> usize {
    std::thread::available_parallelism()
        .ok()
        .map(|value| value.get().min(DEFAULT_CHANNEL_SERVE_CONCURRENCY_CAP))
        .filter(|&value| value > 0)
        .unwrap_or(DEFAULT_CHANNEL_SERVE_CONCURRENCY_FALLBACK)
}

fn process_polled_messages_serial(
    messages: Vec<ChannelMessage>,
    agent: &dyn AgentAdapter,
    estop: Option<&EStop>,
    context: Option<&dyn ContextAdapter>,
) -> Vec<MessageProcessResult> {
    messages
        .into_iter()
        .map(|message| process_single_message(message, agent, estop, context))
        .collect()
}

fn process_polled_messages_parallel(
    executor: &BlockingBatchExecutor,
    messages: Vec<ChannelMessage>,
    agent: SharedAgent,
    estop: Option<Arc<EStop>>,
    context: Option<SharedContext>,
) -> Result<Vec<MessageProcessResult>, String> {
    let tasks = messages
        .into_iter()
        .map(|message| {
            let agent = Arc::clone(&agent);
            let estop = estop.clone();
            let context = context.clone();
            move || {
                process_single_message(
                    message,
                    agent.as_ref(),
                    estop.as_deref(),
                    context.as_deref(),
                )
            }
        })
        .collect::<Vec<_>>();
    executor.run_batch(tasks)
}

fn process_single_message(
    message: ChannelMessage,
    agent: &dyn AgentAdapter,
    estop: Option<&EStop>,
    context: Option<&dyn ContextAdapter>,
) -> MessageProcessResult {
    let input = message.body.trim().to_owned();
    if input.is_empty() {
        return MessageProcessResult::default();
    }

    let action = AgentAction {
        cwd: None,
        message: Some(input),
        model: None,
    };
    let ctx = AgentExecutionContext {
        agent,
        estop,
        context,
    };

    match execute_agent_action(action, ctx) {
        Ok(result) => {
            let reply = extract_reply(&result.kind);
            let reply = if reply.is_empty() {
                None
            } else {
                Some(ChannelMessage::new(message.topic, reply))
            };
            MessageProcessResult {
                processed: true,
                reply,
                error: None,
            }
        }
        Err(error) => MessageProcessResult {
            processed: false,
            reply: None,
            error: Some(error),
        },
    }
}

fn send_with_retries(channel: &mut dyn ChannelAdapter, reply_msg: ChannelMessage) {
    let mut last_err = String::new();
    let mut sent = false;
    for attempt in 0..MAX_SEND_RETRIES {
        match channel_send(channel, reply_msg.clone()) {
            Ok(_) => {
                sent = true;
                break;
            }
            Err(error) => {
                last_err = error.to_string();
                if attempt + 1 < MAX_SEND_RETRIES {
                    std::thread::sleep(std::time::Duration::from_millis(100 * 2u64.pow(attempt)));
                }
            }
        }
    }

    if !sent {
        eprintln!("[channel_serve] send failed after {MAX_SEND_RETRIES} attempts: {last_err}");
    }
}

fn channel_drain(channel: &mut dyn ChannelAdapter) -> Result<Vec<ChannelMessage>, String> {
    global_async_runtime_host().block_on_async("channel_serve.channel_drain", async {
        channel.drain().await.map_err(|error| error.to_string())
    })
}

fn channel_send(channel: &mut dyn ChannelAdapter, message: ChannelMessage) -> Result<(), String> {
    global_async_runtime_host().block_on_async("channel_serve.channel_send", async {
        channel
            .send(message)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
}

/// AgentResultKind에서 텍스트 응답을 추출.
fn extract_reply(kind: &AgentResultKind) -> String {
    match kind {
        AgentResultKind::Single { output, .. } => output.clone(),
        AgentResultKind::Interactive { turns } => {
            turns.last().map(|t| t.output.clone()).unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axonrunner_adapters::contracts::{
        AdapterFuture, AdapterHealth, AgentRequest, AgentResponse, ChannelSendReceipt,
    };
    use axonrunner_adapters::error::AdapterResult;
    use std::collections::VecDeque;
    use std::time::Duration;

    struct EchoAgent;

    impl AgentAdapter for EchoAgent {
        fn id(&self) -> &str {
            "echo-agent"
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

    struct MockChannel {
        inbox: VecDeque<ChannelMessage>,
        outbox: Vec<ChannelMessage>,
        sequence: u64,
    }

    impl MockChannel {
        fn new(messages: Vec<ChannelMessage>) -> Self {
            Self {
                inbox: VecDeque::from(messages),
                outbox: Vec::new(),
                sequence: 0,
            }
        }
    }

    impl ChannelAdapter for MockChannel {
        fn id(&self) -> &str {
            "mock-channel"
        }

        fn health(&self) -> AdapterHealth {
            AdapterHealth::Healthy
        }

        fn send(&mut self, message: ChannelMessage) -> AdapterFuture<'_, ChannelSendReceipt> {
            Box::pin(async move {
                self.sequence = self.sequence.saturating_add(1);
                self.outbox.push(message);
                Ok(ChannelSendReceipt {
                    sequence: self.sequence,
                    accepted: true,
                })
            })
        }

        fn drain(&mut self) -> AdapterFuture<'_, Vec<ChannelMessage>> {
            Box::pin(async move {
                let mut drained = Vec::new();
                while let Some(msg) = self.inbox.pop_front() {
                    drained.push(msg);
                }
                Ok(drained)
            })
        }
    }

    #[test]
    fn serve_loop_processes_messages_and_sends_replies() {
        let mut channel = MockChannel::new(vec![
            ChannelMessage::new("user-123", "hello"),
            ChannelMessage::new("user-456", "world"),
        ]);
        let agent: SharedAgent = Arc::new(EchoAgent);

        let processed = run_channel_serve_loop(
            &mut channel,
            agent,
            None,
            Duration::from_millis(0),
            Some(1),
            None,
        )
        .expect("loop should succeed");

        assert_eq!(processed, 2);
        assert_eq!(channel.outbox.len(), 2);
        assert!(channel.outbox[0].body.contains("echo:hello"));
        assert!(channel.outbox[1].body.contains("echo:world"));
        assert_eq!(channel.outbox[0].topic, "user-123");
        assert_eq!(channel.outbox[1].topic, "user-456");
    }

    #[test]
    fn serve_loop_parallel_path_processes_messages_and_sends_replies() {
        let mut channel = MockChannel::new(vec![
            ChannelMessage::new("chat-111", "ping"),
            ChannelMessage::new("chat-222", "hello"),
            ChannelMessage::new("chat-111", "again"),
        ]);
        let agent: SharedAgent = Arc::new(EchoAgent);

        let processed = run_channel_serve_loop_with_parallelism(
            &mut channel,
            agent,
            None,
            Duration::from_millis(0),
            Some(1),
            None,
            4,
        )
        .expect("parallel loop should succeed");

        assert_eq!(processed, 3);
        assert_eq!(channel.outbox[0].topic, "chat-111");
        assert_eq!(channel.outbox[1].topic, "chat-222");
        assert_eq!(channel.outbox[2].topic, "chat-111");
    }

    #[test]
    fn serve_loop_skips_empty_body() {
        let mut channel = MockChannel::new(vec![
            ChannelMessage::new("user-789", "   "),
            ChannelMessage::new("user-789", "hi"),
        ]);
        let agent: SharedAgent = Arc::new(EchoAgent);

        let processed = run_channel_serve_loop(
            &mut channel,
            agent,
            None,
            Duration::from_millis(0),
            Some(1),
            None,
        )
        .expect("loop should succeed");

        assert_eq!(processed, 1);
        assert_eq!(channel.outbox.len(), 1);
        assert_eq!(channel.outbox[0].topic, "user-789");
    }

    #[test]
    fn serve_loop_respects_estop() {
        let mut channel = MockChannel::new(vec![ChannelMessage::new("mock-channel", "hello")]);
        let agent: SharedAgent = Arc::new(EchoAgent);
        let estop = Arc::new(EStop::new());
        estop.halt();

        let processed = run_channel_serve_loop(
            &mut channel,
            agent,
            Some(estop),
            Duration::from_millis(0),
            None,
            None,
        )
        .expect("loop should succeed with estop");

        assert_eq!(processed, 0);
        assert!(channel.outbox.is_empty());
    }

    #[test]
    fn serve_loop_respects_max_polls_zero() {
        let mut channel = MockChannel::new(vec![ChannelMessage::new("mock-channel", "hello")]);
        let agent: SharedAgent = Arc::new(EchoAgent);

        let processed = run_channel_serve_loop(
            &mut channel,
            agent,
            None,
            Duration::from_millis(0),
            Some(0),
            None,
        )
        .expect("loop should succeed with max_polls=0");

        assert_eq!(processed, 0);
    }

    #[test]
    fn extract_reply_from_single_kind() {
        let kind = AgentResultKind::Single {
            input: String::from("q"),
            output: String::from("answer"),
        };
        assert_eq!(extract_reply(&kind), "answer");
    }

    #[test]
    fn extract_reply_from_empty_interactive_turns() {
        let kind = AgentResultKind::Interactive { turns: vec![] };
        assert_eq!(extract_reply(&kind), "");
    }
}
