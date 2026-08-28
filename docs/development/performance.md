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

Plan 098 adds an automated and a scripted path for large-document verification:

- `cargo test --test runtime large_document::` drives a real server with a generated 50 MiB fixture through open (head+chunks) / edit / save / reload, asserting chunk wire bounds, timing budgets, and the oversize/binary refusal paths.
- `scripts/large-document-smoke.sh` generates the fixtures, starts a workspace-scoped server plus the Tauri/React desktop, and prints the manual checklist.

## Criterion Baseline Benchmarks

Phase 14 uses Criterion for repeatable, local, statistics-backed baseline measurements. Criterion is installed as a development dependency and each benchmark target is configured with `harness = false` as required by the Criterion runner.

Current benchmark targets:

- `benches/editor_baselines.rs`: `editor_visible_extraction`, `editor_editing`, and `editor_scroll_viewport` groups for non-interactive editor buffer, visible extraction, edit, and scroll-adjacent paths.
- `benches/protocol_server_baselines.rs`: `protocol_codec_payloads`, `client_edit_queue_pressure`, `server_document_acknowledgements`, and `server_stale_edit_rejections` groups for `rkyv` frame encoding/decoding, client queue pressure, initial-document payloads, and in-process server acknowledgement/rejection paths.
- `benches/runtime_sdui_baselines.rs`: `runtime_configuration_baselines` and `sdui_application_baselines` groups for deterministic behavior-manifest construction plus native SDUI snapshot/update and SDUI codec paths.
- `benches/markdown_baselines.rs`: `markdown_activation_baselines`, `markdown_parse_and_decoration_baselines`, and `markdown_decorated_editor_baselines` groups for first-party Markdown package classification/activation, parse-update/decorations validation, and native decorated-editor render-adjacent work.
- `benches/first_party_language_baselines.rs`: `first_party_open_parse`, `first_party_incremental_edit`, and `first_party_decorated_scroll` groups for the actual Rust, TypeScript, TSX, JavaScript, and Markdown native descriptors, package queries, representative fixtures, incremental tree reuse, and inert decorated-editor scroll work.
- `benches/window_baselines.rs`: pane paint, tab switch, responsive layout, centered overlay, completion open/filter/layout/selection, Command Centre open, and retained accessibility-tree update groups; all are bounded pure/native projections with local/advisory wall-clock results.
- `tools/bench/markdown-parser.mjs`: advisory Node.js harness for actual Markdown parser cost. It synthesizes 64 KiB, 256 KiB, 1 MiB, 5 MiB, and 16 MiB corpora by repeating the largest committed repository Markdown files, then times the active `markdown-it` parser, full-document package adapter advisory path, and `windowed-adapter` viewport path without creating dummy source documents. JSON output separates parser, adapter, transport, render-adjacent, status/fallback, and memory categories for Phase 18.5 editor-parity verification. Historical mdast timings remain below only as replacement rationale.

Run all benchmarks locally:

```text
cargo bench
```

Compile benchmark targets without running timing loops, which is the preferred CI-friendly validation for this scaffolding:

```text
cargo bench --no-run
```

The `editor_baselines` and `markdown_baselines` Criterion groups were removed
with the native client at Plan 097 Phase 12; their dated results below remain
as the historical record. Markdown parser cost is now measured by the retained
`tools/bench/markdown-parser.mjs` harness; editor work-count bounds are pinned
by the lib `server::syntax` tests.

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

### Plan 099 correlated editor trace and production baseline

Plan 099 adds trace schema version `1` across the browser, Tauri client, and
server syntax path. A trace ID is a content-free numeric counter. For typing,
the enabled browser trace ID is also used as that edit's existing transaction
ID; viewport requests carry an optional `traceId` and native decoration patches
return it. This adds no new user-facing Clay JS API and does not grant package
code parser or telemetry authority.

Profiling stays opt-in and bounded:

```text
cargo run -- smoke-gui --config-fixture language-packages --profile-perf
```

`--profile-perf`/`CLAY_PERF_PROFILE=1` propagates from the launcher to the
Tauri desktop and any supervised server. The frontend recorder is available in
memory as `globalThis.__clayPerfSnapshot()` while profiling is enabled; the
Tauri-only `session_perf_snapshot` command returns the desktop bridge/client
summary. Both outputs contain only schema/version, numeric trace/document/
transaction/version/byte-count fields, sanitized stage names, durations, and
bounded drop counts. They never contain source text, package code, paths,
credentials, clipboard data, or query content.

The common event stages are `browser.input`, `codemirror.update`,
`bridge.enqueue`, `bridge.client_delivery`, `server.receive`,
`server.edit_ack`, `syntax.queue`, `syntax.start`, `syntax.end`,
`bridge.patch_delivery`, `bridge.server_delivery`,
`bridge.forwarder_delivery`, `editor.patch_apply`, `editor.syntax_fresh`,
`editor.paint_adjacent`, `editor.open`, `editor.ready`, `editor.typing`,
`editor.scroll`, `react.commit`, `editor.compartment_reconfigure`, and
`editor.long_task`. Every summary reports event count plus p50/p95/max for
recorded duration samples; count-only stages report zero duration samples.

Production baseline contract on the Linux reference host (AMD Ryzen 9 PRO
7940HS, 61 GiB RAM, WebKitGTK 2.52.5, Node 24.19.0, Rust 1.96.1):

| Trace group                |                                               Initial advisory baseline | Hard evidence                                                                                               |
| -------------------------- | ----------------------------------------------------------------------: | ----------------------------------------------------------------------------------------------------------- |
| Open/first text            |       p95 ≤ 100 ms warm runtime (≤1 MiB); ≤200 ms minimum-device target | `editor.open`/`editor.ready` plus the existing head/chunk smoke path                                        |
| 50 MiB ready               |                         p95 ≤ 1 s reference; ≤2 s minimum-device target | `editor.open` duration and bounded document-transfer evidence                                               |
| Typing/local paint         |       p95 ≤ 4 ms reference; ≤8 ms minimum-device target; hard max 16 ms | `editor.paint_adjacent`, `editor.typing`, and the existing `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` contract |
| Edit acknowledgement       |                                                             p95 ≤ 40 ms | `server.edit_ack` plus existing protocol/server acknowledgement tests                                       |
| Scroll-to-syntax freshness | p95 ≤ 50/100 ms reference/minimum for ≤1 MiB; ≤100/200 ms for 10–50 MiB | `editor.syntax_fresh` paired with `browser.viewport` and `bridge.patch_delivery`                            |
| React/CodeMirror churn     |          0 React commits and 0 compartment reconfigures per normal edit | `react.commit`, `editor.compartment_reconfigure`, and `codemirror.update` counts                            |
| Long tasks                 |                                                      zero events ≥50 ms | `editor.long_task` PerformanceObserver samples                                                              |
| Trace retention            |                            ≤4096 frontend/server snapshots per recorder | `performance.test.ts` and `perf::metrics` capacity tests                                                    |

These are machine-local production baselines, not cross-machine CI timing
failures. Save the frontend snapshot and the Tauri summary together with the
host/build metadata when comparing Plan 099 changes; retain raw traces under
`target/perf/` only.

### Plan 099 editor performance matrix and stable invariants

The matrix harness runs the flows over generated fixtures on a real
Tauri/WebKit desktop:

```bash
scripts/editor-performance-smoke.sh --sizes 1,10,50     --kinds mixed-unicode,many-short-lines,long-lines,newline-heavy
```

It builds the frontend with `VITE_CLAY_PERF_PROFILE=1`, generates fixtures
under `target/perf-fixtures/` (the generator refuses other roots), copies each
shape under `.txt/.md/.rs/.ts/.tsx/.js` extensions so path-driven mode
classification exercises every first-party mode, and launches a profiled
server (`--config-fixture clay-performance-matrix`, which preloads the
markdown/rust/typescript/javascript packages) plus the desktop. When the
window closes, three source-free reports land under
`target/perf/editor-performance/<label>/`: the browser/CodeMirror snapshot
(frontend, flushed on a 10 s interval to avoid teardown races), the desktop
bridge summary, and the server summary (SIGTERM dump). A Python analyzer
merges them into `summary.json` with a p95 table and a verdict.

Blocking invariants:

- CI: deterministic work-count/ownership/retention/history invariants live in
  `tests/editor_performance.rs` (real-protocol server matrix across size x
  kind x language: exactly one `ViewportRenderPatch` per request id, exact
  edit/version accounting, save/reload/resync round-trips, close retirement)
  and `frontend/src/editor/extensions/performance.test.ts` (shared-index
  edits, constant-size 100-scroll retention, 50 MiB single-`Text` no-history
  ownership, linear four-pane patch application, software-render functional
  smoke).
- Harness (`--enforce`): zero long tasks > 50 ms and bounded retention
  (<= 4096 events). Stage-presence gaps (e.g. `bridge.patch_delivery` absent
  because the checklist was not driven) are reported as warnings and gate the
  timing table, not the verdict, until a designated device drives the flows.

Timing enforcement policy: machine-variant p95 numbers are recorded per run
and become blocking only after three stable runs on the designated minimum
device (same device label, consistent p95 across runs). The reference-host
harness run on 2026-08-27 (`target/perf/editor-performance/ref-run-2/`)
passed with all three reports captured; interactive type/scroll flows were
not drivable on that host (no keyboard input backend), so its p95 table
covers protocol codec and runtime-load stages only.

The first post-instrumentation Rust baseline was captured with profiling
_disabled_ so it measures the normal production path (10 samples, 1 s warm-up,
2 s measurement):

