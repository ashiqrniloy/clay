# Phase 1 Text Canvas Foundation

## Objectives
- Stabilize the Phase 0 native editor prototype into a maintainable text canvas foundation.
- Remove whole-buffer rendering assumptions by introducing viewport/range-based text extraction over the `crop` rope.
- Add explicit layout dirty-state caching so Parley layout is rebuilt only when required.
- Keep the implementation single-process and local while preparing the client-side shadow-state boundary needed by later IPC phases.

## Expected Outcome
- `cargo run` still opens the native Clay editor window and supports typing, Backspace, resize, focus, and Escape exit.
- Editor code has clear buffer, viewport, layout, painting, and Masonry widget boundaries.
- Visible text is derived from a bounded rope range rather than unconditional whole-rope materialization for paint.
- Parley layout is cached and invalidated on text, viewport, width, or font changes instead of rebuilt unconditionally during every paint.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- No IPC, `deno_core`, SDUI protocol, server process, extension loading, filesystem access, or network access is introduced in this phase.

## Tasks

- [x] Refactor editor internals into explicit buffer, viewport, and render/layout modules
  - Acceptance Criteria:
    - Functional: Existing Phase 0 behavior remains unchanged from the user's perspective: launch, type printable text, Backspace, resize, focus, accessibility label update, and Escape exit still work.
    - Performance: The refactor does not add new per-key blocking work or per-frame renderer/window lifecycle work.
    - Code Quality: `EditorBuffer`, viewport state, layout cache state, and Masonry widget event routing are separated into small modules with narrow public APIs.
    - Security: The refactor does not add filesystem, network, shell, extension, IPC listener, or script execution behavior.
  - Approach:
    - Documentation Reviewed:
      - Masonry 0.4 docs.rs/code-search: `Widget` methods, `PaintCtx`, `EventCtx`, `request_render`, `request_layout`, `request_accessibility_update`, and custom widget examples.
      - Masonry 0.4 `PaintCtx::text_contexts`: exposes shared Parley `FontContext` and `LayoutContext` for custom text editing widgets.
      - Repository `plans/001-Phase0-NativeTextCanvas.md`: Masonry now owns the `winit` event loop and Vello/wgpu lifecycle; custom editor module owns rope/layout/painting behavior.
    - Options Considered:
      - Keep all editor internals in `src/editor.rs`: minimal churn, but makes viewport and layout cache work harder to isolate.
      - Split into many fine-grained files immediately: clear boundaries, but risks overfitting before the editor model is known.
      - Use a small, conservative split around buffer, viewport, layout, and surface: enough structure for Phase 1 without speculative server abstractions.
    - Chosen Approach:
      - Keep Masonry ownership in `src/masonry_editor.rs`, keep app startup in `src/main.rs`, and split `src/editor.rs` into focused editor modules. Avoid introducing client/server names until IPC work begins.
    - API Notes and Examples:
      ```rust
      impl Widget for EditorWidget {
          fn on_text_event(&mut self, ctx: &mut EventCtx<'_>, _props: &mut PropertiesMut<'_>, event: &TextEvent) {
              if self.editor.handle_text_event(event) {
                  ctx.request_render();
                  ctx.request_accessibility_update();
              }
          }

          fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
              self.editor.paint(ctx, scene);
          }
      }
      ```
    - Files to Create/Edit:
      - `src/editor.rs`: Keep or convert to module root for editor-facing API.
      - `src/editor/buffer.rs`: Move rope-backed buffer logic here.
      - `src/editor/viewport.rs`: Add viewport state types.
      - `src/editor/layout.rs`: Add layout cache state and visible layout preparation.
      - `src/editor/surface.rs`: Keep editor painting/orchestration here if `src/editor.rs` becomes a module root.
      - `src/masonry_editor.rs`: Update imports and preserve Masonry widget boundary.
      - `src/main.rs`: Update test module imports if editor modules move.
    - References:
      - `concept.md` sections 3 and 4 on the client canvas, `crop`, Parley, Vello, and client shadow state.
      - `plans/001-Phase0-NativeTextCanvas.md` compromises on Masonry ownership and deferred layout caching.
      - Masonry docs.rs/code-search results for `Widget`, `PaintCtx`, custom widget examples, and pass-request APIs.
  - Test Cases to Write:
    - Existing editor buffer append/backspace tests continue to pass after module split.
    - Existing printable text filter test continues to pass after module split.
    - Manual smoke test: launch, type text, Backspace, resize, and Escape exit still work.

