use std::fs;
use std::path::{Path, PathBuf};
use crate::env_util::resolve_env_path;
use crate::time_util::unix_now_seconds;
use crate::hex_util::{hex_decode, hex_encode};
use crate::parse_util::parse_number;

const ENV_CRON_STORE_PATH: &str = "AXIOM_CRON_STORE_PATH";
const DEFAULT_CRON_STORE_PATH: &str = ".axiom/cron/jobs.db";
const CRON_STORE_FORMAT: &str = "format=axiom-cron-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronAction {
    List,
    Add { expression: String, command: String },
    Remove { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronJob {
    pub id: String,
    pub expression: String,
    pub command: String,
    pub created_at: u64,
    pub next_run_at: u64,
    pub last_run_at: Option<u64>,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CronStore {
    pub jobs: Vec<CronJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronResult {
    Listed {
        path: PathBuf,
        jobs: Vec<CronJob>,
        due_count: usize,
    },
    Added {
        path: PathBuf,
        job: CronJob,
    },
    Removed {
        path: PathBuf,
        id: String,
        remaining: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CronSchedule {
    EveryMinute,
    EveryNMinutes(u64),
    MinuteOfHour(u8),
}

pub fn execute_cron_action(action: CronAction) -> Result<CronResult, String> {
    let path = resolve_env_path(ENV_CRON_STORE_PATH, Path::new(DEFAULT_CRON_STORE_PATH))?;
    execute_cron_action_at(action, &path, unix_now_seconds())
}

pub fn due_jobs(store: &CronStore, now: u64) -> Vec<CronJob> {
    let mut jobs: Vec<CronJob> = store
        .jobs
        .iter()
        .filter(|job| job.next_run_at <= now)
        .cloned()
        .collect();
    sort_jobs(&mut jobs);
    jobs
}

fn execute_cron_action_at(action: CronAction, path: &Path, now: u64) -> Result<CronResult, String> {
    match action {
        CronAction::List => {
            let mut store = load_store(path)?;
            sort_jobs(&mut store.jobs);
            let due_count = due_jobs(&store, now).len();
            Ok(CronResult::Listed {
                path: path.to_path_buf(),
                jobs: store.jobs,
                due_count,
            })
        }
        CronAction::Add {
            expression,
            command,
        } => {
            let schedule = parse_schedule(&expression)?;
            validate_command(&command)?;

            let mut store = load_store(path)?;
            let job = CronJob {
                id: next_job_id(&store, now),
                expression,
                command,
                created_at: now,
                next_run_at: next_run_at(schedule, now),
                last_run_at: None,
                last_status: None,
            };

            store.jobs.push(job.clone());
            sort_jobs(&mut store.jobs);
            save_store(path, &store)?;

            Ok(CronResult::Added {
                path: path.to_path_buf(),
                job,
            })
        }
        CronAction::Remove { id } => {
            let mut store = load_store(path)?;
            let Some(index) = store.jobs.iter().position(|job| job.id == id) else {
                return Err(format!("cron job '{id}' not found"));
            };

            store.jobs.remove(index);
            sort_jobs(&mut store.jobs);
            save_store(path, &store)?;

            Ok(CronResult::Removed {
                path: path.to_path_buf(),
                id,
                remaining: store.jobs.len(),
            })
        }
    }
}



fn validate_command(command: &str) -> Result<(), String> {
    if command.trim().is_empty() {
        return Err(String::from("cron command must not be empty"));
    }
    if command.contains('\n') || command.contains('\r') {
        return Err(String::from("cron command must be single-line text"));
    }
    Ok(())
}

fn parse_schedule(expression: &str) -> Result<CronSchedule, String> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(format!(
            "invalid cron expression '{expression}': expected 5 fields"
        ));
    }

    if fields[1..].iter().any(|field| *field != "*") {
        return Err(format!(
            "invalid cron expression '{expression}': only minute field is supported"
        ));
    }

    let minute = fields[0];
    if minute == "*" {
        return Ok(CronSchedule::EveryMinute);
    }

    if let Some(step) = minute.strip_prefix("*/") {
        let step = parse_number::<u64>(step, "minute step")?;
        if step == 0 {
            return Err(format!(
                "invalid cron expression '{expression}': minute step must be > 0"
            ));
        }
        return Ok(CronSchedule::EveryNMinutes(step));
    }

    let minute_of_hour = parse_number::<u8>(minute, "minute value")?;
    if minute_of_hour > 59 {
        return Err(format!(
            "invalid cron expression '{expression}': minute must be <= 59"
        ));
    }

    Ok(CronSchedule::MinuteOfHour(minute_of_hour))
}


fn next_run_at(schedule: CronSchedule, now: u64) -> u64 {
    let mut minute = now / 60 + 1;

    match schedule {
        CronSchedule::EveryMinute => minute * 60,
        CronSchedule::EveryNMinutes(step) => {
            let remainder = minute % step;
            if remainder != 0 {
                minute += step - remainder;
            }
            minute * 60
        }
        CronSchedule::MinuteOfHour(target) => {
            while minute % 60 != u64::from(target) {
                minute += 1;
            }
            minute * 60
        }
    }
}

fn next_job_id(store: &CronStore, now: u64) -> String {
    let mut index = store.jobs.len() as u64;
    loop {
        let candidate = format!("cron-{now}-{index}");
        if store.jobs.iter().all(|job| job.id != candidate) {
            return candidate;
        }
        index = index.saturating_add(1);
    }
}

fn sort_jobs(jobs: &mut [CronJob]) {
    jobs.sort_by(|left, right| {
        left.next_run_at
            .cmp(&right.next_run_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn load_store(path: &Path) -> Result<CronStore, String> {
    if !path.exists() {
        return Ok(CronStore::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read cron store '{}': {error}", path.display()))?;
    parse_store(&content).map_err(|error| {
        format!(
            "failed to parse cron store '{}': {error}",
            path.to_string_lossy()
        )
    })
}

fn save_store(path: &Path, store: &CronStore) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create cron store directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    let mut output = String::new();
    output.push_str(CRON_STORE_FORMAT);
    output.push('\n');
    for job in &store.jobs {
        output.push_str(&render_job_line(job));
        output.push('\n');
    }

    fs::write(path, output)
        .map_err(|error| format!("failed to write cron store '{}': {error}", path.display()))
}

fn parse_store(content: &str) -> Result<CronStore, String> {
    let mut jobs = Vec::new();

    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line == CRON_STORE_FORMAT {
            continue;
        }

        let job = parse_job_line(line).map_err(|error| format!("line {}: {error}", index + 1))?;
        jobs.push(job);
    }

    sort_jobs(&mut jobs);
    Ok(CronStore { jobs })
}

fn render_job_line(job: &CronJob) -> String {
    let last_run = job
        .last_run_at
        .map(|value| value.to_string())
        .unwrap_or_else(|| String::from("-"));
    let last_status = job
        .last_status
        .as_ref()
        .map(|value| hex_encode(value))
        .unwrap_or_else(|| String::from("-"));

    format!(
        "id_hex={};expr_hex={};cmd_hex={};created_at={};next_run_at={};last_run_at={};last_status_hex={}",
        hex_encode(&job.id),
        hex_encode(&job.expression),
        hex_encode(&job.command),
        job.created_at,
        job.next_run_at,
        last_run,
        last_status
    )
}

fn parse_job_line(line: &str) -> Result<CronJob, String> {
    let mut id_hex: Option<String> = None;
    let mut expr_hex: Option<String> = None;
    let mut cmd_hex: Option<String> = None;
    let mut created_at: Option<u64> = None;
    let mut next_run_at: Option<u64> = None;
    let mut last_run_at: Option<Option<u64>> = None;
    let mut last_status_hex: Option<Option<String>> = None;

    for pair in line.split(';') {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("invalid key/value pair '{pair}'"))?;

        match key {
            "id_hex" => id_hex = Some(value.to_string()),
            "expr_hex" => expr_hex = Some(value.to_string()),
            "cmd_hex" => cmd_hex = Some(value.to_string()),
            "created_at" => created_at = Some(parse_number::<u64>(value, "created_at")?),
            "next_run_at" => next_run_at = Some(parse_number::<u64>(value, "next_run_at")?),
            "last_run_at" => {
                if value == "-" {
                    last_run_at = Some(None);
                } else {
                    last_run_at = Some(Some(parse_number::<u64>(value, "last_run_at")?));
                }
            }
            "last_status_hex" => {
                if value == "-" {
                    last_status_hex = Some(None);
                } else {
                    last_status_hex = Some(Some(value.to_string()));
                }
            }
            _ => return Err(format!("unknown cron field '{key}'")),
        }
    }

    let id_hex = required_string(id_hex, "id_hex")?;
    let expr_hex = required_string(expr_hex, "expr_hex")?;
    let cmd_hex = required_string(cmd_hex, "cmd_hex")?;

    Ok(CronJob {
        id: hex_decode(&id_hex)?,
        expression: hex_decode(&expr_hex)?,
        command: hex_decode(&cmd_hex)?,
        created_at: required_value(created_at, "created_at")?,
        next_run_at: required_value(next_run_at, "next_run_at")?,
        last_run_at: required_value(last_run_at, "last_run_at")?,
        last_status: match required_value(last_status_hex, "last_status_hex")? {
            Some(value) => Some(hex_decode(&value)?),
            None => None,
        },
    })
}

fn required_string(value: Option<String>, field: &str) -> Result<String, String> {
    value.ok_or_else(|| format!("missing field '{field}'"))
}

fn required_value<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("missing field '{field}'"))
}

#[cfg(test)]
mod tests {
    use super::{
        CronAction, CronResult, CronSchedule, CronStore, due_jobs, execute_cron_action_at,
        next_run_at, parse_schedule,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-cron-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn parse_schedule_supports_minute_variants() {
        assert_eq!(parse_schedule("* * * * *"), Ok(CronSchedule::EveryMinute));
        assert_eq!(
            parse_schedule("*/5 * * * *"),
            Ok(CronSchedule::EveryNMinutes(5))
        );
        assert_eq!(
            parse_schedule("15 * * * *"),
            Ok(CronSchedule::MinuteOfHour(15))
        );
    }

    #[test]
    fn parse_schedule_rejects_unsupported_shapes() {
        assert!(parse_schedule("* * *").is_err());
        assert!(parse_schedule("* * * * * *").is_err());
        assert!(parse_schedule("* 10 * * *").is_err());
        assert!(parse_schedule("99 * * * *").is_err());
    }

    #[test]
    fn execute_add_list_remove_roundtrip() {
        let path = unique_path("roundtrip", "db");
        let added = execute_cron_action_at(
            CronAction::Add {
                expression: String::from("*/5 * * * *"),
                command: String::from("echo hello"),
            },
            &path,
            301,
        )
        .expect("add should succeed");

        let id = match added {
            CronResult::Added { job, .. } => job.id,
            _ => panic!("add result should be CronResult::Added"),
        };

        let listed =
            execute_cron_action_at(CronAction::List, &path, 301).expect("list should succeed");
        match listed {
            CronResult::Listed {
                jobs, due_count, ..
            } => {
                assert_eq!(jobs.len(), 1);
                assert_eq!(due_count, 0);
                assert_eq!(jobs[0].id, id);
                assert_eq!(jobs[0].next_run_at, 600);
            }
            _ => panic!("list result should be CronResult::Listed"),
        }

        let removed = execute_cron_action_at(CronAction::Remove { id: id.clone() }, &path, 301)
            .expect("remove should succeed");
        match removed {
            CronResult::Removed { remaining, .. } => assert_eq!(remaining, 0),
            _ => panic!("remove result should be CronResult::Removed"),
        }

        let listed_after =
            execute_cron_action_at(CronAction::List, &path, 301).expect("list should succeed");
        match listed_after {
            CronResult::Listed { jobs, .. } => assert!(jobs.is_empty()),
            _ => panic!("list result should be CronResult::Listed"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn due_jobs_returns_due_items_sorted_by_next_run_then_id() {
        let store = CronStore {
            jobs: vec![
                super::CronJob {
                    id: String::from("b"),
                    expression: String::from("* * * * *"),
                    command: String::from("echo b"),
                    created_at: 0,
                    next_run_at: 120,
                    last_run_at: None,
                    last_status: None,
                },
                super::CronJob {
                    id: String::from("a"),
                    expression: String::from("* * * * *"),
                    command: String::from("echo a"),
                    created_at: 0,
                    next_run_at: 120,
                    last_run_at: None,
                    last_status: None,
                },
                super::CronJob {
                    id: String::from("c"),
                    expression: String::from("* * * * *"),
                    command: String::from("echo c"),
                    created_at: 0,
                    next_run_at: 180,
                    last_run_at: None,
                    last_status: None,
                },
            ],
        };

        let due = due_jobs(&store, 120);
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].id, "a");
        assert_eq!(due[1].id, "b");
    }

    #[test]
    fn next_run_at_computes_expected_minute_alignment() {
        assert_eq!(next_run_at(CronSchedule::EveryMinute, 0), 60);
        assert_eq!(next_run_at(CronSchedule::EveryNMinutes(5), 61), 300);
        assert_eq!(next_run_at(CronSchedule::MinuteOfHour(10), 601), 4200);
    }
}
