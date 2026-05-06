# Phase 2 Editor Interaction Model

## Objectives
- Replace append-only editing with a minimally real local editor interaction model.
- Add cursor, selection, keyboard navigation, pointer hit-testing, and edit-at-offset behavior on top of the Phase 1 text canvas foundation.
- Keep all interaction state local to the Masonry client surface so future IPC phases can translate user edits into explicit versioned text operations.
- Preserve bounded viewport/layout behavior and avoid reintroducing whole-buffer paint assumptions.

## Expected Outcome
- `cargo run` opens the Clay editor window and supports typing at the caret, Backspace/Delete at the caret, Enter, click-to-place-caret, arrow/Home/End navigation, basic selection, scrolling, focus, resize, and Escape exit.
- The caret is visibly rendered and kept in view after local edits and navigation.
- Text edits are represented internally as valid byte-offset/range operations over the `crop` rope.
- Cursor movement and deletion are Unicode-safe and never create invalid UTF-8 byte offsets.
- Phase 1 layout caching, visible-range extraction, logical-line scrolling, and visual wrapped-line overflow handling continue to work.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- No IPC, `deno_core`, SDUI protocol, server process, extension loading, filesystem access, or network access is introduced in this phase.

## Tasks

- [x] Introduce cursor and text-range editing primitives
  - Acceptance Criteria:
    - Functional: `EditorBuffer` can insert text, insert newline, delete a byte range, backspace before an arbitrary caret, and delete after an arbitrary caret while returning updated caret offsets.
    - Performance: Edit operations use `crop::Rope` byte-offset insert/delete/replace APIs and do not materialize the whole buffer during normal editing.
    - Code Quality: Cursor state is represented by a small dedicated type that stores valid byte offsets and keeps buffer mutation separate from Masonry event routing.
    - Security: All edited content remains inert text; range APIs validate/clamp offsets and do not interpret input as commands, paths, markup, IPC payloads, scripts, or filesystem/network requests.
  - Approach:
    - Documentation Reviewed:
      - `crop` 0.4.3 docs/code-search: `Rope` uses UTF-8 byte offsets; `insert` panics if the offset is not on a code point boundary or is out of bounds; `delete` and `replace` operate on byte ranges and require valid code point boundaries; `line_of_byte`, `byte_of_line`, and `byte_slice` convert between line and byte ranges.
      - Phase 1 plan compromises: append-only document-end follow is intentionally temporary until full cursor/selection state exists.
    - Options Considered:
      - Keep cursor state in `EditorSurface`: fastest path, but mixes text model invariants with painting and viewport behavior.
      - Store cursor as byte offsets in a dedicated module: matches `crop` indexing and future IPC edit ranges, but requires explicit boundary helpers.
      - Store cursor as character indices: superficially easier for movement, but requires conversions for every `crop` operation and can hide byte-boundary mistakes.
    - Chosen Approach:
      - Add `src/editor/cursor.rs` with a `CursorState` storing caret byte offset plus optional desired visual x for vertical movement later. Extend `EditorBuffer` with checked byte-range edit methods and boundary helpers. Keep all public mutation APIs returning whether text changed and the new caret/range.
    - API Notes and Examples:
      ```rust
      let mut rope = crop::Rope::from("Hello Earth!");
      rope.insert(11, " 🌎");
      rope.delete(5..16);

      pub struct CursorState {
          caret: usize,
          preferred_x: Option<f32>,
      }
      ```
    - Files to Create/Edit:
      - `src/editor.rs`: Export new cursor/range modules as needed.
      - `src/editor/cursor.rs`: New cursor state and byte-offset invariants.
      - `src/editor/buffer.rs`: Add edit-at-offset, delete-range, backspace/delete-around-caret, line/byte helper APIs, and tests.
      - `src/editor/surface.rs`: Replace append-only insertion calls with cursor-aware editing entry points.
    - References:
      - `concept.md` section 4 on client shadow text state and future versioned mutations.
      - `roadmap.md` Phase 2 interaction model.
      - `plans/002-Phase1-TextCanvasFoundation.md` compromises on append-only end following.
  - Test Cases to Write:
    - `insert_at_caret_updates_buffer_and_caret`: Inserting in the middle of text updates text and advances caret by inserted byte length.
    - `backspace_at_caret_deletes_previous_scalar_boundary`: Backspace removes the previous valid Unicode scalar before the caret.
    - `delete_at_caret_deletes_next_scalar_boundary`: Delete removes the next valid Unicode scalar after the caret.
    - `delete_range_clamps_or_rejects_invalid_ranges`: Invalid/out-of-order ranges do not panic and do not mutate unexpectedly.

