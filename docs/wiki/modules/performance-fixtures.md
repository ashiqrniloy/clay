# Performance Fixtures and Traces

## Source

- `src/perf/fixtures.rs` — deterministic fixture generation and output-path validation.
- `src/perf/baselines.rs` — reusable protocol/server benchmark inputs.
- `src/perf/budgets.rs` — shared payload, latency, memory, parser, and render bounds.
- `src/perf/metrics.rs` — opt-in recorder, summaries, sanitization, and atomic report files.
- `src/cli.rs` — `perf-fixture` and profiling flag wiring.
- `frontend/src/editor/{performance,position-index}.ts` — browser trace recorder and indexed editor work.
- `frontend/src/editor/{performance.test.ts,position-map.test.ts}` and `frontend/src/editor/extensions/performance.test.ts` — deterministic frontend invariants.
- `src/server/{parse_coordinator,syntax_session}.rs` and `src-tauri/src/bridge/forwarder.rs` — Plan 099 scheduler/bridge stages.
- `scripts/editor-performance-smoke.sh` — real Tauri/WebKit report harness.
- `tests/{perf_fixtures,performance_budgets,editor_performance}.rs` — fixture, budget, and protocol matrix tests.
- `benches/protocol_server_baselines.rs` — current Criterion protocol/server baseline target.
- `tools/bench/markdown-parser.mjs` — separate Markdown parser/adapter benchmark harness.
- `docs/development/performance.md` — authoritative commands, budgets, and recorded evidence.

## Overview

This module supplies production-shaped, reproducible inputs for editor and
protocol performance work without reading user documents. Fixtures are valid
UTF-8 and exact-sized. Metrics are disabled by default; when explicitly enabled
they retain bounded numeric snapshots and write sanitized per-process summaries
for the Plan 099 desktop harness.

The code wiki distinguishes deterministic invariants from machine-dependent
latency. CI blocks work counts, ownership, bounds, protocol correctness, and
report integrity. p95 device timing remains advisory until the documented
three-run designated-device rule is satisfied.

## Responsibilities

- Generate large plain-text fixtures with controlled line shapes and Unicode.
- Refuse fixture output outside approved repository roots.
- Provide reusable in-memory protocol/document benchmark helpers.
- Record browser, CodeMirror, bridge, server, and syntax stages only when
  profiling is enabled.
- Keep reports source-free, path-sanitized, bounded, and atomically written.
- Provide the Plan 099 automated matrix and real-device harness without adding
  runtime work to normal editing.

## Fixture generation

`FixtureSpec` combines `FixtureKind`, exact `size_bytes`, and a deterministic
seed. `generate_fixture` streams through a 64 KiB buffer; the CLI writes only
under `target/perf-fixtures/` or the committed test-fixture root
`tests/fixtures/perf/`. Parent traversal and unrelated paths are rejected.

Supported shapes:

- `long-lines` — long lines for layout and intra-line conversion ceilings;
- `many-short-lines` — line-count, viewport, and scrolling pressure;
- `mixed-unicode` — multi-byte scalars, emoji, and combining-width cases; and
- `newline-heavy` — blank-line and line-boundary pressure.

If a generated line would exceed the requested size, ASCII fill reaches the
exact byte count without splitting UTF-8. `default_fixture_path` and
`FixtureKind::parse` keep CLI naming deterministic.

```bash
cargo run --bin clay -- perf-fixture \
  --kind mixed-unicode --size-mib 10 --seed 9001 \
  --output target/perf-fixtures/perf-10mib-mixed-unicode.txt
```

## Recorder and report format

`PerfConfig` enables profiling from `CLAY_PERF_PROFILE=1` or the developer
`--profile-perf` path. A disabled `PerfRecorder` has no backing buffer, so hooks
remain no-ops. An enabled recorder stores at most
`PERF_SNAPSHOT_CAPACITY = 4,096` events and counts dropped events instead of
growing. `PerfSummary` reports schema version, enabled state, retained/dropped
counts, and per-stage count/p50/p95/max duration summaries.

