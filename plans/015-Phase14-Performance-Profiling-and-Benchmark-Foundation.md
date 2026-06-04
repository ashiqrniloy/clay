# Phase 14: Performance Profiling and Benchmark Foundation

## Objectives
- Establish repeatable performance benchmarks for Clay's current plain-text editor, server/client synchronization, IPC, layout, rendering-adjacent, SDUI, and runtime/configuration paths before package-controlled modes are introduced.
- Add large-file fixture generation and validation workflows that can be used by automated benches, targeted tests, and manual GUI smoke runs.
- Add low-overhead profiling hooks around editor input, visible extraction, layout/cache invalidation, render preparation, SDUI application, client queues, server acknowledgements, IPC payloads, and runtime/configuration evaluation.
- Define concrete latency, memory, payload, and hot-path budgets that future package/mode primitive work must satisfy.
- Add deterministic performance guards where reliable, while keeping machine-variant profiling as documented local commands instead of brittle CI thresholds.

## Expected Outcome
- Developers can generate/open representative large text fixtures and run documented benchmark commands such as `cargo bench`, targeted benchmark filters, and manual `cargo run -- smoke-gui` workflows.
- Criterion-based baseline benches report throughput and latency for text extraction, editing, layout invalidation, IPC codec payloads, server acknowledgements, runtime/configuration evaluation, and SDUI application where the code paths are testable without an interactive GPU.
- Internal measurement hooks remain completely disabled during ordinary user sessions and can only collect metrics when an explicit developer opt-in is present, such as `CLAY_PERF_PROFILE=1` or a documented `--profile-perf`/benchmark activation path, without adding blocking work to Masonry paint/text-event handlers or ordinary typing.
- Any regressions discovered by the baseline are either fixed in scoped incremental layout/viewport/cache work or captured as explicit follow-up with measured evidence.
- Performance budgets and profiling commands are documented for future package, primitive, and Markdown-mode phases.
- Clay JS API docs, configuration docs, generated registry state, and the implementation wiki are updated or explicitly verified as unchanged for performance-only internal surfaces.

## Tasks

- [x] Add large-file fixtures and generation workflows
  - Acceptance Criteria:
    - Functional: A deterministic workflow generates representative UTF-8 plain-text fixtures at multiple sizes, including long lines, many short lines, mixed Unicode scalar content, and newline-heavy content for open/edit/scroll validation.
    - Performance: Fixture generation is bounded, streaming or chunked where practical, and avoids loading unnecessary duplicate full-buffer copies during validation.
    - Code Quality: Fixture logic is reusable by benchmarks, tests, and manual smoke workflows without hard-coded machine-specific paths.
    - Security: Fixture generation writes only to explicitly requested test/output paths under the repository target or fixture directory and does not read workspace secrets, invoke a shell, or grant extra filesystem authority to the client.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 14: Add repeatable large-file open/generate workflows for manual and automated validation.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Preserve server authority, document phase boundaries, and avoid hidden filesystem authority.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Avoid full-document IPC for ordinary edits and keep rendering viewport-bounded.
    - Options Considered:
      - Commit multi-megabyte fixtures: simple, but bloats the repository and makes changing cases noisy.
      - Generate random files: broad coverage, but hard to reproduce and compare across baseline runs.
      - Add deterministic fixture generation: preferred because benchmarks and manual validation can reproduce the same inputs without large committed artifacts.
    - Chosen Approach:
      - Add a deterministic Rust fixture generator and optional command/test helper that writes known text shapes to `target/perf-fixtures/` by default, with small committed samples only if needed for tests.
    - API Notes and Examples:
      ```text
      cargo run -- perf-fixture --kind mixed-unicode --size-mib 16 --output target/perf-fixtures/mixed-16m.txt
      cargo run -- smoke-gui --file target/perf-fixtures/mixed-16m.txt
      ```
    - Files to Create/Edit:
      - `src/perf/mod.rs`: Internal performance fixture module root.
      - `src/perf/fixtures.rs`: Deterministic text fixture generation helpers.
      - `src/lib.rs`: Expose the internal performance helper module to tests/benches.
      - `src/main.rs`: Developer-only `perf-fixture` subcommand for fixture generation.
      - `tests/perf_fixtures.rs`: Tests for deterministic fixture shapes and path constraints.
      - `docs/development/performance.md`: Document fixture generation and manual large-file validation commands.
      - `docs/index.md`: Link the performance developer guide.
      - `docs/wiki/index.md`: Link the internal performance fixture implementation page.
      - `docs/wiki/modules/performance-fixtures.md`: Document fixture internals and security boundaries.
    - References:
      - `roadmap.md` Phase 14
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `perf_fixture_generation_is_deterministic`: Same kind/size/seed produces byte-identical UTF-8 output.
    - `perf_fixture_generation_rejects_unsafe_output_paths`: Traversal or unauthorized absolute paths are rejected where path validation is implemented.
    - `perf_fixture_shapes_include_unicode_and_long_lines`: Generated cases exercise scalar movement, visible extraction, and layout edge cases.
    - Validation Run: `cargo fmt --check`, `cargo test --all-targets`, and `cargo run -- perf-fixture --kind mixed-unicode --size-mib 1 --output target/perf-fixtures/test-cli.txt` passed.

