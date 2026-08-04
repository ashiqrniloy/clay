# Editor Movement, Selection, Caret, Ligatures, and Text Objects (Plan 071)

## Source

- `src/protocol/mod.rs` (`EditorCommand`, `MovementRules`, `WordSeparatorPolicy`, `ParagraphStyle`, `LineMovementStyle`, `CaretStyle`, `CaretShape`, `BlinkStyle`, `LigaturePolicy`, `FontProfile`, `EditorBehaviorRules`)
- `src/protocol/textobjects.rs` (`SelectionQuery`, `TextobjectKind`, `SelectionQueryRequest`/`Result`)
- `src/protocol/editor_control.rs` (`EditorCommandRequest` push wire, follow-up round)
- `src/editor/buffer.rs` (word/paragraph/pair boundary classifiers)
- `src/editor/cursor.rs` (`CursorState`)
- `src/editor/selection.rs` (`Selection`, `SelectionState`)
- `src/editor/surface.rs` (`EditorSurface`: movement dispatch, caret resolution, `CaretBlink`, multi-caret edit, cursor undo)
- `src/editor/layout.rs` (`CaretCell` measurement, font-feature push into Parley)
- `src/editor/typography.rs` (`ResolvedFontProfile::font_features`, `resolve_font_features`)
- `src/masonry_editor.rs` (`EditorClientCommand`, default key bindings, selection-query enqueue/apply, anim-frame blink loop)
- `src/server/ops/editor.rs` (trusted editor validation ops)
- `src/server/ops/modes.rs` (`parse_movement_rules`, `parse_caret_style`)
- `src/server/syntax.rs` (`TreeSitterSyntaxHandler::selection_query_ranges`, textobject/smart-select query runners)
- `src/server/connection.rs` (`SelectionQueryRequest` dispatch)
- `packages/{rust,typescript,javascript}/queries/textobjects.scm`
- `runtime/js/editor.js`, `runtime/js/behavior.js`

## Overview

Plan 071 ("Editor Movement, Selection, Caret, and Ligatures") gave Clay first-class keyboard-driven editing: word/paragraph/structural movement, shape- and blink-configurable carets, multi-cursor editing, font-ligature control, and tree-sitter text objects with smart select. Everything is primitive-first: generic Rust primitives configured by inert per-mode manifest data and user typography; no mode-specific Rust. Public usage is documented in the Clay JS API reference (`docs/reference/clay-js-api/editor/*`, `behavior/build-code-editing-manifest.md`, `theme/set-typography.md`); this page explains the implementation behind those APIs.

## Movement primitives (task 4)

`MovementRules` is a wire type on `EditorBehaviorRules` (`movement` manifest key) with: `word_separators` (`WordSeparatorPolicy::Code` — alphanumerics plus optional underscore; `Prose` — alphanumerics only; `Custom(Vec<char>)`), `treat_underscore_as_word`, `camel_case_sub_word`, `paragraph_style` (`BlankLine` / `BlankLineOrWhitespace`), `stop_at_eol_word_end`, `line_movement` (`Character`; `ScreenLine` falls back until wrapping exists), `sticky_column`. `parse_movement_rules` in `src/server/ops/modes.rs` validates each field deny-by-default; absent fields fall back to `MovementRules::default()`. `default_code()` inherits movement from `default_text()` (identical defaults; code-style until a package says otherwise).

`EditorBuffer` boundary classifiers implement the motion semantics: `classify_word` (with combining-mark U+0300–U+036F continuation and long-WORD whitespace-only override), `next/prev_word_start/end` (two-phase skip then scan), `next/prev_sub_word_start` (camelCase `lower→upper`, digit↔letter, underscore transitions), `next/prev_paragraph` + `paragraph_end_byte`, `first/last_non_blank_byte`, and `matching_pair_byte` (depth-counting). `is_completion_word_character` delegates to `WordSeparatorPolicy::Code.is_word_char`, unifying the classifier.

New `EditorCommand` variants: `MoveWordStart`/`MoveWordEnd`/`MoveParagraph` (each with direction + extend flag), `SelectWord`, `SelectLine`, `SelectParagraph`. Default bindings (hardcoded in `EditorWidget::on_text_event`): `Ctrl+Left/Right` word start, `Ctrl+Up/Down` paragraph, `Ctrl+L` select line (Shift variants extend). `preferred_x` (sticky column) survives vertical moves; `stop_at_eol_word_end` controls whether forward word-end motion parks before or after the line break.

