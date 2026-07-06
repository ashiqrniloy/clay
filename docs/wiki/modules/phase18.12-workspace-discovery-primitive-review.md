# Phase 18.12 Workspace Discovery and File Browser Foundation Primitive Review

## Source

- `plans/040-Phase18.12-Workspace-Discovery-and-File-Browser-Foundation.md`
- `roadmap.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/server-file-workspace.md`
- `docs/wiki/modules/masonry-shell.md`
- `docs/wiki/modules/slot-aware-package-ui.md`
- `docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md`
- `docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md`
- `src/server/workspace.rs`
- `src/server/ops/workspace.rs`
- `runtime/js/workspace.ts`
- `src/shell/layout.rs`
- `src/shell/package_ui.rs`
- `src/masonry_shell.rs`
- `src/masonry_sdui.rs`
- `runtime/js/ui.ts`
- `src/server/command_execution.rs`
- `src/server/control_center.rs`
- `src/shell/transient_menu.rs`
- `tests/primitives_docs.rs`

## Overview

Phase 18.12 should add server-owned workspace-root discovery and a bounded file tree/list service, then build a Clay-owned file browser UI on top of existing generic primitives. This review completes the primitive-first gate before implementation. It inventories the existing workspace, shell layout, command execution, transient menu, package UI/component, and mode primitives; records that the left fixed-panel tree and bottom transient fuzzy-open are compositions of existing primitives rather than new primitive categories; identifies the small generic gaps (workspace-root discovery helper, bounded file listing service); and states the authority boundary Phase 18.12 must preserve.

The headline finding is that most of the file browser can be built without new Rust primitives. `WorkspaceState` already supports multi-root metadata; `FixedSlotId::Left` and `PaneSlotLayout` already provide the left fixed panel slot; `TransientMenuSession` + `CommandExecution` already provide the bottom fuzzy-open workflow; the SDUI/component catalog already provides `list` and tree-like composition via `flex`/`stack`; and selected-file grants already handle out-of-root files. The genuine new generic primitives are a server-owned workspace-root discovery helper that extends `WorkspaceState` with cwd/CLI, opened-file ancestry, explicit user grants, and bounded marker-file detection, plus a bounded server file tree/list service with ignore rules, depth/count limits, cancellation, refresh, and diagnostics.

## Existing Primitive Inventory

### Workspace roots and file authority

- `src/server/workspace.rs::WorkspaceState` is the server-side source of truth for workspace roots and open file documents. It already stores `WorkspaceRoot { id, authority }` values with `WorkspaceAuthority::Directory { canonical_path }` and `WorkspaceAuthority::SingleFile { canonical_path }`.
- `WorkspaceState::add_root` canonicalizes a path, deduplicates by canonical path, and returns a stable `WorkspaceRootId`. It rejects paths that are not directories (for directory roots) and already handles `WorkspaceRootMetadata` display names/paths via `list_root_metadata`.
- `WorkspaceState::open_existing_file` opens a file that is already inside a known root, while `WorkspaceState::open_selected_file` creates a single-file grant for a browser-picked file outside any root. The selected-file grant flow is the existing path for user-exposed files that are not under a workspace root.
- `src/server/ops/workspace.rs::op_clay_workspace_list_roots` exposes root metadata to the controlled server runtime, and `runtime/js/workspace.ts::serverListWorkspaceRoots` is the stable Clay JS facade. No direct client filesystem access is exposed.
- `docs/wiki/modules/server-file-workspace.md` documents the server-owned workspace model, canonical path registry, duplicate-open identity, file-backed dirty state, and authority boundaries.

### Shell layout and slots

- `src/shell/layout.rs` implements internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state. `PaneSlotLayout` already has a mandatory `main` slot and optional fixed `left`, `right`, `top`, and `bottom` slots.
- `FixedSlotId::Left` is the intended slot for file trees, outlines, and similar side panels; `FixedSlotId::Bottom` is intended for diagnostics, output, and transient menus.
- `src/masonry_shell.rs::ClayShellWidget` places the editor child from installed layout state. Masonry layout reads validated state only and does not parse packages, run JavaScript, wait on IPC, or mutate package UI state during layout.
- `src/shell/package_ui.rs::PackageUiRuntimeState` stores accepted fixed panels and transient overlays. Accepted fixed panels compose into `PaneSlotLayout` geometry; accepted transient overlays render separately and do not consume fixed slot geometry.
- `docs/wiki/modules/masonry-shell.md` documents the Clay-owned shell root, slot geometry, inert layout updates, and structural observability without native handles.