- [ ] Add viewport state and bounded visible text extraction over `crop`
  - Acceptance Criteria:
    - Functional: The editor can compute a visible text snapshot from a bounded line range, including a small configurable overscan, and painting uses that snapshot instead of unconditional whole-rope `to_string()`.
    - Performance: Visible text extraction is bounded by the selected viewport/overscan line range for non-empty documents; whole-buffer materialization is only allowed in tests or explicit debug helpers.
    - Code Quality: Byte offsets used with `crop` are derived through rope line APIs and remain on valid UTF-8 boundaries; range extraction is covered by unit tests.
    - Security: Extracted text is still inert display text and is not interpreted as markup, paths, commands, or scripts.
  - Approach:
    - Documentation Reviewed:
      - `crop` 0.4.3 docs.rs/code-search: `Rope::line_len`, `Rope::byte_of_line`, `Rope::line_slice`, `Rope::byte_slice`, `RopeSlice::chunks`, and `Rope`'s byte-offset indexing model.
      - `crop` docs.rs/code-search: `crop` uses UTF-8 byte offsets, tracks LF/CRLF line breaks, and supports rope slices as borrowed views into a rope.
      - `concept.md`: future client owns a lightweight shadow copy of visible text, while the server owns canonical state.
    - Options Considered:
      - Use byte ranges directly from scroll pixels: precise eventually, but premature before cursor/hit-testing and line metrics exist.
      - Use line ranges first: maps cleanly to `crop` APIs and prepares for viewport virtualization without depending on full editor interaction work.
      - Keep `visible_text() -> String` over the full rope: simplest but preserves the main Phase 0 compromise.
    - Chosen Approach:
      - Introduce `Viewport` with first visible line, estimated visible line count, and overscan. Convert line range to byte offsets with `Rope::byte_of_line`, then materialize only that `RopeSlice` for Parley layout.
    - API Notes and Examples:
      ```rust
      use crop::Rope;

      let start_line = viewport.first_line();
      let end_line = (start_line + viewport.visible_line_count() + viewport.overscan_lines())
          .min(rope.line_len());
      let start_byte = rope.byte_of_line(start_line);
      let end_byte = rope.byte_of_line(end_line);
      let visible = rope.byte_slice(start_byte..end_byte).to_string();
      ```
    - Files to Create/Edit:
      - `src/editor/buffer.rs`: Add line-count, line-to-byte, and visible-range APIs.
      - `src/editor/viewport.rs`: Add viewport state, overscan settings, and range calculation.
      - `src/editor/surface.rs` or `src/editor.rs`: Use viewport snapshot for paint/accessibility where appropriate.
      - `src/main.rs`: Move or update tests for bounded extraction.
    - References:
      - `crop` 0.4.3 docs.rs/code-search results for `Rope`, `RopeSlice`, line slicing, byte slicing, and line/byte offset conversion.
      - `roadmap.md` Phase 1 notes on replacing whole-buffer rendering assumptions.
  - Test Cases to Write:
    - `visible_snapshot_limits_to_requested_lines`: A multi-line buffer returns only the selected line range plus overscan.
    - `visible_snapshot_clamps_past_document_end`: Viewport ranges beyond the document do not panic and clamp to available lines.
    - `visible_snapshot_preserves_utf8_boundaries`: Unicode text sliced by line APIs remains valid and complete.
    - `empty_buffer_visible_snapshot_is_empty`: Empty documents produce an empty snapshot and keep placeholder behavior in paint.

