use std::path::{Path, PathBuf};

use axiom_adapters::{TelegramChannelAdapter, TelegramConfig};

use crate::env_util::resolve_env_path;
use crate::time_util::unix_now_seconds;

const ENV_CHANNEL_STORE_PATH: &str = "AXIOM_CHANNEL_STORE_PATH";
const DEFAULT_CHANNEL_STORE_PATH: &str = ".axiom/channel/store.db";
const CHANNEL_STORE_FORMAT: &str = "format=axiom-channel-v1";

mod store;

use self::store::{
    load_store, parse_channel_config, parse_channel_kind, save_store, sort_channels,
    try_build_channel_adapter, validate_channel_config, validate_channel_name,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    Telegram,
    Discord,
    Slack,
    Matrix,
    Whatsapp,
    Irc,
    Webhook,
    Cli,
}

impl ChannelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChannelKind::Telegram => "telegram",
            ChannelKind::Discord => "discord",
            ChannelKind::Slack => "slack",
            ChannelKind::Matrix => "matrix",
            ChannelKind::Whatsapp => "whatsapp",
            ChannelKind::Irc => "irc",
            ChannelKind::Webhook => "webhook",
            ChannelKind::Cli => "cli",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelHealthStatus {
    Ok,
    Unhealthy,
}

