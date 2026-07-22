# Workspace Discovery and File Browser

## Source

- `src/server/workspace.rs`
- `src/server/ops/workspace.rs`
- `src/server/ops/commands.rs`
- `runtime/js/workspace.js`
- `runtime/js/commands.js`
- `src/shell/file_browser.rs`
- `src/server/connection.rs`
- `src/server/command_execution.rs`
- `src/masonry_sdui.rs`
- `src/masonry_editor.rs`
- `docs/reference/clay-js-api/workspace/*.md`
- `docs/reference/clay-js-api/commands/server-{execute-command,open-file,reveal-in-tree}.md`

## Overview

Phase 18.12 adds the server-owned workspace discovery and file-browser foundation. Clay discovers workspace roots, lists files through bounded server APIs, renders a Clay-owned left file tree from inert SDUI nodes, builds a bottom transient fuzzy-open session from the same listing snapshot, and routes open/reveal actions through `CommandExecution` plus `WorkspaceState` authority checks.

The file browser is not a package widget. Packages may call documented Clay JS facades, but they cannot add marker files, ignore rules, native tree widgets, raw file listing ops, or direct filesystem authority. It introduces no broad client or package filesystem authority.

## Responsibilities

- `WorkspaceState` owns root discovery, root deduplication, explicit user grants, single-file grants, bounded directory listing, ignore filtering, traversal checks, diagnostics, and cancellation token checks.
- `src/server/ops/workspace.rs` exposes runtime ops behind `runtime/js/workspace.js` facades: `serverAddWorkspaceRoot`, `serverDiscoverWorkspaceRootForPath`, `serverListDirectory`, `serverCreateListingCancelToken`, and `serverCancelListing`.
- `src/shell/file_browser.rs` builds Clay-owned UI state from a `WorkspaceState` snapshot and converts it to an inert `SduiTree` plus `TransientMenuSession` data.
- `src/server/connection.rs` sends the file-browser SDUI snapshot during welcome when a workspace root exists and routes file-browser SDUI actions through workspace command execution. Directory navigation results are converted into refreshed file-browser `SduiSnapshot` messages.
- `src/server/command_execution.rs` owns built-in workspace commands: `clay.workspace.openFile`, `clay.workspace.openFuzzyFile`, `clay.workspace.openDirectory`, `clay.workspace.revealInTree`, and `clay.workspace.toggleFileBrowser`.

## How It Works

### Root discovery

`IpcServer::try_new` adds configured roots from `ServerConfig::workspace_roots`. When none are configured, it calls `WorkspaceState::add_root_from_cwd()` so the process working directory becomes the initial root. `WorkspaceState::add_root` canonicalizes and deduplicates roots by canonical directory path and enforces `MAX_WORKSPACE_ROOTS`.

`WorkspaceState::discover_root_for_path` canonicalizes an existing file path, rejects directories, and walks parent directories up to `MAX_DISCOVERY_ANCESTRY_DEPTH`. The closed marker table is `KNOWN_PROJECT_MARKERS` (`.git`, `Cargo.toml`, `package.json`). If a marker is found, Clay adds that ancestor as a root. If no marker is found, discovery returns `None` so callers can fall back to selected-file single-file grants instead of broadening workspace authority.

`WorkspaceState::add_explicit_user_grant` accepts a user-chosen directory or file. Directories become roots. Files become single-file grants via `add_single_file_grant` and do not appear in `list_root_metadata`, so they cannot become tree-browsable roots by accident.

### Bounded listing

`WorkspaceState::list_directory` accepts a `FileListRequest` with `root_id`, `relative_path`, `max_depth`, and `max_entries`. It canonicalizes the requested directory, checks that it remains under the authorized root, then walks entries iteratively/recursively with hard ceilings:

- `MAX_LIST_DIRECTORY_DEPTH`
- `MAX_LIST_DIRECTORY_ENTRIES`
- `MAX_CHILD_COUNT_SCAN`

The default ignore set is compiled into Clay (`.git`, `node_modules`, `target`). One optional root-level `.gitignore` supports only component-name rules: blank lines and lines beginning with `#` are skipped; other rules contain literal Unicode scalar values, `?` for one scalar, `*` for zero or more scalars with backtracking, and one optional trailing `/` for directory-only matching. Rules apply to each visited filename component; nested ignore files and path rules are not implemented. Negation, escaping, character classes, path separators, `**`, and malformed empty directory rules abort the listing with one diagnostic instead of being silently ignored and broadening traversal. Reads retain at most 1 MiB + 1 byte; parsing inspects at most 4,096 lines, stores at most 1,024 rules, and accepts at most 256 scalars per rule. Invalid UTF-8, non-regular/unreadable/oversized files, or limit overflow return an empty truncated page plus a bounded diagnostic. Missing `.gitignore` means compiled defaults only. Listing otherwise returns a `FileListPage` containing entries, truncation/cancellation flags, and diagnostics. Permission-denied or unreadable children become per-entry diagnostics where possible instead of failing the whole page.

