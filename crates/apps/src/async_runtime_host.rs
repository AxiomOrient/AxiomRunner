use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::env_util::read_env_trimmed;

const ENV_ASYNC_HOST_WORKER_THREADS: &str = "AXONRUNNER_ASYNC_HOST_WORKER_THREADS";
const ENV_ASYNC_HOST_MAX_IN_FLIGHT: &str = "AXONRUNNER_ASYNC_HOST_MAX_IN_FLIGHT";
const ENV_ASYNC_HOST_TIMEOUT_MS: &str = "AXONRUNNER_ASYNC_HOST_TIMEOUT_MS";

const DEFAULT_ASYNC_HOST_WORKER_THREADS: usize = 2;
const DEFAULT_ASYNC_HOST_MAX_IN_FLIGHT: usize = 8;

pub struct AsyncRuntimeHost {
    runtime: tokio::runtime::Runtime,
    semaphore: Arc<tokio::sync::Semaphore>,
    timeout: Option<Duration>,
    worker_threads: usize,
    max_in_flight: usize,
    init_mode: &'static str,
}

impl AsyncRuntimeHost {
    fn from_env() -> Result<Self, String> {
        let worker_threads = env_usize(
            ENV_ASYNC_HOST_WORKER_THREADS,
            DEFAULT_ASYNC_HOST_WORKER_THREADS,
        );
        let max_in_flight = env_usize(
            ENV_ASYNC_HOST_MAX_IN_FLIGHT,
            DEFAULT_ASYNC_HOST_MAX_IN_FLIGHT,
        );
        let timeout = env_u64(ENV_ASYNC_HOST_TIMEOUT_MS).map(Duration::from_millis);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(worker_threads.max(1))
            .build()
            .map_err(|error| format!("async runtime host init failed: {error}"))?;

        Ok(Self {
            runtime,
            semaphore: Arc::new(tokio::sync::Semaphore::new(max_in_flight.max(1))),
            timeout,
            worker_threads: worker_threads.max(1),
            max_in_flight: max_in_flight.max(1),
            init_mode: "configured",
        })
    }

    pub fn block_on_async<T, F>(&self, label: &'static str, future: F) -> Result<T, String>
    where
        F: Future<Output = Result<T, String>>,
    {
        let semaphore = Arc::clone(&self.semaphore);
        let timeout = self.timeout;
        self.runtime.block_on(async move {
            let permit = semaphore
                .acquire_owned()
                .await
                .map_err(|_| format!("{label}: semaphore closed"))?;

            let result = if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, future)
                    .await
                    .map_err(|_| format!("{label}: timed out after {}ms", timeout.as_millis()))?
            } else {
                future.await
            };

            drop(permit);
            result
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncRuntimeHostStatus {
    pub init_mode: &'static str,
    pub worker_threads: usize,
    pub max_in_flight: usize,
    pub timeout_ms: Option<u64>,
    pub init_error: Option<String>,
}

impl AsyncRuntimeHost {
    pub fn status(&self) -> AsyncRuntimeHostStatus {
        AsyncRuntimeHostStatus {
            init_mode: self.init_mode,
            worker_threads: self.worker_threads,
            max_in_flight: self.max_in_flight,
            timeout_ms: self.timeout.map(|timeout| timeout.as_millis() as u64),
            init_error: None,
        }
    }
}

pub fn global_async_runtime_host() -> Result<&'static AsyncRuntimeHost, String> {
    match async_runtime_host_init_result() {
        Ok(host) => Ok(host),
        Err(error) => Err(error.clone()),
    }
}

pub fn global_async_runtime_host_status() -> AsyncRuntimeHostStatus {
    match async_runtime_host_init_result() {
        Ok(host) => host.status(),
        Err(error) => AsyncRuntimeHostStatus {
            init_mode: "failed",
            worker_threads: 0,
            max_in_flight: 0,
            timeout_ms: None,
            init_error: Some(error.clone()),
        },
    }
}

fn async_runtime_host_init_result() -> &'static Result<AsyncRuntimeHost, String> {
    static HOST: OnceLock<Result<AsyncRuntimeHost, String>> = OnceLock::new();
    HOST.get_or_init(AsyncRuntimeHost::from_env)
}

fn env_usize(key: &str, default: usize) -> usize {
    read_env_trimmed(key)
        .ok()
        .flatten()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(default)
}

fn env_u64(key: &str) -> Option<u64> {
    read_env_trimmed(key)
        .ok()
        .flatten()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|&value| value > 0)
}

#[cfg(test)]
mod tests {
    use super::{AsyncRuntimeHost, AsyncRuntimeHostStatus};

    #[test]
    fn block_on_async_executes_future() {
        let host = AsyncRuntimeHost::from_env().expect("host should init");
        let value = host
            .block_on_async("test.block_on_async_executes_future", async {
                Ok::<_, String>(99_u64)
            })
            .expect("future should run");
        assert_eq!(value, 99);
    }

    #[test]
    fn failed_status_carries_init_error_without_fallback_runtime() {
        let status = AsyncRuntimeHostStatus {
            init_mode: "failed",
            worker_threads: 0,
            max_in_flight: 0,
            timeout_ms: None,
            init_error: Some(String::from("async runtime host init failed: boom")),
        };
        assert_eq!(status.init_mode, "failed");
        assert_eq!(status.worker_threads, 0);
        assert_eq!(
            status.init_error.as_deref(),
            Some("async runtime host init failed: boom")
        );
    }
}
