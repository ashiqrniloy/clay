# Manual File Browser Workflow Bugfix Primitive Review

## Source

- `docs/development/manual-file-browser-workflow-bug-contract.md`
- `docs/wiki/modules/end-to-end-file-browser-workflow-primitive-review.md`
- `docs/wiki/modules/workspace-file-browser.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/masonry-shell.md`
- `docs/wiki/modules/masonry-editor.md`
- `docs/wiki/modules/server-ipc-skeleton.md`
- `docs/wiki/modules/client-file-dialog.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`
- `.agents/skills/project-patterns/references/behavior-manifests.md`
- `.agents/skills/project-patterns/references/configuration-system.md`

## Overview

Plan 044 fixes real `cargo run` file-browser workflow regressions reported on Linux/GNOME. This primitive review records which existing Clay primitives own each fix before code changes begin, so the implementation stays generic instead of adding Markdown/Rust/file-browser-specific side paths.

The target workflow remains: user config in `~/.config/clay/init.js`, `cargo run`, `Ctrl+Shift+O` folder selection, selected-folder workspace-root grant, Clay-owned file browser navigation, Rust/TypeScript/JavaScript/Markdown document opens, second-file replacement, scrolling, and copy-selection clipboard writes.

## Existing Primitive Inventory

### Keybinding route and behavior manifests

- Primitive: `KeyRoutingOverride` / behavior-manifest routing.
- Owner: server/configuration compiles inert key rules; client executes installed manifest rules locally.
- Code: `runtime/js/keybindings.js`, `src/server/ops/keybindings.rs`, `src/client/behavior.rs`, `src/masonry_editor.rs`.
- Bug boundary: shifted `Ctrl+Shift+O` must be fixed in shared key matching for character key bindings, not as an `O`-specific shortcut.
- Hot path: key lookup is client-local and must not run IPC, JavaScript, file IO, or desktop portal work before route recognition.

### Client UI command route

- Primitive: `ClientUiCommandRoute` plus documented Clay JS API command IDs.
- Owner: client app driver invokes native UI only after explicit user command; server validates selected paths afterward.
- Code: `src/main.rs::handle_client_ui_command`, `src/client/file_dialog.rs`, `src/client/mod.rs`, `runtime/js/workspace.js`, `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`.
- Bug boundary: `clientOpenFolderDialog()` remains a bindable client UI command. The fix must not add hidden config keys or grant folder authority before `AddSelectedWorkspaceRoot` reaches server validation.

### FileBrowserState and bounded workspace APIs

- Primitive: Clay-owned file-browser composition over `WorkspaceRootDiscovery` and `BoundedFileListService`.
- Owner: server `WorkspaceState` owns roots, listing, traversal checks, and file-open authority; `FileBrowserState` builds inert SDUI rows.
- Code: `src/server/workspace.rs`, `src/shell/file_browser.rs`, `src/server/command_execution.rs`.
- Bug boundary: nested row identity must keep `SduiListItem.id` and `SduiActionSource::ListItem.item_id` identical; root-relative `relativePath` belongs in action arguments only.
- Hot path: browsing/opening is explicit server-first command work; paint and scroll must not relist directories.

### StaticSduiState validation

- Primitive: server-owned SDUI action validation.
- Owner: `StaticSduiState` stores the currently valid Clay-owned workspace browser/action tree; runtime/package trees are separately validated before publication.
- Code: `src/server/sdui.rs`, `src/server/connection.rs`, `src/server/mod.rs::apply_runtime_outputs`.
- Bug boundary: open-time package/classification output must not erase Clay-owned workspace browser validation state. `UnknownActionCommand("workspace.openFile")` after Markdown activation is a validation-state ownership bug, not a Markdown-specific bug.
- Security: keeping strict validation is required; do not make action validation accept undeclared or mismatched commands to mask stale state.

### SduiNativeState rendering and local action regions

- Primitive: client-native SDUI reconciliation/action-region rendering.
- Owner: client owns native rendering, action hit regions, local viewport/scroll state, and pointer routing for already-published inert SDUI.
- Code: `src/masonry_sdui.rs`, `src/masonry_editor.rs`.
- Bug boundary: file-browser scroll belongs in `SduiNativeState` local state plus pointer scroll routing, not in server listing or package JavaScript. Scrolled hit testing must use the same row-height/offset math as paint. `SduiNativeState::scroll_vertical_pixels` treats positive deltas as scrolling down (revealing later rows), matching the editor scroll convention so wheel/trackpad routing does not invert direction.

### PaneSlotLayout and editor region computation

- Primitive: `PaneSlotLayout` / fixed left-slot geometry.
- Owner: Clay shell/client layout owns main-region geometry; packages cannot mutate Masonry widgets or native layout directly.
- Code: `src/shell/layout.rs`, `src/masonry_shell.rs`, `src/masonry_sdui.rs::editor_region_for_document`, `src/masonry_editor.rs::editor_main_rect`.
- Bug boundary: the editor must reserve the Clay-owned left file-browser pane even after the active document ID changes. Rebinding the server file-browser tree for every document open is not required for the generic fix.

### EditorSurface visual scroll and paint chrome

