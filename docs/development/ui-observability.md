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

This coverage is designed to run under normal test commands without opening a window, allocating a GPU surface, or depending on platform font/rasterization behavior. Pixel-buffer snapshots are deferred because they would currently be brittle across operating systems and require a reliable Masonry/winit render target that can run in CI without an interactive desktop session.

## Observable state used by tests

SDUI structural tests use `SduiObservableSnapshot` from `src/masonry_sdui.rs`. A test applies a representative SDUI snapshot or update to `SduiNativeState`, calls `observable_snapshot(...)`, and compares the resulting typed fields. This lets tests assert exact UI structure while avoiding screenshots and golden image files.

GUI status tests use `SduiStatusObservation` from `src/masonry_editor.rs`. `EditorWidget::status_observation()` returns the status line, connection label, access label, latest sync version, and any active runtime diagnostic message without painting or starting a window.

Markdown mode uses the same structural strategy. The deterministic `markdown-mode` fixture publishes a `Markdown Preview` panel with mode, parse, decorations, preview, and toggle-action text; `markdown_structural_sdui_snapshot_matches_fixture` verifies those visible labels through `SduiNativeState` without a window or GPU surface. This is the Phase 18 regression layer for the Markdown preview/status workflow.

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

## Deferred GPU-backed pixel snapshot path

If Masonry/winit later provides reliable headless rendering support, Clay can add GPU-backed pixel snapshot tests as a separate layer. That path should include:

1. a deterministic offscreen render target that works in CI without an interactive desktop;
2. fixed font, DPI, scale-factor, theme, and window-size inputs;
3. stable screenshot capture for the native SDUI editor/sidebar composition;
4. golden image review/update workflow with clear commands and platform expectations;
5. tolerance rules for antialiasing or backend differences; and
6. security review confirming that the render harness does not open remote listeners, read user documents, expose secrets, or grant client filesystem/shell authority.

Until those prerequisites exist, structural snapshots are the hard automated regression layer and GPU-backed pixel snapshots remain deferred follow-up work. GPU-backed pixel snapshots remain deferred for Markdown mode as well; its automated smoke coverage stays structural, typed, headless, and independent of platform font/rasterization differences.

## Performance and payload context

SDUI payload size guardrails are tracked in [Performance Fixtures and Baseline Workflow](performance.md#sdui-payload-budget-findings). The observability tests should stay cheap, typed, and headless; they must not add synchronous JavaScript, IPC, filesystem, GPU, or window work to the ordinary typing/rendering hot path.
