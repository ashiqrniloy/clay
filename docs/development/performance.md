# Performance Fixtures and Baseline Workflow

Phase 14 starts with deterministic large-file fixture generation so editor, server/client, and future benchmark work can share the same inputs without committing multi-megabyte files.

## Generate Plain-Text Fixtures

Use the developer-only `perf-fixture` command. Outputs must be under `target/perf-fixtures/` by default, or under `tests/fixtures/perf/` for intentionally committed small samples.

```text
cargo run -- perf-fixture --kind mixed-unicode --size-mib 16
cargo run -- perf-fixture --kind long-lines --size-mib 16 --output target/perf-fixtures/long-lines-16m.txt
cargo run -- perf-fixture --kind many-short-lines --size-mib 16 --seed 42
cargo run -- perf-fixture --kind newline-heavy --size-mib 16
```

Supported fixture kinds:

- `mixed-unicode`: deterministic UTF-8 scalar content including multi-byte scripts and emoji for cursor/layout edge cases.
- `long-lines`: very long single-line runs for layout and horizontal measurement stress.
- `many-short-lines`: compact line-oriented content for scrolling and visible-window extraction.
- `newline-heavy`: dense blank-line content for line-boundary and viewport behavior.

The generator streams bounded chunks to the destination file and does not read workspace files, configuration, secrets, or user documents. It rejects parent-directory traversal and paths outside the allowed fixture roots.

## Manual Large-File Smoke Setup

Generate fixtures before a manual GUI or server/workspace smoke run:

```text
cargo run -- perf-fixture --kind mixed-unicode --size-mib 16 --output target/perf-fixtures/mixed-16m.txt
cargo run -- smoke-gui
```

Current `smoke-gui` opens the managed smoke document. File-backed large-document opening remains server-authoritative through workspace APIs/configuration fixtures; do not grant the native client direct filesystem authority for performance testing.

## Criterion Baseline Benchmarks

Phase 14 uses Criterion for repeatable, local, statistics-backed baseline measurements. Criterion is installed as a development dependency and each benchmark target is configured with `harness = false` as required by the Criterion runner.

Current benchmark targets:

- `benches/editor_baselines.rs`: `editor_visible_extraction`, `editor_editing`, and `editor_scroll_viewport` groups for non-interactive editor buffer, visible extraction, edit, and scroll-adjacent paths.
- `benches/protocol_server_baselines.rs`: `protocol_codec_payloads`, `client_edit_queue_pressure`, `server_document_acknowledgements`, and `server_stale_edit_rejections` groups for `rkyv` frame encoding/decoding, client queue pressure, initial-document payloads, and in-process server acknowledgement/rejection paths.
- `benches/runtime_sdui_baselines.rs`: `runtime_configuration_baselines` and `sdui_application_baselines` groups for deterministic behavior-manifest construction plus native SDUI snapshot/update and SDUI codec paths.
- `benches/markdown_baselines.rs`: `markdown_activation_baselines`, `markdown_parse_and_decoration_baselines`, and `markdown_decorated_editor_baselines` groups for first-party Markdown package classification/activation, parse-update/decorations validation, and native decorated-editor render-adjacent work.
- `benches/first_party_language_baselines.rs`: `first_party_open_parse`, `first_party_incremental_edit`, and `first_party_decorated_scroll` groups for the actual Rust, TypeScript, TSX, JavaScript, and Markdown native descriptors, package queries, representative fixtures, incremental tree reuse, and inert decorated-editor scroll work.
- `tools/bench/markdown-parser.mjs`: advisory Node.js harness for actual Markdown parser cost. It synthesizes 64 KiB, 256 KiB, 1 MiB, 5 MiB, and 16 MiB corpora by repeating the largest committed repository Markdown files, then times the active `markdown-it` parser, full-document package adapter advisory path, and `windowed-adapter` viewport path without creating dummy source documents. JSON output separates parser, adapter, transport, render-adjacent, status/fallback, and memory categories for Phase 18.5 editor-parity verification. Historical mdast timings remain below only as replacement rationale.

Run all benchmarks locally:

```text
cargo bench
```

Compile benchmark targets without running timing loops, which is the preferred CI-friendly validation for this scaffolding:

```text
cargo bench --no-run
```

Run a short local smoke of the visible-extraction baseline:

```text
cargo bench --bench editor_baselines editor_visible_extraction -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Run short local Markdown verification baselines:

```text
cargo bench --bench markdown_baselines markdown_activation_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench --bench markdown_baselines markdown_parse_and_decoration_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Run the large Markdown parser harness. The install command populates local `packages/markdown/node_modules` only; do not commit it.

```text
npm install --prefix packages/markdown --no-save --no-package-lock --ignore-scripts markdown-it@^14.1.0
node --check tools/bench/markdown-parser.mjs
node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8
node --expose-gc tools/bench/markdown-parser.mjs --sizes 64KiB,256KiB,1MiB,5MiB,16MiB --parser markdown-it,adapter,windowed-adapter --iterations 1 --warmup 0 --json
```

