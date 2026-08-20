# Proposal: First-Class Editor Capabilities — Movement, Selection, Caret Styling, Font Ligatures

Status: **Draft proposal** (not yet an approved plan or decision log).
Owner: editor subsystem (`src/editor/`, `src/masonry_editor.rs`, `src/protocol/`).
Related roadmap stubs: `roadmap.md` lines 1368–1370 ("Font ligatures, Caret Styles …",
"Movement (Next word, previous word, … selection, select word, select line, …)").

---

## 1. Goals

Clay must provide **first-class, keyboard-driven movement, selection, and editing**
for both **code editing** and **document/prose editing**, implemented as **Rust-side
primitives** that **any first- or third-party package can use and configure per Mode**.

The three concrete capabilities requested:

1. **Movement** — next/previous word, next/previous line, next/previous paragraph,
   end-of-paragraph, end-of-file, beginning-of-file, plus selection: select word,
   select line, select next/previous line. Extended (from research) to a complete
   movement + selection vocabulary.
2. **Font ligatures** — configurable OpenType ligature support.
3. **Caret styling & blinking** — Line/Bar/Underscore (Block) shapes with
   configurable width/size and blink behavior.

Non-goals (explicitly deferred, see §10): **modal editing emulation as a
built-in** (Vim/Emacs/Helix personalities come from third-party packages via
manifest + bindings, not a Rust state machine), **per-range ligature control**
(disabling ligatures inside specific spans), and **AI/agent-driven selection**.