| Group                                      |    Median |     p95 interval |
| ------------------------------------------ | --------: | ---------------: |
| `protocol_codec_payloads/hello_roundtrip`  | 106.41 ns | 105.87–106.95 ns |
| `protocol_codec_payloads/client_edit/16`   | 126.24 ns | 125.85–127.00 ns |
| `protocol_codec_payloads/client_edit/1024` | 254.13 ns | 249.98–257.16 ns |
| `server_document_acknowledgements/1`       | 249.01 µs | 248.12–250.10 µs |
| `server_document_acknowledgements/128`     | 263.85 µs | 262.32–265.03 µs |

Command:

```text
cargo bench --bench protocol_server_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

The correlated browser trace is the production GUI baseline artifact: run the
same command with `--profile-perf`, exercise open/ready, a typing burst, and a
scroll/jump, then save `globalThis.__clayPerfSnapshot()` plus the
`session_perf_snapshot` result. No GUI timing is promoted to a hard gate until
three stable runs on designated minimum hardware. On 2026-08-27 this host
launched the profiled GUI and produced a screenshot, but interactive input was
blocked because `ydotool` was unavailable; no browser timing value is claimed.

### Plan 099 current bounds and ownership

These are current implementation bounds, not user configuration knobs:

| Guard                         | Current bound                                                            | Enforcement/source                                                                           |
| ----------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Local keypress to paint       | `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS = 16 ms`                          | CodeMirror applies local edits first; no synchronous IPC, package JavaScript, or parser work |
| Edit acknowledgement          | `EDIT_ACK_P95_BUDGET_MS = 40 ms`                                         | Server acknowledgement and protocol performance tests                                        |
| Scroll-adjacent render        | `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS = 16 ms`                    | Viewport patch application and frontend invariant suites                                     |
| Document transfer             | `MAX_CHUNK_BYTES = 256 KiB`; codec frame `1 MiB`                         | Head/chunk protocol validation before rope slicing or allocation                             |
| Native parser context         | `NATIVE_GRAMMAR_MAX_WINDOW_BYTES = 768 KiB`; query/output budget `4 KiB` | Bounded `ParseWindowSnapshot`/`ParsePolicy` and coordinator validation                       |
| Open document resident memory | `DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES = 256 MiB`                        | Server-owned workspace reservation; open/reload fail closed, never `init.js` configurable    |
| Native syntax concurrency     | `SYNTAX_EXECUTOR_MAX_JOBS = 4`                                           | Shared `spawn_blocking` permits across all document sessions                                 |
| Per-document parser state     | `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES = 64`                                | Session-owned parser/tree cache; eviction beyond the bound is arbitrary, not an LRU promise  |
| Mode activation cache         | `MODE_ACTIVATION_CACHE_ENTRIES = 64` per generation                      | Completed activation manifests are reused only when classification inputs match              |
| Retained syntax/decor data    | `SYNTAX_CACHE_BUDGET_BYTES = 30 MiB`                                     | Server `SyntaxChunkCache` byte accounting plus near-viewport pruning                         |
| Developer trace retention     | `PERF_SNAPSHOT_CAPACITY = 4096`                                          | Frontend/Rust recorders are disabled by default and drop beyond capacity                     |
| Frontend render overscan      | `4,096` UTF-16 positions per side, widened to covered range              | `render-patch.ts` `guardOf`; current patches replace exact covered authority                 |

`SyntaxSession` keeps one latest-wins job per `(generation, document, grammar)`;
`ViewportRenderPatch` keeps one terminal answer per request ID. Packages do
not configure these guards, receive parser handles, or run code in the client.

Native syntax observability separates one accepted edit from parser/query work and decoration fan-out. Plan 099 runs native handlers in per-document `SyntaxSession` workers on the bounded blocking executor; a running superseded job may finish, but its output is stale-dropped and request-scoped completion accounting still closes:

| Metric                                           | Meaning                                                                                                                                                                      |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `syntax.parse.logical_work_items`                | One server-accepted native syntax edit for a document/version.                                                                                                               |
| `syntax.parse.invocations`                       | Actual Tree-sitter parser calls. One selected handler runs per accepted stable version/window; output chunks and viewport patch members do not schedule sibling parser jobs. |
| `syntax.parse.full` / `syntax.parse.incremental` | Parser invocation classification according to cached-tree reuse.                                                                                                             |
| `syntax.query.ranges` / `syntax.query.bytes`     | Query ranges and bytes submitted to Tree-sitter.                                                                                                                             |
| `syntax.decoration.chunks`                       | Validated native decoration chunks accepted for publication, independent of parser calls.                                                                                    |
| `syntax.parse.cancelled_superseded`              | Superseded-session work recorded as stale/cancelled with the document/version; a running native parse is not interrupted mid-call.                                           |
| `syntax.edit_to_publish`                         | Accepted-edit to first current-version native decoration publication duration. One sample is retained per accepted document/version even when multiple chunks publish.       |

All syntax metrics carry only numeric document IDs, document versions, counts, byte counts, and durations. They contain no document text, captures, query text, clipboard data, package code, or paths. Collection occurs in server background parse/publication paths, never in client render or input handlers.

Use deterministic work-count tests as blocking gates:

```text
cargo test --lib server::syntax::tests
cargo test --lib server::parse_coordinator::tests
cargo test --test protocol performance_protocol::syntax_pipeline_metrics
```

The five-language `first_party_incremental_edit` Criterion group (Rust,
TypeScript, TSX, JavaScript, Markdown parse-through-ready-decoration work with
fixture-byte throughput) was removed with the native client at cutover;
wall-clock values below remain machine-local history. Current equivalents: the
lib `server::syntax::tests` / `server::parse_coordinator::tests` work-count and
retention assertions plus the frontend editor suites.

## Plan 056 low-latency syntax Linux verification (2026-07-19)

Linux host: kernel `7.1.3-43.stable`, `x86_64`; Rust/Cargo `1.96.1`. Required Linux gates passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `cargo bench --no-run`.

The five-language incremental benchmark was run locally with 10 samples, 1 s warm-up, and 2 s measurement. It measures one native handler parse/capture pass through ready bounded `DecorationSet` members; values are local/advisory, not CI thresholds.

| Fixture    |    Median |     95% interval | Throughput median |
| ---------- | --------: | ---------------: | ----------------: |
| Rust       | 168.91 µs | 168.12–169.57 µs |      520.34 KiB/s |
| TypeScript | 356.50 µs | 338.27–363.57 µs |      364.33 KiB/s |
| TSX        | 125.02 µs | 124.40–126.03 µs |      851.41 KiB/s |
| JavaScript | 124.91 µs | 123.54–126.39 µs |      914.74 KiB/s |
| Markdown   | 217.54 µs | 178.80–234.92 µs |      318.72 KiB/s |

Command:

```bash
cargo bench --bench first_party_language_baselines first_party_incremental_edit -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Deterministic coverage supplies the work-count evidence that Criterion intentionally does not print: one `syntax.parse.logical_work_items` item per accepted edit/version, one current-version parser invocation per stable window, changed-range query bytes, bounded fan-out via `syntax.decoration.chunks`, superseded-task cancellation, and one `syntax.edit_to_publish` sample for first current-version publication. the then-present `tests/performance_protocol.rs::syntax_pipeline_metrics_are_source_safe_and_retention_bounded`, `tests/parse_coordinator.rs`, and `tests/syntax_grammar.rs` suites passed in that run (since consolidated into the lib `server::syntax`/`server::parse_coordinator` tests and the protocol suite); malformed edits/ranges, stale versions, oversized payloads, wrong provenance, and generation replacement fail closed. Metrics remain numeric-only and never include source text or paths.

## Plan 057 syntax-continuity Linux verification (2026-07-19)

Plan 057 keeps parse scheduling unchanged while making replacement coverage complete and retaining same-word syntax locally. Linux host: kernel `7.1.3-43.stable`, `x86_64`; Rust/Cargo `1.96.1`. The five-language continuity work-count test records one parser call, one query range, and one emitted replacement member for each representative suffix edit:

| Fixture    | Parser calls | Query ranges | Queried bytes | Emitted members |
| ---------- | -----------: | -----------: | ------------: | --------------: |
| Rust       |            1 |            1 |            20 |               1 |
| TypeScript |            1 |            1 |            26 |               1 |
| TSX        |            1 |            1 |            26 |               1 |
| JavaScript |            1 |            1 |            26 |               1 |
| Markdown   |            1 |            1 |            17 |               1 |

`server::syntax::tests::first_party_continuity_edits_keep_one_bounded_parse_and_query` produces these deterministic counts from the real native descriptors and package queries. Each query remains below one 128-byte replacement chunk. `server::parse_coordinator::tests::accepted_native_edit_records_one_logical_item_and_one_latency_sample` records exactly one `syntax.edit_to_publish` duration for first current-version publication; the local instrumentation-plumbing sample was 140.268 µs. This single unit-scale sample is advisory, not a product threshold.

The optimized parse-through-ready-decoration benchmark was rerun with 10 samples, 1 s warm-up, and 2 s measurement:

| Fixture    |  Estimate |     95% interval | Throughput estimate |
| ---------- | --------: | ---------------: | ------------------: |
| Rust       | 167.95 µs | 165.05–170.66 µs |        523.32 KiB/s |
| TypeScript | 361.10 µs | 342.59–368.41 µs |        359.68 KiB/s |
| TSX        | 125.92 µs | 124.14–127.41 µs |        845.33 KiB/s |
| JavaScript | 122.93 µs | 121.22–124.70 µs |        929.46 KiB/s |
| Markdown   | 199.86 µs | 166.32–215.06 µs |        346.92 KiB/s |

Criterion reported no statistically significant performance change for any fixture. Wall-clock values remain machine-local and advisory; one-parse/query counts, payload/cache ceilings, stale-version rejection, and source-safe metric retention remain blocking deterministic gates.

## Plan 058 exact-range replacement Linux verification (2026-07-20)

