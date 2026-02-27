use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const HEARTBEAT_FILE_NAME: &str = "HEARTBEAT.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTask {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatReplay {
    pub tick: u64,
    pub task_id: String,
    pub task_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatConfig {
    pub task_path: PathBuf,
    pub interval_ticks: u64,
    pub max_ticks: u64,
}

impl HeartbeatConfig {
    pub fn new(task_path: impl Into<PathBuf>, interval_ticks: u64, max_ticks: u64) -> Self {
        Self {
            task_path: task_path.into(),
            interval_ticks: interval_ticks.max(1),
            max_ticks: max_ticks.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatStopReason {
    Exhausted,
    StopRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatReport {
    pub ticks: u64,
    pub replayed: u64,
    pub failed: u64,
    pub last_replay_tick: Option<u64>,
    pub stop_reason: HeartbeatStopReason,
}

pub trait HeartbeatExecutor {
    fn replay(&mut self, replay: &HeartbeatReplay) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct NoopHeartbeatExecutor;

impl HeartbeatExecutor for NoopHeartbeatExecutor {
    fn replay(&mut self, _replay: &HeartbeatReplay) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatLoop {
    tasks: Vec<HeartbeatTask>,
    interval_ticks: u64,
    max_ticks: u64,
    tick: u64,
    replayed: u64,
    failed: u64,
    last_replay_tick: Option<u64>,
}

impl HeartbeatLoop {
    pub fn new(config: HeartbeatConfig, tasks: Vec<HeartbeatTask>) -> Self {
        Self {
            tasks,
            interval_ticks: config.interval_ticks.max(1),
            max_ticks: config.max_ticks.max(1),
            tick: 0,
            replayed: 0,
            failed: 0,
            last_replay_tick: None,
        }
    }

    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    pub fn run_until<E, S>(&mut self, executor: &mut E, mut should_stop: S) -> HeartbeatReport
    where
        E: HeartbeatExecutor,
        S: FnMut(&HeartbeatLoop) -> bool,
    {
        loop {
            if should_stop(self) {
                return self.build_report(HeartbeatStopReason::StopRequested);
            }
            if self.tick >= self.max_ticks {
                return self.build_report(HeartbeatStopReason::Exhausted);
            }
            self.tick(executor);
        }
    }

    pub fn tick<E>(&mut self, executor: &mut E)
    where
        E: HeartbeatExecutor,
    {
        self.tick = self.tick.saturating_add(1);
        let replay_batch = collect_replay_batch(&self.tasks, self.tick, self.interval_ticks);
        if replay_batch.is_empty() {
            return;
        }

        self.last_replay_tick = Some(self.tick);
        for replay in &replay_batch {
            match executor.replay(replay) {
                Ok(()) => self.replayed = self.replayed.saturating_add(1),
                Err(_) => self.failed = self.failed.saturating_add(1),
            }
        }
    }

    fn build_report(&self, stop_reason: HeartbeatStopReason) -> HeartbeatReport {
        HeartbeatReport {
            ticks: self.tick,
            replayed: self.replayed,
            failed: self.failed,
            last_replay_tick: self.last_replay_tick,
            stop_reason,
        }
    }
}

pub fn should_replay_on_tick(tick: u64, interval_ticks: u64) -> bool {
    interval_ticks > 0 && tick > 0 && tick.is_multiple_of(interval_ticks)
}

pub fn collect_replay_batch(
    tasks: &[HeartbeatTask],
    tick: u64,
    interval_ticks: u64,
) -> Vec<HeartbeatReplay> {
    if !should_replay_on_tick(tick, interval_ticks) {
        return Vec::new();
    }

    tasks
        .iter()
        .map(|task| HeartbeatReplay {
            tick,
            task_id: task.id.clone(),
            task_text: task.text.clone(),
        })
        .collect()
}

pub fn parse_heartbeat_tasks(content: &str) -> Vec<HeartbeatTask> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            let text = trimmed.strip_prefix("- ")?;
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            Some(HeartbeatTask {
                id: format!("task-{}", index + 1),
                text: text.to_owned(),
            })
        })
        .collect()
}

pub fn load_heartbeat_tasks(task_path: &Path) -> io::Result<Vec<HeartbeatTask>> {
    if !task_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(task_path)?;
    Ok(parse_heartbeat_tasks(&content))
}

pub fn ensure_heartbeat_file(workspace_dir: &Path) -> io::Result<PathBuf> {
    let path = workspace_dir.join(HEARTBEAT_FILE_NAME);
    if path.exists() {
        return Ok(path);
    }

    fs::create_dir_all(workspace_dir)?;
    let default_contents =
        "# Periodic Tasks\n\n# Add tasks below (one per line, starting with `- `)\n";
    fs::write(&path, default_contents)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{
        HeartbeatConfig, HeartbeatLoop, HeartbeatStopReason, NoopHeartbeatExecutor,
        collect_replay_batch, parse_heartbeat_tasks, should_replay_on_tick,
    };

    #[test]
    fn parse_heartbeat_tasks_extracts_dash_bullets() {
        let tasks = parse_heartbeat_tasks("# Tasks\n- one\n - two\n* skip\n- three");
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "task-2");
        assert_eq!(tasks[0].text, "one");
        assert_eq!(tasks[1].id, "task-3");
        assert_eq!(tasks[1].text, "two");
        assert_eq!(tasks[2].id, "task-5");
        assert_eq!(tasks[2].text, "three");
    }

    #[test]
    fn should_replay_on_tick_uses_explicit_interval() {
        assert!(!should_replay_on_tick(1, 2));
        assert!(should_replay_on_tick(2, 2));
        assert!(should_replay_on_tick(6, 3));
        assert!(!should_replay_on_tick(0, 3));
    }

    #[test]
    fn collect_replay_batch_returns_empty_when_not_due() {
        let tasks = parse_heartbeat_tasks("- one\n- two");
        let batch = collect_replay_batch(&tasks, 1, 2);
        assert!(batch.is_empty());
    }

    #[test]
    fn heartbeat_loop_exhausts_after_max_ticks() {
        let config = HeartbeatConfig::new("/tmp/heartbeat.md", 2, 3);
        let tasks = parse_heartbeat_tasks("- one\n- two");
        let mut loop_ = HeartbeatLoop::new(config, tasks);
        let mut executor = NoopHeartbeatExecutor;

        let report = loop_.run_until(&mut executor, |_| false);
        assert_eq!(report.stop_reason, HeartbeatStopReason::Exhausted);
        assert_eq!(report.ticks, 3);
        assert_eq!(report.replayed, 2);
        assert_eq!(report.failed, 0);
        assert_eq!(report.last_replay_tick, Some(2));
    }
}
