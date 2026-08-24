//! Phase 24.1: transient-menu interaction round-trip wire types.
//!
//! Server-owned transient menu sessions (the Control Center first; path mode
//! and future pickers later) are interactive over the protocol: the client
//! sends bounded query/selection/activate/cancel intents keyed by an opaque
//! server-allocated session id, and the server answers with bounded inert
//! snapshots. The client is a dumb renderer while a server-owned session is
//! active: it never mutates query/selection locally and never constructs
//! commands from menu data.
//!
//! Wire contract (Phase 24.1):
//! - [`TransientMenuSnapshotData`] carries inert display data ONLY — no
//!   callbacks, command payloads, paths, or authority fields. Activation is by
//!   opaque session id; the server holds the selected item's action.
//! - Every bounded field is clamped to the shared `TRANSIENT_MENU_MAX_*`
//!   budgets at construction (both the server build and the defensive client
//!   parse).
//! - Session ids are `u64`. Server-owned ids partition from client-local
//!   session ids via the high bit (`1 << 63`), an invariant enforced in
//!   `src/server/menu_sessions.rs` (Phase 24.1 task 5).
//! - Snapshot payloads stay under `DEFAULT_MAX_FRAME_SIZE` by construction:
//!   ≤ `TRANSIENT_MENU_MAX_ITEMS` items with bounded label/detail/
//!   accessibility strings, checked by `perf/baselines.rs`.
//!
//! `TransientMenuSession` (shell layer) is NOT serialized directly: it
//! carries client-local internals (`CompletionMenuAcceptAction`) that must
//! never cross the wire. This DTO is the stable protocol projection.

use crate::perf::budgets::{
    TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS, TRANSIENT_MENU_MAX_DETAIL_CHARS,
    TRANSIENT_MENU_MAX_ITEMS, TRANSIENT_MENU_MAX_LABEL_CHARS, TRANSIENT_MENU_MAX_QUERY_CHARS,
};

/// Char-count truncation shared by every bounded snapshot field.
fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

/// Wire projection of a menu item's display data. Actions stay server-side.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct TransientMenuItemData {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub accessibility_label: String,
}

impl TransientMenuItemData {
    /// Build with label/detail/accessibility clamped to the shared menu
    /// budgets (`TRANSIENT_MENU_MAX_LABEL_CHARS`, `_DETAIL_CHARS`,
    /// `_ACCESSIBILITY_LABEL_CHARS`).
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        detail: Option<String>,
        accessibility_label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: truncate(&label.into(), TRANSIENT_MENU_MAX_LABEL_CHARS),
            detail: detail.map(|d| truncate(&d, TRANSIENT_MENU_MAX_DETAIL_CHARS)),
            accessibility_label: truncate(
                &accessibility_label.into(),
                TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS,
            ),
        }
    }
}

/// Wire projection of the session status. `Cancelled` never crosses the wire:
/// a cancelled server session is removed and reported via
/// [`crate::protocol::ServerMessage::TransientMenuClosed`.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TransientMenuStatusData {
    Active,
    Empty { message: String },
}

/// Mirrors `TransientMenuFocusPolicy` (shell layer). Default `Modal` for
/// server-owned palettes; `Modeless` for future HUD-style pickers.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TransientMenuFocusPolicyData {
    Modal,
    Modeless,
}

/// Mirrors `TransientMenuOrigin` (shell layer): selects the overlay anchor
/// (`Bottom`/`Pointer`/`Main`) or, Phase 24.4, the window-centered Command
/// Centre surface (`Centered`). Additive: `CommandPalette` remains the
/// compatibility spelling for the bottom origin.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TransientMenuOriginData {
    CommandPalette,
    ContextMenu,
    MenuBar,
    Centered,
}

/// Activation kind for `ClientMessage::MenuActivate` (Phase 24.3).
/// `Primary` (Enter/Tab) activates the selected item; `Secondary`
/// (Alt+Enter) activates the session's secondary action for the selected
/// item (path mode: open the directory as the tab's workspace). Kind
/// semantics are interpreted server-side by the session kind, never by the
/// client. Closed enum: unknown archive values fail closed at decode.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TransientMenuActivationData {
    Primary,
    Secondary,
}