- [x] Install Criterion benchmark scaffolding for baseline measurements
  - Acceptance Criteria:
    - Functional: `cargo bench` runs focused Criterion benchmark groups for editor buffer operations, viewport extraction, layout/cache invalidation, protocol codec payloads, server document edits/acknowledgements, SDUI application helpers, and runtime/configuration evaluation where practical.
    - Performance: Benchmarks report throughput or input size for large-data cases and keep setup costs outside timed loops with batched setup where needed.
    - Code Quality: Benchmark code uses public crate/test harness entry points where possible and avoids duplicating production logic or relying on GUI-only state that cannot run in CI.
    - Security: Benchmarks use generated local fixtures and test-only endpoints; they do not open network listeners, read user configuration by default, or require real workspace permissions.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/criterion-rs/criterion.rs`: `criterion_group!`, `criterion_main!`, `benchmark_group`, `BenchmarkId`, `Throughput`, `iter_batched`, `BatchSize`, `measurement_time`, `warm_up_time`, `--save-baseline`, and `--baseline`.
      - `roadmap.md` Phase 14: Add baseline profiles before package and mode work begins.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer automated deterministic checks and actionable commands.
    - Options Considered:
      - Use only `cargo test --release` timing assertions: lightweight, but noisy and statistically weak for regressions.
      - Add Criterion benches for measurable pure/server paths: preferred for baseline statistics, throughput reporting, and local baseline comparison.
      - Add GPU/render frame benchmarks immediately: useful eventually, but likely brittle before visual observability/headless support in Phase 15.
    - Chosen Approach:
      - Add Criterion as a development dependency and create benchmark groups around deterministic non-interactive paths first, leaving interactive/GPU render timing as instrumentation/manual smoke until suitable automation exists.
    - API Notes and Examples:
      ```rust
      use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

      fn visible_extraction(c: &mut Criterion) {
          let mut group = c.benchmark_group("editor_visible_extraction");
          for bytes in [1 << 20, 16 << 20] {
              group.throughput(Throughput::Bytes(bytes as u64));
              group.bench_with_input(BenchmarkId::from_parameter(bytes), &bytes, |b, &bytes| {
                  b.iter_batched(
                      || make_fixture_buffer(bytes),
                      |buffer| extract_visible_text(&buffer),
                      criterion::BatchSize::LargeInput,
                  )
              });
          }
          group.finish();
      }

      criterion_group!(benches, visible_extraction);
      criterion_main!(benches);
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: Add `criterion` under `[dev-dependencies]` and bench target metadata if needed.
      - `Cargo.lock`: Lock Criterion and transitive benchmark-only dependencies.
      - `src/perf/mod.rs`: Expose internal baseline helper module for benchmark targets.
      - `src/perf/baselines.rs`: Shared deterministic benchmark setup helpers for editor, protocol/server, runtime/configuration, and SDUI paths.
      - `src/server/mod.rs`: Make the document module visible within the crate so internal perf helpers can benchmark `DocumentState` without public API exposure.
      - `benches/editor_baselines.rs`: Buffer, viewport extraction, edit, layout/cache invalidation, and scroll-adjacent benches.
      - `benches/protocol_server_baselines.rs`: IPC codec payload and server document acknowledgement benches.
      - `benches/runtime_sdui_baselines.rs`: Runtime/configuration and SDUI validation/application benches where deterministic.
      - `docs/development/performance.md`: Document `cargo bench`, baseline save/compare commands, and interpretation notes.
      - `docs/wiki/modules/performance-fixtures.md`: Document Criterion baseline internals and validation commands.
    - References:
      - Context7 `/criterion-rs/criterion.rs`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `cargo bench --no-run`: Verify benchmark targets compile in CI or local validation.
    - `cargo bench --bench editor_baselines editor_visible_extraction -- --sample-size 10 --warm-up-time 1 --measurement-time 2`: Local smoke command validates the benchmark group can execute quickly.
    - `cargo bench --benches -- --save-baseline phase14-baseline`: Manual baseline command documented for developer comparison.
    - Validation Run: `cargo fmt --check`, `cargo test --all-targets`, `cargo bench --no-run`, and `cargo bench --bench editor_baselines editor_visible_extraction -- --sample-size 10 --warm-up-time 1 --measurement-time 2` passed.