Save and compare advisory local baselines before performance-sensitive changes. Run target-specific Criterion commands so CLI flags are consumed by Criterion rather than by unrelated bench harnesses:

```text
cargo bench --bench protocol_server_baselines -- --save-baseline phase14-baseline
cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline
cargo bench --bench protocol_server_baselines -- --baseline-lenient phase14-baseline
```

Use absolute timing results as local guidance only. Machine-variant Criterion results should not become hard CI failures unless a future task proves a threshold is stable. Prefer deterministic tests for hard guards such as payload ceilings, bounded queues, viewport-bounded extraction invariants, and invalid-frame rejection.

## Profiling Metric Snapshots

Phase 14 profiling hooks are internal and disabled by default. Ordinary user sessions create no metric buffers and no profile output. Enable collection only for developer profiling with the environment variable or the global developer flag:

```text
CLAY_PERF_PROFILE=1 cargo run -- smoke-gui
cargo run -- smoke-gui --profile-perf
```

The initial recorder captures sanitized, typed snapshots for scoped durations, counters, gauges, and byte sizes around editor visible extraction/local edits, layout rebuild/cache decisions, paint preparation, native SDUI snapshot/update application, client edit queue depth and acknowledgements, protocol codec payload sizes/oversized frames, server document edit acknowledgement, and server-side runtime/configuration evaluation. Snapshot metadata is numeric where possible (`document_id`, versions, client/transaction IDs) and path metadata is redacted to a basename-only form; document contents, JavaScript source bodies, secrets, and absolute user paths must not be recorded.

Benchmark and test helpers can construct `PerfRecorder::for_test(true)` directly to assert expected metric names without relying on process environment. The no-op default remains the production path when `CLAY_PERF_PROFILE`/`--profile-perf` is absent. Enabled recorders retain at most `PERF_SNAPSHOT_CAPACITY` (4096) metadata-only snapshots; later events are dropped rather than growing profiling memory without bound.

### Incremental syntax work and latency metrics

Native syntax observability separates one accepted edit from parser/query work and decoration fan-out:

| Metric | Meaning |
| --- | --- |
| `syntax.parse.logical_work_items` | One server-accepted native syntax edit for a document/version. |
| `syntax.parse.invocations` | Actual Tree-sitter parser calls. Before one-parse refactoring this may exceed logical work because output chunks still schedule sibling jobs. |
| `syntax.parse.full` / `syntax.parse.incremental` | Parser invocation classification according to cached-tree reuse. |
| `syntax.query.ranges` / `syntax.query.bytes` | Query ranges and bytes submitted to Tree-sitter. |
| `syntax.decoration.chunks` | Validated native decoration chunks accepted for publication, independent of parser calls. |
| `syntax.parse.cancelled_superseded` | Aborted superseded task count with the cancelled task's document/version. |
| `syntax.edit_to_publish` | Accepted-edit to first current-version native decoration publication duration. One sample is retained per accepted document/version even when multiple chunks publish. |

All syntax metrics carry only numeric document IDs, document versions, counts, byte counts, and durations. They contain no document text, captures, query text, clipboard data, package code, or paths. Collection occurs in server background parse/publication paths, never Masonry paint or text-event handlers.

Use deterministic work-count tests as blocking gates:

```text
cargo test --lib server::syntax::tests
cargo test --lib server::parse_coordinator::tests
cargo test --test performance_protocol syntax_pipeline_metrics
```

Use the existing five-language Criterion fixture for advisory latency and throughput distributions:

```text
cargo bench --bench first_party_language_baselines first_party_incremental_edit -- --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench --bench first_party_language_baselines --no-run
```

`first_party_incremental_edit` covers Rust, TypeScript, TSX, JavaScript, and Markdown and measures parse-through-ready-decoration work while reporting fixture-byte throughput. Wall-clock values remain machine-local and advisory until a stable CI baseline is established; work-count and retention assertions are deterministic.

## Plan 056 low-latency syntax Linux verification (2026-07-19)

Linux host: kernel `7.1.3-43.stable`, `x86_64`; Rust/Cargo `1.96.1`. Required Linux gates passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `cargo bench --no-run`.

The five-language incremental benchmark was run locally with 10 samples, 1 s warm-up, and 2 s measurement. It measures one native handler parse/capture pass through ready bounded `DecorationSet` members; values are local/advisory, not CI thresholds.

| Fixture | Median | 95% interval | Throughput median |
| --- | ---: | ---: | ---: |
| Rust | 168.91 µs | 168.12–169.57 µs | 520.34 KiB/s |
| TypeScript | 356.50 µs | 338.27–363.57 µs | 364.33 KiB/s |
| TSX | 125.02 µs | 124.40–126.03 µs | 851.41 KiB/s |
| JavaScript | 124.91 µs | 123.54–126.39 µs | 914.74 KiB/s |
| Markdown | 217.54 µs | 178.80–234.92 µs | 318.72 KiB/s |