> **What "AI/agent-driven selection" means.** Selection driven by an embedded
> AI agent — a server-side `deno_core` extension that picks or extends a
> document range as part of an automated edit (e.g. "select the function body
> I'm about to refactor"), carrying document version + behavior version +
> range + permission scope, server-authoritative. It is distinct from a human
> pressing movement keys. The human-driven, client-executed movement/selection
> primitives in this proposal are the substrate an AI-selection capability would
> be built upon (per the `extensions-and-ai` AI-mutation pattern: AI locks only
> the required scope, the server emits transactions, the client grants no
> direct local mutation authority). AI selection is a separate, permission-gated,
> server-authoritative surface and is out of scope for this plan.

**In scope (per revision):** multi-cursor + column/box selection, and tree-sitter
text objects (inner/around word/paragraph/function/class/argument/comment/test
+ sibling/parent expansion). These are no longer deferred. These are
*enabling* primitives, not a Vim/Emacs personality.

---

## 2. Current Clay state (grounded in the codebase)

### 2.1 Architecture recap (the constraints the proposal must fit)

- **Client/server**, **inert `BehaviorManifest`** model: `init.js` produces
  declarative metadata validated server-side, shipped to the client; the client
  executes the manifest on the hot path **without running JavaScript**
  (`src/client/behavior.rs`, `src/protocol/mod.rs::BehaviorManifest`).
- **Major modes** carry `EditorBehaviorRules` (`src/protocol/mod.rs:370`,
  fields: `text_edits`, `enter`, `tab`, `pairs`, `comments`, `electric_characters`,
  `autocomplete_triggers`), built declaratively by `clay:behavior::buildCodeEditingManifest`
  and registered via `modes.serverRegisterModePattern`.
- **Key bindings** (`clay:keybindings::bindKey`) bind a chord to a **stable command
  ID** in an explicit **allowlist** (`src/server/ops/keybindings.rs::is_runtime_bindable_command`).
  Routed commands become `RoutedBehavior::ClientUiCommand(ClientUiCommandRoute{command_id, args})`
  → `EditorAction::ClientUiCommand` → `EditorWidget` matches `command_id` and calls a method
  (`src/masonry_editor.rs:1488` and the `local_key` path at `:1845`).
- **Themes** carry `base.caret` color and `FontProfile{families, size}` per
  `FontRole` (`Monospace`/`Proportional`/`Ui`) inside `ActiveTypography`
  (`src/protocol/mod.rs:787`, `:845`); resolved into parley `FontStack`s in
  `src/editor/typography.rs`; pushed into parley in `src/editor/layout.rs::rebuild`.

### 2.2 What exists today

- **`EditorCommand`** enum (`src/editor/surface/mod.rs:53`): `Insert`, `Newline`,
  `Backspace`, `DeleteForward`, `MoveLeft`, `MoveRight`, `SelectLeft`, `SelectRight`,
  `MoveUp`, `MoveDown`, `LineStart`, `LineEnd`, `DocumentStart`, `DocumentEnd`.
  **No word movement, no paragraph movement, no select-word/line/paragraph,
  no select-up/down, no select-line-start/end, no select-doc-start/end.**
- **`CursorState`** (`src/editor/cursor.rs`): `move_to_previous_scalar`,
  `move_to_next_scalar`, `move_to_document_start/end`, `move_to_line_start/end`,
  `move_to_previous_line`, `move_to_next_line` (preserves scalar column via
  `preferred_x`). **No word/paragraph boundaries.**
- **`SelectionState`** (`src/editor/selection.rs`): a **single** anchor/focus
  range. **No multiple cursors, no column/box selection, no per-selection
  direction.** `word_prefix_start` exists (`surface.rs:2683`) but only for
  completion/double-click word detection (`is_completion_word_character` =
  `_` or alphanumeric).
- **Caret rendering** (`surface.rs:2368::paint_caret`): a **hardcoded
  `CARET_WIDTH = 1.5`** vertical bar, color `theme.base.caret`. **No blink,
  no shape enum, no width/thickness config.** `editor.clientSetCursorStyle`
  is documented (`docs/.../editor/client-set-cursor-style.md`) with
  `{color, blinking, type: block|bar|underline}` but is **`planned` (Phase 8),
  not wired** — no op, not in the allowlist, no Rust field.
- **`editor.clientMoveCursor`** (`direction` enum + `extendSelection` bool,
  default keys `Arrow*`, `Home`, `End`, `Ctrl+Home`, `Ctrl+End`) and
  **`editor.clientSetSelection`** are likewise documented but **`planned`/unwired**.
  Movement today is **hardcoded key dispatch** in `masonry_editor.rs::local_key`
  (`ArrowLeft/Right` → `MoveLeft/Right`/`SelectLeft/Right`, `Up/Down` → `Move*`,
  `Home/End` → `LineStart/End` or `DocumentStart/End` with Ctrl/Meta). Movement
  command IDs are **not** in `is_runtime_bindable_command`, so modes/packages
  **cannot rebind or extend movement today**.
- **Ligatures**: `layout.rs::rebuild` pushes `StyleProperty::FontStack`,
  `FontSize`, `FontWeight`, `Underline`, `Brush` into the parley builder —
  **no `FontFeatures`/`FontVariations`**. Ligatures therefore follow the shaper
  default (typically `liga`/`calt` **on** for capable fonts). There is **no
  configuration surface**.

### 2.3 Gap summary

| Capability | Research target | Clay today | Gap |
|---|---|---|---|
| Word motion (next/prev word start/end) | yes | char-only | full |
| Paragraph motion (next/prev/end) | yes | none | full |
| Sub-word / camelCase motion | yes (VSCode `cursorWordPart*`) | none | full |
| Long-WORD motion (whitespace-delimited) | yes (Vim `W/B/E`, Helix long-word) | none | full |
| Select word / line / next-prev line | requested + standard | select-word via pointer only | full |
| Extend-selection variants of every motion | standard | only `SelectLeft/Right` | most |
| Multi-cursor / column selection | standard (VSCode/Helix) | none | full (in scope, §5.5) |
| Tree-sitter text objects / smart select | standard (Helix/VSCode) | none | full (in scope, §9 E.5) |
| Caret shape (bar/block/underline/line) + width | requested | fixed 1.5px bar | full |
| Caret blink + phase + smooth animation | standard | none | full |
| Caret color override + per-mode | standard | theme color only | partial |
| Font ligatures on/off + per-feature + per-mode | requested | shaper default, no config | full |

---

## 3. Research: how VSCode, Emacs, Helix, and Vim do it

Sources are listed in §11. Vim facts are from canonical Vim help
(`motion.txt`, `options.txt` for `'guicursor'`, `'conceal'`, `'guifont'`) —
stable, decades-old reference material; live web search providers were
rate-limited/keyless during research (see §11 note).

### 3.1 Movement primitives

**Vim** (`:help motion.txt`) — the canonical motion vocabulary:
- Char: `h l`, `0` (col 0), `^` (first non-blank), `$` (line end), `g_` (last non-blank).
- Word: `w` (next word start), `b` (prev word start), `e` (next word end),
  `ge` (prev word end); **long WORD** variants `W B E gE` (whitespace-delimited).
- File: `gg` (first line), `G` (last line / `[count]`th line), `G` with count.
- Window: `H M L` (top/middle/bottom of viewport), `Ctrl-d/u` (half page),
  `Ctrl-f/b` (full page), `Ctrl-e/y` (scroll one line).
- Paragraph/sentence: `{` `}` (prev/next paragraph, blank-line delimited),
  `(` `)` (sentence).
- Match/char: `%` (matching bracket), `f/F/t/T<char>` (find/till on line,
  `;`/`,` repeat) — Vim confines these to the line; Helix does not.
- **Text objects** (`:help text-objects`): `iw/aw/iW/aW` (word/WORD inner/around),
  `ip/ap` (paragraph), `is/as` (sentence), `i)/a)/i}/a}/i]/a]` (brackets),
  `i"/a"/i'/a'/i\`/a\`` (quotes), `it/at` (tag), `ie/ae` (entire file). Text
  objects are **selections operated on by operators** (`d/c/y`), not pure motions.

**Helix** (`docs.helix-editor.com/keymap.html`, `textobjects.html`) —
**selection-first** (Kakoune model): there is no separate "move"; every motion
either moves the cursor *and* collapses, or — in **select/extend mode** (`v`) —
*extends* the selection. Motions: `move_char_left/right/up/down`, `move_next_word_start`
(`w`), `move_prev_word_start` (`b`), `move_next_word_end` (`e`), long-word
`move_next/prev_long_word_start/end` (`W/B/E`), `find_till_char`/`find_next_char`
(`t/f`), `goto_line` (`G`), `goto_line_start/end` (`Home/End`), `goto_first_nonwhitespace`,
`goto_file_start` (`gg`), `goto_last_line` (`ge`), `goto_window_top/center/bottom`,
`page_up/down`, `page_cursor_half_up/down`, `jump_forward/backward`, `save_selection`.
**Paragraph** is an **unimpaired** mapping `]p`/`[p` (`goto_next_paragraph`/
`goto_prev_paragraph`). **Tree-sitter text objects**: `select_textobject_around/inner`
with object keys (word, paragraph, function, class, argument, comment, test) and
`sibling/parent` expansion (`expand_selection` to parent node, `select_prev/next_sibling`).

**Emacs** — commands are named Lisp functions, word boundaries come from the
**per-major-mode syntax table**:
- `forward-char`/`backward-char`, `forward-word`/`backward-word`,
  `forward-paragraph`/`backward-paragraph`, `beginning-of-line`/`end-of-line`
  (and visual `move-beginning/end-of-line`), `beginning-of-buffer`/`end-of-buffer`,
  `scroll-up`/`scroll-down`, `recenter-top-bottom`.
- Selection = **the region** between **mark** (`set-mark-command`) and point;
  `exchange-point-and-mark`, `mark-word`, `mark-paragraph`, `mark-whole-buffer`.
- "End of paragraph" is achieved by `forward-paragraph` then end-of-line; Emacs
  paragraphs are defined by `paragraph-start`/`paragraph-separate` regexes per mode.

**VSCode** — command IDs (`code.visualstudio.com/docs/reference/default-keybindings`):
`cursorLeft/Right/Up/Down`, `cursorWordLeft`/`cursorWordRight`,
`cursorWordStartLeft`/`cursorWordStartRight`, **`cursorWordPartLeft`/`cursorWordPartRight`**
(camelCase/sub-word), `cursorWordEndRight`/`cursorWordEndLeft`, `cursorHome`/
`cursorEnd`, `cursorLineStart`/`cursorLineEnd`, `cursorTop`/`cursorBottom`,
`cursorPageUp`/`cursorPageDown`, `scrollLineUp/Down`, `scrollPageUp/Down`,
`expandLineSelection` (`Ctrl+L`, select line then extend to next line),
`editor.action.smartSelect.expand`/`shrink` (AST-based grow/shrink),
`editor.action.jumpToBracket`. Every movement has a `…Select`/`…Select`
variant that extends the selection (`cursorWordLeftSelect`, etc.).
`cursorUndo` (`Ctrl+U`) undoes the last cursor position change.

### 3.2 Selection models

- **Vim**: visual modes — `v` (char), `V` (line), `Ctrl-V` (block/column);
  operators (`d/c/y/=`) act on motions or text objects; one primary selection.
- **Helix**: **selection-first, multiple selections** as a primitive; primary +
  secondary selections; `select_regex`, `split_selection`, `split_selection_on_newline`,
  `merge_selections`, `collapse_selection`, `flip_selections`,
  `ensure_selections_forward`, `keep_primary_selection`, `remove_primary_selection`,
  `copy_selection_on_next/prev_line` (add cursor below/above), `rotate_selections`,
  `select_all`, `extend_line_below` (`x`), `extend_to_line_bounds` (`X`).
- **Emacs**: single region (mark + point); `secondary-selection`/`rectangular-region`
  and packages like `multiple-cursors` add multi-cursor.
- **VSCode**: **multi-cursor** as a first-class primitive — `editor.action.insertCursorBelow`/`Above`
  (add cursor), `addSelectionToNextFindMatch` (`Ctrl+D`, select word / next match),
  `selectHighlights` (`Ctrl+Shift+L`, all occurrences), `changeAll` (`Ctrl+F2`),
  `cursorUndo`; **column/box selection** via `cursorColumnSelect{Down,Up,Left,Right,PageDown,PageUp}`
  and Shift+Alt-drag; `editor.multiCursorModifier` setting (`ctrlCmd` vs `alt`).

### 3.3 Caret styling & blinking

- **Vim `'guicursor'`** (`:help guicursor`): per-mode-group list
  `n-v-c-sm:block, i-ci-ve:ver25, r-cr-o:hor20` — shapes `block`/`ver{N}`/`hor{N}`
  (`N` = percentage of cell width/height), optional `{N}` absolute, plus
  `blinkon{ms}`/`blinkoff{ms}`/`blinkwait{ms}` per group. So caret shape **and
  blink timing are mode-dependent** in Vim.
- **VSCode** (`editorOptions.ts`): `editor.cursorStyle` = `line` | `block` |
  `underline` | `line-thin` | `block-outline` | `underline-thin`; `editor.cursorWidth`
  (px, for line style); `editor.cursorBlinking` = `blink` | `smooth` | `phase` |
  `expand` | `solid`; `editor.cursorSmoothCaretAnimation` = `"off"`/`"explicit"`/
  `"on"` (animated movement between positions). Per-language overrides via `"[lang]":{...}`.
- **Emacs**: `cursor-type` = `t` (box default) | `bar`/`(bar . N)` (N px wide) |
  `hbar`/`(hbar . N)` (underscore, N px tall) | `box` | `hollow` | `nil`;
  `blink-cursor-mode` + `blink-cursor-interval`/`blink-cursor-delay`. Per-frame
  via `cursor-type` frame parameter.
- **Helix**: `cursorline`, and in `config.toml` the cursor uses the theme's
  `ui.cursor.{primary,secondary}`/`ui.cursor.insert` faces; shape is fixed by
  the terminal (Neovim-style `guicursor` is not exposed). Blinking is terminal-controlled.

### 3.4 Font ligatures

- **VSCode**: `editor.fontLigatures` — boolean **or** an object/feature string
  passed to the browser text renderer (e.g. `"calt, liga, ss01"` or
  `"'calt' 1, 'liga' 0"`); per-language `"[lang]": {"editor.fontLigatures": ...}`.
  Programming-font guides (Fira Code) document enabling `calt` and stylistic sets.
- **Emacs**: `prettify-symbols-mode` (substitution), `ligature.el` (explicit
  ligature pairs via font composition), `mac-auto-operator-composition-mode`,
  HarfBuzz composition on Emacs 28+; ligature support is font + build dependent.
- **Helix**: ligatures are on by default via the underlying renderer; toggled
  through the terminal/font, not a first-class Helix setting (some users disable
  via fontconfig to avoid ambiguous glyphs).
- **Vim/Neovim**: ligatures depend on the GUI font (`guifont`) + fontconfig;
  there is no built-in `'ligatures'` option; users disable via fontconfig rules
  or by choosing a non-ligature font. `'conceal'` controls hiding chars, unrelated
  to shaping ligatures.

### 3.5 Synthesis: what Clay should take from each

- **From Vim**: the **complete motion vocabulary** (word/WORD/paragraph/sentence/
  match/char-find/text-objects) and **mode-dependent caret shape + blink**
  (`'guicursor'` per mode group). Text objects are an *operator+motion* concept —
  defer the operator layer; keep the *boundary* math (word/WORD/paragraph) as
  reusable primitives.
- **From Helix**: **selection-first ergonomics** as an *optional* mode policy
  (extend-on-motion), **multiple selections** as a future-capable data model,
  and **tree-sitter text objects** as a later, syntax-aware layer.
- **From Emacs**: **word boundaries defined per mode** (syntax table → Clay
  `EditorBehaviorRules.word_separators`), and the **region/point + mark** model
  mapping cleanly to Clay's anchor/focus.
- **From VSCode**: the **command-ID + select-variant naming** convention
  (`cursorWordLeft`/`cursorWordLeftSelect`), **sub-word (camelCase) motion**,
  **smart-select expand/shrink**, **multi-cursor ergonomics** (add cursor
  above/below, select-next-match), and the **caret style enum + blink style +
  cursorWidth + smooth animation** config vocabulary.

---

## 4. Design principles for Clay

1. **Rust-side primitives, declarative config.** All movement, selection, caret,
   and shaping logic lives in Rust (`src/editor/`, `src/masonry_editor.rs`).
   Packages/modes configure behavior through **inert manifest data**
   (`EditorBehaviorRules` extensions, `FontProfile` extensions, theme tokens),
   never by injecting hot-path JavaScript. This matches the existing
   `buildCodeEditingManifest` + `serverRegisterModePattern` flow.
2. **One movement engine, many command IDs.** Implement movement *semantics*
   once in `CursorState`/`EditorSurface` (word/WORD/paragraph/sub-word boundaries,
   sticky column, affinity). Expose them as **stable, allowlisted command IDs**
   (`editor.clientMoveCursor`, `editor.clientSetSelection`, …) with
   **typed args** (`direction`, `granularity`, `extend`, `count`) so `bindKey`
   and modes can rebind/extend without touching Rust — exactly how clipboard/
   undo/resync commands already work.
3. **Selection is a first-class, multi-selection data structure.** Model
   selections as `Vec<Selection>` with a primary index so multi-cursor,
   column/box selection, and select-next-match are first-class from the start
   (per §5.5, now in scope). Movement/selection commands operate over the
   primary selection and extend/merge/split the set.
4. **Mode-configurable, not mode-imposed.** Defaults come from the active major
   mode's `EditorBehaviorRules` (word separators, paragraph style, caret shape,
   ligature policy). A markdown/prose mode can choose proportional fonts +
   ligatures + bar caret; a code mode can choose monospace + ligatures-off +
   block caret — without new Rust.
