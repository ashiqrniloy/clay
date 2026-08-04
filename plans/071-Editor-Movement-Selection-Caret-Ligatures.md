# 071 — Editor Movement, Selection, Caret Styling, and Font Ligatures

Design proposal: `docs/design/editor-capabilities-movement-caret-ligatures-proposal.md` (read it first; this plan implements it, including the reviewer-directed scope additions: multi-cursor + tree-sitter text objects are **in scope**, not deferred).

Roadmap stubs: `roadmap.md` lines 1368–1370 ("Font ligatures, Caret Styles …",
"Movement (Next word, previous word, … selection, select word, select line, …)").

## Objectives

- Implement Rust-side, keyboard-driven **movement** (word/long-word/sub-word/paragraph/non-blank/matching-pair) and **selection** (select word/line/next-prev line, extend-on-motion) primitives, configurable per major mode, reusable by first- and third-party packages via inert manifest data + key bindings.
- Implement **multi-cursor + column/box selection** (selection set as `Vec<Selection>` with primary index; add-cursor, select-next/prev/all-match, column select, keep/remove, cursor-undo).
- Implement **tree-sitter text objects + smart-select expand/shrink** as generic package-provided grammar contributions (no language-specific Rust).
- Implement **caret styling + blinking** (Bar/Line/Block/Underline, width/height, hollow, color override, blink phase, optional smooth animation), configurable per mode and at runtime via `clientSetCursorStyle`.
- Implement **font ligatures** (configurable OpenType feature policy per `FontRole`, per mode), wired through parley 0.6 `StyleProperty::FontFeatures`.
- Wire the documented-but-unimplemented `clay.editor.clientMoveCursor`, `clientSetSelection`, `clientSetCursorStyle` ops + the new multi-cursor/text-object ops as allowlisted `ClientUiCommand` IDs.
- Document every public surface as a Clay JS API; record primitives in reference docs + code wiki; preserve the two package runtime trust domains and the client hot-path invariant.

## Expected Outcome

- A user can move by word/paragraph/sub-word, select word/line/paragraph, add multiple carets, select-all-matches, column-select, expand/shrink selection by syntax tree, all from default keys and all rebindable from `init.js`.
- Caret shape (Bar/Line/Block/Underline), width, blink, and color are configurable per mode and via `init.js`; ligatures are on/off/configurable per font role and per mode.
- `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass on Linux; no new language-specific Rust branches; no new file/network/shell/AI/WASM authority.

### Planning Notes (per `planning-checklist.md`)

- **Decision alignment:** `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md` (primitive-first); `2026-07-01-0350-phase18-9-generic-text-code-fallback-modes-and-key-behavior.md` (core.* modes, electric kinds); `2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md` (typography role ownership); `2026-07-21-0001-two-package-runtime-trust-domains.md` (trust domains); `2026-05-08-1509-clay-js-api-facade-for-rust-functions.md` + `2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md` (JS API); `2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md` (config); `2026-06-29-2006-package-provided-grammar-and-capability-phases.md` (package-provided grammar).
- **Authority boundary:** Client owns caret/selection/viewport/blink state and inert manifest execution; server owns canonical documents, versions, tree-sitter query execution, and grammar contributions. Movement/selection/caret are client-local `ClientUiCommand`; text-object/smart-select query is server-side read-only, results applied client-side as selections. Ligature policy is typography config (user-owned family/size; packages declare semantic policy only).
- **Client hot path:** Movement/selection/caret/blink run in the client with no IPC per keystroke; no JS, no server round trip, no full-document serialize. Text-object/smart-select issue one bounded server query (reusing the existing parse tree) and apply results client-side; not on the typing hot path.
- **Server authority:** No new document mutation authority; selections are client view state. Text-object op is read-only query over the existing syntax tree; no mutation, no external process, no file/network/shell.
- **Behavior manifest:** `MovementRules`, `caret_style`, and `ligatures` are new **inert data fields** on `EditorBehaviorRules`/`FontProfile` consumed on the client hot path; no new executable manifest *kind* (extension of existing data, per the Phase 18.9 electric-character precedent).
- **Security:** No file IO, network, script execution, WASM, AI mutation, remote listener, or shell introduced. New `ClientUiCommand` IDs grant no authority beyond client-local caret/selection/viewport/blink.
- **Performance:** Reuse `LayoutCacheKey` invalidation for ligature changes; blink/animation use existing `request_anim_frame`; text-object queries bounded + cancellable; selections painted viewport-bounded.
- **Phase boundary:** Multi-cursor + text objects are in scope per reviewer direction; modal/operator-pending emulation and per-range ligature control remain explicitly out of scope (third-party packages / later phase).

## Tasks

- [x] 0. Plan baseline, decision alignment, and authority boundaries ✓ (Baseline Note below)
  - Acceptance Criteria:
    - Functional: A short note in the plan records relevant decision logs, the active `core.code`/`core.text` fallback mode behavior, and the authority split (client vs server) for each pillar.
    - Performance: N/A (documentation-only task).
    - Code Quality: Authority boundary for each new op/manifest field is stated before implementation begins.
    - Security: A list of authorities *not* introduced (file/network/shell/AI/WASM/process) is recorded.
  - Approach:
    - Documentation Reviewed:
      - `docs/design/editor-capabilities-movement-caret-ligatures-proposal.md` (§2, §4, §9, §10).
      - `.agents/skills/project-patterns/references/planning-checklist.md`, `behavior-manifests.md`, `authority-boundaries.md`, `protocol-and-performance.md`.
    - Options Considered:
      - Inline authority notes per task vs. a dedicated baseline task. Dedicated task avoids repetition and gives one place to audit boundaries.
    - Chosen Approach:
      - One baseline task; subsequent tasks reference it instead of restating.
    - API Notes and Examples:
      ```text
      client-local: move_cursor, set_selection, set_cursor_style, add_cursor, select_*_match, column_select, caret blink
      server read-only query: select_textobject, smart_select (tree-sitter)
      typography config (user-owned): FontProfile.ligatures (per FontRole)
      manifest data (inert): EditorBehaviorRules.movement, EditorBehaviorRules.caret_style
      ```
    - Files to Create/Edit:
      - `plans/071-Editor-Movement-Selection-Caret-Ligatures.md`: Baseline Note filled (see end of this task).
    - References:
      - Decision logs listed in Planning Notes above.
  - Test Cases to Write:
    - Manual review: confirm each later task's AC references the authority split and the no-new-authority list.

  ### Baseline Note (Task 0 — completed 2026-07-31)

  **Decision-log alignment — all cited logs verified present in `decision-logs/`:**
  - `2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md` — server-authoritative docs + client behavior manifests (the client/server authority split this plan depends on).
  - `2026-05-08-1509-clay-js-api-facade-for-rust-functions.md` — JS API facade for Rust ops; `client*`/`server*` authority markers.
  - `2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md` — keybindings + `custom_properties` discovery.
  - `2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md` — config-as-API via `init.js`.
  - `2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md` — primitive-first mode planning.
  - `2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md` — one-line package defaults (task 11).
  - `2026-06-29-2006-package-provided-grammar-and-capability-phases.md` — package-provided grammar (text objects, tasks 3 & 10).
  - `2026-07-01-0350-phase18-9-generic-text-code-fallback-modes-and-key-behavior.md` — `core.code`/`core.text` fallback + key behavior.
  - `2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md` — semantic font roles / user-owned typography (ligatures, task 7).
  - `2026-07-21-0001-two-package-runtime-trust-domains.md` — two package runtime trust domains (task 15).
  - API-doc tasks 13/14 also cite `2026-05-08-1419-markdown-authoritative-documentation-registry.md` and `2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`; both verified present.

  **Active built-in fallback modes (verified in `src/packages/modes.rs`):**
  - `core.text` (`CORE_TEXT_MODE_ID`, `core_text_mode()`): universal plain-text fallback, `DocumentFontRole::Proportional`, no patterns, always-on, no owning package. Selected only when no package-declared or built-in pattern matches. Behavior manifest = `BehaviorManifest::minimal_text_editing` → `EditorBehaviorRules::default_text()` (`src/protocol/mod.rs:391`).
  - `core.code` (`CORE_CODE_MODE_ID`, `core_code_mode()`): code-oriented fallback, `DocumentFontRole::Monospace`, declarative extension list + any-shebang (`*`), lowest precedence so language packages win on ties. Behavior manifest = `BehaviorManifest::core_code_editing` → `EditorBehaviorRules::default_code()` (`src/protocol/mod.rs:429`, built on `default_text()`).
  - Package modes override `editor_rules` via `parse_editor_rules` (`src/server/ops/modes.rs:248–322`) from the manifest's `editor_rules_override`.
  - **Implication for this plan:** new `EditorBehaviorRules` fields (`movement`, `caret_style`) must be added with defaults in `default_text()`/`default_code()` so the built-in fallbacks get sensible behavior with **zero behavior change for existing modes**; `parse_editor_rules` must parse the new fields from package manifests. `FontProfile.ligatures` is typography, parsed in `src/server/ops/typography.rs::parse_profile`.

  **Authority split per pillar (client vs server) — stated before implementation:**

  | Pillar | Client (hot path; no IPC/JS per keystroke) | Server (authority) |
  |---|---|---|
  | Movement (E.1) | `CursorState`/`EditorBuffer` word/paragraph/non-blank/pair motions; `EditorCommand` dispatch; consumes `MovementRules` from inert manifest | owns canonical doc + version; no per-keystroke round trip |
  | Selection (E.1) | `SelectionState` set; `clientMoveCursor`/`clientSetSelection` are `ClientUiCommand` (client-local) | none (view state) |
  | Multi-cursor (E.4) | `Vec<Selection>` + primary; all `client*` multi-cursor ops client-local | none (view state) |
  | Text objects / smart-select (E.5) | applies returned ranges as selections (multi-cursor-aware) | read-only tree-sitter query over the existing parse tree via `src/server/syntax.rs`; returns inert ranges; no mutation |
  | Caret styling/blink (E.2) | `paint_caret` shape-aware; blink timer; `clientSetCursorStyle` client-local; `CaretStyle` in `StyleRegistry`/`EditorBehaviorRules.caret_style` (inert) | none (rendering chrome) |
  | Ligatures (E.3) | parley `StyleProperty::FontFeatures` in `layout.rs::rebuild`; `LayoutCacheKey` feature hash | `parse_profile` parses `FontProfile.ligatures`; user-owned family/size unchanged |

  **Authority boundary per new surface (deny-by-default — stated before implementation):**
  - New `ClientUiCommand` IDs (`clientMoveCursor`, `clientSetSelection`, `clientSetCursorStyle`, `clientAddCursor`, `clientSelectNextMatch`, `clientSelectPrevMatch`, `clientSelectAllMatches`, `clientColumnSelect`, `clientCancelMultipleSelections`, `clientKeepSelection`, `clientRemoveSelection`, `clientUndoCursorMove`) → client-local caret/selection/viewport/blink; grant **no** document mutation or external authority.
  - New server read-only ops (`clientSelectTextobject`, `clientSmartSelect`) → execute the active grammar's `textobjects.scm` over the existing parse tree; return inert `rkyv` ranges; **no** mutation, **no** cross-domain V8 object/function/module passing, **no** native artifact loading (first-party resolver-validated grammar only).
  - New inert manifest fields (`EditorBehaviorRules.movement`, `EditorBehaviorRules.caret_style`, `FontProfile.ligatures`) → inert `rkyv` data consumed on the client hot path; **no** new executable manifest kind (extends existing data per the Phase 18.9 electric-character precedent).

  **Authorities NOT introduced (exhaustive):** no file IO, no network, no shell/process spawn, no script execution, no WASM, no AI mutation, no remote listener, no document mutation authority, no new native artifact loading. New surfaces add client-local view state + one read-only server query only.

  **Cross-task reference check (task 0 test):** the per-pillar authority split and the no-new-authority list above are reflected in each implementation task's **Security** acceptance criterion (tasks 4–10, 15) and the trust-domain preservation task (15). Subsequent tasks cite this baseline rather than restating the split; any deviation must be recorded in `Compromises Made`.

- [x] 1. Review existing editor primitives and plan generic primitive gaps before implementation ✓ (Primitive Inventory below)
  - Acceptance Criteria:
    - Functional: An inventory of existing Rust-side editor primitives (movement, selection, keybinding allowlist/routing, parley layout, typography registry, caret paint, tree-sitter query) documents what the new work can reuse vs. what generic primitives it must add. New primitives are generic/reusable across modes and languages (no Markdown/Rust/TypeScript-specific Rust).
    - Performance: Identified hot-path primitives (paint, layout, key dispatch) are confirmed to stay IPC/JS-free per keystroke.
    - Code Quality: The inventory cites exact files/symbols; gaps are named (word-boundary classifier, `Vec<Selection>` model, caret-shape primitive, font-feature primitive, textobject query primitive).
    - Security: No new authority is identified as required for movement/selection/caret/ligature; textobject query is read-only.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/reference/primitives/typography.md`, `docs/reference/primitives/rendering-strategy.md`, `docs/reference/primitives/syntax-vocabulary.md`.
      - `docs/wiki/modules/primitive-architecture.md`, `rendering-primitives.md`, `masonry-editor.md`, `mode-registry.md`, `editor-theme-registry.md`, `parse-coordinator.md`, `decoration-transport.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `behavior-manifests.md`, `typography-role-ownership.md`.
    - Options Considered:
      - Add movement logic per-mode in JS vs. generic Rust primitives + declarative `MovementRules`. Generic Rust keeps hot path JS-free and reusable (per `mode-primitive-first.md`).
      - Reuse `is_completion_word_character` vs. a new configurable classifier. Reuse is required for word/completion agreement; extend to a `WordSeparatorPolicy` enum consumed by both.
    - Chosen Approach:
      - Generic Rust primitives: boundary classifier (`WordSeparatorPolicy`), `Vec<Selection>` selection model, caret-shape paint, font-feature resolution, textobject query runner. Packages/modes supply inert data only.
    - API Notes and Examples:
      ```rust
      // src/editor/buffer.rs (planned primitive)
      pub enum WordSeparatorPolicy { Code, Prose, Custom(Box<[char]>) }
      pub fn next_word_start(text: &str, from: usize, policy: WordSeparatorPolicy, long: bool) -> usize;
      pub fn next_paragraph(text: &str, from: usize, forward: bool) -> usize;
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/registry.md`: register new primitives (boundary classifier, selection set, caret style, font-feature policy, textobject query) — **deferred to task 13 (JS-API) / task 17 (wiki)** per this task's own note; the Primitive Inventory below lists exactly what to register.
      - `docs/wiki/modules/masonry-editor.md`, `editor-theme-registry.md`: note new primitives — **deferred to task 17 (final wiki)**.
    - References:
      - `src/editor/cursor.rs`, `src/editor/selection.rs`, `src/editor/buffer.rs`, `src/editor/surface.rs`, `src/editor/layout.rs`, `src/editor/typography.rs`, `src/server/syntax.rs`, `src/server/ops/keybindings.rs`.
  - Test Cases to Write:
    - Primitive inventory assertion: a `tests/primitives_docs.rs`-style check that the new primitive names appear in `registry.md` (added in the wiki/JS-API tasks).

  ### Primitive Inventory (Task 1 — completed 2026-07-31)

  All symbols verified against current source (file:line). No Markdown/Rust/TypeScript-specific Rust in any existing primitive; all gaps below are generic.

  **Existing primitives — REUSE as-is:**
  - `EditorCommand<'a>` enum (`src/editor/surface.rs:53`) — 14 variants: `Insert`, `Newline`, `Backspace`, `DeleteForward`, `MoveLeft/Right`, `SelectLeft/Right`, `MoveUp/Down`, `LineStart/End`, `DocumentStart/End`. Add new variants here (hardcoded default-key path).
  - `CursorState` (`src/editor/cursor.rs:7`) — single `caret: usize` + `preferred_x: Option<f32>`; methods `move_to_previous_scalar`/`next_scalar` (`:46/54`), `move_to_document_start/end` (`:62/66`), `move_to_line_start/end` (`:70/74`), `move_to_previous_line/next_line` (`:78/86`), private `move_to_line_preserving_scalar_column` (`:101`). Sticky-column logic reusable for new vertical motions.
  - `EditorSurface::move_cursor`/`extend_selection` (`src/editor/surface.rs:3051/3066`) — take `impl FnOnce(&mut CursorState, &EditorBuffer) -> bool`; `move_cursor` clears `selection`, `extend_selection` anchors at the original anchor and builds one `SelectionState`. **This closure pattern is the generic extension point for every new motion** (word/paragraph/non-blank/pair/select-*).
  - `EditorBuffer` (`src/editor/buffer.rs`) — `previous_scalar_boundary`/`next_scalar_boundary` (`:149/162`), `line_start_byte`/`line_end_byte`/`line_of_byte`/`byte_of_line` (`:195/215/183/191`), `scalar_column_of_byte`/`byte_for_line_scalar_column` (`:224/230`), `text_range`/`line_text_before_byte` (`:199/209`), `clamp_byte_offset` (`:141`), `document_start_byte`/`document_end_byte`/`line_len` (`:175/179/269`). Reuse for all motions; extend with word/paragraph/non-blank/pair helpers.
  - `is_completion_word_character` (`src/editor/surface.rs:901`) — the only word classifier: `character == '_' || character.is_alphanumeric()`. Used by completion trigger (`:925`) and `word_prefix_start` (`:2683`, line-bounded). **Extend, do not replace** → `WordSeparatorPolicy` consumed by movement, selection, completion, and decoration so they agree on "word".
  - `SelectionState` (`src/editor/selection.rs:6`) — single `anchor: usize, focus: usize` (`Copy`); `anchor`/`focus`/`is_collapsed`/`set_focus`/`normalized_range`/`clamped`. `EditorSurface.selection: Option<SelectionState>` (`surface.rs:1102`, `None` = collapsed). **Replaced by the selection-set model in E.4** (kept as the per-range `Selection` element).
  - `EditorBehaviorRules` inert manifest (`src/protocol/mod.rs:372`) + `default_text`/`default_code` (`:391/429`) + `BehaviorManifest::minimal_text_editing`/`core_code_editing` (`:162/171`) + package override via `parse_editor_rules` (`src/server/ops/modes.rs:248`). **Extend with `movement`/`caret_style` fields + defaults** (zero behavior change for existing modes).
  - Keybinding allowlist `is_runtime_bindable_command` (`src/server/ops/keybindings.rs:190`, 30 IDs) + `validate_command_id` (`:172`) + `command_routing_policy` (`:225`). **Add new command IDs**; movement/selection/caret → `RoutingPolicy::ClientUiCommand` (`:249`), textobject/smart-select → `ServerFirst` read-only (`:251`) or a dedicated read-only route.
  - `EditorAction::ClientUiCommand` dispatch (`src/masonry_editor.rs:1488`) — the client-side handler for `clientRequestResync`/`clientDismissRecovery`; **the extension point for new movement/selection/caret/multi-cursor op dispatch**.
  - Parley layout `rebuild` (`src/editor/layout.rs:373`) — `ranged_builder` + `push_default(StyleProperty::FontStack/FontSize/LineHeight/Brush)` + per-run `FontWeight/FontStyle/Underline/Strikethrough/Brush`. **Add `push_default(StyleProperty::FontFeatures(...))`** for ligatures.
  - `LayoutCacheKey` (`src/editor/layout.rs:52`) — fields `text_revision/viewport_revision/max_width/typography_revision/layout_style_revision/document_font_role`. **Add a font-feature-set hash** so ligature changes invalidate layout.
  - `ResolvedFontProfile` (`src/editor/typography.rs:99`) `{ families, size }` + `from_wire`/`font_stack`; `TypographyRegistry` (`:144`) with `monospace/proportional/ui` + `profile(role)` (`:187`). **Add `font_features: FontSettings<FontFeature>`** resolved from the new `FontProfile.ligatures`.
  - Tree-sitter engine `TreeSitterSyntaxHandler` (`src/server/syntax.rs:1120`) — `Query::new(&language, q)` (`:1186`) + `QueryCursor` (`use tree_sitter::{Query, QueryCursor}` at `:8`); `SyntaxGrammarContribution` (`:56`) with `highlights_query_path`/`injections_query_path: Option` (`:68/70`); static descriptor table loads queries via `include_str!` (`:253/285/…`). **Add `textobjects_query_path: Option<String>` + `textobjects_query: Option<Arc<Query>>` + a generic `run_textobject_query`/`run_smart_select`** reusing `QueryCursor` — no language-specific Rust.
  - `paint_caret` (`src/editor/surface.rs:2368`) + `CARET_WIDTH = 1.5` (`:45`) + `caret_geometry_for_visible_byte_offset` — fills one rectangle. **Rewrite shape-aware** (Bar/Line/Block/Underline + blink); remove the hardcoded constant.

  **Generic primitive GAPS to add (none exist yet; all mode/language-agnostic):**
  1. **Word-boundary classifier** — `WordSeparatorPolicy { Code, Prose, Custom(Box<[char]>) }` + `next_word_start`/`next_word_end`/`prev_word_start`/`next_sub_word`/`prev_sub_word` on `EditorBuffer` (`buffer.rs`), unifying with `is_completion_word_character`. Consumed by movement, selection, completion, decoration.
  2. **Paragraph boundary** — `next_paragraph`/`prev_paragraph`/`paragraph_end_byte` on `EditorBuffer` (blank-line-delimited; `ParagraphStyle` in `MovementRules`).
  3. **Non-blank line boundaries** — `first_non_blank_byte`/`last_non_blank_byte` on `EditorBuffer`.
  4. **Matching-pair motion** — `matching_pair_byte` helper reusing the existing inert `EditorBehaviorRules.pairs` rules (already manifest data); no new authority.
  5. **Selection-set model** — `SelectionState` → `Vec<Selection>` + primary index (E.4); `cursorUndo` snapshots the set. Replaces `Option<SelectionState>` on `EditorSurface`.
  6. **Caret-shape paint primitive** — `CaretStyle`/`CaretShape`/`BlinkStyle` in editor theme + shape-aware `paint_caret` + blink timer (E.2).
  7. **Font-feature resolution primitive** — `LigaturePolicy` in `FontProfile` → `FontSettings<FontFeature>` in `ResolvedFontProfile` → `StyleProperty::FontFeatures` push in `rebuild` + `LayoutCacheKey` feature hash (E.3).
  8. **Textobject query primitive** — `textobjects_query_path` in `SyntaxGrammarContribution` + `textobjects_query` in `TreeSitterSyntaxHandler` + generic `run_textobject_query`/`run_smart_select` (E.5).

  **Hot-path confirmation (IPC/JS-free per keystroke):** `move_cursor`/`extend_selection`/`paint_caret`/`layout.rebuild`/`route_key_with_event` (`surface.rs:1493`) all run client-side in `EditorSurface`/`EditorWidget`; behavior-manifest keybinding rules are consulted but movement/selection/caret dispatch to `EditorCommand`/`ClientUiCommand` handlers with no server round trip. Only textobject/smart-select issues one bounded server query (reuses the parsed tree) — not on the typing hot path.

  **Security confirmation:** movement/selection/caret/ligature add no authority (client-local view state + typography config); textobject/smart-select is a read-only `QueryCursor` run over the existing parse tree — no mutation, no external process (per task 0 baseline).

