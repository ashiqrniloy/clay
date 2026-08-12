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
