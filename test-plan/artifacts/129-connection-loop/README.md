# Plan 129 connection-loop-decomposition manual-pass artifacts (2026-09-20)

Regression-only manual pass for the pure refactor of `handle_connection_loop`
into a dispatcher (`src/server/connection/{mod,documents,menus,tabs,workspace,runtime}.rs`)
plus the `Box::pin` cold-path future-size work. Modules re-run: 04 (core
editing), 06 (multi-cursor), 07 (caret/typography), 10 (keybindings/commands).

## Harness

- `run-live.sh start editing|cursors|cursorstyle|commands|chords [bytes] | stop` —
  isolated mode-700 root (`/tmp/clay-plan129-live`), private server socket,
  fixture workspace, tree-kill teardown (models `scripts/capture-ui-review.sh`,
  plan 126 `launch-live.sh`, plan 127 `run-live.sh`).
- `probe.py` — AT-SPI probe: `editor`, `text`, `mark`, `rows`, `dump`, `focus`,
  `act <role> <name-substring>` (invokes an AT-SPI action, e.g. the shell's
  `palette Ctrl X O` button), `set <role> <name> <text>` (EditableText).
- `portal-shot.py` — portal screenshot cropped to the Clay window.
- `init-completion.js` / `init-chords.js` / `init-cursor-style.js` /
  `init-invalid-caret.js` — module fixtures (completion trigger on `Ctrl+J`
  because this host's GNOME owns `Ctrl+Space`; two-stroke chord; typography +
  hollow-block caret; deny-by-default caret shape).

## Host ceiling observed this run (new)

`xdg-desktop-portal-gnome` **segfaults** (`g_hash_table_lookup` assertion, then
`code=dumped, status=11/SEGV`) on RemoteDesktop keyboard sessions on this host;
the service restarts and the in-flight keystroke is dropped
(`journalctl --user`: 15:17:22, 15:18:51, … every keyboard batch). Keystrokes
land only in short bursts right after a fresh app launch, so every step below
was driven in a fresh run and verified against the live AT-SPI tree; steps that
needed more input than the window allowed stayed `UNRESOLVED` instead of being
inferred. Plan 126/127 live records (2026-09-19) predate this crash loop.

`wtype` is installed but unusable (GNOME 50 has no virtual-keyboard protocol);
`/dev/uinput` is denied and there is no `ydotoold` socket, so pointer input is
unavailable (`computer_use_linux_click` by coordinate fails). AT-SPI actions on
named nodes work and were used where a step did not need typing.

## Artefacts

- `live-editing/` — module 04 run on the 65 KiB `review.rs` fixture: `steps.txt`
  (run 1: typing/backspace/indent/electric-brace/pairs/Tab/undo), `steps2.txt`
  (run 2: typing + undo + redo attempts), `screenshot-edited-tail.png`,
  `server.log`, `client.log`, `tree-final.txt`.
- `live-cursorstyle/` — module 07: `screenshot.png` / `screenshot-focused.png`
  (ligature sample line, hollow-block caret config), `invalid-style.log`
  (`editor.invalid_set_cursor_style` diagnostic), `steps.txt`,
  `live-cursorstyle.txt`, `steps-completion.txt` (dropped `fn` attempt).
- `live-commands/` — module 10: `live-commands.txt` (palette opened through the
  shell button, 88-row catalogue, reload-row activation), `live-commands-tree2.txt`
  (full tree with the palette open), `live-commands-server-before.log` and
  `server-after-reload.log` (init.js re-ran when `RuntimeReloadConfiguration`
  was activated).
- `automated-companions.txt` — fresh on the refactored tree: `cargo test --lib
  connection::` 96 passed; `cargo test --lib control_center` 28 passed;
  `cargo test --lib completion::` 31 passed; frontend `npx vitest run src/editor`
  9 files / 75 tests passed.

Findings: module 04 E4 **FAIL** (comment continuation declared but not
implemented; see `test-plan/04-core-editing.md`) and the stale module 07 T25
gutter expectation (the full-bleed editor has no gutter by design,
`docs/wiki/modules/react-shell.md`). No step was weakened; `UNRESOLVED` entries
name the host reason.