Command:

```bash
cargo bench --bench first_party_language_baselines first_party_incremental_edit -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Deterministic coverage supplies the work-count evidence that Criterion intentionally does not print: one `syntax.parse.logical_work_items` item per accepted edit/version, one current-version parser invocation per stable window, changed-range query bytes, bounded fan-out via `syntax.decoration.chunks`, superseded-task cancellation, and one `syntax.edit_to_publish` sample for first current-version publication. `tests/performance_protocol.rs::syntax_pipeline_metrics_are_source_safe_and_retention_bounded`, `tests/parse_coordinator.rs`, and `tests/syntax_grammar.rs` passed in the full Linux run; malformed edits/ranges, stale versions, oversized payloads, wrong provenance, and generation replacement fail closed. Metrics remain numeric-only and never include source text or paths.

## Phase 14 Performance Budgets and Guardrails

Phase 14 splits budgets into two categories:

- **Deterministic hard guards** enforced by tests and invariant checks.
- **Advisory local baselines** measured with Criterion/profiling commands and compared against saved baselines on the same machine.

### Deterministic hard guards

| Focus area | Initial budget | Enforcement |
| --- | --- | --- |
| Client edit payload (`ClientMessage::Edit`) | <= 512 bytes | `cargo test --test performance_protocol` (`representative_protocol_payloads_fit_phase14_budgets`) |
| Edit acknowledgement payload (`ServerMessage::EditAck`) | <= 128 bytes | `cargo test --test performance_protocol` |
| Behavior manifest payload (`ServerMessage::BehaviorManifest`) | <= 2048 bytes | `cargo test --test performance_protocol` |
| SDUI snapshot payload (`ServerMessage::SduiSnapshot`) | <= 4096 bytes | `cargo test --test performance_protocol` |
| SDUI update payload (`ServerMessage::SduiUpdate`) | <= 1024 bytes | `cargo test --test performance_protocol` |
| Client edit queue depth and responsiveness | bounded queue (default capacity 256), no blocking enqueue on full queue | `cargo test --test performance_protocol` (`client_edit_queue_reports_depth_without_blocking_input`) |
| Ordinary typing route | local shadow update must happen before server acknowledgement | `cargo test --test performance_protocol` (`ordinary_edit_updates_shadow_before_ack`) |
| Viewport/layout invariants | viewport-bounded extraction and targeted layout invalidation invariants hold | `cargo test --test editor_performance_invariants` |
| Bench target integrity | benchmark scaffolding compiles in CI-friendly mode | `cargo bench --no-run` |

### SDUI Payload Budget Findings

Phase 15 revalidated the representative SDUI fixtures against the Phase 14 hard budget constants in `src/perf/budgets.rs`:

| Payload | Measured rkyv payload | Budget constant | Finding |
| --- | ---: | ---: | --- |
| Representative `ServerMessage::SduiSnapshot` | 816 bytes | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` = 4096 bytes | Passes with 3280 bytes of headroom; no compression or tree shaping needed for the current fixture. |
| Representative `ServerMessage::SduiUpdate` | 192 bytes | `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` = 1024 bytes | Passes with 832 bytes of headroom; no compression or tree shaping needed for the current panel-update fixture. |

The measured sizes exclude the 4-byte frame length prefix and are enforced by `src/protocol/codec.rs` unit tests. If future package-owned SDUI trees exceed either constant before Phase 17, investigate compression, diff shaping, or a budget adjustment with an explicit rationale before expanding the payload surface.

For the headless SDUI structural regression strategy, status observability surface, window-driver smoke relationship, and deferred GPU-backed pixel snapshot path, see [UI Observability and SDUI Structural Regression](ui-observability.md).

### Advisory local baseline budgets (machine-variant)

Treat these as **comparison targets** for local regression triage, not cross-machine CI failure thresholds:

| Focus area | Initial advisory budget | Observe with |
| --- | --- | --- |
| Keypress-to-local-paint proxy (`editor_render_adjacent`) | <= 16 ms (P95, advisory) | `cargo bench --bench editor_baselines editor_render_adjacent -- --sample-size 10 --warm-up-time 1 --measurement-time 2` and optional `CLAY_PERF_PROFILE=1 cargo run -- smoke-gui --profile-perf` |
| Server edit acknowledgement latency (`server_document_acknowledgements`) | <= 40 ms (P95, advisory) | `cargo bench --bench protocol_server_baselines server_document_acknowledgements -- --sample-size 10 --warm-up-time 1 --measurement-time 2` |
| Scroll/layout/render-adjacent paths (`editor_scroll_viewport`, `editor_layout_viewport_bounds`) | <= 16 ms (P95, advisory) | `cargo bench --bench editor_baselines editor_scroll_viewport -- --sample-size 10 --warm-up-time 1 --measurement-time 2` |
| Runtime/configuration evaluation (`runtime_configuration_baselines`) | <= 25 ms (P95, advisory) | `cargo bench --bench runtime_sdui_baselines runtime_configuration_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` |
| Large-file memory envelope during 16 MiB fixture workflows | <= 256 MiB (advisory) | local profiler/task manager during `cargo run -- smoke-gui` with generated fixture workflow |