5. **Caret and ligatures are rendering/typography config, not commands.** They
   flow through theme/typography tokens (`base.caret`, `FontProfile`) plus
   optional per-mode overrides, and are painted by the existing
   `paint_caret`/`layout.rs::rebuild` paths.
6. **No unrequested abstractions (ponytail).** No operator-pending modal layer,
   no plugin trait for movement providers, no speculative text-object grammar
   until tree-sitter text objects are actually built. Cut every "for later" hook.

---

## 5. Pillar 1 — Movement & Selection

### 5.1 Movement vocabulary to implement (Rust + command IDs)

Add to `CursorState` (`src/editor/cursor.rs`) and `EditorBuffer` (`src/editor/buffer.rs`):

- **Word boundaries**: `move_to_next_word_start`, `move_to_prev_word_start`,
  `move_to_next_word_end`, `move_to_prev_word_end` — using a configurable
  **word-character classifier** (see 5.3).
- **Long-WORD boundaries** (whitespace-delimited): `*_long_word_*` variants.
- **Sub-word / camelCase** boundaries: `*_sub_word_*` (split on
  `lower→Upper`, `_`, digit→alpha) — matches VSCode `cursorWordPart*`.
- **Paragraph**: `move_to_next_paragraph`, `move_to_prev_paragraph`,
  `move_to_paragraph_end` — blank-line-delimited (configurable
  `paragraph_separators`), mirroring Helix `]p`/`[p` and Vim `{`/`}`.
