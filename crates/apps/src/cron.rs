use crate::env_util::resolve_env_path;
use crate::hex_util::{hex_decode_utf8, hex_encode};
use crate::parse_util::parse_number;
use crate::time_util::unix_now_seconds;
use std::fs;
use std::path::{Path, PathBuf};

const ENV_CRON_STORE_PATH: &str = "AXIOM_CRON_STORE_PATH";
const DEFAULT_CRON_STORE_PATH: &str = ".axiom/cron/jobs.db";
const CRON_STORE_FORMAT: &str = "format=axiom-cron-v1";
const MAX_NEXT_RUN_SCAN_MINUTES: u64 = 60 * 24 * 366 * 10;

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
struct CronSchedule {
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
    day_of_month_any: bool,
    day_of_week_any: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CronField {
    min: u8,
    max: u8,
    bits: u64,
}

impl CronField {
    fn new(min: u8, max: u8) -> Self {
        Self { min, max, bits: 0 }
    }

    fn set(&mut self, value: u8) {
        let bit_index = u32::from(value.saturating_sub(self.min));
        self.bits |= 1_u64 << bit_index;
    }

    fn contains(&self, value: u8) -> bool {
        if value < self.min || value > self.max {
            return false;
        }
        let bit_index = u32::from(value.saturating_sub(self.min));
        (self.bits & (1_u64 << bit_index)) != 0
    }

    fn is_empty(&self) -> bool {
        self.bits == 0
    }