### Local baseline workflow

Save a target-specific baseline before performance-sensitive refactors:

```text
cargo bench --bench protocol_server_baselines -- --save-baseline phase14-baseline
```

Compare after changes:

```text
cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline
cargo bench --bench protocol_server_baselines -- --baseline-lenient phase14-baseline
```

Use `--baseline-lenient` only on target-specific Criterion commands. On this codebase, `cargo bench --benches -- --baseline-lenient ...` can route the flag to a non-Criterion bench harness and fail before the comparison runs.

Investigate only sustained regressions across repeated local runs. Phase 18.7 repeated local protocol comparisons showed stable nanosecond-scale regressions versus the old Phase 14 baseline for `hello_roundtrip` (~+28–32%) and `client_edit/16` (~+19–20%), while larger payloads and server-document groups varied between regression, no-change, and improvement. No deterministic payload budget regressed (`cargo test --test performance_protocol` passed), and the benchmark code path changed only by Clippy-equivalent match simplification, so the Phase 18.7 result is accepted as a machine/local-baseline refresh signal rather than a protocol shape blocker.

### Security and authority guardrails for profiling/benchmark workflows

Performance workflows must remain local and constrained:

- Profiling snapshots must not expose document contents.
- Profiling snapshots must not expose secrets.
- Bench/profiling commands must not open network listeners.
- Bench/profiling commands must not grant shell authority.
- Bench/profiling commands must not execute arbitrary JavaScript in the client.
- Fixture generation and benchmark helpers must stay within approved output/data boundaries and preserve server-authoritative file/workspace permissions.

## Phase 18.18 first-party language package verification

Phase 18.18 adds deterministic payload/open-order guards and an optimized Criterion target for all five first-party native descriptor/fixture combinations (`.rs`, `.ts`, `.tsx`, `.js`, `.md`). These fixtures are inert repository text; benchmark code loads no config, package JavaScript, network, shell, workspace, or user path.

Hard guards:

- `first_party_decoration_payloads_stay_within_budget_per_language` runs each compiled grammar/query and serializes its real `DecorationSet` and `IncrementalParseUpdate`. It also locks the 4096-byte native parse window, 5000 ms timeout, and 30 MiB syntax-cache ceiling.
- `first_party_open_parse_does_not_block_initial_render_per_language` installs a deliberately delayed handler for each package, enqueues parse work, and proves the editor snapshot is visible before background parse completion.
- Existing `editor_performance_invariants` guards keep parser/package JavaScript/IPC/file IO out of paint, input, layout, and scroll paths.

Measured on the local Linux verification host (2026-07-13, optimized Criterion profile, 10 samples, 1 s warm-up, 2 s measurement):

| Fixture | Decoration/update payload | Open parse median | Incremental one-character edit median | Decorated scroll median |
| --- | ---: | ---: | ---: | ---: |
| Rust | 928 / 1168 B | 70.692 µs | 176.05 µs | 1.141 µs |
| TypeScript | 1480 / 1768 B | 82.379 µs | 122.17 µs | 1.416 µs |
| TSX | 1304 / 1576 B | 71.946 µs | 127.00 µs | 1.216 µs |
| JavaScript | 1304 / 1584 B | 71.403 µs | 133.52 µs | 1.244 µs |
| Markdown | 664 / 920 B | 91.540 µs | 209.38 µs | 1.008 µs |

