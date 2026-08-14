# 03 — Files and Workspace

Open/save/reload, dirty state, conflict recovery, workspace browser,
multi-document switching across panes (Phase 22.2: each pane hosts one
document of the workspace; the open-documents switcher follows pane focus).
Authoritative detail: `docs/development/file-open-save-reload-workflow.md`
(read it for capability tokens, platform matrix, and authority boundaries).

## Setup

```bash
mkdir -p /tmp/clay-manual && cd /tmp/clay-manual
echo "hello" > a.txt
echo "# Doc" > b.md
mkdir sub && echo "nested" > sub/c.txt
```

Minimal init.js additions (or use `examples/init.js`):

```js
bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
```

Open `/tmp/clay-manual` as the workspace.

## Open

| # | Action | Expected |
|---|--------|----------|
| F1 | `Ctrl+O`, select `b.md` | File replaces buffer; markdown mode/decorations activate; native dialog on your platform |
| F2 | Cancel the dialog | No-op, not an error |
| F3 | Open a second file via workspace browser/fuzzy open | Multi-document session; switcher (`clientShowOpenDocuments`) lists both |
| F3a | With the window split into 2 panes, open a second file from the OTHER pane (browser/fuzzy open while that pane is focused) | The new document opens in the pane that requested it (Phase 22.2 focused-pane targeting); the switcher on either pane lists BOTH documents (`pane 1: ...`, `pane 2: ...`). Phase 22.3: the request targets the ACTIVE TAB's focused pane — each tab is an independent client view with its own pane tree and documents (see module 14, T12) |
| F3b | From the switcher, select the OTHER pane's entry | That pane switches to its listed document and receives focus; the requesting pane keeps its own document |
| F3c | From a pane, open a file that is already open in the other pane | No second view — the owning pane is focused instead (duplicate-open rule); the file's caret/content stay untouched |

## Save and dirty state

| # | Action | Expected |
|---|--------|----------|
| F4 | Type in `a.txt`, check status | Dirty indicator / pending edit visible |
| F5 | `Ctrl+S` | Server-first save; dirty clears; disk file updated (`cat a.txt`) |
| F6 | Switch to `b.md` | Per-document dirty is independent |

## Reload and conflicts

| # | Action | Expected |
|---|--------|----------|
| F7 | With a clean document, edit `a.txt` externally (`echo more >> a.txt`), then reload via documented command | New content loads |
| F8 | Stale save: edit file on disk after opening, then `Ctrl+S` without reload | Save conflict → recovery menu (reload / keep edits / compare later), no silent overwrite |
| F9 | Dirty reload conflict: edit in Clay, change disk file, reload | Recovery menu, edits preserved as an option |
| F10 | During conflict | Editor remains accessible; recovery text is sanitized (no raw paths) |

## Workspace browser

| # | Action | Expected |
|---|--------|----------|
| F11 | Toggle the workspace file browser (documented workspace command) | Browser shows `a.txt`, `b.md`, `sub/`; fixed panel resizes the editor main rect (does not cover text) |
| F12 | Fuzzy-open `sub/c.txt` from browser | Opens in editor |
| F12a | Fuzzy-open `sub/c.txt` from a split pane while it is already open in another pane | Focuses the owning pane; no duplicate view (see F3c) |

## Phase 22.8 workspace-pane and hidden-open checks

Deep reference: `docs/development/file-open-save-reload-workflow.md` and
`docs/development/launch-and-gui-smoke.md` (End-to-end file browser workflow).
These checks use the per-tab server workspace and the existing `Ctrl+O`
binding from Setup.

| # | Action | Expected |
|---|--------|----------|
| F13 | Fresh launch with a workspace root and no prior toggle | Workspace pane starts hidden; the editor occupies the left slot; no file-browser `Panel`/`List` is visible |
| F14 | Press `Ctrl+B` | Workspace pane appears for the active tab; header contains `Workspace`, the folder name, and the full workspace location |
| F15 | Press `Ctrl+B` again | Pane disappears; the editor reclaims the left slot; no other tab or document state changes |
| F16 | While pane is hidden, press `Ctrl+O` and select `b.md` | Native file dialog opens normally; selected document opens in the active pane despite hidden workspace chrome; cancellation remains a no-op |

## Path Browser — dired-style filesystem browsing (Phase 24.3)

Built-in server-first browse workflow (`controlCenter.openPath`, shipped with
the temporary default `Ctrl+Alt+P` chord — no init.js needed; see module 10
K48–K53 for rebind/unbind). Deep reference:
`docs/reference/clay-js-api/configuration.md` (Phase 24.3 review),
`docs/development/file-open-save-reload-workflow.md` (browse → grant
conversion). Setup additions:

```bash
mkdir -p /tmp/clay-manual/sub2 && echo "deep" > /tmp/clay-manual/sub2/d.txt
ln -s /tmp/clay-manual/a.txt /tmp/clay-manual/link.txt
```