- [ ] Add basic viewport scrolling and resize-derived visible line estimation
  - Acceptance Criteria:
    - Functional: Wheel or trackpad scrolling adjusts the first visible line, clamps within the document, requests repaint, and changes the displayed line window for multi-line buffers.
    - Performance: Scrolling updates viewport counters and bounded visible extraction only; it does not clone the full rope or rebuild unrelated application state.
    - Code Quality: Pointer/wheel event handling remains in the Masonry widget boundary and delegates viewport mutation to the editor surface through a small method.
    - Security: Scroll events only mutate local viewport state and cannot trigger commands, file access, IPC, network, or script execution.
  - Approach:
    - Documentation Reviewed:
      - Masonry 0.4 docs.rs/code-search: `Widget::on_pointer_event`, `EventCtx::request_render`, and custom widget pass flow.
      - Parley Context7 `/linebender/parley`: layout lines expose metrics such as baselines and line metrics, which can later replace estimated line height.
      - Current code in `src/masonry_editor.rs`: pointer events already request focus; text events already request render/accessibility updates on mutation.
    - Options Considered:
      - Implement pixel-perfect scroll using Parley line metrics immediately: closer to final behavior but entangles Phase 1 with cursor/hit-testing work.
      - Implement line-based scroll first: sufficient to prove viewport extraction and avoids overcomplication.
      - Defer scrolling entirely: bounded extraction would be hard to validate manually.
    - Chosen Approach:
      - Add line-based scroll state using the known text font size and line-height multiplier. Estimate visible line count from widget height and clamp the first visible line after edits/resizes.
    - API Notes and Examples:
      ```rust
      let line_height = TEXT_FONT_SIZE as f64 * 1.4;
      let visible_lines = ((available_height / line_height).ceil() as usize).max(1);
      viewport.set_visible_line_count(visible_lines);
      viewport.scroll_lines(delta_lines, buffer.line_len());
      ```
    - Files to Create/Edit:
      - `src/editor/viewport.rs`: Add line-count updates, scroll methods, and clamping.
      - `src/editor/surface.rs` or `src/editor.rs`: Add public scroll/resize viewport methods.
      - `src/masonry_editor.rs`: Handle wheel/scroll pointer events if available through Masonry's `PointerEvent` API and request repaint on viewport change.
    - References:
      - Masonry docs.rs/code-search results for widget event methods and `request_render`.
      - Parley Context7 docs response `ctx7:docs:4a3c5d29997c6029f0cdd2a9` for line metrics and layout line iteration.
  - Test Cases to Write:
    - `viewport_scroll_clamps_to_document_start`: Negative/upward scroll at the top leaves first visible line at zero.
    - `viewport_scroll_clamps_to_document_end`: Downward scroll cannot move beyond the last useful line window.
    - `viewport_visible_line_count_updates_from_height`: Height changes produce expected minimum and larger visible-line counts.
    - Manual smoke test: Create enough lines, scroll, and confirm displayed text window changes.

- [ ] Introduce Parley layout cache and explicit invalidation
  - Acceptance Criteria:
    - Functional: Text still renders correctly, placeholder rendering still works, and layout is invalidated when visible text, available width, viewport range, or font state changes.
    - Performance: Repeated paint calls with unchanged text, width, viewport, and fonts reuse the cached layout instead of rebuilding it unconditionally.
    - Code Quality: Cache keys and invalidation reasons are explicit and testable; layout cache code does not leak Masonry widget details into `EditorBuffer`.
    - Security: Cached layout contains only inert text/glyph layout state and does not cache or execute external resources.
  - Approach:
    - Documentation Reviewed:
      - Masonry 0.4 docs.rs/code-search: `PaintCtx::text_contexts` provides Parley contexts; `PaintCtx::fonts_changed()` indicates cached text layouts should be invalidated when loaded fonts change.
      - Parley Context7 `/linebender/parley`: use `LayoutContext::ranged_builder`, style defaults, `build`, `break_all_lines`, `align`, line/glyph iteration, and selection/hit-testing APIs for later phases.
      - Masonry `render_text` docs.rs/code-search: renders a `Layout<BrushIndex>` into a Vello `Scene` with a transform and brush list.
    - Options Considered:
      - Keep rebuilding during paint: acceptable for Phase 0 but not a stable text canvas foundation.
      - Cache only the visible string: reduces rope extraction but still rebuilds Parley layout each paint.
      - Cache the Parley layout keyed by visible range/text revision and width: addresses the Phase 0 compromise while keeping invalidation understandable.
    - Chosen Approach:
      - Track an editor text revision incremented by edits, viewport revision incremented by scroll/resize changes, last layout width, and a fonts-changed flag from Masonry. Rebuild Parley layout only when the cache key changes.
    - API Notes and Examples:
      ```rust
      let should_rebuild = cache.text_revision != text_revision
          || cache.viewport_revision != viewport_revision
          || cache.max_width != max_width
          || ctx.fonts_changed();

      if should_rebuild {
          let (font_context, layout_context) = ctx.text_contexts();
          let mut builder = layout_context.ranged_builder(font_context, visible_text, 1.0, true);
          builder.push_default(StyleProperty::FontSize(TEXT_FONT_SIZE));
          let mut layout = builder.build(visible_text);
          layout.break_all_lines(Some(max_width));
          layout.align(Some(max_width), TextAlign::Start, TextAlignOptions::default());
          cache.store(layout, visible_text, text_revision, viewport_revision, max_width);
      }
      ```
    - Files to Create/Edit:
      - `src/editor/layout.rs`: Add layout cache, cache key, invalidation, and render helper.
      - `src/editor/surface.rs` or `src/editor.rs`: Use layout cache during paint.
      - `src/editor/buffer.rs`: Add text revision tracking if not already added.
      - `src/editor/viewport.rs`: Add viewport revision tracking if not already added.
    - References:
      - Masonry docs.rs/code-search results for `PaintCtx::text_contexts`, `PaintCtx::fonts_changed`, and `render_text`.
      - Parley Context7 docs response `ctx7:docs:4a3c5d29997c6029f0cdd2a9` for building/breaking/aligning layouts and line metrics.
      - `plans/001-Phase0-NativeTextCanvas.md` compromise on Parley layout rebuilding during paint.
  - Test Cases to Write:
    - `layout_cache_reuses_unchanged_key`: Same text revision, viewport revision, width, and font state do not trigger rebuild.
    - `layout_cache_invalidates_on_text_revision`: Text edits invalidate cached layout.
    - `layout_cache_invalidates_on_width_change`: Resize width changes invalidate cached layout.
    - `layout_cache_invalidates_on_viewport_revision`: Scrolling invalidates cached visible layout.
    - Manual smoke test: Resize and edit text while confirming rendering remains correct.

