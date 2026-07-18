# Phase 20 Daily Editing Product Hardening Primitive Review

## Source

- `roadmap.md` — Phase 20 focus areas; Phase 18.15 theme supersession; Phase 15 pixel-snapshot deferral; Phase 21 multi-client scaling boundary
- `plans/054-Phase19-Hot-Reload-and-Behavior-Update-Semantics.md`
- `plans/055-Phase20-Daily-Editing-Product-Hardening.md`
- `plans/046-Phase-18.15-Text-Vocabulary-Two-Axis-Decorations-and-Theme-Registry.md`
- `src/client/{clipboard,file_dialog,mod,runtime_state}.rs`
- `src/masonry_editor.rs`, `src/masonry_sdui.rs`, `src/editor/surface.rs`
- `src/server/{workspace,document,connection}.rs`
- `src/shell/transient_menu.rs`
- `docs/wiki/modules/{masonry-editor,client-file-dialog,server-file-workspace,server-document-state,client-snapshot-bootstrap,editor-theme-registry,transient-menu-session,workspace-file-browser,persistent-runtime-hot-reload}.md`
- `docs/development/{ui-observability,launch-and-gui-smoke,performance}.md`
- `docs/reference/clay-js-api/{editor/client-copy-selection,documents/server-save-document,documents/client-open-file-dialog,workspace/client-open-folder-dialog}.md`
- `docs/reference/primitives/{index,registry,syntax-vocabulary,typography}.md`
- Local `masonry 0.4.0` / `masonry_core 0.4.0` IME + `masonry_testing` docs; local `arboard 3.6.1` clipboard API

## Overview

This is the implementation-entry review for roadmap Phase 20. Daily editing hardening must land on the existing client/server authority split, behavior manifests, selected-file grants, theme registry, structural UI observability, and transient-menu command surfaces. It must not invent parallel clipboard, history, filesystem, theme, or document-session systems, and it must not add mode/language-specific Rust branches.

Phase 20's "theme system" roadmap item was pulled forward into Phase 18.15 because rendering features depend on it. This review verifies that theme ownership is already delivered and limits Phase 20 theme work to accessibility/theme polish rather than rebuilding themes.

## Entry Gate

Roadmap Phase 20 starts after the package/mode path and hot-reload semantics are proven. Those gates are complete:

- Plan 054 (`plans/054-Phase19-Hot-Reload-and-Behavior-Update-Semantics.md`) has no unchecked tasks; Compromises Made and Further Actions are filled; wiki marks Phase 19 / Plan 054 complete.
- Phase 18.15 theme registry (Plan 046) is complete; roadmap records that Phase 20's theme-system item was pulled forward into Phase 18.15.
- Phase 18.8 command execution / transient menus, Phase 18.12 selected-file grant/file-browser, and Windows file-open dialog backends already exist for reuse.
- The roadmap Phase 19 heading still lacks an explicit `Complete` suffix; that is documentation lag, not an incomplete implementation gate. Plan 054 completion is the factual entry gate for Phase 20.

Therefore Phase 20 may begin implementation after this primitive review and the follow-on semantics decision task. Treating unchecked Phase 20 product gaps as blockers for the entry gate itself would be inaccurate; `tests/primitives_docs.rs::phase20_daily_editing_product_hardening_primitive_review` locks the factual gate instead.

## Existing Primitive Inventory

