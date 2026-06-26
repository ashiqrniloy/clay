use std::{error::Error, fmt, path::Path, process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{Instant, timeout},
};

const PROTOCOL_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEvaluation {
    pub value: Value,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub struct RuntimeSandboxSupervisor {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    max_payload_bytes: usize,
}

impl RuntimeSandboxSupervisor {
    pub async fn spawn(
        executable: impl AsRef<Path>,
        max_payload_bytes: usize,
    ) -> Result<Self, RuntimeSandboxError> {
        let mut child = Command::new(executable.as_ref())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(RuntimeSandboxError::spawn)?;
        let stdin = child.stdin.take().ok_or(RuntimeSandboxError::MissingPipe)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(RuntimeSandboxError::MissingPipe)?;
        let mut supervisor = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            max_payload_bytes,
        };
        supervisor.handshake().await?;
        Ok(supervisor)
    }

    pub async fn evaluate(
        &mut self,
        source: &str,
        request_timeout: Duration,
    ) -> Result<SandboxEvaluation, RuntimeSandboxError> {
        self.write_frame(json!({ "kind": "evaluate", "source": source }))
            .await?;
        let started = Instant::now();
        let response = match timeout(request_timeout, self.read_frame()).await {
            Ok(response) => response?,
            Err(_) => {
                self.kill().await;
                return Err(RuntimeSandboxError::Timeout);
            }
        };
        match response.get("kind").and_then(Value::as_str) {
            Some("evaluation") => Ok(SandboxEvaluation {
                value: response.get("value").cloned().unwrap_or(Value::Null),
                elapsed: started.elapsed(),
            }),
            Some("diagnostic") => Err(RuntimeSandboxError::ChildDiagnostic(
                response
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("sandbox evaluation failed")
                    .to_owned(),
            )),
            _ => Err(RuntimeSandboxError::Protocol(
                "unexpected response".to_owned(),
            )),
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), RuntimeSandboxError> {
        self.write_frame(json!({ "kind": "shutdown" })).await
    }

    async fn handshake(&mut self) -> Result<(), RuntimeSandboxError> {
        self.write_frame(json!({
            "kind": "handshake",
            "protocolVersion": PROTOCOL_VERSION,
            "maxPayloadBytes": self.max_payload_bytes,
        }))
        .await?;
        let response = self.read_frame().await?;
        if response.get("kind").and_then(Value::as_str) == Some("ready")
            && response.get("protocolVersion").and_then(Value::as_u64) == Some(PROTOCOL_VERSION)
        {
            Ok(())
        } else {
            Err(RuntimeSandboxError::Protocol(
                "sandbox handshake failed".to_owned(),
            ))
        }
    }

    async fn write_frame(&mut self, value: Value) -> Result<(), RuntimeSandboxError> {
        let mut frame = serde_json::to_vec(&value).map_err(RuntimeSandboxError::json)?;
        if frame.len() > self.max_payload_bytes {
            return Err(RuntimeSandboxError::PayloadTooLarge {
                len: frame.len(),
                max: self.max_payload_bytes,
            });
        }
        frame.push(b'\n');
        self.stdin
            .write_all(&frame)
            .await
            .map_err(RuntimeSandboxError::io)?;
        self.stdin.flush().await.map_err(RuntimeSandboxError::io)
    }

    async fn read_frame(&mut self) -> Result<Value, RuntimeSandboxError> {
        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .await
            .map_err(RuntimeSandboxError::io)?;
        if bytes == 0 {
            return Err(RuntimeSandboxError::Protocol(
                "sandbox child exited".to_owned(),
            ));
        }
        if bytes > self.max_payload_bytes {
            self.kill().await;
            return Err(RuntimeSandboxError::PayloadTooLarge {
                len: bytes,
                max: self.max_payload_bytes,
            });
        }
        serde_json::from_str(&line).map_err(RuntimeSandboxError::json)
    }

    async fn kill(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

impl Drop for RuntimeSandboxSupervisor {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[derive(Debug)]
pub enum RuntimeSandboxError {
    Spawn(String),
    MissingPipe,
    Io(std::io::Error),
    Json(String),
    Protocol(String),
    ChildDiagnostic(String),
    PayloadTooLarge { len: usize, max: usize },
    Timeout,
}

impl RuntimeSandboxError {
    fn spawn(error: std::io::Error) -> Self {
        Self::Spawn(error.to_string())
    }

    fn io(error: std::io::Error) -> Self {
        Self::Io(error)
    }

    fn json(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

impl fmt::Display for RuntimeSandboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "failed to spawn runtime sandbox: {error}"),
            Self::MissingPipe => formatter.write_str("runtime sandbox stdio pipe missing"),
            Self::Io(error) => write!(formatter, "runtime sandbox I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "runtime sandbox JSON failed: {error}"),
            Self::Protocol(error) => write!(formatter, "runtime sandbox protocol failed: {error}"),
            Self::ChildDiagnostic(error) => {
                write!(formatter, "runtime sandbox child failed: {error}")
            }
            Self::PayloadTooLarge { len, max } => {
                write!(
                    formatter,
                    "runtime sandbox payload {len} exceeds maximum {max}"
                )
            }
            Self::Timeout => formatter.write_str("runtime sandbox evaluation timed out"),
        }
    }
}

impl Error for RuntimeSandboxError {}
