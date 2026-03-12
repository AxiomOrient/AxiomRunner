use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axonrunner_apps::daemon::{
    DaemonConfig, DaemonLoop, DaemonState, ItemOutcome, RetryClass, Sleeper, StopReason, WorkError,
    WorkExecutor, WorkItem,
};

struct SequenceExecutor {
    steps: VecDeque<Result<(), WorkError>>,
    calls: Vec<(String, u32)>,
}

impl SequenceExecutor {
    fn new(steps: Vec<Result<(), WorkError>>) -> Self {
        Self {
            steps: steps.into(),
            calls: Vec::new(),
        }
    }
}

impl WorkExecutor for SequenceExecutor {
    fn execute(&mut self, item: &WorkItem, attempt: u32) -> Result<(), WorkError> {
        self.calls.push((item.id.clone(), attempt));
        self.steps.pop_front().unwrap_or(Ok(()))
    }
}

#[derive(Default)]
struct RecordingSleeper {
    durations: Vec<Duration>,
}

impl Sleeper for RecordingSleeper {
    fn sleep(&mut self, duration: Duration) {
        self.durations.push(duration);
    }
}

fn health_path(test_name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "axonrunner-daemon-{test_name}-{}-{stamp}.health",
        std::process::id()
    ))
}

#[test]
fn failure_injection_leads_to_retry_with_backoff() {
    let health = health_path("retry-backoff");
    let mut daemon = DaemonLoop::new(
        DaemonConfig::new(&health, vec![Duration::from_millis(25)]),
        vec![WorkItem::new("work-a")],
    );
    let mut executor = SequenceExecutor::new(vec![Err(WorkError::retryable("transient")), Ok(())]);
    let mut sleeper = RecordingSleeper::default();

    let report = daemon
        .run_until(&mut executor, &mut sleeper, |_| false)
        .expect("daemon loop should run");

    assert_eq!(
        executor.calls,
        vec![(String::from("work-a"), 1), (String::from("work-a"), 2)]
    );
    assert_eq!(sleeper.durations, vec![Duration::from_millis(25)]);
    assert_eq!(report.stop_reason, StopReason::Exhausted);
    assert_eq!(report.completed, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(
        daemon.last_outcome(),
        Some(&ItemOutcome::Succeeded {
            item_id: String::from("work-a"),
            attempt: 2
        })
    );

    let _ = fs::remove_file(health);
}

#[test]
fn non_retryable_failure_halts_work_item() {
    let health = health_path("non-retryable");
    let mut daemon = DaemonLoop::new(
        DaemonConfig::new(&health, vec![Duration::from_millis(10)]),
        vec![WorkItem::new("work-b")],
    );
    let mut executor = SequenceExecutor::new(vec![Err(WorkError::non_retryable("invalid"))]);
    let mut sleeper = RecordingSleeper::default();

    let report = daemon
        .run_until(&mut executor, &mut sleeper, |_| false)
        .expect("daemon loop should run");

    assert_eq!(executor.calls, vec![(String::from("work-b"), 1)]);
    assert!(sleeper.durations.is_empty());
    assert_eq!(report.completed, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(
        daemon.last_outcome(),
        Some(&ItemOutcome::Failed {
            item_id: String::from("work-b"),
            attempt: 1,
            class: RetryClass::NonRetryable
        })
    );

    let _ = fs::remove_file(health);
}

#[test]
fn health_file_is_updated_during_loop() {
    let health = health_path("health");
    let mut daemon = DaemonLoop::new(
        DaemonConfig::new(&health, vec![Duration::from_millis(15)]),
        vec![WorkItem::new("work-c")],
    );
    let mut executor = SequenceExecutor::new(vec![Err(WorkError::retryable("try-again"))]);
    let mut sleeper = RecordingSleeper::default();

    daemon
        .tick(&mut executor, &mut sleeper)
        .expect("first tick should succeed");
    let first = fs::read_to_string(&health).expect("first health write should exist");
    assert!(first.contains("tick=1"));
    assert!(first.contains("state=backing_off"));

    daemon
        .tick(&mut executor, &mut sleeper)
        .expect("second tick should succeed");
    let second = fs::read_to_string(&health).expect("second health write should exist");
    assert!(second.contains("tick=2"));
    assert!(second.contains("state=running"));
    assert_ne!(first, second);

    let _ = fs::remove_file(health);
}

#[test]
fn stop_condition_exits_loop_deterministically() {
    let health = health_path("stop");
    let mut daemon = DaemonLoop::new(
        DaemonConfig::new(&health, vec![Duration::from_millis(10)]),
        vec![WorkItem::new("work-d"), WorkItem::new("work-e")],
    );
    let mut executor = SequenceExecutor::new(vec![Ok(()), Ok(())]);
    let mut sleeper = RecordingSleeper::default();

    let report = daemon
        .run_until(&mut executor, &mut sleeper, |loop_ref| {
            loop_ref.tick_count() >= 1
        })
        .expect("daemon loop should run");

    assert_eq!(report.stop_reason, StopReason::StopRequested);
    assert_eq!(report.ticks, 1);
    assert_eq!(executor.calls, vec![(String::from("work-d"), 1)]);
    assert_eq!(daemon.state(), &DaemonState::Stopped);

    let health_contents = fs::read_to_string(daemon.health_path())
        .expect("health status should be persisted on stop");
    assert!(health_contents.contains("state=stopped"));

    let _ = fs::remove_file(health);
}