- [x] Add Unicode-safe cursor movement helpers
  - Acceptance Criteria:
    - Functional: The editor can move the caret to the previous/next valid text boundary, document start, document end, line start, and line end without panicking on multibyte Unicode text.
    - Performance: Movement operates on bounded rope slices around the current offset where practical and avoids cloning the whole document for each key press.
    - Code Quality: Boundary movement helpers are unit-tested independently from Masonry and document whether Phase 2 uses Unicode scalar boundaries or grapheme cluster boundaries.
    - Security: Movement helpers only inspect local rope text and cannot trigger commands, filesystem access, IPC, network, or script execution.
  - Approach:
    - Documentation Reviewed:
      - `crop` docs/code-search: `Rope` indexing is UTF-8 byte based; insertion/deletion require code point boundaries; line conversion APIs provide valid line starts.
      - `unicode-segmentation` docs via Context7 `/websites/rs_unicode-segmentation`: `UnicodeSegmentation::grapheme_indices` and `GraphemeCursor` can find extended grapheme cluster boundaries for cursor movement.
    - Options Considered:
      - Use only Rust `char` boundaries: no new direct dependency and sufficient to avoid invalid UTF-8, but can split user-perceived grapheme clusters such as flags or combining marks.
      - Add a direct `unicode-segmentation` dependency and move by grapheme clusters: better editor behavior and the crate is already present transitively, but adds an explicit dependency and may require chunk-aware helpers for large ropes.
      - Implement full UAX #29 over rope chunks manually: unnecessary and error-prone for this phase.
    - Chosen Approach:
      - Implemented scalar-boundary movement first, using `crop` byte-offset and character-boundary helpers to keep all offsets valid UTF-8. Grapheme-perfect movement with `unicode-segmentation` remains deferred until chunk-aware layout/caret work makes the extra dependency worthwhile.
    - API Notes and Examples:
      ```rust
      use unicode_segmentation::UnicodeSegmentation;

      let text = "a🇷🇸é";
      let boundaries: Vec<usize> = text
          .grapheme_indices(true)
          .map(|(offset, _)| offset)
          .chain(std::iter::once(text.len()))
          .collect();
      ```
    - Files to Create/Edit:
      - `src/editor/cursor.rs`: Add scalar-boundary movement helpers and tests.
      - `src/editor/buffer.rs`: Add bounded byte/line helper APIs needed by movement.
      - `src/editor/surface.rs`: Expose cursor movement commands that keep the caret line visible.
      - `src/masonry_editor.rs`: Map Left/Right/Home/End keyboard events to cursor movement, with Ctrl/Meta+Home/End for document start/end.
    - References:
      - Context7 docs response `ctx7:docs:84c9d8ddc267eeda13d6f4cf` for `unicode-segmentation` boundary APIs.
      - `crop` docs/code-search results for UTF-8 byte offset indexing and boundary panics.
  - Test Cases to Write:
    - `cursor_moves_left_and_right_over_ascii`: Basic byte offsets match expected positions.
    - `cursor_moves_over_multibyte_scalars_without_invalid_offsets`: Emoji/CJK/accented text movement never lands inside a code point.
    - `cursor_boundary_policy_for_combining_marks_is_documented`: A test captures the chosen scalar-vs-grapheme behavior.
    - `line_start_and_line_end_handle_lf_and_final_line`: Home/End helpers work for middle lines and final non-newline-terminated lines.