## Caret styling and blinking (task 6)

`CaretStyle` = `shape` (`CaretShape::Bar|Line|Block|Underline`), `width_px`, `height_pct`, `hollow`, `blink` (`BlinkStyle::Solid|Blink|Phase|Smooth`), `smooth_animation_ms`, `stop_blink_on_typing`. Resolution is three-layer in `EditorSurface::effective_caret_style`: runtime override (`clientSetCursorStyle`; the op validates, field-merges against the manifest layer, and publishes the result on the `caret_styles` broadcast — the connection forwards it as `ServerMessage::CaretStyleOverride`, initial-sync and on generation change, and the client applies it to `EditorSurface` and kicks the blink loop) → per-mode manifest `caret_style` → `StyleRegistry::caret_style` theme default (Bar, from `StyleRegistry::clay_default()`). Caret **color** stays theme-owned (`theme.base.caret`); `CaretStyle` controls geometry/blink only.

Rendering: `CaretCell` (`src/editor/layout.rs`) measures the character advance at the caret via Parley `Cursor::next_visual` to derive Block width / Underline height (falls back to `line_height * 0.6` at end of line/text); `paint_caret` builds shape-specific geometry from it. The IME preedit caret (`paint_preedit_overlay`) shares the shape logic so preedit never regresses to a hardcoded bar.

Blinking: `CaretBlink` (surface.rs) is a `Wait → On → Off` phase state machine advanced by Masonry `on_anim_frame` — Clay's first animation-frame usage. The widget calls `ctx.request_anim_frame()` while a blinkable caret exists; `Solid` always shows; user input resets the timer when `stop_blink_on_typing`. `Phase`/`Smooth` currently use discrete timing (alpha ramp deferred).

## Font ligatures (task 7)

`LigaturePolicy` lives on `FontProfile.ligatures` (boxed field, one per `FontRole`): `enable_standard` (→ `liga`+`clig`, default true), `enable_contextual` (→ `calt`, default true), `discretionary_features` (tags enabled), `raw_features` (CSS-format string parsed by swash), `disable_features` (tags forced to 0). Validation bounds: ≤32 features per list kind, raw ≤256 bytes, tag names 1–4 chars. Resolution merges into one `BTreeMap<tag, u16>` last-wins in that order.

Ownership is the **typography route**: a mode selects a `FontRole`; the role's `FontProfile` selects the ligature policy. There is deliberately no per-mode or per-range ligature field in `EditorBehaviorRules`. `ResolvedFontProfile::font_features()` produces `FontSettings::List` pushed as `StyleProperty::FontFeatures` during Parley layout rebuild (`src/editor/layout.rs`), and `LayoutCacheKey` hashes the feature set so policy changes invalidate cached layouts without a typography-revision bump. Default true/true means Clay explicitly pushes `liga=1 clig=1 calt=1` (pre-Plan-071 Clay pushed no features and relied on font defaults). User control: `setTypography({ monospace: { ligatures: {...} }, ... })`.

## Unified selection state and multi-cursor (tasks 8–9)

`Selection` embeds a `CursorState` (`anchor` + `cursor.caret()` as focus; `Selection` is `Copy`). `SelectionState` wraps `Vec<Selection>` + `primary` index with an always-non-empty invariant. `EditorSurface` replaced its separate `cursor` + `Option<SelectionState>` fields with one `selections: SelectionState`. Single-selection behavior is preserved bit-for-bit (`set_primary_focus` + `clear_selection` pairing; `set_primary_focus` alone creates a spurious range when anchor ≠ focus — the key invariant for future edits).

Multi-cursor commands (`EditorCommand`): `AddCursor` (below/above at same scalar column; refuses stacking when the column is occupied), `ColumnSelect` (Down/Up grow a rectangular box; Left/Right move **all** carets horizontally), `SelectNextMatch`/`SelectPrevMatch` (collapsed caret first press selects the word at position as needle; wrap-around; stops when every occurrence is selected), `SelectAllMatches`, `CancelMultipleSelections`, `KeepSelection`, `RemoveSelection`, `UndoCursorMove`. Defaults: `Ctrl+Alt+Up/Down` add cursor, `Shift+Alt+arrows` column select, `Ctrl+D` select next match (rebound from `SelectWord`, which it subsumes), `Ctrl+Shift+L` all matches, `Ctrl+U` cursor undo. Escape priority chain: completion menu > snippet session > multi-selection cancel.