Cancellation uses server-owned token IDs backed by a process-local registry. `serverCreateListingCancelToken()` creates a token, `serverCancelListing(tokenId)` flips its atomic flag, and listing checks the flag cooperatively between directory reads. This keeps cancellation cheap and avoids holding the workspace lock only to cancel a long request.

### Clay-owned UI composition

`FileBrowserState::from_workspace` picks a visible workspace root at the root directory; `FileBrowserState::from_workspace_at` lists a root-relative current directory. Both ask `WorkspaceState::list_directory` for a bounded depth-1 snapshot and store normalized `FileBrowserEntry` values with the actual `WorkspaceRootId`, relative path, display label, kind, child count, and diagnostics.

`FileBrowserState::to_sdui_tree` composes existing SDUI primitives: a left `Panel`/`Stack` with a workspace/current-directory label and `List` items, plus the normal `EditorView` in a row. It does not add a native `FileTreeWidget` or file-browser branch in Masonry. File rows carry `workspaceRootId` and `relativePath` for `clay.workspace.openFile`; directory rows carry the same bounded root-relative arguments for `clay.workspace.openDirectory`; non-root directories include a `../` parent row. A row's `SduiListItem.id` and `SduiActionSource::ListItem.item_id` are the same display-row identity (for example `main.rs` inside `src/`); the root-relative path (`src/main.rs`) lives only in the typed `relativePath` action argument and is revalidated by `WorkspaceState` on open.

`FileBrowserState::fuzzy_session` builds a bottom `TransientMenuSession` by filtering the same bounded entries locally. Items route to `clay.workspace.openFuzzyFile`; there is no separate fuzzy-open primitive or package-provided picker implementation.

### Open/reveal/navigation command routing

`CommandExecutor::execute_workspace` validates command ID, routing policy, provenance, permissions, target, and bounded arguments before side effects. It also receives the authenticated connection's `ClientId`; open and reveal operations use that identity when acquiring or inspecting document leases. Never substitute a fixed client ID here: the returned `DocumentOpened` metadata and every later edit must name the same lease holder. Directory navigation accepts `{ workspaceRootId, relativePath }`, validates the target by calling `WorkspaceState::list_directory` with tight depth/count bounds, then returns `WorkspaceActionResult::Navigated`. The connection handler rebuilds `FileBrowserState::from_workspace_at`, stores it in `StaticSduiState`, and sends one `ServerMessage::SduiSnapshot` for the explicit click.

Open commands accept either `{ workspaceRootId, relativePath }` or `{ absolutePath }`:

- In-root opens call `WorkspaceState::open_existing_file`.
- Out-of-root explicit picks call `WorkspaceState::open_selected_file`, creating a single-file grant only after file/type/UTF-8 validation.

The result is `WorkspaceActionResult::Opened(OpenDocumentSnapshot)`. The connection handler maps that to `ServerMessage::DocumentOpened { metadata, text }`, then runs the same `open_document_followup_messages` path as `OpenDocument` and selected-file opens so behavior manifests, mode activation, and decoration sets are consistent across open origins.

`clay.workspace.revealInTree` validates a real open `documentId` through `WorkspaceState::document_metadata` before returning `WorkspaceActionResult::Revealed`. `clay.workspace.toggleFileBrowser` currently returns `WorkspaceActionResult::Toggled`; persistence and user-facing visibility settings are deferred.

## Code Examples

```rust
let mut workspace = WorkspaceState::new();
let root_id = workspace.add_root("/workspace/project")?;
let page = workspace.list_directory(FileListRequest {
    root_id,
    relative_path: "src".into(),
    max_depth: 2,
    max_entries: 256,
})?;
```

```js
import { serverListDirectory } from "clay:workspace";
import { serverOpenFile } from "clay:commands";

const page = await serverListDirectory({ rootId, relativePath: "src", maxDepth: 2 });
await serverOpenFile({ workspaceRootId: rootId, relativePath: page.entries[0].relativePath });
// Directory rows use the inert command ID directly through SDUI actions:
// { commandId: "clay.workspace.openDirectory", workspaceRootId: rootId, relativePath: "src" }
```

## Primitive Coverage

