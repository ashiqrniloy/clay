# UI Observability and SDUI Structural Regression

Clay's Phase 15 UI regression coverage is intentionally structural and headless. It validates the server-driven UI (SDUI) editor/sidebar composition by inspecting typed observable state instead of comparing rendered pixels.

## Structural layout regression in Clay

"Structural layout regression" means tests assert the UI semantics and layout-adjacent facts that must remain stable across rendering backends:

- which SDUI node kinds are present (`Panel`, `Label`, `Button`, `List`, `EditorView`, `Flex`, and `Stack`);
- panel titles, visible label text, button labels, and list item IDs/labels;
- editor-view bindings such as `document_id` and `expected_version`;
- layout summary facts that can be computed without a real window, such as whether a sidebar is present and whether the editor region is non-empty;
- accessibility role/label coverage for rendered SDUI nodes; and
- GUI chrome status text and runtime diagnostics.

This coverage is designed to run under normal test commands without opening a window, allocating a GPU surface, or depending on platform font/rasterization behavior. Pixel-buffer / GPU snapshots remain deferred after the Phase 20 Masonry 0.4 revisit: `masonry_testing::TestHarness` / `assert_render_snapshot` exist, but the harness hardcodes Vello `use_cpu: true`, so goldens would not exercise Clay's production GPU path and would still be brittle across fonts/DPI/AA. See `decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`.

## Observable state used by tests

SDUI structural tests use `SduiObservableSnapshot` from `src/masonry_sdui.rs`. A test applies a representative SDUI snapshot or update to `SduiNativeState`, calls `observable_snapshot(...)`, and compares the resulting typed fields. This lets tests assert exact UI structure while avoiding screenshots and golden image files.

GUI status tests use `SduiStatusObservation` from `src/masonry_editor.rs`. `EditorWidget::status_observation()` returns the status line, connection label, access label, latest sync version, any active runtime diagnostic message, compact active `theme_label`, dirty/display-name markers, composing flag, pending-edit count, and sanitized recovery summary without painting or starting a window. Accessibility labels are composed by `src/editor/accessibility.rs` and stay consistent with that observation, including `Theme …`, `Composing.`, dirty/display-name, and recovery markers. SDUI/shell roots publish `Role::Group`; transient menus publish `Role::Menu` / `Role::MenuItem`. Phase 20 save/conflict recovery menus reuse the same transient-menu accessibility path when `StaleFileMetadata` or `DirtyDocument` failures arrive. Pending-edit depth, edit rejections, disconnect reconnect guidance, and explicit `clientRequestResync` / `clientDismissRecovery` recovery menus also surface through the same status/accessibility channel rather than stderr-only diagnostics.

Markdown mode uses the same structural strategy. The deterministic `markdown-mode` fixture publishes a `Markdown Preview` panel with mode, parse, decorations, preview, and toggle-action text; `markdown_structural_sdui_snapshot_matches_fixture` verifies those visible labels through `SduiNativeState` without a window or GPU surface. Phase 18.5 also keeps large-file Markdown status structural and inert: package SDUI can report `full`, `windowed`, `degraded`, or `plain-text-fallback` highlighting with sanitized fixed strings, including a policy label for partial/viewport-only highlighting and plain-text fallback. This is the regression layer for the Markdown preview/status workflow.

The main focused command for this coverage is:

```text
cargo test -p clay --lib masonry_sdui
```

Useful adjacent checks include:

```text
cargo test -p clay --lib masonry_editor
cargo test --all-targets
```

## Relationship to window-driver smoke coverage

Manual and app-managed window smoke validation remains documented in [Launch and GUI Smoke Validation](launch-and-gui-smoke.md). Use `cargo run -- smoke-gui` when you need to observe actual native window behavior, local IPC startup, runtime-backed SDUI publication, or GUI status text in a real desktop session.

The automated structural tests do not replace window smoke runs. They provide deterministic coverage for tree shape, update handling, accessibility labels, status observations, and payload guardrails; window smoke validates that those states are visible through the native application shell.

Plan 087's repeatable live review wrapper is `scripts/capture-ui-review.sh`. It
uses the existing `clay server`/`clay client` launch path with a mode-700
private root, fixed `900×600` logical window request, fixture-only documents,
and bounded cleanup. It stores a portal PNG plus a Clay-only AT-SPI dump under
a caller-selected artifact directory. Missing desktop capture prerequisites
produce `review.status`=`UNRESOLVED` and exit 2; they never weaken structural
CI coverage or become a false visual pass. See [Launch and GUI Smoke
Validation](launch-and-gui-smoke.md#repeatable-ui-review-harness-plan-087-task-2)
for fixture names, interaction checkpoints, and output files.

## Deferred GPU-backed pixel snapshot path

Phase 20 revisited Masonry 0.4 `TestHarness` / `assert_render_snapshot` and **re-deferred** pixel / GPU snapshots with evidence (`decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`):

- Masonry now has a headless capture path, but `TestHarness` forces `use_cpu: true`. That is intentional for CPU determinism and does **not** match Clay's production `masonry_winit` renderer (`use_cpu: false`, wgpu texture blit).
- Clay therefore does not depend on `masonry_testing`, does not land golden PNGs, and keeps structural observability as the hard CI layer.
- Renderer upgrades that could improve GPU fidelity (newer Vello/Parley/wgpu) remain blocked until a newer Masonry release; Masonry 0.4.0 is still the latest published line.

Promote GPU-backed pixel snapshots only when all of the following exist:

1. a deterministic offscreen render target that works in CI without an interactive desktop and can run the **production GPU** path (`use_cpu: false`) or an explicitly accepted GPU-faithful alternative;
2. fixed font, DPI, scale-factor, theme, and window-size inputs;
3. stable screenshot capture for the native SDUI editor/sidebar composition;
4. golden image review/update workflow with clear commands and platform expectations;
5. tolerance rules for antialiasing or backend differences; and
6. security review confirming that the render harness does not open remote listeners, read user documents, expose secrets, or grant client filesystem/shell authority.

Until those prerequisites exist, structural snapshots are the hard automated regression layer and GPU-backed pixel snapshots remain deferred follow-up work. GPU-backed pixel snapshots remain deferred for Markdown mode as well; its automated smoke coverage stays structural, typed, headless, and independent of platform font/rasterization differences.

## Performance and payload context

SDUI payload size guardrails are tracked in [Performance Fixtures and Baseline Workflow](performance.md#sdui-payload-budget-findings). The observability tests should stay cheap, typed, and headless; they must not add synchronous JavaScript, IPC, filesystem, GPU, or window work to the ordinary typing/rendering hot path. Markdown large-file policy/status decisions are load/open/reload/configuration or explicit viewport-refresh work; paint reads already-validated SDUI and decoration state only.