Editing with many carets: `multi_caret_edit` sorts carets by byte offset and applies right-to-left so earlier offsets stay valid; typing/backspace/delete insert at every caret; `selected_text` concatenates non-collapsed selections in order. `HistoryEntry` carries `forward_ops`/`inverse_ops` vectors plus selection-set snapshots; undo applies inverse ops in **ascending** offset order (each inverse op is valid in the coordinate system at forward-apply time), redo replays stored order. Cursor-movement undo is a separate bounded stack (`cursor_undo_stack`, `CURSOR_UNDO_MAX_DEPTH` = 64) snapshotted before every selection-changing command. Ceiling: multi-caret edits record N history entries; batching them as one undo unit is deferred.

## Command-ID and op architecture

Two coordinated surfaces:
1. **Direction-specific argless command IDs** (keybinding-execution surface): `clay.editor.clientMoveCursor.nextWordStart`, `...clientAddCursor.below`, etc. `KeyBindingRule` has no arguments field, so every rebindable action needs a self-describing ID. Client dispatch: `EditorClientCommand::from_command_id` maps ID → `EditorCommand`; routed `ClientUiCommand`; executed client-locally in `main.rs`/widget. No server round-trip.
2. **Typed validation ops** (programmatic JS surface): `op_clay_editor_move_cursor`, `op_clay_editor_set_selection`, `op_clay_editor_set_cursor_style`, `op_clay_editor_add_cursor`, `op_clay_editor_column_select`, `op_clay_editor_select_textobject`, `op_clay_editor_smart_select` validate arguments deny-by-default and return the command-ID descriptor. Since the follow-up round they are registered in **both** runtime domains behind the `editor-control` gate (below). `op_clay_editor_execute_command` adds the gated programmatic execution channel.

## Tree-sitter text objects and smart select (task 10)

Wire: `ClientMessage::SelectionQueryRequest` (request id, document id/version, behavior version, `SelectionQuery`, up to `MAX_SELECTION_QUERY_CURSORS` = 256 cursors) → `ServerMessage::SelectionQueryResult` (one `Option<Range>` per cursor). `PROTOCOL_VERSION` is 7 for these variants.

Server: `connection.rs` validates, resolves document metadata/text, re-resolves the native handler from `runtime_generation.current()` per request (no stale state), and calls `ParseHandler::selection_query_ranges`. `TreeSitterSyntaxHandler` implements it using `packages/*/queries/textobjects.scm` (capture schema `@textobject.<kind>.<scope>`; kinds function/class/argument/comment/loop/conditional/call/statement; `inner` falls back to `around`): Current = smallest containing node, Next = earliest start strictly after focus, Previous = latest end at/before focus. Smart select needs no query file: Expand walks the parent chain to the first strictly larger ancestor; Shrink DFS-finds the largest descendant strictly inside the selection. Cached trees are reused only when document version and full-document coverage match; otherwise a bounded fresh parse runs. **Every miss degrades to empty ranges — advisory queries never block editing.**

Client: `EditorWidget` keeps `pending_selection_query` (request id + cursor snapshot); results apply only on matching request/document/version, preserving backward selections and leaving `None` carets untouched. Cursor-undo snapshots the pre-query set.

Authority boundaries (task 15 + follow-up round): text-object queries are pure Rust over inert ranges (no V8 involvement — JS parse handlers inherit the `None` default); package grammar `queries` metadata rejects any key outside {highlights, locals, injections} deny-by-default (`src/packages/record.rs`), and grammar contributions stay first-party-only.

## `editor-control` trust boundary (follow-up round, approved 2026-08-03)

First- and third-party packages may access the editor ops and trigger execution programmatically, under a mode-scoped deny-by-default boundary:

- **Permission**: `PackagePermission::EditorControl` (`editor-control`, `src/packages/permissions.rs`) — not a prohibited authority, but adoption approval surfaces it for third parties.
- **Mode declaration**: package.json `clay.editorControl.modes` (`src/packages/manifest.rs::parse_editor_control`) — exact mode IDs only (≤32, no wildcards), closed object shape, requires the permission declaration. Foreign modes (e.g. `core.code`) are allowed.
- **Gate** (`src/server/ops/editor.rs::require_editor_control`, every editor op): caller inside a *package activation* → approved `editor-control` AND active major mode ∈ declared modes; trusted caller outside any activation (user configuration) → allowed. "Inside package activation" is a nesting depth on `ClayOpState` entered by the loadPackage stamp, controlled package evaluations, and host-invoked parse/completion/analysis callbacks, and exited by `op_clay_packages_end_package_activation` after each loadEntry — the attribution stamp (`current_package`) intentionally outlives activations for later package-facing registrations, so the gate keys on the activation scope, not stamp presence. Active mode resolves from the active manifest's document scope + mode registry on the trusted worker, and from a host-replicated snapshot (`RuntimeCommand::UpdateActiveEditorMode`) on the third-party worker, which holds no mode registry of its own.
- **Execution channel** (protocol v8): `op_clay_editor_execute_command` validates a known editor command ID through the same gate and publishes `EditorCommandRequest` (`src/protocol/editor_control.rs`) on a bounded broadcast shared by both domain workers; every connection loop forwards it as `ServerMessage::EditorCommandRequest`; the client (`EditorWidget::apply_editor_command_request`) re-parses deny-by-default and dispatches through the keybinding paths. Non-editor IDs (e.g. `clay.application.quit`) are rejected server-side and dropped client-side.
- **Override semantics**: activated packages override default behavior through their mode's `keymaps` + `editorRules` (manifest routing beats hardcoded widget defaults); the ops add the gesture-free surface.
- **Conflicts**: coexistence, no arbitration — the user deactivates packages (adoption revoke / settings disable, live via runtime reload).
- **Known ceiling**: modes that are never activated through the mode registry (e.g. documents editing under the bare default manifest) report no active mode, so package callers deny there by design.

## Invariants and constraints

- Single-selection paths remain bit-for-bit compatible; multi-cursor code gates on `selections.len() > 1`.
- `SelectionState` is never empty; `primary` is always clamped.
- `set_primary_focus` without `clear_selection` is the canonical spurious-selection bug; keep the pairing.
- Caret color is theme-owned; `CaretStyle` never carries color.
- Ligature policy follows `FontRole`; there is no manifest or per-range ligature field.
- Text-object/smart-select results are advisory: stale or missing results leave carets unchanged.
- Direction-specific command IDs are parsed, never string-enumerated (50 IDs would bloat allowlists).

## Tests

- `src/editor/surface.rs`: movement classifier/motion tests, `effective_caret_style_resolves_override_manifest_theme`, multi-cursor edit/undo (`add_cursor_refuses_to_stack_on_same_line_or_past_edges`, `cursor_undo_restores_previous_selection_set`), selection-query request/apply round-trip.
- `src/masonry_editor.rs`: `editor_client_command_maps_ids_and_moves_caret`, `editor_client_command_dispatches_multi_cursor_commands`, `selection_query_result_applies_ranges_keeps_unmatched_and_drops_stale`.
- `src/server/syntax.rs`: textobject query compile + function/comment direction tests, smart-select expand/shrink monotonicity, markdown degrade-to-none.
- `src/server/ops/editor.rs`: deny-by-default validation tests for every editor op.
- `src/protocol/textobjects.rs`: command-ID round trips, unknown-ID rejection, cursor-bound validation.
- `src/server/js_runtime.rs`: `third_party_runtime_cannot_see_trusted_ops_or_admin_modules` (editor ops visible but gated third-party), `editor_control_gate_enforces_permission_and_declared_mode`, `third_party_editor_control_gate_requires_declared_mode`, `editor_control_execute_publishes_gated_known_commands_only`.
- `tests/syntax_grammar.rs`: `syntax_grammar_rejects_unknown_query_kind_deny_by_default`.
- Commands: `cargo test --lib`, `cargo test --test editor`, `cargo test --test protocol`, `cargo test --test runtime`.

## Related

- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [Mode Registry](mode-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Behavior Manifests](behavior-manifests.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- Plan: `plans/071-Editor-Movement-Selection-Caret-Ligatures.md`
- Reference docs: `docs/reference/clay-js-api/editor/` (movement/selection/caret/multi-cursor/textobject APIs), `docs/reference/clay-js-api/behavior/build-code-editing-manifest.md`, `docs/reference/clay-js-api/theme/set-typography.md`, `docs/reference/packages/creating-packages.md` (movement/caretStyle/textobjects authoring)
