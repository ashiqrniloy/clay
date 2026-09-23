pub mod agent;
pub mod behavior;
pub mod caret;
pub mod codec;
pub mod completion;
pub mod decorations;
pub mod diagnostics;
pub mod document;
pub mod editor_control;
pub mod editor_rules;
pub mod folding;
pub mod language_intelligence;
pub mod launcher;
pub mod menu;
pub mod messages;
pub mod parse;
pub mod runtime;
pub mod sdui;
pub mod shell;
pub mod textobjects;
pub mod theme;
pub mod typography;

pub use agent::*;
pub use behavior::*;
pub use caret::*;
pub use codec::*;
pub use completion::*;
pub use decorations::*;
pub use diagnostics::*;
pub use document::*;
pub use editor_control::*;
pub use editor_rules::*;
pub use folding::*;
pub use language_intelligence::*;
pub use launcher::*;
pub use menu::*;
pub use messages::*;
pub use parse::*;
pub use runtime::*;
pub use sdui::*;
pub use shell::*;
pub use textobjects::*;
pub use theme::*;
pub use typography::*;

/// Current wire protocol version for the local Clay IPC boundary.
///
/// Version 2 added `DecorationViewportRequest`; version 3 adds grouped native
/// decoration chunks and removes grammar-recovery diagnostics. Version 5 adds
/// `DecorationBatch` so one parse update's chunks ship in a single frame.
/// Version 7 adds `SelectionQueryRequest`/`SelectionQueryResult` for
/// tree-sitter text objects and smart select (Plan 071 task 10).
/// Version 8 adds `EditorCommandRequest` for the gated `editor-control`
/// programmatic execution channel (Plan 071 follow-up round).
/// Version 9 adds `CaretStyleOverride` so the gated `clientSetCursorStyle`
/// runtime override reaches the client (Plan 071 caret-transport fix).
/// Version 10 adds `ShellPreferences` so the `setPaneFocusPolicy` configuration
/// option reaches the client shell widget (Phase 22.1).
/// Version 11 adds the server-authoritative tab registry: `TabCommand`
/// (new/open-workspace/close/activate/reclaim) and the `TabRegistry` snapshot
/// broadcast so each tab's connection sees the same tab order/active tab
/// (Phase 22.3).
/// Version 13 adds tab reorder commands (`MoveLeft`/`MoveRight`/`MoveTo`)
/// so registry order becomes server-authoritative and reorderable via the
/// Phase 22.4 keyboard tab commands.
/// Version 15 defers `InitialDocument` and the initial SDUI/file-browser
/// snapshot until the connection binds a tab with `TabCommand::New` or
/// `TabCommand::Reclaim`.
/// Version 16 (Phase 24.3) adds the generic semantic `MenuBackspace` intent
/// and the `MenuActivate` activation kind (`Primary`/`Secondary`).
/// Version 17 (Phase 24.4) adds `TransientMenuOriginData::Centered` so
/// command/path mode snapshots can select the window-centered Command Centre
/// surface (client-side layout/presentation only).
/// Version 18 (Phase 26) adds `EditorLayoutOverride` so the user-owned
/// `setEditorLayout` wrap-policy override (init.js / configuration reload)
/// reaches every client editor surface, beating the per-mode manifest.
/// Version 19 (Phase 28.2) adds `EditorBehaviorRules.heading_prefixes` so
/// heading rotate is package data (no ATX literals in Rust).
/// Version 20 (Phase 28.3) adds `FoldingRangeSet` (publication capped by
/// `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`) so validated folds reach the client;
/// collapse state stays client-local.
/// Version 21 (Phase 28.4) adds `DecorationKind::Link` plus optional
/// `DecorationTarget` on `DecorationSpan`.
/// Version 22 (Phase 28.5) adds `DecorationKind::InlayHint`, inlay payload,
/// and `EditorChrome.inlay_hints`.
/// Version 23 (Phase 28.6) adds bounded completion recency hints to
/// `CompletionRequest`; the ring is process-local and never persisted.
/// Version 24 (Phase 25) adds boxed `ClientMessage::Agent` /
/// `ServerMessage::Agent` for the core-owned clay-agent host. Event union
/// includes unused tool/permission variants for Phase 29.
/// Version 25 publishes optional `PackageUiSnapshot.empty_tab` for the
/// generic pane-content contribution.
/// Version 26 carries complete validated package panels, overlays, components,
/// input routes, and host-stamped provenance/trust labels.
/// Version 27 replaces whole-document snapshot text with bounded document
/// heads and adds pull-based, versioned document chunk messages.
/// Version 28 adds optional content-free performance trace IDs to viewport,
/// parse, and decoration messages.
/// Version 29 replaces the heuristic `DecorationViewportRequest` with the
/// atomic `ViewportRenderRequest`/`ViewportRenderPatch` pair: one bounded
/// patch envelope (with covered ranges, ordered decoration/diagnostic/fold
/// members, and a complete/empty/rejected status) answers each request id.
/// Version 30 adds the current book selection (`provider`, `model`) to
/// `AgentInventory` so a freshly mounted webview learns the configured
/// pair from the `listSessions` inventory snapshot instead of waiting for
/// a picker event.
/// Version 31 (plan 124) adds the palette's row fields: `TransientMenuItemData`
/// carries the item's `scope` tag (the `All · Session · Shell · Files` chips)
/// and its `bindings` (the per-row chord chips), and `MenuQueryUpdate` carries
/// the chip the client selected, so the server filters and selects over the
/// scoped item set.
/// Version 32 (plan 125) adds `TransientMenuSnapshotData.mode`: the bounded,
/// closed presentation vocabulary (`catalogue` | `path` | `picker` | `secret` |
/// `url` | `oauth`) that tells one `CommandPalette` sheet how to render its
/// session — a stage line, a shielded field for `secret` — now that pickers are
/// `CommandPalette` sessions too. Absent decodes as the catalogue. Plan 125 also
/// retires `TransientMenuOriginData::Centered` as a *produced* value: the
/// variant stays on the wire (older peers must decode), no constructor emits it,
/// and the shell's projection falls it in with the palette's bottom anchor.
pub const PROTOCOL_VERSION: u32 = 32;

pub type PerformanceTraceId = u64;

/// Monotonic request identity for the atomic viewport render protocol.
pub type ViewportRequestId = u64;

pub type ClientId = u64;

pub type DocumentId = u64;

pub type DocumentVersion = u64;

/// Phase 22.3: stable server-assigned tab identity. Tabs are real separate
/// client connections; the registry binds a `TabId` to a `ClientId` and a
/// workspace root. Survives client reconnects (the binding is re-pointed at the
/// reconnecting connection's `ClientId`).
pub type TabId = u64;

pub type BehaviorVersion = u64;

pub type TransactionId = u64;

pub type LeaseId = u64;

pub type RegionLockId = u64;

pub type WorkspaceRootId = u64;

/// Serde boundary helpers for wire fields that exceed JavaScript's
/// `Number.MAX_SAFE_INTEGER`. Menu session ids carry the server high bit
/// (`1 << 63`), so they cross the JSON bridge as strings, never numbers.
pub mod menu_session_id_serde {
    pub fn serialize<S: serde::Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        text.parse::<u64>()
            .map_err(|_| serde::de::Error::custom("menu session id must be a u64 decimal string"))
    }
}
