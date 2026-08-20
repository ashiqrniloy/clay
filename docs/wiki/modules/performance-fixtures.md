# Performance Fixture Generation

## Scope

Covers `src/perf/mod.rs`, `src/perf/fixtures.rs`, `src/perf/baselines.rs`, `src/perf/metrics.rs`, the `clay perf-fixture` and `--profile-perf` CLI paths in `src/cli.rs`, profiling hooks in editor/layout/SDUI/client/server/protocol/runtime modules, `benches/editor_baselines.rs`, `benches/protocol_server_baselines.rs`, `benches/runtime_sdui_baselines.rs`, `benches/markdown_baselines.rs`, `benches/first_party_language_baselines.rs`, `benches/window_baselines.rs`, the conformance/performance suites under `tests/`, and the developer guide at `docs/development/performance.md`.

## Responsibilities

The performance fixture module generates deterministic large UTF-8 plain-text files for Phase 14 benchmarks, targeted tests, and manual smoke preparation. It provides reusable Rust helpers plus a developer-only CLI command so large files can be reproduced locally instead of committed to the repository.

The baseline module exposes internal, non-user-facing helpers for Criterion targets. These helpers assemble deterministic editor surfaces, protocol messages, server documents, behavior manifests, and SDUI trees so benchmark files measure production paths without duplicating fixture or protocol construction logic. Plan 088 adds `responsive_layout_work`, which drives the production SDUI sidebar/editor slot decision across pane widths and UI typography without exposing document text or user paths.

The metrics module provides Clay-owned, low-overhead profiling primitives for Phase 14. `PerfConfig` centralizes activation from `CLAY_PERF_PROFILE=1`, the developer-only `--profile-perf` flag, or test helpers. `PerfRecorder` is no-op by default, and enabled recorders collect typed `MetricSnapshot` values for durations, counters, gauges, and byte counts.

## How It Works

`FixtureSpec` selects a `FixtureKind`, exact byte size, and deterministic seed. `generate_fixture` writes to any `Write` sink using a bounded in-memory string buffer, so tests can generate bytes in memory while the CLI streams chunks to a file through `generate_fixture_file`.

Fixture shapes are intentionally simple and reproducible:

- `long-lines` emits very long lines for layout measurement stress.
- `many-short-lines` emits many compact lines for viewport and scrolling scenarios.
- `mixed-unicode` emits multi-byte Unicode scalar content and emoji for cursor/layout edge cases.
- `newline-heavy` emits dense blank lines for line-boundary behavior.

If a generated line would exceed the requested byte size, the generator fills the remainder with ASCII text. This keeps output exactly sized and always valid UTF-8 without slicing a multi-byte scalar.

## Profiling Hooks and Metric Snapshots

`src/perf/metrics.rs` keeps profiling internal and typed. Disabled recorders have no backing metric buffer, so hot-path hooks can create no-op scopes without collecting snapshots. Enabled recorders store snapshots behind a mutex for developer/test inspection; this is intentionally opt-in and not part of the public Clay JS API.

Current hooks cover:

- Editor visible extraction and local edit counters through `EditorSurface` test/bench recorder helpers.
- Layout paint/rebuild/cache-hit/cache-miss timing in `src/editor/layout.rs`.
- Masonry paint preparation in `src/masonry_editor.rs` without GPU synchronization.
- Native SDUI snapshot/update application duration plus node/operation counts in `src/masonry_sdui.rs`.
- Client edit queue enqueue duration, pending depth, enqueue failures, and acknowledgement application metadata in `src/client/mod.rs`.
- Protocol encode/decode duration, payload byte counts, and oversized-frame counters in `src/protocol/codec.rs`.
- Server document edit acknowledgement duration/counters in `src/server/document.rs`.
- Server-side JavaScript runtime/configuration evaluation duration in `src/server/js_runtime/mod.rs`.

Metric metadata is numeric and sanitized: document/client/version/transaction IDs are allowed, while document text, file contents, JavaScript source bodies, secrets, and absolute user paths are not recorded. Path metadata uses `sanitize_path`, which redacts parent directories and keeps only a basename marker for diagnostics.

## Criterion Baseline Scaffolding

`Cargo.toml` installs Criterion as a development dependency and declares each bench target with `harness = false`. The initial groups intentionally stay non-interactive:

- `editor_visible_extraction`, `editor_editing`, and `editor_scroll_viewport` use `EditorSurface` and generated fixtures for buffer, visible extraction, edit, and scroll-adjacent measurements.
- `editor_typography_viewport_bounds` runs the same small/large fixtures with 10 px and 40 px document profiles, preserving a local regression check that configured typography changes only the bounded viewport window rather than triggering full-document work. Deterministic verification also exercises 500 mixed-role visible spans/1,000 normalized boundaries and statically excludes JavaScript, IPC, filesystem, network, shell, and font-discovery work from editor/SDUI hot paths.
- `responsive_layout_baselines` measures the production SDUI sidebar/editor slot decision at narrow, normal, wide, and large-UI-typography inputs. Its returned flags are sanitized layout facts; timings remain local/advisory while the typed bounds matrix is blocking.
- `protocol_codec_payloads` and `server_document_acknowledgements` use the production `Codec` and in-process `DocumentState` acknowledgement logic for deterministic IPC/server baselines.
- `runtime_configuration_baselines` and `sdui_application_baselines` cover deterministic behavior-manifest creation plus native SDUI snapshot/update and codec paths.
- `markdown_activation_baselines`, `markdown_parse_and_decoration_baselines`, and `markdown_decorated_editor_baselines` cover first-party Markdown package activation, representative parse/decorations validation, and native decorated-editor render-adjacent work.
- `markdown_large_file_windowed_baselines` and `markdown_large_file_visible_render_baselines` cover bounded parse-window metadata, syntax-memory accounting, visible decoration chunk validation at 64 KiB/256 KiB/1 MiB/5 MiB/16 MiB sizes, and 16 MiB render-adjacent editor work after applying a windowed Markdown chunk.
- `tools/bench/markdown-parser.mjs` covers actual parser cost outside Criterion. It builds exact 64 KiB, 256 KiB, 1 MiB, 5 MiB, and 16 MiB Markdown corpora by repeating the largest committed repository `.md` files, then times the active `markdown-it` parser, full-document adapter advisory path, and `windowed-adapter` visible path with Node.js. Historical mdast measurements are retained only as parser replacement rationale.

Benchmarks report bytes or element throughput for large-data cases where practical and use Criterion batched setup so fixture/surface construction stays separate from the timed operation when needed. `cargo bench --no-run` is the CI-friendly validation command; full timing and `--save-baseline`/`--baseline` comparisons are local advisory workflows. Markdown benchmark timings are advisory evidence for parser/adapter decisions; hard gates remain deterministic payload, cache-budget, benchmark-script, and no-hot-path tests.

The Markdown parser harness intentionally uses existing repository Markdown rather than dummy generated prose. Local Phase 18 measurements showed historical `mdast-util-from-markdown` taking about 1.28 s for 1 MiB, 16.24 s for 5 MiB, and not completing a 16 MiB parse within a 120 s guard window, while `markdown-it` completed the same sizes in about 66.5 ms, 397.6 ms, and 849.7 ms. The removed mdast adapter's full-document path took about 49.3 s at 1 MiB because byte-offset conversion repeatedly scanned from the start of the document. After the rewrite, the active `markdown-it` plus package-adapter harness completed local 1.01 MiB, 5.02 MiB, and 16.01 MiB repository-Markdown corpora in about 127.2/190.2 ms, 428.6/608.7 ms, and 1007.4/1577.8 ms respectively for parser/adapter paths. Large-file Markdown support therefore must stay background/viewport-bounded and optimize the active markdown-it adapter before being considered durable.

Phase 18.5 uses an editor-parity memory contract instead of treating total RSS as the 30 MiB target. Total RSS remains reported for triage, but the large-file Markdown pass/fail budget is `markdown_overhead <= 30 MiB`, where `markdown_overhead = markdown_parser_temporary_allocations + retained_decoration_cache_memory`. Benchmark JSON for future large-file Markdown runs must separate `total_rss`, `baseline_rss`, `document_memory`, `markdown_parser_temporary_allocations`, `retained_decoration_cache_memory`, and `markdown_overhead` so a 16 MiB document is not confused with parser/cache overhead. The Phase 18.5 Node harness now emits these categories plus `parserInputBytes`, `hotPathAllowed`, parser/adapter category names, and `markdown_overhead_budget_met`; the Criterion harness covers transport/render-adjacent categories. A bare server-side JavaScript runtime can exceed 30 MiB RSS before any document is opened, so the achievable goal is bounded viewport/window syntax work rather than a whole-process 30 MiB cap.