Payload ceilings are 8192 B per decoration set and 4096 B per combined incremental update. All measured scroll work is far below the advisory 16 ms render-adjacent budget; all open/incremental parses are below 0.25 ms median on these small representative fixtures. Direct monitoring of the optimized benchmark process (excluding Cargo/compiler parent RSS) observed 16.9 MiB maximum RSS, below `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` (256 MiB); retained syntax accounting remains independently capped at `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB).

Baseline workflow used:

```text
cargo bench --bench first_party_language_baselines -- --save-baseline phase18.18-entry --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench --bench first_party_language_baselines -- --baseline phase18.18-entry --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench --bench first_party_language_baselines -- first_party_incremental_edit/tsx --baseline phase18.18-entry --sample-size 20 --warm-up-time 2 --measurement-time 4
```

Immediate comparison found no sustained actionable regression. The first short TSX comparison reported +6.57%; the repeated 20-sample run narrowed to +1.48% and Criterion classified it within the noise threshold. Absolute results and baseline deltas remain advisory and machine-local until Phase 21; deterministic payload, enqueue-order, cache, and no-hot-path checks remain the hard CI gates. Baseline files stay under ignored `target/criterion/`, not source control.

## Markdown mode verification

Phase 18 adds deterministic Markdown performance/regression guards around the first-party `@clay/markdown` package without turning machine-variant timings into hard CI thresholds.

Hard guards and regression tests:

- `markdown_behavior_manifest_fits_budget` encodes the actual Markdown behavior manifest with package commands/keymaps and verifies it stays within `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`.
- `markdown_parse_and_decoration_payloads_fit_budgets` serializes a representative Markdown `IncrementalParseUpdate` plus `DecorationSet` for headings, strong/emphasis, inline code, fenced code blocks, and list markers; both stay under `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` and `DECORATION_PAYLOAD_BUDGET_BYTES`.
- `markdown_typing_does_not_wait_for_markdown_it_parse` schedules a slow Markdown parser and proves local editor insertion completes before the server parse result, preserving the no-hot-path JavaScript rule.
- `markdown_large_file_typing_does_not_wait_for_windowed_parse` schedules a slow bounded-window parser and proves large-file local typing still applies before parser completion.
- `markdown_reload_reinstalls_manifest_and_decorations`, `markdown_disabled_falls_back_to_plain_text_after_rewrite`, `markdown_invalid_package_reports_sanitized_diagnostics`, and `markdown_fixture_activates_with_markdown_it_adapter` cover reload/restart, disabled fallback, invalid package diagnostics, package activation, and fixture/smoke setup without granting extra package authority.
- `markdown_structural_sdui_snapshot_matches_fixture` keeps Markdown preview/status smoke coverage structural and headless; the fixture publishes inert `Markdown Preview` SDUI labels without screenshots, GPU work, or client-side package JavaScript.
- Parser correctness evidence remains in the package/runtime tests: the `markdown-it` token-stream adapter emits required span kinds, keeps parser-specific data behind `packages/markdown/dist/parser.js`, avoids `mdast-util-from-markdown` imports, and verifies the UTF-8 fixture `# Hé 🦀` maps to exact Clay byte ranges.
- `markdown_it_adapter_large_fixture_span_counts_are_stable` runs the package adapter over a deterministic repeated token-stream fixture and proves stable nonzero span counts for headings, strong/emphasis, inline code, fenced code blocks, and unordered/ordered list markers.
- Clay JS API docs/registry lookup is checked separately by `cargo test --test clay_js_doc_registry`, while package docs path lookup remains covered by `markdown_package_docs_path_is_required_and_resolvable`.

Advisory local Markdown benchmark findings:

- `markdown_activation_baselines` measures package metadata classification, major-mode activation, and behavior-manifest selection.
- `markdown_parse_and_decoration_baselines` measures representative parse-update validation and server-side decoration publication validation.
- `markdown_decorated_editor_baselines` measures native visible-editor work after applying inert Markdown decoration spans.
- `markdown_large_file_windowed_baselines` measures bounded parse-window request metadata, syntax-memory budget accounting, and visible decoration chunk validation at 64 KiB, 256 KiB, 1 MiB, 5 MiB, and 16 MiB document sizes.
- `markdown_large_file_visible_render_baselines` measures render-adjacent editor work after applying a visible windowed Markdown decoration chunk to a 16 MiB document.

Local Phase 18 runs should compare the existing Phase 14/15/17 benchmark targets against `phase14-baseline` with `--baseline-lenient` and record any sustained regression in the plan. Newly added Markdown groups did not exist in the saved Phase 14 baseline, so their first local run is recorded as advisory Markdown evidence rather than a hard baseline comparison.

### Active markdown-it benchmark verification (2026-06-04)

The active parser/adapter verification used Node v26.2.0 and the documented local-only command:

```text
node --expose-gc tools/bench/markdown-parser.mjs --sizes 1MiB,5MiB,16MiB --parser markdown-it,adapter --iterations 1 --warmup 0
```

The harness built corpora from the largest committed repository Markdown files repeated to requested sizes, excluded `target` and `node_modules`, printed only repository-relative source paths plus aggregate counts/timing/memory, did not mutate fixtures/source files, did not open network listeners, and did not execute client-side JavaScript. Results from this workstation were:

| Corpus | Coverage highlights | `markdown-it` parse | Active package adapter path |
| --- | --- | ---: | ---: |
| 1.01 MiB | 260 headings, 118 strong spans, 502 fences, UTF-8 present | 127.234 ms, 47,190 tokens, peak RSS 160.79 MiB | 190.213 ms, 14,945 spans, peak RSS 192.47 MiB |
| 5.02 MiB | 1,458 headings, 596 strong spans, 2,590 fences, UTF-8 present | 428.597 ms, 230,108 tokens, peak RSS 256.58 MiB | 608.680 ms, 72,654 spans, peak RSS 325.42 MiB |
| 16.01 MiB | 4,852 headings, 1,984 strong spans, 8,430 fences, UTF-8 present | 1007.381 ms, 733,415 tokens, peak RSS 455.23 MiB | 1577.844 ms, 231,008 spans, peak RSS 632.51 MiB |