### Command execution and transient menus

- `src/server/command_execution.rs::CommandExecutor` / `CommandExecutionRequest` provide the server-owned command activation boundary used by SDUI actions, package UI actions, behavior-manifest keybindings, and transient-menu selections.
- `src/shell/transient_menu.rs::TransientMenuSession` is the generic query/selection/status/session model for bottom-pane command browsing and future picker workflows.
- `src/server/control_center.rs` builds a `TransientMenuSession` from the current `CommandRegistry` snapshot and routes selected items through `CommandExecutor`. This is the model for the Phase 18.12 fuzzy-open workflow: build a session from file-list metadata and route activation through `CommandExecution`.
- `docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md` documents the generic gaps and implementation of `CommandExecution` and `TransientMenuSession`.

### Package UI components and action intents

- `runtime/js/ui.ts` exposes `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and related `clay:ui` facades. The component catalog includes `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, and `statusItem`.
- Tree-like rendering can be composed from generic `list`, `flex`/`stack`, and `label`/`button` components; no file-browser-specific Rust tree widget is required for the smallest working product.
- `UiActionIntent` carries only a registered command ID and bounded primitive arguments. File browser actions (open, reveal) will normalize to registered commands such as `clay.workspace.openFile` and `clay.workspace.revealInTree`.
- `docs/wiki/modules/slot-aware-package-ui.md` documents the runtime-backed contribution registry, component catalog, fixed panel and transient overlay composition, action validation, and security boundaries.

### Document classification and fallback modes

- `src/packages/modes.rs::ModeRegistry` and the Phase 18.9 `core.text`/`core.code` fallback modes ensure any opened file is editable even when no language package matches. The file browser's open action can therefore rely on the existing mode activation path rather than adding mode-selection logic.
- `docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md` documents the fallback-mode, classification, and generic key-behavior primitives.

## Generic Phase 18.12 Primitive Gaps

### Server-owned workspace-root discovery

The missing generic primitive is a server-owned discovery helper that extends `WorkspaceState` with four root sources while preserving the existing multi-root model and `add_root` insertion point:

1. **CLI/current directory at startup:** the server's starting cwd or an explicit CLI/root argument is canonicalized and added as a directory root.
2. **Opened-file ancestry:** when a file is opened and no existing root covers it, walk a bounded number of ancestors looking for a known project marker (`.git`, `Cargo.toml`, `package.json`, and a closed, documented set). If a marker is found, add that directory as a root; if the walk reaches the bound without a marker, the file may be handled as a single-file grant instead of inventing a root.
3. **Explicit user grant:** a Clay JS API (facade + op) lets the user grant a directory or single file as a workspace root. The server canonicalizes and records it through `add_root` or the single-file grant path.
4. **Bounded marker files:** a closed, named constant/table of known project markers is checked at each root candidate. Marker checks are presence/metadata-only; markers are never executed or parsed for arbitrary content.

Required shape (illustrative, names to be finalized during implementation):

```rust
const KNOWN_PROJECT_MARKERS: &[&str] = &[".git", "Cargo.toml", "package.json", /* closed set */];

impl WorkspaceState {
    pub(crate) fn discover_root_for_path(&mut self, path: &Path) -> Result<Option<WorkspaceRootId>, WorkspaceError>;
    pub(crate) fn add_explicit_user_grant(&mut self, path: &Path) -> Result<WorkspaceRootId, WorkspaceError>;
}
```

Implementation implications:

- Reuse `WorkspaceState::add_root` as the single insertion point; deduplicate by canonical path.
- Discovery work happens server-side, off the typing/render/paint path. Canonicalization and marker scans are bounded by a max-depth and a max-roots limit.
- No client-side filesystem access; no package filesystem authority; packages cannot add roots, markers, or discovery triggers.
- The existing `WorkspaceRoot`/`WorkspaceAuthority` model is preserved. The one-root default UI is a display decision, not a model change: the underlying multi-root model remains future-compatible.

### Bounded server file tree/list service

The missing generic primitive is a server service that lists directory entries under a known workspace root and returns bounded metadata suitable for tree/list rendering:

