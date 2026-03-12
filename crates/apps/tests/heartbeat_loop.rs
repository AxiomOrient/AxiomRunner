use std::collections::VecDeque;

use axonrunner_apps::heartbeat::{
    HeartbeatConfig, HeartbeatExecutor, HeartbeatLoop, HeartbeatReplay, HeartbeatStopReason,
    parse_heartbeat_tasks,
};

#[derive(Debug, Default)]
struct RecordingExecutor {
    calls: VecDeque<(u64, String)>,
}

impl HeartbeatExecutor for RecordingExecutor {
    fn replay(&mut self, replay: &HeartbeatReplay) -> Result<(), String> {
        self.calls.push_back((replay.tick, replay.task_id.clone()));
        Ok(())
    }
}

#[test]
fn heartbeat_loop_replays_tasks_periodically() {
    let config = HeartbeatConfig::new("/tmp/heartbeat.md", 2, 5);
    let tasks = parse_heartbeat_tasks("- first\n- second");
    let mut loop_ = HeartbeatLoop::new(config, tasks);
    let mut executor = RecordingExecutor::default();

    let report = loop_.run_until(&mut executor, |_| false);

    assert_eq!(report.stop_reason, HeartbeatStopReason::Exhausted);
    assert_eq!(report.ticks, 5);
    assert_eq!(report.replayed, 4);
    assert_eq!(report.failed, 0);
    assert_eq!(report.last_replay_tick, Some(4));
    assert_eq!(
        executor.calls.into_iter().collect::<Vec<(u64, String)>>(),
        vec![
            (2, String::from("task-1")),
            (2, String::from("task-2")),
            (4, String::from("task-1")),
            (4, String::from("task-2")),
        ]
    );
}

#[test]
fn heartbeat_loop_obeys_stop_condition() {
    let config = HeartbeatConfig::new("/tmp/heartbeat.md", 1, 10);
    let tasks = parse_heartbeat_tasks("- only");
    let mut loop_ = HeartbeatLoop::new(config, tasks);
    let mut executor = RecordingExecutor::default();

    let report = loop_.run_until(&mut executor, |loop_ref| loop_ref.tick_count() >= 3);

    assert_eq!(report.stop_reason, HeartbeatStopReason::StopRequested);
    assert_eq!(report.ticks, 3);
    assert_eq!(report.replayed, 3);
    assert_eq!(report.failed, 0);
    assert_eq!(report.last_replay_tick, Some(3));
    assert_eq!(
        executor.calls.into_iter().collect::<Vec<(u64, String)>>(),
        vec![
            (1, String::from("task-1")),
            (2, String::from("task-1")),
            (3, String::from("task-1")),
        ]
    );
}