Plan 058 changes only client application of current authoritative decoration sets. Linux host: kernel `7.1.3-43.stable`, `x86_64`; Rust/Cargo `1.96.1`. `server::syntax::tests::first_party_continuity_edits_keep_one_bounded_parse_and_query` retains the Plan 057 work counts exactly: one parser call, one query range, and one emitted member with 20 queried bytes for Rust, 26 for TypeScript/TSX/JavaScript, and 17 for Markdown. No parser, query, or transport work was added.

Deterministic client bounds now cover both drift directions and long runs. `plan058_first_party_languages_preserve_shifted_boundary_continuity` checks every local-edit, acknowledgement, and streamed-authority state after three repeated pre-boundary insertions in all five native grammars. `plan058_repeated_insert_delete_authority_cycles_preserve_boundary_geometry` runs 128 insertion/deletion pairs through validated transport without an undecorated byte. `repeated_authority_keeps_local_residual_cache_bounded` runs 512 authoritative applications while retaining exactly two chunks/two spans, one provisional residual, exact serialized-byte accounting, and `retained_bytes <= SYNTAX_CACHE_BUDGET_BYTES`. `exact_range_decoration_replacement_stays_off_edit_and_paint_hot_paths` keeps subtraction/coalescing out of local edit and paint bodies.

A focused Criterion benchmark measures one current authoritative apply that subtracts `[0,128)` from shifted provisional chunks and locally coalesces the right residual. With 20 samples, 1 s warm-up, and 2 s measurement, `first_party_authoritative_replacement/apply_and_coalesce_residual` measured 1.8150 µs (95% interval 1.6250–1.9959 µs). Setup and optimistic edit creation are outside the timed body.

The unchanged five-language incremental benchmark was rerun with 10 samples, 1 s warm-up, and 2 s measurement:

| Fixture    |  Estimate |     95% interval | Criterion comparison |
| ---------- | --------: | ---------------: | -------------------- |
| Rust       | 152.39 µs | 97.456–187.70 µs | improvement reported |
| TypeScript | 344.39 µs | 326.19–350.92 µs | no change            |
| TSX        | 125.50 µs | 124.36–126.48 µs | no change            |
| JavaScript | 123.55 µs | 122.74–124.65 µs | no change            |
| Markdown   | 199.49 µs | 165.71–214.08 µs | no change            |

Criterion found no statistically significant regression. Wall-clock values remain machine-local and advisory; deterministic geometry, work-count, cache-budget, stale-version, provenance, payload, and hot-path tests are blocking.

## Phase 14 Performance Budgets and Guardrails

Phase 14 splits budgets into two categories:

- **Deterministic hard guards** enforced by tests and invariant checks.
- **Advisory local baselines** measured with Criterion/profiling commands and compared against saved baselines on the same machine.

### Deterministic hard guards

| Focus area                                                                    | Initial budget                                                               | Enforcement                                                                                                             |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Client edit payload (`ClientMessage::Edit`)                                   | <= 512 bytes                                                                 | payload budget pins in `tests/performance_budgets.rs` + codec round-trip tests                                          |
| Edit acknowledgement payload (`ServerMessage::EditAck`)                       | <= 128 bytes                                                                 | `EDIT_ACK_PAYLOAD_BUDGET_BYTES` pin in `tests/performance_budgets.rs`                                                   |
| Behavior manifest payload (`ServerMessage::BehaviorManifest`)                 | <= 2048 bytes                                                                | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` pin in `tests/performance_budgets.rs`                                          |
| Package manifest metadata (`clay.*` incl. contributions and extension points) | <= 8192 bytes                                                                | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`; `cargo test --test protocol performance_budgets::`                            |
| SDUI snapshot payload (`ServerMessage::SduiSnapshot`)                         | <= 4096 bytes                                                                | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` pin in `tests/performance_budgets.rs`                                              |
| SDUI update payload (`ServerMessage::SduiUpdate`)                             | <= 1024 bytes                                                                | `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` pin in `tests/performance_budgets.rs`                                                |
| Client edit queue depth and responsiveness                                    | bounded queue (default capacity 256), no blocking enqueue on full queue      | `ClientEditQueue::bounded` unit tests in `src/client/mod.rs`; bridge forwarder tests                                    |
| Ordinary typing route                                                         | local shadow update must happen before server acknowledgement                | frontend optimistic-sync tests (`frontend/src/test/editor.test.tsx`, sync/session suites)                               |
| Viewport/layout invariants                                                    | viewport-bounded extraction and targeted layout invalidation invariants hold | CodeMirror library-owned viewport rendering + position-map tests (`src/editor/position_map.rs`, frontend editor suites) |
| Bench target integrity                                                        | benchmark scaffolding compiles in CI-friendly mode                           | `cargo bench --no-run`                                                                                                  |

### SDUI Payload Budget Findings

Phase 15 revalidated the representative SDUI fixtures against the Phase 14 hard budget constants in `src/perf/budgets.rs`:

| Payload                                      | Measured rkyv payload |                                   Budget constant | Finding                                                                                                        |
| -------------------------------------------- | --------------------: | ------------------------------------------------: | -------------------------------------------------------------------------------------------------------------- |
| Representative `ServerMessage::SduiSnapshot` |             816 bytes | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` = 4096 bytes | Passes with 3280 bytes of headroom; no compression or tree shaping needed for the current fixture.             |
| Representative `ServerMessage::SduiUpdate`   |             192 bytes |   `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` = 1024 bytes | Passes with 832 bytes of headroom; no compression or tree shaping needed for the current panel-update fixture. |

The measured sizes exclude the 4-byte frame length prefix and are enforced by `src/protocol/codec.rs` unit tests. If future package-owned SDUI trees exceed either constant before Phase 17, investigate compression, diff shaping, or a budget adjustment with an explicit rationale before expanding the payload surface.

For the headless SDUI structural regression strategy, status observability surface, window-driver smoke relationship, and deferred GPU-backed pixel snapshot path, see [UI Observability and SDUI Structural Regression](ui-observability.md).

### Advisory local baseline budgets (machine-variant)

Treat these as **comparison targets** for local regression triage, not cross-machine CI failure thresholds:

| Focus area                                                                                      | Initial advisory budget  | Observe with                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Keypress-to-local-paint proxy (`editor_render_adjacent`)                                        | <= 16 ms (P95, advisory) | `cargo bench --bench editor_baselines editor_render_adjacent -- --sample-size 10 --warm-up-time 1 --measurement-time 2` and optional `CLAY_PERF_PROFILE=1 cargo run -- smoke-gui --profile-perf` |
| Server edit acknowledgement latency (`server_document_acknowledgements`)                        | <= 40 ms (P95, advisory) | `cargo bench --bench protocol_server_baselines server_document_acknowledgements -- --sample-size 10 --warm-up-time 1 --measurement-time 2`                                                       |
| Scroll/layout/render-adjacent paths (`editor_scroll_viewport`, `editor_layout_viewport_bounds`) | <= 16 ms (P95, advisory) | `cargo bench --bench editor_baselines editor_scroll_viewport -- --sample-size 10 --warm-up-time 1 --measurement-time 2`                                                                          |
| Runtime/configuration evaluation (`runtime_configuration_baselines`)                            | <= 25 ms (P95, advisory) | `cargo bench --bench runtime_sdui_baselines runtime_configuration_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2`                                                           |
| Large-file memory envelope during 16 MiB fixture workflows                                      | <= 256 MiB (advisory)    | local profiler/task manager during `cargo run -- smoke-gui` with generated fixture workflow                                                                                                      |
| Gutter paint (`GUTTER_PAINT_P95_BUDGET_MS`)                                                     | <= 2 ms (P95, advisory)  | visible-line numbers only; `cargo test --test editor phase26_7_chrome_paint_budgets_fit_inside_keypress_envelope` locks the envelope vs `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`                  |
| Active-line highlight (`ACTIVE_LINE_PAINT_P95_BUDGET_MS`)                                       | <= 1 ms (P95, advisory)  | one full-line fill; same budget lock                                                                                                                                                             |
| Bracket-match highlight (`BRACKET_MATCH_PAINT_P95_BUDGET_MS`)                                   | <= 1 ms (P95, advisory)  | pair scan capped at 64 KiB; same budget lock                                                                                                                                                     |
| Decoration background fills (`DECORATION_BACKGROUND_FILL_P95_BUDGET_MS`)                        | <= 2 ms (P95, advisory)  | Quote/CodeBlock/SearchMatch/Deprecated run fills before glyphs; same budget lock                                                                                                                 |

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

Investigate only sustained regressions across repeated local runs. Phase 18.7 repeated local protocol comparisons showed stable nanosecond-scale regressions versus the old Phase 14 baseline for `hello_roundtrip` (~~+28–32%) and `client_edit/16` (~~+19–20%), while larger payloads and server-document groups varied between regression, no-change, and improvement. No deterministic payload budget regressed (`cargo test --test protocol performance_protocol::` passed), and the benchmark code path changed only by Clippy-equivalent match simplification, so the Phase 18.7 result is accepted as a machine/local-baseline refresh signal rather than a protocol shape blocker.

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

- `first_party_decoration_payloads_stay_within_budget_per_language` runs each compiled grammar/query and serializes its real `DecorationSet` and `IncrementalParseUpdate`. It locks 4096-byte native parse windows for code grammars, Markdown's bounded data-only context ceiling (`NATIVE_GRAMMAR_MAX_WINDOW_BYTES` = 768 KiB), the independent 4096-byte ordinary parse envelope, and the derived `INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES` envelope when folding is attached (`4096 + 2048 = 6144`), plus the 5000 ms timeout and 30 MiB syntax-cache ceiling. Same-version Markdown scroll requests reuse the cached full tree, so the larger context ceiling does not mean a full reparse per viewport. File loading is separate: open/reload stream into ropes under the 256 MiB server-owned resident budget and publish bounded heads/chunks.
- `first_party_open_parse_does_not_block_initial_render_per_language` installs a deliberately delayed handler for each package, enqueues parse work, and proves the editor snapshot is visible before background parse completion.
- Existing `editor_performance_invariants` guards keep parser/package JavaScript/IPC/file IO out of paint, input, layout, and scroll paths.