Allowed metadata is numeric document/client/transaction/version/trace IDs,
byte counts, and sanitized feature/path markers. `sanitize_path` retains only a
basename marker. Reports are written only when `CLAY_PERF_REPORT_DIR` is set;
labels are reduced to ASCII alphanumeric/hyphen characters and the file is
replaced through a temporary file plus rename.

Plan 099 stages include:

- browser input, CodeMirror update, editor typing, viewport, scroll, syntax
  freshness, React commit, compartment reconfiguration, long task, and
  paint-adjacent frame completion;
- bridge enqueue/client delivery/forwarder delivery/patch delivery;
- server receive/edit acknowledgement; and
- syntax queue/start/end plus logical parse/query counters.

`PerformanceTraceId` links related stages without storing source text, query
text, credentials, package code, or absolute paths. Frontend snapshots expose
the developer-only `globalThis.__clayPerfSnapshot()` hook; Tauri commands and
process reports remain harness plumbing, not package APIs.

## Plan 088 window and responsive baselines

The Plan 088 responsive baseline remains a historical documentation contract,
not a current native-editor implementation. Its `responsive_layout_work`
marker and structural bounds are maintained in
`docs/development/performance.md` and guarded by
`tests/primitives_docs.rs`/`tests/performance_budgets.rs`; current Plan 099
measurement uses the React/Tauri paths documented below.

## Plan 099 deterministic matrix

`tests/editor_performance.rs::editor_performance_matrix_holds_deterministic_invariants`
uses one real IPC server and 30 generated-fixture cells. It drives the typed
protocol and asserts mode classification, one atomic patch per request ID,
exact edit/version accounting, save/reload/resync behavior, and close
retirement across sizes, line shapes, and first-party extensions.

`frontend/src/editor/extensions/performance.test.ts` drives the real
`createEditor` path and asserts:

- the shared `bytePositionField` follows every repeated 1 MiB edit without a
  rebuild;
- 100 sliding patches retain constant-size render data;
- a 50 MiB document remains one current `Text` with no programmatic history;
- four-pane patch work is linear and pane-isolated; and
- decoration application does not lose text.

Neither suite uses wall-clock thresholds as CI invariants. The source-free
reference/designated-device harness is separate:

```bash
CLAY_PERF_PROFILE=1 scripts/editor-performance-smoke.sh \
  --sizes 1,10,50 \
  --kinds mixed-unicode,many-short-lines,long-lines,newline-heavy \
  --label local-run --enforce
```

The harness builds an instrumented frontend and binaries, generates fixtures,
launches a private profiled server and desktop, and writes:

```text
target/perf/editor-performance/<label>/
  frontend-frontend-perf-snapshot.json
  clay-desktop-perf-summary.json
  clay-server-perf-summary.json
  summary.json
```

The analyzer always fails missing frontend reports or retention above 4,096;
`--enforce` additionally fails long tasks over 50 ms. Missing interactive
stages are warnings, not timing passes. The reference host cannot claim
keyboard typing/scrolling when no input backend or WebKit document accessibility
is available; those p95s remain pending a designated device.

## Large-file Markdown editor-parity contract (Phase 18.5)

The large-file Markdown target is overhead, not a whole-process 30 MiB cap:
`markdown_overhead <= 30 MiB`. Benchmark JSON for future large-file Markdown
runs must separate `total_rss`, `baseline_rss`, `document_memory`,
`markdown_parser_temporary_allocations`, `retained_decoration_cache_memory`,
and `markdown_overhead`. This keeps document memory from being mistaken for
parser/cache overhead.

The stable policy markers are kept verbatim for documentation tests:

```text
large files (`> 5 MiB`) must not use full-document parse/decorate on ordinary open, edit, or scroll paths
Benchmark JSON for future large-file Markdown runs must separate
typing/local paint
visible decoration refresh
parser cancellation
```