- [ ] Wire keyboard editing and navigation through Masonry text events
  - Acceptance Criteria:
    - Functional: Printable input, IME commits, Enter, Backspace, Delete, Left/Right, Up/Down, Home/End, and Escape behave as expected with the caret instead of always targeting the buffer end.
    - Performance: Key handling performs only local buffer/cursor/viewport mutations and requests Masonry render/accessibility updates when state changes.
    - Code Quality: `src/masonry_editor.rs` remains an event-routing boundary; editor semantics live in `EditorSurface`, `EditorBuffer`, and cursor modules.
    - Security: Keyboard shortcuts and control keys are handled only as local editor commands; no shell, filesystem, network, IPC, extension, or script execution behavior is added.
  - Approach:
    - Documentation Reviewed:
      - Masonry 0.4 docs/code-search: `Widget::on_text_event`, `TextEvent::Keyboard`, `KeyState::Down`, `Key::Named`, `NamedKey`, `EventCtx::request_render`, `request_accessibility_update`, `set_handled`, and `request_focus`.
      - Masonry 0.4 custom widget docs/code-search: event methods mutate widget state and request rendering explicitly; focused widgets receive keyboard events.
    - Options Considered:
      - Handle all keys directly in `EditorWidget`: easy to add, but repeats Phase 0/1 coupling and makes future IPC edit operation extraction harder.
      - Convert Masonry events into small editor commands: keeps UI boundary thin and makes future client/server edit messages clearer.
      - Use Masonry built-in `TextInput`/`TextArea`: would discard the custom `crop`/Parley/Vello canvas risk that Clay is proving.
    - Chosen Approach:
      - Add an editor command layer such as `EditorCommand::{Insert, Newline, Backspace, Delete, MoveLeft, MoveRight, MoveUp, MoveDown, Home, End}`. `EditorWidget` maps Masonry text events to commands and delegates behavior to `EditorSurface`.
    - API Notes and Examples:
      ```rust
      match &key_event.key {
          Key::Named(NamedKey::ArrowLeft) => editor.command(EditorCommand::MoveLeft),
          Key::Named(NamedKey::Delete) => editor.command(EditorCommand::DeleteForward),
          Key::Character(text) => editor.insert_text(text),
          _ => false,
      };
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`: Map new key events to editor commands and preserve Escape action.
      - `src/editor/surface.rs`: Add command handling and viewport/caret visibility updates.
      - `src/editor/cursor.rs`: Add movement command support.
      - `src/editor/buffer.rs`: Add deletion helpers used by commands.
    - References:
      - Masonry docs/code-search results for `Widget`, `TextEvent`, focus, render requests, and custom widget lifecycle.
      - `src/masonry_editor.rs`: Existing Escape/Backspace/Enter/text routing.
  - Test Cases to Write:
    - `editor_insert_text_replaces_append_only_behavior`: Moving caret left then typing inserts before existing trailing text.
    - `editor_backspace_at_start_is_noop`: Backspace at offset zero does not mutate or panic.
    - `editor_delete_at_end_is_noop`: Delete at document end does not mutate or panic.
    - Manual smoke test: Type `abc`, move left, type `X`, verify `abXc`, then use Backspace/Delete/Home/End.

- [ ] Prepare layout hit-testing and caret geometry APIs
  - Acceptance Criteria:
    - Functional: The layout layer can map a visible text coordinate to a visible-snapshot byte offset and can return caret geometry for a visible-snapshot byte offset.
    - Performance: Hit-testing and caret geometry reuse the cached Parley layout whenever the cache key is unchanged and do not rebuild layout more than the existing cache policy requires.
    - Code Quality: Coordinate conversions explicitly account for `TEXT_INSET`, logical-line visible snapshot start offsets, and visual scroll translation.
    - Security: Hit-testing only maps local pointer coordinates to local text offsets and cannot trigger commands, filesystem access, IPC, network, or script execution.
  - Approach:
    - Documentation Reviewed:
      - Parley Context7 `/linebender/parley`: `parley::editing::Cursor::from_point(&layout, x, y)` creates a cursor from coordinates; `Cursor` stores byte index and affinity; selections/cursors expose geometry for caret drawing.
      - Masonry `PaintCtx::text_contexts` docs/code-search: text layouts should be built from shared Parley contexts and invalidated when fonts change.
    - Options Considered:
      - Implement approximate monospace hit-testing: quick but wrong for proportional fonts, wrapping, bidi, and Parley layout decisions.
      - Use Parley editing cursor APIs over cached layouts: matches the chosen text engine and keeps hit-testing consistent with glyph layout.
      - Defer hit-testing until after IPC: not viable because real local editing needs click-to-place and selection before synchronization.
    - Chosen Approach:
      - Extend `LayoutState` with methods that expose hit-testing and caret metrics from the cached layout. Convert between document byte offsets and visible-snapshot-relative byte offsets at the `EditorSurface` boundary.
    - API Notes and Examples:
      ```rust
      use parley::editing::Cursor;

      let local_x = pointer_x - TEXT_INSET as f32;
      let local_y = pointer_y - TEXT_INSET as f32 + visual_scroll_y as f32;
      let cursor = Cursor::from_point(&layout, local_x, local_y);
      let visible_offset = cursor.index();
      ```
    - Files to Create/Edit:
      - `src/editor/layout.rs`: Add cached-layout hit-test and caret-geometry helpers.
      - `src/editor/surface.rs`: Convert widget coordinates to layout coordinates and visible offsets to document offsets.
      - `src/editor/buffer.rs`: Expose visible snapshot start byte offset, not only line range, so local/document offset conversion is reliable.
      - `src/editor/viewport.rs`: Preserve visible range metadata needed by coordinate conversion.
    - References:
      - Context7 docs response `ctx7:docs:95021dc4b462504fb49442d4` for Parley cursor, hit-testing, selection, and geometry APIs.
      - `plans/002-Phase1-TextCanvasFoundation.md` compromise on visual-line overflow handling and layout caching.
  - Test Cases to Write:
    - `visible_snapshot_includes_start_byte_offset`: Snapshot metadata can convert local layout offsets to document offsets.
    - `hit_test_clamps_before_and_after_text`: Points before/after text produce valid document offsets.
    - `caret_geometry_is_available_for_visible_caret`: A visible caret offset returns finite geometry suitable for painting.
    - Manual smoke test: Click before, inside, and after text and verify subsequent typing appears at the clicked location.

