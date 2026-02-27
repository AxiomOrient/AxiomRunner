use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupervisorComponentKind {
    Gateway,
    Channels,
    Scheduler,
    Heartbeat,
}

impl SupervisorComponentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SupervisorComponentKind::Gateway => "gateway",
            SupervisorComponentKind::Channels => "channels",
            SupervisorComponentKind::Scheduler => "scheduler",
            SupervisorComponentKind::Heartbeat => "heartbeat",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorComponentSpec {
    pub name: &'static str,
    pub kind: SupervisorComponentKind,
    pub retry_backoff: Vec<Duration>,
}

impl SupervisorComponentSpec {
    pub fn new(
        name: &'static str,
        kind: SupervisorComponentKind,
        retry_backoff: Vec<Duration>,
    ) -> Self {
        Self {
            name,
            kind,
            retry_backoff,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorComponentStatus {
    Healthy,
    BackingOff,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorComponentReport {
    pub name: &'static str,
    pub kind: SupervisorComponentKind,
    pub attempts: u32,
    pub restart_count: u32,
    pub status: SupervisorComponentStatus,
    pub last_error: Option<String>,
    pub last_backoff: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorRunReport {
    pub components: Vec<SupervisorComponentReport>,
    pub total_restarts: u32,
    pub failed_components: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorError {
    Retryable(String),
    Terminal(String),
}

impl SupervisorError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable(message.into())
    }

    pub fn terminal(message: impl Into<String>) -> Self {
        Self::Terminal(message.into())
    }

    fn message(&self) -> &str {
        match self {
            SupervisorError::Retryable(message) | SupervisorError::Terminal(message) => message,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorStepDecision {
    Succeeded,
    Retry { delay: Duration, next_attempt: u32 },
    FailedTerminal,
    FailedExhausted,
}

pub fn decide_supervisor_step(
    attempt: u32,
    retry_backoff: &[Duration],
    result: &Result<(), SupervisorError>,
) -> SupervisorStepDecision {
    match result {
        Ok(()) => SupervisorStepDecision::Succeeded,
        Err(SupervisorError::Terminal(_)) => SupervisorStepDecision::FailedTerminal,
        Err(SupervisorError::Retryable(_)) => {
            let index = attempt.saturating_sub(1) as usize;
            match retry_backoff.get(index).copied() {
                Some(delay) => SupervisorStepDecision::Retry {
                    delay,
                    next_attempt: attempt.saturating_add(1),
                },
                None => SupervisorStepDecision::FailedExhausted,
            }
        }
    }
}

pub trait SupervisorSleeper {
    fn sleep(&mut self, duration: Duration);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSupervisorSleeper;

impl SupervisorSleeper for NoopSupervisorSleeper {
    fn sleep(&mut self, _duration: Duration) {}
}

pub struct StdSupervisorSleeper;

impl SupervisorSleeper for StdSupervisorSleeper {
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

pub fn default_supervisor_components(retry_backoff: &[Duration]) -> Vec<SupervisorComponentSpec> {
    vec![
        SupervisorComponentSpec::new(
            "gateway",
            SupervisorComponentKind::Gateway,
            retry_backoff.to_vec(),
        ),
        SupervisorComponentSpec::new(
            "channels",
            SupervisorComponentKind::Channels,
            retry_backoff.to_vec(),
        ),
        SupervisorComponentSpec::new(
            "scheduler",
            SupervisorComponentKind::Scheduler,
            retry_backoff.to_vec(),
        ),
        SupervisorComponentSpec::new(
            "heartbeat",
            SupervisorComponentKind::Heartbeat,
            retry_backoff.to_vec(),
        ),
    ]
}

pub fn run_supervisor_cycle<R, S>(
    components: &[SupervisorComponentSpec],
    runner: &mut R,
    sleeper: &mut S,
) -> SupervisorRunReport
where
    R: FnMut(&SupervisorComponentSpec, u32) -> Result<(), SupervisorError>,
    S: SupervisorSleeper,
{
    let mut reports = Vec::with_capacity(components.len());
    let mut total_restarts = 0_u32;
    let mut failed_components = 0_usize;

    for component in components {
        let mut attempt = 1_u32;
        let mut restart_count = 0_u32;
        let mut last_backoff: Option<Duration> = None;

        let component_report = loop {
            let result = runner(component, attempt);
            let decision = decide_supervisor_step(attempt, &component.retry_backoff, &result);

            match decision {
                SupervisorStepDecision::Succeeded => {
                    break SupervisorComponentReport {
                        name: component.name,
                        kind: component.kind,
                        attempts: attempt,
                        restart_count,
                        status: SupervisorComponentStatus::Healthy,
                        last_error: None,
                        last_backoff,
                    };
                }
                SupervisorStepDecision::Retry {
                    delay,
                    next_attempt,
                } => {
                    restart_count = restart_count.saturating_add(1);
                    last_backoff = Some(delay);
                    sleeper.sleep(delay);
                    attempt = next_attempt;
                }
                SupervisorStepDecision::FailedTerminal
                | SupervisorStepDecision::FailedExhausted => {
                    failed_components = failed_components.saturating_add(1);
                    break SupervisorComponentReport {
                        name: component.name,
                        kind: component.kind,
                        attempts: attempt,
                        restart_count,
                        status: SupervisorComponentStatus::Failed,
                        last_error: result.err().map(|error| error.message().to_owned()),
                        last_backoff,
                    };
                }
            }
        };

        total_restarts = total_restarts.saturating_add(restart_count);
        reports.push(component_report);
    }

    SupervisorRunReport {
        components: reports,
        total_restarts,
        failed_components,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NoopSupervisorSleeper, SupervisorComponentKind, SupervisorComponentSpec,
        SupervisorComponentStatus, SupervisorError, SupervisorStepDecision, decide_supervisor_step,
        run_supervisor_cycle,
    };
    use std::collections::VecDeque;
    use std::time::Duration;

    #[test]
    fn decide_step_returns_retry_for_retryable_with_backoff() {
        let decision = decide_supervisor_step(
            1,
            &[Duration::from_millis(10)],
            &Err(SupervisorError::retryable("transient")),
        );
        assert_eq!(
            decision,
            SupervisorStepDecision::Retry {
                delay: Duration::from_millis(10),
                next_attempt: 2
            }
        );
    }

    #[test]
    fn decide_step_returns_failed_exhausted_when_backoff_missing() {
        let decision = decide_supervisor_step(
            2,
            &[Duration::from_millis(10)],
            &Err(SupervisorError::retryable("still failing")),
        );
        assert_eq!(decision, SupervisorStepDecision::FailedExhausted);
    }

    #[test]
    fn supervisor_cycle_retries_and_recovers_component() {
        let components = vec![SupervisorComponentSpec::new(
            "gateway",
            SupervisorComponentKind::Gateway,
            vec![Duration::from_millis(5), Duration::from_millis(10)],
        )];
        let mut steps: VecDeque<Result<(), SupervisorError>> = vec![
            Err(SupervisorError::retryable("first")),
            Err(SupervisorError::retryable("second")),
            Ok(()),
        ]
        .into();
        let mut runner = |_component: &SupervisorComponentSpec, _attempt: u32| {
            steps.pop_front().unwrap_or(Ok(()))
        };
        let mut sleeper = RecordingSleeper::default();

        let report = run_supervisor_cycle(&components, &mut runner, &mut sleeper);
        let component = &report.components[0];

        assert_eq!(report.total_restarts, 2);
        assert_eq!(report.failed_components, 0);
        assert_eq!(component.attempts, 3);
        assert_eq!(component.restart_count, 2);
        assert_eq!(component.status, SupervisorComponentStatus::Healthy);
        assert_eq!(
            sleeper.durations,
            vec![Duration::from_millis(5), Duration::from_millis(10)]
        );
    }

    #[test]
    fn supervisor_cycle_marks_failure_when_retry_budget_exhausted() {
        let components = vec![SupervisorComponentSpec::new(
            "scheduler",
            SupervisorComponentKind::Scheduler,
            vec![Duration::from_millis(7)],
        )];
        let mut steps: VecDeque<Result<(), SupervisorError>> = vec![
            Err(SupervisorError::retryable("first")),
            Err(SupervisorError::retryable("second")),
        ]
        .into();
        let mut runner = |_component: &SupervisorComponentSpec, _attempt: u32| {
            steps.pop_front().unwrap_or(Ok(()))
        };
        let mut sleeper = RecordingSleeper::default();

        let report = run_supervisor_cycle(&components, &mut runner, &mut sleeper);
        let component = &report.components[0];

        assert_eq!(report.total_restarts, 1);
        assert_eq!(report.failed_components, 1);
        assert_eq!(component.attempts, 2);
        assert_eq!(component.restart_count, 1);
        assert_eq!(component.status, SupervisorComponentStatus::Failed);
        assert_eq!(component.last_error.as_deref(), Some("second"));
        assert_eq!(sleeper.durations, vec![Duration::from_millis(7)]);
    }

    #[derive(Debug, Default)]
    struct RecordingSleeper {
        durations: Vec<Duration>,
    }

    impl super::SupervisorSleeper for RecordingSleeper {
        fn sleep(&mut self, duration: Duration) {
            self.durations.push(duration);
        }
    }

    #[test]
    fn noop_sleeper_is_noop() {
        let mut sleeper = NoopSupervisorSleeper;
        super::SupervisorSleeper::sleep(&mut sleeper, Duration::from_millis(3));
    }
}
