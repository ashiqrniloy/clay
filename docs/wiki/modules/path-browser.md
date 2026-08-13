# Path Browser (Phase 24.3)

Built-in dired-style filesystem browsing (`controlCenter.openPath`, “Browse
Filesystem”) — the second server-owned transient-menu session kind, a
sibling of the Control Center. It browses user-authorized paths with an
editable path bar, a derived fuzzy filter, descend/ascend/direct-jump
navigation, and primary/secondary activation (`Enter`/`Alt+Enter`). It is a
pure wiring phase over existing primitives: the Phase 18.8
`TransientMenuSession` state/projection, the Phase 24.1 server-owned session
store and menu round trip, and the Phase 24.2 shared fuzzy scorer and
generation-stamped command routing (`plans/083`).

## What it is

A user-authorized browse session that lives entirely server-side:

- **Seed** — the session opens on a canonical directory derived from the
  active document's directory, falling back to the bound tab's workspace
  root, then the server's current directory.
- **Path input** — the query line of the transient menu *is* the editable
  path bar; typing filters the installed listing, editing a directory
  prefix relists.
- **Navigation** — `Enter` descends into a directory (primary activation),
  `Backspace` on an empty filter ascends, a typed path with a trailing
  separator jumps directly, `Alt+Enter` opens a directory as the bound
  tab's workspace (secondary activation), `Escape` cancels.
- **Authority** — browsing is ephemeral: navigation alone creates no grant;
  activating a file converts browse authority into exactly one explicit
  `SingleFile` grant, and `Alt+Enter` on a directory into exactly one
  `Directory` root grant for the bound tab. Packages get no equivalent
  authority and cannot open, drive, intercept, or receive paths from the
  session.

The native file/folder dialogs (`documents.clientOpenFileDialog`,
workspace-open) remain the fallback capability issuers; path mode never
disables them. The shipped default is the Global `Ctrl+X Ctrl+F` sequence
chord (Phase 24.5; the pre-24.5 temporary default was `Ctrl+Alt+P`, the
command id never changed).

## Source files

- `src/shell/path_browser.rs` — `PathBrowserSession` (pure state machine:
  input parsing, filter derivation, transitions, activation resolution),
  `PathBrowserTransition` (`FilterOnly` / `Relist { target }`),
  `PathBrowserActivation` (`Descend` / `OpenFile` / `OpenWorkspace`),
  `PathBrowserEntry`.
- `src/server/workspace.rs` — the built-in user-browse listing primitive:
  `UserBrowseListingPlan`, `UserBrowsePage`/`UserBrowseEntry`/
  `UserBrowseEntryKind`, `UserBrowseError`, `traverse_user_browse_directory`
  (sync, bounded), `execute_user_browse_listing` (`spawn_blocking` wrapper),
  `WorkspaceState::document_canonical_path`, `resolve_user_browse_seed`.
- `src/server/menu_sessions.rs` — `ServerMenuSessionKind::PathBrowser`,
  `ServerMenuSessions::open_path_browser` /
  `install_path_browser` / `set_path_browser_error`, `MenuEdit`,
  `ServerMenuActivateOutcome` (`Navigate` / `OpenFile` / `OpenWorkspace` /
  `Dispatch`), kind-dispatching `set_query`/`backspace`/`activate`.
- `src/server/connection.rs` — `open_command_centre_session` (shared
  open helper for `controlCenter.open` and `controlCenter.openPath`),
  `path_browser_relist` (bounded relist on the blocking pool),
  `open_workspace_for_bound_tab` (shared with `TabCommand::OpenWorkspace`),
  and the `MenuQueryUpdate`/`MenuBackspace`/`MenuActivate` handler arms.
- `src/protocol/menu.rs`, `src/protocol/mod.rs` — `MenuBackspace` intent,
  `MenuActivate` activation `kind` (`Primary`/`Secondary`),
  `PROTOCOL_VERSION` 16, `controlCenter.openPath` declaration and default
  keymap.
- `src/client/mod.rs`, `src/masonry_pane_document.rs` — client intent
  enqueuers (`enqueue_menu_backspace`, activation kind) and
  `dispatch_server_menu_key` routing (Enter/Tab primary, Alt+Enter
  secondary, other Alt-chords fall through to the editor).
