# 03 — Files and Workspace

Open/save/reload, dirty state, conflict recovery, workspace browser.
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
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
```

Open `/tmp/clay-manual` as the workspace.

## Open

| # | Action | Expected |
|---|--------|----------|
| F1 | `Ctrl+O`, select `b.md` | File replaces buffer; markdown mode/decorations activate; native dialog on your platform |
| F2 | Cancel the dialog | No-op, not an error |
| F3 | Open a second file via workspace browser/fuzzy open | Multi-document session; switcher (`clientShowOpenDocuments`) lists both |

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

## Negative checks

- Opening files grants access to the selected file + workspace roots only —
  no broadened filesystem authority.
- Save/conflict paths never run package JavaScript or parser work in the
  keystroke/paint path.

## Known ceilings

- Save in some smoke fixtures is deliberately disabled (fixture docs say so).
- Windows dialog specifics belong to module 12.
