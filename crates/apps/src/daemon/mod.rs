use axiom_apps::metrics::{
    MetricsSnapshot, record_copy_bytes, record_lock_wait_ns, record_queue_depth,
};
use axiom_apps::metrics_http;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::estop::EStop;
use axiom_adapters::AxiommeContextAdapter;
use axiom_adapters::contracts::ContextAdapter;

mod supervisor;
mod types;

pub use supervisor::*;
pub use types::*;

const ENV_DAEMON_WORK_ITEMS: &str = "AXIOM_DAEMON_WORK_ITEMS";
const ENV_DAEMON_HEALTH_PATH: &str = "AXIOM_DAEMON_HEALTH_PATH";
const ENV_DAEMON_HEALTH_STATE_PATH: &str = "AXIOM_DAEMON_HEALTH_STATE_PATH";
const ENV_DAEMON_MAX_TICKS: &str = "AXIOM_DAEMON_MAX_TICKS";
const ENV_DAEMON_IDLE_SECS: &str = "AXIOM_DAEMON_IDLE_SECS";
const ENV_DAEMON_CHANNEL: &str = "AXIOM_RUNTIME_CHANNEL";
const ENV_DAEMON_SUPERVISOR_INTERVAL_TICKS: &str = "AXIOM_DAEMON_SUPERVISOR_INTERVAL_TICKS";
const DEFAULT_DAEMON_WORK_ITEM: &str = "startup-check";
const DEFAULT_DAEMON_MAX_TICKS: u64 = 32;
const DEFAULT_DAEMON_SUPERVISOR_INTERVAL_TICKS: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRunInput {
    pub health_path: PathBuf,
    pub retry_backoff: Vec<Duration>,
    pub work_items: Vec<WorkItem>,
    pub max_ticks: u64,
    pub channel_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRunSummary {
    pub report: DaemonReport,
    pub health_path: PathBuf,
    pub metrics: MetricsSnapshot,
    pub supervisor: SupervisorRunReport,
}

pub fn run(profile: &str, endpoint: &str) {
    println!("daemon started profile={} endpoint={}", profile, endpoint);

    // Start the Prometheus metrics HTTP server if AXIOM_METRICS_PORT is set.
    let metrics_arc: Arc<Mutex<MetricsSnapshot>> = Arc::new(Mutex::new(MetricsSnapshot::default()));
    if let Some(port) = metrics_http::metrics_port_from_env() {
        metrics_http::spawn_metrics_server(port, Arc::clone(&metrics_arc));
    }

    let run_input = build_daemon_run_input(profile, endpoint);
    let health_state_path = health_state_path(profile, endpoint);
    if let Err(error) = persist_health_state_path(&health_state_path, &run_input.health_path) {
        eprintln!(
            "daemon health state write failed path={} error={error}",
            health_state_path.display()
        );
    }

    match execute_daemon_run(run_input) {
        Ok(summary) => {
            // Publish final metrics snapshot so the HTTP server can serve them.
            if let Ok(mut guard) = metrics_arc.lock() {
                *guard = summary.metrics;
            }
            println!(
                "daemon summary stop={} ticks={} completed={} failed={} health_path={}",
                stop_reason_name(&summary.report.stop_reason),
                summary.report.ticks,
                summary.report.completed,
                summary.report.failed,
                summary.health_path.display()
            );
            println!(
                "daemon metrics queue.current={} queue.peak={} lock.wait_count={} copy.out_bytes={}",
                summary.metrics.queue.current_depth,
                summary.metrics.queue.peak_depth,
                summary.metrics.lock.wait_count,
                summary.metrics.copy.out_bytes
            );
            println!(
                "daemon supervisor components={} failed={} restarts={}",
                summary.supervisor.components.len(),
                summary.supervisor.failed_components,
                summary.supervisor.total_restarts
            );
            println!(
                "daemon health_state path={}",
                health_state_path.as_path().display()
            );
        }
        Err(error) => {
            eprintln!("daemon run failed: {error}");
        }
    }
}

pub fn execute_daemon_run(input: DaemonRunInput) -> io::Result<DaemonRunSummary> {
    let retry_backoff = input.retry_backoff.clone();
    let supervisor_components = default_supervisor_components(&retry_backoff);

    // Spawn channel polling thread if channel_id is configured.
    let estop = Arc::new(EStop::new());
    let channel_thread = if let Some(ref channel_id) = input.channel_id {
        match axiom_adapters::build_contract_channel(channel_id) {
            Ok(mut channel) => match axiom_adapters::build_contract_agent("") {
                Ok(agent) => {
                    let estop_clone = Arc::clone(&estop);
                    let agent: Arc<dyn axiom_adapters::contracts::AgentAdapter> = Arc::from(agent);
                    let rag_context: Option<Arc<dyn ContextAdapter>> =
                        std::env::var("AXIOM_CONTEXT_ROOT")
                            .ok()
                            .filter(|s| !s.trim().is_empty())
                            .and_then(|root| {
                                AxiommeContextAdapter::new(std::path::Path::new(root.trim()))
                            .map(|a| Arc::new(a) as Arc<dyn ContextAdapter>)
                            .map_err(|e| {
                                eprintln!(
                                    "daemon channel context adapter init failed (RAG disabled): {e}"
                                );
                                e
                            })
                            .ok()
                            });
                    Some(std::thread::spawn(move || {
                        crate::channel_serve::run_channel_serve_loop(
                            channel.as_mut(),
                            agent,
                            Some(estop_clone),
                            Duration::from_secs(2),
                            None,
                            rag_context,
                        )
                    }))
                }
                Err(e) => {
                    eprintln!("daemon channel agent init failed: {e}");
                    None
                }
            },
            Err(e) => {
                eprintln!("daemon channel adapter init failed: {e}");
                None
            }
        }
    } else {
        None
    };

    let mut daemon = DaemonLoop::new(
        DaemonConfig::new(input.health_path, retry_backoff),
        input.work_items,
    );
    let mut executor = SuccessExecutor;
    let mut sleeper = NoopSleeper;
    let max_ticks = input.max_ticks;
    let supervisor_interval_ticks = env::var(ENV_DAEMON_SUPERVISOR_INTERVAL_TICKS)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|ticks| *ticks > 0)
        .unwrap_or(DEFAULT_DAEMON_SUPERVISOR_INTERVAL_TICKS);
    let health_path_for_supervisor = daemon.health_path().to_path_buf();
    let mut supervisor_runner = |component: &SupervisorComponentSpec,
                                 _attempt: u32|
     -> Result<(), SupervisorError> {
        match component.kind {
            SupervisorComponentKind::Gateway => {
                // Attempt a TCP connection to the metrics port when
                // AXIOM_METRICS_PORT is configured.  A successful connect
                // (or immediate RST) proves the port is bound.  If the env
                // var is absent the gateway check is treated as a pass.
                //
                // Timeout is configurable via AXIOM_SUPERVISOR_GATEWAY_TIMEOUT_MS
                // (default 500 ms).
                let timeout_ms = env::var("AXIOM_SUPERVISOR_GATEWAY_TIMEOUT_MS")
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(500);
                match env::var(axiom_apps::metrics_http::ENV_METRICS_PORT)
                    .ok()
                    .and_then(|raw| raw.trim().parse::<u16>().ok())
                    .filter(|&p| p > 0)
                {
                    Some(port) => {
                        let addr = format!("127.0.0.1:{port}");
                        TcpStream::connect_timeout(
                            &addr.parse().map_err(|_| {
                                SupervisorError::terminal("metrics port addr parse failed")
                            })?,
                            Duration::from_millis(timeout_ms),
                        )
                        .map(|_| ())
                        .map_err(|e| {
                            SupervisorError::retryable(format!("gateway tcp connect failed: {e}"))
                        })
                    }
                    None => Ok(()),
                }
            }
            SupervisorComponentKind::Channels => {
                // Channel presence is optional; an unconfigured channel is not
                // an error.  When configured, the name must be a known channel
                // type.
                const VALID_CHANNELS: &[&str] =
                    &["telegram", "discord", "slack", "irc", "matrix", "whatsapp"];
                match env::var(ENV_DAEMON_CHANNEL)
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                {
                    None => Ok(()),
                    Some(name) => {
                        let name = name.trim().to_lowercase();
                        if VALID_CHANNELS.contains(&name.as_str()) {
                            Ok(())
                        } else {
                            Err(SupervisorError::retryable(format!(
                                "unknown channel type '{}'; valid: {}",
                                name,
                                VALID_CHANNELS.join(", ")
                            )))
                        }
                    }
                }
            }
            SupervisorComponentKind::Scheduler => {
                // A cron expression is optional.  When configured, it must
                // have at least 5 whitespace-separated fields (standard cron
                // format: minute hour day month weekday).
                match env::var("AXIOM_CRON_EXPR")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                {
                    None => Ok(()),
                    Some(expr) => {
                        let field_count = expr.split_whitespace().count();
                        if field_count >= 5 {
                            Ok(())
                        } else {
                            Err(SupervisorError::retryable(format!(
                                "invalid AXIOM_CRON_EXPR '{}': expected at least 5 fields, got {}",
                                expr.trim(),
                                field_count
                            )))
                        }
                    }
                }
            }
            SupervisorComponentKind::Heartbeat => {
                // The health file must exist and be a regular file.
                if health_path_for_supervisor.exists() {
                    Ok(())
                } else {
                    Err(SupervisorError::retryable(format!(
                        "health file missing path={}",
                        health_path_for_supervisor.display()
                    )))
                }
            }
        }
    };
    let mut supervisor_sleeper = NoopSupervisorSleeper;

    // Run supervisor health checks during daemon execution, not only after
    // loop completion. This allows earlier detection of component failures.
    let mut supervisor = SupervisorRunReport {
        components: Vec::new(),
        total_restarts: 0,
        failed_components: 0,
    };

    let report = daemon.run_until_with_probe(
        &mut executor,
        &mut sleeper,
        |loop_ref| loop_ref.tick_count() >= max_ticks,
        |loop_ref| {
            if loop_ref.tick_count() > 0 && loop_ref.tick_count() % supervisor_interval_ticks == 0 {
                supervisor = run_supervisor_cycle(
                    &supervisor_components,
                    &mut supervisor_runner,
                    &mut supervisor_sleeper,
                );
            }
        },
    )?;

    // Always run one final cycle after loop completion so summary reflects
    // final daemon state/health file availability.
    supervisor = run_supervisor_cycle(
        &supervisor_components,
        &mut supervisor_runner,
        &mut supervisor_sleeper,
    );

    // If AXIOM_DAEMON_IDLE_SECS is set, keep the process alive (and metrics server
    // reachable) for that many seconds before exiting.
    if let Ok(raw) = env::var(ENV_DAEMON_IDLE_SECS)
        && let Ok(secs) = raw.trim().parse::<u64>()
        && secs > 0
    {
        println!("daemon idle sleep={secs}s");
        std::thread::sleep(Duration::from_secs(secs));
    }

    // Signal estop first so the channel polling loop can observe the halt
    // and exit cleanly, then join the thread.
    estop.halt();
    if let Some(handle) = channel_thread {
        match handle.join() {
            Ok(Ok(processed)) => {
                println!("daemon channel thread finished processed={processed}");
            }
            Ok(Err(e)) => eprintln!("daemon channel thread error: {e}"),
            Err(_) => eprintln!("daemon channel thread panicked"),
        }
    }

    Ok(DaemonRunSummary {
        report,
        health_path: daemon.health_path().to_path_buf(),
        metrics: daemon.metrics_snapshot(),
        supervisor,
    })
}