- `src/server/command_execution.rs`, `src/server/ops/keybindings.rs` —
  `OPEN_PATH_BROWSER_COMMAND_ID` registration (`server_intent`,
  `ServerFirst`) and the runtime-bindable allowlist entry.

## Seed resolution

`resolve_user_browse_seed(workspace, active_document_id, tab_root)` picks
the first that exists:

1. `WorkspaceState::document_canonical_path(document_id)`'s `parent()` —
   the active document's real canonical directory (works for
   `SingleFile`-grant documents whose true path lies outside workspace
   roots).
2. The bound tab's `workspace_root` string (welcome/scratch documents have
   no canonical path).
3. `std::env::current_dir()`, with a `.` fallback if the cwd is unset.

The active document id comes from `CommandExecutionTarget::ActiveDocument`,
so the seed is server-authorized by construction.

## Session state and transitions

`PathBrowserSession` holds: `canonical_dir` (last successfully listed
directory), the displayed path input, the derived filter fragment, bounded
installed entries, a persisted `selected_index`, and a sticky
`error: Option<String>`.

- **Input parsing** — `parse_input` splits at the last platform separator
  (`std::path::is_separator`) into a directory part (including its trailing
  separator) plus a filter fragment; an input with no separator is all
  filter with an empty directory part. No canonicalization happens in the
  session — `canonical_dir.join(dir_part)` is textual and the listing
  canonicalizes the target.
- **`set_input`** — relists only when the directory part is non-empty and
  textually differs from the last relisted `input_dir`; mid-path edits like
  `/home/arn/Pro` just re-filter. Absolute parts become the target,
  relative parts resolve as `canonical_dir.join(dir_part)`.
- **`backspace`** — a non-empty derived filter pops one character
  (`FilterOnly`); an empty filter ascends (`Relist { target: parent }`),
  and at the filesystem root `parent()` is `None` → no-op `FilterOnly`.
- **`install(page)`** — rewrites the displayed input to the canonical
  directory (`dir_display`: trailing separator, root `/` avoids a double
  separator), resets selection to 0, clears the sticky error, and caps
  entries defensively. This resolves the relative-path accumulation
  problem: the input always reflects the canonical truth after a relist.
- **Filtering** — empty filter: deterministic directory-first/name order
  via stable partition of the name-sorted listing; non-empty: the Phase
  24.2 shared fuzzy scorer (`score` desc, index asc) over installed
  metadata only — no filesystem work per keystroke.
- **Activation** — resolves only from installed entries via
  `filtered[selected_index]`, never typed or client-supplied paths:
  `Directory` → primary `Descend(canonical_path)` / secondary
  `OpenWorkspace(canonical_path)`; `File`/`Other` → primary
  `OpenFile(canonical_path)` / secondary `None`. Activation fails closed
  while a sticky error is set; selection clamps across filter changes and
  wraps via `rem_euclid` like the Control Center.
- **Projection** — prompt `Browse · {canonical_dir}`, query = input, inert
  `TransientMenuAction::new("")` items, empty states “Empty directory” /
  “No matches for {filter}”, same overlay composition and tokens as the
  Control Center (bottom anchor, `z.overlay`, Modal focus, hosted
  `MenuA11y`).

## Built-in user-browse listing primitive

`traverse_user_browse_directory` (in `src/server/workspace.rs`, `pub(crate)`,
reachable only from the built-in session):

- Canonicalizes the requested directory, verifies it is a directory, and
  returns depth-1 inert entries (name, canonical activation path, kind,
  optional size) sorted deterministically (name, insertion-bounded window)
  and capped at `max_entries` (default `TRANSIENT_MENU_MAX_ITEMS` = 256 —
  no new budget constant).
- Skips non-UTF-8 names, unreadable entries, and broken symlinks; applies
  **no** `.gitignore` or `DEFAULT_IGNORED_NAMES` filtering — a raw
  filesystem browse shows everything.
- Symlink targets are canonicalized before activation (`EntryKind` has no
  `Symlink` variant; `From<UserBrowseEntryKind>` for
  `FileBrowserEntryKind` maps links to files).
- Supports absolute/relative paths with `Path`/`PathBuf` platform
  separators; no shell/env/tilde expansion.
- `execute_user_browse_listing` wraps it in `tokio::task::spawn_blocking`
  with **no workspace/tab/menu mutex held**; caps exist because started
  blocking tasks are not abortable (`ponytail:` ceiling — cooperative
  `ListingCancelToken` cancels between entries, but a running blocking
  traversal runs to completion; keep plans depth-1 and entry-capped).