- **First non-blank**: `move_to_first_non_whitespace` (Vim `^`, Helix `goto_first_nonwhitespace`).
- **Last non-blank**: `move_to_last_non_whitespace` (Vim `g_`).
- **Matching bracket**: `move_to_matching_pair` (Vim `%`, VSCode `jumpToBracket`)
  — reuse existing `PairRule`/bracket data.
- Already present: scalar, line start/end, doc start/end, prev/next line, sticky column.

Add to `EditorSurface` (`src/editor/surface/mod.rs`) and `EditorCommand`:

```
EditorCommand::MoveWordStart{ forward: bool, long: bool, extend: bool }
EditorCommand::MoveWordEnd{ forward: bool, long: bool, extend: bool }
EditorCommand::MoveSubWord{ forward: bool, extend: bool }
EditorCommand::MoveParagraph{ forward: bool, to_end: bool, extend: bool }
EditorCommand::MoveFirstNonWhitespace{ extend: bool }
EditorCommand::MoveLastNonWhitespace{ extend: bool }
EditorCommand::MoveMatchingPair{ extend: bool }
EditorCommand::SelectWord                // select word at/around caret
EditorCommand::SelectLine{ extend: bool } // select line; extend to next/prev line
EditorCommand::SelectAll                 // optional, VSCode selectAll/Helix %
```

Selection-extend variants reuse the existing `extend_selection` helper
(`surface.rs:3066`) — every motion becomes "move + optional extend", matching
the VSCode `…Select` convention and Helix's extend mode, without a separate
visual-mode state machine.

### 5.2 Selection vocabulary

- **Select word**: select the word (per mode classifier) around the caret;
  repeated → add next occurrence as a multi-cursor selection (VSCode `Ctrl+D`,
  implemented in E.4). E.1 ships single-selection select-word; E.4 adds the
  repeat→multi-cursor growth.
- **Select line** / **select next line** / **select previous line**: line-wise
  selection that extends by whole lines (VSCode `expandLineSelection` `Ctrl+L`,
  Helix `extend_line_below` `x`).
- **Select paragraph** (from research): select inside/around paragraph
  (`select_textobject_inner/around`-style) — E.5, after paragraph motion (E.1).
- **Smart select expand/shrink** (VSCode `smartSelect.expand/shrink`): in scope
  (E.5), tree-sitter-driven parent-chain expand / child shrink.

### 5.3 Mode-configurable word/paragraph semantics (`EditorBehaviorRules`)

Extend `EditorBehaviorRules` (`src/protocol/mod.rs:370`) with a new
`movement: MovementRules` field (default = code-oriented):

```
MovementRules {
    word_separators: WordSeparatorPolicy,   // Code | Prose | Custom(chars)
    treat_underscore_as_word: bool,         // code: true (matches is_completion_word_character)
    camel_case_sub_word: bool,              // code: true; prose: false
    paragraph_style: ParagraphStyle,        // BlankLineDelimited | MarkdownAware
    stop_at_eol_word_end: bool,             // VSCode vs Vim word-end semantics
    line_movement: LineMovementStyle,       // Visual | Logical (Helix goto j/k textual)
    sticky_column: bool,                    // preserve preferred_x across up/down (already on)
}
```

- **Code mode** (`buildCodeEditingManifest`): `word_separators = Code`
  (punctuation separates, `_` is word, camelCase sub-word on), paragraph =
  blank-line-delimited, sticky column on.
- **Markdown/prose mode**: `word_separators = Prose` (whitespace + sentence
  punctuation), `camel_case_sub_word = false`, paragraph may be
  `MarkdownAware` (treat blank lines and heading boundaries), stop-at-EOL
  matches prose editing expectations.
- A **vim-emulation third-party package** can register a mode that sets
  `word_separators` to Vim's `iskeyword`-like set and binds motions to `w/b/e/…`.

### 5.4 Approaches (pros/cons)

**Approach A — Extend `EditorCommand` enum + hardcoded default keys + allowlist
the command IDs.** (Recommended)
- *Pros*: smallest diff; reuses `move_cursor`/`extend_selection` plumbing and the
  existing `ClientUiCommand` route; default keys stay in Rust (consistent with
  `Arrow*`/`Home`/`End` today); modes rebind via `bindKey` once IDs are allowlisted.
- *Cons*: `EditorCommand` grows; args on enum variants require the key-dispatch
  switch to compute them; the command enum is currently `&'a`-lifetime-ish and
  value-based, so multi-arg variants are fine but must be constructed in
  `masonry_editor.rs` key handling.

**Approach B — Generic movement command table (string direction + granularity +
  extend + count) routed through one command ID `editor.clientMoveCursor`
  with typed args.**
- *Pros*: one command ID, maximum rebinding flexibility, matches the *documented*
  `clientMoveCursor{direction, extendSelection}` contract; trivially extensible
  to new granularities without new enum variants; `count` (Vim-style
  `3w`) falls out naturally.
- *Cons*: loses compile-time exhaustiveness of `EditorCommand`; the hardcoded
  default-key path still needs to produce the args; slightly more validation in
  the op/allowlist layer.