fn build_daemon_run_input(profile: &str, endpoint: &str) -> DaemonRunInput {
    let health_path = env::var(ENV_DAEMON_HEALTH_PATH)
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_health_path(profile, endpoint));

    let work_items = env::var(ENV_DAEMON_WORK_ITEMS)
        .ok()
        .map(|raw| parse_work_items(&raw))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![WorkItem::new(DEFAULT_DAEMON_WORK_ITEM)]);

    let max_ticks = env::var(ENV_DAEMON_MAX_TICKS)
        .ok()
        .and_then(|raw| parse_max_ticks(&raw))
        .unwrap_or(DEFAULT_DAEMON_MAX_TICKS);

    let channel_id = env::var(ENV_DAEMON_CHANNEL)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    DaemonRunInput {
        health_path,
        retry_backoff: vec![
            Duration::from_millis(10),
            Duration::from_millis(25),
            Duration::from_millis(50),
        ],
        work_items,
        max_ticks,
        channel_id,
    }
}

fn parse_work_items(raw: &str) -> Vec<WorkItem> {
    raw.split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(WorkItem::new)
        .collect()
}

fn parse_max_ticks(raw: &str) -> Option<u64> {
    let ticks = raw.trim().parse::<u64>().ok()?;
    (ticks > 0).then_some(ticks)
}