```rust
pub(crate) struct FileListRequest {
    root_id: WorkspaceRootId,
    dir_relative_path: PathBuf,
    max_depth: u32,
    max_entries: usize,
    cancel: CancelToken,
}

pub(crate) struct FileListEntry {
    name: String,
    kind: FileEntryKind,
    relative_path: PathBuf,
    size_hint: Option<u64>,
    diagnostic: Option<FileEntryDiagnostic>,
}
```

Implementation implications:

- Scope every request to a known workspace root; reject relative paths containing `..` or resolving outside the root's canonical boundary.
- Honor a closed ignore source: a Clay default ignore set (e.g., `.git`, `node_modules`, `target`) plus a root-level single `.gitignore` parse if the smallest working product needs it. Defer nested `.gitignore` hierarchy to a later phase.
- Enforce named max-depth and max-entry constants; return a truncation diagnostic rather than failing the whole request.
- Support cancellation of in-flight listings and explicit refresh.
- Report per-entry diagnostics (e.g., permission denied) without failing the whole request.
- Reuse stdlib `fs::read_dir` / `tokio_fs::read_dir`; do not add a new dependency unless the ignore parse genuinely needs one, and prefer an already-installed crate.
- The service is a generic reusable primitive, not file-browser-specific. Results are inert serializable data consumed by SDUI/component trees; no client-side filesystem reads.

### File browser UI is a composition, not a new primitive

The file browser UI is Clay-owned, and the left fixed-panel file tree and the bottom transient fuzzy-open workflow do not require new primitive categories. They are compositions of existing primitives:

- **Left fixed panel:** `FixedSlotId::Left` + a Clay-owned fixed panel populated by inert `FileListEntry` data rendered through the component catalog (`list`, `flex`/`stack`, `label`, `button`). No file-browser-specific Rust tree widget.
- **Bottom fuzzy-open:** `TransientMenuSession` whose items are bounded file-path metadata. Query filtering runs locally on already-installed metadata. Activation emits a `CommandExecution` request for `clay.workspace.openFile` or `clay.workspace.revealInTree`.
- **Open/reveal actions:** registered commands validated by `CommandExecutor`, resolving through `WorkspaceState::open_existing_file` for in-root files and `open_selected_file` for out-of-root picks.

This matches the roadmap's "Clay-owned file browser UI" requirement while preserving the primitive-first rule.

## Hot-Path Classification

Phase 18.12 classifies work explicitly:

| Work | Classification | Allowed path |
| --- | --- | --- |
| Root discovery at startup | Startup/configuration work, off typing path | Canonicalize cwd/CLI argument; bounded marker/ancestry walk |
| Root discovery on open | Open-time server work | Bounded ancestry walk from already-known path; single-file grant fallback |
| Directory listing | Async/cancellable server service | `WorkspaceState::list_directory` or equivalent off the typing/render path |
| Tree rendering | Paint/layout read of installed inert listing state | Masonry reads bounded `FileListEntry` data and component tree only |
| Fuzzy filtering | Local bounded UI state work | Filter installed file-path metadata in `TransientMenuSession` |
| File activation | Server-first command execution | `CommandExecution` validates and routes to `open_existing_file` / `open_selected_file` |
| Reveal-in-tree | UI state + server command | Update left-panel focus state via command; no filesystem read in paint path |

Ordinary typing, caret movement, local edit application, scroll, paint, layout, pointer hit testing, keypress dispatch, and text-event handling must not synchronously discover roots, scan directories, list files, execute commands, call package JavaScript, wait on IPC, read files beyond the bounded listing service, call shell/network/AI, or serialize full documents.

## Rejected Implementation Shapes

- Do not add a `FileBrowserWidget`, `FileTreeWidget`, `WorkspaceDiscoveryWidget`, `MarkdownFileBrowser`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust file-browser branch.
- Do not implement client-side workspace discovery or file listing. The server owns canonical paths, roots, and directory listing authority.
- Do not allow packages to add workspace roots, project markers, or ignore rules. Only Clay-owned code and explicit user grants may broaden workspace authority.
- Do not implement a full nested `.gitignore` parser or pull in a heavy ignore crate for the smallest working product. A closed Clay default ignore set plus optional root-level `.gitignore` is sufficient; defer hierarchy semantics.
- Do not pass raw client-chosen paths straight to an open op. Every open/reveal action must route through `CommandExecution` with bounded args validated against roots and selected-file grants.
- Do not make the file tree a package contribution for the smallest working product. The roadmap calls for a Clay-owned file browser UI; package panel contributions can consume the same primitives in later phases.
- Do not add file-browser-specific Rust rendering branches in `masonry_sdui.rs` or `masonry_shell.rs`. Native widget mapping stays inside the existing Clay-owned component catalog. File browser UI is Clay-owned.
- Do not expose Masonry `Widget`/`WidgetId`/`WidgetPod`, native handles, Vello/Parley callbacks, raw op names, raw CSS, or client-side JavaScript as file-browser APIs.
- Do not treat a public Clay JS API as implemented by adding only a raw op or inventory row; public APIs require facade, op, docs, registry, tests, security notes, and naming metadata.