- [ ] Render caret and keep it visible during edits/navigation
  - Acceptance Criteria:
    - Functional: A visible caret is drawn at the current insertion point when the editor has focus; edits and navigation keep the caret within the visible logical/visual viewport when possible.
    - Performance: Caret painting reuses cached layout/geometry and draws simple Vello primitives without forcing unrelated layout rebuilds.
    - Code Quality: Caret rendering is isolated from buffer mutation and uses explicit colors/metrics; viewport follow logic is renamed or generalized away from append-only document-end behavior.
    - Security: Caret visibility updates are local viewport mutations only and cannot trigger commands, filesystem access, IPC, network, or script execution.
  - Approach:
    - Documentation Reviewed:
      - Parley Context7 docs: cursor geometry can be used to draw caret indicators after layout.
      - Masonry docs/code-search: `PaintCtx::is_focus_target` reports text focus; custom widgets draw into a Vello `Scene`; `request_render` should be called when appearance changes.
      - Vello/Masonry existing project code: `scene.fill` with `kurbo::Rect` is already used for panels/background and can draw a caret rectangle.
    - Options Considered:
      - Draw a fixed-position caret independent of layout: easy but incorrect with wrapping, scrolling, and proportional fonts.
      - Use Parley cursor geometry from cached layout: correct for visible text and prepares for selection rendering.
      - Add caret blinking now: useful UX, but requires animation/timers and is not necessary for the interaction model baseline.
    - Chosen Approach:
      - Draw a non-blinking caret rectangle from cached Parley geometry when focused. Add `ensure_caret_visible` to scroll logical lines or visual offset based on the caret's document offset and layout geometry.
    - API Notes and Examples:
      ```rust
      let caret = Rect::new(x, y, x + 1.5, y + height);
      scene.fill(Fill::NonZero, Affine::IDENTITY, CARET_COLOR, None, &caret);
      ```
    - Files to Create/Edit:
      - `src/editor/layout.rs`: Return caret rectangle/metrics from cached layout.
      - `src/editor/surface.rs`: Paint caret, track focus-aware rendering inputs, and ensure caret visibility after commands.
      - `src/masonry_editor.rs`: Pass focus state or let surface query paint context focus where appropriate.
      - `src/editor/viewport.rs`: Add caret-line ensure-visible helpers if existing methods need generalization.
    - References:
      - Context7 Parley cursor docs response `ctx7:docs:95021dc4b462504fb49442d4`.
      - Masonry docs/code-search for `PaintCtx::is_focus_target`, `request_render`, and Vello scene painting.
  - Test Cases to Write:
    - `ensure_caret_visible_scrolls_to_caret_line`: Moving caret outside the logical viewport adjusts the first visible line.
    - `ensure_caret_visible_preserves_visible_caret`: No scroll occurs when caret is already visible.
    - `caret_paint_geometry_is_clipped_to_text_area`: Geometry used for caret painting lies inside the text clip when visible.
    - Manual smoke test: Move the caret across wrapped and multiline content and verify it remains visible.