- [x] 2. Review Clay UI catalog and plan primitive/component reuse before caret/ligature UI work ✓ (UI Catalog Review below)
  - Acceptance Criteria:
    - Functional: Confirm caret styling + ligatures reuse existing editor chrome primitives (`src/editor/surface.rs` `paint_caret`, `StyleRegistry`, `base.caret`, `FontProfile`) and the typography registry, not the package-facing component catalog; justify why no new `ComponentKind`/typed theme token is required for caret shape/blink. **Run `npx ui-skills start`** and load the smallest useful UI skill set (prefer 1, max 3) covering OpenType features, caret styling, and accessibility/reduced-motion; apply that guidance to the caret-shape/blink and ligature design so the UI is aligned with current UI-craft conventions, not just the existing code.
    - Performance: Caret/ligature config resolution stays at theme/typography install time; paint reads cached resolved values only (no per-frame re-resolution).
    - Code Quality: Any new token (if needed for caret width/blink) is additive with a same-typed core fallback per `tokens.md`; caret color stays in editor `BaseUiColors` (`base.caret`).
    - Security: No raw colors/CSS/concrete font families from packages (packages declare semantic policy only, per `typography-role-ownership.md`).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/references/components.md` (catalog: `editorView` is editor-`StyleRegistry`-driven; caret is internal chrome in `src/editor/surface.rs`), `tokens.md` (ten typed token domains; `base.caret` lives in editor `BaseUiColors`, not the SDUI typed catalog).
      - `docs/reference/packages/creating-packages.md` (Phase 20.7 authoring contract).
      - **ui-skills CLI** (run before UI design): `npx ui-skills start` (ui-skills-root routing skill) → `npx ui-skills categories` → `npx ui-skills list --category typography` / `--category accessibility` → `npx ui-skills get <slug>`. Primary pick: `jakubkrehel/better-typography` (in both `typography` + `accessibility`; its triggers explicitly cover OpenType features, `font-feature-settings`, variable fonts, and styling underlines/selection/placeholders/**carets**). Optional second: `mengto/minimalist-ui` (matches Clay's minimalist design language per `tokens.md`). Prefer 1, max 3. (Confirmed runnable: `npx ui-skills@latest start` installs `ui-skills@0.2.4` on demand and prints the routing skill.)
    - Options Considered:
      - Expose caret shape/blink as typed SDUI theme tokens vs. editor-internal `CaretStyle` in `StyleRegistry`/manifest. Editor-internal matches existing `base.caret` ownership and keeps chrome out of the package token catalog.
      - Per-range ligature control (Approach C) vs. per-role `FontProfile` policy (Approach B). B is in scope; C is deferred.
    - Chosen Approach:
      - Run `npx ui-skills start`; load `jakubkrehel/better-typography` (OpenType features + caret styling + accessibility/reduced-motion) and, only if visual restraint guidance is needed, `mengto/minimalist-ui` (max 3 total); load the project-local `clay-ui` skill (`components.md` + `tokens.md`). Apply the loaded UI guidance to caret shape/blink geometry, the no-blink/reduced-motion option, and the OpenType feature policy **before** implementing tasks 6–7; record any deviation with a reason.
      - `CaretStyle` is editor chrome config in the `StyleRegistry`/`EditorBehaviorRules.caret_style` (theme base default + per-mode override + `clientSetCursorStyle` runtime escape hatch). `LigaturePolicy` extends `FontProfile` (user-owned family/size unchanged; packages declare semantic policy). No new `ComponentKind`; if a caret-width/blink token is desired, add it additively with a core fallback.
    - API Notes and Examples:
      ```rust
      // src/editor/theme.rs (planned, editor-internal, not SDUI token)
      pub struct CaretStyle { pub shape: CaretShape, pub width_px: u16, pub height_pct: u16,
          pub hollow: bool, pub color: Option<Color>, pub blink: BlinkStyle,
          pub smooth_animation_ms: u16, pub stop_blink_on_typing: bool }
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-ui/references/tokens.md`: **Confirmed — no caret token introduced.** Caret style is editor-internal chrome, not a catalog token; the tokens.md note recording this is deferred to task 12 (authoring contract) / task 17 (wiki).
      - `docs/reference/packages/creating-packages.md`: document the `caret_style`/`ligatures` manifest fields — deferred to task 12 (authoring contract).
    - References:
      - `src/editor/theme.rs` (`BaseUiColors`, `caret`), `src/editor/typography.rs`, `src/editor/surface.rs` (`paint_caret`, `CARET_WIDTH`), `src/shell/primitives.rs`.
  - Test Cases to Write:
    - Conformance: `tests/ui_primitive_conformance.rs` / `tests/package_ui_conformance.rs` continue to pass (no new raw-color/sizes outside `primitives.rs`/`theme.rs`); if a token is added, `core_token_catalog_matches_tokens_md` stays green.
    - UI alignment: caret-shape/blink and ligature design choices are checked against the `npx ui-skills start`-loaded `better-typography` guidance (caret styling, OpenType `font-feature-settings`, reduced-motion/no-blink accessibility option); deviations recorded with a reason.

  ### UI Catalog Review (Task 2 — completed 2026-07-31)

  **UI skills loaded (via `npx ui-skills start` → `categories` → `list --category typography` → `get`):**
  - `jakubkrehel/better-typography` (1 skill; in `typography` + `accessibility`). Sufficient on its own — its triggers cover OpenType features, `font-feature-settings`, variable fonts, and styling underlines/selection/placeholders/**carets**. `mengto/minimalist-ui` not loaded: caret/ligature is chrome + typography, not layout restraint; available if a later visual task needs it (max 3 cap respected).
  - Project-local `clay-ui` skill: `references/components.md` + `references/tokens.md`.

  **Editor chrome vs SDUI token boundary (verified, exact citations):**
  - `editorView` (`src/shell/components.rs` `ComponentKind`; `components.md:13`) is a **pane-slot binding only** — the editor canvas is bespoke-painted by `EditorWidget`, not an SDUI component (`components.md:50`); its chrome is editor-`StyleRegistry`-driven (`components.md:39`).
  - Caret **color** lives at `BaseUiColors.caret` (`src/editor/theme.rs:72`) inside the editor `StyleRegistry` (`theme.rs:99`, default white `:143`) — "separate from SDUI typed tokens" (`tokens.md:66`).
  - SDUI typed tokens = `ThemeTokenType` ten additive domains (`src/shell/theme.rs`); package-facing via `clay.ui.serverRegisterThemeToken` / `clay.contributions.themeTokens`/`designTokens`; **raw colors/CSS/style strings rejected at load time** (`tokens.md:204/206`); new tokens must be one of the ten types + same-typed core fallback (`tokens.md:211`).
  - SDUI chrome primitives = `src/shell/primitives.rs` `pub(crate)` inert helpers reading `ResolvedUiTheme` tokens; packages cannot call them (`components.md:108–110`).

  **Reuse decision (no new `ComponentKind`, no new `ThemeTokenType`):**
  - **Caret style** → editor-internal `StyleRegistry`/`EditorBehaviorRules.caret_style` + shape-aware `paint_caret`. Reuse `BaseUiColors.caret` (color), `paint_caret` + `caret_geometry_for_visible_byte_offset` (`surface.rs:2368`). Caret **width/blink timing = `CaretStyle` fields (editor-internal)**, NOT SDUI `dimension`/`motion-duration` tokens — keeps ownership with editor chrome; if a package ever needs to theme them, that is a later additive token with a core fallback, not now.
  - **Ligatures** → typography registry (`TypographyRegistry`/`ResolvedFontProfile`/`parse_profile`) + parley `StyleProperty::FontFeatures` + `LayoutCacheKey` feature hash. Reuse `FontProfile` (user-owned family/size, per `typography-role-ownership.md`) + `layout.rs::rebuild`. NOT an SDUI token/component.

  **better-typography guidance applied:**
  - **§2 "Properties Over Raw Tags"** maps directly to `LigaturePolicy`: semantic toggles first (`enable_standard`→`liga`+`clig`, `enable_contextual`→`calt`); reserve the raw escape hatch (`raw_features`/`disable_features`) for niche features with no semantic toggle (`ss0X`/`cv0X`/`zero`/`onum`). **Validates the `LigaturePolicy` field design exactly.** Sub-reference `variable-fonts-and-opentype.md` is the detailed OpenType vocabulary for tasks 6–7.
  - **Accessibility (§19 + `details-and-accessibility.md`):** keep text selectable; provide `BlinkStyle::Solid` (no-blink) as the reduced-motion option + `stop_blink_on_typing` to reset on input. Already in the `CaretStyle` design (task 6).

  **Deviation recorded (with reason):** `better-typography` is web-oriented (CSS properties, `.woff2`, iOS zoom, Tailwind). Clay is native (parley/vello/masonry). The skill's *principles* (semantic toggles over raw tags, restraint, accessibility/reduced-motion, keep selectable) are mapped to parley `StyleProperty::FontFeatures` + editor chrome; the **OpenType feature vocabulary transfers directly**; the CSS/Tailwind mechanics are N/A and not applied.

  **Conformance:** no `ComponentKind`, typed-style-variable, token-name, or package-facing contract change → all four drift guards in `tests/package_ui_conformance.rs` stay green; no raw color/size outside `primitives.rs`/`theme.rs`.

- [x] 3. Review package-provided grammar primitives before tree-sitter text-object work ✓ (Grammar Primitive Review below)
  - Acceptance Criteria:
    - Functional: Inventory `src/server/syntax.rs` (tree-sitter `Query`/`QueryCursor`/`Node`, `include_str!` query loading), `packages/*/queries/*.scm`, `src/packages/record.rs` `styleMap`, and the parse/decoration transport; confirm text objects can be a new generic grammar contribution (`queries/textobjects.scm`) with no language-specific Rust.
    - Performance: Text-object queries reuse the existing parsed tree (no re-parse); query execution is bounded + cancellable; not on the typing hot path.
    - Code Quality: The text-object query schema is generic and reusable by every future language package.
    - Security: Arbitrary third-party native grammar/artifact loading remains out of scope; only resolver-validated first-party packages register live grammar contributions.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/syntax-vocabulary.md`, `docs/wiki/modules/parse-coordinator.md`, `decoration-transport.md`, `low-latency-incremental-syntax-decoration-primitive-review.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md` (package-provided grammar authority).
    - Options Considered:
      - Hard-coded per-language text objects in Rust vs. Helix-style `@textobject.{start,end}` query captures in `textobjects.scm`. Query-based keeps it generic (no per-language Rust) and matches the package-provided-grammar pattern.
    - Chosen Approach:
      - New `textobjects.scm` query files per built-in language package; a generic server op runs the query for the active document's grammar and returns ranges; the client applies them as selections. Object vocabulary: word/paragraph/function/class/argument/comment/comment-block/test/tag.
    - API Notes and Examples:
      ```scheme
      ;; packages/rust/queries/textobjects.scm (planned)
      (function_item name: (identifier) @textobject.function.start) @textobject.function.end
      (block) @textobject.class.inner
      ```
      ```rust
      // src/server/ops/editor.rs (planned, read-only)
      fn op_clay_editor_select_textobject(doc, object, around, direction) -> Vec<Range>;
      ```
    - Files to Create/Edit:
      - `packages/{rust,typescript,javascript,markdown}/queries/textobjects.scm`: object captures (≥1 built-in language in this phase).
      - `src/server/syntax.rs`: add a `textobjects_query` descriptor + `run_textobject_query` (generic, no language branch).
    - References:
      - `src/server/syntax.rs`, `src/packages/record.rs`, `src/server/js_runtime.rs` (`queries` config).
  - Test Cases to Write:
    - Query load: `textobjects.scm` parses for the built-in language; invalid query falls back gracefully.
    - Range correctness: inner/around function/class/argument/comment ranges at known offsets match expectations.

  ### Grammar Primitive Review (Task 3 — completed 2026-07-31)

  All symbols verified against current source (file:line). Tree-sitter deps: `tree-sitter 0.25`, `-javascript 0.25`, `-md-025 0.5.6`, `-rust 0.24.2`, `-typescript 0.23.2` (`Cargo.toml:30-34`). Engine imports `Query, QueryCursor, Node, Tree, InputEdit, Language, Parser, Point` (`syntax.rs:8`).

  **Existing grammar primitives — REUSE as-is:**
  - **Two-tier grammar registry** (`TreeSitterSyntaxHandler`, `syntax.rs:1120`):
    - *First-party native (compile-time):* `FIRST_PARTY_NATIVE_GRAMMARS: &[NativeGrammarDescriptor]` (`:274`) — 5 entries (rust, typescript×2, javascript, markdown); each carries `highlights_query: &'static str` loaded via `include_str!("../../packages/…/queries/*.scm")` (`:285/308/326/344/362`) + `injections_query: Option<&'static str>` (`:370`, markdown only). Registered via `register_first_party_native_grammars` (`:513`) → `SyntaxGrammarContribution::from_descriptor` (`:78/578`).
    - *Third-party WASM (runtime, permission-gated):* `SyntaxGrammarContributionDescriptor` (`packages/record.rs:144`) declared in package manifests as `syntaxGrammars` (`:482/920`); requires `PackagePermission::ParseDocument` (`:835` = `clay.syntax.serverRegisterSyntaxGrammar`); WASM tier only (`grammar_kind == "tree-sitter-wasm"`, `:154`); paths confined by `validate_wasm_path`/`validate_query_path` (`syntax.rs:134-151`, package-root-confined). Tier2 `tier2_override` lets a third-party grammar shadow a first-party one (`:585/602/715/743`).
  - **Query fields (the extension surface):** `SyntaxGrammarContribution` (runtime, `syntax.rs:56`) has `highlights_query_path: String` (`:68`), `locals_query_path: Option<String>` (`:69`), `injections_query_path: Option<String>` (`:70`). `NativeGrammarDescriptor` (`:203`) mirrors with `&'static str` + the `include_str!`-loaded content. `SyntaxGrammarContributionDescriptor` (`record.rs:144`) mirrors for manifests. **Add `textobjects_query_path: Option<String>`/`Option<&'static str>` + `textobjects_query: Option<&'static str>` to all three — same shape as `injections_query_path`, validated by the existing `validate_query_path`.**
  - **Query execution primitive:** `Query::new(&language, query_str)` (`:1186` highlights, `:1219` injections, `:1665` embedded) + `QueryCursor::new()` (`:1430/1532/1603`) over the already-parsed `Tree`. **`run_textobject_query` reuses this exactly** — `QueryCursor` over the existing tree, no re-parse, no language branch.
  - **Incremental parse + bounded query:** continuity edits keep one bounded parse + query per edit (test `:2287`); textobjects inherit the same reused tree — **not on the typing hot path**, only on explicit textobject/smart-select command.
  - **Style map:** `style_map: BTreeMap<String, SyntaxStyleMapEntry>` (`record.rs:167`) maps highlight *capture names → Clay style tokens*. Textobject captures (`@textobject.{object}.{start|end|inner|around}`) are **NOT style tokens** — they are range markers consumed by the selection op, not the painter. **Textobjects do NOT touch `style_map`.**
  - **Injection security:** the injection executor refuses any embedded grammar name not in `FIRST_PARTY_EMBEDDED_GRAMMARS` (`:250/1657`) — only Clay-vendored artifacts parse an injected range. Textobjects do not inject, so this guard is untouched.

  **Generic primitive GAP to add (one, language-agnostic):**
  - **Textobject query contribution + runner:** `textobjects_query_path` field on the three descriptor/contribution structs (+ `textobjects_query` `&'static str` on `NativeGrammarDescriptor`, `include_str!`-loaded for first-party); `queries/textobjects.scm` per built-in language package (Helix-style captures `@textobject.function.start`/`.end`/`.inner`/`.around`, object vocabulary word/paragraph/function/class/argument/comment/comment-block/test/tag); generic `run_textobject_query(tree, query, byte_offset, object, around, direction) -> Vec<Range>` + `run_smart_select(tree, query, byte_offset, action) -> Vec<Range>` in `syntax.rs` reusing `QueryCursor`. No per-language Rust; every future language package ships only a `.scm` file.

  **Security confirmation:** textobject queries add no new authority. First-party: vendored `include_str!` (compile-time, same as highlights). Third-party: same WASM-tier + `validate_query_path` package-root-confinement + `ParseDocument` permission gate as highlights/injections — no new permission, no native artifact loading, no injection. Execution is a read-only `QueryCursor` run over the existing parse tree — no mutation, no external process (per task 0 baseline).

  **Performance confirmation:** reuses the already-parsed `Tree` (no re-parse); bounded query (same budget model as highlights); fired only on explicit `clientSelectTextobject`/`clientSmartSelect` commands, never per keystroke.

  **Schema decision (Helix-compatible):** capture names follow `@textobject.{object}.{start|end}` for boundary motions (goto-next/prev-function jumps to the capture range) and `@textobject.{object}.inner`/`@textobject.{object}.around` for select-inner/around. This matches Helix's `select_textobject_around/inner` + the unimpaired `]f/[f`/`]c/[c`/`]t/[t` vocabulary (proposal §9) and keeps textobjects orthogonal to the highlight `style_map` namespace.

- [x] 4. Implement movement primitives + `MovementRules` (E.1 part 1) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `CursorState`/`EditorBuffer` support next/prev word start/end, long-WORD, sub-word (camelCase), next/prev paragraph + paragraph end, first/last non-blank, matching pair; all honor a configurable `WordSeparatorPolicy` and `ParagraphStyle`; sticky column preserved for vertical motion. `MovementRules` added to `EditorBehaviorRules` (default = current code behavior) with no behavior change for existing modes.
    - Performance: O(text length) worst case per motion; no allocation on the hot path for simple motions; no IPC/JS per keystroke.
    - Code Quality: Boundary classifier unifies with `is_completion_word_character` so movement, selection, and completion agree on "word"; Unicode-safe (scalar boundaries, combining marks, CRLF).
    - Security: Read-only buffer inspection; no authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-editor.md`, `docs/reference/primitives/registry.md`, `behavior-manifests.md`.
    - Options Considered:
      - Extend `EditorCommand` enum (Approach A) vs. generic command table (Approach B) vs. selection-first rewrite (C). Per proposal §5.4: A+B hybrid — `EditorCommand` variants for the hardcoded default-key path, allowlisted arg-bearing `clientMoveCursor` for rebinding.
    - Chosen Approach:
      - Add `EditorCommand` variants (`MoveWordStart{forward,long,extend}`, `MoveWordEnd{...}`, `MoveSubWord{forward,extend}`, `MoveParagraph{forward,to_end,extend}`, `MoveFirstNonWhitespace{extend}`, `MoveLastNonWhitespace{extend}`, `MoveMatchingPair{extend}`) routed through `EditorSurface::move_cursor`/`extend_selection`; add `MovementRules` to `EditorBehaviorRules`.
    - API Notes and Examples:
      ```rust
      // src/protocol/mod.rs (planned, inert data, rkyv)
      pub struct MovementRules { pub word_separators: WordSeparatorPolicy,
          pub treat_underscore_as_word: bool, pub camel_case_sub_word: bool,
          pub paragraph_style: ParagraphStyle, pub stop_at_eol_word_end: bool,
          pub line_movement: LineMovementStyle, pub sticky_column: bool }
      ```
    - Files to Create/Edit:
      - `src/editor/cursor.rs`: add word/paragraph/non-blank/matching-pair motions.
      - `src/editor/buffer.rs`: add boundary classifier (`WordSeparatorPolicy`, `next_word_start`, `next_paragraph`, …).
      - `src/editor/surface.rs`: add `EditorCommand` variants + dispatch.
      - `src/protocol/mod.rs`: add `MovementRules` to `EditorBehaviorRules`; defaults in `default_text`/`default_code`.
      - `src/packages/manifest.rs`: `buildCodeEditingManifest` sets code `MovementRules`; expose `buildMovementRules` if appropriate (verify naming per `clay-js-api-naming.md`).
    - References:
      - `src/editor/cursor.rs`, `buffer.rs`, `surface.rs`, `src/protocol/mod.rs:370`, `src/packages/manifest.rs`.
  - Test Cases to Write:
      - Word motion (code policy): `foo.bar_baz` → next-word-start lands on `b` of `bar`; sub-word lands at `b` of `baz` and `B` of `Baz` in camelCase.
      - Long-WORD: `a, b, c` → next long-word-start skips punctuation.
      - Paragraph: blank-line-delimited next/prev + end-of-paragraph.
      - Non-blank: first/last non-whitespace on a line.
      - Matching pair: `({[]})` toggles between matching brackets.
      - Unicode: combining marks do not split a grapheme; CRLF handled.

  ### Task 4 Implementation Note (completed 2026-07-31)

  **Implemented (all verified, Linux gate green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`):**
  - `src/protocol/mod.rs`: `WordSeparatorPolicy` (`Code`/`Prose`/`Custom(Vec<char>)`) with `is_word_char`, `ParagraphStyle`, `LineMovementStyle`, `MovementRules` (`default_code`/`default_text`/`Default`); `movement: MovementRules` field on `EditorBehaviorRules` set in `default_text` (inherited by `default_code`).
  - `src/editor/buffer.rs`: `char_at`/`char_before`, `classify_word` (with `long` WORD override + combining-mark grapheme continuation, U+0300–U+036F), `next_word_start`/`next_word_end`/`prev_word_start`/`prev_word_end`, `next_sub_word_start`/`prev_sub_word_start` (camelCase + underscore + digit boundaries via `is_sub_word_start`), `next_paragraph`/`prev_paragraph`/`paragraph_end_byte`/`is_blank_line`, `first_non_blank_byte`/`last_non_blank_byte`, `matching_pair_byte` (single-char distinct pairs, depth-balanced forward/backward).
  - `src/editor/cursor.rs`: `move_to_next_word_start`/`prev_word_start`/`next_word_end`/`prev_word_end`/`next_sub_word_start`/`prev_sub_word_start`/`next_paragraph`/`prev_paragraph`/`paragraph_end`/`first_non_blank`/`last_non_blank`/`matching_pair` (fall back to doc start/end on `None`).
  - `src/editor/surface.rs`: `EditorCommand` variants `MoveWordStart{forward,long,extend}`/`MoveWordEnd{…}`/`MoveSubWord{forward,extend}`/`MoveParagraph{forward,to_end,extend}`/`MoveFirstNonWhitespace{extend}`/`MoveLastNonWhitespace{extend}`/`MoveMatchingPair{extend}` + dispatch in `command_with_event`; `movement_rules()`/`sticky_column_enabled()`/`move_or_extend()` helpers; `move_word_start`/`move_word_end`/`move_sub_word`/`move_paragraph`/`move_first_non_blank`/`move_last_non_blank`/`move_matching_pair`; `move_up`/`move_down` now honor `sticky_column` (default `true` = unchanged); `is_completion_word_character` unified to `WordSeparatorPolicy::Code.is_word_char(c, true)` (one classifier, completion behavior unchanged).
  - `src/server/ops/modes.rs`: `parse_movement_rules` parses the optional `editorRules.movement` override (`wordSeparators` "code"/"prose"/`{custom:[…]}`, `treatUnderscoreAsWord`, `camelCaseSubWord`, `paragraphStyle`, `stopAtEolWordEnd`, `lineMovement`, `stickyColumn`); absent/partial → `MovementRules::default()` (no behaviour change for existing modes).

  **Classifier unification:** movement, selection, and completion share `WordSeparatorPolicy::is_word_char`; `is_completion_word_character` delegates to `Code` with `treat_underscore_as_word=true` (exact match to the historical `_ || alphanumeric`). Completion deliberately stays on the code default so token detection is stable across prose/custom movement policies; word motion adds combining-mark continuation on top for grapheme safety.

  **Root-cause fix (not a task-4 symptom patch):** adding `MovementRules` to `EditorBehaviorRules` inflated the `ServerMessage` rkyv enum (sized by its largest variant, `BehaviorManifest`), pushing `EditAck` serialized size 128→144 and breaking `BEHAVIOR_MANIFEST`/EditAck budget guard. Fixed by boxing the manifest: `ServerMessage::BehaviorManifest(Box<BehaviorManifest>)`. This decouples small messages (EditAck/Welcome/etc.) from manifest size and **future-proofs E.2 (caret_style) and E.3 (ligatures)** which add more `EditorBehaviorRules` fields — the enum no longer re-breaks. `PROTOCOL_VERSION` bumped 5→6 (wire-layout change; dev tree, no live compat). 46 construction sites wrapped (`Box::new`), match sites use auto-deref / `*manifest` / `manifest.as_ref().clone()`.

  **Ceilings (`ponytail:` comments in source):** `LineMovementStyle::ScreenLine` falls back to `Character` (visual-line motion needs laid-out text); combining-mark continuation covers U+0300–U+036F only (full grapheme clustering needs `unicode-segmentation`); `matching_pair_byte` handles single-char distinct open/close pairs only (same-char/multi-char pairs skipped — bracket matching is the common case).

  **Tests added:** 14 `editor::buffer::movement_tests` (word start/end, long-WORD, sub-word camel+underscore, sub-word camel-disabled, paragraph next/prev/end, non-blank first/last, matching-pair toggle/caret-after-close/nested, combining-mark grapheme, CRLF) + 2 `editor::surface::tests` (MoveWordStart command dispatch + extend selection via manifest policy; MoveMatchingPair command toggle via manifest pairs). All 1162 lib tests pass; no regressions.

  **Pre-existing failure (NOT task-4):** `primitives_docs::plan061_runtime_package_authority_rebaseline_matches_source_inventory` fails on clean HEAD (source op-inventory count 69 vs plan-061 baseline 68). Confirmed via `git stash`: unrelated stale plan-061 baseline; not caused by task 4 (adds no `op_clay_*` op). Out of scope here; the plan-061 inventory marker needs rebaselining independently.

- [x] 5. Wire `clientMoveCursor` + `clientSetSelection` as allowlisted `ClientUiCommand` ops (E.1 part 2) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `clay.editor.clientMoveCursor` (`{direction, granularity, extend, count}`) and `clay.editor.clientSetSelection` (`{action: selectWord|selectLine|selectParagraph, extend, direction}`) are real `deno_core` ops in `src/server/ops/editor.rs`, allowlisted in `is_runtime_bindable_command`, routed `ClientUiCommand`, dispatched in `EditorWidget`; default keys wired (Ctrl+Left/Right word, Ctrl+Shift extend, Ctrl+Up/Down paragraph, Ctrl+Shift+Up/Down extend paragraph, Ctrl+L select-line, Ctrl+D select-word). Rebinding from `init.js` works.
    - Performance: Commands execute client-local; no server round trip on the hot path.
    - Code Quality: Typed args validated server-side (deny-by-default unknown enum values); matches documented `client-move-cursor.md`/`client-set-selection.md` schema; `clay-js-api-naming.md` applied (`client*` = client-executed).
    - Security: New ops grant no document mutation/external authority; movement/selection are client view state.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/editor/client-move-cursor.md`, `client-set-selection.md` (currently `stability: planned`); `clay-js-api-naming.md`, `clay-js-api-schema.md`.
      - `src/server/ops/keybindings.rs` (`is_runtime_bindable_command`, `command_routing_policy`), `src/masonry_editor.rs` (`EditorAction::ClientUiCommand` handler).
    - Options Considered:
      - One generic `clientMoveCursor` with `granularity` enum vs. many narrow command IDs. Generic + typed args (Approach B) gives max rebinding flexibility and Vim-style `count`; kept alongside `EditorCommand` defaults (Approach A).
    - Chosen Approach:
      - Create `src/server/ops/editor.rs` with `op_clay_editor_move_cursor`/`op_clay_editor_set_selection`; add IDs to the bindable allowlist + `command_routing_policy` (`ClientUiCommand`); dispatch in `EditorWidget` translating args → `EditorCommand`/`CursorState` methods; update the existing planned docs from `planned` → implemented.
    - API Notes and Examples:
      ```ts
      import { clientMoveCursor, clientSetSelection } from "clay:editor";
      clientMoveCursor({ direction: "nextWordStart", extend: false, count: 1 });
      clientSetSelection({ action: "selectLine", extend: true });
      ```
    - Files to Create/Edit:
      - `src/server/ops/editor.rs`: new file; ops + validation.
      - `src/server/ops/keybindings.rs`: add IDs to `is_runtime_bindable_command`; routing in `command_routing_policy`.
      - `src/masonry_editor.rs`: dispatch in `EditorAction::ClientUiCommand` handler.
      - `runtime/js/editor.js`: implement `clientMoveCursor`/`clientSetSelection` facades (currently planned stubs).
      - `docs/reference/clay-js-api/editor/client-move-cursor.md`, `client-set-selection.md`: `stability: planned` → implemented; fill `backing_rust`/`deno_op`/`custom_properties`.
    - References:
      - `src/server/ops/keybindings.rs`, `src/masonry_editor.rs:1488`, `runtime/js/editor.js`.
  - Test Cases to Write:
    - Op routing: a bound chord reaches `EditorSurface` with correct args; unknown enum value is rejected.
    - Rebinding: `init.js` remapping `Ctrl+Right` to `nextParagraph` moves the caret by paragraph.
    - Default keys: the listed defaults produce the documented motions.

  ### Task 5 Implementation Note (completed 2026-07-31)

  **Architecture decision (root-cause, sets the pattern for E.2/E.6):** the keybinding route is **argless** (`KeyBindingRule`/`ClientUiCommandRoute` carry only `command_id`; `bindKey` accepts no args). So direction-specific movement cannot flow args through a chord. Two clean surfaces instead of one overloaded one:
  - **Keybinding/rebinding surface = six argless direction-specific command IDs** (the architecture-native pattern, like `clientCopySelection`): `clay.editor.clientMoveCursor.{nextWordStart,prevWordStart,nextParagraph,prevParagraph}` + `clay.editor.clientSetSelection.{selectWord,selectLine}`. Allowlisted in `is_runtime_bindable_command`, routed `ClientUiCommand`, dispatched in `EditorWidget`. Rebinding works: `bindKey("Ctrl+Right", "clay.editor.clientMoveCursor.nextParagraph")`.
  - **Programmatic surface = the generic typed-args ops** `op_clay_editor_move_cursor({direction,granularity,extend,count})` / `op_clay_editor_set_selection({action,extend,direction})` in new `src/server/ops/editor.rs`, deny-by-default enum validation, registered in the **trusted** extension only (third-party access deferred to task 15; `bindKey` is admin-only anyway so third parties can't register keybindings).

  **Implemented (all verified, Linux gate green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`):**
  - `src/editor/buffer.rs`: `word_range_at` (word run at caret, combining-mark aware, `None` on separator), `line_range` (line content excl. terminator), `paragraph_range` (maximal non-blank line run).
  - `src/editor/surface.rs`: `EditorCommand::{SelectWord,SelectLine,SelectParagraph}` + dispatch; `EditorSurface::{select_word,select_line,select_paragraph}` + `set_selection_range` helper.
  - `src/masonry_editor.rs`: default keys in `on_text_event` — Ctrl+Left/Right = word start (Shift extends), Ctrl+Up/Down = paragraph (Shift extends), Ctrl+L = select-line, Ctrl+D = select-word; `EditorClientCommand` enum + `from_command_id` + `EditorWidget::apply_editor_client_command` (ID → `EditorCommand`).
  - `src/server/ops/editor.rs` (NEW): `op_clay_editor_move_cursor`/`op_clay_editor_set_selection` + `validate_move_cursor`/`validate_set_selection` (plain, unit-testable); registered in trusted extension (`ops/mod.rs`, trusted op count 69→71).
  - `src/server/ops/keybindings.rs`: six IDs added to `is_runtime_bindable_command` + `command_routing_policy(ClientUiCommand)`.
  - `src/main.rs`: `ClientUiCommandResult::EditorCommand(EditorClientCommand)` variant; `handle_client_ui_command` maps the six IDs; dispatch arm calls `apply_editor_client_command`.
  - `runtime/js/editor.js` + `editor.d.ts`: `clientMoveCursor`/`clientSetSelection` now call the real ops (were `plannedApi` stubs); direction vocabulary + `granularity`/`extend`/`count` / `action` types.
  - Docs: `client-move-cursor.md` / `client-set-selection.md` `planned`→`runtime-backed`, full schema; `api-inventory.toml` entries updated; `docs/generated/clay-js-api-registry.json` regenerated via `cargo run --bin update-doc-registry`.

  **Ceilings (`ponytail:` in source):** (1) the ops **validate + return** the command descriptor; live programmatic op→client-cursor execution needs a server→client `ExecuteClientUiCommand` push channel (not wired) — the keybinding route is the execution path; the push channel is deferred and will be reused by `clientSetCursorStyle` (E.2) / `clientScrollTo` / `clientSetViewport`. (2) Extend-variant rebinding uses the chord's Shift modifier on the default keys; argless extend command IDs are not provided (rebind extend via the default Shift chords). (3) `select_word` no-ops when the caret is on a separator (VSCode selects the next word) — add when a `count`-aware select op needs it.

  **Tests added:** 7 `server::ops::editor` validation tests (known/unknown direction, missing direction, malformed JSON, count clamp, known/unknown action) + 4 `editor::surface` select tests (select_word at caret / no-op on separator, select_line, select_paragraph) + 1 rebinding routing test (`editor_routes_rebound_move_cursor_command_to_client_ui`: Ctrl+Right bound to `nextParagraph` routes `ClientUiCommand`, no local mutation) + 1 `masonry_editor` test (`editor_client_command_maps_ids_and_moves_caret`: all six ID mappings + `apply_editor_client_command` moves caret 0→7 and selects "text" 7..11). 1175 lib tests pass; no regressions.

  **Pre-existing failure (NOT task-5):** `primitives_docs::plan061_runtime_package_authority_rebaseline_matches_source_inventory` still fails on clean HEAD (stale plan-061 op-inventory baseline 68 vs source 69) — unrelated, confirmed in task 4.

- [x] 6. Implement caret styling + blink (E.2) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `CaretStyle` (Bar/Line/Block/Underline, `width_px`, `height_pct`, `hollow`, color override, `BlinkStyle`, `smooth_animation_ms`, `stop_blink_on_typing`) lives in the editor `StyleRegistry` (theme base default) + `EditorBehaviorRules.caret_style` (per-mode override) + `clay.editor.clientSetCursorStyle` (runtime). `paint_caret` renders all shapes; blink phases on/off/wait, reset on input; optional smooth animation reuses the `visual_scroll_y` interpolation; IME preedit caret stays shape-consistent.
    - Performance: Blink/animation use existing `request_anim_frame`; no per-frame re-resolution; primary caret blinks, secondary carets (post-E.4) render solid.
    - Code Quality: No raw colors/sizes outside `theme.rs`/`primitives.rs` (per UI conformance); `CaretStyle` is `rkyv`-serializable inert data.
    - Security: `clientSetCursorStyle` is client-local; no authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/references/tokens.md`, `components.md` (`editorView` chrome is `StyleRegistry`-driven); `docs/wiki/modules/editor-theme-registry.md`.
      - `jakubkrehel/better-typography` UI skill loaded in task 2 (caret-styling + accessibility/reduced-motion guidance).
    - Options Considered:
      - Theme-token only (A) vs. manifest `caret_style` (B) vs. separate `CaretProfile` side channel (C). Per proposal §6.3: B (theme default + `EditorBehaviorRules.caret_style` override + `clientSetCursorStyle` runtime).
    - Chosen Approach:
      - Add `CaretStyle` to editor theme base; add `EditorBehaviorRules.caret_style: Option<CaretStyle>`; rewrite `paint_caret` shape-aware; add blink timer in `EditorSurface`/`EditorWidget`; wire `clientSetCursorStyle` op + allowlist. Keep a no-blink `Solid` option and honor `stop_blink_on_typing`; per `better-typography` accessibility guidance, document the no-blink option as the reduced-motion choice (secondary carets render solid).
    - API Notes and Examples:
      ```rust
      pub enum CaretShape { Bar, Line, Block, Underline }
      pub enum BlinkStyle { Solid, Blink { on_ms: u32, off_ms: u32, wait_ms: u32 }, Phase, Smooth }
      ```
    - Files to Create/Edit:
      - `src/editor/theme.rs`: `CaretStyle` + `caret_style` in `StyleRegistry`; theme key `caretStyle`.
      - `src/editor/surface.rs`: rewrite `paint_caret`; remove hardcoded `CARET_WIDTH`.
      - `src/protocol/mod.rs`: `EditorBehaviorRules.caret_style`.
      - `src/server/ops/editor.rs`: `op_clay_editor_set_cursor_style` + allowlist.
      - `runtime/js/editor.js`: `clientSetCursorStyle` facade.
      - `docs/reference/clay-js-api/editor/client-set-cursor-style.md`: `planned` → implemented.
    - References:
      - `src/editor/surface.rs:2368` (`paint_caret`, `CARET_WIDTH=1.5`), `src/editor/theme.rs:72/143` (`caret`).
  - Test Cases to Write:
    - Shape paint: Bar/Line/Block(hollow)/Underline render correct geometry at a known caret x.
    - Blink: phase toggles on/off per timing; resets to on + restarts wait on a keystroke; `Solid` never hides.
    - Per-mode override: a fixture mode's `caret_style` overrides the theme default.
    - Runtime: `clientSetCursorStyle({ shape: "block", blink: "solid" })` takes effect.
    - IME: preedit caret respects the active shape.

  ### Task 6 Implementation Note (completed 2026-07-31)

  **Types (protocol/mod.rs, rkyv wire data):** `CaretShape { Bar, Line, Block, Underline }`, `BlinkStyle { Solid, Blink{on_ms,off_ms,wait_ms}, Phase{period_ms}, Smooth{period_ms} }` (with `animates()/wait_ms()/on_ms()/off_ms()` accessors), and `CaretStyle { shape, width_px:f32, height_pct:f32, hollow, blink, smooth_animation_ms, stop_blink_on_typing }` (`const fn default_bar()` = solid 1.5px bar, the reduced-motion-safe default that reproduces the historical caret). `EditorBehaviorRules.caret_style: Option<CaretStyle>` added. **Colour stays theme-owned** (`base.caret`); `CaretStyle` owns shape+blink only, so it never carries raw colour (UI-conformance + `typography-role-ownership`).

  **Eq cascade (root-cause):** `CaretStyle` has `f32` fields, so `Eq` was dropped from the habit-derived chain that transitively contains it: `EditorBehaviorRules`, `BehaviorManifest`, `ClientBehaviorState`, `EditorDocumentState`, `PackageBehaviorContribution`, `ActiveBehaviorManifest`. Verified none are map/set keys (only `PartialEq` is load-bearing, for `assert_eq!`); `ClientInitialState` was already `PartialEq`-only.

  **Implemented (all verified, Linux gate green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`):**
  - `src/editor/theme.rs`: `StyleRegistry.caret_style: CaretStyle` (editor-chrome default, `clay_default()`); `CaretStyle` imported from protocol.
  - `src/editor/layout.rs`: `CaretCell { x, line_top, line_bottom, advance }` + `caret_cell_for_visible_byte_offset` — measures the char-cell advance via parley `Cursor::next_visual` when the successor shares the line, else a line-height×0.6 estimate (EOL/EOF).
  - `src/editor/surface.rs`: `CaretBlink` pure state machine (`BlinkPhase` Wait/On/Off; `advance`/`reset`/`is_visible`; zero-phase skip bounded against degenerate periods) + `EditorSurface` fields `caret_style_override`/`caret_blink`; `effective_caret_style()` (runtime override → manifest → theme), `set_caret_style_override`, `advance_blink` (returns visibility-changed), `caret_animates`, `caret_blink_visible`; **`paint_caret` rewritten shape-aware** (Bar/Line stroke, Block fill or hollow stroke, Underline baseline bar; `height_pct` centre-anchored; skips paint in the off-phase) using `style.width_px` instead of the hardcoded `CARET_WIDTH`; **preedit (IME) caret shape-consistent**; `command()` resets the blink when `stop_blink_on_typing`.
  - `src/masonry_editor.rs`: `Widget::update` kicks off the blink loop on `Update::FocusChanged(true)`; `Widget::on_anim_frame` advances the blink (ns→ms), repaints on visibility change, and self-perpetuates while focused+animating (loop ends when focus is lost or the style is `Solid`).
  - `src/server/ops/modes.rs`: `parse_caret_style` (lenient field-by-field manifest parsing, mirroring `parse_movement_rules`) wired into `parse_editor_rules`.
  - `src/server/ops/editor.rs`: `op_clay_editor_set_cursor_style` + `validate_set_cursor_style` (deny-by-default for present `shape`/`blink` via new `optional_string_strict`); registered **trusted-only** (op count 71→72; `package_extension_is_strict_subset` baseline bumped).
  - `runtime/js/editor.js` + `editor.d.ts`: `clientSetCursorStyle` now calls the real op (was a `plannedApi` stub); new `shape`/`blink`/`widthPx`/`heightPct`/`hollow`/`stopBlinkOnTyping` vocabulary (old `color`/`blinking`/`type` retired — colour is theme-owned).
  - Docs: `client-set-cursor-style.md` `planned`→`runtime-backed` + full new schema; `api-inventory.toml` entry updated; `docs/generated/clay-js-api-registry.json` regenerated; `tests/clay_js_doc_registry.rs` cursor-style contract tests rewritten for the new vocabulary.

  **Ceilings (`ponytail:` in source):** (1) `BlinkStyle::Phase`/`Smooth` render with discrete on/off timing — true per-frame alpha-fade is deferred (the anim-frame loop supports it). (2) The `clientSetCursorStyle` op **validates + returns**; live op→client application uses the same deferred server→client `ExecuteClientUiCommand` push channel as task 5's `clientMoveCursor` — the working execution paths today are the per-mode manifest `caret_style` and the client-local `EditorSurface::set_caret_style_override`. (3) Block/Underline cell width at EOL/EOF uses a line-height×0.6 estimate (no same-line successor to measure). (4) The blink loop kicks off on focus/interaction; a programmatically-focused editor with no interaction shows a solid caret until the first frame request.

  **Tests added (12):** 5 `editor::surface` (Solid always visible; Blink Wait→On→Off→On timing; reset returns to visible+Wait; `effective_caret_style` override→manifest→theme resolution + clear-fallback; `command()` resets blink when `stop_blink_on_typing`) + 4 `server::ops::editor` cursor-style validation (known shape/blink, all-optional, unknown shape/blink deny-by-default) + 3 `server::ops::modes` `parse_caret_style` (absent→None, shape/blink mapping, partial→defaults). 1187 lib tests pass; no regressions.

  **Pre-existing failure (NOT task-6):** `primitives_docs::plan061_runtime_package_authority_rebaseline_matches_source_inventory` still fails (stale plan-061 op-inventory baseline) — unrelated, deferred to tasks 13/17.

- [x] 7. Implement font ligatures (E.3) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `FontProfile` gains `ligatures: LigaturePolicy` (`enable_standard`, `enable_contextual`, `discretionary_features`, `raw_features`, `disable_features`); parsed in `parse_profile`; resolved into a `FontSettings<FontFeature>` in `ResolvedFontProfile`; `layout.rs::rebuild` pushes `StyleProperty::FontFeatures(...)` per role; `LayoutCacheKey` extended by a feature-set hash so changes invalidate layout; per-mode policy via `EditorBehaviorRules.ligatures` (or typography override). `init.js` can disable `calt`.
    - Performance: Feature resolution at typography install time; layout cache keyed on features; no per-frame re-resolution.
    - Code Quality: Uses parley 0.6 `StyleProperty::FontFeatures` + `swash::Setting<u16>` (value 0=off/1=on); prefers `FontSettings::Source(CSS string)` unless precise control needed.
    - Security: Packages declare semantic ligature *policy* only; concrete families/sizes stay user-owned (`typography-role-ownership.md`).
  - Approach:
    - Documentation Reviewed:
      - parley 0.6 `src/style/mod.rs` (`StyleProperty::FontFeatures/FontVariations`), `src/style/font.rs` (`FontFeature = swash::Setting<u16>`), `src/shape/mod.rs` (consumes `style.font_features`); swash `src/setting.rs` (`{tag, value}`).
      - `docs/reference/primitives/typography.md`, `.agents/skills/project-patterns/references/typography-role-ownership.md`.
      - `jakubkrehel/better-typography` UI skill loaded in task 2 (OpenType `font-feature-settings` guidance: `liga`/`clig`, `calt`, stylistic sets `ss0X`, character variants `cv0X`, `zero`, `onum`).
    - Options Considered:
      - Global boolean (A) vs. per-role `LigaturePolicy` (B) vs. per-range span features (C). Per proposal §7.4: B (C deferred).
    - Chosen Approach:
      - Extend `FontProfile` with `LigaturePolicy`; resolve to `FontSettings`; push into parley; key layout cache on features.
    - API Notes and Examples:
      ```rust
      pub struct LigaturePolicy { pub enable_standard: bool, pub enable_contextual: bool,
          pub discretionary_features: Vec<String>, pub raw_features: Option<String>,
          pub disable_features: Vec<String> }
      // layout.rs::rebuild
      builder.push_default(StyleProperty::FontFeatures(profile.font_features()));
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: `LigaturePolicy` in `FontProfile`; defaults in `ActiveTypography::default`.
      - `src/server/ops/typography.rs`: `parse_profile` parses `ligatures`.
      - `src/editor/typography.rs`: resolve `FontSettings` per profile.
      - `src/editor/layout.rs`: push `FontFeatures`; extend `LayoutCacheKey`.
      - `src/packages/manifest.rs`: per-mode ligature policy on `buildCodeEditingManifest`/prose modes.
    - References:
      - `src/protocol/mod.rs:787/845`, `src/server/ops/typography.rs:104`, `src/editor/typography.rs:187`, `src/editor/layout.rs::rebuild`.
  - Test Cases to Write:
    - Toggle: a layout with `liga on` vs `liga off` yields different glyph/cluster counts (assert via parley `is_ligature_start`).
    - CSS source: `raw_features = "'calt' 1, 'liga' 0"` disables `liga` only.
    - Cache: changing `ligatures` invalidates the layout cache (no stale glyphs).
    - Per-mode: markdown vs code mode resolve different policies from `init.js`/manifest.

  ### Task 7 Implementation Note (completed 2026-07-31)

  **Wire type (`protocol/mod.rs`, rkyv):** `LigaturePolicy { enable_standard, enable_contextual, discretionary_features: Vec<String>, raw_features: Option<String>, disable_features: Vec<String> }` with a manual `Default` (both enables `true` — reproduces the implicit ligature-on shaping Clay relied on; `#[derive(Default)]` would have wrongly defaulted the bools to `false`). Added to `FontProfile` as **`ligatures: Box<LigaturePolicy>`** — boxed deliberately. Measured: `ArchivedServerMessage` union floor is 128 B and `ArchivedActiveTypography` was 72 B (56 B headroom); an unboxed `LigaturePolicy` (3× ~40 B) would have driven the floor past 128 and broken the `EditAck` ≤ 128 B budget. Boxing the field adds only 3× 8 B = 24 B → `ArchivedActiveTypography` 96 B ≤ 128, so small payloads stay at 128 B (verified: `representative_protocol_payloads_fit_phase14_budgets` passes). Same root-cause pattern as the task-4 `BehaviorManifest` boxing, localized to the field this time (no `PROTOCOL_VERSION` bump, no codec/match changes).

  **Trust-boundary validation:** `LigaturePolicy::validate` (called from `FontProfile::validate`) caps `discretionary_features`/`disable_features` at `MAX_LIGATURE_FEATURES_PER_KIND=32`, `raw_features` at `MAX_LIGATURE_RAW_FEATURE_BYTES=256`, and requires each tag to be 1..=4 ASCII bytes (real OpenType tag shape). New `FontProfileValidationError` variants: `TooManyDiscretionaryFeatures`, `TooManyDisabledFeatures`, `RawFeaturesTooLong`, `InvalidFeatureName`.

  **Server parse (`server/ops/typography.rs`):** `parse_profile` now requires `families`+`size` and **deny-by-default** rejects unknown profile keys (only `ligatures` extra allowed); `parse_ligature_policy` rejects unknown ligature keys. Refactored the old exact-count `require_only_keys` into `require_keys` (all-required-present) + `reject_unknown_keys` (no-unknown) so the top-level object can carry the optional `hierarchy` and profiles the optional `ligatures` without the brittle `object.len()` check. `setTypography` without `ligatures` keeps the default (ligatures on); `init.js` disables `calt` via `{ligatures:{disableFeatures:["calt"]}}`.

  **Client resolve (`editor/typography.rs`):** `ResolvedFontProfile` gains `features: Vec<FontFeature>` resolved once in `from_wire` by `resolve_font_features(policy)`: a `BTreeMap<Tag, u16>` builds a tag-sorted, last-declared-wins feature list (`liga`/`clig`/`calt` from the semantic toggles, `raw_features` parsed via `swash::Setting::parse_list`, `disable_features` applied **last** so it overrides). `font_features() -> FontSettings<'_, FontFeature>` returns `FontSettings::List(Cow::Borrowed(&self.features))` (precise control; `Source(CSS)` deferred — duplicate-tag precedence needed precise ordering).

  **Layout (`editor/layout.rs`):** `rebuild` + the test builder now `push_default`/`push` `StyleProperty::FontFeatures(profile.font_features())` for the default role and every style run. `LayoutCacheKey` gains `ligature_hash: u64` + a `with_ligatures(hash)` builder; `EditorSurface` sets it from `typography.profile(document_font_role).feature_hash()` (FNV-1a over the resolved feature list). `ponytail:` the hash currently co-varies with `typography_revision` (ligatures are `FontProfile`-owned, so a revision bump already invalidates) — kept explicit so the cache invariant is self-documenting and a future per-mode override would not need to re-architect the key.

  **Per-mode route:** satisfied via the **typography override** clause of the acceptance ("per-mode policy via `EditorBehaviorRules.ligatures` **(or typography override)**"). A mode selects its `document_font_role` (manifest), and each role's `FontProfile.ligatures` is independently user-owned, so markdown (proportional) and code (monospace) resolve different feature lists. No `EditorBehaviorRules.ligatures` field was added (the "or" makes it optional); package manifest-side ligature override is a Further Action if a package needs to override the user's per-role policy without touching typography (would thread an effective policy through `paint_text`→`rebuild` and make the hash non-redundant). Security respected: packages declare semantic policy only; concrete families/sizes stay user-owned (`typography-role-ownership`).

  **Tests added (13):** 6 `editor::typography` (`resolve_font_features` default-on, disable-standard→liga 0, disable-list overrides enable, raw-source disables liga only, discretionary tags, `feature_hash` differs on/off, per-role independent resolution) + 1 `editor::layout` (cache invalidates on ligature-hash change, stays valid when unchanged) + 6 `typography_protocol` validation (default-on, too-many-discretionary, too-many-disabled, raw-too-long, invalid-feature-name, disable-overriding-enable valid) + 4 `server::ops::typography` parse (optional ligatures absent, parses all fields, rejects unknown ligature key, rejects unknown top-level key). 1199 lib tests + 155 editor-suite tests pass; only the pre-existing `primitives_docs::plan061` op-inventory baseline fails (deferred to tasks 13/17).

  **Ceilings (`ponytail:` in source):** (1) `BlinkStyle`-style alpha-fade N/A here; feature resolution is exact. (2) `FontSettings::Source(CSS)` deferred in favor of `List` for precise duplicate-tag precedence. (3) `ligature_hash` is currently redundant with `typography_revision` (documented in the struct). (4) No per-mode `EditorBehaviorRules.ligatures` override (typography route used per the acceptance's "or"); add if a package must override the user's per-role ligature policy without a typography change.

- [x] 8. Multi-cursor: refactor `SelectionState` → `Vec<Selection>` + primary index (E.4 part 1) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `SelectionState` becomes a selection set with a primary index; every caller (insert/delete, copy/cut, undo/redo, search, IME, decoration tracking, paint) updated; existing single-selection behavior is preserved bit-for-bit when the set has one element.
    - Performance: Paint/insert/delete are O(selections) and viewport-bounded; no allocation growth for the single-selection case.
    - Code Quality: Single source of truth for the selection set; `cursorUndo` snapshots the set.
    - Security: No authority change (client view state).
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-editor.md`, `docs/reference/primitives/registry.md`.
    - Options Considered:
      - Keep single selection + parallel multi-cursor state vs. unified `Vec<Selection>`. Unified avoids divergence and matches VSCode/Helix.
    - Chosen Approach:
      - Unified `Vec<Selection>` + primary index; update all call sites in one task; gate behind the E.4 regression suite before adding multi-cursor commands.
    - API Notes and Examples:
      ```rust
      pub struct SelectionState { pub selections: Vec<Selection>, pub primary: usize, ... }
      ```
    - Files to Create/Edit:
      - `src/editor/selection.rs`: `Vec<Selection>` model + primary index.
      - `src/editor/surface.rs`, `cursor.rs`, `buffer.rs`: all selection readers/writers.
      - `src/masonry_editor.rs`: paint + IME + clipboard + undo paths.
    - References:
      - `src/editor/selection.rs`, `surface.rs` (`paint_caret`/`paint_selection`, `extend_selection`), `src/masonry_editor.rs`.
  - Test Cases to Write:
    - Single-selection parity: all existing editor tests pass unchanged.
    - Multi paint: two selections both render; primary blinks, secondary solid.
    - IME: preedit attaches to the primary caret only.

  ### Task 8 Implementation Note (completed 2026-07-31)

  **Unified model (`editor/selection.rs`):** replaced the legacy pair of a single `CursorState` plus an optional `SelectionState { anchor, focus }` with one store: `SelectionState { selections: Vec<Selection>, primary: usize }` (non-empty invariant; a collapsed primary selection means "no range", preserving the old `Option::None` bit-for-bit). `Selection { anchor, cursor: CursorState }` embeds the cursor so the focus (caret) and `preferred_x` live inside the selection — the existing `CursorState::move_to_*` API drives the focus unchanged (movement closures borrow `selections.primary_mut().cursor_mut()`), so no movement code was rewritten. `Selection: Copy` (both halves trivially copyable); `SelectionState: Clone + Default + PartialEq` (no `Eq` — `CursorState` carries `Option<f32>` preferred_x; nothing keys on it).

  **EditorSurface field swap:** `cursor: CursorState` + `selection: Option<SelectionState>` → `selections: SelectionState`. Added thin accessors so the ~60 call sites stayed mechanical: `caret()` (primary focus), `set_primary_focus(focus)` (mirrors `CursorState::set_caret` — sets focus, clears preferred_x, leaves anchor), `clear_selection()` (collapse anchor:=focus, replaces `self.selection = None`), `has_selection()` (primary not collapsed, replaces `is_some()`). The global `self.cursor.caret()` → `self.caret()` and `self.cursor.set_caret(` → `self.set_primary_focus(` were sed-applied (only those two `CursorState` methods were ever used); every `self.selection = None` became `clear_selection()` and every `self.selection = Some(SelectionState::new(a,f).clamped())` became explicit anchor+focus with a collapse-when-equal branch.

  **Bit-for-bit single-selection:** `move_cursor`/`extend_selection` borrow the primary cursor for the movement, then collapse or clamp the anchor — exactly the old `set_caret` + `selection = None` / `selection = Some` semantics. `extend_selection`'s anchor now reads `primary_anchor()` (which equals the focus when collapsed, so the old `map_or_else(caret, anchor)` collapses to a single read). `install_runtime_typography` snapshots `(caret, has_selection, primary_anchor)` and restores via `set_primary_focus` + anchor (clearing preferred_x, matching the old `set_caret` quirk). `capture_history_selection`/`restore_history_selection` (edit-undo) snapshot the PRIMARY caret+anchor; for one selection this is identical to before. Edit-undo stays single-cursor; task 9's `cursorUndo` (cursor-movement undo, a separate history) will snapshot the full set — the unified store is the single source of truth that enables it.

  **Multi-cursor paint (data model + paint):** `visible_selection_range` (one `Option<Range>`) → `visible_selection_ranges` (a `Vec<Range>` over every non-collapsed selection). `layout.rs::paint_text` signature changed `selection_visible_byte_range: Option<Range>` → `selection_visible_byte_ranges: &[Range]`, iterating fills (clone per range — `Range` is not `Copy`). `paint_caret` rewritten to iterate `selections.selections()`: the primary caret honours the blink cycle, secondary carets stay solid (so every cursor stays visible while typing with multiple selections); the gate is a testable `caret_should_paint(index)` helper. `caret_visible_offset` / `visible_caret_offset` extracted a `visible_byte_offset(byte, snapshot)` helper reused per selection. IME `paint_preedit_overlay` + `ime_cursor_area` keep using the PRIMARY caret geometry (preedit attaches to the primary caret only).

  **Files touched (4):** `editor/selection.rs` (rewritten: `Selection` + set-shaped `SelectionState` + 8 tests), `editor/surface.rs` (field swap, ~60 mechanical call-site rewrites, `visible_selection_ranges`/`visible_byte_offset`/`paint_caret` iterate/`caret_should_paint` + 3 acceptance tests), `editor/layout.rs` (`paint_text` takes `&[Range]`, iterates fills), `editor/cursor.rs` (unchanged — movement API reused).

  **Tests:** 8 `selection` unit tests (selection normalize/collapsed/set-focus clears preferred_x/set-anchor preserves focus+preferred_x/SelectionState default/has-selection/collapse-primary) + 3 `editor::surface` acceptance tests (`multi_selection_paint_data_renders_both_ranges` — two selections feed 2 visible ranges; `multi_caret_paint_gates_primary_on_blink_secondary_solid` — primary hides on blink-off, secondary solid, reset restores; `ime_preedit_attaches_to_primary_caret_only` — `visible_caret_offset` returns the primary focus with a secondary present). All 1207 lib tests + 155 editor-suite tests + 139 protocol tests pass (only the pre-existing `primitives_docs::plan061` op-inventory baseline fails, deferred to tasks 13/17). Single-selection parity confirmed: every pre-existing editor test passes unchanged.

  **Ceilings (`ponytail:` in source):** (1) `selection_count`/`push_selection` are `#[allow(dead_code)]` until task 9 (multi-cursor commands) consumes them. (2) Edit-undo (`HistorySelection`) snapshots the primary only — bit-for-bit for one selection; task 9's `cursorUndo` will snapshot the full set. (3) No `Selection::clamped`/`Selection::cursor` immutable accessor — removed with the legacy `.clamped()` call sites; task 9 re-adds if it needs to clamp a pushed selection in place. (4) `Range` clone in the layout paint loop (`Range` not `Copy`) — negligible for a handful of selections.

- [x] 9. Multi-cursor commands (E.4 part 2) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: Allowlist + implement `clay.editor.clientAddCursor` (above/below), `clientSelectNextMatch`/`clientSelectPrevMatch`, `clientSelectAllMatches`, `clientColumnSelect`, `clientCancelMultipleSelections`, `clientKeepSelection`, `clientRemoveSelection`, `clientUndoCursorMove`; default keys `Ctrl+Alt+Down/Up`, `Ctrl+D`, `Ctrl+Shift+L`, `Shift+Alt+Down/Up/Left/Right`, `Escape`, `Ctrl+U`. Rebinding works.
    - Performance: `select_next_match` search is bounded + cancellable; column select is O(lines in column).
    - Code Quality: Commands operate on the selection set generically; no per-command paint duplication.
    - Security: Client-local view state; no authority.
  - Approach:
    - Documentation Reviewed:
      - VSCode `cursorColumnSelect*`/`insertCursorBelow`/`addSelectionToNextFindMatch`/`selectHighlights`; Helix `copy_selection_on_next_line`/`keep_primary_selection`.
    - Options Considered:
      - Match-next over document text vs. over search results. Document-text match (VSCode `Ctrl+D`) is the default; search-driven is a later refinement.
    - Chosen Approach:
      - Document-text match-next; column select as N carets across visual lines; allowlisted `ClientUiCommand` ops.
    - API Notes and Examples:
      ```ts
      clientAddCursor({ direction: "below" });
      clientSelectNextMatch({}); clientSelectAllMatches({});
      clientColumnSelect({ direction: "down" });
      ```
    - Files to Create/Edit:
      - `src/server/ops/editor.rs`: the 9 ops + validation + allowlist + routing.
      - `src/editor/surface.rs`/`selection.rs`: set operations.
      - `src/masonry_editor.rs`: dispatch + default keys.
      - `runtime/js/editor.js`: facades.
      - `docs/reference/clay-js-api/editor/*.md`: 9 new API docs.
    - References:
      - `src/server/ops/keybindings.rs`, `src/masonry_editor.rs`.
  - Test Cases to Write:
    - Add cursor below: two carets; typing inserts at both.
    - Select-next-match: `Ctrl+D` on a word selects the next occurrence; loops/wraps behavior defined; `Escape` collapses.
    - Select-all-matches: all occurrences selected; copy copies union.
    - Column select: `Shift+Alt+Down` creates a box; left/right moves all carets.
    - Cursor-undo: `Ctrl+U` restores the previous selection set.
    - Keep/remove primary: `clientKeepSelection`/`clientRemoveSelection` behave per Helix.

  ### Task 9 Implementation Note (completed 2026-07-31)

  **Commands + set operations (`editor/surface.rs`):** 9 new `EditorCommand` variants (`AddCursor`, `ColumnSelect`, `SelectNextMatch`, `SelectPrevMatch`, `SelectAllMatches`, `CancelMultipleSelections`, `KeepSelection`, `RemoveSelection`, `UndoCursorMove`) all dispatch through one generic selection-set store; no command carries its own paint path (task 8's paint already iterates the set). `SelectionState` gained the set primitives (`push_and_make_primary`, `set_selections`, `keep_only_primary`, `remove_primary`, `clamp_to`, `selection_mut`); `Selection::collapsed` builds bare carets.

  **Selection semantics:** `select_next_match` picks the primary range text (or the word at a collapsed caret) as the needle; first press on a collapsed caret selects the word, later presses add the next/prev unselected occurrence with one bounded `match_indices` scan that wraps once and stops when every occurrence is selected. `select_all_matches` replaces the set with every occurrence (primary = the occurrence containing the original caret). `add_cursor_line` places a caret at the same scalar column on the adjacent line (clamped to line end) and refuses to stack two carets on one line; column-select down/up share it, column-select left/right move every caret one scalar. `cancel_multiple_selections` collapses to the primary caret; `keep_selection`/`remove_selection` follow Helix.

  **Cursor-undo (`UndoCursorMove`, Ctrl+U):** a bounded `VecDeque<SelectionState>` snapshots the set before every caret-moving/selection-reshaping command (`EditorCommand::is_selection_changing`), dedups identical neighbours, and restores on demand (clamped to the buffer, since snapshots may predate edits). Edits keep their own history; the stack clears on `load_snapshot`.

  **Multi-cursor edits:** `multi_caret_edit` applies one operation per caret right-to-left (byte offsets stay valid), records ONE combined `HistoryEntry` (new `forward_ops`/`inverse_ops`/`selection_set_*`/`primary_index` fields; single-caret entries keep the legacy fields) and emits one edit event per caret via the new `EditorCommandOutcome.edit_events` (the connection layer stamps each with an ascending optimistic base version, so the server applies them in order). Undo replays the inverse ops in reverse of forward order; redo replays forward ops as stored. Insert/newline/backspace/delete/paste branch into it when more than one selection exists; single-cursor paths are bit-for-bit unchanged. Copy (`selected_text`) unions every range in document order; cut deletes every range.

  **Wiring:** default keys in `on_text_event` (Ctrl+Alt+Down/Up add-cursor, Shift+Alt+arrows column-select, Ctrl+D select-next-match, Ctrl+Shift+L select-all, Ctrl+U cursor-undo) + Escape collapses the set in `route_key_with_event` (after menu/snippet, before manifest routing). 13 command IDs allowlisted in `keybindings.rs` (`is_runtime_bindable_command` + `ClientUiCommand` routing), mapped in `EditorClientCommand::from_command_id` and dispatched by `apply_editor_client_command` (rebinding works, tested). New ops `op_clay_editor_add_cursor`/`op_clay_editor_column_select` (deny-by-default direction validation) registered trusted-only; the 7 argless facades return stable command-ID strings. JS facades + `.d.ts` + 9 API docs + `api-inventory.toml` + `docs/index.md` + regenerated registry; trusted op count 72→74.

  **Tests:** 11 `editor::surface` acceptance tests (add-cursor-below typing inserts at both; next/prev match wrap; select-all + copy union; column box + move-all; Escape collapse; keep/remove per Helix; cursor-undo restores the set; multi-caret typing undoes/redoes as one step; multi-caret backspace), 6 `selection` unit tests, 4 op-validation tests, extended `EditorClientCommand` mapping/dispatch tests, and a rebound multi-cursor route test. Full gate: clippy `-D warnings` clean, fmt clean, 1229 lib + 155 editor-suite + 139 protocol tests pass (only the pre-existing `primitives_docs::plan061` op-inventory baseline fails, deferred to tasks 13/17). Ctrl+D default moved task-5's SelectWord binding to SelectNextMatch (VSCode parity; SelectWord stays reachable via its command ID).

  **Ceilings (`ponytail:` in source):** (1) overlapping selections apply as-is (callers build non-overlapping sets); (2) snippet sessions are single-caret and drop on a multi edit; (3) Escape now has two documented bindings (`clay.application.quit` app-level fallback + editor cancel) — the editor consumes it first; (4) IME commit inserts at every caret (preedit overlay stays primary-only); (5) cut with mixed collapsed/ranged carets also deletes the char after collapsed carets.

- [x] 10. Tree-sitter text objects + smart select (E.5) ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `clay.editor.clientSelectTextobject` (`{object, around, direction}`) and `clay.editor.clientSmartSelect` (`{action: expand|shrink}`) query the document's syntax tree via the generic `src/server/syntax.rs` runner and apply ranges as selections; multi-cursor-aware (grows the set across carets); `textobjects.scm` shipped for ≥1 built-in language; smart-select expand walks the tree parent chain, shrink reverses. No language-specific Rust.
    - Performance: One bounded server query per command (reuses parsed tree); cancellable; not on the typing hot path.
    - Code Quality: Generic primitive + per-package query files (package-provided-grammar pattern); invalid/missing query falls back gracefully.
    - Security: Read-only query; no mutation, no external process, no native artifact loading.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/parse-coordinator.md`, `decoration-transport.md`, `low-latency-incremental-syntax-decoration-primitive-review.md`; Helix `textobjects.html` (`@textobject` captures).
    - Options Considered:
      - Server computes ranges vs. client computes. Server has the parsed tree + validated grammar; client applies inert ranges (preserves server authority + hot-path invariants).
    - Chosen Approach:
      - Server-side `op_clay_editor_select_textobject`/`op_clay_editor_smart_select` run the active grammar's `textobjects.scm` via `QueryCursor` and return ranges; client applies them as selections; built-in packages ship query files.
    - API Notes and Examples:
      ```scheme
      ;; packages/rust/queries/textobjects.scm
      (function_item) @textobject.function.around
      (block) @textobject.function.inner
      (arguments) @textobject.argument.around
      ```
      ```ts
      clientSelectTextobject({ object: "function", around: false, direction: "current" });
      clientSmartSelect({ action: "expand" });
      ```
    - Files to Create/Edit:
      - `packages/{rust,typescript,javascript,markdown}/queries/textobjects.scm`: object captures.
      - `src/server/syntax.rs`: `textobjects_query` descriptor + `run_textobject_query`/`run_smart_select`.
      - `src/server/ops/editor.rs`: `op_clay_editor_select_textobject`/`op_clay_editor_smart_select` + allowlist + routing (`ServerFirst` read-only, results applied client-side).
      - `src/masonry_editor.rs`: apply returned ranges as selections (multi-cursor-aware).
      - `runtime/js/editor.js`: facades; `docs/reference/clay-js-api/editor/*.md`: 2 new API docs.
    - References:
      - `src/server/syntax.rs`, `src/packages/record.rs`, `packages/*/queries/`.
  - Test Cases to Write:
    - Built-in language `textobjects.scm` parses; inner/around function/class/argument/comment ranges correct at known offsets.
    - Smart-select expand walks parent chain; shrink reverses.
    - Multi-cursor: textobject grows the set across all carets.
    - Fallback: a document with no grammar/invalid query returns no ranges without panicking.
    - `init.js` binding `Ctrl+Shift+\` (expand) / `Ctrl+Shift+Alt+\` (shrink) and `]f`/`[f` (next/prev function, package-bound) works.

  ### Task 10 Implementation Note (completed 2026-08-03)

  **Architecture (chosen: server computes, client applies, mirroring `LanguageIntelligenceRequest`):** new wire pair `ClientMessage::SelectionQueryRequest` / `ServerMessage::SelectionQueryResult` (`src/protocol/textobjects.rs`, PROTOCOL_VERSION 6→7). Key → `UiReactivePriority` route (`SelectionQuery::from_command_id` in `keybindings.rs` allowlist+routing) → widget captures the whole selection set + versions (`EditorSurface::selection_query_request_for`) → one bounded server round trip → `EditorWidget::apply_selection_query_result` installs ranges. No document mutation crosses the boundary; read-only by construction.

  **Server query runner (`src/server/syntax.rs`):** `NativeGrammarDescriptor` gained `textobjects_query[_path]`; `TreeSitterSyntaxHandler::enable_textobjects` compiles fail-closed; `selection_query_ranges` reuses the cached full-document tree at the same version, else one timeout-bounded fresh parse (native descriptors parse full-file context, so cache hits are the norm). Textobject matching is capture-name-driven (`textobject.<kind>.<inner|around>`, Helix-style) — zero language-specific Rust: kinds `function/class/argument/comment/loop/conditional/call/statement`, directions `current` (innermost containing, smallest), `next` (earliest strictly after), `previous` (latest ending ≤ caret), no wrap; `inner` falls back to `around` when the grammar defines none. Smart select needs no query file: expand walks `descendant_for_byte_range` up the parent chain to the first strictly-larger range; shrink picks the largest node range strictly inside the selection. `ParseHandler` trait gained `selection_query_ranges` (default `None` for JS handlers) + `ParseCoordinator::handler_for`; the connection arm resolves the native handler via `registered_native_syntax_handler` and degrades every miss (validation/no grammar/no handler/parse timeout) to empty ranges instead of an error.

  **Query files shipped:** `packages/rust/queries/textobjects.scm`, `packages/typescript/queries/textobjects.scm` (shared by TS+TSX), `packages/javascript/queries/textobjects.scm`. Markdown ships none (no meaningful function/class/argument objects); smart select still works off its block tree. Command-ID surface: 48 generated `clay.editor.clientSelectTextobject.<kind>.<inner|around>[.next|.previous]` + `clay.editor.clientSmartSelect.expand|.shrink`, parsed programmatically (no 50-entry enumeration); `bindKey` auto-declares them via the existing `command_for_rule` path. New ops `op_clay_editor_select_textobject`/`op_clay_editor_smart_select` (deny-by-default validation, trusted-only; count 74→76) return the direction/action-specific command ID for binding.

  **Client application:** `SelectionQueryResult.ranges` aligns index-for-index with the request's cursors; matched carets take their range (input direction preserved), unmatched keep their selection, stale results (replaced request or moved document version) drop silently. Primary index preserved; `apply_selection_query_result` snapshots the set first so cursor-undo (Ctrl+U) restores the pre-query state. Multi-cursor: every selection queries/applies independently.

  **Tests:** 5 protocol command-ID round-trip/deny tests; 5 syntax runner tests (all shipped queries compile against their grammars; Rust function inner/around/next/previous at exact offsets; comment inner→around fallback; expand strictly grows to full doc then None + shrink reverses + collapsed-shrink no-op; markdown degrades to `None` with working smart select); 4 op validation tests; bindable/routing allowlist tests (unknown kind/scope/direction rejected); surface routing test (rebound `clientSmartSelect.expand` routes `UiReactivePriority` ServerIntent, no mutation) + request-capture/apply/cursor-undo test; widget apply test (direction preserved, unmatched kept, stale dropped, orphan ignored). Gate: clippy `-D warnings` 0, fmt clean, 1246 lib + 155 editor + 139 protocol tests pass (only pre-existing `primitives_docs::plan061` fails, deferred to tasks 13/17). Docs: 2 new API docs + inventory + index + regenerated registry.

  **Ceilings/notes:** (1) `]f`/`[f` two-stroke chords are not runtime-backed yet — `parse_key_chord` rejects multi-stroke sequences (pre-existing limit; single-chord bindings like `Ctrl+]` work today). (2) Default `init.js` key bindings (`Ctrl+Shift+\` etc.) ride task 11's package default init.js loading; the IDs are bindable now. (3) Package-provided (WASM) grammars cannot register textobject queries yet — native first-party only; the trait default keeps JS handlers graceful. (4) Shrink is stateless (largest strict sub-node), not an expansion-history stack. (5) Request cursors capped at `MAX_SELECTION_QUERY_CURSORS` (256).

- [x] 11. Define and verify the package default `init.js` loading experience ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `core.code`/`core.text` built-in fallback modes ship sensible default `MovementRules`/`caret_style`/`ligatures` without an owning package; first-party language packages (`@clay/rust`, `@clay/typescript`, `@clay/javascript`, `@clay/markdown`) set mode-appropriate values via manifest; a one-line `loadPackage("@clay/markdown")` (or equivalent) yields correct prose movement/ligatures/caret with no copied manifests or low-level plumbing.
    - Performance: Defaults resolve at mode/theme install time; no per-keystroke work.
    - Code Quality: No silent behavior-changing default from a package load; package customization optional and documented.
    - Security: Loading a package grants no new authority via these manifest fields.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-distribution.md`, `mode-primitive-first.md` (built-in fallback modes); `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`.
    - Options Considered:
      - Force a global movement/caret/ligature default vs. per-mode manifest data. Per-mode manifest data (with sensible built-in `core.*` defaults) preserves mode-configurability.
    - Chosen Approach:
      - `core.*` built-ins ship defaults; language packages override via manifest; verify one-line load works and customization is optional.
    - API Notes and Examples:
      ```ts
      loadPackage("@clay/markdown"); // prose movement + ligatures + bar caret via manifest, no extra config
      ```
    - Files to Create/Edit:
      - `packages/{rust,typescript,javascript,markdown}/` manifest/`init` (or equivalent) if they opt into non-default `MovementRules`/`caret_style`/`ligatures`.
      - `docs/reference/packages/creating-packages.md`: one-line load + customization example.
    - References:
      - `src/packages/manifest.rs` (`buildCodeEditingManifest`), built-in fallback mode registration.
  - Test Cases to Write:
    - Default load: after `loadPackage("@clay/markdown")`, a `.md` file uses prose word separators + ligatures per the markdown manifest.
    - No-silent-default: a package load does not change movement/caret/ligature for unrelated document types.
    - Customization: `init.js` can override a mode's `caret_style`/`ligatures` via documented APIs.

  ### Task 11 Implementation Note (completed 2026-08-03)

  **Loading experience (per approved decision 2026-06-09-0219):** packages are explicitly opt-in from `~/.config/clay/init.js` — auto-loading first-party packages without init.js was rejected. The one-line default `await loadPackage("@clay/markdown")` already existed (resolver → enable → `loadEntry` execution, connection test `default_init_js_load_package_powers_selected_markdown_open` proves open-time activation end to end). Task 11's delta is what that one line now yields for movement/caret/ligatures.

  **Built-in fallback defaults (no owning package):** `core.code`/`core.text` ship `MovementRules` via `EditorBehaviorRules::default_code()`/`default_text()` (tasks 4), caret defers to the editor `StyleRegistry` default bar (`caret_style: None`, task 6), and ligatures ship per font role in the typography baseline (`ActiveTypography::default()` carries `LigaturePolicy::default()` — standard+contextual on — for monospace/proportional/ui, task 7). core.code selects Monospace, core.text selects Proportional, so each fallback role resolves ligatures at install time. No code change was needed here — verification only.

  **Package manifest values:** `buildCodeEditingManifest` (runtime/js/behavior.js + .d.ts) gained optional `movement`/`caretStyle` pass-through (plain objects only; server-side `parse_movement_rules`/`parse_caret_style` own field-by-field validation and fallback). `@clay/markdown` declares prose movement (`wordSeparators: "prose"`, `treatUnderscoreAsWord: false`, `camelCaseSubWord: false`); `@clay/rust`/`@clay/typescript`/`@clay/javascript` declare explicit code movement (identical to the built-in default — intent documentation, zero behaviour change). No package ships a caret override (customization is opt-in). Ligatures stay typography-owned per the task-7 architecture decision: a mode's `defaultFontRole` selects the `FontProfile` whose `ligatures` policy applies (markdown → proportional), users customize per role via `setTypography`; packages never set ligatures directly and no new authority is granted by any of these manifest fields (pure validated data).

  **Tests:** `each_language_mode_registers_indent_electric_pairs_comment_triggers` extended with per-package movement/caret assertions (prose vs code policy, caret None) incl. manifest payload budgets; new `markdown_load_yields_prose_movement_without_touching_code_defaults` (one-line `loadPackage("@clay/markdown")` → `.md` classifies markdown, `src/main.rs`/`notes` still resolve to `core.code`/`core.text`; built-in fallback manifests carry code movement + default caret + role ligature baseline — no silent cross-mode change); new `package_manifest_can_customize_movement_and_caret_style` (trusted synthetic package registers+activates a mode with `editorRules.movement` + `editorRules.caretStyle` overrides via documented facades; absent fields keep defaults). Docs: `creating-packages.md` — one-line-load outcome paragraph, editorRules example + Behavior-manifest bullets for movement/caretStyle/ligatures.

  **Gate:** clippy `-D warnings` 0, fmt clean, 1248 lib + 155 editor + 139 protocol tests pass (only pre-existing `primitives_docs::plan061` fails, deferred to tasks 13/17). Package `package.json` fingerprints unchanged (dist-only edits), so the bundled-trust inventory is unaffected.

  **Notes/ceilings:** trust-domain separation for package init/load entries (two runtime domains, third-party fail-closed `loadPackage`) is existing Plan 061 infrastructure, preserved and re-verified by task 15. Broken-package-init diagnostics already exist (package-scoped `ClayRuntimeError` → `RuntimeDiagnostic`, editor continues) and were not re-built here.