- Primitive/category: `WorkspaceRootDiscovery`, `BoundedFileListService`, Clay-owned file-browser composition, workspace command execution.
- Rust owners: `src/server/workspace.rs`, `src/shell/file_browser.rs`, `src/server/command_execution.rs`.
- Ops/facades: `op_clay_workspace_*`, `op_clay_commands_execute_command`, `runtime/js/workspace.js`, `runtime/js/commands.js`.
- Public docs: `docs/reference/clay-js-api/workspace/`, `docs/reference/clay-js-api/commands/server-execute-command.md`, `server-open-file.md`, `server-open-directory.md`, `server-reveal-in-tree.md`, and `docs/development/launch-and-gui-smoke.md#end-to-end-file-browser-workflow-smoke`.
- Hot-path policy: discovery/listing/opening are server/runtime work; typing, local paint, layout, scroll, and package JavaScript hot paths do not list directories or scan workspaces.
- Client-local scroll: `src/masonry_sdui.rs::SduiNativeState` keeps a vertical `scroll_offset` (pixels) for the Clay-owned left file-browser panel. `scrolls_point(size, point)` routes `PointerEvent::Scroll` to the file browser only when the pointer is inside the left panel; otherwise `src/masonry_editor.rs::on_pointer_event` scrolls the editor as before. `scroll_vertical_pixels`/`scroll_lines` treat positive deltas as scrolling down (revealing later rows), matching the editor scroll convention. The offset clamps to `[0, max_scroll]` where `max_scroll = (content_height - viewport_height).max(0)`, measured during paint with a `push_clip_layer` over the sidebar so scrolled-out rows never paint over the editor. Scrolling reveals only rows already present in the bounded snapshot and never relists directories, calls the server, runs package JavaScript, or enqueues workspace actions. The offset resets to zero whenever a new `SduiSnapshot` or `SduiTreeUpdate` is applied.
- Package rule: packages consume documented facades and inert commands; they do not contribute roots, marker tables, ignore rules, native widgets, or raw path passthrough.

## Invariants and Constraints

- Workspace roots are server-owned, canonicalized, deduplicated, and bounded.
- Marker files are a compiled closed set; packages cannot extend them.
- Directory listing is bounded by depth/count/child-scan and root-ignore byte/line/rule/rule-length limits. Unsupported root-ignore semantics abort visibly; they never degrade to a broader traversal.
- All file opens re-check root or selected-file grant authority server-side.
- Absolute paths are accepted only as explicit selected-file-style grants, not as raw package/client authority.
- Save/save-as/rename/delete/file watchers/autosave/conflict UX remain deferred.
- The implementation assumes Linux is the primary validation platform. Windows transport/dialog code remains a long-term target, but Linux failures on the host are blocking while Windows-only gaps are not blocking unless a task explicitly targets Windows.

## Tests

- `src/server/workspace.rs`: root discovery, explicit grants, root deduplication, bounded directory listing, `*` backtracking/`?`/Unicode/directory-only ignore rules, unsupported-rule and oversized-input fail-closed pages, traversal rejection, cancellation, child counts, and diagnostics.
- `src/shell/file_browser.rs`: SDUI tree shape, current-directory parent row, row/action source identity for nested files, directory-row navigation command IDs, fuzzy session filtering, command IDs, and list action opening through the workspace API.
- `src/server/command_execution.rs`: workspace open/directory-navigation/reveal/toggle execution, selected-file grants, missing arguments, and save-related command absence.
- `src/server/connection.rs`: `workspace_directory_action_sends_refreshed_file_browser_snapshot` verifies directory navigation returns a refreshed `SduiSnapshot`; `file_browser_open_uses_generic_open_document_followups` opens as client 99 and proves that same client can immediately submit an accepted edit with the returned lease.
- `src/masonry_sdui.rs`: `file_browser_scroll_reveals_later_rows_without_relisting`, `file_browser_scrolled_action_hits_visible_row`, and `scrolls_point_routes_scroll_to_file_browser_only_inside_left_pane` verify client-local file-browser scroll, scrolled action hit testing, and the scroll-routing boundary.
- `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`: public API docs/facades/registry coverage.
- `tests/manual_smoke_docs.rs::end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract` and `tests/fixtures/configuration/file-browser-workflow/init.js`: Linux manual smoke documentation for launch, selected-folder grant, directory navigation, Rust/TypeScript/JavaScript package activation, and copy-selection clipboard behavior.
- Focused commands:

```text
cargo test --lib server::workspace::tests::discover_root_for_path_finds_marker_ancestor --quiet
cargo test --lib server::workspace::tests::list_directory_returns_immediate_children --quiet
cargo test --lib shell::file_browser --quiet
cargo test --lib server::command_execution --quiet
cargo test --test protocol clay_js_api_inventory:: --quiet
cargo test --test protocol clay_js_doc_registry:: --quiet
cargo test --test protocol clay_js_facade_layout:: --quiet
```

## Related

- [Server File Workspace Model](server-file-workspace.md)
- [Command Registry](command-registry.md)
- [Transient Menu Session](transient-menu-session.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Configuration Runtime](configuration-runtime.md)
- [Phase 18.12 Workspace Discovery and File Browser Foundation Primitive Review](phase18.12-workspace-discovery-primitive-review.md)
- `plans/040-Phase18.12-Workspace-Discovery-and-File-Browser-Foundation.md`