**Approach C — Selection-first (Kakoune/Helix) model as the primary API.**
- *Pros*: most powerful; multi-cursor native; matches modern modal editors.
- *Cons*: large rewrite of `CursorState`/`SelectionState`/`EditorSurface` and
  all call sites; breaks the existing single-caret paint/IME/history paths; high
  risk. **The multi-selection *data model* is adopted in E.4 (in scope); the
  full Kakoune *operator-pending modal model* stays deferred** (third-party
  packages build it via manifest + bindings).

**Recommendation: A + B hybrid.** Keep `EditorCommand` variants for the
hardcoded default-key path (compile-time safety, minimal diff), **and** allowlist
`editor.clientMoveCursor` / `editor.clientSetSelection` as
arg-bearing `ClientUiCommand` IDs (Approach B) so packages/modes can bind any
motion to any chord and even add motions Clay's defaults don't ship. The
`EditorWidget` command handler translates both paths into the same
`CursorState`/`EditorSurface` methods. This is exactly the dual pattern already
used (hardcoded `Ctrl+C` vs bindable `editor.clientCopySelection`).

### 5.5 Multi-cursor (in scope)

Research shows multi-cursor is central to VSCode/Helix and is **now in scope**
(per revision). Refactor `SelectionState` to `Vec<Selection>` with a primary
index; movement/selection commands operate on the primary selection by default
and the multi-cursor commands grow the set.

Commands (all stable Clay JS API IDs, allowlisted as `ClientUiCommand`):
- `editor.clientAddCursor` — `{ direction: above | below }` add a caret on
  the next/previous visual line (VSCode `insertCursorBelow/Above`).
- `editor.clientSelectNextMatch` / `clientSelectPrevMatch` — select next/
  previous occurrence of the primary selection's text, adding a selection
  (VSCode `addSelectionToNextFindMatch` `Ctrl+D`).
- `editor.clientSelectAllMatches` — select all occurrences (VSCode
  `selectHighlights` `Ctrl+Shift+L`).
- `editor.clientCancelMultipleSelections` — collapse to primary
  (VSCode `removeSecondaryCursors` `Escape`).
- `editor.clientColumnSelect` — `{ direction }` start/extend box selection
  (VSCode `cursorColumnSelect*`); renders as N carets across the column.
- `editor.clientKeepSelection` / `clientRemoveSelection` —
  Helix `keep_primary_selection`/`remove_primary_selection` ergonomics.
- `editor.clientUndoCursorMove` — restore the previous selection *set*
  (VSCode `cursorUndo` `Ctrl+U`).

Paint path (`paint_caret`/`paint_selection`) draws every selection; the primary
caret keeps the blink phase, secondary carets render solid (VSCode/Helix
convention). The `SelectionState` → `Vec<Selection>` refactor is the single
riskiest change in this proposal and is gated behind its own phase (E.4) with a
focused regression suite.

---

## 6. Pillar 2 — Caret Styling & Blinking

### 6.1 Config vocabulary (unify VSCode + Vim + Emacs)

Extend theme/typography with a `CaretStyle` token, overridable per mode:

```
CaretStyle {
    shape: CaretShape,        // Bar | Block | Underline | Line  (Line = full-height bar; Bar = thin vertical)
    width_px: u16,            // Bar/Line thickness (VSCode cursorWidth; Vim ver{N})
    height_pct: u16,          // Underline/Block height % (Vim hor{N}; Emacs (hbar . N))
    hollow: bool,             // BlockOutline (VSCode block-outline; Emacs hollow)
    color: Option<Color>,     // override theme.base.caret
    blink: BlinkStyle,        // Solid | Blink{on_ms, off_ms, wait_ms} | Phase | Smooth
    smooth_animation_ms: u16, // VSCode cursorSmoothCaretAnimation
    stop_blink_on_typing: bool,
}
```

- `Line` vs `Bar`: `Line` = full glyph-height vertical (thicker, like a
  selection-edge); `Bar` = thin vertical at the caret x (current Clay default,
  `CARET_WIDTH=1.5`). The request names "Line, Bar or Underscore" — map `Line`
  to full-height bar, `Bar` to thin bar, `Underline` to bottom underscore.
- **Per mode group** (Vim `'guicursor'` concept): the active major mode may
  supply a `caret_style` in its manifest; a future modal-editing package can
  switch shape on mode change (insert=bar, normal=block) without new Rust —
  the shape is just data consumed by `paint_caret`.

### 6.2 Implementation points

- `src/editor/theme.rs`: add `caret_style: CaretStyle` to the base style
  registry (next to `base.caret` color), with theme-key `caretStyle`.
- `src/editor/surface/mod.rs::paint_caret`: replace the fixed `CARET_WIDTH` bar
  with shape-aware geometry: `Bar`/`Line` → vertical rect of `width_px`;
  `Block` → glyph cell rect (optionally `hollow` = stroke only, draw the
  underlying glyph in the inverse/background color); `Underline` → bottom rect
  of `height_pct`. Keep the existing clip + `theme.base.caret` color path.
- **Blink**: add a blink phase to `EditorSurface` (already has animation
  request hooks via `ctx.request_anim_frame`/`TimeDt` in masonry); paint
  caret only when `blink_phase` is "on"; reset to on + restart timer on any
  key/pointer/insert (VSCode/Emacs behavior). `Solid` = always on.
- **Smooth animation**: interpolate caret x/y between positions over
  `smooth_animation_ms` using the existing `visual_scroll_y` animation pattern.
- **`clientSetCursorStyle`**: wire the *planned* op — add it to the bindable
  allowlist as a `ClientUiCommand` (or a pure config op) that updates the
  active document's resolved `CaretStyle` at runtime (for `init.js`/packages),
  with per-mode/theme as the default source.

### 6.3 Approaches (pros/cons)

**Approach A — Theme-token only (`caretStyle` in theme).**
- *Pros*: simplest; reuses theme pipeline; one place to paint.
- *Cons*: cannot differ per major mode or per caret mode (insert/normal) without
  theme switching; doesn't satisfy "configurable based on Mode".

**Approach B — `EditorBehaviorRules.caret_style` manifest field + theme default.** (Recommended)
- *Pros*: per-mode by construction; a vim-emulation package sets
  insert=bar/normal=block via manifest data; theme provides the default; no
  hot-path JS; matches `EditorBehaviorRules` pattern.
- *Cons*: `EditorBehaviorRules` gains a rendering field (slightly mixes editing
  + presentation), mitigated by keeping it pure data.

**Approach C — Separate `CaretProfile` in `ActiveTypography`/theme, mode
  overrides via a side channel.**
