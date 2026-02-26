use axiom_adapters::contracts::{AgentAdapter, ChannelAdapter, ChannelMessage};

use crate::agent_loop::{AgentAction, AgentExecutionContext, AgentResultKind, execute_agent_action};
use crate::estop::EStop;

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
    agent: &dyn AgentAdapter,
    estop: Option<&EStop>,
    poll_interval: std::time::Duration,
    max_polls: Option<u64>,
) -> Result<u64, String> {
    let mut processed = 0u64;
    let mut poll_count = 0u64;

    loop {
        if let Some(e) = estop
            && e.is_stopped() {
                break;
            }
        if let Some(max) = max_polls
            && poll_count >= max {
                break;
            }

        match channel.drain() {
            Ok(messages) => {
                for msg in messages {
                    let input = msg.body.trim().to_owned();
                    if input.is_empty() {
                        continue;
                    }

                    let action = AgentAction {
                        cwd: None,
                        message: Some(input),
                        model: None,
                    };
                    let ctx = AgentExecutionContext {
                        agent,
                        estop,
                        context: None,
                    };

                    match execute_agent_action(action, ctx) {
                        Ok(result) => {
                            let reply = extract_reply(&result.kind);
                            if !reply.is_empty() {
                                let reply_msg = ChannelMessage::new(msg.topic.clone(), reply);
                                if let Err(e) = channel.send(reply_msg) {
                                    eprintln!("channel send error: {e}");
                                }
                            }
                            processed = processed.saturating_add(1);
                        }
                        Err(e) => eprintln!("agent error: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("channel drain error: {e}"),
        }

        std::thread::sleep(poll_interval);
        poll_count = poll_count.saturating_add(1);
    }

    Ok(processed)
}

/// AgentResultKind에서 텍스트 응답을 추출.
fn extract_reply(kind: &AgentResultKind) -> String {
    match kind {
        AgentResultKind::Single { output, .. } => output.clone(),
        AgentResultKind::Interactive { turns } => turns
            .last()
            .map(|t| t.output.clone())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estop::EStop;
    use axiom_adapters::contracts::{
        AdapterHealth, AgentAdapter, AgentRequest, AgentResponse, ChannelAdapter, ChannelMessage,
        ChannelSendReceipt,
    };
    use axiom_adapters::error::AdapterResult;
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

        fn send(&mut self, message: ChannelMessage) -> AdapterResult<ChannelSendReceipt> {
            self.sequence = self.sequence.saturating_add(1);
            self.outbox.push(message);
            Ok(ChannelSendReceipt {
                sequence: self.sequence,
                accepted: true,
            })
        }

        fn drain(&mut self) -> AdapterResult<Vec<ChannelMessage>> {
            let mut drained = Vec::new();
            while let Some(msg) = self.inbox.pop_front() {
                drained.push(msg);
            }
            Ok(drained)
        }
    }

    #[test]
    fn serve_loop_processes_messages_and_sends_replies() {
        let mut channel = MockChannel::new(vec![
            ChannelMessage::new("mock-channel", "hello"),
            ChannelMessage::new("mock-channel", "world"),
        ]);
        let agent = EchoAgent;

        let processed =
            run_channel_serve_loop(&mut channel, &agent, None, Duration::from_millis(0), Some(1))
                .expect("loop should succeed");

        assert_eq!(processed, 2);
        assert_eq!(channel.outbox.len(), 2);
        assert!(channel.outbox[0].body.contains("echo:hello"));
        assert_eq!(channel.outbox[0].topic, "mock-channel");
        assert!(channel.outbox[1].body.contains("echo:world"));
        assert_eq!(channel.outbox[1].topic, "mock-channel");
    }

    #[test]
    fn serve_loop_skips_empty_body() {
        let mut channel = MockChannel::new(vec![
            ChannelMessage::new("mock-channel", "   "),
            ChannelMessage::new("mock-channel", "hi"),
        ]);
        let agent = EchoAgent;

        let processed =
            run_channel_serve_loop(&mut channel, &agent, None, Duration::from_millis(0), Some(1))
                .expect("loop should succeed");

        assert_eq!(processed, 1);
        assert_eq!(channel.outbox.len(), 1);
    }

    #[test]
    fn serve_loop_respects_estop() {
        let mut channel = MockChannel::new(vec![ChannelMessage::new("mock-channel", "hello")]);
        let agent = EchoAgent;
        let estop = EStop::new();
        estop.halt();

        let processed = run_channel_serve_loop(
            &mut channel,
            &agent,
            Some(&estop),
            Duration::from_millis(0),
            None,
        )
        .expect("loop should succeed with estop");

        assert_eq!(processed, 0);
        assert!(channel.outbox.is_empty());
    }

    #[test]
    fn serve_loop_respects_max_polls_zero() {
        let mut channel = MockChannel::new(vec![ChannelMessage::new("mock-channel", "hello")]);
        let agent = EchoAgent;

        let processed =
            run_channel_serve_loop(&mut channel, &agent, None, Duration::from_millis(0), Some(0))
                .expect("loop should succeed with max_polls=0");

        assert_eq!(processed, 0);
    }

    #[test]
    fn extract_reply_from_single_kind() {
        use crate::agent_loop::AgentResultKind;
        let kind = AgentResultKind::Single {
            input: String::from("q"),
            output: String::from("answer"),
        };
        assert_eq!(extract_reply(&kind), "answer");
    }

    #[test]
    fn extract_reply_from_empty_interactive_turns() {
        use crate::agent_loop::AgentResultKind;
        let kind = AgentResultKind::Interactive { turns: vec![] };
        assert_eq!(extract_reply(&kind), "");
    }
}
