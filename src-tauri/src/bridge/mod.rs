//! Typed Clay server ↔ webview bridge (Plan 097 Phase 3).
//!
//! Architecture in one paragraph: the desktop shell owns a single
//! [`session::BridgeSession`] that connects to the Clay server as a regular
//! IPC client through `clay::client` (handshake, optimistic edit queue,
//! staleness validation all reused, never duplicated). Server-side events are
//! translated 1:1 into serde-typed envelopes and delivered to the webview
//! through Tauri channels; frontend requests arrive as typed commands, are
//! size-capped and sanitized, stamped with the session's server-issued
//! identity, and forwarded over the client's bounded queue. The webview never
//! sees archive bytes, frame codecs, socket state, or protocol versioning.

pub mod agent;
pub mod dto;
pub mod editor;
pub mod errors;
pub mod forwarder;
pub mod layout;
pub mod session;

pub use agent::{AgentRelay, AgentStreamEvent};
pub use dto::{BootstrapDto, BridgeEnvelope, ThemeSnapshotDto, TypographySnapshotDto};
pub use errors::BridgeError;
pub use session::BridgeState;