For editor comparison, small Markdown files (`<= 1 MiB`) may still use full-document parse/decorate on open or explicit resync when advisory timings remain low; medium files (`> 1 MiB` and `<= 5 MiB`) should default to viewport-first/windowed parsing for ordinary edits and scroll; large files (`> 5 MiB`) must not use full-document parse/decorate on ordinary open, edit, or scroll paths. The benchmark suite measures typing/local paint, scroll/render-adjacent work, visible decoration refresh, parser cancellation, parser/decorator CPU by file size, parser, adapter, transport, render-adjacent, status/fallback, and memory categories separately.

Phase 18.5 verification ran:

```text
node --expose-gc tools/bench/markdown-parser.mjs --sizes 64KiB,256KiB,1MiB,5MiB,16MiB --parser markdown-it,adapter,windowed-adapter --iterations 1 --warmup 0 --json
```

Local results showed `windowed-adapter` parsing exactly a 64 KiB window for medium/large corpora and keeping `markdown_overhead` under budget: 3.64 MiB at 5 MiB and 3.64 MiB at 16 MiB. The same run marked full-document `markdown-it` and `adapter` rows as `hotPathAllowed=false` for 5 MiB and 16 MiB; the 16 MiB full adapter advisory row reported 2356.308 ms and 750.48 MiB Markdown overhead, while the status/fallback check took 0.260 ms, confirming that full-document adapter work must not return to ordinary open/edit/scroll paths.

## Plan 088 window and responsive baselines

`benches/window_baselines.rs` keeps ten fixed-input Criterion groups advisory: pane paint, tab switch, responsive layout, centered overlay, completion open/filter/layout/selection, Command Centre open, and retained accessibility-tree update. The pure helpers in `src/perf/baselines.rs` measure bounded geometry/projection work rather than launching UI, serializing documents, or invoking package code. `responsive_layout_work(width, ui_size)` is the blocking layout fact: it records whether the sidebar, editor, and usable-main-width constraints hold at representative 320/900/1200 logical widths and 12/24/96 UI sizes. `AccessibilityTreeBench` constructs the retained shell once, then times label updates that reuse owner/client-derived virtual IDs.

The promotion boundary is deliberate. `tests/editor_performance_invariants.rs::responsive_layout_work_preserves_sidebar_and_editor_bounds`, `accessibility_updates_reuse_stable_virtual_ids_without_allocator_churn`, and `retained_accessibility_update_fixture_stays_bounded` plus the source hot-path guards are blocking; Criterion medians and regression comparisons are machine-local advisory evidence. A benchmark comparison warning does not turn into a CI failure or justify weakening a layout invariant. All benchmark helpers remain `doc(hidden)`/internal and are not Clay JS APIs.

### Plan 089 cost guards

The existing `editor_baselines` `editor_render_adjacent` group remains the
local typing/paint proxy; `protocol_server_baselines` retains edit queue and
acknowledgement groups, and `runtime_sdui_baselines` retains configuration and
SDUI groups. The new `window_baselines` groups are:

- `command_centre_open_baselines`: bounded 16/60/256 catalogue projection;
- `completion_selection_baselines`: selected-row projection at 1/8/60/256
  items;
- `accessibility_tree_update_baselines`: retained shell label updates at
  2/4/8/16 tabs after initial tree construction.

`completion_filter_baselines` is the shared fuzzy-filter measurement for both
completion and Command Centre queries. Run the fixed-input set with:

```text
cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

`AccessibilityTreeBench::update` mutates labels while preserving virtual IDs
based on retained owner/client slots. The deterministic editor invariants
reject `WidgetId::next()` in virtual-ID construction and cap the retained
update fixture; existing `accesskit_consumer` shell tests validate reachable
incremental trees. Criterion timing remains advisory; no budget is raised from
the broad local after-run shifts recorded in `docs/development/performance.md`.

## Security and Authority Boundaries

The generator does not read workspace files, configuration, user documents, secrets, or shell commands. `validate_output_path` resolves relative paths from the repository root, rejects `..` traversal, and only allows writes under `target/perf-fixtures/` or `tests/fixtures/perf/`. The native client receives no new filesystem authority; file-backed performance smoke remains server-authoritative.

Benchmark helpers use in-memory generated fixtures, local data structures, and protocol frames only. They do not open IPC listeners, read user configuration by default, grant workspace permissions, or expose document contents outside local Criterion output.

Profiling hooks are also developer-only. They do not grant filesystem, network, shell, JavaScript, or client-side document authority, and metric snapshots must not include document content or unsanitized paths.

## Extending

Add a new fixture kind by updating `FixtureKind`, `FixtureGenerator::next_line`, CLI documentation in `CLI_USAGE`, and `docs/development/performance.md`. Add shape-specific tests in `tests/perf_fixtures.rs` so future benchmark inputs remain deterministic and UTF-8 valid.

## Phase 14 Performance Budgets and Guardrails

Phase 14 adds `src/perf/budgets.rs` which centralises all typed budget constants shared between tests, the documentation, and future enforcement points.

### Hard-guard constants (compile-time)

| Constant | Value | Checked by |
|---|---|---|
| `CLIENT_EDIT_PAYLOAD_BUDGET_BYTES` | 512 B | `cargo test --test protocol performance_protocol::` |
| `EDIT_ACK_PAYLOAD_BUDGET_BYTES` | 128 B | `cargo test --test protocol performance_protocol::` |
| `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | 2 048 B | `cargo test --test protocol performance_protocol::` |
| `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` | 4 096 B | `cargo test --test protocol performance_protocol::` |
| `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` | 1 024 B | `cargo test --test protocol performance_protocol::` |

### Advisory latency/memory budgets

| Constant | Value | Observed with |
|---|---|---|
| `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` | 16 ms | `cargo bench --bench editor_baselines editor_render_adjacent` |
| `EDIT_ACK_P95_BUDGET_MS` | 40 ms | `cargo bench --bench protocol_server_baselines server_document_acknowledgements` |
| `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` | 16 ms | `cargo bench --bench editor_baselines editor_scroll_viewport` |
| `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS` | 25 ms | `cargo bench --bench runtime_sdui_baselines runtime_configuration_baselines` |
| `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` | 256 MiB | local profiler during `smoke-gui` fixture workflow |

Advisory values are local-machine comparison targets only; they must not become hard CI thresholds until proven stable across platforms. For Phase 18.7 protocol comparisons, use target-specific Criterion commands (`cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline` or `--baseline-lenient phase14-baseline`) rather than `cargo bench --benches -- --baseline-lenient ...`, which can route flags to non-Criterion harnesses.

Security guardrails: profiling/benchmark workflows must not expose document contents, secrets, open network listeners, grant shell authority, or execute arbitrary JavaScript in the client.

## Tests

- `cargo test --test protocol performance_budgets::`: verifies benchmark command discoverability, budget constant/doc alignment, constant values (compile-time guard), developer-only profiling policy, active Markdown benchmark documentation, Phase 18 markdown-it rewrite decision/performance evidence in the plan/docs, structural UI observability documentation, and Plan 088 responsive layout coverage.
- `cargo test --test protocol performance_protocol::`: deterministic payload-size budgets, client-first typing invariants, queue depth/responsiveness, and oversized-frame rejection.
- `cargo test --test editor editor_performance_invariants::`: viewport-bounded extraction, scroll layout stability, responsive sidebar/editor bounds, layout cache invalidation, hot-path boundaries, and Unicode safety.
- `cargo test --test editor package_ui_conformance::` and `cargo test --test editor ui_primitive_conformance::`: blocking theme, catalog/token drift, state, primitive, and package-chrome conformance checks.
- `cargo bench --bench window_baselines responsive_layout_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2`: local responsive layout timing signal.
- `cargo bench --no-run`: compiles all Criterion targets, including `window_baselines`, without machine-variant timing.

## Related

- Developer guide: `docs/development/performance.md`
- Budget constants: `src/perf/budgets.rs`
- Plan: `plans/015-Phase14-Performance-Profiling-and-Benchmark-Foundation.md`
- Pattern: `.agents/skills/project-patterns/references/protocol-and-performance.md`