Measured on the local Linux verification host (2026-07-13, optimized Criterion profile, 10 samples, 1 s warm-up, 2 s measurement):

| Fixture    | Decoration/update payload | Open parse median | Incremental one-character edit median | Decorated scroll median |
| ---------- | ------------------------: | ----------------: | ------------------------------------: | ----------------------: |
| Rust       |              928 / 1168 B |         70.692 µs |                             176.05 µs |                1.141 µs |
| TypeScript |             1480 / 1768 B |         82.379 µs |                             122.17 µs |                1.416 µs |
| TSX        |             1304 / 1576 B |         71.946 µs |                             127.00 µs |                1.216 µs |
| JavaScript |             1304 / 1584 B |         71.403 µs |                             133.52 µs |                1.244 µs |
| Markdown   |               664 / 920 B |         91.540 µs |                             209.38 µs |                1.008 µs |

Payload ceilings are 8192 B per decoration set, 4096 B for an ordinary incremental update, and 6144 B when the independently capped 2048 B folding set is attached. All measured scroll work is far below the advisory 16 ms render-adjacent budget; all open/incremental parses are below 0.25 ms median on these small representative fixtures. Direct monitoring of the optimized benchmark process (excluding Cargo/compiler parent RSS) observed 16.9 MiB maximum RSS, below `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` (256 MiB); retained syntax accounting remains independently capped at `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB).

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
- `markdown_parse_and_decoration_payloads_fit_budgets` serializes a representative Markdown `IncrementalParseUpdate` plus `DecorationSet` for headings, strong/emphasis, inline code, fenced code blocks, and list markers; ordinary updates stay under `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, folded updates use the derived `INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES`, and decorations stay under `DECORATION_PAYLOAD_BUDGET_BYTES`.
- `markdown_typing_does_not_wait_for_markdown_it_parse` schedules a slow Markdown parser and proves local editor insertion completes before the server parse result, preserving the no-hot-path JavaScript rule.
- `markdown_large_file_typing_does_not_wait_for_windowed_parse` schedules a slow bounded-window parser and proves large-file local typing still applies before parser completion.
- `markdown_reload_reinstalls_manifest_and_decorations`, `markdown_disabled_falls_back_to_plain_text_after_rewrite`, `markdown_invalid_package_reports_sanitized_diagnostics`, and `markdown_fixture_activates_with_markdown_it_adapter` cover reload/restart, disabled fallback, invalid package diagnostics, package activation, and fixture/smoke setup without granting extra package authority.
- `markdown_structural_sdui_snapshot_matches_fixture` keeps Markdown preview/status smoke coverage structural and headless; the fixture publishes inert `Markdown Preview` SDUI labels without screenshots, GPU work, or client-side package JavaScript.
- Parser correctness evidence remains in the package/runtime tests: the `markdown-it` token-stream adapter emits required span kinds, keeps parser-specific data behind `packages/markdown/dist/parser.js`, avoids `mdast-util-from-markdown` imports, and verifies the UTF-8 fixture `# Hé 🦀` maps to exact Clay byte ranges.
- `markdown_it_adapter_large_fixture_span_counts_are_stable` runs the package adapter over a deterministic repeated token-stream fixture and proves stable nonzero span counts for headings, strong/emphasis, inline code, fenced code blocks, and unordered/ordered list markers.
- Clay JS API docs/registry lookup is checked separately by `cargo test --test protocol clay_js_doc_registry::`, while package docs path lookup remains covered by `markdown_package_docs_path_is_required_and_resolvable`.

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

| Corpus    | Coverage highlights                                             |                              `markdown-it` parse |                     Active package adapter path |
| --------- | --------------------------------------------------------------- | -----------------------------------------------: | ----------------------------------------------: |
| 1.01 MiB  | 260 headings, 118 strong spans, 502 fences, UTF-8 present       |   127.234 ms, 47,190 tokens, peak RSS 160.79 MiB |   190.213 ms, 14,945 spans, peak RSS 192.47 MiB |
| 5.02 MiB  | 1,458 headings, 596 strong spans, 2,590 fences, UTF-8 present   |  428.597 ms, 230,108 tokens, peak RSS 256.58 MiB |   608.680 ms, 72,654 spans, peak RSS 325.42 MiB |
| 16.01 MiB | 4,852 headings, 1,984 strong spans, 8,430 fences, UTF-8 present | 1007.381 ms, 733,415 tokens, peak RSS 455.23 MiB | 1577.844 ms, 231,008 spans, peak RSS 632.51 MiB |

These values are advisory local evidence only. The deterministic gates remain payload budgets, non-blocking typing, structural SDUI, docs/registry lookup, benchmark script policy checks, and `cargo bench --no-run` benchmark compilation.

### Large-file Markdown windowed benchmark verification (2026-06-05)

After the Phase 18.5 windowed adapter and chunk cache work, the active benchmark command was extended and run locally with Node v26.2.0:

```text
node --expose-gc tools/bench/markdown-parser.mjs --sizes 64KiB,256KiB,1MiB,5MiB,16MiB --parser markdown-it,adapter,windowed-adapter --iterations 1 --warmup 0 --json
```

The JSON was written to `target/markdown-phase18_5-benchmark.json` during verification and remains a local artifact, not a committed fixture. It reported `total_rss`, `baseline_rss`, `document_memory`, `markdown_parser_temporary_allocations`, `retained_decoration_cache_memory`, and `markdown_overhead` for each parser path. The full-document `markdown-it` and `adapter` rows are advisory evidence only for medium/large files; `windowed-adapter` is the ordinary editor path for visible Markdown refresh.

| Corpus  |                      Full `markdown-it` |   Full adapter advisory | `windowed-adapter` visible path | Editor-parity finding                                                                                                                                             |
| ------- | --------------------------------------: | ----------------------: | ------------------------------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 64 KiB  | 20.127 ms, `markdown_overhead` 1.76 MiB |     19.077 ms, 4.10 MiB |             14.199 ms, 3.73 MiB | Small-file paths stayed under the 30 MiB overhead budget; status/fallback check took 0.410 ms.                                                                    |
| 256 KiB |                     30.211 ms, 5.78 MiB |    51.087 ms, 16.65 MiB |             16.948 ms, 3.61 MiB | Windowed path stayed bounded while full adapter remained allowed only because this is small; status/fallback check took 0.074 ms.                                 |
| 1 MiB   |                   136.551 ms, 26.31 MiB |   277.604 ms, 37.73 MiB |             28.511 ms, 3.62 MiB | Full adapter exceeded the overhead budget; windowed path stayed under budget; status/fallback check took 0.111 ms.                                                |
| 5 MiB   |                   557.065 ms, 76.14 MiB |  934.864 ms, 148.74 MiB |             21.954 ms, 3.64 MiB | Full-document paths are not hot-path eligible; windowed path stayed under budget; status/fallback check took 0.113 ms.                                            |
| 16 MiB  |                 1312.502 ms, 261.71 MiB | 2356.308 ms, 750.48 MiB |             26.272 ms, 3.64 MiB | Large-file editor path met the `windowed-adapter markdown_overhead <= 30 MiB` target and avoided full-document parser input; status/fallback check took 0.260 ms. |

Deterministic guards now verify the same policy without machine-variant timing thresholds: `markdown_windowed_benchmark_uses_real_parser_and_repo_corpus`, `markdown_benchmark_json_reports_editor_parity_categories`, `markdown_large_file_memory_overhead_fits_budget`, `markdown_full_document_adapter_is_not_large_file_hot_path`, `markdown_large_file_typing_does_not_wait_for_windowed_parse`, and `markdown_full_document_adapter_is_not_large_file_hot_path_static_guard`.

### Large-file Markdown editor-parity contract (Phase 18.5)

Established editor parity means responsive typing and scrolling with bounded syntax work, not synchronous full-document Markdown decoration. The Phase 18.5 contract is:

- **Small Markdown files (`<= 1 MiB`)**: full-document `markdown-it` parsing and adapter work may run on open/reload or explicit resync when advisory local results stay comfortably below interactive thresholds, but it still must not block keypress-to-local-paint.
- **Medium Markdown files (`> 1 MiB` and `<= 5 MiB`)**: viewport-first/windowed parsing is the default for ordinary edits and scroll. Full-document work is allowed only as cancellable idle/background validation and must not be part of open, edit, or scroll response.
- **Large Markdown files (`> 5 MiB`, including the 16 MiB target)**: ordinary open, edit, and scroll paths must not run full-document parse/decorate. The package must parse bounded viewport/near-viewport windows, publish bounded decoration chunks, cancel stale work, and degrade to partial/plain-text highlighting when budgets would be exceeded.

Editor-comparison targets for large Markdown workflows are local advisory targets until deterministic cross-machine enforcement exists:

| Target                            | Phase 18.5 expectation                                                                                                                                            | Measurement path                                                                                                                            |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Typing/local paint                | `<= 16 ms` p95; Markdown parser delay may only affect decoration freshness                                                                                        | Existing `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `markdown_typing_does_not_wait_for_markdown_it_parse`, and future large-file typing guard |
| Scroll/render-adjacent work       | `<= 16 ms` p95 for local visible extraction/layout/paint-adjacent work                                                                                            | Existing `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` and future windowed decoration scroll benchmark                                      |
| Visible decoration refresh        | Target `<= 100 ms` p95 for viewport/near-viewport Markdown chunks on local benchmark hardware; stale chunks may temporarily remain or clear                       | Future `windowed-adapter` benchmark and decoration chunk publication tests                                                                  |
| Parser cancellation               | Superseded viewport/edit parse work should be cancelled or marked stale before publishing, target `<= 50 ms` p95 cancellation observation in local tests          | Future parse-window coordinator tests                                                                                                       |
| Parser/decorator CPU by file size | Full-document path may remain advisory for `<= 1 MiB`; `5 MiB` and `16 MiB` ordinary workflows must use bounded windows rather than full-document adapter timings | `tools/bench/markdown-parser.mjs` full-document evidence plus future windowed mode                                                          |
| Markdown memory overhead          | `<= 30 MiB` retained/temporary Markdown-specific overhead for the 16 MiB workflow                                                                                 | Future benchmark JSON memory categories and cache accounting tests                                                                          |

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

| Corpus    |                                                                                            `mdast-util-from-markdown` `fromMarkdown` |                             `markdown-it` parse |                                                     Package adapter path |
| --------- | -----------------------------------------------------------------------------------------------------------------------------------: | ----------------------------------------------: | -----------------------------------------------------------------------: |
| 1.01 MiB  |                                                                                1,278.715 ms, 37,132 mdast nodes, peak RSS 315.33 MiB |   66.528 ms, 46,877 tokens, peak RSS 298.63 MiB |                         49,311.589 ms, 15,182 spans, peak RSS 297.46 MiB |
| 5.03 MiB  |                                                                              16,239.409 ms, 181,939 mdast nodes, peak RSS 716.49 MiB | 397.630 ms, 227,471 tokens, peak RSS 727.71 MiB | Not run; 1 MiB adapter result is already too slow for full-document use. |
| 16.03 MiB | Did not complete within a 120 second local guard window; an earlier combined run also exceeded 600 seconds before producing results. | 849.659 ms, 725,141 tokens, peak RSS 434.30 MiB |                            Not run; full-document adapter is infeasible. |

The `mdast-util-from-markdown` adapter result above is historical replacement evidence, not an active implementation path. The active package now depends on `markdown-it`; future large-file work should optimize the token-stream adapter and viewport/range mapping rather than restore mdast. Do not add full-document parser IPC or client-side JavaScript to compensate.

Manual smoke command:

```text
cargo run -- smoke-gui --config-fixture markdown-mode
```

The smoke fixture validates package activation, command/action provenance, parse/decorations status, inert `Markdown Preview` SDUI, and plain document fallback behavior without reading arbitrary user paths, opening network listeners, granting shell authority, exposing document contents, or executing client-side JavaScript.

## Phase 18.21 LSP bridge / document-analysis budgets

Phase 18.21 keeps Rust core LSP-wire neutral. Deterministic hard guards cover session/worker ceilings and payload budgets; real language-server wall-clock timings stay environment-gated and advisory.

### Deterministic hard guards

| Focus area                                                    | Budget                                                               | Enforcement                                                                         |
| ------------------------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Language-server stdin/stdout chunk                            | <= 1048576 bytes (`LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`)            | `cargo test --test security language_server_authority::` / `performance_protocol`   |
| Language-server stderr retain                                 | <= 65536 bytes (`LANGUAGE_SERVER_STDERR_BUDGET_BYTES`)               | `cargo test --test security language_server_authority::`                            |
| Concurrent language-server sessions                           | <= 16 (`LANGUAGE_SERVER_MAX_SESSIONS`)                               | `cargo test --test security language_server_authority::`                            |
| Document-analysis workers                                     | <= 4 (`DOCUMENT_ANALYSIS_MAX_WORKERS`)                               | `cargo test --test editor editor_performance_invariants::` / `performance_protocol` |
| Document-analysis worker heap                                 | <= 67108864 bytes (`DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES`)            | `cargo test --test protocol performance_protocol::`                                 |
| Documents per analysis worker                                 | <= 32                                                                | `cargo test --test protocol performance_protocol::`                                 |
| Synced document text                                          | <= 262144 bytes (`DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES`)             | document-analysis unit tests + performance locks                                    |
| Analysis input mailbox                                        | <= 64 events / 2097152 bytes                                         | document-analysis unit tests                                                        |
| Analysis output queue                                         | <= 64 events / 524288 bytes                                          | document-analysis unit tests                                                        |
| Pending child requests                                        | <= 8                                                                 | shared LSP client + performance locks                                               |
| Decoration / diagnostics / completion / intelligence payloads | <= 8192 / 8192 / 16384 / 16384 bytes                                 | `cargo test --test protocol performance_protocol::`                                 |
| Fake-server bridge matrix latency                             | < 5 s wall clock for open/init/request/shutdown across four packages | `cargo test --test protocol performance_protocol::fake_server_bridge_matrix`        |

### Advisory measurements

- Keep using `cargo bench --bench first_party_language_baselines` for Tree-sitter open/incremental/scroll baselines before and after enabling bridge packages (`--save-baseline pre-lsp` / `--baseline-lenient pre-lsp`).

```text
cargo bench --bench first_party_language_baselines -- --save-baseline pre-lsp
cargo bench --bench first_party_language_baselines -- --baseline-lenient pre-lsp
```

- Real rust-analyzer / typescript-language-server / marksman open/init/request latency is measured only under `CLAY_LSP_REAL_SMOKE=1` via `cargo test --test runtime lsp_real_servers:: -- --nocapture`. Do not promote those timings to Criterion CI gates.
- Edit acknowledgement and local paint must never wait on worker/JS/subprocess work; see `tests/editor_performance_invariants.rs`.

## Plan 087 completion projection budgets

The Clay-owned completion surface uses the shared transient-menu item cap and
adds only small geometry ceilings. `COMPLETION_MAX_VISIBLE_ROWS` bounds the
rows exposed in the caret-adjacent viewport before scrolling; the underlying
result remains capped by `COMPLETION_RESULT_MAX_ITEMS`. `COMPLETION_MAX_WIDTH_PX`
keeps the surface compact while the active pane clamps its final rect. Neither
budget adds work to ordinary typing, paint, or layout when no completion menu is
active.

| Focus area                  | Budget                                                | Enforcement                                                                                         |
| --------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Caret-adjacent visible rows | 8 visible rows (`COMPLETION_MAX_VISIBLE_ROWS`)        | `cargo test --test protocol performance_budgets::plan087_completion_surface_budgets_are_documented` |
| Caret-adjacent width        | 480 logical px (`COMPLETION_MAX_WIDTH_PX`)            | same budget lock + `shell::package_ui` geometry tests                                               |
| Completion result items     | <= 256 (`COMPLETION_RESULT_MAX_ITEMS`)                | completion protocol/result validation + transient-menu cap                                          |
| Completion projection work  | bounded item list; no IPC/JS/file I/O in paint/layout | `tests/editor_performance_invariants.rs` and pane/menu unit tests                                   |

## Phase 19 hot-reload runtime-state budgets

Phase 19 keeps reload evaluation/commit off the ordinary edit and paint paths. Complete runtime-generation snapshots reuse the existing 1 MiB IPC frame ceiling; the 768 KiB payload / 16 ms install figures are review thresholds for a future diff upgrade, not soft pass/fail gates.

### Deterministic hard guards

| Focus area                           | Budget                                                                  | Enforcement                                                                                                    |
| ------------------------------------ | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Runtime-state broadcast capacity     | 16 (`RUNTIME_STATE_BROADCAST_CAPACITY`)                                 | `cargo test --test protocol performance_protocol::phase19_runtime_state_snapshot_and_grace_budgets_are_locked` |
| Snapshot document / diagnostic caps  | 64 / 32                                                                 | same + `RuntimeStateSnapshot::validate`                                                                        |
| Snapshot hard frame ceiling          | <= 1 MiB (`DEFAULT_MAX_FRAME_SIZE`)                                     | prepare encode-check + `tests/runtime_update_protocol.rs`                                                      |
| Diff-upgrade review payload          | 768 KiB p95 (`RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES`)        | budget lock + representative encode test                                                                       |
| Diff-upgrade review install          | 16 ms p95 (`RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS`)                  | budget lock; advisory until measured                                                                           |
| Previous-behavior grace              | 2000 ms / 256 transactions                                              | `cargo test --lib behavior::tests` / grace integration tests                                                   |
| Reload work outside editor hot paths | no candidate validate/install/reload symbols in paint/text-event bodies | `runtime_generation_install_stays_outside_paint_and_text_event_hot_paths`                                      |
| Edits during blocked candidate       | EditAck continues while reload waits on test barrier                    | `typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier`                              |

### Focused verification

```text
cargo test --test runtime persistent_runtime_hot_reload::
cargo test --test runtime runtime_update_protocol::
cargo test --test protocol performance_protocol::phase19_runtime_state
cargo test --test editor editor_performance_invariants::runtime_generation_install
cargo test --lib typing_and_edit_ack_continue_while_candidate
cargo test --lib failed_reload_broadcasts_diagnostic_but_no_generation_snapshot
cargo test --lib successful_reload_is_observed_as_one_generation_by_all_clients
cargo test --lib reload_preserves_authority_denials_and_cleans_old_lsp_worker
```

## Validation

Run the fixture tests after changing generator logic:

```text
cargo test --test protocol perf_fixtures::
```

Run focused profiling-hook tests after changing metric collection logic:

```text
cargo test perf_recorder -- --nocapture
cargo test editor_visible_extraction_records_metric_when_enabled
```

Run protocol/queue performance guards after changing client edit queue, server acknowledgement/rejection, or codec payload handling:

```text
cargo test --test protocol performance_protocol::
```

Run Phase 18.21 fake-server and budget locks after changing LSP bridge or document-analysis limits:

```text
cargo test --test runtime lsp_bridge::
cargo test --test security language_server_authority::
cargo test --test protocol performance_protocol::phase18_21
cargo test --test editor editor_performance_invariants::document_analysis
```

Run the benchmark compile check after changing benchmark scaffolding or the measured non-interactive paths:

```text
cargo bench --no-run
```

These checks verify deterministic output, UTF-8 validity, shape coverage, exact byte sizing, output path constraints, disabled-by-default profiling behavior, snapshot sanitization, enabled editor metrics, client-first queue invariants, representative protocol payload budgets, documented benchmark command discoverability, and benchmark target compilation.

## Phase 22.6 window-model budgets (pane paint, tab switch, decoration traffic)

Phase 22.6 (plan 077 task 5) adds per-pane paint, tab-switch latency, and
multi-pane decoration-traffic budgets in the established split: deterministic
work-count/payload gates run on every push; wall-clock figures stay advisory
until the Phase 21 stable-CI-runner promotion rule is met.

### Deterministic hard guards

| Focus area                    | Budget                                                                                                                                       | Enforcement                                                                                          |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Per-pane paint chrome work    | linear in pane count (1 pane = 0 pieces, N panes = N pieces: N-1 dividers + focus ring)                                                      | `pane_chrome_geometry_work_scales_linearly_with_pane_count`                                          |
| Tab switch reserialization    | no document text serialization / client messages / tab-command enqueue in the shell + pane-host switch path                                  | `tab_switch_path_performs_no_document_reserialization` + `tab_switch_submits_no_actions_or_messages` |
| Multi-pane decoration traffic | per-pane <= 8192 bytes (`DECORATION_PAYLOAD_BUDGET_BYTES`); 4-pane aggregate <= 32768 bytes (`MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES`) | `four_pane_decoration_aggregate_payload_fits_budget`                                                 |
| Phase 22.6 constants          | pinned values + docs markers                                                                                                                 | `phase22_6_window_budget_constants_are_pinned_and_documented`                                        |

### Advisory wall-clock budgets (historical native-client record)

The pre-cutover native client measured these with the since-removed
`window_baselines` Criterion target (2026-08-08, sample 10):
`pane_paint_baselines` 1/2/4 panes ≈ 69/382/743 ns;
`tab_switch_baselines` 1/2/4 panes ≈ 88/389/807 ns — linear in pane count,
with the tab-switch pass roughly one chrome geometry pass over the target
tab. The advisory ceilings are pinned with ~1000x headroom for debug builds
and assistive-technology overhead (advisory ceilings pinned as
`PANE_PAINT_P95_BUDGET_MS` / `TAB_SWITCH_P95_BUDGET_MS`; current-state React
commit and tab-switch guarantees are enforced by the deterministic frontend
and bridge tests above).

<= 1 ms (P95, advisory) per pane paint and per tab switch remains the pinned
Phase 22.6 budget row.

### Phase 24.4 centered overlay guards (deterministic)

The centered Command Centre surface keeps paint/layout work independent of
document size: one token-driven full-window scrim fill plus the existing
bounded overlay subtree. The React Command Centre modal keeps one scrim, one
window-level host, window-bounded geometry, and no
blur/offscreen/filter/IPC/IO work in the render path; open/close cycles reuse
the mounted dialog without orphan roots (`frontend/src/command-centre`
component tests). The pre-cutover `centered_overlay_baselines` Criterion group
was removed with `window_baselines`.

## Plan 087 focused UI regression coverage

Plan 087 keeps UI regression in two layers: deterministic structural and
accessibility guards are blocking, while timing measurements stay local/advisory.
No pixel goldens are used; visual evidence comes from the CDP fixture captures
under `code-reviews/screenshots/2026-08-24-tauri-react-parity/`.

Blocking coverage includes:

- `frontend/src/test/shell.test.tsx` and the welcome-state component checks:
  loading, connected, runtime-error, local-fallback, and disconnected entry
  states with basename-only workspace labels and no ambient path leakage.
- `frontend/src/sdui` registry tests reject stale snapshots carrying a foreign document or behavior version before replacing rendered content.
- Frontend editor extension suites keep completion projection bounded (shared
  eight-row/480-pixel caps) and keep completion projection, filesystem,
  JavaScript, IPC, and shell work off the keystroke-to-local-paint path.
- Command Centre component tests check caret anchoring semantics of the server
  snapshot, modeless containment, selected-state labels, and absent command
  targets. Existing 60-result centered containment and 256-item
  sanitized-label tests remain blocking.

`window_baselines` also recorded advisory `completion_open_baselines`,
`completion_filter_baselines`, and `completion_layout_baselines` groups over
the bounded transient-menu projection, shared fuzzy matcher, and caret
geometry helper at 1/8/60/256-item scales (removed at cutover; dated medians
below remain the historical record).

Local advisory run (2026-08-14, optimized Criterion profile, 10 samples, 1 s
warm-up, 2 s measurement) produced these median estimates:

| Group                         | Input                                                     |                           Median |
| ----------------------------- | --------------------------------------------------------- | -------------------------------: |
| `completion_open_baselines`   | 1 / 8 / 60 / 256 items                                    | 2.41 / 13.40 / 89.95 / 362.10 µs |
| `completion_filter_baselines` | 16 empty-query / 60 `split` / 256 `split pane` candidates |        12.21 / 73.61 / 416.08 µs |
| `completion_layout_baselines` | 1@20 / 8@280 / 256@560 caret positions                    |            0.98 / 0.88 / 0.89 µs |

These wall-clock results remain local/advisory; deterministic row, geometry,
accessibility-tree, stale-provenance, and no-hot-path checks are the hard gate.

## Plan 089 editor, menu, tab, completion, and accessibility cost guards

Plan 089 extended the then-existing native benchmark targets with bounded
surfaces (`command_centre_open_baselines`, `completion_selection_baselines`,
`accessibility_tree_update_baselines` in `window_baselines`) — all removed at
the Plan 097 Phase 12 cutover; their dated measurements below remain history.

- `command_centre_open_baselines`: 16/60/256 inert catalogue projections;
- `completion_selection_baselines`: selected last-row projection at 1/8/60/256
  items;
- `accessibility_tree_update_baselines`: a retained Clay shell updates labels
  for 2/4/8/16 tabs after initial construction. The same owner/client-derived
  virtual IDs are reused; the timed closure excludes initial tree setup.

`completion_filter_baselines` measures the shared fuzzy matcher used by both
completion and Command Centre query updates. No benchmark opens IPC, invokes
package JavaScript, reads documents/files, or creates a network/process
authority boundary.

Live verification still covering these surfaces:

```text
cargo test --test runtime lsp_bridge::
```

The blocking guards are the retained no-document-reserialization,
no-hot-path-IPC/JS, completion-bound, payload, and responsive-layout pins plus
the frontend Command Centre/editor suites. (The stable-virtual-ID accessibility
guards were removed with the native tree builder.)

### Plan 089 local before/after record (2026-08-16)

The exact Plan 088 window command was run before and after adding these
measurements on the same Linux host. Medians are shown as low/mid/high inputs;
Criterion's saved-target comparisons remain advisory and are owned by the next
Plan 089 triage task.

| Group                                     |       Before median range |          After median range |
| ----------------------------------------- | ------------------------: | --------------------------: |
| `pane_paint_baselines` (1/2/4)            |             72/373/718 ns |             94/481/1.803 µs |
| `tab_switch_baselines` (1/2/4)            |             87/380/783 ns |            180/864/1.898 µs |
| `responsive_layout_baselines`             |              2.13–2.35 µs |                4.43–5.47 µs |
| `centered_overlay_baselines` (1/4/16)     |            216/240/220 ps |              527/391/360 ps |
| `completion_open_baselines` (1/8/60/256)  | 1.62/8.42/55.83/234.64 µs | 2.82/17.03/110.26/484.64 µs |
| `completion_filter_baselines` (16/60/256) |      8.04/45.96/255.61 µs |      15.97/100.49/533.28 µs |
| `completion_layout_baselines` (1/8/256)   |            550/545/554 ns |          967/1.084/1.118 µs |

New local estimates (10 samples, 1 s warm-up, 2 s measurement) were
`command_centre_open_baselines` 22.74/84.33/270.86 µs for 16/60/256,
`completion_selection_baselines` 2.93/14.80/97.85/448.24 µs for 1/8/60/256,
and `accessibility_tree_update_baselines` 70.61/121.98/208.08/410.33 µs for
2/4/8/16 tabs. The broad after-run shifts are not promoted to a regression or
budget change here; Plan 089's next Criterion-triage task must repeat the fixed
command and classify host variance, benchmark instability, or implementation
regression before any policy decision.

Existing editor/protocol/runtime checks remain the hard-path companions:
`editor_render_adjacent`, `client_edit_queue_pressure`/`server_document_acknowledgements`,
and `runtime_configuration_baselines`/`sdui_application_baselines`. The same
local run produced these current medians:

| Group                                             |                 Current median |
| ------------------------------------------------- | -----------------------------: |
| `editor_render_adjacent` (64 KiB / 1 MiB)         |           332.21 µs / 5.320 ms |
| `client_edit_queue_pressure` (1 / 64 / 256)       | 292.86 ns / 10.082 / 39.677 µs |
| `server_document_acknowledgements` (1 / 16 / 128) |    315.62 / 316.37 / 332.74 µs |
| `runtime_configuration_baselines`                 |                       5.916 µs |
| `sdui_application_baselines` (apply / codec)      |               1.970 / 1.117 µs |

Their wall-clock values remain machine-local; deterministic work, payload,
allocation, stable-ID, and hot-path guards are blocking.

### Plan 089 Criterion triage (2026-08-16)

The exact fixed-input command was run three times sequentially on the same
Linux host with no competing Cargo/rustc process. Benchmark inputs,
`--sample-size 10`, 1 s warm-up, and 2 s measurement stayed unchanged; CPU
pinning and a cross-machine runner were not claimed. Central medians below use
low/mid/high input order (or the documented input order for each group):

| Group                                            |                            Run 1 |                             Run 2 |                             Run 3 | Classification                            |
| ------------------------------------------------ | -------------------------------: | --------------------------------: | --------------------------------: | ----------------------------------------- |
| `pane_paint_baselines` (1/2/4)                   |             0.106/0.587/1.135 µs |              0.098/0.529/0.937 µs |              0.139/0.743/1.655 µs | Machine variance                          |
| `tab_switch_baselines` (1/2/4)                   |             0.139/0.600/1.000 µs |              0.199/0.743/1.282 µs |              0.215/0.823/1.317 µs | Machine variance                          |
| `responsive_layout_baselines` (six inputs)       |                     2.53–2.99 µs |                      3.81–5.37 µs |                      3.66–8.44 µs | Machine variance                          |
| `centered_overlay_baselines` (1/4/16)            |             0.261/0.253/0.250 ps |              0.321/0.512/0.539 ps |              0.511/0.504/0.548 ps | Benchmark instability: sub-ns timer scale |
| `completion_open_baselines` (1/8/60/256)         |   1.797/10.935/73.091/291.220 µs |   3.556/63.021/193.320/612.900 µs |   4.053/21.196/137.100/479.530 µs | Machine variance                          |
| `completion_filter_baselines` (16/60/256)        |         10.535/65.972/354.360 µs |          15.359/96.359/620.860 µs |         15.601/103.240/542.240 µs | Machine variance                          |
| `command_centre_open_baselines` (16/60/256)      |         13.140/43.678/176.020 µs |          22.035/65.416/253.520 µs |          26.724/51.886/316.750 µs | Machine variance                          |
| `completion_selection_baselines` (1/8/60/256)    |    1.815/9.977/68.526/281.160 µs |   7.434/31.876/149.440/498.410 µs |    2.370/16.965/76.521/324.000 µs | Machine variance                          |
| `accessibility_tree_update_baselines` (2/4/8/16) | 54.735/91.487/169.910/326.870 µs | 77.336/118.840/222.620/521.290 µs | 72.042/105.220/186.790/311.040 µs | Machine variance                          |
| `completion_layout_baselines` (1/8/256)          |             0.780/0.773/0.815 µs |              1.247/1.349/1.462 µs |              0.875/0.853/1.090 µs | Machine variance                          |

Criterion's saved-target direction confirms the instability rather than a
source regression: Run 1 reported 34 improvements, 1 regression, and 1
unchanged group; Run 2 reported 2 improvements, 33 regressions, and 1
unchanged group; Run 3 reported 14 improvements, 8 regressions, and 14
unchanged groups. No hot-path or benchmark code changed between these runs.
The centered-overlay values are below useful wall-clock resolution and are
classified as measurement instability; all other groups are machine-variance
warnings. No reproducible implementation regression was found, no budget was
raised, and no advisory result was promoted to CI policy. Re-run this exact
command on a stable/cross-machine runner before changing benchmark code or
policy.

## Plan 088 modernization conformance and responsive layout baselines

Plan 088 keeps modernization checks split into deterministic host-authority
conformance and local/advisory timing. Screenshot goldens remain deferred;
structural, typed, accessibility, payload, provenance, and hot-path checks are
blocking while Criterion timings stay machine-local until the stable-runner
promotion rule is met.

### Deterministic hard guards

- `bundled_theme_conformance_matrix` validates every bundled theme as inert
  style data and runs the active SDUI contrast gate.
- `catalog_is_drift_free_across_doc_enum_and_paint_path`,
  `style_variable_catalog_matches_components_md`, and
  `core_token_catalog_matches_tokens_md` keep source/catalog/token tables in
  sync; `component_catalog_status_partition_is_current` keeps implemented,
  planned, and reserved states explicit.
- `shell_chrome_paint_files_source_color_from_primitives_only`,
  `shell_chrome_paint_files_have_no_hardcoded_chrome_sizes`, and
  `hot_path_no_theme_resolution_or_package_js` keep modernized chrome
  token-driven and free of per-frame theme, JavaScript, IPC, filesystem, or
  shell work.
- `responsive_layout_work_preserves_sidebar_and_editor_bounds` covers the
  production SDUI left-slot decision at narrow, normal, wide, and large-UI
  typography inputs. Existing shell, SDUI, package-overlay, and contrast
  tests remain the state-level behavioral matrix.

### Responsive layout bounds (historical timing note)

The pre-cutover `responsive_layout_baselines` Criterion group measured the
sidebar/region geometry helper at 320/900/1200 logical pixels and UI sizes
12/24/96; the bench was removed with the native client. The decision itself is
unchanged and pinned by `responsive_layout_work_preserves_sidebar_and_editor_bounds`;
wide/narrow visual evidence comes from the UI-review harness captures.

This baseline is diagnostic only. The blocking contract is the typed layout
matrix and bounded geometry tests; do not promote wall-clock values to CI
thresholds without repeated stable-runner evidence.

## Phase 24.5 Command Centre budgets, guards, and browse-grant authority review

Phase 24.5 (plan 085 task 7) budgets the Command Centre in the established
split: deterministic work-count/payload gates run on every push; wall-clock
figures stay advisory until the Phase 21 stable-CI-runner promotion rule is
met (single-machine evidence only, as Plan 084 deferred menu-latency
measurement to this phase).

### Advisory budgets (`src/perf/budgets.rs`)

| Constant                                      | Value  | What it covers                                                                                            |
| --------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------- |
| `COMMAND_CENTRE_OPEN_P95_BUDGET_MS`           | 50     | one menu open: server catalogue snapshot + session construction + snapshot encode; no document-sized work |
| `COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS`  | 4      | one per-keystroke filter update: fuzzy-score the installed candidate list (<= 256 entries) locally        |
| `COMMAND_CENTRE_LISTING_MAX_ENTRIES`          | 256    | path-browser listing snapshot entry ceiling (aliases `TRANSIENT_MENU_MAX_ITEMS`)                          |
| `COMMAND_CENTRE_LISTING_PAYLOAD_BUDGET_BYTES` | 64 KiB | advisory serialized-size ceiling for one listing snapshot; far below the 1 MiB codec frame ceiling        |

`KEY_CHORD_PENDING_TIMEOUT_MS` (1500, advisory) bounds a stale pending
multi-stroke chord: the chord cancels and the next key re-evaluates fresh.

### Deterministic hard guards (pre-cutover suite removed; claims retained)

The pre-cutover `tests/editor_performance_invariants.rs` suite was removed with
the native client (`accessibility_updates_reuse_stable_virtual_ids_without_allocator_churn`
and `retained_accessibility_update_fixture_stays_bounded` among its guards;
see the performance-fixtures wiki page). Current equivalents: menu open reads
no document text and bounds the browse listing plan by
`COMMAND_CENTRE_LISTING_MAX_ENTRIES` (server connection/menu-session tests);
per-keystroke filters touch no `DocumentState`; the React Command Centre
renders only bounded server listings (`frontend/src/command-centre` tests);
the pending chord buffer keeps its one-stroke-per-outcome bound (lib chord
tests).

### Browse-grant authority review (recorded 2026-08-13)

The built-in browse grant (path-mode traversal outside workspace roots) is
reachable only from the user-driven built-in path-mode surface:

- The only session-opening helper (`open_command_centre_session`,
  `src/server/connection/mod.rs`) has exactly two call sites, both fed by
  user-driven client messages: the `CommandIntent` special case for the two
  builtin ids and menu activation of `controlCenter.openPath`.
- Package JavaScript runs in the op layer: no op/facade (including
  `commands.execute`) calls the helper or constructs a browse session; the
  package `executeCommand` facade validates and acknowledges without opening
  a session.
- Package `registerCommand` cannot claim either builtin id: command IDs must
  live in the package's own apiPrefix namespace and `clay.` ids are rejected
  (`control_center_command_ids_are_not_registerable_by_packages`).
- Source guards: `phase24_5_command_centre_sessions_are_not_a_package_programmatic_surface`
  (`tests/rust_visibility_api_mapping.rs`, security suite) pins the two-call-
  site invariant and the absence of browse-session construction outside the
  connection layer.
- Browsed-path conversions stay on the existing `SingleFile`/`Directory`
  grant paths; a file open converts browse authority into exactly one grant
  through the same selected-file open path as before.

## Phase 25 agent-host budgets

Phase 25 (plan 096) budgets the Prism host the same way as Command Centre:
deterministic size/source gates on every push; wall-clock spawn and
first-delta numbers stay advisory until the Phase 21 stable-runner rule.
Agent pickers reuse Command Centre open/filter budgets. Deltas never block
keypress-to-local-paint: `dispatch` returns after queueing, and paint reads
the last snapshot only.

### Advisory wall-clock (`src/perf/budgets.rs`)

| Constant                                     | Value | What it covers                                              |
| -------------------------------------------- | ----- | ----------------------------------------------------------- |
| `AGENT_DAEMON_SPAWN_P95_BUDGET_MS`           | 2000  | first `clay-agent` spawn + `initialize`                     |
| `AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS`  | 2000  | mock/local first `message_delta`; real model time is not CI |
| `AGENT_DELTA_IPC_P95_BUDGET_MS`              | 4     | apply one already-received delta into the transcript        |
| `COMMAND_CENTRE_OPEN_P95_BUDGET_MS`          | 50    | agent/provider/model/setup/session picker open              |
| `COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS` | 4     | picker filter keystroke                                     |

### Hard size caps (`src/protocol/agent.rs`)

| Constant                                 | Value   | What it covers                 |
| ---------------------------------------- | ------- | ------------------------------ |
| `AGENT_MAX_PROMPT_BYTES`                 | 32 KiB  | composer/prompt fail-closed    |
| `AGENT_DELTA_MAX_TEXT_BYTES`             | 8 KiB   | one inbound delta slice        |
| `AGENT_MAX_ENTRY_TEXT_BYTES`             | 32 KiB  | one coalesced transcript entry |
| `AGENT_MAX_SNAPSHOT_ENTRIES`             | 200     | retained entry count           |
| `AGENT_TRANSCRIPT_SNAPSHOT_BUDGET_BYTES` | 256 KiB | sum of retained entry text     |
| `AGENT_DAEMON_MAX_LINE_BYTES`            | 1 MiB   | NDJSON line / codec frame      |

### Deterministic hard guards

| Guard                                            | Claim                                                                |
| ------------------------------------------------ | -------------------------------------------------------------------- |
| `slow_daemon_submit_does_not_block_caller`       | prompt dispatch stays inside `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` |
| `mock_daemon_prompt_persists_no_secret_on_ack`   | first mock delta arrives inside the first-delta budget               |
| `transcript_caps_delta_entry_and_snapshot_bytes` | size caps hold                                                       |
| `agent_io_stays_off_paint_and_keypress`          | paint/keypress sources do not call the daemon                        |

## Phase 28.7 command/intelligence payload pins

Phase 28.7 (plan 094) reuses existing Phase 16/18 budgets. Phase 28.6
adds only the two bounded recency constants below; no new result allocation
budget. Link/inlay sets use the decoration cap. Ranking stays inside the completion
scan. Folding publish (when 28.3 lands) uses the folding-range cap. Hover
and decoration-intent payloads stay at the language-intelligence hover cap.
Wall-clock stays advisory (Phase 21). Structural gates are hard in
`cargo test`.

### Advisory / reused constants (`src/perf/budgets.rs`)

| Constant                                         | Value | What it covers                                                         |
| ------------------------------------------------ | ----- | ---------------------------------------------------------------------- |
| `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`             | 2048  | one folding-range publish; deny above cap, do not truncate             |
| `DECORATION_PAYLOAD_BUDGET_BYTES`                | 8192  | link + inlay decoration sets; same deny path as other kinds            |
| `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`         | 16384 | ranked completion result; no second allocation budget                  |
| `COMPLETION_RESULT_MAX_ITEMS`                    | 256   | ranking scan / result item cap                                         |
| `COMPLETION_RECENCY_MAX_ITEMS`                   | 4     | accepted completion strings carried in one request                     |
| `COMPLETION_RECENCY_MAX_ITEM_CHARS`              | 64    | per-string recency cap; keeps request payload bounded                  |
| `LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS` | 4096  | hover markdown; decoration hover/click intent stays at this size class |
| `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`          | 16    | must not regress; ranking / intent / fold-publish stay off this path   |

### Deterministic hard guards

| Focus area                    | Budget                               | Enforcement                                                                                                 |
| ----------------------------- | ------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| Folding-range payload         | <= 2048 bytes                        | `phase28_budget_constants_match_docs` + `folding_and_inlay_payloads_deny_above_cap`                         |
| Link/inlay decoration payload | <= 8192 bytes                        | same + existing `validate_decoration_publication` deny                                                      |
| Ranked completion result      | <= 16384 bytes                       | `completion_ranking_stays_inside_existing_scan_budget`                                                      |
| Ranked completion items       | <= 256 (COMPLETION_RESULT_MAX_ITEMS) | same scan / `check_result_payload_budget`                                                                   |
| Completion recency hints      | <= 4 × 64 chars                      | `CompletionRequest::validate`; boxed request field avoids `ClientMessage` size growth                       |
| Hover markdown                | <= 4096 chars                        | language-intelligence validation + `folding_and_inlay_payloads_deny_above_cap`                              |
| Keypress-to-local-paint       | <= 16 ms (P95, advisory)             | `completion_ranking_is_not_on_keypress_to_local_paint_path` + `hover_intent_is_not_on_paint_or_layout_path` |

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check                           | Result                                  | Evidence                                                                                                                                                            |
| ------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wide/narrow rendered surfaces   | PASS static                             | 20 CDP captures under `code-reviews/screenshots/2026-08-24-tauri-react-parity/` at 1440×900 and 780×900 show no clipping, duplicate overlay, or visible layout jank |
| Editor/package/Chat render cost | PASS structural; stream feel unresolved | Existing CodeMirror, SDUI, AG-UI, list, and hot-path tests pass; provider setup/input prevented a live streaming-latency claim                                      |
| Bundle budget                   | PASS                                    | Frontend build: shell 160.6 kB gzip / 180 kB budget; total 343.2 kB / 400 kB budget                                                                                 |
| Keyboard/filter/resize feel     | UNRESOLVED live                         | Host cannot safely deliver keyboard or compositor resize actions; no visual pass inferred from source/tests                                                         |

No performance budget was changed.

## Tauri / React client budgets

The webview bundle replaces the removed native-client Criterion groups as the
client-side size gate. `npm --prefix frontend run check:budget` is the hard
gzip gate (`frontend/scripts/bundle-budget.mjs`, wired into CI).

| Surface        | Budget                          | Enforcement                          |
| -------------- | ------------------------------- | ------------------------------------ |
| Startup shell  | <= 180 kB gzip (startup shell)  | `frontend/scripts/bundle-budget.mjs` |
| Total frontend | <= 400 kB gzip (total frontend) | `frontend/scripts/bundle-budget.mjs` |

Latest measured production build (2026-08-24): shell 161.0 kB gzip,
total 343.7 kB gzip — within budget, none raised.

### Editor offset conversion and viewport requests

The keystroke-to-paint rule (no full-document work per edit) is enforced in
the client by construction:

- `frontend/src/editor/position-index.ts` defines the shared incremental
  `BytePositionIndex` and `bytePositionField`. It builds once for a CodeMirror
  `Text`, stores numeric UTF-16/UTF-8 widths in 64-line chunks, and path-copies
  only the whole-line region touched by each change. Edit emission, viewport
  requests, decoration/diagnostic/fold projection, completion, intelligence,
  and selection conversion reuse `positionIndex(state)`; no WeakMap document
  line table is rebuilt on the keystroke path. Conversion remains a tree
  descent plus an O(line) intra-line scan, so a single 1 MiB line is an
  explicit advisory ceiling. `position-map.test.ts` differential/property
  tests keep the incremental field equal to the linear reference.
- Plan 099 syntax sessions: native Tree-sitter parses run on a bounded
  blocking executor (`SYNTAX_EXECUTOR_MAX_JOBS = 4` permits) inside
  per-document sessions with latest-wins mailboxes; each document owns its
  parser (no grammar-global mutex), the per-document tree cache is bounded
  (`SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES = 64`), and repeat document opens
  reuse a cached mode activation instead of evaluating a generated module in
  V8 (measured ~15 ms per V8 open activation on the dev host; see
  `mode_activation_cache_hit_skips_generated_module_evaluation`). See
  `docs/wiki/modules/syntax-sessions.md`.
- Viewport requests use the protocol v29 atomic render protocol
  (`ViewportRenderRequest`/`ViewportRenderPatch`): immediate first send with
  a monotonic request id, latest-wins follow-up the moment the patch reply
  lands, stale request ids dropped on arrival. The 400 ms heuristic safety
  valve is removed — every request receives exactly one terminal
  complete/empty/rejected answer, so scroll storms cost at most one request
  per round trip instead of one per input event.
- Feature replies batch into ONE editor transaction per envelope
  (`EditorProjection.handleEnvelope` buffers decoration/diagnostic/fold
  effects), so a multi-window reply costs one update cycle, not N reflows.
- Server-side, `handle_viewport_render_request` builds one rope-sliced parse
  window for the requested range via `Document::parse_windows_covering` (cap 1).
  The client already sends one on-screen fragment clamped to 64 KiB chars, so
  a long-line min/max of `visibleRanges` cannot stall the atomic remaining
  counter. It no longer clones the full document text or rescan the prefix
  for the base line.

### Linux compositing path (verified 2026-08-24)

The real desktop (`clay client` → `clay-desktop` → wry → WebKitGTK 2.52.5)
was probed on the reference Linux host (Wayland session, Mesa AMD iGPU,
1920x1200@60 scale 1.0) to rule the shell's compositor path out as a jitter
source:

- GTK runs native Wayland (`GDK_BACKEND=wayland`, `WAYLAND_DISPLAY=wayland-0`).
- The GPU DMA-BUF renderer is active: the UI process holds
  `/dev/dri/renderD129` and `/dmabuf:` file descriptors; no EGL/GBM fallback
  warnings appear with `WEBKIT_DEBUG=GLContext,Compositing,Layers`.
- Zero idle CPU: both `clay-desktop` and `WebKitWebProcess` consumed no CPU
  ticks over a 5 s idle sample, so there is no software-repaint storm.
- Integer display scale (1.0), non-VRR current mode — neither fractional-
  scaling repaint amplification nor VRR judder applies on this host.

Clay sets no WEBKIT_* environment overrides; the defaults are correct here.
If a user's machine falls off the accelerated path (commonly NVIDIA + DMABuf
renderer artifacts), the escape hatches are `WEBKIT_DISABLE_DMABUF_RENDERER=1`
or `WEBKIT_DISABLE_COMPOSITING_MODE=1` — both force slower software paths,
so treat them as user-side diagnostics, not shipped configuration. Lazy workflow/chat/package
chunks are classified separately from the shell so async features never
inflate the startup path. Keystroke-to-local-paint stays owned by CodeMirror
with bounded ordered deltas queued asynchronously; server work, package
JavaScript, IPC batching, and AI streams never sit on the local paint path.