- [ ] Add large-buffer and regression checks for the Phase 1 foundation
  - Acceptance Criteria:
    - Functional: A large multi-line in-memory buffer can be used in unit tests to verify bounded extraction, viewport clamping, and layout cache invalidation without changing runtime startup behavior.
    - Performance: Tests assert or otherwise demonstrate that paint-facing visible extraction is bounded by viewport size rather than total document size.
    - Code Quality: Test helpers are local to test modules or clearly marked as debug/test-only; production APIs do not expose whole-buffer materialization for paint.
    - Security: Large-buffer tests generate in-memory text only and do not perform filesystem, network, shell, extension, or IPC operations.
  - Approach:
    - Documentation Reviewed:
      - Rust/Cargo standard tooling: `cargo fmt`, `cargo test`, and `cargo check` are the project verification baseline established in Phase 0.
      - `crop` docs.rs/code-search: `RopeBuilder` can build larger ropes incrementally; `Rope` clones are cheap due to shared storage, but production paint should still use bounded slices.
    - Options Considered:
      - Add benchmarks now: useful later, but Phase 1 needs regression tests more than formal benchmarks.
      - Add deterministic unit tests with large generated strings: enough to guard the key compromise without extra dependencies.
      - Depend on manual testing only: insufficient for future refactors.
    - Chosen Approach:
      - Add generated multi-line buffer tests that validate viewport extraction lengths/ranges and cache invalidation counters or observable state. Run the standard Rust verification commands.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test
      cargo check
      ```
      ```rust
      let text = (0..10_000)
          .map(|line| format!("line {line}\n"))
          .collect::<String>();
      let buffer = EditorBuffer::from_text(&text);
      let snapshot = buffer.visible_snapshot(viewport.range(buffer.line_len()));
      assert!(snapshot.text.len() < text.len());
      ```
    - Files to Create/Edit:
      - `src/editor/buffer.rs`: Add test-only constructors/helpers if needed.
      - `src/editor/viewport.rs`: Add viewport unit tests.
      - `src/editor/layout.rs`: Add cache invalidation unit tests.
      - `src/main.rs`: Remove or relocate Phase 0 tests if module split requires it.
      - `plans/002-Phase1-TextCanvasFoundation.md`: Mark completed tasks and fill post-implementation notes after verification.
    - References:
      - Phase 0 verification baseline in `plans/001-Phase0-NativeTextCanvas.md`.
      - `roadmap.md` Phase 1 expected outcome.
  - Test Cases to Write:
    - `large_buffer_visible_extraction_is_bounded`: A 10,000-line buffer produces a visible snapshot much smaller than the full document for a small viewport.
    - `large_buffer_scroll_changes_snapshot`: Scrolling changes the visible snapshot without changing the full buffer.
    - `phase1_regression_commands`: `cargo fmt`, `cargo test`, and `cargo check` all pass.
    - Manual GUI smoke test: launch, type, resize, scroll multi-line content, Backspace, and Escape exit.

## Compromises Made
- Completed the initial module split without changing runtime behavior; `Viewport` and `LayoutState` are explicit module-owned state boundaries, while bounded extraction, scrolling, and layout caching remain deferred to the later Phase 1 tasks that define those behaviors.

## Further Actions
- Continue with the remaining Phase 1 tasks in order: bounded `crop` visible extraction, line-based scrolling/resize estimation, Parley layout cache invalidation, and large-buffer regression checks.
