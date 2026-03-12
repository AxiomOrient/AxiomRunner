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

pub fn global_async_runtime_host() -> &'static AsyncRuntimeHost {
    static HOST: OnceLock<AsyncRuntimeHost> = OnceLock::new();
    HOST.get_or_init(|| {
        AsyncRuntimeHost::from_env().unwrap_or_else(|error| {
            eprintln!("{error}");
            AsyncRuntimeHost {
                runtime: tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(DEFAULT_ASYNC_HOST_WORKER_THREADS)
                    .build()
                    .expect("fallback async runtime host should initialize"),
                semaphore: Arc::new(tokio::sync::Semaphore::new(
                    DEFAULT_ASYNC_HOST_MAX_IN_FLIGHT,
                )),
                timeout: None,
            }
        })
    })
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
    use super::AsyncRuntimeHost;

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
}
