# UI Observability and SDUI Structural Regression

Clay's UI regression coverage is intentionally structural and headless. It
validates the server-driven UI (SDUI) editor/sidebar/package composition by
inspecting typed observable state instead of comparing rendered pixels. After
the Plan 097 cutover, the rendered client is the Tauri v2 + React shell, so
the hard structural layer is the frontend Vitest suites plus the retained
server-side SDUI snapshot validation; pixel goldens remain deferred.

## Structural layout regression in Clay

"Structural layout regression" means tests assert the UI semantics and layout-adjacent facts that must remain stable across rendering details:

- which SDUI node kinds are present (`Panel`, `Label`, `Button`, `List`, `EditorView`, `Flex`, `Stack`, plus the package catalog extensions such as `Dropdown`/`Modal`/`TextInput`);
- panel titles, visible label text, button labels, and list item IDs/labels;
- editor-view bindings such as `document_id` and `expected_version`;
- layout facts computable without a real desktop, such as slot composition (top/left/right/bottom/status), sidebar presence, and non-empty editor regions;
- accessibility role/label coverage for rendered nodes; and
- shell status text and runtime diagnostics.

This coverage runs under normal test commands without opening a window,
allocating a GPU surface, or depending on platform font/rasterization
behavior. GPU-backed pixel snapshots remain deferred: golden images would be
brittle across fonts/DPI/AA/backends and add no authority guarantees. The
pre-cutover native `TestHarness` revisit and re-deferral rationale is
recorded in `decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`
(historical).

## Observable state used by tests

Current-state structural coverage:

- Frontend component suites (`frontend/src/test/*.test.tsx`,
  `frontend/src/command-centre`, `frontend/src/settings`, `frontend/src/sdui`)
  render real React surfaces under jsdom and assert tree shape, labels, roles,
  provenance, typed action payloads, and state preservation across
  reconciliation.
- Server-side SDUI snapshot validation (`src/server/ui.rs`,
  `src/protocol/runtime.rs`) validates trees, slots, visibility, action
  targets, duplicate IDs, and bounded payload sizes before anything reaches a
  client; the runtime suite pins these invariants.
- The generation-stamped wire snapshot carries host-stamped provenance and
  trust-domain labels so tests can assert exactly what the renderer will
  project (`docs/wiki/modules/react-sdui-package-ui.md`).

Status and diagnostics remain server-owned: runtime diagnostic events carry
sanitized messages to the workspace controller, which surfaces them through
the footer status live region (`role="status"`); recovery menus (pending-edit
depth, edit rejections, disconnect guidance, explicit resync/dismiss) surface
through server-owned menu snapshots rather than stderr-only diagnostics.

Markdown mode uses the same structural strategy (`markdown_structural_sdui_snapshot_matches_fixture`). The deterministic `markdown-mode` fixture publishes a `Markdown Preview` panel with mode, parse, decorations, preview, and toggle-action text; fixture captures verify those visible labels without a GPU surface. Large-file Markdown status stays
structural and inert: package SDUI can report `full`, `windowed`, `degraded`,
or `plain-text-fallback` highlighting with sanitized fixed strings, including
a policy label for partial/viewport-only highlighting and plain-text fallback.

The main focused commands for this coverage are:

```text
npm --prefix frontend run test
cargo test --all-targets
```

## Relationship to desktop smoke coverage

Manual and app-managed smoke validation remains documented in [Launch and GUI Smoke Validation](launch-and-gui-smoke.md). Launch the Tauri desktop (`clay`) when you need to observe actual webview behavior, local IPC startup, runtime-backed SDUI publication, or status text in a real desktop session.

The automated structural tests do not replace desktop smoke runs. They provide deterministic coverage for tree shape, update handling, accessibility labels, status observations, and payload guardrails; a desktop run validates that those states are visible through the real application shell.

Plan 087's repeatable live review wrapper is `scripts/capture-ui-review.sh`.
It launches the current Tauri build with a mode-700 private root, fixed
fixture-only documents, and bounded cleanup. It stores a portal PNG plus a
Clay-only AT-SPI dump under a caller-selected artifact directory. Missing
desktop capture prerequisites produce `review.status`=`UNRESOLVED` and exit 2;
they never weaken structural CI coverage or become a false visual pass. See
[Launch and GUI Smoke Validation](launch-and-gui-smoke.md#repeatable-ui-review-harness-plan-087-task-2)
for fixture names, interaction checkpoints, and output files.

Known AT-SPI ceiling on the current stack: WebKitGTK does not expose static
text inside the footer/live region as accessible names or Text-interface
content, so name-based dumps cannot see connection/diagnostic status text.
Interactive keyboard states additionally require a TTY this host cannot
provide; both ceilings are recorded in `test-plan/index.md`.

## Deferred pixel snapshot path

Pixel/GPU snapshots stay deferred. Prerequisites before promoting them:

1. a deterministic offscreen render target that works in CI without an interactive desktop and exercises the production webview rendering;
2. fixed font, DPI, scale-factor, theme, and window-size inputs;
3. stable screenshot capture for the SDUI editor/sidebar composition;
4. golden image review/update workflow with clear commands and platform expectations;
5. tolerance rules for antialiasing or backend differences; and
6. security review confirming that the render harness does not open remote listeners, read user documents, expose secrets, or grant client filesystem/shell authority.

Until then, structural snapshots are the hard automated regression layer — including Markdown mode, whose automated smoke coverage stays structural, typed, headless, and independent of platform font/rasterization differences.

## Performance and payload context

SDUI payload size guardrails are tracked in [Performance Fixtures and Baseline Workflow](performance.md#sdui-payload-budget-findings). The observability tests should stay cheap, typed, and headless; they must not add synchronous JavaScript, IPC, filesystem, GPU, or window work to the ordinary typing/rendering hot path. Markdown large-file policy/status decisions are load/open/reload/configuration or explicit viewport-refresh work; renders read already-validated SDUI and decoration state only.