pub fn resolve_health_path_from_state(profile: &str, endpoint: &str) -> Option<PathBuf> {
    read_health_state_path(&health_state_path(profile, endpoint))
}

pub fn health_state_path(profile: &str, endpoint: &str) -> PathBuf {
    env::var(ENV_DAEMON_HEALTH_STATE_PATH)
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_health_state_path(profile, endpoint))
}

fn default_health_state_path(profile: &str, endpoint: &str) -> PathBuf {
    let profile = sanitize_token(profile);
    let endpoint = sanitize_token(endpoint);

    env::temp_dir().join(format!("axiom-daemon-{profile}-{endpoint}.health-path"))
}

fn default_health_path(profile: &str, endpoint: &str) -> PathBuf {
    let profile = sanitize_token(profile);
    let endpoint = sanitize_token(endpoint);
    let pid = std::process::id();

    env::temp_dir().join(format!("axiom-daemon-{profile}-{endpoint}-{pid}.health"))
}

fn persist_health_state_path(state_path: &Path, health_path: &Path) -> io::Result<()> {
    if let Some(parent) = state_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    fs::write(state_path, format!("{}\n", health_path.display()))
}

fn read_health_state_path(state_path: &Path) -> Option<PathBuf> {
    let raw = fs::read_to_string(state_path).ok()?;
    let first_line = raw.lines().next()?.trim();
    if first_line.is_empty() {
        return None;
    }

    Some(PathBuf::from(first_line))
}

