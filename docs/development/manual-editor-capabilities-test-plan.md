# Manual Test Plan: Editor Movement, Caret, Ligatures, Multi-Cursor, Text Objects (Plan 071)

End-user workflow verification for everything Plan 071 (tasks 0–22) shipped:
movement primitives, caret styling/blink, font ligatures, unified multi-cursor
selection, tree-sitter text objects + smart select, package prose movement,
keybinding override, and the `editor-control` execution push channel.

Everything below runs through the real product path: `~/.config/clay/init.js`
+ bundled `@clay/*` packages + default keybindings. No test-only code paths.

## 0. Setup

### 0.1 Scratch workspace

```bash
mkdir -p /tmp/clay-manual && cd /tmp/clay-manual
```

`/tmp/clay-manual/test.rs`:

```rust
// Ligature sample: =>  !=  ==  ->  ::  ||  >=

fn compute_total_value(firstItem: i64, second_item: i64) -> i64 {
    let camelCaseCounter = firstItem + second_item;
    if camelCaseCounter >= 100 {
        return camelCaseCounter;
    }
    camelCaseCounter
}

// Paragraph two starts after a blank line.
// It contains snake_case_words for prose comparison.

fn main() {
    let total = compute_total_value(1, 2);
    println!("{total}");
}
```

`/tmp/clay-manual/test.md`:

```markdown
# Movement Sample

First paragraph with snake_case_words and camelCaseWords mixed together.
It should take several word stops to cross this line.

Second paragraph after a blank line. Word movement should jump between
these two paragraphs with Ctrl+Up and Ctrl+Down.
```

### 0.2 `~/.config/clay/init.js`

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";
import { setTypography } from "clay:theme";
import { clientSetCursorStyle, clientExecuteEditorCommand } from "clay:editor";

// Bundled first-party packages (one-line loaders).
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/settings");

// Settings panel access (used by Task H's live-reload trigger).
bindKey("Ctrl+,", "settings.open", { scope: "editor" });