- [x] 12. Update the package UI/layout authoring contract and package guide ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` documents the new inert manifest fields (`MovementRules`, `caret_style`, `ligatures`), the text-object grammar contribution (`queries/textobjects.scm`), APIs, examples, limitations, permissions, and testing guidance; `.agents/skills/clay-ui/references/{components,tokens}.md` updated if any token/component changed.
    - Performance: Documented fields are inert (no client JS on the hot path).
    - Code Quality: Manifest field schema documented; packages declare semantic values only (no concrete families/sizes/raw colors).
    - Security: Permissions/authority notes state these fields grant no file/network/shell/AI authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`, `typography-role-ownership.md`, `clay-js-api-schema.md`.
    - Options Considered:
      - Document inline in API docs vs. a consolidated authoring section. Consolidated authoring section in `creating-packages.md` + per-API docs (cross-linked) per the Phase 20.7 contract.
    - Chosen Approach:
      - Update `creating-packages.md` authoring contract + cross-link from per-API docs; update clay-ui references only if a token/component actually changed.
    - API Notes and Examples:
      ```text
      creating-packages.md § editor manifest: MovementRules, caret_style, ligatures
      creating-packages.md § grammar: queries/textobjects.scm schema
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`.
      - `.agents/skills/clay-ui/references/components.md`, `tokens.md` (only if a token/component changed).
    - References:
      - `docs/reference/packages/creating-packages.md`, `.agents/skills/clay-ui/`.
  - Test Cases to Write:
    - Doc-conformance: `tests/package_ui_conformance.rs` catalog-drift guards stay green; any new token is in the catalog and `tokens.md`.

  ### Task 12 Implementation Note (completed 2026-08-03)

  Docs-only task; no code or token/component changes. Three doc files updated:

  - `docs/reference/primitives/ui-chrome-primitives.md` — "Package authoring contract" gains an **editor chrome is not SDUI chrome** block: caret shape/blink are inert `editorRules.caretStyle` manifest data (omitted = editor default bar from `StyleRegistry`), caret **color** stays theme-owned (`caret` token), ligatures follow the mode's font role via the user-owned `FontProfile` policy; no `serverRegisterThemeToken`/`designTokens`/component surface accepts caret or ligature policy. Native rendering pointers recorded (`paint_caret`, parley `StyleProperty::FontFeatures`).
  - `docs/reference/primitives/typography.md` — new **Ligature Policy** section: per-role user-owned baseline, semantic toggles first (`enableStandard`→liga/clig, `enableContextual`→calt) with bounded tag-list/CSS escape hatches, default-when-absent behavior (default = standard+contextual on; disabling is explicit), package surface limited to role selection, layout-cache invalidation on policy change.
  - `docs/reference/packages/creating-packages.md` — (a) unified UI/layout authoring contract bullet: editor chrome vs SDUI, the two allowed contribution surfaces, no capability grants caret/ligature override authority, omitted fields fall back to built-in defaults (task 11's manifest bullets already state these fields grant no new authority); (b) grammar section gains **Text-object grammar contributions** (`queries/textobjects.scm`): capture schema `@textobject.<kind>.<scope>` (8 closed kinds, around/inner with inner→around fallback, directions are runtime command-ID concerns), native-descriptor-only contribution route (metadata `queries` accepts only `highlights`), same `parse-document` permission + package-root confinement + explicit no-file/network/shell/AI/WASM/client-JS authority note, advisory degrade semantics, auto-declared command IDs via `bindKey` prefix validation, smart-select parent-chain behavior, bindKey example, and testing/limitation guidance.

  **clay-ui skill references:** verified `components.md` (editorView chrome is editor-`StyleRegistry`-driven) and `tokens.md` (editor base color keys incl. `caret`) — both already consistent; no token/component changed so no edits needed per the "only if changed" criterion.

  **Verification:** all suites run explicitly (full `cargo test` aborts after the known protocol failure, hiding later binaries): 1248 lib + 34 main + 155 editor + 139/140 protocol + 196 runtime + 121/122 security. Doc-validation green: `package_loading_docs` 5, `primitives_docs` 22 (skipping deferred plan061), `clay_js_doc_registry` 38, `package_ui_conformance` catalog guards. Two pre-existing failures, both verified failing on clean HEAD with every plan-071 change stashed: `primitives_docs::plan061` inventory drift (tasks 13/17) and `security::package_loading::package_manifest_rejects_invalid_slot_ui_contribution_metadata` (raw-CSS style rejection — unrelated to this plan, no plan-071 code touches UI contribution validation). Anchor targets resolve against real headings.

  **Notes/ceilings:** third-party grammar metadata cannot declare textobjects today (native descriptors only) — documented as a limitation; promoting a metadata `textobjects` key is future work. `setTypography` API doc/inventory lack the `ligatures` custom properties — task 13 scope.

- [x] 13. Create or verify Clay JS APIs for public programmatic surfaces ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: Every new/changed public surface has a Clay JS API doc with stable ID, JS module/export/facade, backing Rust path, `deno_op`, `user_facing_name`, `key_bindings`, `custom_properties`, summary, owner, phase, visibility, permissions/security, agent guidance, lookup tags, app/help visibility; linked from `docs/index.md`; generated registry updated. APIs: `clientMoveCursor`, `clientSetSelection`, `clientSetCursorStyle`, `clientAddCursor`, `clientSelectNextMatch`, `clientSelectPrevMatch`, `clientSelectAllMatches`, `clientColumnSelect`, `clientCancelMultipleSelections`, `clientKeepSelection`, `clientRemoveSelection`, `clientUndoCursorMove`, `clientSelectTextobject`, `clientSmartSelect`; plus any `clay:behavior` helpers (`buildMovementRules`/`buildCaretStyle`/`buildLigaturePolicy`) if exposed.
    - Performance: Docs/registry generation adds no runtime work.
    - Code Quality: `clay-js-api-naming.md` applied (concise behavior-oriented callables; `client*`/`server*` authority markers; raw op names out of user-facing exports); `clay-js-api-schema.md` enforced.
    - Security: Each API doc states it grants no document mutation/external authority (movement/selection/caret client-local; textobject/smart-select read-only query).
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/editor/client-move-cursor.md` (existing planned doc format); `clay-js-api-naming.md`, `clay-js-api-schema.md`.
    - Options Considered:
      - Expose `buildMovementRules`/`buildCaretStyle`/`buildLigaturePolicy` as `clay:behavior` helpers vs. inline manifest objects. Helpers reduce boilerplate and match `buildCodeEditingManifest`; verify density warrants a module split per naming guidance.
    - Chosen Approach:
      - Update the 3 planned docs → implemented; add 11 new editor API docs; add `clay:behavior` helpers if justified; link all from `docs/index.md`; regenerate the registry.
    - API Notes and Examples:
      ```text
      id: clay.editor.clientSelectTextobject  | js_export: clientSelectTextobject
      user_facing_name: Select Text Object     | owner: client+server (read query)
      key_bindings: []  (package-bound)  custom_properties: [object, around, direction]
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/editor/*.md` (3 updated + 11 new).
      - `docs/reference/clay-js-api/behavior/*.md` (helpers, if added).
      - `docs/index.md`: links.
      - `runtime/js/editor.js`, `runtime/js/behavior.js`: facades.
      - Generated registry artifacts (via the project doc-registry command).
    - References:
      - `decision-logs/2026-05-08-1509-...`, `2026-05-08-1840-...`, `2026-05-08-1419-...`.
  - Test Cases to Write:
    - Coverage gate: `cargo test` fails if a required API doc, master-index link, registry entry, key-binding/custom-property field, or lookup entry is missing/stale (per `doc-registry-tests.md`).

- [x] 14. Create or verify Clay configuration APIs ✓ (Implementation Note below)

  ### Task 13 Implementation Note (completed 2026-08-03)

  Most API-doc work landed inline with tasks 5/6/9/10 (3 docs updated planned→implemented/runtime-backed, 11 new editor docs, 2 textobject docs, inventory entries, index links, registry regenerations). Task 13 closed the remaining gaps:

  **1. Changed-surface doc gap — `setTypography` ligatures (task 7).** `docs/reference/clay-js-api/theme/set-typography.md` gained the three `<role>.ligatures` custom properties (object|undefined, default `{ enableStandard: true, enableContextual: true }`, full field schema in description), updated summary/description (role-selects-policy ownership rule), and a ligature usage example. `runtime/js/theme.d.ts` `FontProfile` gained the missing optional `ligatures?: LigaturePolicy` type (parser already accepted the field; the public type surface lagged). `api-inventory.toml` custom_properties + registry regenerated — `clay_js_doc_registry`/`clay_js_api_inventory` all green (52 filtered tests).

  **2. Verification of all 14 required APIs.** Existence + `docs/index.md` links + registry freshness confirmed for `clientMoveCursor`, `clientSetSelection`, `clientSetCursorStyle`, `clientAddCursor`, `clientColumnSelect`, `clientSelectNextMatch`, `clientSelectPrevMatch`, `clientSelectAllMatches`, `clientCancelMultipleSelections`, `clientKeepSelection`, `clientRemoveSelection`, `clientUndoCursorMove`, `clientSelectTextobject`, `clientSmartSelect`. Naming/schema compliance is test-enforced (`every_public_api_contract_matches_generated_markdown_metadata` etc.) and passes.

  **3. `clay:behavior` helpers — deliberately NOT added (YAGNI).** `buildMovementRules`/`buildCaretStyle`/`buildLigaturePolicy` skipped: `movement`/`caretStyle` are inline plain objects in `editorRules` already passed through `buildCodeEditingManifest` (task 11) and validated field-by-field server-side (`parse_movement_rules`/`parse_caret_style`), so extra builder helpers add no validation or density; ligatures never flow through manifests (typography-route decision, task 7). Add helpers only if a real boilerplate pattern emerges across ≥3 packages.

  **4. Deferred plan-061 inventory rebaseline (per tasks 13/17 deferral).** `primitives_docs::plan061_runtime_package_authority_rebaseline_matches_source_inventory` was pre-existing-failing on clean HEAD (verified via stash). Fixed by rebaselining `plans/061-...md` marker sections to current source: op-inventory 68→76 (added the 7 Plan 071 `op_clay_editor_*` ops as Trusted-only with the task-15 third-party-access deferral note, plus pre-existing drift `op_clay_theme_set_appearance` as Configuration/admin-only) and package-inventory 11→14 (added `@clay/settings`, `@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi` rows with accurate module/permission usage); test counts updated to match. Facade inventory (21) unchanged — no new `clay:*` facades.

  **Gate:** clippy `-D warnings` 0, fmt clean; protocol suite now fully green 140/140 (was 139/140), runtime 196, editor 155, lib 1248. One remaining failure repo-wide: `security::package_manifest_rejects_invalid_slot_ui_contribution_metadata` — verified pre-existing on clean HEAD with all plan-071 changes stashed (UI contribution validation, unrelated surface); out of plan-071 scope.

  **Notes/ceilings:** third-party access to the editor ops stays trusted-only pending task 15; `op_clay_theme_set_appearance` was undocumented drift predating plan 071 — now inventoried (its API doc already existed).
  - Acceptance Criteria:
    - Functional: Per-mode `MovementRules`/`caret_style`/`ligatures` and runtime `clientSetCursorStyle`/ligature overrides are exposed as documented Clay JS APIs (not undocumented config keys); `~/.config/clay/init.js` is the entry point; examples cover per-mode and global configuration.
    - Performance: Config resolution at install time; no per-keystroke work.
    - Code Quality: Every config option listed in `custom_properties`; `clay-js-api-schema.md` enforced.
    - Security: Configuration grants no filesystem/network/shell/extension-loading/AI-mutation/workspace authority; ligature/caret config is typography/rendering only.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`; `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
    - Options Considered:
      - Undocumented settings JSON vs. documented Clay JS APIs. APIs only (per the configuration pattern).
    - Chosen Approach:
      - Config-as-API: `init.js` sets per-mode manifest fields via documented APIs; runtime overrides via `clientSetCursorStyle`/typography API; tests fail for undocumented behavior-changing settings.
    - API Notes and Examples:
      ```ts
      // ~/.config/clay/init.js
      loadPackage("@clay/markdown");
      bindKey("Ctrl+Right", clientMoveCursor({ direction: "nextWordStart" }));
      clientSetCursorStyle({ shape: "block", blink: "solid" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration/*.md` (if new config APIs) + cross-links.
      - `docs/index.md`.
    - References:
      - `decision-logs/2026-05-08-1841-...`.
  - Test Cases to Write:
    - Coverage gate: a behavior-changing setting without a `custom_properties` entry fails `cargo test`.

  ### Task 14 Implementation Note (completed 2026-08-03)

  Config-as-API verified; no new config APIs needed (all configuration flows through existing documented APIs). Gaps found and closed:

  **Doc gaps closed (changed-surface parity):**
  - `build-code-editing-manifest.md` + inventory: gained `movement` and `caretStyle` custom properties (full field schemas matching `behavior.d.ts`/`parse_movement_rules`/`parse_caret_style`), description + prose-mode usage note, explicit "ligatures are not configured here — typography route" pointer.
  - `server-register-mode-pattern.md` + inventory: gained `editorRules` custom property (was entirely missing despite the op parsing it at `modes.rs:172`); documents generic rule fields + movement/caretStyle, deny-by-default server validation.
  - `server-activate-major-mode.md` + inventory: gained `editorRules` custom property (activation-time override per the op doc comment).
  - `configuration.md` (config model doc): typography section + example now cover per-role ligatures; config-surface table gained rows for per-mode movement/caret (`editorRules` via serverRegisterModePattern) and runtime caret override (`clientSetCursorStyle` precedence over manifest values).
  - Registry regenerated; `clay_js` doc tests 52/52.

  **Verified already-correct surfaces:** `clientSetCursorStyle` doc ↔ `validate_set_cursor_style` field parity exact (shape/blink/widthPx/heightPct/hollow/stopBlinkOnTyping); `setTypography` ligatures documented in task 13; per-mode config examples in `creating-packages.md` (tasks 11/12); `init.js` remains the single entry point.

  **No-hidden-keys check:** scanned `src/server/configuration.rs` and `@clay/settings` dist for movement/caret/ligature keys — none; configuration changes behavior only through the documented APIs above (decision 2026-05-08-1841 contract preserved).

  **Coverage gate:** the existing `every_public_api_contract_matches_generated_markdown_metadata` test fails on any md↔toml custom-property-name drift plus missing required sections/registry staleness — that is the standing enforcement for "behavior-changing setting without a custom_properties entry". No new gate built (YAGNI: the surface is tested parity, not runtime introspection of op args).

  **Security wording:** every touched doc carries the no-filesystem/network/shell/extension-loading/AI/workspace authority statements; caret/ligature config is rendering-only inert data validated server-side.

  **Gate:** clippy 0, fmt clean; 1248 lib + 155 editor + 140 protocol + 196 runtime green; security 121/122 (known pre-existing failure, out of scope).

  **Notes/ceilings:** `serverActivateClassifiedMode` and `serverSelectDocumentManifest` remain undocumented stubs predating plan 071 (no plan-071 config flows through them); `commands`/`keymaps` passthrough on register/activate is also undocumented pre-existing surface — both out of task-14 scope.

- [x] 15. Preserve the two package runtime trust domains ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: New `ClientUiCommand` IDs and manifest fields grant no trusted-runtime authority; movement/selection/caret are client-local; `textobjects.scm` is a resolver-validated first-party grammar contribution (no third-party native artifact); text-object ops are read-only with no cross-domain V8 object/function/module passing (inert ranges only).
    - Performance: No process/IPC work added to the editor hot path.
    - Code Quality: No Clay-internal op exposed to adopted packages; no `clay:*` facade leaking conformance.
    - Security: Deny-by-default; tests prove cross-domain internal-op denial and stale-generation rejection for the new surfaces.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`, `package-security.md`, `authority-boundaries.md`; `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
    - Options Considered:
      - Dedicated task vs. fold into each impl task. Dedicated task gives one auditable trust-domain check + explicit tests.
    - Chosen Approach:
      - Dedicated verification task: audit new ops/fields/queries against the two-domain rules; add denial/stale-generation tests.
    - API Notes and Examples:
      ```text
      deny: third-party textobjects.scm native artifact loading; raw op access; AI mutation via caret/selection
      inert: textobject ranges (rkyv), caret_style/ligatures (rkyv manifest data)
      ```
    - Files to Create/Edit:
      - `src/server/ops/editor.rs`: validation/deny-by-default for new op args.
      - `src/packages/record.rs`: `textobjects.scm` is a resolver-validated first-party grammar contribution only.
      - Tests: cross-domain denial + stale-generation for new surfaces.
    - References:
      - `src/packages/record.rs`, `src/server/ops/keybindings.rs`, `src/server/syntax.rs`.
  - Test Cases to Write:
    - Cross-domain denial: an adopted package cannot call the new internal ops or pass V8 objects.
    - Stale generation: a text-object op under an old behavior version is rejected and republished.
    - No native artifact: a third-party `textobjects.scm` referencing native artifacts is rejected.

  ### Task 15 Implementation Note (completed 2026-08-03)

  Audit + tests. Two-domain rules hold for every Plan 071 surface; one hardening change made.

  **Audit findings (no code needed):**
  - All 7 new `op_clay_editor_*` ops registered in the trusted extension only; `clay:editor` facade already unresolvable in the third-party domain (existing facade-rejection loop). Movement/selection/caret stay client-local (ClientUiCommand dispatch) or trusted-op validated; no manifest field carries authority — `MovementRules`/`CaretStyle`/`LigaturePolicy` are inert rkyv data validated server-side deny-by-default.
  - Text-object ops take `#[string]` JSON and return JSON command descriptors; the query itself runs in pure Rust (`TreeSitterSyntaxHandler::selection_query_ranges`), results are inert `Option<Range>` bytes on the wire. No cross-domain V8 object/function/module passing is possible by construction.
  - JS parse handlers are the blanket closure impl of `ParseHandler`; they inherit the default `selection_query_ranges -> None`, so third-party handlers never participate in text-object queries and cannot inject V8 into the path.
  - Stale generation: selection queries hold no cross-request registration state — the connection loop re-resolves the native handler + parse handler from `runtime_generation.current()` per request, so a terminated/reloaded generation's handlers disappear immediately; client-side stale results are dropped on document_version mismatch (task-10 test).

  **Hardening change (deny-by-default):** `record.rs` grammar `queries` metadata previously ignored unknown keys silently. Now rejects any key outside {highlights, locals, injections} with a named-key error — a third-party (or first-party) manifest declaring `queries.textobjects` fails package record assembly instead of being silently dropped. Grammar contributions remain first-party-only (`@clay/` check, Phase 18.10), so the metadata path can never load a textobjects query either way. `creating-packages.md` sentence corrected to list the accepted key set.

  **Tests added:**
  - `third_party_runtime_cannot_see_trusted_ops_or_admin_modules` probe extended: all 7 editor ops are `undefined` in the third-party isolate and `function` in the trusted isolate.
  - `syntax_grammar_rejects_unknown_query_kind_deny_by_default` (runtime suite): `queries.textobjects`/`localsX`/`folds` each rejected with the offending key named.
  - `selection_query_request_validate_bounds_cursors_deny_by_default` (protocol): advisory wire path bounded at `MAX_SELECTION_QUERY_CURSORS`; over-bound rejected with `TooManySelections`.

  **Gate:** clippy `-D warnings` 0, fmt clean; 1250 lib + 155 editor + 140 protocol + 197 runtime green; security 121/122 (known pre-existing failure, unrelated).

  **Notes/ceilings:** third-party access to the editor ops remains trusted-only (no adoption path opened); the stale-behavior_version test case in the plan maps to the client-side stale-drop + per-request generation re-resolution above — a hard server-side behavior_version rejection was deliberately not added for an advisory path (would couple advisory queries to manifest churn).

- [x] 16. Run Linux-blocking verification gates ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass on Linux; editor test suite (`tests/suites/editor.rs`) green including new movement/selection/multi-cursor/caret/ligature/text-object cases; primitive-doc and JS-API/doc-registry coverage gates green.
    - Performance: No hot-path regression (typing/rendering) vs. baseline.
    - Code Quality: No new warnings; no language-specific Rust branches.
    - Security: Trust-domain tests green.
  - Approach:
    - Documentation Reviewed:
      - `AGENTS.md` (Linux-blocking gates, Windows not a required pass from a Linux host).
    - Options Considered:
      - Per-task gates vs. a final gate task. Per-task gates + a final consolidated gate.
    - Chosen Approach:
      - Final consolidated Linux gate; record any Windows-only smoke tests skipped from the Linux host per the platform-validation policy.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test
      ```
    - Files to Create/Edit:
      - None (verification).
    - References:
      - `AGENTS.md`.
  - Test Cases to Write:
      - All gates pass; Windows-only smoke tests documented as skipped-from-Linux.

  ### Task 16 Implementation Note (completed 2026-08-03)

  Final consolidated Linux gate (`autotests = false`; all 35 `tests/*.rs` files fold into the four declared suite targets, so `cargo test` covers everything):

  | Gate | Result |
  |---|---|
  | `cargo fmt --check` | ✅ clean |
  | `cargo check --all-targets` | ✅ 0 errors |
  | `cargo clippy --all-targets -- -D warnings` | ✅ 0 warnings |
  | `cargo test` — lib | ✅ 1249 passed, 2 ignored |
  | `cargo test` — main bin | ✅ 34 passed |
  | `tests/suites/editor.rs` (movement/selection/multi-cursor/caret/text-object suites incl. `editor_performance_invariants`, `markdown_mode`, `typography_protocol`, `theme_packages`) | ✅ 155 passed |
  | `tests/suites/protocol.rs` (incl. `performance_budgets`, `performance_protocol`, `primitives_docs`, `clay_js_*` gates) | ✅ 140 passed |
  | `tests/suites/runtime.rs` (incl. `syntax_grammar` trust tests, `lsp_bridge`, `parse_coordinator`) | ✅ 197 passed |
  | `tests/suites/security.rs` | ⚠️ 121 passed, 1 failed — **pre-existing** `package_manifest_rejects_invalid_slot_ui_contribution_metadata`, verified failing on clean HEAD with all plan-071 changes stashed (task 12); UI-contribution validation surface, out of plan-071 scope |

  **Performance:** no hot-path regression — the hot-path ceilings are codified as assertions in `editor_performance_invariants`, `performance_budgets` (incl. the boxed-manifest `EditAck` payload floor), and `performance_protocol`; all pass. Per-task micro-gates during implementation (single-selection bit-for-bit path, advisory query degrade-to-empty, layout-cache keying) kept typing/paint paths unchanged for the single-cursor case.

  **Windows:** no Windows-only smoke-test targets exist; `#[cfg(windows)]` branches (named-pipe connect helper in `selected_file_markdown_smoke`, workspace/file_dialog) compile-check only per `AGENTS.md` platform-validation policy — nothing skipped-from-Linux to document beyond this note.

  **Trust-domain tests:** task-15 suite green (third-party op invisibility, queries-key deny-by-default, cursor-bound reject, first-party-only grammars).

- [x] 17. Update or verify the code wiki after implementation ✓ (Implementation Note below)
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks complete (or explicitly verified unchanged); master index links relevant pages.
    - Performance: Wiki adds no runtime work; documents performance-relevant details (layout-cache keying, blink/animation, selection-set paint, text-object query).
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples, and index links.
    - Security: Wiki documents touched boundaries (client vs server authority, trust domains, read-only text-object query, typography ownership).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md` (workflow + quality bar), `page-template.md`.
    - Options Considered:
      - Update after each task vs. once after tests pass. Once after tests pass (less churn).
    - Chosen Approach:
      - After implementation + verification pass, update the Markdown code wiki once (master index + `masonry-editor.md`, `editor-theme-registry.md`, `mode-registry.md`, `parse-coordinator.md`/`decoration-transport.md` for text objects, and a new movement/selection/multi-cursor/ligature page if warranted).
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/masonry-editor.md
      docs/wiki/modules/editor-theme-registry.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation links.
      - `docs/wiki/modules/**`: updated/new pages.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`.
  - Test Cases to Write:
    - Manual wiki review: master index links relevant pages; updated pages explain the changed implementation and authority boundaries.

  ### Task 17 Implementation Note (completed 2026-08-03)

  One consolidated wiki pass after all implementation + verification (per chosen approach):

  - **New page** `docs/wiki/modules/editor-movement-selection-caret.md` — comprehensive Plan 071 implementation page: movement primitives (`MovementRules`/`WordSeparatorPolicy`/buffer classifiers), caret resolution layers + `CaretBlink` first-anim-frame loop + `CaretCell` measurement, per-role `LigaturePolicy` through the typography route (feature merge order, `LayoutCacheKey` hashing), unified `SelectionState` + multi-caret edit right-to-left + multi-op history + cursor undo stack, direction-specific command IDs vs trusted validation ops, advisory `SelectionQueryRequest`/`Result` text-object path, trust-domain boundaries, invariants/spurious-selection warning, test inventory with commands, and links to authoritative Clay JS API reference docs (linked, not duplicated).
  - **Master index** `docs/wiki/index.md` — new Modules entry describing the page.
  - **Cross-links into existing pages** (each gained a short Plan 071 section + Related link): `masonry-editor.md` (default bindings, client-local dispatch, blink anim loop, selection-set iteration, Escape chain), `editor-theme-registry.md` (caret shape/blink chrome vs theme-owned color), `typography-registry-and-font-roles.md` (ligature policy delivery + cache invalidation + explicit-push default change), `mode-registry.md` (manifest `movement`/`caretStyle`, fallback defaults, markdown prose movement), `parse-coordinator.md` (`handler_for` + `selection_query_ranges` default-None JS exclusion), `syntax-grammar-registry.md` (textobjects.scm provenance, metadata deny-by-default, first-party-only).
  - **Verified:** no wiki-asserting test broken — protocol 140, editor 155, lib 1249 all green after the doc-only changes; clippy 0, fmt clean.

### Follow-up Round Implementation Notes (completed 2026-08-03)

**Task 18 — `editor-control` permission + declaration.** `PackagePermission::EditorControl` (`editor-control`, not a prohibited authority); `clay.editorControl.modes` parsed in `src/packages/manifest.rs::parse_editor_control` — closed object shape (unknown keys rejected deny-by-default), exact mode IDs only (≤32, bounded, no wildcards, unique), foreign modes allowed, requires the permission declaration. Exposed via manifest validation op (`editorControlModes`) for user visibility. Tests: parse/default/permission-required/unknown-key/wildcard denial.

**Task 19 — mode-scoped gate + shared registration.** `require_editor_control` (`src/server/ops/editor.rs`) gates all seven editor ops: package caller → approved `editor-control` AND active major mode ∈ declared modes; trusted caller without package context → allowed (user configuration); package-less third-party → denied. Active mode resolves from the trusted worker's manifest document scope + mode registry; the third-party worker reads a host-replicated snapshot (`RuntimeCommand::UpdateActiveEditorMode` pushed after every behavior-manifest replacement and on bridge rewire — the third-party worker holds no mode registry). Ops registered in both extensions (trusted 76→77, package 36→44); third-party visibility test and plan061 inventory rebaselined. Tests: trusted allow/missing-permission/wrong-mode, third-party allow/wrong-mode/no-context, all end-to-end through the JS runtime harness. Known ceiling: documents editing under the bare default manifest (no registry-activated mode) deny package callers by design.

**Task 20 — `EditorCommandRequest` push channel (protocol v8).** `src/protocol/editor_control.rs` wire type (bounded; boxed `ServerMessage` variant keeps small-payload budgets); `op_clay_editor_execute_command` validates a known editor command ID (EditorClientCommand/SelectionQuery allowlists, deny-by-default), passes the task-19 gate, stamps host provenance (apiPrefix or `clay.config`), and publishes on a bounded broadcast shared by both domain workers (`ClayJsRuntimeService.editor_commands`, survives reload, rewired on worker replacement). Every connection loop forwards requests as `ServerMessage::EditorCommandRequest` (lagged drops — advisory). Client `EditorWidget::apply_editor_command_request` re-parses deny-by-default and dispatches through the exact keybinding paths; non-editor IDs (`clay.application.quit`) rejected server-side and dropped client-side. Facade `clientExecuteEditorCommand` + typed d.ts. Tests: protocol bounds, publish-with-provenance (trusted + third-party via subscriber), unknown-ID denial, widget apply/drop.

**Task 21 — docs + inventory + wiki.** New API doc `client-execute-editor-command.md` (Phase 23, `editor-control` permission recorded) + inventory entry + index link + registry regenerated; `creating-packages.md` gained the permissions-table row and the full "Editor Control" boundary section (declaration shape, per-call enforcement, execution flow, override semantics, conflict/deactivation policy, revocation); wiki `editor-movement-selection-caret.md` gained the trust-boundary section and updated op architecture. Existing editor API docs needed no trust-wording changes (they never claimed trusted-only).

**Gate:** fmt clean, clippy 0, lib 1257, editor 155, protocol 140, runtime 197, security 121+1 pre-existing (`package_manifest_rejects_invalid_slot_ui_contribution_metadata`, unrelated).

## Compromises Made

Final list (constraints accepted deliberately; each has a recorded upgrade path):

1. **Per-role ligature policy only** — no per-range (syntax-span-aware) ligature control (Approach C rejected); `FontProfile.ligatures` per `FontRole` is the single ownership point.
2. **Secondary carets do not blink** — only the primary caret runs the blink state machine; secondaries paint solid (matches VSCode).
3. **`select_next_match` is document-text based** — plain byte-substring occurrences; search-engine-driven refinement is later.
4. **Discrete blink for Phase/Smooth styles** — `CaretBlink` uses on/off timing; true alpha ramp / smooth caret animation deferred (`smooth_animation_ms` field already wired).
5. **Multi-caret edits record N history entries** — undo replays them one at a time; batching as one undo unit deferred (`HistoryEntry` already stores op vectors, so the upgrade is grouping, not restructuring).
6. **No default keybindings for text-object/smart-select** — packages bind them via `bindKey` (IDs auto-declared); multi-stroke chords (`]f`) remain unsupported by `bindKey`.
7. ~~**Editor ops stay trusted-only**~~ — SUPERSEDED by the follow-up round (2026-08-03): all seven ops plus `op_clay_editor_execute_command` are shared across both runtime domains behind the mode-scoped `editor-control` gate. `bindKey` remains admin-only; package keymaps come through mode declarations instead.
8. **`core.text` keeps code-style movement defaults** — prose movement comes from package manifests (`@clay/markdown`), avoiding a silent behavior change for unclaimed files.
9. **`line_movement: screenLine` falls back to character movement** until soft-wrapping exists.
10. **ServerMessage `BehaviorManifest` boxed (protocol v6→v7)** — manifest growth no longer inflates small payloads; wire version bumped once for tasks 4–10 combined.

## Further Actions

Priority-ordered follow-ups (none blocking):

1. **Multi-stroke chord support in `bindKey`** (unblocks Helix-style `]f`/`[f` textobject bindings and goto-mode emulation) — medium priority, protocol+keybindings scope.
2. ~~**Third-party access decision for editor ops**~~ — DONE in the follow-up round (approved 2026-08-03): `editor-control` permission + `clay.editorControl.modes` declaration + per-call gate + advisory push channel. Remaining follow-ups: wildcards/scope widening (rejected for v1), built-in `core.*` modes activated through the registry so packages can declare them end-to-end.
3. **Search-driven `select_next_match` refinement** — reuse the search primitive when it lands; current byte-substring needle is the documented ceiling.
4. **Batch multi-caret undo** — group the N per-caret history entries of one multi-caret gesture into a single undo unit.
5. **Smooth caret animation** — alpha ramp honoring `smooth_animation_ms` inside `CaretBlink` (animation frame loop already exists).
6. **More built-in `textobjects.scm` languages** — Python/Go/C-family when their first-party grammar packages land.
7. **Per-range ligature control** (Approach C) — syntax-span-aware feature switching inside strings/comments; needs Parley ranged-feature support or layout segmentation.
8. **Modal/operator-pending emulation as a third-party package** — feasible now on the shipped primitives (movement ops + bindKey + command registry); no core changes expected.
9. **AI/agent-driven selection** — server-authoritative surface built on the selection primitives per the extensions-and-AI pattern.
10. ~~**Fix pre-existing `security::package_manifest_rejects_invalid_slot_ui_contribution_metadata`**~~ — DONE as standalone task 22 (2026-08-03): stale-test fix, security suite 122/0.
---

## Follow-up Round: Third-party editor-op access (`editor-control`)

Approved 2026-08-03: mode-scoped, deny-by-default package access to the seven editor ops plus a programmatic execution channel. Boundary: new `editor-control` permission + package-record `editorControl.modes` declaration; every op enforces caller capability AND active-major-mode membership; coexistence on conflicts (user deactivates packages); programmatic triggers via a new bounded server→client push message.

- [x] 18. `editor-control` permission and `editorControl.modes` package-record declaration

  - Acceptance Criteria:
    - `PackagePermission::EditorControl` exists (`editor-control`), parses, and is NOT a prohibited authority.
    - Package record metadata gains `editorControl: { modes: [...] }` — exact mode IDs only, bounded, unknown keys rejected deny-by-default.
    - Invalid declarations produce typed record errors naming the offending field.

- [x] 19. Mode-scoped gate on the seven editor ops + package-extension registration

  - Acceptance Criteria:
    - All seven `op_clay_editor_*` ops are registered in BOTH runtime extensions.
    - With an active package context: op succeeds only when the package holds approved `editor-control` AND the active document's major mode ∈ its declared `editorControl.modes`; otherwise typed deny-by-default error.
    - Trusted-domain calls without a package context (user configuration) remain allowed.
    - Existing third-party op-visibility and plan061 inventory tests are rebaselined to match the shared registration.

- [x] 20. `EditorCommandRequest` programmatic execution channel (protocol v8)

  - Acceptance Criteria:
    - `ServerMessage::EditorCommandRequest { command_id, package_prefix, mode_id }` exists; PROTOCOL_VERSION bumps 7→8.
    - New op `op_clay_editor_execute_command` validates a known editor command ID through the same task-19 gate, then publishes to a bounded broadcast consumed by every connection loop.
    - Client dispatches received command IDs through the SAME path as keybinding-routed command IDs (deny-by-default re-parse); unknown IDs dropped silently.
    - Packages without `editor-control` or outside their declared mode never reach the push channel.

- [x] 21. Documentation, inventory rebaseline, wiki, and plan closure for the follow-up round

  - Acceptance Criteria:
    - `creating-packages.md` documents `editor-control`, `editorControl.modes`, and the execution flow with a conflict/deactivation note.
    - New API doc + inventory entry for `clientExecuteEditorCommand` (or chosen name); registry regenerated; editor op docs updated from trusted-only to gated package-facing.
    - plan061 op inventory rebaselined (editor ops shared between domains).
    - Wiki `editor-movement-selection-caret.md` trust-boundary section updated.

---

## Standalone Fix Round (unrelated to the editor-capabilities work)

- [x] 22. Fix pre-existing `security::package_manifest_rejects_invalid_slot_ui_contribution_metadata`

  **Root cause (not what plans 068/069 recorded):** the test is stale, not the validation. Phase 20.5 promoted `modal` to a supported `ComponentKind` (z-modal focus-trap dialog, production-used; `src/shell/components.rs`), leaving `table` as the only `DeferredComponentKind`. The test still asserted `modal` children are rejected as "reserved for a later" phase, so `assemble_package_record` correctly returned `Ok` and `unwrap_err()` panicked. The raw-CSS subcase (`style.background = "#ffffff"`) and the payload-budget subcase were never the failure — both enforce correctly once the test reaches them.

  **Fix:** point the unsupported-kind subcase at the actually-deferred kind `table` (`tests/package_loading.rs`), with a comment recording the Phase 20.5 promotion. One-line test update; zero validation-code changes — deny-by-default UI validation untouched.

  **Gate:** `cargo test --test security` → 122 passed, 0 failed (first fully green security suite since Phase 20.5); fmt clean.