- The existing `prepare_directory_listing`/`traverse_directory` workspace
  listing path (depth ≤ 8, 1000 entries, ignored names) is untouched and
  remains the file-browser primitive.

## Protocol and wiring

- `MenuBackspace` is a new semantic intent beside `MenuQueryUpdate`
  (dedicated backspace rather than a full query update); `MenuActivate`
  carries a bounded `Primary`/`Secondary` activation kind (Enter/Tab vs
  Alt+Enter). `PROTOCOL_VERSION` bumped once (15 → 16). No path-specific
  wire variants and no filesystem paths/actions cross the wire; activation
  resolves server-side from installed entries, failing closed on unknown/
  stale session ids and unknown enum data. Control Center behavior is
  byte-for-byte equivalent.
- Client: `dispatch_server_menu_key` pops the mirrored
  `server_query_buffer` and sends `MenuBackspace`; Enter/Tab enqueue
  `MenuActivate Primary`, Alt+Enter `MenuActivate Secondary`; every other
  Alt-key chord falls through to the editor. The local pop is a
  common-case approximation — a stale mirror self-corrects on the next
  `TransientMenuSnapshot` resync.
- Server handler arms: `set_query`/`backspace` return `MenuEdit` whose
  `relist: Option<PathBuf>` triggers `path_browser_relist` only on
  directory-prefix change or empty-filter ascent (Control Center arm always
  `None`); `activate(kind)` returns `ServerMenuActivateOutcome` —
  `Navigate` keeps the session open with exactly one snapshot,
  `Dispatch`/`OpenFile`/`OpenWorkspace` cancel the session first
  (`TransientMenuClosed`) then dispatch. Sequential per-connection intent
  processing makes out-of-order installs impossible, so no epoch/generation
  stamping was needed.
- The connection `CommandIntent` handler special-cases both centre commands
  through `open_command_centre_session` (the only route that opens the
  session; packages cannot emit `CommandIntent` from JS, and generic
  `execute_command_intent("controlCenter.openPath")` returns nothing on the
  wire). The path session is the one built-in exception that acts as its
  own authorization event, replacing the native dialog as capability issuer
  for this flow.

## Open flows and grant conversion

- **File open** — primary activation on a file: cancel the session
  (`TransientMenuClosed`), then `open_selected_file_response` +
  `write_document_open_response`, reusing the Phase 22.2 Owner > Pending >
  Active pane routing (duplicate open focuses the owning pane; opens target
  the focused pane). No capability token — the browse activation itself is
  the authorization event converting ephemeral browse authority to one
  canonical `SingleFile` grant.
- **Workspace open** — secondary activation on a directory: cancel the
  session, then `open_workspace_for_bound_tab`: requires a bound tab,
  `workspace.add_root` (canonicalizes; `RootUnavailable`/`RootNotDirectory`
  fail closed; repeat opens dedupe by canonical path and reuse the root
  id), `TabRegistry::open_workspace`, broadcast the reconciled
  `TabRegistrySnapshot`, and refresh the tab's file-browser snapshot when
  the workspace pane is visible. Other tabs' roots/documents/grants/menus
  stay untouched. The same helper now backs `TabCommand::OpenWorkspace`,
  which consequently also pushes the file-browser refresh (consistent with
  the native-dialog flow).

## Lifecycle

- **Tab switch** (`TabCommand::Activate`): the server cancels the session
  (`TransientMenuClosed`) — cancel-on-tab-switch beats a hidden-but-alive
  menu.
- **Runtime reload**: generation replacement cancels the active session
  before `RuntimeStateSnapshot`; reopening allocates a fresh high-bit
  session id; stale-generation activation fails closed (unit-covered),
  cancel still works.
- **Reopen while open**: replace → `TransientMenuClosed(old)` then
  snapshot(new); stale intents → bounded `menu.unknown_session` Info
  diagnostic, never an error or disconnect.
- **Cross-client**: intents carry connection-scoped session ids; a foreign
  client's `MenuActivate` with another client's opaque id gets the bounded
  diagnostic while the owner's session stays cancellable.
- **Disconnect**: drop-on-exit sweeps the connection-local store.
- **Unlistable seed**: the session opens empty with a sticky error and
  stays cancellable — never a disconnect.