- [ ] Add pointer hit-testing for click-to-place-caret
  - Acceptance Criteria:
    - Functional: Primary pointer click focuses the editor, maps the click location to the nearest text offset in the visible layout, moves the caret, clears selection, and requests repaint/accessibility update.
    - Performance: Pointer hit-testing uses cached layout state and bounded visible snapshot text; it does not clone the full buffer or rebuild renderer/window state.
    - Code Quality: Pointer event handling remains in `EditorWidget`, while coordinate-to-offset semantics live in `EditorSurface`/`LayoutState`.
    - Security: Pointer events only change local focus/caret/selection state and cannot trigger commands, filesystem access, IPC, network, or script execution.
  - Approach:
    - Documentation Reviewed:
      - Masonry docs/code-search: `Widget::on_pointer_event`, `PointerEvent::Down`, pointer position access through logical coordinates, `EventCtx::request_focus`, `capture_pointer`, `request_render`, and `set_handled`.
      - Parley Context7 docs: `Cursor::from_point` hit-tests layout coordinates.
    - Options Considered:
      - Move caret only on keyboard navigation: simpler but leaves editor below minimal usability.
      - Approximate line/column from fixed width: incorrect for Parley wrapping/font fallback.
      - Use Parley hit-testing from pointer coordinates: consistent with rendered text and prepares for drag selection.
    - Chosen Approach:
      - On primary pointer down, request focus and call `EditorSurface::place_caret_at_point(logical_point)`. Use existing visual scroll offset and text inset to convert widget coordinates into Parley layout coordinates.
    - API Notes and Examples:
      ```rust
      if let PointerEvent::Down { button: Some(PointerButton::Primary), position, .. } = event {
          ctx.request_focus();
          let changed = self.editor.place_caret_at_point(position.logical_point());
          ctx.request_render();
      }
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`: Handle primary pointer down separately from scroll events.
      - `src/editor/surface.rs`: Add `place_caret_at_point` and clear-selection behavior.
      - `src/editor/layout.rs`: Add public hit-test helper over cached layout.
    - References:
      - Masonry custom widget docs/code-search for pointer event flow and focus requests.
      - Context7 Parley hit-testing docs response `ctx7:docs:95021dc4b462504fb49442d4`.
  - Test Cases to Write:
    - `place_caret_at_point_before_text_moves_to_visible_start`: Click left of first glyph maps to the start of the visible snapshot.
    - `place_caret_at_point_after_text_moves_to_visible_end`: Click after trailing text maps to a valid end offset.
    - Manual smoke test: Click the middle of existing text and type; inserted text appears at the clicked caret.

- [ ] Add basic selection model and selected-range editing
  - Acceptance Criteria:
    - Functional: The editor supports an optional selection range with anchor/focus offsets, Shift+Left/Right extension, pointer drag selection if practical, replacement of selected text on printable input/Enter, and deletion of selected text on Backspace/Delete.
    - Performance: Selection updates mutate small cursor/selection state and reuse cached layout geometry for painting; selected text deletion uses `crop` range deletion.
    - Code Quality: Selection state is represented independently from rendering, normalizes ranges before mutation, and keeps anchor/focus semantics clear for future IPC edit operations.
    - Security: Selection and replacement only mutate local inert text and cannot trigger commands, filesystem access, IPC, network, or script execution.
  - Approach:
    - Documentation Reviewed:
      - Parley Context7 docs: `parley::editing::Selection` supports caret/range/line selections, extension, `text_range`, and selection geometry.
      - `crop` docs/code-search: `Rope::delete` and `Rope::replace` mutate byte ranges and require valid UTF-8 boundaries.
      - Masonry docs/code-search: keyboard modifiers are exposed with keyboard events; pointer capture can support drag selection if needed.
    - Options Considered:
      - Implement only cursor with no selection: enough for typing, but below roadmap Phase 2 and makes delete/replace flows less representative of real editor operations.
      - Implement keyboard-only selection first: simpler and testable; pointer drag can be added if Masonry pointer APIs are straightforward.
      - Implement full word/line multi-click selection: useful later, but too much for this phase.
    - Chosen Approach:
      - Add `SelectionState { anchor, focus }` with normalized selected range helpers. Implement Shift+Left/Right first, selected-range deletion/replacement, and selection highlight painting. Add pointer drag selection if primary-down/move/up and pointer capture are straightforward in Masonry 0.4.
    - API Notes and Examples:
      ```rust
      pub struct SelectionState {
          anchor: usize,
          focus: usize,
      }

      let range = selection.normalized_range();
      rope.replace(range, inserted_text);
      ```
    - Files to Create/Edit:
      - `src/editor/selection.rs`: New selection state and range normalization tests.
      - `src/editor.rs`: Export selection module as needed.
      - `src/editor/surface.rs`: Add selected-range replace/delete and selection-aware movement commands.
      - `src/editor/layout.rs`: Add selection highlight geometry/render support.
      - `src/masonry_editor.rs`: Map Shift+navigation and optional drag selection events.
    - References:
      - Context7 Parley selection docs response `ctx7:docs:95021dc4b462504fb49442d4`.
      - `roadmap.md` Phase 2 basic selection model.
  - Test Cases to Write:
    - `selection_normalizes_anchor_and_focus`: Selected range is stable regardless of drag/navigation direction.
    - `typing_replaces_selected_range`: Printable input replaces selected text and collapses caret after inserted text.
    - `backspace_deletes_selected_range`: Backspace removes selection rather than only the previous character.
    - `shift_left_and_right_extend_selection`: Keyboard selection updates anchor/focus as expected.
    - Manual smoke test: Select text with Shift+arrows, type replacement text, then delete another selection.