fn sanitize_token(raw: &str) -> String {
    let mut token = String::with_capacity(raw.len());

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            token.push(ch.to_ascii_lowercase());
        } else {
            token.push('_');
        }
    }

    while token.contains("__") {
        token = token.replace("__", "_");
    }

    token.trim_matches('_').to_string().if_empty_then("default")
}

fn stop_reason_name(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Exhausted => "exhausted",
        StopReason::StopRequested => "stop_requested",
    }
}

#[derive(Debug, Default)]
struct SuccessExecutor;

impl WorkExecutor for SuccessExecutor {
    fn execute(&mut self, _item: &WorkItem, _attempt: u32) -> Result<(), WorkError> {
        Ok(())
    }
}

trait EmptyToDefault {
    fn if_empty_then(self, default: &str) -> Self;
}

impl EmptyToDefault for String {
    fn if_empty_then(self, default: &str) -> Self {
        if self.is_empty() {
            default.to_owned()
        } else {
            self
        }
    }
}

#[derive(Debug, Clone)]
struct InFlight {
    item: WorkItem,
    attempt: u32,
}

pub struct DaemonLoop {
    queue: VecDeque<WorkItem>,
    in_flight: Option<InFlight>,
    state: DaemonState,
    tick: u64,
    completed: usize,
    failed: usize,
    retry_backoff: Vec<Duration>,
    health_path: PathBuf,
    last_outcome: Option<ItemOutcome>,
    metrics: MetricsSnapshot,
}

impl DaemonLoop {
    pub fn new(config: DaemonConfig, work_items: impl IntoIterator<Item = WorkItem>) -> Self {
        let queue: VecDeque<WorkItem> = work_items.into_iter().collect();
        let mut metrics = MetricsSnapshot::default();
        metrics = record_queue_depth(metrics, queue.len() as u64);

        Self {
            queue,
            in_flight: None,
            state: DaemonState::Idle,
            tick: 0,
            completed: 0,
            failed: 0,
            retry_backoff: config.retry_backoff,
            health_path: config.health_path,
            last_outcome: None,
            metrics,
        }
    }

    pub fn state(&self) -> &DaemonState {
        &self.state
    }

    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    pub fn last_outcome(&self) -> Option<&ItemOutcome> {
        self.last_outcome.as_ref()
    }

