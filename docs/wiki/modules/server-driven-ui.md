# Server-Driven UI Protocol Schema

## Source

- `src/protocol/sdui.rs`
- `src/protocol/mod.rs`
- `src/protocol/sdui.rs` unit tests

## Overview

The initial server-driven UI (SDUI) schema defines an inert, typed Rust protocol model for UI trees before Phase 12 adds protocol messages, server snapshot generation, or Masonry rendering. The server will own declarative UI state and publish trees/updates; the client remains responsible for native rendering, input handling, focus, caret, viewport, and other transient widget state.

## Responsibilities

- Represent stable SDUI node IDs, tree versions, and bounded tree updates.
- Model panels, labels, buttons, lists, document-bound editor views, flex containers, and stack containers.
- Express user actions as server-routed command intents with typed metadata.
- Avoid carrying client-executable scripts or document text payloads in UI nodes.

## How It Works

`SduiTree` stores a `ui_version`, a `root_id`, and a flat list of `SduiNode` values. Each node has a stable `SduiNodeId` and a `SduiNodeKind`. Container nodes refer to children by ID instead of nesting widget state directly, so later reconciliation can replace or remove nodes by stable ID.

`SduiNodeKind::EditorView` uses `SduiEditorBinding` with a `DocumentId` and optional expected document version. The editor view schema never embeds full document text; existing document snapshot/edit protocol messages remain responsible for text synchronization.

`SduiActionIntent` represents button and list-item activations as inert command IDs, sources, and typed arguments. These intents are routed back to the server in later tasks instead of executing filesystem, network, shell, extension-loading, WASM, AI mutation, or client-side JavaScript authority on the client.

`SduiTreeUpdate` and `SduiTreeOperation` define the first update shape: replace the root, replace a node, or remove a node against explicit base/new UI versions. The schema stays separate from the `rkyv` codec boundary even though payload types derive `Archive`, `Serialize`, and `Deserialize` for future protocol use.

## Code Examples

```rust
use clay::protocol::{
    SduiEditorBinding, SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
};

let root_id = SduiNodeId(1);
let editor_id = SduiNodeId(2);
let tree = SduiTree {
    ui_version: 1,
    root_id,
    nodes: vec![
        SduiNode::new(
            root_id,
            SduiNodeKind::Flex {
                direction: SduiFlexDirection::Row,
                children: vec![editor_id],
            },
        ),
        SduiNode::new(
            editor_id,
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: 42,
                    expected_version: None,
                },
            },
        ),
    ],
};
```

## Invariants and Constraints

- `SduiNodeId` values are stable reconciliation keys, not Masonry widget IDs.
- SDUI schema state is server-owned declarative state; client-owned native widget state remains outside the schema.
- Editor views bind to documents by ID/version and do not serialize full document contents.
- Actions are command intents only and do not contain executable code or permission-bearing authorities.
- Codec and wire-message decisions remain outside `src/protocol/sdui.rs` until the protocol-message task wires snapshots and updates.

## Tests

- `src/protocol/sdui.rs`: `sdui_schema_represents_initial_widget_kinds` validates all initial widget/layout kinds.
- `src/protocol/sdui.rs`: `sdui_editor_view_uses_document_binding_not_text_payload` validates editor binding without embedded text.
- `src/protocol/sdui.rs`: `sdui_actions_are_server_routed_intents` validates inert command intent shape.
- `src/protocol/sdui.rs`: `sdui_updates_target_stable_node_ids` validates stable-ID update operations.
- Command: `cargo test sdui --quiet`

## Related

- [Protocol Codec](protocol-codec.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
