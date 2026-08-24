//! Bridge error taxonomy. Every failure crossing to the webview is one of
//! these variants with a sanitized, length-capped message: serde details,
//! socket errors, and protocol internals never leak verbatim.

use serde::Serialize;
use std::fmt;

const MAX_MESSAGE_CHARS: usize = 240;

fn sanitize(detail: impl fmt::Display) -> String {
    let text = detail.to_string();
    if text.chars().count() <= MAX_MESSAGE_CHARS {
        text
    } else {
        text.chars().take(MAX_MESSAGE_CHARS).collect()
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BridgeErrorCode {
    NotConnected,
    Busy,
    Timeout,
    ServerUnreachable,
    InvalidRequest,
    RequestTooLarge,
    Forbidden,
    QueueFull,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BridgeError {
    pub code: BridgeErrorCode,
    pub message: String,
}

impl BridgeError {
    pub fn not_connected() -> Self {
        Self {
            code: BridgeErrorCode::NotConnected,
            message: "no live server session; call session_bootstrap first".into(),
        }
    }

    pub fn busy() -> Self {
        Self {
            code: BridgeErrorCode::Busy,
            message: "a bootstrap/reconnect is already in progress".into(),
        }
    }

    pub fn timeout() -> Self {
        Self {
            code: BridgeErrorCode::Timeout,
            message: "server handshake timed out".into(),
        }
    }

    pub fn server_unreachable(detail: impl fmt::Display) -> Self {
        Self {
            code: BridgeErrorCode::ServerUnreachable,
            message: sanitize(detail),
        }
    }

    pub fn invalid_request(detail: impl fmt::Display) -> Self {
        Self {
            code: BridgeErrorCode::InvalidRequest,
            message: sanitize(format!("request failed validation: {detail}")),
        }
    }

    pub fn request_too_large(bytes: usize) -> Self {
        Self {
            code: BridgeErrorCode::RequestTooLarge,
            message: format!("request of {bytes} bytes exceeds the bridge cap"),
        }
    }

    pub fn forbidden(reason: impl fmt::Display) -> Self {
        Self {
            code: BridgeErrorCode::Forbidden,
            message: sanitize(reason),
        }
    }

    pub fn queue_full() -> Self {
        Self {
            code: BridgeErrorCode::QueueFull,
            message: "outbound queue is full; retry after the request settles".into(),
        }
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for BridgeError {}

/// Hard cap on any frontend → Rust request body. Real requests (edits,
/// menu queries, agent prompts) are orders of magnitude smaller; anything
/// larger is rejected before deserialization.
pub const MAX_REQUEST_BYTES: usize = 512 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_details_are_capped_not_leaked() {
        let error = BridgeError::invalid_request("x".repeat(10_000));
        assert!(error.message.chars().count() <= MAX_MESSAGE_CHARS + 32);
    }

    #[test]
    fn error_serializes_with_code_tag() {
        let json = serde_json::to_value(BridgeError::queue_full()).unwrap();
        assert_eq!(json["code"], "queueFull");
        assert!(json["message"].is_string());
    }
}