The comparison policy is explicit: small files may use full-document work when
advisory cost is low; medium files use viewport/windowed parsing; large files
(`> 5 MiB`) must not use full-document parse/decorate on ordinary open, edit, or
scroll paths. The measured categories remain `typing/local paint`, `visible
decoration refresh`, `parser cancellation`, transport, status/fallback, and
memory. Markdown parser delay may affect decoration freshness, never local text
or edit acknowledgement.

## Plan 089 editor, menu, tab, completion, and accessibility cost guards

The existing `editor_baselines` `editor_render_adjacent` group remains the local
editor proxy. Plan 089's `command_centre_open_baselines`,
`completion_selection_baselines`, and `accessibility_tree_update_baselines`
remain bounded structural/advisory coverage; Criterion's saved-target
comparisons remain advisory. Stable IDs and retained-update bounds are blocking
invariants, while machine timing does not become a CI threshold by itself.

## Bounds and policy

- `PERF_SNAPSHOT_CAPACITY = 4,096` retained events per recorder;
- `SYNTAX_EXECUTOR_MAX_JOBS = 4` native syntax permits;
- `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES = 64` per-document parser states;
- `MODE_ACTIVATION_CACHE_ENTRIES = 64` activations per runtime generation;
- `SYNTAX_CACHE_BUDGET_BYTES = 30 MiB` retained syntax-window budget;
- `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB = 256 MiB` document envelope;
- `MAX_CHUNK_BYTES = 256 KiB` document transfer chunk;
- `VIEWPORT_OVERSCAN = 4,096` UTF-16 positions in the frontend render guard.

These are host safety/performance controls, not package configuration keys.
Profiling remains opt-in and has no production document/parser authority.

## Tests

- `tests/perf_fixtures.rs` — deterministic bytes, UTF-8 validity, shape
  coverage, and approved output roots.
- `src/perf/metrics.rs` — disabled/no-op behavior, report label/path
  sanitization, percentile summaries, source-safe metadata, and capacity drop.
- `tests/performance_budgets.rs` — named budget values, document streaming,
  Markdown benchmark contracts, and hot-path/security documentation.
- `frontend/src/editor/performance.test.ts` — recorder capacity, source safety,
  stage ordering, and disabled behavior.
- `frontend/src/editor/position-map.test.ts` — indexed conversion/work bounds.
- `frontend/src/editor/extensions/performance.test.ts` — editor ownership,
  render retention, pane isolation, and no-history invariants.
- `tests/editor_performance.rs` — real-protocol matrix and patch/edit counts.

Run focused coverage with:

```bash
cargo test --test protocol perf_fixtures:: performance_budgets::
cargo test --test runtime editor_performance_matrix_holds_deterministic_invariants -- --exact
cd frontend && npm test -- --run src/editor/performance.test.ts src/editor/extensions/performance.test.ts
```

## Security and extension

Fixture generation never reads workspace files, user configuration, secrets, or
shell output. `validate_output_path` rejects parent traversal and writes only
to approved roots. Reports are content-free and path-sanitized. No performance
hook exposes filesystem, network, shell, package, parser, executor, or client
JavaScript authority.

To add a fixture shape, update `FixtureKind`, its generator branch, CLI help,
and `tests/perf_fixtures.rs`; keep exact-size valid UTF-8 output and document
whether the shape changes any performance ceiling. Add a new trace stage only
when it identifies a real boundary; keep metadata numeric and update the
performance documentation rather than adding always-on collection.

## Related

- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Frontend Edit Synchronization](../flows/frontend-edit-synchronization.md)
- [Syntax Sessions](syntax-sessions.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [Desktop Typed Bridge](desktop-typed-bridge.md)
- [Protocol Codec](protocol-codec.md)
- [Maintenance Validation](maintenance-validation.md)
- `docs/development/performance.md`
- `src/perf/budgets.rs`