| Focus area | Existing generic primitive and owner | What already works | Phase 20 gap |
| --- | --- | --- | --- |
| Clipboard | `ClipboardSink` / `SystemClipboard` in `src/client/clipboard.rs`; `clay.editor.clientCopySelection` / `clientCutSelection` / `clientPasteClipboard`; `EditorWidget` cut/copy/paste helpers | Explicit user copy/cut/paste via `set_text`/`get_text`; cut deletes after copy; paste inserts/replaces as ordinary local edits; failures become sanitized runtime diagnostics; fake/memory sinks support tests; Phase 20 does not invent package/config/AI clipboard-contents APIs | Done for cut/paste command path |
| Undo/redo | Optimistic local edit + `ClientMessage::Edit` validation; leases/region locks; resync recovery; `EditHistory` in `src/editor/history.rs`; `clay.editor.clientUndo` / `clientRedo` | Ordinary insert/delete/replace edits apply locally then acknowledge; stale/lease rejection recovers through existing resync; bounded inverse-edit undo/redo (256) emits normal edits | Done for inverse-edit undo/redo command path. Entry-gate gap was: No History/undo/redo stack anywhere in `src/`; no inverse-edit recording; no undo/redo command IDs or chords |
| IME/composition | Masonry/winit `TextEvent::Ime`; `EditorWidget::on_text_event`; `CompositionState` in `src/editor/composition.rs` | `Ime::Enabled`/`Preedit`/`Commit`/`Disabled` handled; preedit is paint-only; `Ime::Commit(text)` clears overlay and inserts through `EditorCommand::Insert`; `set_ime_area` from layout/event; cancel on Disabled/focus loss/undo/redo/load | Done for preedit overlay + commit path. Entry-gate gap was: `Ime::{Enabled,Preedit,Disabled}` ignored; no preedit overlay/paint; no `set_ime_area`; composition cancel on focus loss/document switch undefined |
| Theme system | Phase 18.15 `StyleRegistry`, `ActiveTheme`, `clay.theme.setTheme`, Gruvbox Material themes, typography profiles | Inert theme packages, bootstrap/live theme install, vocabulary-token paint, status/chrome colors | **Satisfied by Phase 18.15.** Phase 20 verification closed: status/chrome already resolve through `StyleRegistry`; added theme-label observability + status-chrome AA contrast checks only — do not rebuild themes |
| Accessibility | `src/editor/accessibility.rs` helpers; `EditorWidget::{accessibility_label,accessibility_role}`; AccessKit status/SDUI/menu children; `TransientMenuSession` accessibility labels | Dirty/display-name/composing/theme/recovery markers in status + accessibility; basename-only titles; active transient menus expose `Role::Menu`/`MenuItem`; SDUI/shell roots use `Role::Group`; status observation stays consistent with accessibility text | Done for daily-editing label/role polish, including save/conflict recovery menus |
| Native file dialogs | `src/client/file_dialog.rs`; `clay.documents.clientOpenFileDialog`; `clay.workspace.clientOpenFolderDialog`; selected-path capability tokens | Windows Shell COM, Linux xdg-desktop-portal OpenFile/folder, and macOS `NSOpenPanel` file/folder backends; truly unavailable platforms still return `Unsupported` diagnostics without panicking; server consumes selected-path grants | Done for Linux/macOS file-open (+ macOS folder); save-as dialog still out of scope |
| Multi-document | Server `WorkspaceState` open registry (`HashMap<DocumentId, OpenDocument>`), `open_document_snapshots`, leases, per-document metadata (`dirty`, mode, path); client `DocumentSessionStore` (`src/editor/document_session.rs`); `clay.editor.clientShowOpenDocuments` | Server can hold multiple open documents concurrently; opening another file issues `DocumentOpened` | **Done for retain/switch MVP.** Opening another file retains the prior session (bound 64); `activate_document` restores caret/viewport/history without re-download; open-documents transient menu lists dirty/active markers. Entry-gate gap was: Client `EditorWidget` replace-on-open (`opening_second_file_browser_file_replaces_editor_snapshot`) |
| Selected-file save/conflict | `WorkspaceState::{save_document,reload_document}`; dirty/stale metadata; `clay.documents.serverSaveDocument`; `DocumentSaved`/`DocumentReloaded` client events; conflict recovery `TransientMenuSession` | Server dirty tracking, save/reload, force-reload gate, stale-metadata conflict errors that keep dirty state; Ctrl+S bindable through manifests; client dirty chrome + recovery menu | **Done for selected-file/workspace save/conflict MVP.** Dirty visible in status/accessibility; bound `Ctrl+S` enqueues `SaveDocument`; successful save clears dirty; `StaleFileMetadata`/`DirtyDocument` open recovery menus (reload/keep/compare-later or save-first) without silent overwrite. Save-as/watchers/autosave remain later |
| Pending-edit/error recovery | `ClientEditQueue`, auto-resync for stale/lease/read-only/region-lock/behavior rejections, `SduiStatusObservation.pending_edit_count`, recovery menus, `clay.editor.clientRequestResync` / `clientDismissRecovery` | Pending outbound depth visible in status/accessibility; edit rejections and disconnects surface sanitized diagnostics; actionable invalid-range/document and server errors open Resync/Dismiss menus; disconnect opens reconnect guidance + Dismiss; explicit resync reuses `RequestResync` | Done for pending-edit / disconnect / resync recovery UX. Entry-gate gap was: No dedicated pending-edit HUD/prompt, reconnect/resync confirmation UX, or richer recovery menus beyond status/diagnostic text |
| Pixel / GPU snapshots | Phase 15 `SduiObservableSnapshot` / `SduiStatusObservation`; Masonry 0.4 `TestHarness` / `assert_render_snapshot` investigated | Structural headless SDUI/status regression is the hard CI layer | **Re-deferred with evidence** (`decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`): `TestHarness` hardcodes Vello `use_cpu: true`, so goldens would not exercise Clay's production GPU path; Clay does not depend on `masonry_testing`; font/DPI/AA brittleness remains |