    pub fn health_path(&self) -> &Path {
        &self.health_path
    }

    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics
    }

    pub fn run_until<E, S>(
        &mut self,
        executor: &mut E,
        sleeper: &mut dyn Sleeper,
        should_stop: S,
    ) -> io::Result<DaemonReport>
    where
        E: WorkExecutor,
        S: FnMut(&DaemonLoop) -> bool,
    {
        self.run_until_with_probe(executor, sleeper, should_stop, |_| {})
    }

    pub fn run_until_with_probe<E, S, P>(
        &mut self,
        executor: &mut E,
        sleeper: &mut dyn Sleeper,
        mut should_stop: S,
        mut on_tick: P,
    ) -> io::Result<DaemonReport>
    where
        E: WorkExecutor,
        S: FnMut(&DaemonLoop) -> bool,
        P: FnMut(&DaemonLoop),
    {
        loop {
            on_tick(self);
            if should_stop(self) {
                self.state = DaemonState::Stopped;
                self.update_queue_metrics();
                self.write_health()?;
                return Ok(self.build_report(StopReason::StopRequested));
            }

            if self.queue.is_empty() && self.in_flight.is_none() {
                self.state = DaemonState::Complete;
                self.update_queue_metrics();
                self.write_health()?;
                return Ok(self.build_report(StopReason::Exhausted));
            }

            self.tick(executor, sleeper)?;
        }
    }

    pub fn tick<E>(&mut self, executor: &mut E, sleeper: &mut dyn Sleeper) -> io::Result<()>
    where
        E: WorkExecutor,
    {
        self.tick = self.tick.saturating_add(1);
        self.update_queue_metrics();

        if let DaemonState::BackingOff {
            item_id,
            failed_attempt,
            next_attempt,
            delay,
        } = self.state.clone()
        {
            self.metrics = record_lock_wait_ns(self.metrics, duration_as_ns(delay));
            sleeper.sleep(delay);
            if let Some(in_flight) = self.in_flight.as_mut() {
                in_flight.attempt = next_attempt;
            }
            self.state = DaemonState::Running {
                item_id,
                attempt: next_attempt,
            };
            self.update_queue_metrics();
            self.write_health_detail(&format!(
                "resuming_after_backoff failed_attempt={failed_attempt}"
            ))?;
            return Ok(());
        }

        if self.in_flight.is_none() {
            if let Some(next) = self.queue.pop_front() {
                self.metrics = record_copy_bytes(self.metrics, next.id.len() as u64, 0);
                self.in_flight = Some(InFlight {
                    item: next,
                    attempt: 1,
                });
                self.update_queue_metrics();
            } else {
                self.state = DaemonState::Complete;
                self.update_queue_metrics();
                self.write_health()?;
                return Ok(());
            }
        }

        let current = self.in_flight.clone().expect("in-flight item should exist");
        self.state = DaemonState::Running {
            item_id: current.item.id.clone(),
            attempt: current.attempt,
        };
        self.write_health()?;

        match executor.execute(&current.item, current.attempt) {
            Ok(()) => {
                self.completed = self.completed.saturating_add(1);
                self.last_outcome = Some(ItemOutcome::Succeeded {
                    item_id: current.item.id.clone(),
                    attempt: current.attempt,
                });
                self.state = DaemonState::ItemSucceeded {
                    item_id: current.item.id,
                    attempt: current.attempt,
                };
                self.in_flight = None;
                self.update_queue_metrics();
                self.write_health()?;
            }
            Err(error) => {
                let class = error.classify();
                if class == RetryClass::Retryable {
                    if let Some(delay) = self.retry_delay(current.attempt) {
                        self.state = DaemonState::BackingOff {
                            item_id: current.item.id,
                            failed_attempt: current.attempt,
                            next_attempt: current.attempt.saturating_add(1),
                            delay,
                        };
                        self.write_health_detail(error.message())?;
                    } else {
                        self.record_failure(current.item.id, current.attempt, class);
                        self.write_health_detail(error.message())?;
                    }
                } else {
                    self.record_failure(current.item.id, current.attempt, class);
                    self.write_health_detail(error.message())?;
                }
            }
        }

        Ok(())
    }

    fn build_report(&self, stop_reason: StopReason) -> DaemonReport {
        DaemonReport {
            ticks: self.tick,
            completed: self.completed,
            failed: self.failed,
            stop_reason,
        }
    }

    fn retry_delay(&self, failed_attempt: u32) -> Option<Duration> {
        failed_attempt
            .checked_sub(1)
            .and_then(|index| self.retry_backoff.get(index as usize).copied())
    }

    fn record_failure(&mut self, item_id: String, attempt: u32, class: RetryClass) {
        self.failed = self.failed.saturating_add(1);
        self.last_outcome = Some(ItemOutcome::Failed {
            item_id: item_id.clone(),
            attempt,
            class,
        });
        self.state = DaemonState::ItemFailed {
            item_id,
            attempt,
            class,
        };
        self.in_flight = None;
        self.update_queue_metrics();
    }

    fn write_health(&mut self) -> io::Result<()> {
        self.write_health_detail("-")
    }

    fn write_health_detail(&mut self, detail: &str) -> io::Result<()> {
        if let Some(parent) = self.health_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let health = self.render_health(detail);
        self.metrics = record_copy_bytes(self.metrics, 0, health.len() as u64);

        fs::write(&self.health_path, health)
    }

    fn render_health(&self, detail: &str) -> String {
        let (in_flight_id, in_flight_attempt) = self
            .in_flight
            .as_ref()
            .map(|in_flight| (in_flight.item.id.as_str(), in_flight.attempt))
            .unwrap_or(("-", 0));

        let state_detail = match &self.state {
            DaemonState::Running { item_id, attempt } => {
                format!("item={item_id} attempt={attempt}")
            }
            DaemonState::BackingOff {
                item_id,
                failed_attempt,
                next_attempt,
                delay,
            } => format!(
                "item={item_id} failed_attempt={failed_attempt} next_attempt={next_attempt} delay_ms={}",
                delay.as_millis()
            ),
            DaemonState::ItemSucceeded { item_id, attempt } => {
                format!("item={item_id} attempt={attempt}")
            }
            DaemonState::ItemFailed {
                item_id,
                attempt,
                class,
            } => format!("item={item_id} attempt={attempt} class={}", class.as_str()),
            DaemonState::Idle | DaemonState::Complete | DaemonState::Stopped => String::from("-"),
        };

        format!(
            "tick={}\nstate={}\nstate_detail={}\nreason={}\nin_flight={}\nin_flight_attempt={}\nqueue_depth={}\ncompleted={}\nfailed={}\n",
            self.tick,
            self.state.label(),
            state_detail,
            detail,
            in_flight_id,
            in_flight_attempt,
            self.queue.len(),
            self.completed,
            self.failed
        )
    }

    fn update_queue_metrics(&mut self) {
        self.metrics = record_queue_depth(self.metrics, self.queue_depth_for_metrics());
    }

    fn queue_depth_for_metrics(&self) -> u64 {
        self.queue.len() as u64 + u64::from(self.in_flight.is_some())
    }
}