- [x] Add low-overhead profiling hooks and metric snapshots
  - Acceptance Criteria:
    - Functional: Instrumentation records scoped durations, counters, queue depths, payload sizes, and version/transaction metadata for the Phase 14 focus paths without changing functional behavior, but only when profiling is explicitly activated by `CLAY_PERF_PROFILE=1`, an equivalent documented developer flag such as `--profile-perf`, or a test/bench-only activation helper.
    - Performance: The default recorder is no-op and ordinary user sessions do not collect snapshots, allocate metric buffers, emit profile output, or evaluate expensive timing/reporting paths; enabled hooks avoid allocation-heavy or blocking work in keypress, paint, layout, and IPC hot paths.
    - Code Quality: Profiling types are small, typed, testable, and isolated from business logic; activation is centralized in a `PerfConfig`/recorder factory instead of scattered environment checks or ad hoc `println!` timing.
    - Security: Metrics are sanitized and do not record document contents, file contents, secrets, absolute user paths in public diagnostics, or JavaScript source bodies.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 14: Add trace points around editor input, visible extraction, Parley layout/cache invalidation, Vello/GPU rendering, SDUI application, client send queues, server acknowledgement, and runtime/configuration evaluation.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: No blocking IPC or server work in Masonry paint/text-event handlers; use bounded queues and per-document ordering.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: State ownership and security boundaries explicitly.
    - Options Considered:
      - Leave profiling always on with a low-overhead recorder: simpler to reason about, but rejected because ordinary user sessions must not collect metric snapshots or pay reporting/allocation costs.
      - Add a full tracing/logging dependency immediately: powerful, but may be more API surface and configuration than needed for the first baseline.
      - Use raw log statements: quick, but noisy, hard to test, and easy to leak sensitive context.
      - Add Clay-owned metric scopes/counters behind an explicit developer opt-in: preferred because it is minimal, typed, off by default, and can later bridge to external tracing if needed.
    - Chosen Approach:
      - Implement an internal `PerfConfig` plus `PerfRecorder`/snapshot abstraction with no-op default behavior. Collection is enabled only by a documented opt-in path (`CLAY_PERF_PROFILE=1`, a developer CLI flag such as `--profile-perf`, or a test/bench helper), and all normal startup paths construct the no-op recorder.
    - API Notes and Examples:
      ```text
      CLAY_PERF_PROFILE=1 cargo run -- smoke-gui --profile-perf --file target/perf-fixtures/mixed-16m.txt
      cargo bench --bench editor_baselines editor_visible_extraction
      ```
      ```rust
      let perf = PerfRecorder::from_config(PerfConfig::from_env_and_args());
      let _span = perf.scope("editor.visible_extraction");
      let visible = viewport.extract_visible_text(&buffer);
      perf.record_bytes("ipc.payload_bytes", encoded_len as u64);
      ```
    - Files to Create/Edit:
      - `src/perf/metrics.rs`: Internal metric configuration, activation parsing, recorder factory, no-op recorder, snapshots, sanitization helpers, global recorder installation, and tests.
      - `src/perf/mod.rs`: Expose the metrics module.
      - `src/main.rs`: Wire the documented developer profiling flag/environment activation into developer workflows while keeping normal runs disabled by default.
      - `src/editor/surface.rs`: Hook editor visible extraction and local edit counters through an explicit test/bench recorder helper without blocking the event handler.
      - `src/editor/layout.rs`: Hook layout paint, rebuild, cache-hit, and cache-miss timing/counters.
      - `src/masonry_editor.rs`: Hook render-preparation and paint-adjacent timing without GPU synchronization.
      - `src/masonry_sdui.rs`: Hook SDUI snapshot/update timing and payload node/operation counts.
      - `src/client/mod.rs`: Hook client edit queue depth and send/ack timing metadata.
      - `src/server/document.rs`: Hook server document edit application and acknowledgement counters.
      - `src/protocol/codec.rs`: Hook encoded/decoded frame sizes and oversized rejection counts.
      - `src/server/js_runtime.rs`: Hook runtime/configuration evaluation timing.
      - `docs/development/performance.md`: Document profiling activation, metrics scope, and validation commands.
      - `docs/index.md`: Update the performance guide summary.
      - `docs/wiki/index.md`: Update the performance implementation page summary.
      - `docs/wiki/modules/performance-fixtures.md`: Document metrics internals, hooks, security boundaries, and tests.
    - References:
      - `roadmap.md` Phase 14
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
  - Test Cases to Write:
    - `perf_recorder_is_disabled_without_env_or_flag`: Normal startup creates a no-op recorder and produces no snapshots or profile output.
    - `perf_recorder_enables_only_with_env_flag_or_test_helper`: `CLAY_PERF_PROFILE=1`, the documented developer flag, or a bench/test helper activates collection.
    - `perf_recorder_noop_does_not_allocate_snapshots`: Disabled instrumentation is inert for production paths.
    - `perf_snapshot_sanitizes_paths_and_content`: Snapshots omit document text, JavaScript source, and unsanitized user paths.
    - `editor_visible_extraction_records_metric_when_enabled`: A focused editor test captures the expected metric name and count.
    - `protocol_codec_records_payload_size_metric`: Encoding/decoding records byte counts without changing codec results.
    - Validation Run: `cargo fmt --check`, `cargo test --all-targets`, and `cargo bench --no-run` passed.

