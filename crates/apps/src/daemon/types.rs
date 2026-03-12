pub use axonrunner_infra::RetryClass;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkError {
    Retryable(String),
    NonRetryable(String),
    PolicyDenied(String),
}

impl WorkError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable(message.into())
    }

    pub fn non_retryable(message: impl Into<String>) -> Self {
        Self::NonRetryable(message.into())
    }

    pub fn policy_denied(message: impl Into<String>) -> Self {
        Self::PolicyDenied(message.into())
    }

    pub fn classify(&self) -> RetryClass {
        match self {
            WorkError::Retryable(_) => RetryClass::Retryable,
            WorkError::NonRetryable(_) => RetryClass::NonRetryable,
            WorkError::PolicyDenied(_) => RetryClass::PolicyDenied,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            WorkError::Retryable(message)
            | WorkError::NonRetryable(message)
            | WorkError::PolicyDenied(message) => message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItem {
    pub id: String,
}

impl WorkItem {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

pub trait WorkExecutor {
    fn execute(&mut self, item: &WorkItem, attempt: u32) -> Result<(), WorkError>;
}

pub trait Sleeper {
    fn sleep(&mut self, duration: Duration);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopSleeper;

impl Sleeper for NoopSleeper {
    fn sleep(&mut self, _duration: Duration) {}
}

pub struct StdSleeper;

impl Sleeper for StdSleeper {
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonState {
    Idle,
    Running {
        item_id: String,
        attempt: u32,
    },
    BackingOff {
        item_id: String,
        failed_attempt: u32,
        next_attempt: u32,
        delay: Duration,
    },
    ItemSucceeded {
        item_id: String,
        attempt: u32,
    },
    ItemFailed {
        item_id: String,
        attempt: u32,
        class: RetryClass,
    },
    Complete,
    Stopped,
}

impl DaemonState {
    pub(super) fn label(&self) -> &'static str {
        match self {
            DaemonState::Idle => "idle",
            DaemonState::Running { .. } => "running",
            DaemonState::BackingOff { .. } => "backing_off",
            DaemonState::ItemSucceeded { .. } => "item_succeeded",
            DaemonState::ItemFailed { .. } => "item_failed",
            DaemonState::Complete => "complete",
            DaemonState::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemOutcome {
    Succeeded {
        item_id: String,
        attempt: u32,
    },
    Failed {
        item_id: String,
        attempt: u32,
        class: RetryClass,
    },
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub health_path: PathBuf,
    pub retry_backoff: Vec<Duration>,
}

impl DaemonConfig {
    pub fn new(health_path: impl Into<PathBuf>, retry_backoff: Vec<Duration>) -> Self {
        Self {
            health_path: health_path.into(),
            retry_backoff,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    Exhausted,
    StopRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonReport {
    pub ticks: u64,
    pub completed: usize,
    pub failed: usize,
    pub stop_reason: StopReason,
}