fn duration_as_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_DAEMON_MAX_TICKS, DEFAULT_DAEMON_WORK_ITEM, execute_daemon_run, health_state_path,
        parse_max_ticks, parse_work_items, resolve_health_path_from_state,
    };
    use crate::daemon::build_daemon_run_input;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_token(label: &str) -> String {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        format!("{label}-{}-{tick}", std::process::id())
    }

    fn unique_health_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "axiom-daemon-health-{label}-{}.status",
            unique_token("path")
        ))
    }

    #[test]
    fn parse_work_items_skips_empty_segments() {
        let items = parse_work_items("a,, b , ,c");
        let ids: Vec<String> = items.into_iter().map(|item| item.id).collect();

        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_max_ticks_requires_positive_integer() {
        assert_eq!(parse_max_ticks("12"), Some(12));
        assert_eq!(parse_max_ticks("0"), None);
        assert_eq!(parse_max_ticks("abc"), None);
    }

    #[test]
    fn build_daemon_run_input_defaults_when_env_absent() {
        let input = build_daemon_run_input("prod", "http://127.0.0.1:8080");

        assert_eq!(input.work_items.len(), 1);
        assert_eq!(input.work_items[0].id, DEFAULT_DAEMON_WORK_ITEM);
        assert_eq!(input.max_ticks, DEFAULT_DAEMON_MAX_TICKS);
        assert_eq!(input.channel_id, None);
    }

    #[test]
    fn execute_daemon_run_completes_with_success_executor() {
        let mut input = build_daemon_run_input("prod", "http://127.0.0.1:8080");
        input.max_ticks = 8;

        let summary = execute_daemon_run(input).expect("daemon run should complete");

        assert_eq!(summary.report.completed, 1);
        assert_eq!(summary.report.failed, 0);
        assert!(summary.health_path.exists());
        assert_eq!(summary.supervisor.components.len(), 4);
        assert_eq!(summary.supervisor.failed_components, 0);
        assert_eq!(summary.supervisor.total_restarts, 0);

        let _ = std::fs::remove_file(summary.health_path);
    }

    #[test]
    fn health_state_path_round_trip_resolves_health_file() {
        let profile = unique_token("profile");
        let endpoint = format!("http://{}.local", unique_token("endpoint"));
        let health = unique_health_path("round-trip");
        let state = health_state_path(&profile, &endpoint);

        fs::write(&health, "tick=1\nstate=running\nstate_detail=-\nreason=-\nin_flight=-\nin_flight_attempt=0\nqueue_depth=0\ncompleted=0\nfailed=0\n")
            .expect("health fixture should be writable");
        fs::write(&state, format!("{}\n", health.display()))
            .expect("state pointer should be writable");

        let resolved = resolve_health_path_from_state(&profile, &endpoint);
        assert_eq!(resolved, Some(health.clone()));

        let _ = fs::remove_file(state);
        let _ = fs::remove_file(health);
    }
}