- Primitive: client-owned editor viewport/visual scroll and native paint chrome.
- Owner: `EditorSurface` owns text viewport, caret, selection, visual scroll state, and editor paint inside the shell-provided main rect.
- Code: `src/editor/surface.rs`, `src/editor/viewport.rs`, `src/masonry_editor.rs`.
- Bug boundary: remove the permanent purple decorative circle and visible inset editor card/padding as paint-chrome changes. Add main text-area scrollbar using existing `visual_scroll_y` / `last_visual_max_scroll_y` rather than a second scroll model. The sub-line caret-keep-visible helper must be gated by a one-shot caret-pin flag so explicit scrolling can move the view away from the caret instead of snapping back on every paint.

### Open-document follow-ups

- Primitive: `DocumentClassification`, `MajorModeActivation`, `IncrementalParseUpdate`, and `DecorationRange` on explicit open/reload.
- Owner: server/runtime classifies and parses open documents asynchronously; client receives inert behavior/decorations/diagnostics.
- Code: `src/server/connection.rs::open_document_followup_messages`, `src/server/mod.rs::apply_runtime_outputs`, `src/server/parse_coordinator.rs`.
- Bug boundary: `parse.open_activation_timeout` should be a status/diagnostic result only. It must not poison file-browser navigation or action validation.

## Generic Fix Map

| Reported failure | Primitive to fix | Generic rule |
| --- | --- | --- |
| `Ctrl+Shift+O` does not open folder picker | behavior-manifest key route / `ClientUiCommandRoute` | Character key matching for command bindings is normalized at lookup while text insertion preserves actual typed text. |
| Nested `src/main.rs` fails with `ActionSourceMismatch` | `FileBrowserState` SDUI row construction / `StaticSduiState` validation | Row ID and action source item ID match exactly; `relativePath` remains an argument. |
| Browser actions fail after Markdown activation | server `StaticSduiState` ownership | Open-time package outputs cannot replace Clay-owned workspace browser validation state. |
| `parse.open_activation_timeout` hangs workflow | open-document follow-up diagnostics | Diagnostics are non-fatal status; stale parse/decor work is skipped without invalidating navigation. |
| Second file does not replace first | document-open event application | Every workspace/file-browser open sends/applies the latest `DocumentOpened` snapshot generically. |
| Editor overlaps file browser | `PaneSlotLayout` / editor-region computation | Visible Clay-owned left slot reserves editor region independent of the active document ID. |
| Purple circle and visible card padding | `EditorSurface` paint chrome | Paint only useful editor background/text/scroller chrome; no permanent decorative canvas. |
| File browser cannot scroll | `SduiNativeState` local scroll state | Wheel/trackpad over the left panel scrolls already-published SDUI rows locally. |
| Main text area lacks scroller | `EditorSurface` visual scroll chrome | Draw a scrollbar indicator from existing editor scroll metrics. |

## Rejected Approaches

- Do not add Markdown-specific Rust branches for activation, parsing, SDUI, layout, or diagnostics.
- Do not add Rust/TypeScript/JavaScript-specific file-open branches; language packages consume generic open-document follow-ups.
- Do not relax `StaticSduiState::validate_action` to accept unknown commands, stale rows, source mismatches, or raw paths.
- Do not route file-browser scrolling through server relisting, package JavaScript, IPC, or filesystem work.
- Do not add hidden JSON/TOML/ad hoc config keys for folder picker, scrollbars, padding, or file-browser behavior.
- Do not create package-owned native file browser widgets, Masonry widget handles, raw CSS hooks, Vello/Parley callbacks, client-side JavaScript handlers, or raw `Deno.core.ops` routes.

## Hot-Path and Security Boundaries

- Client hot paths remain local: typing, paint, layout, pointer movement, wheel scrolling, selection, caret movement, and first local paint after input do not run IPC, JavaScript, package runtime work, AI, shell commands, filesystem scans, full-document serialization, or desktop portal dialogs.
- Server owns workspace/file authority: roots, selected-folder grants, root-relative opens, traversal checks, document snapshots, command execution, behavior activation, parse scheduling, and SDUI validation.
- Client owns native rendering/input state: SDUI action regions, file-browser scroll offset, editor viewport, editor scrollbar, caret, selection, and copy-selection routing.
- Packages still cannot read clipboard contents, paste/cut, write arbitrary clipboard text, add roots, add marker/ignore rules, list arbitrary paths, open raw absolute paths, mutate native layout, access native widget handles, run shell/network/WASM/AI authority, or call raw `Deno.core.ops`.

## Tests Planned

- `keybinding_shifted_character_routes_client_ui_command`
- `shifted_printable_unbound_character_still_inserts_shifted_text`
- `file_browser_nested_file_action_source_matches_list_item_id`
- `workspace_nested_file_action_opens_file_through_workspace_api`
- `file_browser_action_survives_markdown_open_followup_diagnostic`
- `opening_second_workspace_file_replaces_editor_snapshot`
- `workspace_browser_reserves_left_slot_after_document_id_changes`
- `file_browser_scroll_reveals_later_rows_without_relisting`
- `file_browser_scrolled_action_hits_visible_row`
- `scrolls_point_routes_scroll_to_file_browser_only_inside_left_pane`
- `editor_scrollbar_thumb_reflects_visual_scroll_position`
- `editor_scrollbar_hidden_when_content_fits`
- `editor_scrollbar_stays_inside_main_editor_region_with_left_browser`

## Related

- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
- [Workspace Discovery and File Browser](workspace-file-browser.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Client File Dialog Backend](client-file-dialog.md)