    fn is_full(&self) -> bool {
        let width = u32::from(self.max.saturating_sub(self.min) + 1);
        let mask = if width >= 64 {
            u64::MAX
        } else {
            (1_u64 << width) - 1
        };
        self.bits == mask
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CronDateParts {
    minute: u8,
    hour: u8,
    day_of_month: u8,
    month: u8,
    day_of_week: u8, // 0=Sunday
}

impl CronSchedule {
    fn matches_minute(&self, minute_index: u64) -> bool {
        let parts = minute_index_to_date_parts(minute_index);
        if !self.minute.contains(parts.minute)
            || !self.hour.contains(parts.hour)
            || !self.month.contains(parts.month)
        {
            return false;
        }

        let day_of_month_matches = self.day_of_month.contains(parts.day_of_month);
        let day_of_week_matches = self.day_of_week.contains(parts.day_of_week);

        if self.day_of_month_any && self.day_of_week_any {
            return day_of_month_matches && day_of_week_matches;
        }
        if self.day_of_month_any {
            return day_of_week_matches;
        }
        if self.day_of_week_any {
            return day_of_month_matches;
        }
        day_of_month_matches || day_of_week_matches
    }
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
            let next_run_at = next_run_at(schedule, now)
                .map_err(|error| format!("invalid cron expression '{expression}': {error}"))?;

            let mut store = load_store(path)?;
            let job = CronJob {
                id: next_job_id(&store, now),
                expression,
                command,
                created_at: now,
                next_run_at,
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

    let minute = parse_cron_field(fields[0], "minute", 0, 59, false)?;
    let hour = parse_cron_field(fields[1], "hour", 0, 23, false)?;
    let day_of_month = parse_cron_field(fields[2], "day of month", 1, 31, false)?;
    let month = parse_cron_field(fields[3], "month", 1, 12, false)?;
    let day_of_week = parse_cron_field(fields[4], "day of week", 0, 6, true)?;

    Ok(CronSchedule {
        minute,
        hour,
        day_of_month,
        month,
        day_of_week,
        day_of_month_any: day_of_month.is_full(),
        day_of_week_any: day_of_week.is_full(),
    })
}

fn parse_cron_field(
    field: &str,
    label: &str,
    min: u8,
    max: u8,
    allow_sunday_alias: bool,
) -> Result<CronField, String> {
    let mut parsed = CronField::new(min, max);
    for term in field.split(',').map(str::trim) {
        if term.is_empty() {
            return Err(format!(
                "invalid cron {label} field '{field}': empty list segment"
            ));
        }
        parse_cron_term(term, label, min, max, allow_sunday_alias, &mut parsed)?;
    }

    if parsed.is_empty() {
        return Err(format!(
            "invalid cron {label} field '{field}': no values selected"
        ));
    }

    Ok(parsed)
}

fn parse_cron_term(
    term: &str,
    label: &str,
    min: u8,
    max: u8,
    allow_sunday_alias: bool,
    out: &mut CronField,
) -> Result<(), String> {
    let (base, step) = match term.split_once('/') {
        Some((base, step_raw)) => {
            let step = parse_number::<u16>(step_raw, &format!("{label} step"))?;
            if step == 0 {
                return Err(format!(
                    "invalid cron {label} field '{term}': step must be > 0"
                ));
            }
            (base.trim(), step)
        }
        None => (term, 1),
    };

    if base == "*" {
        let raw_max = if allow_sunday_alias {
            u16::from(max) + 1
        } else {
            u16::from(max)
        };
        return apply_cron_range(
            u16::from(min),
            raw_max,
            step,
            label,
            min,
            max,
            allow_sunday_alias,
            out,
        );
    }

    if let Some((start_raw, end_raw)) = base.split_once('-') {
        let start = parse_number::<u16>(start_raw, &format!("{label} range start"))?;
        let end = parse_number::<u16>(end_raw, &format!("{label} range end"))?;
        if start > end {
            return Err(format!(
                "invalid cron {label} field '{term}': range start must be <= range end"
            ));
        }
        return apply_cron_range(start, end, step, label, min, max, allow_sunday_alias, out);
    }

    if step != 1 {
        return Err(format!(
            "invalid cron {label} field '{term}': step requires '*' or range"
        ));
    }

    let value = parse_number::<u16>(base, label)?;
    let normalized = normalize_cron_value(value, label, min, max, allow_sunday_alias)?;
    out.set(normalized);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_cron_range(
    start: u16,
    end: u16,
    step: u16,
    label: &str,
    min: u8,
    max: u8,
    allow_sunday_alias: bool,
    out: &mut CronField,
) -> Result<(), String> {
    let mut current = start;
    while current <= end {
        let normalized = normalize_cron_value(current, label, min, max, allow_sunday_alias)?;
        out.set(normalized);
        let next = current.saturating_add(step);
        if next == current {
            break;
        }
        current = next;
    }
    Ok(())
}

fn normalize_cron_value(
    raw: u16,
    label: &str,
    min: u8,
    max: u8,
    allow_sunday_alias: bool,
) -> Result<u8, String> {
    if allow_sunday_alias && raw == 7 {
        return Ok(0);
    }
    let value = u8::try_from(raw).map_err(|_| {
        format!(
            "invalid cron {label} value '{raw}': expected {min}..{}",
            if allow_sunday_alias { 7 } else { max }
        )
    })?;
    if value < min || value > max {
        return Err(format!(
            "invalid cron {label} value '{raw}': expected {min}..{}",
            if allow_sunday_alias { 7 } else { max }
        ));
    }
    Ok(value)
}

fn next_run_at(schedule: CronSchedule, now: u64) -> Result<u64, String> {
    let start_minute = now / 60 + 1;
    for offset in 0..=MAX_NEXT_RUN_SCAN_MINUTES {
        let candidate = start_minute.saturating_add(offset);
        if schedule.matches_minute(candidate) {
            return Ok(candidate.saturating_mul(60));
        }
    }
    Err(String::from(
        "no execution time found within scan horizon (10 years)",
    ))
}

fn minute_index_to_date_parts(minute_index: u64) -> CronDateParts {
    const MINUTES_PER_DAY: u64 = 24 * 60;
    let days_since_epoch = (minute_index / MINUTES_PER_DAY) as i64;
    let minute_of_day = minute_index % MINUTES_PER_DAY;
    let hour = (minute_of_day / 60) as u8;
    let minute = (minute_of_day % 60) as u8;
    let (_, month, day_of_month) = civil_from_days(days_since_epoch);
    let day_of_week = ((days_since_epoch + 4).rem_euclid(7)) as u8;

    CronDateParts {
        minute,
        hour,
        day_of_month: day_of_month as u8,
        month: month as u8,
        day_of_week,
    }
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm.
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    (year as i32, month as u32, day as u32)
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
        id: hex_decode_utf8(&id_hex)?,
        expression: hex_decode_utf8(&expr_hex)?,
        command: hex_decode_utf8(&cmd_hex)?,
        created_at: required_value(created_at, "created_at")?,
        next_run_at: required_value(next_run_at, "next_run_at")?,
        last_run_at: required_value(last_run_at, "last_run_at")?,
        last_status: match required_value(last_status_hex, "last_status_hex")? {
            Some(value) => Some(hex_decode_utf8(&value)?),
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
        CronAction, CronResult, CronStore, due_jobs, execute_cron_action_at, next_run_at,
        parse_schedule,
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
        assert!(parse_schedule("* * * * *").is_ok());
        assert!(parse_schedule("*/5 * * * *").is_ok());
        assert!(parse_schedule("15 * * * *").is_ok());
    }

    #[test]
    fn parse_schedule_supports_standard_five_field_expressions() {
        assert!(parse_schedule("0 9-17/2 * * 1-5").is_ok());
        assert!(parse_schedule("0,15,30,45 * * * *").is_ok());
        assert!(parse_schedule("30 14 1 1 *").is_ok());
        assert!(parse_schedule("0 0 * * 7").is_ok());
    }

    #[test]
    fn parse_schedule_rejects_unsupported_shapes() {
        assert!(parse_schedule("* * *").is_err());
        assert!(parse_schedule("* * * * * *").is_err());
        assert!(parse_schedule("99 * * * *").is_err());
        assert!(parse_schedule("* 24 * * *").is_err());
        assert!(parse_schedule("* * 0 * *").is_err());
        assert!(parse_schedule("* * * 13 *").is_err());
        assert!(parse_schedule("* * * * 8").is_err());
        assert!(parse_schedule("1,,2 * * * *").is_err());
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
        assert_eq!(
            next_run_at(parse_schedule("* * * * *").expect("parse"), 0).expect("next run"),
            60
        );
        assert_eq!(
            next_run_at(parse_schedule("*/5 * * * *").expect("parse"), 61).expect("next run"),
            300
        );
        assert_eq!(
            next_run_at(parse_schedule("10 * * * *").expect("parse"), 601).expect("next run"),
            4200
        );
    }

    #[test]
    fn next_run_at_honors_day_of_week_and_alias() {
        // Jan 1 1970 is Thursday (dow=4). With dow=5 (Friday), first run is Jan 2 00:00.
        assert_eq!(
            next_run_at(parse_schedule("0 0 * * 5").expect("parse"), 0).expect("next run"),
            86_400
        );
        // Sunday alias 7 should match Jan 4 1970 00:00.
        assert_eq!(
            next_run_at(parse_schedule("0 0 * * 7").expect("parse"), 0).expect("next run"),
            259_200
        );
    }

    #[test]
    fn next_run_at_treats_step_one_as_wildcard_for_dom_dow_semantics() {
        // DOM "*/1" means wildcard. With DOW=0 (Sunday), next midnight should be Sunday.
        assert_eq!(
            next_run_at(parse_schedule("0 0 */1 * 0").expect("parse"), 0).expect("next run"),
            259_200
        );
    }
}