## Client-Local vs Server-First Ownership

Phase 20 operations must stay on the existing authority split:

| Operation | Ownership | Hot-path rule |
| --- | --- | --- |
| Copy / cut write, paste clipboard read | Client-local, user-mediated OS clipboard | Only on explicit cut/copy/paste commands; never during paint/layout/scroll/ordinary key insertion |
| Preedit paint / IME area updates | Client-local overlay | Local invalidation only; no IPC/server/JS per preedit event |
| Undo/redo inverse application | Client-local stack applying ordinary optimistic edits | Local apply first; enqueue normal `Edit`; no full-document IPC for ordinary undo |
| Active-document switch chrome / tab list rendering | Client session chrome over server document list | Chrome/paint local; open/switch authority and lease/metadata remain server-first |
| Save / reload / conflict resolution | Server-first workspace authority | Background relative to paint; reuse selected-file/workspace-root grants |
| File dialog show + selected-path grant consumption | Client shows dialog; server authorizes path | Dialog only from explicit UI command; never on typing/paint |
| Theme polish / accessibility updates | Client render + AccessKit over inert theme snapshots | No package JS or filesystem on paint/input |

Ordinary local text application, caret/selection, key routing, paint, layout, scroll, pointer handling, and edit acknowledgement must not wait on clipboard OS round-trips beyond the explicit command, file dialogs, save/reload disk work, package JavaScript, or multi-document registry scans.

## Security and Authority Boundary

- Clipboard read/write for Phase 20 daily editing stays user-mediated and client-owned on explicit cut/copy/paste commands.
- Save/open remain selected-file/workspace-root authorized. Native dialogs only pick paths; `OpenSelectedFile` / workspace opens still consume server-issued capabilities and grants.
- Undo/history cannot bypass leases, region locks, or server edit validation. Rejected undo edits recover through existing resync paths.
- IME preedit is paint-only until commit; diagnostics must not record raw composition strings beyond sanitized failure codes.
- Multi-document session state is client cache keyed by server `DocumentId`; the server remains registry/lease/dirty authority.
- Package, configuration, and AI authority over clipboard contents, filesystem, shell, network, and raw ops is **deferred** and must be established in a later decision (`decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`). Phase 20 does not invent those surfaces while implementing daily-editing commands.

## Theme Supersession Note

Roadmap supersession text: Phase 20's "theme system" daily-editing item is pulled forward into Phase 18.15 because every rendering feature depends on it.

**Phase 20 theme-system item: satisfied by Phase 18.15.** Evidence:

- Plan 046 complete; `docs/wiki/modules/editor-theme-registry.md` and `text-vocabulary-and-theme-primitive-review.md` document the delivered `StyleRegistry` / `ActiveTheme` / `setTheme` path.
- First-party Gruvbox Material themes and vocabulary-token paint are live; status/shell chrome (`statusBg`/`statusText`/`shellBg`/`panelBg`/selection/caret/scrollbar) already resolve through `StyleRegistry` — no second theme architecture.
- Phase 20 polish landed only accessibility/theme-label/contrast gaps: `SduiStatusObservation.theme_label` + accessibility `Theme …` marker from the active specifier; WCAG AA status-chrome contrast helpers/tests for Clay default and both Gruvbox Material themes (`status_chrome_meets_contrast`).
- Do not rebuild a second theme system in later Phase 20 tasks.

## Generic Gaps Required Before Implementation

