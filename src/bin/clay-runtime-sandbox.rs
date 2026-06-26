use std::error::Error;

use deno_core::{JsRuntime, RuntimeOptions, serde_v8, v8};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: u64 = 1;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();
    let mut max_payload_bytes = 1024 * 1024;

    while let Some(line) = lines.next_line().await? {
        let request: Value = serde_json::from_str(&line)?;
        match request.get("kind").and_then(Value::as_str) {
            Some("handshake") => {
                max_payload_bytes = request
                    .get("maxPayloadBytes")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(max_payload_bytes);
                write_response(
                    &mut stdout,
                    max_payload_bytes,
                    json!({ "kind": "ready", "protocolVersion": PROTOCOL_VERSION }),
                )
                .await?;
            }
            Some("evaluate") => {
                let source = request.get("source").and_then(Value::as_str).unwrap_or("");
                let response = match evaluate(source) {
                    Ok(value) => json!({ "kind": "evaluation", "value": value }),
                    Err(error) => json!({
                        "kind": "diagnostic",
                        "code": "clay.runtime.sandbox_child",
                        "message": sanitize(&error),
                    }),
                };
                write_response(&mut stdout, max_payload_bytes, response).await?;
            }
            Some("shutdown") => break,
            _ => {
                write_response(
                    &mut stdout,
                    max_payload_bytes,
                    json!({
                        "kind": "diagnostic",
                        "code": "clay.runtime.sandbox_protocol",
                        "message": "unsupported sandbox request",
                    }),
                )
                .await?;
            }
        }
    }

    Ok(())
}

fn evaluate(source: &str) -> Result<Value, String> {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let global = runtime
        .execute_script("clay://sandbox/controlled.js", source.to_owned())
        .map_err(|error| error.to_string())?;
    deno_core::scope!(scope, runtime);
    let local = v8::Local::new(scope, global);
    serde_v8::from_v8::<Value>(scope, local).map_err(|error| error.to_string())
}

async fn write_response(
    stdout: &mut tokio::io::Stdout,
    _max_payload_bytes: usize,
    value: Value,
) -> Result<(), Box<dyn Error>> {
    let mut frame = serde_json::to_vec(&value)?;
    frame.push(b'\n');
    stdout.write_all(&frame).await?;
    stdout.flush().await?;
    Ok(())
}

fn sanitize(error: &str) -> String {
    let first_line = error.lines().next().unwrap_or("sandbox evaluation failed");
    first_line.chars().take(160).collect()
}