## Security and authority

- Browse authority is session-scoped, ephemeral, and Clay-owned:
  navigation creates no root or grant; activation converts into exactly one
  explicit grant (`SingleFile` on file open, `Directory` root on workspace
  open for the bound tab).
- Activation resolves only from server-held installed entries; typed paths
  never act directly. All targets canonicalized; symlink targets resolve to
  their canonical path before activation.
- Packages: no facade, op, SDUI action, or command callback can open,
  populate, intercept, or receive paths from the session;
  `controlCenter.*` ids are reserved and the runtime-bindable allowlist
  contains exactly `controlCenter.openPath`.
- Per-tab isolation: workspace opens rebind only the bound tab; foreign
  tabs' roots/documents/grants/menus are untouched.

## Performance

- One bounded `spawn_blocking` listing per directory-prefix change or
  empty-filter ascent; **zero** filesystem work for filter-only edits
  (installed-snapshot fuzzy scoring).
- No workspace/tab/menu lock held during listings; caps (depth 1,
  `TRANSIENT_MENU_MAX_ITEMS` entries, `TRANSIENT_MENU_MAX_QUERY_CHARS`
  input) keep blocking work bounded because started blocking tasks are not
  abortable.
- Snapshots stay under the 1 MiB frame ceiling (256-entry worst case
  serialized in `path_browser_snapshot_stays_under_frame_ceiling`);
  exactly one snapshot per accepted transition.
- No package JavaScript and no filesystem work on paint/layout/key/text
  hot paths; listings are never read on the paint path.

## Tests

- `src/shell/path_browser.rs` — 15 unit tests (seed display, filter without
  relist, directory-prefix relist, backspace filter-then-ascend, install
  canonicalization, directory-first order, fuzzy ranking, selection clamp/
  wrap, oversize clamps, sticky error suppression, activation resolution,
  descend target, no-Symlink conversion, projection).
- `src/server/workspace.rs` — 8 `user_browse` tests (bounded windows,
  deterministic order, non-directory/error paths, seed resolution).
- `src/server/menu_sessions.rs` — 11 tests (navigate relists, activation
  dispatch incl. `OpenFile`/`OpenWorkspace` outcomes, no-op helpers on
  Control Center, cancel clears store + fresh id, frame-ceiling snapshot).
- `src/server/connection.rs` — 10 e2e tests (open from keybinding +
  catalogue, sticky-error unlistable seed, descend/ascend/direct jump,
  file open converts browse → `SingleFile` grant, workspace open rebinds
  only the bound tab + vanished-directory denial, navigation-only creates
  no grants, cross-client denial, tab-switch + disconnect survival,
  reload dismissal).
- `src/server/js_runtime.rs` — default/unbind/rebind of `Ctrl+X Ctrl+F`
  through `clay:keybindings`.
- `src/protocol/mod.rs` — default keymap contains the path-browser binding.
- Manual plan: `test-plan/03-files-and-workspace.md` F17–F29,
  `test-plan/10-keybindings-and-commands.md` K48–K54.

Run with:

```text
cargo test --lib path_browser --quiet
cargo test --lib user_browse --quiet
cargo test --lib server::menu_sessions --quiet
cargo test --lib server::connection::tests --quiet
```

## Related

- [Transient Menu Session](transient-menu-session.md) — the shared state model
- [Transient Menu Round Trip](transient-menu-round-trip.md) — wire DTOs, intents, store, client routing
- [Control Center](control-center.md) — the sibling server-owned session kind
- [Fuzzy Matching](fuzzy-matching.md) — the shared scorer used for filter derivation
- [Workspace File Browser](workspace-file-browser.md) — the workspace-root-bound listing and SDUI tree
- [Client File Dialog](client-file-dialog.md) — the native-dialog fallback capability issuer
- [Tabs and Clients](tabs-and-clients.md) — per-tab workspaces and tab-switch cancellation
- `docs/reference/primitives/registry.md` — BuiltInUserBrowseListing row
- `docs/reference/clay-js-api/configuration.md` — Phase 24.3 configuration review
- `docs/development/file-open-save-reload-workflow.md` — browse → grant conversion
- `.agents/skills/project-patterns/references/authority-boundaries.md` — built-in browse grant
- `plans/083-Phase24.3-Path-Mode-Dired-Style-Filesystem-Browsing.md`