- *Pros*: clean separation of rendering config from editing rules.
- *Cons*: a new override channel + merge logic; more moving parts for the same
  outcome; redundant with the theme's existing `base.caret`.

**Recommendation: B**, with the theme providing the default `CaretStyle` and
`EditorBehaviorRules.caret_style: Option<CaretStyle>` overriding per mode;
`clientSetCursorStyle` remains the runtime/`init.js` escape hatch.

---

## 7. Pillar 3 — Font Ligatures

### 7.1 What parley/swash already give us (verified in the resolved crate)

- `parley 0.6` exposes `StyleProperty::FontFeatures(FontSettings<'a, FontFeature>)`
  and `StyleProperty::FontVariations(FontSettings<'a, FontVariation>)`
  (`parley/src/style/mod.rs:110/112`).
- `FontFeature = swash::Setting<u16>` = `{ tag: [u8;4], value: u16 }` —
  value `0` = disable, `1` = enable, `2+` = alternate selection.
- `FontSettings` = `Source(Cow<str>)` (CSS-like, e.g. `"calt, liga off, ss01"`)
  **or** `List(Cow<[FontFeature]>)` (parsed records). `From<&str>` is
  implemented, so a CSS string is the simplest config surface.
- The shaper consumes `style.font_features` (`parley/src/shape/mod.rs:99/465/515`).
- Clay's `layout.rs::rebuild` currently pushes **no** `FontFeatures`, so the
  shaper default (typically `liga`/`calt` on) applies.

**Conclusion: ligature support is feasible now by pushing
`StyleProperty::FontFeatures(...)` into the parley builder.** No upstream work
required (the older "piet will be extended…" note in parley docs is stale
relative to the 0.6 API that already exists).

### 7.2 Config vocabulary

Extend `FontProfile` (`src/protocol/mod.rs:787`) and its parser
(`src/server/ops/typography.rs::parse_profile`):

```
FontProfile {
    families: Vec<FontFamily>,
    size: f32,
    ligatures: LigaturePolicy,   // NEW
}
LigaturePolicy {
    enable_standard: bool,        // liga + clig
    enable_contextual: bool,      // calt (programming-font ligatures)
    discretionary_features: Vec<String>, // e.g. ["ss01","ss02","cv01","zero","onum"]
    raw_features: Option<String>, // verbatim CSS source, e.g. "'calt' 1, 'liga' 0"
    disable_features: Vec<String>,// e.g. ["liga"] to force off
}
```

Resolution (`src/editor/typography.rs::ResolvedFontProfile`): build a
`FontSettings<'static, FontFeature>` (prefer `Source(CssString)` for simplicity,
or `List` for precise control) from the policy and hand it to
`layout.rs::rebuild`, which pushes
`builder.push_default(StyleProperty::FontFeatures(settings))` per font role.

### 7.3 Per-mode policy

- **Code mode**: `enable_standard = true`, `enable_contextual = true` by default
  (Fira Code / JetBrains Mono ligatures), but **a package/user can set
  `enable_contextual = false`** to disable `-> => !== ==` style ligatures — a
  common request for accessibility/ambiguity. Stylistic sets (`ss01`, `zero`,
  `cv01`) configurable for per-language preferences.
- **Markdown/prose mode**: `enable_standard = true`, `enable_contextual =
  false`, discretionary off — prose typography. Proportional font role.
- A **third-party package** can ship a `FontProfile` override per mode via the
  same typography/theme path; no new authority.

### 7.4 Approaches (pros/cons)

**Approach A — Single global boolean `ligatures: bool`.**
- *Pros*: trivial; one setting.
- *Cons*: cannot express "ligatures on but `calt` off" or stylistic sets; cannot
  differ per font role/mode; insufficient for the research norm (VSCode allows
  feature strings).

**Approach B — `LigaturePolicy` per `FontProfile` (per role) + per-mode override.** (Recommended)
- *Pros*: per font role (monospace vs proportional) and per mode; covers VSCode's
  feature-string power via `raw_features`; reuses `ActiveTypography`/theme path;
  declarative, no hot-path JS; smallest general solution that satisfies "code
  AND document editing".
- *Cons*: more fields to parse/validate; must cache resolved `FontSettings` per
  profile in the layout cache key (extend `LayoutCacheKey`) so feature changes
  invalidate layout.

**Approach C — Per-range `StyleProperty::FontFeatures` pushed for specific
  text spans (e.g. disable ligatures inside strings/comments).**
- *Pros*: maximum fidelity (some editors disable ligatures in comments/strings
  to avoid `//` ligatures in code).
- *Cons*: requires span-level styling wired to syntax decorations; significant
  work; ambiguous benefit. **Defer** (mark `ponytail:` ceiling — per-range
  ligature control is a later syntax-aware refinement).

**Recommendation: B**, with `LayoutCacheKey` extended by a profile-feature hash.
Defer C.

---

## 8. How first/third-party packages use this (concrete examples)

```ts
// init.js — user/global
import { bindKey } from "clay:keybindings";
import { clientMoveCursor, clientSetSelection, clientSetCursorStyle } from "clay:editor";

// Word/paragraph movement + selection, mode-agnostic defaults
bindKey("Ctrl+Right", clientMoveCursor({ direction: "nextWordEnd", extend: false }), { scope: "editor" });
bindKey("Ctrl+Left",  clientMoveCursor({ direction: "prevWordStart", extend: false }), { scope: "editor" });
bindKey("Ctrl+Shift+Right", clientMoveCursor({ direction: "nextWordEnd", extend: true }), { scope: "editor" });
bindKey("Ctrl+Shift+Left",  clientMoveCursor({ direction: "prevWordStart", extend: true }), { scope: "editor" });
bindKey("Ctrl+Down",  clientMoveCursor({ direction: "nextParagraph" }), { scope: "editor" });
bindKey("Ctrl+Up",    clientMoveCursor({ direction: "prevParagraph" }), { scope: "editor" });
bindKey("Ctrl+Shift+Home", clientMoveCursor({ direction: "documentStart", extend: true }), { scope: "editor" });
bindKey("Ctrl+Shift+End",  clientMoveCursor({ direction: "documentEnd", extend: true }), { scope: "editor" });
bindKey("Ctrl+L",     clientSetSelection({ action: "selectLine", extend: true }), { scope: "editor" });
bindKey("Ctrl+D",     clientSetSelection({ action: "selectWord" }), { scope: "editor" });
```

