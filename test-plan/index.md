# Clay Manual Test Plan — Index

Manual verification guide for the whole Clay application. Automated suites
(`cargo test`, 4 declared suites) gate every change; these documents cover
what only a human at a real keyboard/screen can verify: rendering, blink,
ligature glyphs, IME feel, native dialogs, focus, timing.

## How to use this plan

1. Build once: `cargo build` (or `cargo run` which builds on demand).
2. Pick the module file(s) relevant to what you changed — the table below.
3. Each module file is self-contained: setup, numbered steps with expected
   results, negative checks, and known ceilings that are NOT bugs.
4. Record results inline (copy the table or keep notes). Failures that match
   a file's "known ceilings" section are expected behavior, not defects.
5. Every plan document that changes user-visible behavior must update the
   affected module file(s) and this index (enforced by the create-plan skill
   manual-test-plan task).

## Prerequisites (all modules)

- Linux host (primary platform), Rust toolchain, `cargo`.
- Optional: `~/.config/clay/init.js` (canonical example: `examples/init.js`).
- Scratch workspace: `mkdir -p /tmp/clay-manual` with sample files (each
  module file lists the files it needs, or points at a shared setup).
- Font for ligature checks: Fira Code (`FiraCode Nerd Font Mono` works).

## Module map

| # | Module file | Covers | Deep-reference doc |
|---|-------------|--------|-------------------|
| 01 | [Launch and connection](01-launch-and-connection.md) | server/client lifecycle, lease, read-only observer, restart, status line | `docs/development/launch-and-gui-smoke.md` |
| 02 | [Configuration (init.js)](02-configuration-init-js.md) | init.js evaluation, modular loading, diagnostics, live reload, planned-API denial | `docs/reference/clay-js-api/configuration.md`, `examples/init.js` |
| 03 | [Files and workspace](03-files-and-workspace.md) | open/save/reload, dirty state, conflicts, file browser, multi-document | `docs/development/file-open-save-reload-workflow.md` |
| 04 | [Core editing](04-core-editing.md) | typing, undo/redo, clipboard, newline/indent rules, IME preedit | — |
| 05 | [Movement and selection](05-movement-and-selection.md) | word/paragraph/line movement, sticky column, line/word selection, prose vs code | `docs/development/manual-editor-capabilities-test-plan.md` |
| 06 | [Multi-cursor editing](06-multi-cursor.md) | Ctrl+D match selection, column select, add-cursor, cursor undo, escape priority | `docs/development/manual-editor-capabilities-test-plan.md` |
| 07 | [Caret and typography](07-caret-and-typography.md) | caret shape/blink, width, ligature policies per font role | `docs/development/manual-editor-capabilities-test-plan.md` |
| 08 | [Syntax and text objects](08-syntax-and-textobjects.md) | grammar highlighting, textobject/smart-select, engine tiers, advisory degrade | `docs/development/manual-editor-capabilities-test-plan.md` |
| 09 | [Packages and modes](09-packages-and-modes.md) | package loading, mode classification/activation, settings UI, theme switching | `docs/development/launch-and-gui-smoke.md` |
| 10 | [Keybindings and commands](10-keybindings-and-commands.md) | bindKey override, unbind, deny-by-default, execution push channel | `docs/development/manual-editor-capabilities-test-plan.md` |
| 11 | [Performance](11-performance.md) | large files, scroll/type latency, parse feel | `docs/development/performance.md` |
| 12 | [Platform: Windows](12-platform-windows.md) | MSVC toolchain, named pipes, native dialogs | `docs/development/windows.md` |

## Coverage matrix (what to run when)

| Change touches | Minimum manual modules |
|----------------|------------------------|
| Client rendering / surface / paint | 01, 04, 07 |
| Editor movement/selection/caret primitives | 05, 06, 07, 10 |
| Typography / font features | 07 |
| Protocol / IPC / connection | 01, 03, 04 |
| Configuration surface / init.js APIs | 02, 10 + the module of the feature configured |
| Syntax / grammar / decorations | 08 |
| Package loading / modes / trust boundary | 09, 02 |
| File IO / save / dialogs | 03 |
| Keybinding routing / commands | 10, 05 |
| Anything user-visible | 01 always (launch gate) |

## Conventions

- Steps are numbered `<module><step>` (e.g. `E3`) so failures can cite them.
- "Expected" columns describe the product contract; visual judgments
  (smoothness, glyph shape, blink rhythm) are part of the check.
- Restart = `cargo run` again; live reload = settings appearance switch
  (module 02) unless stated otherwise.
