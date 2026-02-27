use std::sync::OnceLock;
use std::time::Duration;

pub(crate) struct AsyncHttpBridge {
    execution: HttpExecution,
}

pub(crate) struct HttpTextResponse {
    pub(crate) status: reqwest::StatusCode,
    pub(crate) body: String,
}

enum HttpExecution {
    Async {
        runtime: &'static tokio::runtime::Runtime,
        client: reqwest::Client,
    },
    Blocking {
        client: reqwest::blocking::Client,
    },
}

impl AsyncHttpBridge {
    pub(crate) fn with_timeouts(
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, String> {
        match shared_runtime() {
            Ok(runtime) => {
                let client = reqwest::Client::builder()
                    .connect_timeout(connect_timeout)
                    .timeout(request_timeout)
                    .build()
                    .map_err(|error| format!("http client init failed: {error}"))?;
                Ok(Self {
                    execution: HttpExecution::Async { runtime, client },
                })
            }
            Err(runtime_error) => {
                let client = reqwest::blocking::Client::builder()
                    .connect_timeout(connect_timeout)
                    .timeout(request_timeout)
                    .build()
                    .map_err(|error| {
                        format!(
                            "http client init failed: {error}; runtime fallback: {runtime_error}"
                        )
                    })?;
                Ok(Self {
                    execution: HttpExecution::Blocking { client },
                })
            }
        }
    }

    pub(crate) fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<HttpTextResponse, reqwest::Error> {
        match &self.execution {
            HttpExecution::Async { runtime, client } => runtime.block_on(async {
                let mut request = client.post(url);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }

                let response = request.json(body).send().await?;
                let status = response.status();
                let body = response.text().await?;
                Ok(HttpTextResponse { status, body })
            }),
            HttpExecution::Blocking { client } => {
                let mut request = client.post(url);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }

                let response = request.json(body).send()?;
                let status = response.status();
                let body = response.text()?;
                Ok(HttpTextResponse { status, body })
            }
        }
    }

    #[cfg(feature = "channel-matrix")]
    pub(crate) fn put_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<HttpTextResponse, reqwest::Error> {
        match &self.execution {
            HttpExecution::Async { runtime, client } => runtime.block_on(async {
                let mut request = client.put(url);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }

                let response = request.json(body).send().await?;
                let status = response.status();
                let body = response.text().await?;
                Ok(HttpTextResponse { status, body })
            }),
            HttpExecution::Blocking { client } => {
                let mut request = client.put(url);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }

                let response = request.json(body).send()?;
                let status = response.status();
                let body = response.text()?;
                Ok(HttpTextResponse { status, body })
            }
        }
    }

    #[cfg(feature = "channel-matrix")]
    pub(crate) fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<HttpTextResponse, reqwest::Error> {
        match &self.execution {
            HttpExecution::Async { runtime, client } => runtime.block_on(async {
                let mut request = client.get(url);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }

                let response = request.send().await?;
                let status = response.status();
                let body = response.text().await?;
                Ok(HttpTextResponse { status, body })
            }),
            HttpExecution::Blocking { client } => {
                let mut request = client.get(url);
                for (name, value) in headers {
                    request = request.header(*name, *value);
                }

                let response = request.send()?;
                let status = response.status();
                let body = response.text()?;
                Ok(HttpTextResponse { status, body })
            }
        }
    }
}

impl Default for AsyncHttpBridge {
    fn default() -> Self {
        Self::with_timeouts(Duration::from_secs(5), Duration::from_secs(30)).unwrap_or_else(|_| {
            Self {
                execution: HttpExecution::Blocking {
                    client: reqwest::blocking::Client::new(),
                },
            }
        })
    }
}

fn shared_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .ok()
    });
    runtime
        .as_ref()
        .ok_or_else(|| String::from("shared runtime init failed"))
}

#[cfg(test)]
mod tests {
    use super::AsyncHttpBridge;
    use std::time::Duration;

    #[test]
    fn post_json_rejects_invalid_url() {
        let bridge = AsyncHttpBridge::with_timeouts(Duration::from_secs(1), Duration::from_secs(1))
            .expect("bridge should initialize");
        let body = serde_json::json!({"hello": "world"});
        let result = bridge.post_json("not-a-valid-url", &[], &body);
        assert!(result.is_err());
    }
}