- [x] Measure and guard client/server edit latency, queues, and IPC payload sizes
  - Acceptance Criteria:
    - Functional: Tests or benches measure local edit application latency, client edit queue behavior, server acknowledgement latency, stale-edit/resync paths, and representative `rkyv` payload sizes for snapshots, edits, acknowledgements, behavior manifests, and SDUI messages.
    - Performance: Ordinary edits remain client-first and do not require synchronous IPC, server, JavaScript, file IO, or full-document serialization before local visible state changes.
    - Code Quality: Measurements reuse existing protocol/server helpers and assert invariants separately from machine-dependent latency numbers.
    - Security: IPC test inputs remain fallible and bounded; invalid or oversized frames are still rejected and benchmarks do not bypass validation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep `rkyv` behind a length-prefixed codec, validate archived bytes, bound frame sizes, avoid full-document IPC for ordinary edits, and use bounded queues.
      - `roadmap.md` Phase 14: Measure keypress-to-local-paint latency, edit acknowledgement latency, client edit queue behavior, and IPC payload sizes.
      - Context7 `/criterion-rs/criterion.rs`: Use benchmark groups, throughput, and baselines for comparable measurements.
    - Options Considered:
      - Measure only through an end-to-end GUI run: valuable, but hard to make deterministic and inspect in CI.
      - Measure pure protocol/server paths only: deterministic, but misses client queue and optimistic local-edit invariants.
      - Combine deterministic protocol/server benches with client hot-path invariant tests: preferred for CI reliability and practical baseline coverage.
    - Chosen Approach:
      - Add benchmark cases for codec/server acknowledgement paths and focused tests proving local edit application happens before async acknowledgement work. Record payload byte snapshots for representative protocol messages.
    - API Notes and Examples:
      ```rust
      let encoded = encode_frame(&ProtocolMessage::ClientEdit(edit))?;
      perf.record_bytes("protocol.client_edit.payload_bytes", encoded.len() as u64);
      assert!(encoded.len() <= CLIENT_EDIT_PAYLOAD_BUDGET_BYTES);
      ```
    - Files to Create/Edit:
      - `benches/protocol_server_baselines.rs`: Codec and acknowledgement benchmarks.
      - `src/protocol/codec.rs`: Payload-size metric hooks and any test-only helpers needed for representative messages.
      - `src/client/behavior.rs`: Preserve or add local-first edit tests around manifest-declared edits.
      - `src/client/mod.rs`: Queue-depth instrumentation and tests.
      - `src/server/document.rs`: Edit acknowledgement timing benchmarks/tests.
      - `tests/performance_protocol.rs`: Deterministic protocol payload and queue invariant tests.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `docs/wiki/flows/client-edit-emission.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - `ordinary_edit_updates_shadow_before_ack`: Local shadow state changes before a server acknowledgement is awaited.
    - `client_edit_queue_reports_depth_without_blocking_input`: Queue metrics update while local edit application remains immediate.
    - `representative_protocol_payloads_fit_phase14_budgets`: Client edit/ack/manifest/SDUI payload sizes stay within documented initial budgets or fail with actionable messages.
    - `oversized_and_invalid_frames_still_rejected_with_metrics_enabled`: Instrumentation does not weaken codec validation.
    - Validation Run: `cargo fmt --all`, `cargo test --test performance_protocol`, and `cargo bench --bench protocol_server_baselines --no-run` passed.

- [x] Baseline and improve viewport, layout, scroll, and render-adjacent paths
  - Acceptance Criteria:
    - Functional: Benchmarks or focused tests cover large-buffer visible extraction, scroll offset changes, layout cache invalidation, line/window bounds, and render-preparation state updates for current plain-text content.
    - Performance: The baseline does not regress viewport-bounded rendering; if measurements reveal obvious full-buffer or repeated-layout work, scoped fixes improve incremental Parley layout/cache invalidation or scroll handling without changing the broader architecture.
    - Code Quality: Any optimizations preserve Unicode-safe cursor/selection behavior and keep layout/viewport/editor responsibilities separated.
    - Security: Optimizations do not introduce client filesystem access, network access, JavaScript execution in paint/input paths, or unsafe text slicing.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 14: Improve incremental Parley layout, viewport virtualization, pixel-accurate scrolling, and layout cache invalidation where baseline benchmarks already show regressions.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep rendering viewport-bounded and avoid blocking paint/text-event handlers.
      - `docs/wiki/modules/server-driven-ui.md`: SDUI composition exists but native client still owns rendering.
    - Options Considered:
      - Defer all optimization to later hardening: risks designing package primitives without knowing current bottlenecks.
      - Rewrite layout/rendering around a new architecture: too broad for a baseline foundation phase.
      - Apply only evidence-backed scoped fixes: preferred because Phase 14 is measurement-first and should not churn stable editor code unnecessarily.
    - Chosen Approach:
      - Run the new benches first, inspect metrics for full-buffer extraction/re-layout or cache churn, and make surgical fixes only when a measured regression or obvious invariant violation is found.
      - Extend editor baseline groups to cover viewport bounds and render-adjacent caret/selection updates, then remove duplicate visible-snapshot extraction from focused paint/caret paths by reusing the already computed visible snapshot offsets.
    - API Notes and Examples:
      ```rust
      // Desired invariant for scroll/layout benchmarks:
      // changing scroll offset should update visible bounds and reuse unchanged layout/cache state
      // unless text, width, font, or viewport dimensions invalidate it.
      ```
    - Files to Create/Edit:
      - `benches/editor_baselines.rs`: Viewport, layout, scroll, and render-preparation baseline groups.
      - `src/editor/viewport.rs`: Scroll and visible-window measurement/fixes if needed.
      - `src/editor/layout.rs`: Layout cache invalidation measurement/fixes if needed.
      - `src/editor/surface.rs`: Editor hot-path measurement/fixes if needed.
      - `src/masonry_editor.rs`: Render-preparation measurement/fixes if needed.
      - `src/perf/baselines.rs`: Add viewport/window, resize, and render-adjacent benchmark helpers.
      - `tests/editor_performance_invariants.rs`: Deterministic invariants for viewport-bounded behavior and cache invalidation.
    - References:
      - `roadmap.md` Phase 14
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `docs/wiki/modules/server-driven-ui.md`
  - Test Cases to Write:
    - `visible_extraction_scales_with_viewport_not_document_size`: Instrumented extraction counts stay bounded for large documents and fixed viewport windows.
    - `scroll_does_not_force_unrelated_full_layout_rebuilds`: Scroll-only changes do not invalidate text/layout state beyond the visible bounds required.
    - `layout_cache_invalidates_on_text_width_font_or_viewport_changes`: Required invalidation still happens for correctness.
    - `unicode_boundaries_remain_valid_after_layout_optimizations`: Existing Unicode movement/selection tests continue to pass.
    - Validation Run: `cargo fmt --all`, `cargo test --all-targets`, `cargo bench --bench editor_baselines --no-run`, and `cargo bench --bench editor_baselines editor_render_adjacent -- --sample-size 10 --warm-up-time 1 --measurement-time 2` passed.

- [x] Define performance budgets and developer documentation
  - Acceptance Criteria:
    - Functional: A performance document records baseline commands, fixture workflows, measured focus areas, initial budgets, guardrails for future package/mode primitives, and how to save/compare local baselines.
    - Performance: Budgets explicitly cover keypress-to-local-paint expectations, edit acknowledgement latency, scroll/layout/render-adjacent latency, memory, queue depth, IPC payload sizes, SDUI payload sizes, and runtime/configuration evaluation boundaries where measurable.
    - Code Quality: Budgets distinguish deterministic CI checks from advisory local benchmarks, and each guard references the command or test that enforces or observes it.
    - Security: Documentation reiterates that profiling and benchmark workflows must not expose document contents, secrets, network listeners, shell authority, or arbitrary JavaScript/client execution.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 14: Define performance budgets for package/mode primitives and add CI-friendly guards where deterministic enough.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: No synchronous JavaScript in keypress/paint paths, viewport-bounded rendering, bounded queues, and no full-document IPC for ordinary edits.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Deterministic checks should fail with actionable repair commands.
      - Context7 `/criterion-rs/criterion.rs`: `cargo bench -- --save-baseline`, `--baseline`, `--baseline-lenient`, sample size, warm-up, and measurement time; in this crate use `cargo bench --benches -- ...` when passing Criterion-only arguments so lib/bin bench harnesses do not receive them.
    - Options Considered:
      - Enforce all latency thresholds in CI: attractive, but likely flaky across machines.
      - Make all performance work advisory: easy, but regressions remain invisible.
      - Split hard invariants from local advisory baselines: preferred because payload sizes, no-full-document IPC, and hot-path routing are deterministic while absolute timings vary.
    - Chosen Approach:
      - Document initial budgets as a mix of hard invariants, CI-friendly payload/queue/cache checks, and local Criterion baseline comparison commands.
    - API Notes and Examples:
      ```text
      cargo bench --benches -- --save-baseline phase14-baseline
      cargo bench --benches -- --baseline-lenient phase14-baseline
      cargo test performance_protocol -- --nocapture
      ```
    - Files to Create/Edit:
      - `docs/development/performance.md`: Baseline commands, fixture workflows, budgets, and interpretation guidance.
      - `docs/index.md`: Link the performance development documentation if project docs index covers development docs.
      - `README.md`: Optional short pointer to the performance guide if developer workflows are listed there.
      - `tests/performance_budgets.rs`: Deterministic budget checks for payloads, queue invariants, and no-full-document edit messages.
    - References:
      - `roadmap.md` Phase 14
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - Context7 `/criterion-rs/criterion.rs`
  - Test Cases to Write:
    - `performance_docs_list_all_supported_benchmark_commands`: A docs test or manual check confirms commands are discoverable.
    - `performance_budget_payload_constants_match_docs`: Payload budget constants and documented values stay aligned if constants are added.
    - `cargo bench --no-run`: Bench documentation remains compile-valid by building all benchmark targets.

- [x] Add CI-friendly performance guards and verification workflow
  - Acceptance Criteria:
    - Functional: Deterministic performance guard tests run under normal development validation and check structural invariants such as payload size ceilings, bounded queues, viewport-bounded extraction counts, and benchmark target compilation.
    - Performance: CI guards avoid absolute latency thresholds unless proven stable, and long-running Criterion measurements remain opt-in local commands.
    - Code Quality: Guard failures explain the violated budget and the command/documentation to use for deeper profiling.
    - Security: CI tests use generated fixtures and local in-process helpers only; no user config, network, shell, or external service is required.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Checks should fail with actionable repair commands and avoid silent mutation.
      - `roadmap.md` Phase 14: Add CI-friendly performance guards where deterministic enough.
      - Context7 `/criterion-rs/criterion.rs`: Use `cargo bench --no-run` for benchmark compilation without running machine-variant timing loops.
    - Options Considered:
      - Add benchmark runs to every `cargo test`: too slow and machine-variant.
      - Add no automated guard until CI exists: misses deterministic regressions already known to be important.
      - Add focused invariant tests plus bench compilation: preferred as a stable foundation until broader CI is introduced.
    - Chosen Approach:
      - Add tests for performance invariants and document a validation sequence that combines `cargo fmt --check`, `cargo test --all-targets`, `cargo bench --no-run`, and optional local baseline comparison.
    - API Notes and Examples:
      ```text
      cargo fmt --check
      cargo test --all-targets
      cargo bench --no-run
      cargo bench --benches -- --baseline-lenient phase14-baseline
      ```
    - Files to Create/Edit:
      - `tests/performance_budgets.rs`: CI-friendly structural budget tests.
      - `tests/performance_protocol.rs`: Protocol and queue invariants if not covered elsewhere.
      - `benches/*.rs`: Benchmark targets that compile under `cargo bench --no-run`.
      - `docs/development/performance.md`: Verification workflow and expected commands.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `cargo fmt --check`: Formatting remains stable.
    - `cargo test --all-targets`: Deterministic budget and invariant tests pass.
    - `cargo bench --no-run`: Benchmark targets compile without executing long measurements.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The completed phase is reviewed for any user-visible profiling, diagnostics, budget, benchmark, performance-display settings, or developer activation flags/environment variables that should be configuration APIs; required APIs are documented and registry-backed, or the absence of public configuration is explicitly justified.
    - Performance: Any configurable profiling/diagnostic collection remains opt-in or low overhead and never enables synchronous JavaScript, IPC, file IO, or blocking metric export in typing/rendering paths.
    - Code Quality: Configuration APIs, if added, follow the Clay JS API schema with stable IDs, user-facing names, key binding metadata, custom properties, examples, errors, and lookup tags.
    - Security: Configuration does not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace authority, or permission to expose document contents through metrics.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Every Clay plan that changes user-visible behavior, commands, server APIs, protocol capabilities, or public programmatic surfaces must include a configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Every configuration option is a Clay JS API rooted at `~/.config/clay/init.js`.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Public APIs require Markdown docs, generated registry entries, and lookup coverage.
    - Options Considered:
      - Add user-facing performance toggles immediately: useful for diagnostics, but may expand public API before metric semantics stabilize.
      - Keep Phase 14 profiling internal/developer-only: simpler and likely sufficient for benchmark foundation.
      - Review at implementation end and expose only stable surfaces: preferred because it preserves documentation-as-code without prematurely freezing internal metric names. The Phase 14 developer-only `CLAY_PERF_PROFILE`/`--profile-perf` activation path should remain outside normal user configuration unless the implementation intentionally promotes it to a stable user-facing diagnostic feature.
    - Chosen Approach:
      - After implementation, inventory any performance settings or diagnostics exposed outside tests/benches/developer commands. Add documented Clay configuration APIs only for stable user-facing behavior; keep unstable benchmark internals private or developer-doc-only.
    - API Notes and Examples:
      ```js
      // Only if a stable user-facing setting is introduced:
      import { configurePerformanceDiagnostics } from "clay:configuration";
      configurePerformanceDiagnostics({ enabled: true, includeDocumentContent: false });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration/*.md`: Add configuration API docs if stable performance settings are introduced.
      - `docs/reference/clay-js-api/api-inventory.toml`: Add or verify inventory entries for any new configuration APIs.
      - `docs/index.md`: Link new API docs if created.
      - `src/docs/registry.rs` or generated registry artifacts: Update through the project registry command if docs change.
      - `tests/clay_js_doc_registry.rs`: Existing or new coverage for docs/registry freshness.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - `configuration_api_inventory_covers_performance_settings`: Any public performance setting is listed in the API inventory.
    - `configuration_docs_registry_is_fresh`: Registry tests fail if new configuration docs are missing or stale.
    - `no_public_configuration_needed_for_internal_perf_hooks`: If no config API is added, a review note or test confirms hooks remain internal/developer-only.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: All server-side Rust public functions and public programmatic behavior introduced or changed by Phase 14 are inventoried; stable public capabilities are exposed through explicit `deno_core` ops and Clay JS/TS facades, while internal helpers are made private or `pub(crate)`.
    - Performance: Any public performance/diagnostic API is asynchronous or snapshot-based and cannot block client typing, paint, layout, IPC frame handling, or server edit acknowledgement paths.
    - Code Quality: New public APIs have Markdown docs, stable IDs, user-facing names, key binding metadata or empty lists, custom properties, examples, errors, permissions, backing Rust/op/facade paths, generated registry entries, and lookup coverage.
    - Security: APIs do not expose document contents, secrets, unsanitized paths, raw metric internals, raw Rust functions, raw `Deno.core.ops.op_*` calls, filesystem/network/shell authority, or arbitrary client JavaScript execution.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay JS API verification task requirements.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic surface is the Clay JS/TS API, not raw Rust or raw ops.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown-authoritative API docs and generated registry coverage.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Tests should detect stale generated artifacts.
    - Options Considered:
      - Expose all performance metrics as public APIs now: maximally inspectable, but risks freezing unstable implementation details.
      - Keep all performance work internal forever: simple, but may limit user/agent observability later.
      - Expose only stable, sanitized, user/agent-useful surfaces and keep raw benchmark internals private: preferred for Phase 14.
    - Chosen Approach:
      - Review implementation at the end of the phase. If stable performance diagnostics are public, add facade APIs and docs; otherwise make new Rust helpers internal and document developer workflows in `docs/development/performance.md` plus the code wiki.
    - API Notes and Examples:
      ```js
      // Only if a stable public diagnostic API is introduced:
      import { getPerformanceSnapshot } from "clay:diagnostics";
      const snapshot = await getPerformanceSnapshot({ sanitize: true });
      ```
    - Files to Create/Edit:
      - `src/server/ops/*.rs`: Add explicit op wrappers only for stable public server-side diagnostics, if any.
      - `runtime/js/**/*.ts`: Add Clay JS facade modules/exports if public APIs are introduced.
      - `docs/reference/clay-js-api/**/*.md`: Document public performance/diagnostic APIs if introduced.
      - `docs/reference/clay-js-api/api-inventory.toml`: Inventory any new public APIs.
      - `docs/index.md`: Link new API docs.
      - `tests/clay_js_api_inventory.rs`: Verify inventory entries.
      - `tests/rust_visibility_api_mapping.rs`: Verify new server-side Rust public functions are exposed or intentionally internal.
      - `tests/clay_js_doc_registry.rs`: Verify docs and generated registry freshness.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - `rust_visibility_mapping_covers_phase14_public_functions`: New server-side public functions are either exposed through documented Clay JS APIs or made non-public.
    - `clay_js_api_inventory_includes_public_performance_surfaces`: Any public diagnostic/performance API appears in the inventory.
    - `clay_js_doc_registry_is_fresh_after_phase14`: Generated registry tests fail on missing/stale docs or index links.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- Advisory latency budgets (`KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `EDIT_ACK_P95_BUDGET_MS`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`, `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS`) and the memory budget (`LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB`) are documented and tested for constant/doc alignment but are not enforced as hard CI thresholds because machine-variant Criterion timing is unreliable across environments. Enforcing them requires a stable, consistent CI runner that does not yet exist for this project.
- The `cargo bench --no-run` CI guard is documented as a command and validated manually, but is not yet wired into an automated CI pipeline. A CI pipeline is deferred to Phase 21.
- Phase 14 profiling hooks remain developer-only. If future observability or package-mode profiling needs stable user-facing diagnostics, a separate task should introduce a `clay:diagnostics` Clay JS API with Markdown docs, inventory entry, and registry coverage.

## Further Actions
- **Phase 21 (Priority: High)**: Wire `cargo fmt --check`, `cargo test --all-targets`, and `cargo bench --no-run` into a CI pipeline. Promote advisory latency budgets to hard thresholds only after verifying stability on the CI runner.
- **Phase 15–16 (Priority: Medium)**: As SDUI complexity grows and package-mode primitives are added, recheck SDUI payload budgets (`SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`). If representative trees exceed 4 KiB snapshots or 1 KiB updates, revisit SDUI update compression as noted in the Phase 12 roadmap.
- **Phase 18 (Priority: Medium)**: Use Phase 14 Criterion baselines as the comparison baseline for the Markdown mode package proof of concept. Instrument large Markdown document open/edit/scroll paths against the documented budget constants.
- **Phase 14 follow-up (Priority: Low)**: If developer profiling diagnostics are promoted to a stable user-facing surface, add a `clay:diagnostics` Clay JS API (module, Markdown doc, inventory entry, registry entry, lookup tags) and remove the `no_public_configuration_needed_for_internal_perf_hooks` guard test.