## Security and Authority Boundary

The Phase 18.12 review introduces no broad client or package filesystem authority.

Allowed authority remains narrow:

- Workspace-root discovery only canonicalizes paths the user already exposed (cwd, CLI arg, opened file, explicit grant). Marker files are checked by name/presence only and are never executed or parsed for arbitrary content.
- Directory listing is scoped to known workspace roots; traversal outside a root's canonical boundary is rejected. Packages cannot list arbitrary paths.
- Explicit user grants are the only path that broadens authority, and they are recorded as workspace roots with display metadata.
- UI actions carry inert command intents only; the server re-checks every activation (command ID, routing policy, permissions, target context, argument bounds, session freshness).
- Browser-picked files outside a known root trigger the existing selected-file single-file grant flow; no directory authority is granted.
- No package may add roots, markers, ignore rules, or listing scopes. Package filesystem authority is not expanded.

## Planned Documentation and Test Coverage

- `docs/wiki/modules/phase18.12-workspace-discovery-primitive-review.md` (this page) records the inventory, generic gaps, hot-path classification, rejected shapes, and no-new-authority boundary.
- `docs/reference/primitives/registry.md` should record the workspace-root discovery and bounded file tree/list service as extensions of the workspace primitive family, or as new rows if the implementation task chooses distinct primitive category names.
- `docs/reference/primitives/backlog.md` should note Phase 18.12 reuses existing shell/transient-menu/command/component primitives and adds only the two generic workspace/file-list gaps.
- `docs/reference/packages/creating-packages.md` should be updated in the package-guide task to state the file-browser-era shell contract: Clay owns slots/components/native widgets; packages declare inert contributions only; packages cannot add roots/markers/list arbitrary paths.
- `tests/primitives_docs.rs` should require this review page to be linked from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`, and should assert it records inventory, generic gaps, hot-path classification, rejected file-browser-specific shapes, and no-new-authority text.

## Invariants and Constraints

- `WorkspaceState` remains the canonical source of workspace roots and open file documents.
- Root discovery reuses `add_root`; deduplication is by canonical path.
- The multi-root model is preserved; the one-root default UI is a display decision.
- Directory listing is bounded by depth/count, scoped to known roots, cancellable, refreshable, and reports diagnostics without failing the whole request.
- The file browser UI is Clay-owned and composed of existing shell/transient-menu/command/component primitives.
- All open/reveal/save-related actions route through server-authoritative workspace APIs and selected-file grants.
- No package filesystem authority is introduced; packages cannot add roots, markers, ignore rules, or listing scopes.
- No filesystem scan, directory walk, or listing work runs in Masonry paint/layout/pointer/scroll/keypress/text-event handlers.

## Tests

- `tests/primitives_docs.rs::phase18_12_workspace_discovery_primitive_review_records_inventory_and_gaps`: verifies wiki/index and primitive-architecture links plus required primitive-review contents.
- Implementation-time tests (later tasks) should cover startup cwd root discovery, opened-file ancestry with and without markers, marker detection, canonical-path dedup, closed marker-set rejection, explicit user grant, listing depth/count bounds, ignore rules, traversal-escape rejection, cancellation, refresh, permission-denied diagnostics, and UI structural observations that prove the left panel and fuzzy-open compose existing primitives.
- Run focused documentation coverage with:

```text
CARGO_TARGET_DIR=target/pi-verify cargo test --test primitives_docs phase18_12_workspace_discovery_primitive_review_records_inventory_and_gaps --quiet
```

## Related

- [Server File Workspace Model](server-file-workspace.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Command Registry](command-registry.md)
- [Transient Menu Session](transient-menu-session.md)
- [Control Center](control-center.md)
- [Mode Registry](mode-registry.md)
- [Primitive Architecture](primitive-architecture.md)
