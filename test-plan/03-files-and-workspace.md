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

## Negative checks

- Opening files grants access to the selected file + workspace roots only —
  no broadened filesystem authority (holds per pane — Phase 22.2 opens are
  capability-gated through the same server path as before; Phase 22.3: each
  tab's connection carries its own grants, so opens are also per tab).
- A file cannot be opened twice across panes (F3c, F12a); the pane-scoped
  switcher never creates a second view of a document.
- Save/conflict paths never run package JavaScript or parser work in the
  keystroke/paint path.

## Known ceilings

- Save in some smoke fixtures is deliberately disabled (fixture docs say so).
- Windows dialog specifics belong to module 12.