1. **Clipboard cut/paste:** extend `ClipboardSink` with `get_text`; add cut = copy + delete selection and paste = insert/replace as ordinary edits; bindable command IDs beside copy.
2. **Undo/redo history:** bounded per-document client inverse-edit stack (256) that emits normal `Edit` transactions under the editable lease; unfinished preedit cancelled rather than partially undone; clear stacks on full resync/hard open-replace. Semantics approved in `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.
3. **IME preedit overlay:** Done — handle `Enabled`/`Preedit`/`Commit`/`Disabled`; paint-only preedit with optional cursor span; update IME area; commit as one edit; cancel on focus loss/document switch/undo/redo/load (`src/editor/composition.rs`).
4. **Multi-document client session:** Done — `DocumentSessionStore` retains per-document shadow/caret/viewport/pending/history/status chrome (bound 64); `DocumentOpened` stashes rather than destroys siblings; `clientShowOpenDocuments` + `activate_document` switch locally without re-download; server remains open-registry/lease/dirty authority.
5. **Dirty/save/conflict UX:** Done — dirty chrome after local edits; `DocumentSaved`/`DocumentReloaded` clear/update dirty; bound `Ctrl+S` enqueues `SaveDocument`; `StaleFileMetadata`/`DirtyDocument` open `TransientMenuSession` recovery prompts (reload/keep/compare-later or save-first) without silent overwrite.
6. **Linux/macOS file-open dialogs:** Done — Linux portal `OpenFile` and macOS `NSOpenPanel` reuse the existing selected-path grant consumption path; save-as remains deferred.
7. **Accessibility polish:** done for composing/dirty/display-name/recovery/menu roles via centralized helpers; multi-doc tab-list announcements remain with the multi-document task.
8. **Pixel snapshot revisit:** Done — evaluated `masonry_testing::TestHarness` / `assert_render_snapshot`; **re-deferred with evidence** because the harness hardcodes `use_cpu: true` (not production-GPU-faithful) and CI font/DPI/AA brittleness remains. Structural observability stays the hard layer (`decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`).

These are reusable editor/workspace primitives. No gap justifies `if markdown`, `if rust`, package-specific clipboard/history/IME branches, client filesystem authority, or a second theme registry.

## Rejected Implementation Shapes

- Treat Phase 20 as greenfield product features that ignore existing save/dirty/clipboard/dialog/theme primitives.
- Rebuild the theme system in Phase 20 despite the Phase 18.15 supersession.
- Server-owned undo log with dedicated undo protocol in this phase (larger Phase 21 collaboration surface).
- Client-only undo that rewrites the local rope without server transactions.
- Continuous IME preedit commits as document edits.
- Broad client filesystem reads from dialogs or clipboard helpers.
- Phase 20 invention of package/config/AI clipboard-contents APIs or undo mutation outside ordinary edit validation (broader package/config/AI authority remains deferred).
- Pixel snapshots that require interactive desktops or brittle font/raster golden images without a CI-safe harness.
- Adopting CPU-only `TestHarness` goldens in Phase 20 as if they validated Clay's production GPU renderer (`use_cpu: false`).

## Approved Semantics

Approved in `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`:

- Undo/redo: per-document bounded client stack (256) of inverse operations emitted as normal edits under the editable lease; clear on full resync/hard open-replace; cancel unfinished IME before undo/redo.
- Clipboard: extend `ClipboardSink` with `get_text`; cut = copy + delete; paste inserts as an ordinary local edit; no server clipboard proxy; Phase 20 does not invent package/config/AI clipboard-contents APIs.
- IME: local preedit overlay until `Commit`, then one ordinary insert/replace; handle Enabled/Disabled; set IME area; cancel on focus loss/document switch.
- Multi-document: server list/open/switch authority + client session map keyed by `DocumentId` (bound 64); opening another file no longer destroys prior session state.
- Deferred: package/configuration/AI clipboard, filesystem, shell, network, and raw-op authority — establish later.

## Tests

- `cargo test --test primitives_docs phase20_daily_editing_product_hardening_primitive_review`
- `tests/primitives_docs.rs::phase20_daily_editing_product_hardening_primitive_review` verifies Plan 054 entry-gate evidence, Phase 18.15 theme supersession note, all focus-area gap rows, client-local vs server-first ownership, no-hot-path rule, security boundaries, and the generic-only rule.
- `cargo test --test primitives_docs`
- `cargo fmt --check`

## Related

- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md)
- [Phase 19 Hot Reload and Behavior Update Primitive Review](phase19-hot-reload-behavior-update-primitive-review.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Client File Dialog Backend](client-file-dialog.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Server Document State](server-document-state.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Transient Menu Session](transient-menu-session.md)
- [Workspace Discovery and File Browser](workspace-file-browser.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Package Authoring Guide](../../reference/packages/creating-packages.md) — Phase 20 multi-document / dirty-save / recovery chrome contract for packages
- [Configuration overview](../../reference/clay-js-api/configuration.md#phase-20-daily-editing-product-hardening-configuration-review) — Phase 20 configuration review (bindKey-only; compiled ceilings; no new `clay:configuration` APIs)
- [File Open, Save, and Reload Workflow](../../development/file-open-save-reload-workflow.md)
- [UI Observability and SDUI Structural Regression](../../development/ui-observability.md)
- `plans/055-Phase20-Daily-Editing-Product-Hardening.md`
- `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`
- `decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md` — Phase 20 pixel/GPU snapshot re-deferral (`TestHarness` `use_cpu: true`)
