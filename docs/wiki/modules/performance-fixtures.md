# Performance Fixture Generation

## Scope

Covers `src/perf/mod.rs`, `src/perf/fixtures.rs`, `src/perf/baselines.rs`, `src/perf/metrics.rs`, the `clay perf-fixture` and `--profile-perf` CLI paths in `src/main.rs`, profiling hooks in editor/layout/SDUI/client/server/protocol/runtime modules, `benches/editor_baselines.rs`, `benches/protocol_server_baselines.rs`, `benches/runtime_sdui_baselines.rs`, `benches/markdown_baselines.rs`, `tests/perf_fixtures.rs`, and the developer guide at `docs/development/performance.md`.

## Responsibilities

The performance fixture module generates deterministic large UTF-8 plain-text files for Phase 14 benchmarks, targeted tests, and manual smoke preparation. It provides reusable Rust helpers plus a developer-only CLI command so large files can be reproduced locally instead of committed to the repository.

The baseline module exposes internal, non-user-facing helpers for Criterion targets. These helpers assemble deterministic editor surfaces, protocol messages, server documents, behavior manifests, and SDUI trees so benchmark files measure production paths without duplicating fixture or protocol construction logic.

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
- Server-side JavaScript runtime/configuration evaluation duration in `src/server/js_runtime.rs`.

Metric metadata is numeric and sanitized: document/client/version/transaction IDs are allowed, while document text, file contents, JavaScript source bodies, secrets, and absolute user paths are not recorded. Path metadata uses `sanitize_path`, which redacts parent directories and keeps only a basename marker for diagnostics.

## Criterion Baseline Scaffolding

`Cargo.toml` installs Criterion as a development dependency and declares each bench target with `harness = false`. The initial groups intentionally stay non-interactive:

- `editor_visible_extraction`, `editor_editing`, and `editor_scroll_viewport` use `EditorSurface` and generated fixtures for buffer, visible extraction, edit, and scroll-adjacent measurements.
- `protocol_codec_payloads` and `server_document_acknowledgements` use the production `Codec` and in-process `DocumentState` acknowledgement logic for deterministic IPC/server baselines.
- `runtime_configuration_baselines` and `sdui_application_baselines` cover deterministic behavior-manifest creation plus native SDUI snapshot/update and codec paths.
- `markdown_activation_baselines`, `markdown_parse_and_decoration_baselines`, and `markdown_decorated_editor_baselines` cover first-party Markdown package activation, representative parse/decorations validation, and native decorated-editor render-adjacent work.
- `tools/bench/markdown-parser.mjs` covers actual parser cost outside Criterion. It builds large Markdown corpora by repeating the largest committed repository `.md` files, then times the active `markdown-it` parser and package adapter path with Node.js. Historical mdast measurements are retained only as parser replacement rationale.

Benchmarks report bytes or element throughput for large-data cases where practical and use Criterion batched setup so fixture/surface construction stays separate from the timed operation when needed. `cargo bench --no-run` is the CI-friendly validation command; full timing and `--save-baseline`/`--baseline` comparisons are local advisory workflows. Markdown benchmark timings are advisory evidence for parser/adapter decisions; hard gates remain deterministic payload and no-hot-path tests.

The Markdown parser harness intentionally uses existing repository Markdown rather than dummy generated prose. Local Phase 18 measurements showed historical `mdast-util-from-markdown` taking about 1.28 s for 1 MiB, 16.24 s for 5 MiB, and not completing a 16 MiB parse within a 120 s guard window, while `markdown-it` completed the same sizes in about 66.5 ms, 397.6 ms, and 849.7 ms. The removed mdast adapter's full-document path took about 49.3 s at 1 MiB because byte-offset conversion repeatedly scanned from the start of the document. After the rewrite, the active `markdown-it` plus package-adapter harness completed local 1.01 MiB, 5.02 MiB, and 16.01 MiB repository-Markdown corpora in about 127.2/190.2 ms, 428.6/608.7 ms, and 1007.4/1577.8 ms respectively for parser/adapter paths. Large-file Markdown support therefore must stay background/viewport-bounded and optimize the active markdown-it adapter before being considered durable.

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
| `CLIENT_EDIT_PAYLOAD_BUDGET_BYTES` | 512 B | `cargo test --test performance_protocol` |
| `EDIT_ACK_PAYLOAD_BUDGET_BYTES` | 96 B | `cargo test --test performance_protocol` |
| `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | 2 048 B | `cargo test --test performance_protocol` |
| `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` | 4 096 B | `cargo test --test performance_protocol` |
| `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` | 1 024 B | `cargo test --test performance_protocol` |

### Advisory latency/memory budgets

| Constant | Value | Observed with |
|---|---|---|
| `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` | 16 ms | `cargo bench --bench editor_baselines editor_render_adjacent` |
| `EDIT_ACK_P95_BUDGET_MS` | 40 ms | `cargo bench --bench protocol_server_baselines server_document_acknowledgements` |
| `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` | 16 ms | `cargo bench --bench editor_baselines editor_scroll_viewport` |
| `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS` | 25 ms | `cargo bench --bench runtime_sdui_baselines runtime_configuration_baselines` |
| `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` | 256 MiB | local profiler during `smoke-gui` fixture workflow |

Advisory values are local-machine comparison targets only; they must not become hard CI thresholds until proven stable across platforms.

Security guardrails: profiling/benchmark workflows must not expose document contents, secrets, open network listeners, grant shell authority, or execute arbitrary JavaScript in the client.

## Tests

- `cargo test --test performance_budgets`: verifies benchmark command discoverability, budget constant/doc alignment, constant values (compile-time guard), developer-only profiling policy, active Markdown benchmark documentation, Phase 18 markdown-it rewrite decision/performance evidence in the plan/docs, and structural UI observability documentation.
- `cargo test --test performance_protocol`: deterministic payload-size budgets, client-first typing invariants, queue depth/responsiveness, and oversized-frame rejection.
- `cargo test --test editor_performance_invariants`: viewport-bounded extraction, scroll layout stability, layout cache invalidation, and Unicode safety.
- `cargo bench --no-run`: compiles all Criterion targets, including `markdown_baselines`, without machine-variant timing.

## Related

- Developer guide: `docs/development/performance.md`
- Budget constants: `src/perf/budgets.rs`
- Plan: `plans/015-Phase14-Performance-Profiling-and-Benchmark-Foundation.md`
- Pattern: `.agents/skills/project-patterns/references/protocol-and-performance.md`
