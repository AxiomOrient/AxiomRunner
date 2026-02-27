use crate::contracts::{
    AdapterHealth, RuntimeAdapter as ContractRuntimeAdapter, RuntimeState as ContractRuntimeState,
    RuntimeTick as ContractRuntimeTick,
};
use crate::error::{AdapterError, AdapterResult, RetryClass};
use std::fmt;
use std::io::ErrorKind;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRequest {
    pub program: String,
    pub args: Vec<String>,
    pub timeout: Option<Duration>,
}

impl RuntimeRequest {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            timeout: None,
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    Spawn { program: String, message: String },
    Wait { program: String, message: String },
    Timeout { program: String, timeout: Duration },
    NonZeroExit { program: String, code: Option<i32> },
    Terminate { program: String, message: String },
    ProcessNotRunning { program: String },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Spawn { program, message } => {
                write!(f, "failed to spawn `{program}`: {message}")
            }
            RuntimeError::Wait { program, message } => {
                write!(f, "failed while waiting for `{program}`: {message}")
            }
            RuntimeError::Timeout { program, timeout } => {
                write!(
                    f,
                    "runtime command `{program}` timed out after {}ms",
                    timeout.as_millis()
                )
            }
            RuntimeError::NonZeroExit { program, code } => {
                write!(f, "runtime command `{program}` exited with {:?}", code)
            }
            RuntimeError::Terminate { program, message } => {
                write!(f, "failed to terminate `{program}`: {message}")
            }
            RuntimeError::ProcessNotRunning { program } => {
                write!(f, "runtime process `{program}` is not running")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeRuntime;

impl NativeRuntime {
    pub const fn new() -> Self {
        Self
    }

    pub fn start(&self, request: &RuntimeRequest) -> RuntimeResult<NativeProcess> {
        let mut command = Command::new(&request.program);
        command.args(&request.args);

        let child = command.spawn().map_err(|error| RuntimeError::Spawn {
            program: request.program.clone(),
            message: error.to_string(),
        })?;

        Ok(NativeProcess {
            program: request.program.clone(),
            child: Some(child),
        })
    }

    pub fn execute(&self, request: &RuntimeRequest) -> RuntimeResult<ExecutionResult> {
        let mut process = self.start(request)?;
        self.wait(&mut process, request.timeout)
    }

    pub fn wait(
        &self,
        process: &mut NativeProcess,
        timeout: Option<Duration>,
    ) -> RuntimeResult<ExecutionResult> {
        let mut child = process
            .child
            .take()
            .ok_or_else(|| RuntimeError::ProcessNotRunning {
                program: process.program.clone(),
            })?;

        let started = Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        return Ok(ExecutionResult {
                            exit_code: status.code().unwrap_or(0),
                        });
                    }

                    return Err(RuntimeError::NonZeroExit {
                        program: process.program.clone(),
                        code: status.code(),
                    });
                }
                Ok(None) => {
                    if let Some(limit) = timeout
                        && started.elapsed() >= limit
                    {
                        match child.kill() {
                            Ok(()) => {}
                            Err(error) if error.kind() == ErrorKind::InvalidInput => {
                                if let Ok(Some(status)) = child.try_wait() {
                                    if status.success() {
                                        return Ok(ExecutionResult {
                                            exit_code: status.code().unwrap_or(0),
                                        });
                                    }
                                    return Err(RuntimeError::NonZeroExit {
                                        program: process.program.clone(),
                                        code: status.code(),
                                    });
                                }
                            }
                            Err(error) => {
                                return Err(RuntimeError::Terminate {
                                    program: process.program.clone(),
                                    message: error.to_string(),
                                });
                            }
                        }

                        let _ = child.wait();

                        return Err(RuntimeError::Timeout {
                            program: process.program.clone(),
                            timeout: limit,
                        });
                    }

                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(RuntimeError::Wait {
                        program: process.program.clone(),
                        message: error.to_string(),
                    });
                }
            }
        }
    }

    pub fn terminate(&self, process: &mut NativeProcess) -> RuntimeResult<()> {
        let mut child = process
            .child
            .take()
            .ok_or_else(|| RuntimeError::ProcessNotRunning {
                program: process.program.clone(),
            })?;

        if let Ok(Some(_)) = child.try_wait() {
            let _ = child.wait();
            return Ok(());
        }

        child.kill().map_err(|error| RuntimeError::Terminate {
            program: process.program.clone(),
            message: error.to_string(),
        })?;

        child.wait().map_err(|error| RuntimeError::Wait {
            program: process.program.clone(),
            message: error.to_string(),
        })?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct NativeRuntimeAdapter {
    runtime: NativeRuntime,
    request: RuntimeRequest,
    process: Option<NativeProcess>,
    state: ContractRuntimeState,
    step: u64,
}

impl NativeRuntimeAdapter {
    pub fn new(request: RuntimeRequest) -> Self {
        Self {
            runtime: NativeRuntime::new(),
            request,
            process: None,
            state: ContractRuntimeState::Stopped,
            step: 0,
        }
    }

    pub fn with_runtime(runtime: NativeRuntime, request: RuntimeRequest) -> Self {
        Self {
            runtime,
            request,
            process: None,
            state: ContractRuntimeState::Stopped,
            step: 0,
        }
    }
}

impl ContractRuntimeAdapter for NativeRuntimeAdapter {
    fn id(&self) -> &str {
        "runtime.native"
    }

    fn health(&self) -> AdapterHealth {
        if self.state == ContractRuntimeState::Running {
            AdapterHealth::Healthy
        } else {
            AdapterHealth::Degraded
        }
    }

    fn start(&mut self) -> AdapterResult<()> {
        if self.state == ContractRuntimeState::Running {
            return Ok(());
        }

        let process = self
            .runtime
            .start(&self.request)
            .map_err(|error| map_runtime_error("runtime.start", error))?;
        self.process = Some(process);
        self.state = ContractRuntimeState::Running;
        self.step = 0;
        Ok(())
    }

    fn tick(&mut self) -> AdapterResult<ContractRuntimeTick> {
        if self.state != ContractRuntimeState::Running {
            return Err(AdapterError::unavailable("runtime", "is not running"));
        }

        self.step = self.step.saturating_add(1);

        if let Some(process) = self.process.as_mut()
            && let Some(child) = process.child.as_mut()
        {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.process = None;
                    self.state = ContractRuntimeState::Stopped;
                    if status.success() {
                        return Ok(ContractRuntimeTick {
                            step: self.step,
                            state: self.state,
                        });
                    }

                    return Err(AdapterError::failed(
                        "runtime.tick",
                        format!("process exited with {:?}", status.code()),
                        RetryClass::NonRetryable,
                    ));
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(AdapterError::failed(
                        "runtime.tick",
                        error.to_string(),
                        RetryClass::Retryable,
                    ));
                }
            }
        }

        Ok(ContractRuntimeTick {
            step: self.step,
            state: self.state,
        })
    }

    fn stop(&mut self) -> AdapterResult<()> {
        if self.state == ContractRuntimeState::Stopped {
            return Ok(());
        }

        if let Some(process) = self.process.as_mut() {
            self.runtime
                .terminate(process)
                .map_err(|error| map_runtime_error("runtime.stop", error))?;
        }

        self.process = None;
        self.state = ContractRuntimeState::Stopped;
        Ok(())
    }

    fn state(&self) -> ContractRuntimeState {
        self.state
    }
}

#[derive(Debug)]
pub struct NativeProcess {
    program: String,
    child: Option<Child>,
}

impl NativeProcess {
    pub fn id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }
}

fn map_runtime_error(operation: &'static str, error: RuntimeError) -> AdapterError {
    let retry = match error {
        RuntimeError::Spawn { .. } => RetryClass::Retryable,
        RuntimeError::Wait { .. } => RetryClass::Retryable,
        RuntimeError::Timeout { .. } => RetryClass::Retryable,
        RuntimeError::NonZeroExit { .. } => RetryClass::NonRetryable,
        RuntimeError::Terminate { .. } => RetryClass::Retryable,
        RuntimeError::ProcessNotRunning { .. } => RetryClass::NonRetryable,
    };

    AdapterError::failed(operation, error.to_string(), retry)
}