/// Bounded inert display snapshot of a server-owned transient menu session.
/// Boxed inside `ServerMessage` so the variant's inline size never inflates
/// the union floor that small payloads like `EditAck` pay.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct TransientMenuSnapshotData {
    #[serde(with = "crate::protocol::menu_session_id_serde")]
    pub session_id: u64,
    pub prompt: String,
    pub query: String,
    pub items: Vec<TransientMenuItemData>,
    pub selected_index: u32,
    pub status: TransientMenuStatusData,
    pub focus_policy: TransientMenuFocusPolicyData,
    pub origin: TransientMenuOriginData,
}

impl TransientMenuSnapshotData {
    /// Build a snapshot, clamping every bounded field to the shared menu
    /// budgets (`TRANSIENT_MENU_MAX_QUERY_CHARS`/`_LABEL_CHARS` for
    /// prompt/query, `_ITEMS` for the list). The server build and the
    /// defensive client parse both route through this constructor.
    #[allow(clippy::too_many_arguments)] // one positional arg per DTO field; mirrors the wire shape
    pub fn new(
        session_id: u64,
        prompt: impl Into<String>,
        query: impl Into<String>,
        items: Vec<TransientMenuItemData>,
        selected_index: u32,
        status: TransientMenuStatusData,
        focus_policy: TransientMenuFocusPolicyData,
        origin: TransientMenuOriginData,
    ) -> Self {
        Self {
            session_id,
            prompt: truncate(&prompt.into(), TRANSIENT_MENU_MAX_LABEL_CHARS),
            query: truncate(&query.into(), TRANSIENT_MENU_MAX_QUERY_CHARS),
            items: items.into_iter().take(TRANSIENT_MENU_MAX_ITEMS).collect(),
            selected_index,
            status: match status {
                TransientMenuStatusData::Empty { message } => TransientMenuStatusData::Empty {
                    message: truncate(&message, TRANSIENT_MENU_MAX_DETAIL_CHARS),
                },
                other => other,
            },
            focus_policy,
            origin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::codec::Codec;

    fn codec() -> Codec {
        Codec::default()
    }

    fn sample_snapshot() -> TransientMenuSnapshotData {
        TransientMenuSnapshotData::new(
            1 << 63 | 7,
            "Control Center",
            "reload",
            vec![TransientMenuItemData::new(
                "runtime.reloadConfiguration",
                "Reload Configuration",
                Some("Reload configuration files".to_string()),
                "Reload configuration",
            )],
            0,
            TransientMenuStatusData::Active,
            TransientMenuFocusPolicyData::Modal,
            TransientMenuOriginData::CommandPalette,
        )
    }

    #[test]
    fn snapshot_round_trips_through_codec() {
        let codec = codec();
        let message =
            crate::protocol::ServerMessage::TransientMenuSnapshot(Box::new(sample_snapshot()));
        let frame = codec.encode_server_message(&message).unwrap();
        let restored = codec.decode_server_message(&frame).unwrap();
        assert_eq!(restored, message);
    }

    #[test]
    fn centered_origin_snapshot_round_trips_through_codec() {
        let codec = codec();
        let mut snapshot = sample_snapshot();
        snapshot.origin = TransientMenuOriginData::Centered;
        let message = crate::protocol::ServerMessage::TransientMenuSnapshot(Box::new(snapshot));
        let frame = codec.encode_server_message(&message).unwrap();
        let restored = codec.decode_server_message(&frame).unwrap();
        assert_eq!(restored, message);
        let crate::protocol::ServerMessage::TransientMenuSnapshot(restored) = restored else {
            panic!("unexpected message variant");
        };
        assert_eq!(restored.origin, TransientMenuOriginData::Centered);
    }

    #[test]
    fn closed_round_trips_through_codec() {
        let codec = codec();
        let message = crate::protocol::ServerMessage::TransientMenuClosed { session_id: 7 };
        let frame = codec.encode_server_message(&message).unwrap();
        let restored = codec.decode_server_message(&frame).unwrap();
        assert_eq!(restored, message);
    }

    #[test]
    fn shell_command_request_round_trips_through_codec() {
        let codec = codec();
        let message = crate::protocol::ServerMessage::ShellClientCommandRequest {
            command_id: "shell.clientSplitPaneVertical".to_string(),
        };
        let frame = codec.encode_server_message(&message).unwrap();
        let restored = codec.decode_server_message(&frame).unwrap();
        assert_eq!(restored, message);
    }

    #[test]
    fn query_update_round_trips_through_codec() {
        let codec = codec();
        let message = crate::protocol::ClientMessage::MenuQueryUpdate {
            client_id: 42,
            session_id: 1 << 63 | 7,
            query: "reload config".to_string(),
        };
        let frame = codec.encode_client_message(&message).unwrap();
        let restored = codec.decode_client_message(&frame).unwrap();
        assert_eq!(restored, message);
    }

    #[test]
    fn selection_move_round_trips_through_codec() {
        let codec = codec();
        let message = crate::protocol::ClientMessage::MenuSelectionMove {
            client_id: 42,
            session_id: 1 << 63 | 7,
            delta: -1,
        };
        let frame = codec.encode_client_message(&message).unwrap();
        let restored = codec.decode_client_message(&frame).unwrap();
        assert_eq!(restored, message);
    }

    #[test]
    fn activate_and_cancel_round_trip_through_codec() {
        let codec = codec();
        let activate = crate::protocol::ClientMessage::MenuActivate {
            client_id: 42,
            session_id: 1 << 63 | 7,
            kind: TransientMenuActivationData::Primary,
        };
        let activate_secondary = crate::protocol::ClientMessage::MenuActivate {
            client_id: 42,
            session_id: 1 << 63 | 7,
            kind: TransientMenuActivationData::Secondary,
        };
        let cancel = crate::protocol::ClientMessage::MenuCancel {
            client_id: 42,
            session_id: 1 << 63 | 7,
        };
        for message in [activate, activate_secondary, cancel] {
            let frame = codec.encode_client_message(&message).unwrap();
            let restored = codec.decode_client_message(&frame).unwrap();
            assert_eq!(restored, message);
        }
    }

    #[test]
    fn backspace_intent_round_trips_through_codec() {
        let codec = codec();
        let message = crate::protocol::ClientMessage::MenuBackspace {
            client_id: 42,
            session_id: 1 << 63 | 7,
        };
        let frame = codec.encode_client_message(&message).unwrap();
        let restored = codec.decode_client_message(&frame).unwrap();
        assert_eq!(restored, message);
    }

    #[test]
    fn constructor_clamps_every_bounded_field() {
        let long: String = "x".repeat(10_000);
        let snapshot = TransientMenuSnapshotData::new(
            1,
            long.clone(),
            long.clone(),
            (0..10_000)
                .map(|i| {
                    TransientMenuItemData::new(
                        format!("id-{i}"),
                        long.clone(),
                        Some(long.clone()),
                        long.clone(),
                    )
                })
                .collect(),
            0,
            TransientMenuStatusData::Empty {
                message: long.clone(),
            },
            TransientMenuFocusPolicyData::Modal,
            TransientMenuOriginData::CommandPalette,
        );
        assert_eq!(
            snapshot.prompt.chars().count(),
            TRANSIENT_MENU_MAX_LABEL_CHARS
        );
        assert_eq!(
            snapshot.query.chars().count(),
            TRANSIENT_MENU_MAX_QUERY_CHARS
        );
        assert_eq!(snapshot.items.len(), TRANSIENT_MENU_MAX_ITEMS);
        for item in &snapshot.items {
            assert_eq!(item.label.chars().count(), TRANSIENT_MENU_MAX_LABEL_CHARS);
            assert_eq!(
                item.detail.as_ref().unwrap().chars().count(),
                TRANSIENT_MENU_MAX_DETAIL_CHARS
            );
            assert_eq!(
                item.accessibility_label.chars().count(),
                TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS
            );
        }
        // Non-string fields are preserved verbatim.
        assert_eq!(snapshot.session_id, 1);
        assert_eq!(snapshot.selected_index, 0);
        assert_eq!(
            snapshot.status,
            TransientMenuStatusData::Empty {
                message: "x".repeat(TRANSIENT_MENU_MAX_DETAIL_CHARS),
            }
        );
    }

    /// Phase 24.1 invariant: the boxed snapshot variant must not inflate the
    /// `ServerMessage` union floor. 176 bytes is the pre-24.1 floor (64-bit),
    /// driven by the largest existing non-boxed variant; a future large
    /// NON-boxed variant grows this and must justify itself.
    #[test]
    fn server_message_union_floor_does_not_grow() {
        assert_eq!(size_of::<crate::protocol::ServerMessage>(), 176);
    }
}