```ts
// A first-party code language package (e.g. packages/rust)
import { buildCodeEditingManifest, buildMovementRules, buildCaretStyle, buildLigaturePolicy } from "clay:behavior";
import { serverRegisterModePattern } from "clay:modes";

serverRegisterModePattern(manifest, {
  modeId: "rust", displayName: "Rust", extensions: ["rs"],
  editorRules: {
    ...buildCodeEditingManifest({ indentSize: 4, lineComment: "//", electricOutdentCharacters: ["}"] }),
    movement: buildMovementRules({ wordSeparators: "code", camelCaseSubWord: true, treatUnderscoreAsWord: true }),
    caretStyle: buildCaretStyle({ shape: "bar", widthPx: 2, blink: "blink", blinkOnMs: 530, blinkOffMs: 470 }),
    ligatures: buildLigaturePolicy({ enableContextual: true, discretionaryFeatures: ["ss01"] }),
  },
});
```

```ts
// A third-party vim-emulation package registers a "vim-normal" mode that:
//  - sets caretStyle shape: "block" (normal), "bar" (insert) via manifest data
//  - sets movement word_separators to Vim's iskeyword set
//  - binds w/b/e/ge/{/}/gg/G/0/^/$ to editor.clientMoveCursor with the right direction
//  - uses extend:true for all motions to get Helix/Vim-visual-style selection
// No new Rust required — purely declarative manifest + key bindings.
```

All three pillars are therefore **mode-configurable** and **package-usable**
through the existing `serverRegisterModePattern` + `bindKey` + theme/typography
channels. Rust owns the primitives; packages own the personality.

---

## 9. Phased implementation plan (gates + acceptance)

Each phase is independently shippable and follows Clay's Linux-blocking gates
(`cargo fmt --check`, `cargo check --all-targets`,
`cargo clippy --all-targets -- -D warnings`, `tests/suites/editor.rs`).

### Phase E.1 — Movement primitives (Rust) + allowlisted command IDs
- Add word/long-word/sub-word/paragraph/first-non-blank/last-non-blank/matching-pair
  to `CursorState` + `EditorBuffer` boundary helpers, with unit tests
  (extend `cursor.rs`/`buffer.rs` test modules — Unicode, combining marks, blank-line
  paragraphs, camelCase, CRLF).
- Extend `EditorCommand` + `EditorSurface::move_cursor`/`extend_selection`;
- add `SelectWord`, `SelectLine{extend}`, `select_next_line`/`select_previous_line`.
- Add `MovementRules` to `EditorBehaviorRules` (default = current code behavior).
- Allowlist `editor.clientMoveCursor`, `editor.clientSetSelection`
  as `ClientUiCommand` with typed args; wire `EditorWidget` dispatch.
- Default keys: `Ctrl+Left/Right` = prev/next word start; `Ctrl+Shift+Left/Right`
  = extend; `Ctrl+Up/Down` = prev/next paragraph; `Ctrl+Shift+Up/Down` = extend
  paragraph; `Home`/`End` unchanged; `Ctrl+Home/End` doc start/end (already);
  `Ctrl+L` = select line + extend; `Ctrl+D` = select word. `Ctrl+Left/Right`
  currently fall through to insertion — this removes that conflict.
- **Acceptance**: `tests/suites/editor.rs` covers every motion + extend + select
  variant; a markdown mode fixture sets `word_separators = prose` and assertions
  differ from code mode; `init.js` can rebind `Ctrl+Right` to a different motion.

### Phase E.2 — Caret styling + blink (rendering)
- Add `CaretStyle` to theme base + `EditorBehaviorRules.caret_style: Option<CaretStyle>`.
- Rewrite `paint_caret` for Bar/Line/Block(hollow)/Underline + `width_px`/`height_pct`
  + color override; keep IME preedit caret consistent.
- Add blink timer in `EditorSurface`/`EditorWidget` (on/off/wait, reset on input);
  optional smooth caret animation reusing the `visual_scroll_y` interpolation.
- Wire `editor.clientSetCursorStyle` op + allowlist.
- **Acceptance**: visual smoke (screenshot/test) for each shape; blink phase unit
  test; per-mode `caretStyle` override verified via a fixture mode; `init.js`
  can set `clientSetCursorStyle({ shape: "block", blink: "solid" })`.

### Phase E.3 — Font ligatures
- Add `LigaturePolicy` to `FontProfile`; parse in `parse_profile`; resolve in
  `ResolvedFontProfile`; push `StyleProperty::FontFeatures` in `layout.rs::rebuild`.
- Extend `LayoutCacheKey` with a feature-set hash so changes invalidate layout.
- Per-mode policy via `EditorBehaviorRules.ligatures` (or typography override).
- **Acceptance**: a test that builds a layout with `liga off` vs `liga on` and
  asserts different glyph/cluster counts using parley's `is_ligature_start`;
  markdown vs code mode fixtures resolve different policies; a user can disable
  `calt` from `init.js`.

### Phase E.4 — Multi-cursor & advanced selection (in scope)
- Refactor `SelectionState` → `Vec<Selection>` + primary index; update every
  caller (insert/delete, copy/cut, undo/redo, search, IME, decoration tracking).
- Implement + allowlist `clientAddCursor`, `clientSelectNextMatch`,
  `clientSelectPrevMatch`, `clientSelectAllMatches`, `clientColumnSelect`,
  `clientCancelMultipleSelections`, `clientKeepSelection`, `clientRemoveSelection`,
  `clientUndoCursorMove`.
- Default keys: `Ctrl+Alt+Down/Up` add cursor below/above, `Ctrl+D`
  select-next-match, `Ctrl+Shift+L` select-all-matches, `Shift+Alt+Down/Up/Left/Right`
  column-select, `Escape` collapse to primary, `Ctrl+U` cursor-undo.
- **Acceptance**: `tests/suites/editor.rs` multi-cursor regression suite (add
  cursor, select-next-match loops, column box, keep/remove primary, copy over
  union, undo restores set, IME with multiple carets); `init.js` can bind each;
  `paint_caret`/`paint_selection` draw all selections.

### Phase E.5 — Tree-sitter text objects & smart select (in scope)
- Add a generic server-side text-object query primitive: packages ship
  `queries/textobjects.scm` with Helix-style `@textobject.{start,end}` captures
  (with `#match?`/`#not-match?` predicates where needed) for objects
  word/paragraph/function/class/argument/comment/comment-block/test/tag.
- New `ClientUiCommand` IDs `editor.clientSelectTextobject`
  (`{ object, around: bool, direction: next | prev | current }`) and
  `editor.clientSmartSelect` (`{ action: expand | shrink }`) that query the
  document's syntax tree (reusing `src/server/syntax.rs` `tree_sitter` engine +
  `QueryCursor`) for the range(s) around the primary caret and apply them as
  selection(s). Multi-cursor-aware: `clientSelectTextobject` can grow the
  selection set across all carets.