// --- Ligatures: Fira Code in the monospace role (installed on this machine) ---
setTypography({
  monospace: {
    families: ["FiraCode Nerd Font Mono", "monospace"],
    size: 16,
    // Task C toggles this object; omit for defaults (ligatures on).
    ligatures: { enableStandard: true, enableContextual: true },
  },
  proportional: { families: ["sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
});

// --- Caret: uncomment ONE line at a time in Task B ---
// clientSetCursorStyle({ shape: "block", blink: "blink" });
// clientSetCursorStyle({ shape: "underline", blink: "phase", stopBlinkOnTyping: false });
// clientSetCursorStyle({ shape: "bar", blink: "solid", widthPx: 2.5 });

// --- Text objects + smart select (no default bindings by design) ---
bindKey("Alt+I", "editor.clientSelectTextobject.function.inner.current", { scope: "editor" });
bindKey("Alt+O", "editor.clientSelectTextobject.function.around.current", { scope: "editor" });
bindKey("Alt+A", "editor.clientSelectTextobject.argument.inner.current", { scope: "editor" });
bindKey("Alt+C", "editor.clientSelectTextobject.comment.around.current", { scope: "editor" });
bindKey("Alt+E", "editor.clientSmartSelect.expand", { scope: "editor" });
bindKey("Alt+R", "editor.clientSmartSelect.shrink", { scope: "editor" });

// --- Task G: user rebinding of a movement default ---
bindKey("Ctrl+B", "editor.clientMoveCursor.prevWordStart", { scope: "editor" });

// --- Task H: execution push channel (observable after a live reload) ---
clientExecuteEditorCommand({ commandId: "editor.clientSetSelection.selectLine" });
```

### 0.3 Launch

```bash
cd ~/Projects/clay
cargo run            # or: cargo run --release
```

Open `/tmp/clay-manual` as the workspace, open `test.rs`.
Watch the launching terminal: init.js evaluation errors print there as
`runtime.*` diagnostics.

## A. Movement primitives (test.rs)

| # | Action | Expected |
|---|--------|----------|
| A1 | Caret at line start of `compute_total_value`, `Ctrl+Right` repeatedly | ONE stop per identifier — the code policy joins underscores into the word (`compute_total_value` is one word); stops land at `(`, `firstItem`, etc. |
| A2 | `Ctrl+Left` from end of the identifier | Mirrors A1 backwards |
| A3 | Caret mid-file, `Ctrl+Down` / `Ctrl+Up` | Jumps across the blank line to paragraph starts/ends |
| A4 | `Home` / `End` on an indented line | First non-whitespace / line end |
| A5 | `Ctrl+Home` / `Ctrl+End` | Document start / end |
| A6 | Caret at column 20, `ArrowUp` onto a short line, `ArrowUp` again onto a long line, `ArrowDown` back | Column restored (sticky column / preferred_x) |
| A7 | `Ctrl+L` | Whole current line selected |
| A8 | `Ctrl+B` (custom binding from init.js) | Caret jumps one word back — proves `bindKey` override of a direction-specific command ID works |

## B. Prose vs code movement (package manifest rules, task 11)

Open `test.md` (mode activates through `@clay/markdown`, which declares
`wordSeparators: "prose"`, no camelCase subwords).

| # | Action | Expected |
|---|--------|----------|
| B1 | `Ctrl+Right` across `snake_case_words` in the .md file | Stops at EVERY underscore segment (prose: `_` is a separator) — `snake`, `case`, `words` |
| B2 | `Ctrl+Right` across `camelCaseWords` in the .md file | ONE stop — whole identifier is one word |
| B3 | `Ctrl+Right` across the same `snake_case_words` text in `test.rs` | ONE stop — the exact same text moves differently per mode; this underscore contrast is the prose-vs-code proof |
| B4 | `Ctrl+Down` in the .md file | Blank-line paragraph jumping works in prose too |

## C. Caret styling + blink (task 6)

Cycle the three commented `clientSetCursorStyle` lines in init.js (one at a
time), then either restart (`cargo run`) or trigger a live runtime reload via
the settings appearance switch (Task H mechanism) — the override reaches the
connected client without restart.

| # | Config line | Expected |
|---|-------------|----------|
| C1 | `{ shape: "block", blink: "blink" }` | Caret is a filled block covering the next character; blinks when idle |
| C2 | Type a few characters, then wait | Blink stops while typing, resumes after idle (`stopBlinkOnTyping` default true) |
| C3 | `{ shape: "underline", blink: "phase", stopBlinkOnTyping: false }` | Underline bar under the cell; keeps blinking through typing |
| C4 | `{ shape: "bar", blink: "solid", widthPx: 2.5 }` | Thick non-blinking bar |
| C5 | Delete all lines (defaults) | Theme-default thin bar, standard blink |
| C6 | IME/preedit (optional, if an IME is configured) | Preedit caret matches the active shape |

Negative: `clientSetCursorStyle({ shape: "triangle" })` → init.js diagnostic
`editor.invalid_set_cursor_style` in the terminal, editor unaffected
(deny-by-default enum).

## D. Font ligatures (task 7)

All in `test.rs`, monospace role = Fira Code.

| # | Config | Expected on `=>  !=  ==  ->  ::  \|\|  >=` |
|---|--------|----------|
| D1 | `ligatures: { enableStandard: true, enableContextual: true }` (or omitted) | Fira Code ligatures render (joined arrows/equals) — Fira Code's ligatures are `calt` |
| D2 | `ligatures: { enableStandard: true, enableContextual: false }` | Ligatures OFF — individual glyphs |
| D3 | `ligatures: { enableStandard: true, enableContextual: true, disableFeatures: ["calt"] }` | OFF again — escape hatch overrides the semantic toggle (last-wins merge) |
| D4 | `ligatures: { rawFeatures: '"calt" 0' }` | OFF via raw CSS-format features |
| D5 | Invalid: `rawFeatures: "x".repeat(300)` style oversize | init.js diagnostic, previous typography kept (all-or-nothing replacement) |

Note: per-role ownership — the ligature policy applies to every document
using the monospace font role; markdown and code share it. There is no
per-mode ligature override (documented design decision).

## E. Multi-cursor editing (task 9)

Back in `test.rs`, default bindings.

| # | Action | Expected |
|---|--------|----------|
| E1 | Caret inside `camelCaseCounter` (collapsed), `Ctrl+D` | First press selects the word at caret |
| E2 | `Ctrl+D` ×3 more | One selection per next occurrence, wrap-around at EOF, primary advances |
| E3 | Type `_x` | Inserted at ALL four carets simultaneously |
| E4 | `Backspace` ×2 | Deleted at all carets |
| E5 | `Ctrl+U` | Caret/selection set steps back through cursor-move history |
| E6 | `Escape` | Collapses to single caret at primary |
| E7 | `Ctrl+Shift+L` with a word selected | Selects ALL occurrences at once |
| E8 | Caret mid-line, `Ctrl+Alt+Down` ×2, `Ctrl+Alt+Up` | Carets stack on lines below/above at same column; refused where a caret already sits |
| E9 | With stacked carets, `Shift+Alt+Right` ×3 then type | Rectangular column selection grows; typing replaces the block on every line |
| E10 | `Shift+Alt+Left` | Column shrinks from the other side |
| E11 | Multi-caret `Ctrl+Z` | Text edits undo (one history step per caret edit — documented compromise) |
| E12 | `Escape` while completion menu is open | Menu closes FIRST; second `Escape` collapses the selection set (priority chain) |

## F. Text objects + smart select (task 10, Rust grammar)

In `test.rs`, caret inside `compute_total_value` body.

| # | Key | Expected |
|---|-----|----------|
| F1 | `Alt+I` | Selects function body INSIDE `fn compute_total_value` (inner) |
| F2 | `Alt+O` | Selection grows to include the `fn` signature line (around) |
| F3 | Caret on `firstItem` in the signature, `Alt+A` | Selects that parameter only |
| F4 | Caret in the top comment, `Alt+C` | Selects the whole comment block |
| F5 | Caret anywhere in a statement, `Alt+E` repeatedly | Smart select grows: expression → statement → block → function → file |
| F6 | `Alt+R` after F5 | Shrinks back down the same chain |
| F7 | Repeat F1 in `test.md` | No crash, no selection change (no grammar textobjects → advisory degrade, carets untouched) |

## G. Keybinding override + management

| # | Action | Expected |
|---|--------|----------|
| G1 | `Ctrl+B` in any file | Moves one word back (bound in init.js — already proven in A8) |
| G2 | Add `bindKey("Ctrl+B", "editor.clientMoveCursor.nextWordStart", { scope: "editor" })` below the first, reload | Last binding wins — `Ctrl+B` now moves forward |
| G3 | Negative: `bindKey("Ctrl+G", "application.quit", …)` | Rejected — non-editor/undeclared IDs are deny-by-default; diagnostic in terminal |

## H. `editor-control` execution push channel (task 20, protocol v8)

The `clientExecuteEditorCommand` call in init.js publishes an
`EditorCommandRequest` through the server→client push channel. At cold start
no connection is subscribed yet, so make it observable live:

1. Open `test.rs`, put the caret on any line, note that line is NOT selected.
2. Press `Ctrl+,` (bound to `settings.open` in init.js) and switch the
   appearance in the settings panel. The appearance choice persists and
   triggers a runtime generation reload — init.js reruns WHILE the client
   stays connected and subscribed to the command channel.
3. Expected: the line under the caret becomes selected (`clientSetSelection.selectLine`
   executed through op → gate → broadcast → connection loop → widget dispatch).
   The theme change itself confirms the reload happened.

Fallback if the settings panel is not reachable in your build: restart the
app — the channel delivery itself is covered end-to-end by automated tests
(`editor_control_execute_publishes_gated_known_commands_only`,
`editor_command_request_applies_known_ids_and_drops_unknown`).

Gate checks observable manually: init.js is trusted user configuration (no
package context) and passes the gate. Third-party denial (missing
`editor-control` permission / undeclared mode) requires an installed
third-party package and is covered by automated tests
(`third_party_editor_control_gate_requires_declared_mode`,
`editor_control_execute_publishes_gated_known_commands_only`) — not reachable
from `init.js` by design.

Negative: change the call to `commandId: "application.quit"` → op rejects
with "not a known editor command" diagnostic; nothing is published.

## I. Cleanup

Remove the scratch workspace and restore your normal `~/.config/clay/init.js`:

```bash
rm -rf /tmp/clay-manual
```

## Known ceilings (do not file as bugs)

- Multi-stroke chords (`]f`) unsupported by `bindKey` — single strokes only.
- Multi-caret undo replays one history entry per caret edit.
- camelCase sub-word movement (`MoveSubWord`) exists in the editor core but has no command-ID/keybinding surface yet — word movement treats camelCase identifiers as single words in all modes.
- `line_movement: screenLine` falls back to character movement (no soft wrap yet).
- Documents with no registry-activated mode deny package `editor-control` callers.
- `Phase`/`Smooth` blink use discrete on/off timing (no alpha ramp yet).
