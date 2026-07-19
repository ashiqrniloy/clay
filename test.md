# Phase 19 + Phase 20 Manual Test Plan (real `init.js`)

Use your real `~/.config/clay/init.js`. Do **not** use `smoke-gui` or config fixtures.

## 0. Launch

```bash
cd /home/arn/Projects/clay
cargo run
```

Expected: Clay window opens, status shows connected, your packages/LSP bindings from `init.js` load.

---

## 1. Open a file (Phase 19 / 20 file dialog)

1. Press `Ctrl+O`.
2. Native file picker opens (xdg-desktop-portal on Linux).
   - If nothing appears and Clay freezes, that was the pre-fix UI-thread portal deadlock; rebuild/restart Clay first.
3. Open `/tmp/src/main.rs` (or any editable UTF-8 file).
   - Default filter is Markdown; switch the dialog filter to **All files** to select `.rs` / other non-Markdown files.
4. Cancel once and confirm cancel is a quiet no-op (no crash, no error menu).

Pass: file text appears; status shows connected/editable; document name visible.

### Open a directory

1. Press `Ctrl+Shift+O`.
2. Select a directory.
3. Confirm the selected directory becomes a workspace root and its files appear in Clay's workspace browser.
4. Select a file in that browser and confirm it opens in the editor.

Pass: file and directory chooser paths both complete through server validation; cancel remains a no-op.

---

## 2. Clipboard copy / cut / paste (Phase 20) — **verify paste fix**

Native chords (no `bindKey` required):

| Chord | Action |
|-------|--------|
| `Ctrl+C` | Copy selection |
| `Ctrl+X` | Cut selection |
| `Ctrl+V` | Paste |

Steps:

1. Select a few words in the open file.
2. `Ctrl+C`.
3. Move the caret elsewhere in the **same** document.
4. `Ctrl+V`.
5. Confirm the copied text is inserted at the caret.
6. Select other text, `Ctrl+X` — selection is removed and clipboard updated.
7. `Ctrl+V` elsewhere — cut text inserts.
8. Optional: paste into an external app after `Ctrl+C` to confirm system clipboard write still works.

Pass: paste works inside Clay after copy/cut. Fail was: copy worked externally, paste inside Clay did nothing (fixed by handling Masonry `ClipboardPaste`).

---

## 3. Undo / redo (Phase 20)

Native chords:

| Chord | Action |
|-------|--------|
| `Ctrl+Z` | Undo |
| `Ctrl+Shift+Z` or `Ctrl+Y` | Redo |

Steps:

1. Type a short word, then paste something.
2. `Ctrl+Z` undoes paste; another `Ctrl+Z` undoes typing.
3. `Ctrl+Y` (or `Ctrl+Shift+Z`) redoes.
4. After undo, type new text — redo stack clears (new branch).

Pass: undo/redo reverse ordinary local edits; dirty marker tracks edits.

---

## 4. Save / dirty / conflict (Phase 20)

Requires `bindKey("Ctrl+S", "clay.documents.serverSaveDocument", …)` in `init.js` (you already have this).

Steps:

1. Edit the file → status shows dirty (`— Dirty` / dirty marker).
2. `Ctrl+S` → dirty clears; file on disk updates.
3. Edit again (leave dirty). In another terminal:

   ```bash
   echo '// external' >> /tmp/src/main.rs
   ```

4. `Ctrl+S` → stale-save recovery menu (reload / keep / defer).
5. Try each option once across runs:
   - **Reload** replaces buffer from disk, clears dirty.
   - **Keep / Defer** leaves local text; no forced overwrite.

Pass: save works; conflict menu appears for stale on-disk metadata.

---

## 5. Multi-document sessions (Phase 20)

Requires `bindKey("Ctrl+Shift+E", clientShowOpenDocuments(), …)`.

Steps:

1. With file A open and dirty (optional), `Ctrl+O` and open a second file B.
2. Confirm A is retained (not replaced away forever).
3. `Ctrl+Shift+E` → open-documents menu lists both; dirty/active markers show.
4. Activate A → caret/text/dirty state restored.
5. Activate B again.

Pass: switching preserves per-document text, caret, dirty, undo history.

---

## 6. Pending-edit / recovery chrome (Phase 20)

Optional bindings you have:

- `Ctrl+Shift+Z` → `clientRequestResync`
- `Ctrl+Shift+D` → `clientDismissRecovery`

Steps:

1. While connected, make a few quick edits; status may show pending-edit count briefly.
2. If a sync recovery / rejection menu appears: Resync and Dismiss both work.
3. `Ctrl+Shift+D` dismisses recovery chrome without escalating authority.

Pass: recovery is visible in GUI status/menus (not stderr-only).

---

## 7. IME / composition (Phase 20)

If ibus/fcitx is available:

1. Switch to a CJK or dead-key input method.
2. Start composing — preedit underline appears; buffer text does not change yet.
3. Commit — text inserts once.
4. Start composing, press Escape or click away — composition cancels; no commit.

Pass: paint-only preedit; commit is the only buffer mutation.

---

## 8. Hot reload / behavior update (Phase 19)

Requires `bindKey("Ctrl+Shift+R", "clay.runtime.reloadConfiguration", …)`.

Steps:

1. With Clay running, edit `~/.config/clay/init.js` (e.g. temporarily change a harmless bind or comment).
2. Focus Clay, press `Ctrl+Shift+R`.
3. Confirm reload applies without restart (new binding works, or documented reload status).
4. Restore `init.js` and reload again.

Pass: configuration/behavior reloads live; editor stays usable; no panic.

Also smoke:

1. Keep typing during/after reload — local typing stays responsive.
2. Open documents survive reload (or documented reconnect/resync behavior is clear).

---

## 9. Language intelligence (sanity with your init.js)

Your bindings:

| Chord | Command |
|-------|---------|
| `Alt+H` | hover |
| `Alt+D` | go to definition |
| `Alt+A` | code actions |
| `Alt+S` | signature help |
| `Ctrl+Space` | completion trigger |

Steps (in a Rust file under your workspace):

1. `Ctrl+Space` on a word prefix → completion menu.
2. `Alt+H` on a symbol → hover menu/panel.
3. `Alt+D` → navigates or offers definitions.

Pass: menus are inert UI; accept/navigate does not grant extra FS/shell authority.

---

## 10. Accessibility / status chrome (quick check)

1. Confirm status line remains readable (theme contrast).
2. Dirty, pending, and recovery states show in status text / accessibility label when active.
3. Open-documents count appears when more than one document is open.

---

## Suggested order for a focused pass

1. Launch (§0)
2. Open `/tmp/src/main.rs` (§1)
3. **Copy → move caret → paste** (§2) ← primary regression for this session
4. Undo/redo (§3)
5. Save + conflict (§4)
6. Second file + switcher (§5)
7. Hot reload (§8)
8. Optional: IME (§7), recovery (§6), LSP (§9)

---

## Notes

- Native clipboard chords (`Ctrl+C/X/V`, `Ctrl+Z/Y`) are handled in the editor; they do not require `bindKey`.
- `Ctrl+S`, `Ctrl+O`, open-documents, reload, and recovery chords depend on your `init.js`.
- Do not use `cargo run -- smoke-gui` for this plan; that path loads fixtures, not your real config.