- Built-in `@clay/{rust,typescript,javascript,markdown}` packages ship
  `textobjects.scm`; the generic primitive lets any future language package add
  objects with no Rust change (package-provided-grammar pattern).
- **Acceptance**: `textobjects.scm` for ≥1 built-in language with tests asserting
  inner/around function/class/argument/comment ranges at known offsets;
  `clientSmartSelect.expand` walks the tree parent chain, `shrink` reverses;
  no language-specific Rust branch; `init.js` can bind `Ctrl+Shift+\` /
  `Ctrl+Shift+Alt+\` to smart-select expand/shrink and `]f`/`[f` (package-bound)
  to next/prev function.

---

## 10. Risks, open questions, and explicit deferrals (ponytail)

- **Multi-cursor refactor is the riskiest change.** `SelectionState` →
  `Vec<Selection>` touches every caller (insert/delete, copy/cut, undo/redo,
  search, IME, decoration tracking). E.4 is gated behind a dedicated regression
  suite before E.5 builds multi-cursor-aware text objects on it. `ponytail:`
  ceiling — single primary caret blink; secondary carets render solid, no
  per-secondary blink phase unless a later need is measured.
- **No modal/operator layer built-in.** Vim/Emacs/Helix personalities come from
  third-party packages via manifest + bindings, not Rust state machines.
  `ponytail:` no speculative operator-pending mode.
- **No per-range ligature control (Approach C) in first cut.** `ponytail:`
  per-span ligature disabling (e.g. inside strings) needs syntax-span styling;
  add only if a real accessibility/ambiguity need is measured.
- **Word-boundary semantics must match `is_completion_word_character`** so
  movement, selection, and completion agree on what a "word" is — unify the
  classifier or mode movement will fight completion triggers.
- **Layout cache invalidation** for ligature changes is required (E.3) or stale
  glyphs render after a font-feature toggle.
- **Blink + smooth animation must not regress IME/preedit** paint
  (`paint_preedit_overlay` draws its own caret) — keep the preedit caret path
  shape-aware too.
- **Windows note** (per project platform policy): caret blink timing and
  parley shaping are cross-platform; no Windows-specific gate is weakened.
  Linux `cargo` gates remain blocking.
- **Resolved — `Ctrl+Up/Down` is paragraph motion.** `Ctrl+Up/Down` =
  prev/next paragraph (extend with Shift); `Ctrl+Shift+Up/Down` = extend
  paragraph. `PageUp/Down`/`Ctrl+PageUp/Down` keep scroll semantics
  (VSCode-aligned). Decided per reviewer agreement; no further confirmation needed.
- **Resolved — `Line` vs `Bar` naming.** `Bar` = thin vertical (default
  `width_px = 2`, close to today's 1.5); `Line` = full glyph-height vertical;
  `Block` = glyph cell (optional hollow); `Underline` = bottom rect. Matches
  the request's 'Line, Bar or Underscore' phrasing with `Block` as the fourth
  shape. Decided per reviewer agreement; no further confirmation needed.

---

## 11. Sources

Codebase (read directly):
- `src/editor/cursor.rs`, `src/editor/selection.rs`, `src/editor/buffer.rs`,
  `src/editor/viewport.rs`, `src/editor/surface/mod.rs` (`EditorCommand`,
  `move_cursor`, `extend_selection`, `paint_caret`, `word_prefix_start`,
  `CARET_WIDTH`), `src/editor/layout.rs`, `src/editor/typography.rs`,
  `src/editor/theme.rs`.
- `src/protocol/mod.rs` (`EditorBehaviorRules`, `FontProfile`,
  `ActiveTypography`, `FontRole`, `BehaviorManifest`, `KeyBindingRule`).
- `src/masonry_editor.rs` (`local_key`, `EditorAction::ClientUiCommand`).
- `src/server/ops/keybindings.rs` (`is_runtime_bindable_command`,
  `command_routing_policy`), `src/server/ops/typography.rs` (`parse_profile`).
- `docs/reference/clay-js-api/{editor,modes,behavior,keybindings}/*.md`.
- Resolved crate: `parley-0.6.0` (`src/style/mod.rs`, `src/style/font.rs`,
  `src/shape/mod.rs`) and `swash-0.2.7` (`src/setting.rs`).

Web (retrieved 2026-07-31):
- VSCode — Basic editing, Default keybindings, editorOptions.ts:
  https://code.visualstudio.com/docs/editing/codebasics ,
  https://code.visualstudio.com/docs/reference/default-keybindings ,
  https://github.com/microsoft/vscode/blob/main/src/vs/editor/common/config/editorOptions.ts ,
  https://github.com/tonsky/FiraCode/wiki/VS-Code-Instructions .
- Helix — Keymap, Usage, Text objects, Configuration:
  https://docs.helix-editor.com/keymap.html ,
  https://docs.helix-editor.com/usage.html ,
  https://docs.helix-editor.com/textobjects.html ,
  https://docs.helix-editor.com/configuration.html .
- Emacs — prettify-symbols / ligature.el / HarfBuzz composition:
  https://doc.emacsen.de/master/fun/prettify-symbols-mode.html ,
  https://github.com/mickeynp/ligature.el ,
  https://docs.doomemacs.org/latest/modules/ui/ligatures/ ,
  https://github.com/tonsky/FiraCode/wiki/Emacs-instructions .

Vim (canonical help, used from established reference; live search providers were
rate-limited (OpenAI usage cap, Exa 429) or keyless (Brave/Tavily/Perplexity/Gemini)
during research, so Vim facts are taken from the stable, canonical Vim
documentation rather than a live fetch):
- `:help motion.txt` (w/b/e/ge, W/B/E/gE, 0/^/$, gg/G, {/}, %, f/F/t/T, H/M/L).
- `:help text-objects` (iw/aw/iW/aW, ip/ap, is/as, i)/a)/i}/a}/i]/a], i"/a", it/at, ie/ae).
- `:help guicursor` (block/ver{N}/hor{N}, blinkon/blinkoff/blinkwait per mode group).
- `:help 'conceal'`, `:help 'guifont'` (ligatures are font/fontconfig-controlled,
  no built-in `'ligatures'` option).

---

## 12. Next step

On approval, promote this to a numbered plan under `plans/` (per the
`create-plan` skill) and record the design decision in `decision-logs/`
(per the `create-decision-log` skill), then implement Phase E.1 behind the
existing Linux-blocking CI gates.