| # | Action | Expected |
|---|--------|----------|
| F17 | With `a.txt` open, press `Ctrl+Alt+P` | Overlay opens, prompt `Browse · /tmp/clay-manual` (seed = active document's canonical directory), query `/tmp/clay-manual/`; entries directory-first: `sub/`, `sub2/`, then files `a.txt`, `b.md`, `link.txt`; exactly one snapshot (automated: `path_browser_opens_from_keybinding_and_control_center_catalogue`, `default_keymaps_contain_path_browser_open_binding`) |
| F18 | Type `sub` | Fuzzy filter narrows to `sub/` + `sub2/` (case-insensitive); no filesystem work per keystroke — visuals update only from pushed snapshots; selection persists |
| F19 | `Enter` on `sub/` | Descends: prompt `Browse · /tmp/clay-manual/sub`, query `/tmp/clay-manual/sub/`, item `c.txt`; session id unchanged; exactly one snapshot (automated: `path_browser_navigates_descend_ascend_and_direct_jump`) |
| F20 | `Backspace` with an empty filter | Ascends to the parent: prompt `Browse · /tmp/clay-manual`, query back to `/tmp/clay-manual/`; at the filesystem root `/`, `Backspace` is a no-op |
| F21 | Edit the path directly: replace the query with `/tmp/clay-manual/sub2/` (trailing slash) | Direct jump: relists to `sub2` — prompt `Browse · /tmp/clay-manual/sub2`, item `d.txt`; a typed path WITHOUT a trailing slash is a filter within the current directory, not a jump |
| F22 | Invalid path recovery: set the query to `/root/missing/` | Sticky bounded status (listing failed), items suppressed, menu stays open; `Backspace` clears the error and edits the path; fixing the path relists (automated: `path_browser_opens_with_sticky_error_for_unlistable_seed`) |
| F23 | From `sub`, filter `c.txt`, `Enter` | File opens in the active pane: `TransientMenuClosed` first, then `DocumentOpened`; the browse activation is the grant conversion — the file now holds one explicit `SingleFile` grant (automated: `path_browser_open_file_converts_browse_to_single_file_grant`) |
| F23a | Re-open the already-open `c.txt` from the browser | Duplicate-open rule: no second view — the owning pane is focused instead (see F3c) |
| F23b | With a 2-pane split, open the path browser while the OTHER pane is focused, then open a file | Document opens in the focused pane (Phase 22.2 active-pane targeting); the switcher lists both documents |
| F24 | On a directory entry, `Alt+Enter` | Current tab's workspace rebinds to that directory: menu closes (`TransientMenuClosed`), file browser refreshes to the new root, other tabs' workspaces/documents untouched (per-tab isolation: module 14 T12); the activation converts browse authority to one explicit `Directory` root grant; repeating on the same directory reuses the same root id (automated: `path_browser_workspace_open_rebinds_only_bound_tab`) |
| F25 | `Escape` | Menu closes; no command runs; no root or grant created |
| F26 | Seed fallback: fresh tab with only the welcome document, press `Ctrl+Alt+P` | Seeds from the tab's workspace root (welcome doc has no canonical path); with no workspace bound, falls back to the server's current directory |
| F27 | Native-dialog fallback: `Ctrl+O`, and the workspace-open dialog | Unchanged (F1, F16) — native dialogs remain the fallback capability issuers; path mode never disables them |
| F28 | Switch tabs while the path browser is open (module 14 tab bar) | Session dismissed with explicit `TransientMenuClosed`; stale intents for the old session → bounded `menu.unknown_session` diagnostic; the other tab is unaffected (automated: `path_browser_survives_tab_switch_and_disconnect`) |
| F29 | Trigger a runtime reload while the path browser is open (`Ctrl+Shift+R`) | Generation replacement cancels the session (`TransientMenuClosed`) before `RuntimeStateSnapshot`; reopening uses a fresh session id (automated: `path_browser_activation_after_runtime_reload_fails_closed`) |
| F30 | Open Path Browser and inspect its surface/accessibility tree | One centered Spotlight-style panel dims full window; exactly one named modal Dialog contains Menu/MenuItems and a polite Status with `0 results`, `1 result`, or `{n} results`; path prompt is bounded/sanitized. |
| F31 | Click scrim, type unsupported modifier/function input, paste, or start IME while Path Browser is open | Scrim/input is contained; no editor text/caret/selection mutation or path authority change. Escape closes and returns focus to originating pane. |

## Plan 087 entry-state steps (welcome)

| # | Action | Expected |
|---|--------|----------|
| F32 | Fresh launch on an empty tab (no restored document) | Welcome entry state is the Clay-owned surface: `Welcome to Clay` with `Open File` and `Open Folder` buttons; no prototype/stale document text; status bar normal |
| F33 | Activate `Open File` (click or Space/Enter on the button) | Native file dialog opens (user dialog, no implicit authority); cancelling leaves the welcome state intact |
| F34 | Select a file in the native dialog and accept | Document opens in the pane; welcome hides; status/entry show `doc N` with the basename only (e.g. `review.md — doc 3 — v1`) |
| F35 | Repeat with `Open Folder` and accept a directory | Workspace root rebinds to the chosen folder (existing validated-grant path); welcome stays absent while a document is open |
| F36 | Close the last document/pane | Pane returns to the welcome state (`welcome_visible`), buttons functional again |
| F37 | Negative: check AT-SPI names for the welcome state | Labels show basenames and sanitized copy only — no host path segments (`/home/…`, `/tmp/…`) or secrets |

## Linux execution record (Plan 086 task 11, 2026-08-14)

- **PASS — restored multi-document panes:** the isolated v2 layout restored `a.txt` and `b.md` into separate panes. AT-SPI exposed `Pane 1 of 2: editor` / `Pane 2 of 2: b.md`, separate editor/status nodes, and `Open docs: 2`; the connection remained live.
- **BLOCKED — native dialog steps (F1/F2/F16/F23):** this host's portal path could open a dialog but could not safely target/select its UI from the agent, so file-picker selection/cancellation was not re-run. No product failure inferred.
- **FAIL/BLOCKER — dirty close path:** typing into `a.txt` and pressing `Ctrl+Alt+W` reproduced a client panic, `accesskit_consumer-0.31.0/src/tree.rs:34:13: Focused ID #4 is not in the node list`; the isolated server stayed alive. Evidence: `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log`. Clean pane close passed separately and announced `Closed pane; 1 pane remains`.
- **PASS — negative checks:** status/entry labels showed sanitized basenames and bounded diagnostics, not `/tmp` paths or document secrets. HOME/XDG config/data roots were isolated under the mode-700 temporary root; no ambient config was used.

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — F32/F33/F34/F37:** from the welcome state, AT-SPI `click` on the `Open File` button opened the native Nautilus Open File dialog (no implicit authority — a real user dialog was required); typing the workspace path into the dialog's location box and accepting opened `review.md` as `doc 3` (`DocumentOpened` in the client log, entry `Clay — Connected — Editable — review.md — doc 3 — v1`, welcome hidden). AT-SPI names showed only basenames — no `/tmp/…` or `/home/…` segments.
- **Coverage note:** F35 (Open Folder) and F36 (close-last-pane returns to welcome) were not re-run this session; F36's welcome-return is covered by unit tests (`close_pane` resets to welcome) and S35 below.

## Negative checks

- Opening files grants access to the selected file + workspace roots only —
  no broadened filesystem authority (holds per pane — Phase 22.2 opens are
  capability-gated through the same server path as before; Phase 22.3: each
  tab's connection carries its own grants, so opens are also per tab).
- Path-mode navigation alone creates no roots or grants: browse/filter/
  descend/ascend/jump/cancel leaves the tab's directory roots and grants
  unchanged (automated: `path_browser_navigation_only_creates_no_grants`).
- A file deleted between listing and activation → `FileOperationFailed`,
  no new grant; a directory that vanishes before `Alt+Enter` →
  `FileOperationFailed` (`NotFound`), tab root unchanged (automated:
  `path_browser_workspace_open_rejects_vanished_directory`).
- Cross-client/cross-tab activation is denied: while tab A has the path
  browser open, an intent carrying A's opaque session id from tab B yields a
  bounded `menu.unknown_session` diagnostic; A's session stays cancellable
  (automated: `path_browser_cross_client_activation_denied`).
- Packages cannot open, drive, intercept, or receive paths from the built-in
  session — no package facade or op exists (module 09; automated:
  `package_command_lane_cannot_open_path_browser`); reserved ids
  (`controlCenter.*`, `shell.*`) cannot be registered by packages (K46).
- Symlink entries are canonicalized: `link.txt` opens `a.txt`'s content (and
  is the same document under the duplicate-open rule, F23a); descending into
  a symlinked directory targets its canonical path.
- A file cannot be opened twice across panes (F3c, F12a, F23a); the pane-scoped
  switcher never creates a second view of a document.
- Save/conflict paths never run package JavaScript or parser work in the
  keystroke/paint path.

## Path-mode known ceilings (not bugs)

- Listing is depth-1 per directory, capped at 256 entries
  (`TRANSIENT_MENU_MAX_ITEMS`); input capped at 256 chars
  (`TRANSIENT_MENU_MAX_QUERY_CHARS`); one bounded scan per directory change,
  zero scans per filter keystroke; snapshots stay under the 1 MiB frame
  ceiling (automated: `path_browser_snapshot_stays_under_frame_ceiling`).
- `Ctrl+Alt+P` is a temporary default — Phase 24.5 replaces it with sequence
  defaults without changing the command id; `Alt+Enter` is the fixed
  secondary activation (not configurable in 24.3).
- Windows dialog specifics belong to module 12.
