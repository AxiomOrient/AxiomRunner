use std::collections::VecDeque;
use std::time::Duration;

use axonrunner_apps::daemon::{
    SupervisorComponentKind, SupervisorComponentSpec, SupervisorComponentStatus, SupervisorError,
    SupervisorSleeper, run_supervisor_cycle,
};

#[derive(Debug, Default)]
struct RecordingSleeper {
    durations: Vec<Duration>,
}

impl SupervisorSleeper for RecordingSleeper {
    fn sleep(&mut self, duration: Duration) {
        self.durations.push(duration);
    }
}

#[test]
fn supervisor_cycle_retries_with_backoff_then_recovers() {
    let components = vec![SupervisorComponentSpec::new(
        "gateway",
        SupervisorComponentKind::Gateway,
        vec![Duration::from_millis(25), Duration::from_millis(50)],
    )];
    let mut steps: VecDeque<Result<(), SupervisorError>> = vec![
        Err(SupervisorError::retryable("transient-one")),
        Err(SupervisorError::retryable("transient-two")),
        Ok(()),
    ]
    .into();
    let mut calls: Vec<(String, u32)> = Vec::new();
    let mut runner = |component: &SupervisorComponentSpec, attempt: u32| {
        calls.push((component.name.to_string(), attempt));
        steps.pop_front().unwrap_or(Ok(()))
    };
    let mut sleeper = RecordingSleeper::default();

    let report = run_supervisor_cycle(&components, &mut runner, &mut sleeper);
    let component = &report.components[0];

    assert_eq!(
        calls,
        vec![
            (String::from("gateway"), 1),
            (String::from("gateway"), 2),
            (String::from("gateway"), 3)
        ]
    );
    assert_eq!(
        sleeper.durations,
        vec![Duration::from_millis(25), Duration::from_millis(50)]
    );
    assert_eq!(report.total_restarts, 2);
    assert_eq!(report.failed_components, 0);
    assert_eq!(component.attempts, 3);
    assert_eq!(component.restart_count, 2);
    assert_eq!(component.status, SupervisorComponentStatus::Healthy);
}

#[test]
fn supervisor_cycle_fails_after_backoff_budget_exhaustion() {
    let components = vec![SupervisorComponentSpec::new(
        "scheduler",
        SupervisorComponentKind::Scheduler,
        vec![Duration::from_millis(40)],
    )];
    let mut steps: VecDeque<Result<(), SupervisorError>> = vec![
        Err(SupervisorError::retryable("first")),
        Err(SupervisorError::retryable("second")),
    ]
    .into();
    let mut calls: Vec<(String, u32)> = Vec::new();
    let mut runner = |component: &SupervisorComponentSpec, attempt: u32| {
        calls.push((component.name.to_string(), attempt));
        steps.pop_front().unwrap_or(Ok(()))
    };
    let mut sleeper = RecordingSleeper::default();

    let report = run_supervisor_cycle(&components, &mut runner, &mut sleeper);
    let component = &report.components[0];

    assert_eq!(
        calls,
        vec![
            (String::from("scheduler"), 1),
            (String::from("scheduler"), 2)
        ]
    );
    assert_eq!(sleeper.durations, vec![Duration::from_millis(40)]);
    assert_eq!(report.total_restarts, 1);
    assert_eq!(report.failed_components, 1);
    assert_eq!(component.status, SupervisorComponentStatus::Failed);
    assert_eq!(component.attempts, 2);
    assert_eq!(component.restart_count, 1);
    assert_eq!(component.last_error.as_deref(), Some("second"));
}