These values are advisory local evidence only. The deterministic gates remain payload budgets, non-blocking typing, structural SDUI, docs/registry lookup, benchmark script policy checks, and `cargo bench --no-run` benchmark compilation.

### Large-file Markdown windowed benchmark verification (2026-06-05)

After the Phase 18.5 windowed adapter and chunk cache work, the active benchmark command was extended and run locally with Node v26.2.0:

```text
node --expose-gc tools/bench/markdown-parser.mjs --sizes 64KiB,256KiB,1MiB,5MiB,16MiB --parser markdown-it,adapter,windowed-adapter --iterations 1 --warmup 0 --json
```

The JSON was written to `target/markdown-phase18_5-benchmark.json` during verification and remains a local artifact, not a committed fixture. It reported `total_rss`, `baseline_rss`, `document_memory`, `markdown_parser_temporary_allocations`, `retained_decoration_cache_memory`, and `markdown_overhead` for each parser path. The full-document `markdown-it` and `adapter` rows are advisory evidence only for medium/large files; `windowed-adapter` is the ordinary editor path for visible Markdown refresh.

| Corpus | Full `markdown-it` | Full adapter advisory | `windowed-adapter` visible path | Editor-parity finding |
| --- | ---: | ---: | ---: | --- |
| 64 KiB | 20.127 ms, `markdown_overhead` 1.76 MiB | 19.077 ms, 4.10 MiB | 14.199 ms, 3.73 MiB | Small-file paths stayed under the 30 MiB overhead budget; status/fallback check took 0.410 ms. |
| 256 KiB | 30.211 ms, 5.78 MiB | 51.087 ms, 16.65 MiB | 16.948 ms, 3.61 MiB | Windowed path stayed bounded while full adapter remained allowed only because this is small; status/fallback check took 0.074 ms. |
| 1 MiB | 136.551 ms, 26.31 MiB | 277.604 ms, 37.73 MiB | 28.511 ms, 3.62 MiB | Full adapter exceeded the overhead budget; windowed path stayed under budget; status/fallback check took 0.111 ms. |
| 5 MiB | 557.065 ms, 76.14 MiB | 934.864 ms, 148.74 MiB | 21.954 ms, 3.64 MiB | Full-document paths are not hot-path eligible; windowed path stayed under budget; status/fallback check took 0.113 ms. |
| 16 MiB | 1312.502 ms, 261.71 MiB | 2356.308 ms, 750.48 MiB | 26.272 ms, 3.64 MiB | Large-file editor path met the `windowed-adapter markdown_overhead <= 30 MiB` target and avoided full-document parser input; status/fallback check took 0.260 ms. |

Deterministic guards now verify the same policy without machine-variant timing thresholds: `markdown_windowed_benchmark_uses_real_parser_and_repo_corpus`, `markdown_benchmark_json_reports_editor_parity_categories`, `markdown_large_file_memory_overhead_fits_budget`, `markdown_full_document_adapter_is_not_large_file_hot_path`, `markdown_large_file_typing_does_not_wait_for_windowed_parse`, and `markdown_full_document_adapter_is_not_large_file_hot_path_static_guard`.

### Large-file Markdown editor-parity contract (Phase 18.5)

Established editor parity means responsive typing and scrolling with bounded syntax work, not synchronous full-document Markdown decoration. The Phase 18.5 contract is:

- **Small Markdown files (`<= 1 MiB`)**: full-document `markdown-it` parsing and adapter work may run on open/reload or explicit resync when advisory local results stay comfortably below interactive thresholds, but it still must not block keypress-to-local-paint.
- **Medium Markdown files (`> 1 MiB` and `<= 5 MiB`)**: viewport-first/windowed parsing is the default for ordinary edits and scroll. Full-document work is allowed only as cancellable idle/background validation and must not be part of open, edit, or scroll response.
- **Large Markdown files (`> 5 MiB`, including the 16 MiB target)**: ordinary open, edit, and scroll paths must not run full-document parse/decorate. The package must parse bounded viewport/near-viewport windows, publish bounded decoration chunks, cancel stale work, and degrade to partial/plain-text highlighting when budgets would be exceeded.

Editor-comparison targets for large Markdown workflows are local advisory targets until deterministic cross-machine enforcement exists:

| Target | Phase 18.5 expectation | Measurement path |
| --- | --- | --- |
| Typing/local paint | `<= 16 ms` p95; Markdown parser delay may only affect decoration freshness | Existing `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `markdown_typing_does_not_wait_for_markdown_it_parse`, and future large-file typing guard |
| Scroll/render-adjacent work | `<= 16 ms` p95 for local visible extraction/layout/paint-adjacent work | Existing `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` and future windowed decoration scroll benchmark |
| Visible decoration refresh | Target `<= 100 ms` p95 for viewport/near-viewport Markdown chunks on local benchmark hardware; stale chunks may temporarily remain or clear | Future `windowed-adapter` benchmark and decoration chunk publication tests |
| Parser cancellation | Superseded viewport/edit parse work should be cancelled or marked stale before publishing, target `<= 50 ms` p95 cancellation observation in local tests | Future parse-window coordinator tests |
| Parser/decorator CPU by file size | Full-document path may remain advisory for `<= 1 MiB`; `5 MiB` and `16 MiB` ordinary workflows must use bounded windows rather than full-document adapter timings | `tools/bench/markdown-parser.mjs` full-document evidence plus future windowed mode |
| Markdown memory overhead | `<= 30 MiB` retained/temporary Markdown-specific overhead for the 16 MiB workflow | Future benchmark JSON memory categories and cache accounting tests |

Memory accounting separates total process size from Markdown-specific overhead:

```text
total_rss = process memory reported by the OS/runtime; always reported, not capped at 30 MiB
baseline_rss = Clay/runtime process before opening or parsing the Markdown document
document_memory = canonical document rope/text, layout metadata, edit state, and other non-Markdown parser state
markdown_parser_temporary_allocations = bounded parse-window strings, markdown-it tokens, source indexes, and transient span arrays
retained_decoration_cache_memory = validated syntax/decor chunks retained for visible and near-viewport ranges
markdown_overhead = markdown_parser_temporary_allocations + retained_decoration_cache_memory
Phase 18.5 target: markdown_overhead <= 30 MiB for a 16 MiB Markdown workflow
```

The 30 MiB target applies to **Markdown-specific overhead only**, not total process RSS. Total RSS must still be reported because it is useful for triage, but it is not the 30 MiB pass/fail value: a bare Node/V8 process on this workstation already reports more than 30 MiB RSS before opening a document. Benchmark JSON for large-file Markdown work must expose separate `total_rss`, `baseline_rss`, `document_memory`, `markdown_parser_temporary_allocations`, `retained_decoration_cache_memory`, and `markdown_overhead` categories so later tasks can compare the overhead budget without hiding process memory.

Large-file parser recommendation from the local Node.js parser harness (Node v26.2.0 on this workstation): do **not** treat full-document `mdast-util-from-markdown` parsing as proven for ordinary large-file editing. The harness used existing repository Markdown files only, led by `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`, `roadmap.md`, and large plan documents; the synthesized corpora include headings, strong/emphasis, inline code, fenced code blocks, ordered/unordered lists, long paragraphs, many short sections, and UTF-8 content. Results were:

| Corpus | `mdast-util-from-markdown` `fromMarkdown` | `markdown-it` parse | Package adapter path |
| --- | ---: | ---: | ---: |
| 1.01 MiB | 1,278.715 ms, 37,132 mdast nodes, peak RSS 315.33 MiB | 66.528 ms, 46,877 tokens, peak RSS 298.63 MiB | 49,311.589 ms, 15,182 spans, peak RSS 297.46 MiB |
| 5.03 MiB | 16,239.409 ms, 181,939 mdast nodes, peak RSS 716.49 MiB | 397.630 ms, 227,471 tokens, peak RSS 727.71 MiB | Not run; 1 MiB adapter result is already too slow for full-document use. |
| 16.03 MiB | Did not complete within a 120 second local guard window; an earlier combined run also exceeded 600 seconds before producing results. | 849.659 ms, 725,141 tokens, peak RSS 434.30 MiB | Not run; full-document adapter is infeasible. |

The `mdast-util-from-markdown` adapter result above is historical replacement evidence, not an active implementation path. The active package now depends on `markdown-it`; future large-file work should optimize the token-stream adapter and viewport/range mapping rather than restore mdast. Do not add full-document parser IPC or client-side JavaScript to compensate.

Manual smoke command:

```text
cargo run -- smoke-gui --config-fixture markdown-mode
```

The smoke fixture validates package activation, command/action provenance, parse/decorations status, inert `Markdown Preview` SDUI, and plain document fallback behavior without reading arbitrary user paths, opening network listeners, granting shell authority, exposing document contents, or executing client-side JavaScript.

## Phase 18.21 LSP bridge / document-analysis budgets

Phase 18.21 keeps Rust core LSP-wire neutral. Deterministic hard guards cover session/worker ceilings and payload budgets; real language-server wall-clock timings stay environment-gated and advisory.

### Deterministic hard guards

| Focus area | Budget | Enforcement |
| --- | --- | --- |
| Language-server stdin/stdout chunk | <= 1048576 bytes (`LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`) | `cargo test --test language_server_authority` / `performance_protocol` |
| Language-server stderr retain | <= 65536 bytes (`LANGUAGE_SERVER_STDERR_BUDGET_BYTES`) | `cargo test --test language_server_authority` |
| Concurrent language-server sessions | <= 16 (`LANGUAGE_SERVER_MAX_SESSIONS`) | `cargo test --test language_server_authority` |
| Document-analysis workers | <= 4 (`DOCUMENT_ANALYSIS_MAX_WORKERS`) | `cargo test --test editor_performance_invariants` / `performance_protocol` |
| Document-analysis worker heap | <= 67108864 bytes (`DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES`) | `cargo test --test performance_protocol` |
| Documents per analysis worker | <= 32 | `cargo test --test performance_protocol` |
| Synced document text | <= 262144 bytes (`DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES`) | document-analysis unit tests + performance locks |
| Analysis input mailbox | <= 64 events / 2097152 bytes | document-analysis unit tests |
| Analysis output queue | <= 64 events / 524288 bytes | document-analysis unit tests |
| Pending child requests | <= 8 | shared LSP client + performance locks |
| Decoration / diagnostics / completion / intelligence payloads | <= 8192 / 8192 / 16384 / 16384 bytes | `cargo test --test performance_protocol` |
| Fake-server bridge matrix latency | < 5 s wall clock for open/init/request/shutdown across four packages | `cargo test --test performance_protocol fake_server_bridge_matrix` |

### Advisory measurements

- Keep using `cargo bench --bench first_party_language_baselines` for Tree-sitter open/incremental/scroll baselines before and after enabling bridge packages (`--save-baseline pre-lsp` / `--baseline-lenient pre-lsp`).

```text
cargo bench --bench first_party_language_baselines -- --save-baseline pre-lsp
cargo bench --bench first_party_language_baselines -- --baseline-lenient pre-lsp
```

- Real rust-analyzer / typescript-language-server / marksman open/init/request latency is measured only under `CLAY_LSP_REAL_SMOKE=1` via `cargo test --test lsp_real_servers -- --nocapture`. Do not promote those timings to Criterion CI gates.
- Edit acknowledgement and local paint must never wait on worker/JS/subprocess work; see `tests/editor_performance_invariants.rs`.

## Phase 19 hot-reload runtime-state budgets

Phase 19 keeps reload evaluation/commit off the ordinary edit and paint paths. Complete runtime-generation snapshots reuse the existing 1 MiB IPC frame ceiling; the 768 KiB payload / 16 ms install figures are review thresholds for a future diff upgrade, not soft pass/fail gates.

### Deterministic hard guards

| Focus area | Budget | Enforcement |
| --- | --- | --- |
| Runtime-state broadcast capacity | 16 (`RUNTIME_STATE_BROADCAST_CAPACITY`) | `cargo test --test performance_protocol phase19_runtime_state_snapshot_and_grace_budgets_are_locked` |
| Snapshot document / diagnostic caps | 64 / 32 | same + `RuntimeStateSnapshot::validate` |
| Snapshot hard frame ceiling | <= 1 MiB (`DEFAULT_MAX_FRAME_SIZE`) | prepare encode-check + `tests/runtime_update_protocol.rs` |
| Diff-upgrade review payload | 768 KiB p95 (`RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES`) | budget lock + representative encode test |
| Diff-upgrade review install | 16 ms p95 (`RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS`) | budget lock; advisory until measured |
| Previous-behavior grace | 2000 ms / 256 transactions | `cargo test --lib behavior::tests` / grace integration tests |
| Reload work outside editor hot paths | no candidate validate/install/reload symbols in paint/text-event bodies | `runtime_generation_install_stays_outside_paint_and_text_event_hot_paths` |
| Edits during blocked candidate | EditAck continues while reload waits on test barrier | `typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier` |

### Focused verification

```text
cargo test --test persistent_runtime_hot_reload
cargo test --test runtime_update_protocol
cargo test --test performance_protocol phase19_runtime_state
cargo test --test editor_performance_invariants runtime_generation_install
cargo test --lib typing_and_edit_ack_continue_while_candidate
cargo test --lib failed_reload_broadcasts_diagnostic_but_no_generation_snapshot
cargo test --lib successful_reload_is_observed_as_one_generation_by_all_clients
cargo test --lib reload_preserves_authority_denials_and_cleans_old_lsp_worker
```

## Validation

Run the fixture tests after changing generator logic:

```text
cargo test --test perf_fixtures
```

Run focused profiling-hook tests after changing metric collection logic:

```text
cargo test perf_recorder -- --nocapture
cargo test editor_visible_extraction_records_metric_when_enabled
```

Run protocol/queue performance guards after changing client edit queue, server acknowledgement/rejection, or codec payload handling:

```text
cargo test --test performance_protocol
```

Run Phase 18.21 fake-server and budget locks after changing LSP bridge or document-analysis limits:

```text
cargo test --test lsp_bridge
cargo test --test language_server_authority
cargo test --test performance_protocol phase18_21
cargo test --test editor_performance_invariants document_analysis
```

Run the benchmark compile check after changing benchmark scaffolding or the measured non-interactive paths:

```text
cargo bench --no-run
```

These checks verify deterministic output, UTF-8 validity, shape coverage, exact byte sizing, output path constraints, disabled-by-default profiling behavior, snapshot sanitization, enabled editor metrics, client-first queue invariants, representative protocol payload budgets, documented benchmark command discoverability, and benchmark target compilation.