- [ ] Preserve viewport, layout cache, accessibility, and large-buffer regressions
  - Acceptance Criteria:
    - Functional: Existing Phase 1 behavior for bounded visible extraction, scrolling, visual wrapped-line overflow, placeholder text, accessibility label updates, resize, focus, and Escape exit still works after cursor/selection changes.
    - Performance: Large-buffer tests continue to demonstrate viewport-bounded extraction and layout cache reuse/invalidation; no new normal paint path uses whole-buffer `to_string()`.
    - Code Quality: New tests cover editor interaction state separately from Masonry smoke behavior; `cargo fmt`, `cargo test`, and `cargo check` pass.
    - Security: Verification remains local and in-memory only; no filesystem mutation beyond normal Cargo build artifacts, no network, no IPC listener, no extension loading, and no script execution are introduced.
  - Approach:
    - Documentation Reviewed:
      - Rust/Cargo standard tooling: `cargo fmt`, `cargo test`, and `cargo check` are the verification baseline established in Phase 0 and Phase 1.
      - Masonry docs/code-search: accessibility updates should be requested when widget state changes and `Role::MultilineTextInput` remains appropriate.
    - Options Considered:
      - Replace Phase 1 tests wholesale: risks losing important viewport/cache regressions.
      - Preserve and extend existing tests: keeps foundation guarantees while adding interaction coverage.
      - Add formal benchmarks now: useful later but not necessary for Phase 2 acceptance.
    - Chosen Approach:
      - Update existing tests only where semantics intentionally change from append-only to caret-based behavior. Add interaction-focused unit tests and keep manual GUI smoke testing as the final check.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test
      cargo check
      cargo run
      ```
    - Files to Create/Edit:
      - `src/editor/buffer.rs`: Update existing append/backspace tests for caret-aware APIs while preserving viewport extraction tests.
      - `src/editor/layout.rs`: Preserve cache and visual-line tests; add caret/selection geometry tests where practical.
      - `src/editor/surface.rs`: Update editor behavior tests for caret/selection semantics.
      - `src/editor/viewport.rs`: Preserve viewport tests and add caret-visible cases if not added earlier.
      - `src/masonry_editor.rs`: Preserve focus/accessibility behavior.
      - `plans/003-Phase2-EditorInteractionModel.md`: Mark completed tasks and fill post-implementation notes after verification.
    - References:
      - `plans/002-Phase1-TextCanvasFoundation.md` large-buffer and cache-regression tasks.
      - `roadmap.md` Phase 2 expected outcome.
  - Test Cases to Write:
    - `large_buffer_visible_extraction_remains_bounded_after_cursor_changes`: Interaction state does not force full-buffer paint extraction.
    - `layout_cache_invalidates_on_caret_relevant_viewport_change_only_when_needed`: Caret/selection paint changes do not unnecessarily rebuild text layout unless text/range/width/font changes.
    - `accessibility_label_updates_after_caret_edit`: Accessibility label reflects visible text after middle insertion/deletion.
    - `phase2_regression_commands`: `cargo fmt`, `cargo test`, and `cargo check` all pass.
    - Manual GUI smoke test: launch, type, click to move caret, edit middle text, navigate with arrows/Home/End, select/replace/delete text, scroll wrapped and multiline content, resize, Backspace/Delete, and Escape exit.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