impl ChannelHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ChannelHealthStatus::Ok => "ok",
            ChannelHealthStatus::Unhealthy => "unhealthy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelRecord {
    pub name: String,
    pub channel_type: ChannelKind,
    pub config: String,
    pub running: bool,
    pub last_health: Option<ChannelHealthStatus>,
    pub last_checked_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChannelStore {
    pub channels: Vec<ChannelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDoctorCheck {
    pub name: String,
    pub channel_type: ChannelKind,
    pub status: ChannelHealthStatus,
    pub detail: String,
    pub checked_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelAction {
    List,
    Start,
    Doctor,
    Add {
        channel_type: String,
        config: String,
    },
    Remove {
        name: String,
    },
    Serve {
        poll_interval_secs: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelResult {
    Listed {
        path: PathBuf,
        channels: Vec<ChannelRecord>,
        running: usize,
    },
    Started {
        path: PathBuf,
        started: usize,
        total_running: usize,
    },
    Doctored {
        path: PathBuf,
        checks: Vec<ChannelDoctorCheck>,
        healthy: usize,
        unhealthy: usize,
    },
    Added {
        path: PathBuf,
        channel: ChannelRecord,
    },
    Removed {
        path: PathBuf,
        name: String,
        remaining: usize,
    },
    Served {
        channel_name: String,
        processed: u64,
    },
}

pub fn execute_channel_action(action: ChannelAction) -> Result<ChannelResult, String> {
    // Serve는 실시간 어댑터와 에이전트가 필요하므로 별도 처리.
    if let ChannelAction::Serve { poll_interval_secs } = action {
        return execute_channel_serve(poll_interval_secs);
    }
    let path = resolve_env_path(
        ENV_CHANNEL_STORE_PATH,
        Path::new(DEFAULT_CHANNEL_STORE_PATH),
    )?;
    execute_channel_action_at(action, &path, unix_now_seconds())
}

fn execute_channel_serve(poll_interval_secs: u64) -> Result<ChannelResult, String> {
    use crate::channel_serve::run_channel_serve_loop;
    use crate::estop::EStop;
    use std::sync::Arc;
    use std::time::Duration;

    // 채널 스토어에서 Telegram 설정 로드
    let path = resolve_env_path(
        ENV_CHANNEL_STORE_PATH,
        Path::new(DEFAULT_CHANNEL_STORE_PATH),
    )?;
    let store = load_store(&path)?;

    let telegram_record = store
        .channels
        .iter()
        .find(|c| c.channel_type == ChannelKind::Telegram)
        .ok_or_else(|| {
            String::from(
                "no telegram channel configured; run 'channel add telegram bot_token=<token>,allowed_users=<id>' first",
            )
        })?;

    let cfg = parse_channel_config(&telegram_record.config);
    let bot_token = cfg
        .get("bot_token")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| String::from("missing bot_token in telegram config"))?
        .clone();
    let allowed_users = cfg
        .get("allowed_users")
        .map(|v| v.split(':').map(String::from).collect::<Vec<_>>())
        .unwrap_or_default();

    let tg_config = TelegramConfig::new(bot_token, allowed_users).map_err(|e| e.to_string())?;
    let channel_name = telegram_record.name.clone();

    let mut channel = TelegramChannelAdapter::live(tg_config)?;

    let agent: Arc<dyn axiom_adapters::contracts::AgentAdapter> = Arc::from(
        axiom_adapters::build_contract_agent("")
            .map_err(|e| format!("agent backend init failed: {e}"))?,
    );

    let interval = Duration::from_secs(poll_interval_secs.max(1));
    eprintln!(
        "channel serve started channel={channel_name} poll_interval_secs={}",
        interval.as_secs()
    );

    let estop = Arc::new(EStop::new());
    let rag_context: Option<Arc<dyn axiom_adapters::contracts::ContextAdapter>> =
        std::env::var("AXIOM_CONTEXT_ROOT")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .and_then(|root| {
                axiom_adapters::AxiommeContextAdapter::new(std::path::Path::new(root.trim()))
                    .map(|a| Arc::new(a) as Arc<dyn axiom_adapters::contracts::ContextAdapter>)
                    .map_err(|e| {
                        eprintln!("channel serve context adapter init failed (RAG disabled): {e}");
                        e
                    })
                    .ok()
            });
    let processed = run_channel_serve_loop(
        &mut channel,
        agent,
        Some(Arc::clone(&estop)),
        interval,
        None,
        rag_context,
    )?;
    estop.halt();

    Ok(ChannelResult::Served {
        channel_name,
        processed,
    })
}

fn execute_channel_action_at(
    action: ChannelAction,
    path: &Path,
    now: u64,
) -> Result<ChannelResult, String> {
    let mut store = load_store(path)?;
    match action {
        ChannelAction::List => {
            sort_channels(&mut store.channels);
            let running = store
                .channels
                .iter()
                .filter(|channel| channel.running)
                .count();
            Ok(ChannelResult::Listed {
                path: path.to_path_buf(),
                channels: store.channels,
                running,
            })
        }
        ChannelAction::Add {
            channel_type,
            config,
        } => {
            let channel_type = parse_channel_kind(&channel_type)?;
            validate_channel_config(&config)?;
            let name = channel_type.as_str().to_string();

            if store
                .channels
                .iter()
                .any(|channel| channel.name.eq_ignore_ascii_case(&name))
            {
                return Err(format!("channel '{name}' already exists"));
            }

            let channel = ChannelRecord {
                name,
                channel_type,
                config,
                running: false,
                last_health: None,
                last_checked_at: None,
                created_at: now,
                updated_at: now,
            };
            store.channels.push(channel.clone());
            sort_channels(&mut store.channels);
            save_store(path, &store)?;

            Ok(ChannelResult::Added {
                path: path.to_path_buf(),
                channel,
            })
        }
        ChannelAction::Remove { name } => {
            validate_channel_name(&name)?;
            let Some(index) = store
                .channels
                .iter()
                .position(|channel| channel.name.eq_ignore_ascii_case(&name))
            else {
                return Err(format!("channel '{name}' not found"));
            };

            let removed = store.channels.remove(index);
            sort_channels(&mut store.channels);
            save_store(path, &store)?;

            Ok(ChannelResult::Removed {
                path: path.to_path_buf(),
                name: removed.name,
                remaining: store.channels.len(),
            })
        }
        ChannelAction::Start => {
            if store.channels.is_empty() {
                return Err(String::from("no channels configured; add a channel first"));
            }

            let mut started = 0;
            for channel in &mut store.channels {
                if channel.running {
                    channel.last_checked_at = Some(now);
                    channel.updated_at = now;
                    continue;
                }

                match try_build_channel_adapter(channel.channel_type, &channel.config) {
                    Ok(()) => {
                        channel.running = true;
                        channel.last_health = Some(ChannelHealthStatus::Ok);
                        channel.last_checked_at = Some(now);
                        channel.updated_at = now;
                        started += 1;
                    }
                    Err(_reason) => {
                        channel.last_health = Some(ChannelHealthStatus::Unhealthy);
                        channel.last_checked_at = Some(now);
                        channel.updated_at = now;
                        // Do not set running = true; channel stays stopped
                    }
                }
            }
            sort_channels(&mut store.channels);
            save_store(path, &store)?;

            let total_running = store
                .channels
                .iter()
                .filter(|channel| channel.running)
                .count();
            Ok(ChannelResult::Started {
                path: path.to_path_buf(),
                started,
                total_running,
            })
        }
        ChannelAction::Doctor => {
            let mut checks = Vec::with_capacity(store.channels.len());
            let mut healthy = 0;
            let mut unhealthy = 0;

            for channel in &mut store.channels {
                let (status, detail) = health_for_channel(channel);
                if status == ChannelHealthStatus::Ok {
                    healthy += 1;
                } else {
                    unhealthy += 1;
                }

                channel.last_health = Some(status);
                channel.last_checked_at = Some(now);
                channel.updated_at = now;

                checks.push(ChannelDoctorCheck {
                    name: channel.name.clone(),
                    channel_type: channel.channel_type,
                    status,
                    detail,
                    checked_at: now,
                });
            }

            sort_channels(&mut store.channels);
            save_store(path, &store)?;

            Ok(ChannelResult::Doctored {
                path: path.to_path_buf(),
                checks,
                healthy,
                unhealthy,
            })
        }
        // Serve는 execute_channel_action에서 조기 처리되므로 여기 도달하지 않음.
        ChannelAction::Serve { .. } => Err(String::from(
            "internal: Serve action must be handled before execute_channel_action_at",
        )),
    }
}

fn health_for_channel(channel: &ChannelRecord) -> (ChannelHealthStatus, String) {
    let config = channel.config.trim();
    if config.is_empty() {
        return (ChannelHealthStatus::Unhealthy, String::from("empty_config"));
    }
    if config.contains('\n') || config.contains('\r') {
        return (
            ChannelHealthStatus::Unhealthy,
            String::from("multiline_config_not_allowed"),
        );
    }

    (ChannelHealthStatus::Ok, String::from("config_present"))
}

#[cfg(test)]
mod tests {
    use super::{ChannelAction, ChannelHealthStatus, ChannelResult, execute_channel_action_at};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_path(label: &str, extension: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axiom-channel-{label}-{}-{tick}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn channel_add_list_start_doctor_remove_flow() {
        let path = unique_path("flow", "db");

        let add = execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("telegram"),
                config: String::from("bot_token=t"),
            },
            &path,
            10,
        )
        .expect("add should succeed");
        match add {
            ChannelResult::Added { channel, .. } => {
                assert_eq!(channel.name, "telegram");
                assert_eq!(channel.channel_type.as_str(), "telegram");
                assert!(!channel.running);
            }
            _ => panic!("expected added result"),
        }

        let list = execute_channel_action_at(ChannelAction::List, &path, 11)
            .expect("list should succeed after add");
        match list {
            ChannelResult::Listed {
                channels, running, ..
            } => {
                assert_eq!(channels.len(), 1);
                assert_eq!(running, 0);
            }
            _ => panic!("expected listed result"),
        }

        let start = execute_channel_action_at(ChannelAction::Start, &path, 12)
            .expect("start should succeed after add");
        match start {
            ChannelResult::Started {
                started,
                total_running,
                ..
            } => {
                assert_eq!(started, 1);
                assert_eq!(total_running, 1);
            }
            _ => panic!("expected started result"),
        }

        let doctor = execute_channel_action_at(ChannelAction::Doctor, &path, 13)
            .expect("doctor should succeed");
        match doctor {
            ChannelResult::Doctored {
                checks,
                healthy,
                unhealthy,
                ..
            } => {
                assert_eq!(checks.len(), 1);
                assert_eq!(healthy, 1);
                assert_eq!(unhealthy, 0);
                assert_eq!(checks[0].status, ChannelHealthStatus::Ok);
            }
            _ => panic!("expected doctored result"),
        }

        let remove = execute_channel_action_at(
            ChannelAction::Remove {
                name: String::from("telegram"),
            },
            &path,
            14,
        )
        .expect("remove should succeed");
        match remove {
            ChannelResult::Removed { remaining, .. } => {
                assert_eq!(remaining, 0);
            }
            _ => panic!("expected removed result"),
        }

        let list = execute_channel_action_at(ChannelAction::List, &path, 15)
            .expect("list should still succeed");
        match list {
            ChannelResult::Listed { channels, .. } => {
                assert!(channels.is_empty());
            }
            _ => panic!("expected listed result"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn channel_add_duplicate_name_rejected() {
        let path = unique_path("duplicate", "db");
        execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("slack"),
                config: String::from("token=abc"),
            },
            &path,
            20,
        )
        .expect("initial add should succeed");

        let error = execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("slack"),
                config: String::from("token=def"),
            },
            &path,
            21,
        )
        .expect_err("duplicate add should fail");
        assert!(error.contains("already exists"), "error={error}");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn channel_remove_missing_name_rejected() {
        let path = unique_path("remove-missing", "db");
        let error = execute_channel_action_at(
            ChannelAction::Remove {
                name: String::from("unknown"),
            },
            &path,
            30,
        )
        .expect_err("missing remove should fail");

        assert!(error.contains("not found"), "error={error}");
    }

    #[test]
    fn channel_add_rejects_unsupported_type() {
        let path = unique_path("invalid-type", "db");
        let error = execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("sms"),
                config: String::from("token=abc"),
            },
            &path,
            40,
        )
        .expect_err("unsupported type should fail");
        assert!(error.contains("unsupported channel type"), "error={error}");
    }

    #[test]
    fn channel_add_accepts_extended_external_types() {
        let path = unique_path("extended-types", "db");

        let add_matrix = execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("matrix"),
                config: String::from("homeserver=matrix.org"),
            },
            &path,
            50,
        )
        .expect("matrix add should succeed");
        match add_matrix {
            ChannelResult::Added { channel, .. } => {
                assert_eq!(channel.name, "matrix");
                assert_eq!(channel.channel_type.as_str(), "matrix");
            }
            _ => panic!("expected added result"),
        }

        let add_whatsapp = execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("whatsapp"),
                config: String::from("phone=123"),
            },
            &path,
            51,
        )
        .expect("whatsapp add should succeed");
        match add_whatsapp {
            ChannelResult::Added { channel, .. } => {
                assert_eq!(channel.name, "whatsapp");
                assert_eq!(channel.channel_type.as_str(), "whatsapp");
            }
            _ => panic!("expected added result"),
        }

        let add_irc = execute_channel_action_at(
            ChannelAction::Add {
                channel_type: String::from("irc"),
                config: String::from("server=irc.libera.chat"),
            },
            &path,
            52,
        )
        .expect("irc add should succeed");
        match add_irc {
            ChannelResult::Added { channel, .. } => {
                assert_eq!(channel.name, "irc");
                assert_eq!(channel.channel_type.as_str(), "irc");
            }
            _ => panic!("expected added result"),
        }

        let _ = fs::remove_file(path);
    }
}
