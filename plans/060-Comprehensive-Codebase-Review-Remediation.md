# Comprehensive Codebase Review Remediation

**Plan date:** 2026-07-20

**Source review:** `code-reviews/2026-07-19-comprehensive-codebase-review.md`

## Objectives

- Resolve every P0–P3 finding from the 2026-07-19 comprehensive review, closing root causes at shared authority, routing, lifecycle, filesystem, concurrency, and maintenance boundaries.
- Defer package-runtime/provenance implementation to prerequisite Plan 061, then resume this plan against its final two-domain architecture without duplicating authority work.
- Preserve server-authoritative documents, package provenance, bounded protocol payloads, local-first editing, and Linux quality gates.
- Prefer deletion, existing primitives, standard-library/platform facilities, and measured build changes over speculative abstractions or dependencies.
- Keep a finding-to-task ledger so already-resolved or superseded findings close through current evidence rather than duplicate implementation.

## Expected Outcome

- Plan 061 establishes two fixed JavaScript runtime trust domains, host-stamped package provenance, explicit approved composition, and first-party replacement; this plan verifies that boundary before continuing dependent remediation.
- Post-handshake IPC identity is connection-owned, request/results are routed only to authorized clients, and save/reload/status/list enforce document access and versions.
- File reads, saves, directory traversal, process I/O, channels, diagnostics, metrics, document state, and accepted connections have explicit lifecycle/resource ceilings.
- Dependency audit, build shape, facade ownership, documentation checks, and module orchestration are simpler and measurably cheaper while all Linux blocking checks pass.
- Every review ID is marked resolved, already resolved with evidence, or explicitly deferred with owner, reason, expiry, and follow-up priority.

## Tasks

- [x] Rebaseline every review finding and establish the remediation ledger
  - Acceptance Criteria:
    - Functional: Create a ledger mapping P0-1 through P3-3 and all seven deletion-pass items to one task, current reproducer, owner, dependencies, and closure evidence; recheck changes landed after review, especially Plans 056–059 and the 4096-entry performance recorder cap.
    - Performance: Record current test-binary count, clean/incremental target sizes, representative link times, queue/map lengths, and syntax parse-work counts without treating machine-variable timings as hard gates yet.
    - Code Quality: Do not reimplement findings already fixed; distinguish fully resolved, partially resolved, reproducible, and blocked states with exact source/test evidence.
    - Security: Re-run `cargo audit`; preserve P0 blockers until adversarial tests prove package and multi-client isolation.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-07-19-comprehensive-codebase-review.md`: authoritative finding list and suggested order.
      - `plans/056-Low-Latency-Incremental-Syntax-Decoration.md`, `plans/057-Syntax-Decoration-Continuity-and-Replacement-Correctness.md`, `plans/058-Exact-Range-Provisional-Decoration-Replacement.md`, `plans/059-IPC-Framing-Cancellation-Safety-and-Markdown-Inline-Syntax.md`: work landed near/after review.
      - `docs/wiki/modules/{maintenance-validation,server-ipc-skeleton,server-file-workspace,embedded-js-runtime,multi-document-sessions}.md`.
      - `.agents/skills/project-patterns/references/{planning-checklist,maintenance-validation,protocol-and-performance,authority-boundaries}.md`.
    - Options Considered:
      - Assume report remains exact: fastest, but duplicates already-landed fixes.
      - Re-review entire repository: expensive and unnecessary.
      - Focused reproducer/evidence pass for every listed finding. Chosen.
    - Chosen Approach:
      - Add a compact ledger to this plan during execution; each later task updates only its mapped rows and cannot close a row without a test, command, or exact code reference.
    - API Notes and Examples:
      ```text
      P1-6 | already resolved by Plan 056 | one parse/version/window tests | verify, do not rebuild
      P1-8 | partial | metrics capped; channels/diagnostics remain unbounded | tasks 6 and 14
      ```
    - Files to Create/Edit:
      - `plans/060-Comprehensive-Codebase-Review-Remediation.md`: completion ledger and measured baseline.
      - No production files in this task.
    - References:
      - `code-reviews/2026-07-19-comprehensive-codebase-review.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Baseline commands: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, focused existing tests, `cargo audit`, and `git diff --check`.
    - Inventory commands: integration harness count, `du -sh target`, and exact dependency paths for each advisory.
  - Completion Evidence (2026-07-20):
    - Status vocabulary: **reproducible** means current source still contains the reviewed root cause; **partial** means later work closed only part; **already resolved** requires current executable evidence; **blocked** identifies a proven dependency, not an excuse to close the row.
    - Task owners below use this plan's task order: T2 primitive/authority review, T3 Plan 061 prerequisite gate, T4 connection identity/routing, T5 filesystem integrity, T6 lifecycle/bounds, T7 dependencies, T8 async/process concurrency, T9 build shape, T10 facade/orchestration, T11 documentation validators, T12 ignore/sandbox, T13 dialogs/clipboard, and T14 final closure.

    | Finding | Current status and reproducer | Owner / dependencies | Required closure evidence |
    |---|---|---|---|
    | P0-1 | **Reproducible.** `package_from_options`/`package_value_from_options` still accept `packageManifest` or caller fields in `src/server/ops/{decorations,syntax,completion}.rs`; diagnostics/parse/document-analysis/language-intelligence reuse that path. No host-stamped package context exists. | T3 / prerequisite Plan 061 implements approved two-domain provenance and composition architecture. | Cross-domain internal-op/module denial, A-as-B host-op rejection, permission inflation, unapproved first-party mutation, stale-generation, replacement, LSP, and one-line package-load tests pass. |
    | P0-2 | **Reproducible.** `ClientMessage` still carries post-Hello IDs; `SaveDocument { client_id: _ }` discards identity at `src/server/connection.rs:738-744`; edit/open/reload/status/list lack one pre-dispatch validator. | T4 / T3 first in execution order; protocol migration can proceed independently. | Table-driven forged-ID and document-access/version tests for every message family pass. |
    | P0-3 | **Reproducible.** Every connection still drains global `next_update`, `next_diagnostic`, and `next_output`; completion tasks race on global `next_result` (`src/server/connection.rs:244-285,943-949`). | T4 / canonical T4 connection ownership. | Deterministic two-client parse/diagnostic/analysis/completion isolation tests pass with bounded routes. |
    | P1-1 | **Reproducible; quick-xml upgrade-blocked.** `cargo audit` reports 3 vulnerabilities and 5 warnings. `crossbeam-epoch 0.9.18` is dev-only via Criterion/Rayon; `quick-xml 0.39.3` is runtime/build reachable via Wayland scanner/UI and `zbus_xml`. | T7 / compatible Wayland/Winit/Masonry/zbus releases for quick-xml. | Audit passes or only explicit unexpired, reachability-documented exceptions remain; exact `cargo tree -i` paths recorded. |
    | P1-2 | **Reproducible.** `check_openable_size` precedes path-based `tokio_fs::read` in open/reload (`src/server/workspace.rs:860,1078,1439,1564`). | T5 / none. | Handle-based `max + 1` boundary and replace/grow race tests pass. |
    | P1-3 | **Reproducible.** Predictable PID/counter temp path, `File::create`, best-effort permission restoration, and path rename remain (`src/server/workspace.rs:1460-1524`). | T5 / P1-2 shared handle/file-identity helpers. | Exclusive unpredictable temp, permission failure, symlink/precreate, target replacement, and unchanged-target tests pass. |
    | P1-4 | **Reproducible.** No `CloseDocument`; client retention is 64 but workspace documents, syntax trees, parse/completion/intelligence version maps, and analysis routes remain uncapped `HashMap`s. | T6 / T4 subscriptions/access model. | Thousands-of-documents churn remains within per-client/server ceilings and final close removes every mapped state. |
    | P1-5 | **Reproducible.** `op_clay_workspace_list_directory` holds `workspace.lock()` while synchronous recursive traversal and uncapped `.gitignore` read run (`src/server/ops/workspace.rs:106-107`; `src/server/workspace.rs:549-810,1999`). | T8 / T6 budgets; T12 matcher/input semantics. | Slow listing does not delay open/save/cancel; RAII cancellation and bounded ignore-input tests pass. |
    | P1-6 | **Already resolved by Plans 056-058; retain T14 verification only.** Bounded Rope slicing, one same-window/version schedule, exact `InputEdit`, changed ranges, bounded fan-out, and provisional exact-range interpolation exist. Current focused suites passed: editor performance 23, parse coordinator 29, performance protocol 19; representative syntax fixtures assert one parser call. | T14 / no new implementation. | Final Linux gate repeats one-parse/version/window, large-document no-full-copy, cancellation, payload, and continuity checks. |
    | P1-7 | **Reproducible.** One 64-command router calls awaited `handle_read`/`handle_write` against a shared session table (`src/server/language_server.rs:347-399,487-550`). | T8 / T2 confirms subprocess authority unchanged. | Hung session A cannot delay responsive session B; bounded actor ingress and revocation tests pass. |
    | P1-8 | **Partial.** Plan 056 capped metric snapshots at 4,096 and runtime snapshots cap diagnostics at 32; metric overflow is silently dropped. Parse has two unbounded channels, completion one, each connection two, and `runtime_diagnostics: Vec` remains unbounded. Document-analysis input/output are already bounded at 64 events. | T6 / T4 routing before channel replacement. | Saturation/coalescing/dedup/drop-count tests prove all process-lifetime lanes and stores bounded. |
    | P1-9 | **Reproducible and now 33 integration harnesses.** Current checkout is 118 GiB; fresh all-target no-run build is 21 GiB. No dev/test debug profile exists; many docs still prescribe `target/pi-verify`. | T9 / final test inventory from preceding security work. | Fewer harness binaries and measured clean/warm link and artifact deltas with unchanged test-name/security coverage. |
    | P1-10 | **Reproducible.** Endpoint parent validates owner only, not mode (`src/server/mod.rs:1519-1540`); Unix/Windows accept loops spawn without active-connection permits. | T6 / none. | Unsafe-parent and connection-flood tests pass; default endpoint remains valid. |
    | P2-1 | **Reproducible.** Twenty-one `CLAY_FACADE_*` raw JS bodies remain beside 21 `runtime/js/*.ts` facade files. | Plan 061 owns domain-critical trusted/public facade split and executable single-source work; T10 verifies/completes remaining cleanup. | Rust includes the single executable source; trusted/third-party inventory/export tests catch drift. |
    | P2-2 | **Partial.** `open_document_followup_messages` is now shared by three open origins, but `Edit`/`EditorIntent` still duplicate apply/ack/analysis/completion/syntax flow. Large production sections remain, e.g. `record.rs` 4,548 and `surface.rs` 3,118 lines before tests. | T10 / T4 identity/routing changes first. | Shared edit flow passes behavior parity; any task-local module split reduces mixed responsibilities without new interfaces. |
    | P2-3 | **Reproducible.** Current files are 7,408/4,736/3,055 lines with 876/612/551 assertion-or-needle-like occurrences in primitives/API/package docs tests. | T11 / T10 authoritative inventory changes first. | Generic schema tests reject missing facts but accept harmless prose rewrites; obsolete needles deleted. |
    | P2-4 | **Reproducible.** `glob_matches` still commits at first post-`*` character and treats `[` as a glob without implementing classes; unsupported semantics remain silent. | T12 / T8 listing extraction can land first. | Minimal documented grammar tests include `*ab` vs `aab`, unsupported syntax, UTF-8, and path behavior. |
    | P2-5 | **Reproducible.** `Driver::spawn_native_dialog_command(&self, ...)` starts a detached `clay-native-dialog` thread per command with no in-flight state. | T13 / none. | Repeated command, cancellation/error reset, and stale-generation tests prove one request per kind. |
    | P2-6 | **Reproducible.** Sandbox `read_frame` uses unbounded `read_line` before checking `max_payload_bytes` (`src/server/runtime_sandbox.rs:125-142`). | T12 / none. | Unterminated/terminated overflow tests prove parent allocation bounded and child reaped. |
    | P2-7 | **Reproducible.** Production `src/main.rs` and `src/bin/clay-server.rs` still call panic-wrapper `IpcServer::new`; `try_new` already exists. | T10 / none. | Invalid startup roots return typed errors in both binaries; wrapper removed or test-only. |
    | P2-8 | **Reproducible.** `read_capped_stderr` breaks when retained bytes reach the 64 KiB budget instead of draining/discarding (`src/server/language_server.rs:613-631`). | T8 / P1-7 actor ownership. | Verbose child reaches normal EOF without Clay closing pipe; retained prefix/truncation remains bounded. |
    | P3-1 | **Reproducible.** Direct `arboard 3.6.1` remains while Masonry brings `copypasta`; Clay fallback calls arboard directly. | T13 / platform smoke availability. | Keep both or remove one based on Wayland/X11 plus documented macOS/Windows parity evidence. |
    | P3-2 | **Reproducible.** `discover_workspace_statuses` awaits each root sequentially (`src/server/git.rs:125-136`); a separate cache-refresh path already demonstrates `JoinSet` fan-out. | T8 / shared process concurrency budget. | Multi-root test proves bounded concurrency and result/root association. |
    | P3-3 | **Reproducible.** IDs remain duplicated across Rust routes/raw facades/TS/docs/inventory/exact-list tests; no shared generated identifier table closes the spread. | T10 / P2-1 single executable source. | Checked-in inventory-derived identifiers stay fresh while security fields retain independent validation. |

    | Deletion pass | Current status and reproducer | Owner / dependencies | Required closure evidence |
    |---|---|---|---|
    | D1 facade bodies | **Reproducible.** 21 embedded bodies remain. | Plan 061 runtime-domain split, then T10 verification. | Raw bodies deleted; one executable source feeds explicit trusted/public facade allowlists. |
    | D2 edit orchestration | **Reproducible.** `Edit` and `EditorIntent` remain parallel branches. | T10 / T4. | One shared apply/ack/follow-up function and parity tests. |
    | D3 open follow-ups | **Already resolved.** `open_document_followup_messages` is called for workspace, selected-file, and file-browser opens. | T14 verification only. | Existing three-origin tests remain green. |
    | D4 prose needles | **Reproducible.** Counts recorded above remain large. | T11 / T10. | Generic validators replace equivalent phase needles. |
    | D5 test harnesses | **Reproducible.** 33 integration plus 5 benchmark harnesses produce 43 all-target executables including five unit/bin targets. | T9 / none. | Reduced executable count with identical test inventory. |
    | D6 panic constructor | **Reproducible.** Production and tests use `IpcServer::new`. | T10 / none. | Production uses `try_new`; wrapper removed/test-scoped. |
    | D7 clipboard backend | **Reproducible, decision remains evidence-dependent.** Both arboard and transitive copypasta remain. | T13 / platform smoke. | Backend matrix justifies deletion or explicitly closes as retained. |

    - Measured build baseline on this Linux host:
      - Source shape: 33 top-level integration-test files, 5 benchmark files, and 43 expected all-target executable harnesses including library/main/three bin targets.
      - Existing checkout: `target/` 118 GiB; `target/debug` 78 GiB; `target/debug/deps` 60 GiB; `target/debug/incremental` 17 GiB; duplicate `target/pi-verify` 37 GiB. Existing directory includes historical hashes and is not a clean-build size.
      - Fresh non-destructive all-target no-run build in a temporary target: 80.479 s, 21 GiB total, 18 GiB `debug/deps`, 1.8 GiB incremental; temporary target removed after measurement.
      - Warm all-target no-run/link snapshot: 15.903 s. Largest current-hash outputs include `clay` 595,458,728 bytes, library tests 513,105,352 bytes, main tests 470,493,648 bytes, and selected-file smoke 460,771,736 bytes. Timings are comparison snapshots, not gates.
    - Queue/store baseline:
      - Bounded: client edit/event/read-pump lanes 256; server read pump 64; document-analysis input/output 64 events; LSP router 64 commands and 16 sessions; runtime-state broadcast 16; runtime snapshot diagnostics 32; performance snapshots 4,096.
      - Unbounded: parse update + diagnostic channels; completion result channel; per-connection completion + language-intelligence channels; runtime diagnostic `Vec`; workspace documents, syntax trees, coordinator versions/generations, and analysis route/active-document maps. No protocol close message or accepted-connection ceiling exists.
    - Plan 056-059 recheck: all task checkboxes are complete. Plans 056-058 provide current P1-6 closure; Plan 059 adds bounded cancellation-safe IPC read pumps but does not alter P0 identity/result ownership. No other reviewed row became fully resolved.
    - Validation results:
      - Passed: `cargo fmt --check` (0.684 s), `cargo check --all-targets` (2.928 s), `cargo clippy --all-targets -- -D warnings` (6.317 s), focused syntax suites (71 tests total), warm and fresh `cargo test --all-targets --no-run`, tracked `git diff --check`, and standalone untracked-plan whitespace check.
      - Expected security failure preserved: `cargo audit` scanned 582 dependencies and found `RUSTSEC-2026-0204` (`crossbeam-epoch 0.9.18`) plus high-severity `RUSTSEC-2026-0194`/`0195` (`quick-xml 0.39.3`), and warnings for `bincode 1.3.3`, `paste 1.0.15`, `ttf-parser 0.25.1`, `anyhow 1.0.102`, and `memmap2 0.9.10`. Exact inverse trees were captured during execution; P1-1 stays open.
      - Baseline used the pre-existing dirty working tree; this task changed no production, API, configuration, generated, or wiki files. Existing unrelated modifications were left untouched.

- [x] Review package-principal, result-routing, and external-process authority primitives before authority changes
  - Acceptance Criteria:
    - Functional: Inventory `PackageService`, authorization/grant records, enabled package generations, runtime module loading, `ClayOpState`, publication/registration ops, package disable/replacement, parse/completion/analysis coordinators, connection subscriptions, request IDs, and language-server session/router ownership; state what existing primitives can enforce before proposing new code.
    - Performance: Principal lookup and output dispatch remain constant-time/bounded and outside typing, paint, layout, scroll, and local edit application; per-session actors must not add process work to those paths.
    - Code Quality: Define only generic principal/routing gaps; reject per-op manifest parsers, string bearer tokens visible to sibling modules, duplicate coordinator implementations, and a generic process/actor framework.
    - Security: Compare principal-specific isolates/processes, host-created unforgeable closures/capabilities, and exact enabled-record lookup; verify the approved fixed-contribution language-server authority remains unchanged by actor refactoring. Obtain explicit user approval and create/update a decision log before selecting any new package isolation/capability architecture or changing subprocess authority.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`.
      - `decision-logs/2026-07-14-2023-language-server-package-authority.md`.
      - `docs/reference/primitives/{index,registry,package-security,package-loading}.md`.
      - `docs/wiki/modules/{primitive-architecture,third-party-runtime-authority,embedded-js-runtime,parse-coordinator,language-intelligence,language-server-process-service}.md`.
      - `.agents/skills/create-plan/references/clay.md`: primitive-first, one-line package loading, and external-process authority requirements.
      - `.agents/skills/project-patterns/references/{authority-boundaries,mode-primitive-first}.md`.
    - Options Considered:
      - Exact caller-supplied manifest lookup only: blocks permission inflation but does not authenticate sibling-package identity.
      - Secret string token in shared globals: forgeable/readable by sibling code.
      - Host-bound provenance in one shared runtime: supports composition but cannot protect trusted globals/internal ops.
      - Principal-specific isolate per package: strongest sibling isolation but rejected for repeated V8 resources and composition cost.
      - Two fixed runtime trust domains plus package-scoped provenance/approved graph inside third-party cohort: fixed overhead and explicit first/third-party boundary. Chosen and approved.
    - Chosen Approach:
      - Produce an indexed primitive/gap matrix and threat model, obtain approval, record the two-domain decision, inventory first-party extension APIs, and move implementation into prerequisite Plan 061.
    - API Notes and Examples:
      ```text
      trusted runtime <-> typed bounded Rust APIs <-> shared third-party runtime
      third-party package -> host-stamped provenance -> exact grants + approved graph edge
      first-party mutation -> owner extension point + user approval
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/package-principal-and-result-routing-primitive-review.md`: inventory and threat model.
      - `docs/wiki/index.md`: link primitive review.
      - `tests/primitives_docs.rs`: deterministic inventory coverage.
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`: approved architecture, superseding same-runtime portions of the prior package-authority decision.
      - `.agents/skills/project-patterns/references/{authority-boundaries,package-runtime-trust-domains,package-distribution}.md`: reusable approved guidance.
      - `docs/wiki/modules/first-party-package-extension-api-review.md`: bundled package/API/replacement inventory.
      - `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`: prerequisite implementation plan.
    - References:
      - P0-1 and P0-3 in source review.
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - Documentation test requires host-stamped package provenance, two-domain op/module separation, generation binding, user-approved composition/replacement, routing ownership, and rejected JavaScript bearer-token design.
    - `cargo test --test primitives_docs`
  - Execution Evidence (2026-07-20; approval gate open):
    - Added `docs/wiki/modules/package-principal-and-result-routing-primitive-review.md` and indexed it from `docs/wiki/index.md`. The review inventories every requested package, runtime, coordinator, connection, request, and process primitive; records adversarial package/client/process boundaries; and maps the migration order.
    - Existing primitives retained: exact `PackageService` records/grants, package/runtime/provider generations, package-control checks, package-root module allowlists, coordinator cancellation/stale checks, request IDs, language-intelligence `oneshot` replies, document access state, and exact language-server grants.
    - Confirmed generic gaps: package-facing ops authenticate no executing sibling package because one `JsRuntime`/`ClayOpState` serves all package modules and ops accept caller manifests/names; parse/diagnostic/analysis/completion shared receivers have no authorized connection owner.
    - Compared exact lookup, JavaScript bearer/resource tokens, shared-runtime host capabilities, per-package V8 isolates, two fixed trust-domain runtimes, and OS processes. User rejected per-package isolates and approved exactly two persistent domains: trusted Clay/integrity-verified bundled packages and one shared adopted-third-party cohort. Distinct op/module allowlists form the hard boundary; host-stamped package provenance and explicit approved graph edges govern supported third-party composition.
    - User confirmed third-party packages cannot be promoted into trusted runtime through normal approval and are intentionally not isolated from each other. Cross-domain first-party mutation requires a target-declared API plus user approval; user-approved full replacement withdraws a first-party package while replacement remains third-party and preserves provenance.
    - Routing selection needs no new authority: completion joins language intelligence on request-scoped `oneshot`; one bounded server-owned dispatcher fans parse/diagnostic/analysis streams to access-validated document subscriptions. Connections stop draining shared receivers.
    - External-process authority is frozen to the approved fixed-contribution contract. Per-session actors may change I/O ownership only; grants, fingerprints, executable/argv/environment/root identity, direct no-shell spawn, limits, revocation, and trusted-subprocess disclosure remain unchanged.
    - `deno_core 0.400.0` was verified through Context7 `/denoland/deno_core` plus locally resolved crate source: `OpState` is runtime-associated, `ModuleLoader` receives resolve/load provenance, and ordinary ops do not receive authenticated calling-module identity. This rules out exact lookup or a plain shared-runtime token as caller authentication.
    - Added deterministic documentation coverage in `tests/primitives_docs.rs`; `cargo test --test primitives_docs` passed 130 tests after the approved review/API inventory updates.
    - Created approved decision log `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`, updated reusable package runtime/distribution/authority/planning patterns, and created prerequisite Plan 061. Authority implementation now belongs to Plan 061 rather than this plan.

- [ ] Complete prerequisite Plan 061 before remaining review remediation
  - Acceptance Criteria:
    - Functional: Every task in `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md` is complete; two runtime domains, host-stamped third-party provenance, approved composition, first-party APIs/adoption/replacement, revocation, docs, and Linux closure are verified before this plan resumes.
    - Performance: Plan 061 records acceptable fixed two-runtime resource cost and proves package/cross-domain work remains off editor hot paths; this plan reuses rather than remeasures those results except at final closure.
    - Code Quality: Plan 060 does not duplicate Plan 061 implementation. After Plan 061 closes, rebaseline affected ledger rows/files/tests and update later task approaches before editing them.
    - Security: P0-1 closes only through executable cross-domain denial, caller-provenance, approval, mutation, replacement, stale-generation, and process-grant tests—not documentation alone.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
      - `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`.
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`.
    - Options Considered:
      - Keep per-package principal implementation in Plan 060: conflicts with approved architecture and duplicates major work.
      - Execute independent Plan 060 tasks while Plan 061 moves: technically possible for filesystem/dependency/build/sandbox/dialog work, but increases branch/baseline churn during the P0 runtime rewrite.
      - Pause remaining Plan 060 execution until Plan 061 closes, then rebaseline and resume. Chosen default; independent tasks require a separate explicit user request.
    - Chosen Approach:
      - Treat Plan 061 as a hard sequential prerequisite. Its final evidence replaces this plan's former T3 package-principal implementation and domain-critical facade work; then update T4/T6/T8/T10/T11/T14-T17 against final architecture.
    - API Notes and Examples:
      ```text
      Plan 061 complete -> rebaseline Plan 060 affected rows -> resume T4
      ```
    - Files to Create/Edit:
      - `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`: execute all tasks and evidence.
      - `plans/060-Comprehensive-Codebase-Review-Remediation.md`: post-061 ledger/dependency/test updates and gate completion.
    - References:
      - P0-1, P0-3, P1-8, P1-7, P2-1, P2-3, P3-3, D1.
  - Test Cases to Write:
    - Plan 061 full closure commands/results are present and all checkboxes complete.
    - Post-061 rebaseline confirms no caller-manifest authority, no third-party trusted op/module access, exact facade inventories, and identifies any remaining Plan 060 routing/bounds/concurrency work.

- [ ] Make connection identity canonical and route outputs to authorized clients
  - Acceptance Criteria:
    - Functional: One pre-dispatch boundary rejects every mismatched legacy `client_id`; next protocol shape removes post-`Hello` IDs where migration allows; save/reload/status/list require connection-owned document access and save validates `known_version`; parse, diagnostics, analysis, completion, and intelligence results reach only matching authorized subscriptions/requests.
    - Performance: Use request-scoped `oneshot` replies or one server dispatcher with bounded per-connection channels; no global lock/serialization across documents or connections.
    - Code Quality: Normalize `Edit` and `EditorIntent` into one shared apply/ack/follow-up path and keep protocol identity separate from codec framing.
    - Security: Forged IDs, guessed document IDs, stale versions, read-only saves, disconnect/reconnect, and unsubscribed document results fail closed without leaking metadata or payloads.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/{server-ipc-skeleton,protocol-codec,multi-document-sessions,parse-coordinator,language-intelligence}.md`.
      - `docs/wiki/flows/{client-server-edit-ack,document-leases-and-region-locks,versioned-text-synchronization}.md`.
      - Tokio 1.52.2 local rustdoc/source for `mpsc`, `oneshot`, and `watch`; Context7 `/websites/rs_tokio_1_49_0` channel examples.
      - `.agents/skills/project-patterns/references/{protocol-and-performance,authority-boundaries}.md`.
    - Options Considered:
      - Per-arm ID checks: current drift.
      - Broadcast every result then client-filter: leaks and wastes work.
      - Central connection-context validation plus request-scoped replies/subscription dispatcher. Chosen.
    - Chosen Approach:
      - Add one legacy-message identity validator, pass only canonical connection ID downstream, convert request results to `oneshot`, and route document updates from one server-owned dispatcher to bounded authorized subscriptions.
    - API Notes and Examples:
      ```rust
      let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
      coordinator.schedule(request.with_reply(reply_tx))?;
      let result = reply_rx.await?;
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: protocol migration/`CloseDocument`-adjacent client shape and round trips.
      - `src/server/connection.rs`: central identity validation, shared edit/open follow-ups, subscriptions, bounded result delivery.
      - `src/server/{workspace,parse_coordinator,document_analysis,completion,language_intelligence}.rs`: access checks and request/subscription routing.
      - `src/client/mod.rs`: migrated message construction and close/subscription messages.
      - `tests/{parse_coordinator,completion_provider,language_intelligence,decoration_transport,runtime_update_protocol}.rs`: deterministic two-client tests.
    - References:
      - P0-2, P0-3; deletion-pass items 2–3.
  - Test Cases to Write:
    - Table-driven forged-ID tests for every `ClientMessage` family.
    - Two clients concurrently parse/complete/analyze different documents; no stolen, duplicated, or leaked output.
    - Read-only, stale-version, guessed-document save/reload/status/list tests.
    - Codec round trips for migrated messages and malformed/oversized frame regression.

- [ ] Harden bounded file reads and atomic save replacement
  - Acceptance Criteria:
    - Functional: Open/reload uses one opened handle, verifies handle type/metadata, reads at most `MAX_OPENABLE_FILE_BYTES + 1`, and rejects overflow; save creates an exclusive unpredictable same-directory temp, preserves required permissions, revalidates stable target identity immediately before replace, and leaves target unchanged on every failure.
    - Performance: Reads allocate at most the configured ceiling plus one byte; no heavy file I/O occurs under workspace mutex; collision retries are small and bounded.
    - Code Quality: Reuse one bounded-read helper across open/reload and applicable config/package/`.gitignore` reads; use standard `OpenOptions::create_new` and platform metadata rather than a new temp-file dependency unless exact Windows replacement semantics require otherwise.
    - Security: Symlink/file replacement, predictable temp precreation, same-length external edits, permission-copy failure, and TOCTOU races fail closed; Unix temp mode starts `0o600`.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/server-file-workspace.md`.
      - Tokio 1.52.2 local rustdoc/source and Context7 docs for `AsyncReadExt::take` and `OpenOptions::create_new`.
      - Rust standard-library docs for `std::fs::MetadataExt`/`OpenOptionsExt`; platform-gated Windows file identity source already used by project when available.
    - Options Considered:
      - Keep metadata-before-path-read: vulnerable.
      - Add third-party tempfile crate: unnecessary until standard APIs prove insufficient.
      - Open-once bounded read and exclusive same-directory temp. Chosen.
    - Chosen Approach:
      - Build the smallest shared handle-based read helper; generate names from OS randomness or existing secure platform facility, retry `AlreadyExists`, fsync temp, require metadata restoration, revalidate target identity, then atomic replace.
    - API Notes and Examples:
      ```rust
      let mut limited = file.take((MAX_OPENABLE_FILE_BYTES + 1) as u64);
      limited.read_to_end(&mut bytes).await?;
      let temp = tokio::fs::OpenOptions::new().write(true).create_new(true).open(path).await?;
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: bounded reads, stable file identity, exclusive temp, replacement validation/tests.
      - `src/perf/budgets.rs`: shared bounded auxiliary-read budgets only if absent.
      - Config/package source readers found by baseline: use same helper where authority/path model permits (tentative).
      - `docs/development/file-open-save-reload-workflow.md`: changed conflict behavior.
    - References:
      - P1-2, P1-3.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Hooked grow/replace between validation and read stays bounded and rejects.
    - Pre-created temp file/symlink, bounded collision exhaustion, permission-copy failure, target inode/file-ID replacement, and same-length edit preserve original target.
    - UTF-8, file-size boundary, concurrent edit during save, and existing atomicity regressions.

- [ ] Add document close lifecycle, bounded stores, and connection ceilings
  - Acceptance Criteria:
    - Functional: `CloseDocument`/unsubscribe is sent on explicit close and client LRU eviction; per-client and server-wide open-document ceilings align with the 64-session client budget; final close cancels work and removes document trees, caches, versions/generations, analysis routes, provider state, and leases.
    - Performance: All process-lifetime queues/stores have bounded capacity or coalescing/ring semantics; saturation exposes dropped/coalesced counts and never grows welcome/runtime snapshots past frame budgets.
    - Code Quality: Capacities live in existing budget modules; latest-state uses `watch`/coalescing where appropriate, events use bounded `mpsc`, diagnostics use bounded deduplicating `VecDeque`, and already-capped metrics are retained rather than rewritten.
    - Security: Accept loops enforce active connection caps; Unix endpoint parents reject unsafe group/world write permissions except an explicitly tested sticky-directory policy; disconnect/revocation cleans authority-bearing state.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/{multi-document-sessions,server-document-state,server-ipc-skeleton,parse-task-lifecycle}.md`.
      - Tokio 1.52.2 local rustdoc/source and Context7 docs for bounded `mpsc`, `watch`, and `Semaphore`.
      - `src/perf/budgets.rs`; `RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS` and current `PERF_SNAPSHOT_CAPACITY`.
    - Options Considered:
      - Raise limits only: postpones leak.
      - Global periodic GC: more moving parts and weak ownership.
      - Explicit final-close ownership cleanup plus hard ceilings. Chosen.
    - Chosen Approach:
      - Track client subscriptions/access in workspace state, remove state when final owner closes, use existing cancellation hooks, replace only genuinely unbounded stores, and guard listener accepts with a semaphore permit owned by each connection task.
    - API Notes and Examples:
      ```rust
      let permit = connection_limit.clone().acquire_owned().await?;
      tokio::spawn(async move { let _permit = permit; handle_connection(stream).await });
      ```
    - Files to Create/Edit:
      - `src/perf/budgets.rs`: document, channel, diagnostic, metric/drop, and connection capacities.
      - `src/protocol/mod.rs`, `src/client/mod.rs`, `src/editor/document_session.rs`: close/unsubscribe lifecycle.
      - `src/server/{mod,connection,workspace,syntax,parse_coordinator,completion,language_intelligence,document_analysis}.rs`: ceilings and final-close cleanup.
      - `src/perf/metrics.rs`: retain cap; add dropped count/aggregation only if baseline shows missing observability.
      - Relevant saturation/churn integration tests (tentative consolidation under task 9).
    - References:
      - P1-4, P1-8, P1-10.
  - Test Cases to Write:
    - Open/evict thousands of document IDs; retained maps/bytes stay under ceilings and dirty/leased close policy is explicit.
    - Saturate each queue; latest-state coalesces, request lanes reject/backpressure predictably, disconnected clients release capacity.
    - Repeated diagnostics deduplicate/cap and welcome/runtime snapshots remain encodable.
    - Unix unsafe parent and connection-flood tests; preserve default `$XDG_RUNTIME_DIR` behavior.

- [ ] Remediate RustSec advisories and enforce expiring audit policy
  - Acceptance Criteria:
    - Functional: Update `crossbeam-epoch` to at least 0.9.20; upgrade upstream chains permitting `quick-xml >=0.41` when compatible; classify each remaining advisory/warning by exact dependency path and runtime reachability.
    - Performance: Dependency upgrades do not regress startup, binary size, or existing benchmark work counts; avoid duplicate incompatible XML stacks solely to silence output.
    - Code Quality: CI runs `cargo audit`; any temporary ignore records advisory, reachability, upstream issue, owner, and expiry and fails after expiry.
    - Security: No advisory is silently allowed; untrusted D-Bus/introspection exposure is explicitly tested/documented until upstream remediation lands.
  - Approach:
    - Documentation Reviewed:
      - `Cargo.lock`, `cargo tree -i <crate>`, `cargo audit` advisory records.
      - Current upstream release notes/source for Winit/Masonry/Wayland/zbus chains, checked at execution time.
      - `AGENTS.md` version-exact Cargo/rustdoc policy.
    - Options Considered:
      - Blanket ignore: rejected.
      - Force duplicate `quick-xml`: may not remove vulnerable transitive use.
      - Immediate resolvable update plus tracked upstream-compatible upgrades. Chosen.
    - Chosen Approach:
      - Land lockfile-only fixes first, then smallest compatible direct dependency upgrades; use narrow expiring CI exceptions only where upstream constraints prove blocking.
    - API Notes and Examples:
      ```bash
      cargo update -p crossbeam-epoch --precise 0.9.20
      cargo tree -i quick-xml
      cargo audit
      ```
    - Files to Create/Edit:
      - `Cargo.toml`, `Cargo.lock`: compatible upgrades/profile constraints if required.
      - Linux CI workflow/config discovered in baseline: audit gate and expiring exceptions.
      - `docs/development/security.md` or existing dependency-policy page: temporary exposure/owner/expiry.
    - References:
      - P1-1.
  - Test Cases to Write:
    - `cargo audit` passes or reports only documented unexpired non-reachable exceptions.
    - Full Linux checks after each direct dependency upgrade.
    - Focused D-Bus/Wayland startup smoke when the UI/XML chain changes.

- [ ] Remove blocking work and head-of-line blocking from async services
  - Acceptance Criteria:
    - Functional: Directory listing snapshots authority under lock then traverses via bounded `spawn_blocking`; language-server sessions own independent actor tasks/queues; capped stderr continues draining after retention fills; git roots run concurrently under a small semaphore while preserving per-root command order.
    - Performance: Slow listing does not delay open/save/cancellation; one hung LSP cannot delay another session's read/write/stop; git latency scales with bounded root concurrency and existing per-command limits.
    - Code Quality: Reuse Tokio tasks, channels, semaphores, cancellation, and existing service types; no executor, filesystem-service trait, or generic actor framework.
    - Security: Authority/provenance is revalidated before work and actor ingress; all queues, traversal pages, `.gitignore`, stdio, timeouts, and subprocess counts stay bounded and sanitized.
  - Approach:
    - Documentation Reviewed:
      - Tokio 1.52.2 local rustdoc/source and Context7 `/websites/rs_tokio_1_49_0` `spawn_blocking`, bounded channel, and semaphore guidance.
      - `docs/wiki/modules/{server-file-workspace,language-server-process-service,git-discovery-service}.md`.
      - Approved language-server authority decision.
    - Options Considered:
      - Increase router timeout: keeps head-of-line blocking.
      - Generic actor framework: unnecessary.
      - One task per existing LSP session plus bounded central table routing. Chosen.
    - Chosen Approach:
      - Keep centralized identity/table mutations short; move child I/O into session-owned tasks, add RAII cancellation cleanup for listing, and use one small semaphore for root-level git concurrency.
    - API Notes and Examples:
      ```rust
      let page = tokio::task::spawn_blocking(move || traverse(snapshot, cancel)).await??;
      // Central router sends command; session actor alone awaits child stdio.
      ```
    - Files to Create/Edit:
      - `src/server/ops/workspace.rs`, `src/server/workspace.rs`: lock-free blocking traversal and bounded ignore input.
      - `src/server/language_server.rs`: session actors, bounded queues, drain/discard stderr.
      - `src/server/git.rs`: bounded root concurrency.
      - `src/perf/budgets.rs`: capacities/concurrency constants.
      - `tests/{language_server_authority,performance_protocol}.rs` plus workspace/git tests.
    - References:
      - P1-5, P1-7, P2-8, P3-2.
  - Test Cases to Write:
    - Deliberately slow listing while open/save completes; cancellation removes token on success/error/panic.
    - Hung LSP A while LSP B responds/stops within own deadline.
    - Verbose stderr beyond cap is drained to EOF while retained prefix/truncation flag stay bounded.
    - Multiple slow git roots never exceed semaphore capacity and preserve result association.

- [ ] Reduce test link/storage cost before considering crate splits
  - Acceptance Criteria:
    - Functional: Add measured dev/test debug-info profiles and consolidate 32 integration harness roots into a smaller coherent set without dropping test names, platform gates, or security coverage.
    - Performance: Record before/after clean and warm link time, binary count, `target/debug/deps` size, incremental size, and debugger usability; use one normal target directory and eliminate routine duplicate `target/pi-verify` guidance.
    - Code Quality: Consolidation uses plain test modules; no speculative production crate split or test framework wrapper.
    - Security: Audit, adversarial package/multi-client/filesystem tests, and Linux all-target coverage remain blocking and discoverable.
  - Approach:
    - Documentation Reviewed:
      - Cargo profile documentation for `debug = "line-tables-only"`/`debug = 1`, verified against installed Cargo documentation at execution time.
      - `tests/` harness inventory and current CI commands.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
    - Options Considered:
      - Split production crates now: high churn without measurement.
      - Set `debug = 0`: smallest artifacts but may harm debugger workflow.
      - Measure line tables/debug 1 and consolidate harnesses first. Chosen.
    - Chosen Approach:
      - Add profiles, benchmark one clean/warm cycle, group related files under a few roots (`security`, `runtime`, `editor`, `protocol` as measured), and keep source modules nearly unchanged via `#[path] mod` where useful.
    - API Notes and Examples:
      ```toml
      [profile.test]
      debug = "line-tables-only"
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: dev/test profile.
      - `tests/*.rs`: fewer harness roots with module grouping (exact mapping recorded before edits).
      - CI scripts/workflows: single target directory and target-size reporting.
      - `docs/development/build-and-test.md` or existing equivalent: cleanup/retention guidance.
    - References:
      - P1-9; deletion-pass item 5.
  - Test Cases to Write:
    - Enumerate pre/post test names to prove no coverage loss.
    - `cargo test --all-targets --no-run` and full Linux tests.
    - Record artifact count/bytes and link-time deltas; revert profile choice if debugger acceptance fails.

- [ ] Make runtime facades, registration identifiers, and orchestration single-source and remove panic startup
  - Acceptance Criteria:
    - Functional: Verify Plan 061 left one checked-in executable facade source feeding explicit trusted/third-party allowlists, with no embedded raw-string copy; finish repeated API/command identifier ownership, shared edit/open follow-ups, and fallible `IpcServer::try_new` startup migration.
    - Performance: Facade loading remains compile-time/static; orchestration consolidation adds no runtime dispatch layer or allocation.
    - Code Quality: Split oversized modules only along demonstrated existing responsibilities while touching them; use plain modules/functions, not factories, traits, or organization-only crates.
    - Security: Generated/included facades preserve closed exports and op allowlists; startup validation remains fail-closed; routing/security fields remain independently validated even when identifiers are generated.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/{clay-js-facade-skeleton,clay-js-doc-registry,embedded-js-runtime,server-ipc-skeleton}.md`.
      - Plan 061 final facade/domain inventory and `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,package-runtime-trust-domains}.md`.
    - Options Considered:
      - Redo Plan 061 facade/domain work here: duplicate churn; rejected.
      - Verify/finish Plan 061's checked-in executable facade and domain allowlists, then address only remaining identifiers/orchestration/startup. Chosen.
    - Chosen Approach:
      - Delete only residual facade duplication found by post-061 rebaseline, generate/reuse checked-in identifier tables from authoritative inventory while keeping independent security-field validation, share existing orchestration as plain functions, and remove `IpcServer::new` after callers migrate.
    - API Notes and Examples:
      ```rust
      const CLAY_FACADE_DOCUMENTS: &str = include_str!("../../runtime/js/documents.js");
      let server = IpcServer::try_new(config)?;
      ```
    - Files to Create/Edit:
      - `runtime/js/*.js` and paired `.d.ts`/`.ts` declarations: verify/finish Plan 061 one-source trusted/public facade ownership.
      - `src/server/js_runtime.rs`: remove any residual embedded bodies/domain drift after Plan 061.
      - `src/bin/update-doc-registry.rs` and checked-in generated registry/routing tables: reuse authoritative identifiers where feasible without merging security policy copies.
      - `src/server/connection.rs`: shared edit/open orchestration.
      - `src/server/mod.rs`, `src/main.rs`, `src/bin/clay-server.rs`, tests: fallible constructor migration.
      - `src/packages/record.rs` and other large modules only if task-local responsibility extraction produces a smaller diff (tentative).
      - `tests/{clay_js_facade_layout,rust_visibility_api_mapping}.rs`: source/visibility coverage.
    - References:
      - P2-1, P2-2, P2-7, P3-3; deletion-pass items 1–3 and 6.
  - Test Cases to Write:
    - Every trusted/third-party facade export/op route loads from included authoritative files and both inventories remain complete/disjoint where required.
    - Missing facade export fails deterministic tests.
    - Invalid startup root returns typed error from both binaries without panic.
    - Shared edit/open paths preserve acknowledgements, mode activation, analysis, completion, and syntax follow-ups.

- [ ] Replace brittle documentation needles with structured generic validators
  - Acceptance Criteria:
    - Functional: Enforceable primitive/package/API facts move to structured frontmatter/inventory schemas; generic validators enumerate all entries; only security statements whose exact semantic presence matters retain targeted markers.
    - Performance: Documentation tests compile/run materially faster or at minimum remove thousands of duplicated assertions without reducing coverage.
    - Code Quality: Delete superseded phase-specific needles and generate command/API matrices from the authoritative registry; tests never mutate docs.
    - Security: Permission, provenance, authority, hot-path, and containment invariants remain independently validated and failures identify exact page/field.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/{documentation-as-code,doc-registry-tests,maintenance-validation}.md`.
      - `docs/wiki/modules/{clay-js-doc-registry,maintenance-validation}.md`.
      - `tests/{primitives_docs,clay_js_api_inventory,package_loading_docs}.rs`.
    - Options Considered:
      - Delete doc tests wholesale: loses valuable coverage.
      - Keep exact prose needles: current maintenance burden.
      - Structured schema plus a small security-marker set. Chosen.
    - Chosen Approach:
      - Inventory unique facts, add the smallest schema fields needed, write one generic validator per registry type, then delete equivalent phrase assertions.
    - API Notes and Examples:
      ```text
      for entry in inventory.entries(): validate_required_fields(entry)
      security_markers = ["trusted subprocess authority", "no raw Deno.core.ops"]
      ```
    - Files to Create/Edit:
      - `docs/reference/**` frontmatter/inventory sources: structured facts where missing.
      - `tests/{primitives_docs,clay_js_api_inventory,package_loading_docs}.rs`: generic validators and deletions.
      - `src/bin/update-doc-registry.rs`: only if existing generator needs generic field support.
      - Generated registry artifacts via existing update command when source docs change.
    - References:
      - P2-3; deletion-pass item 4.
  - Test Cases to Write:
    - Removing any required registry entry/security field/index link fails with actionable path.
    - Semantically harmless prose rewrite passes.
    - `cargo run --bin update-doc-registry` produces no diff after regeneration.

- [ ] Correct ignore matching and bound sandbox framing
  - Acceptance Criteria:
    - Functional: Directory ignore behavior either implements a truthful minimal `*`/`?` grammar with correct backtracking and rejects unsupported syntax, or reuses an already-installed proven matcher at zero new dependency cost; sandbox reads reject an unterminated frame after at most `max + 1` bytes and kill/reap child.
    - Performance: Ignore work and input bytes/lines/patterns are bounded; sandbox parent allocation never scales beyond payload ceiling.
    - Code Quality: Prefer a proven already-installed crate only if Cargo confirms it is directly reusable without adding dependency/build cost; otherwise implement/test the small documented matcher and no partial `.gitignore` claim.
    - Security: Unsupported negation, escaping, character classes, path semantics, oversized `.gitignore`, and oversized child output fail visibly rather than broadening traversal or exhausting memory.
  - Approach:
    - Documentation Reviewed:
      - `src/server/workspace.rs::{build_ignore_set,gitignore_pattern_matches,glob_matches}`.
      - `src/server/runtime_sandbox.rs::read_frame` and `tests/runtime_sandbox_harness.rs`.
      - Tokio 1.52.2 local bounded read APIs.
    - Options Considered:
      - Full custom `.gitignore`: unnecessary and error-prone.
      - New glob dependency: avoid unless already resolved/directly usable.
      - Explicit minimal matcher or existing zero-cost implementation. Chosen after inventory.
    - Chosen Approach:
      - Make supported syntax truthful and small; replace `read_line` with bounded buffered accumulation that detects newline/overflow before JSON parsing.
    - API Notes and Examples:
      ```rust
      let mut limited = reader.take((max_payload_bytes + 1) as u64);
      // reject when no newline appears within limit
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: matcher/input limits/tests.
      - `src/server/runtime_sandbox.rs`: bounded frame reader.
      - `tests/runtime_sandbox_harness.rs`: hostile stream coverage.
      - Workspace/file-browser docs: exact supported ignore semantics.
    - References:
      - P2-4, P2-6.
  - Test Cases to Write:
    - `*ab` matches `aab`; `?`, UTF-8, path separators, unsupported `[x]`, negation, escape, and malformed patterns follow documented behavior.
    - Oversized `.gitignore` returns bounded diagnostic/page.
    - Oversized newline-terminated and unterminated sandbox streams allocate bounded memory and terminate child.

- [ ] Limit native dialogs and validate clipboard backend simplification
  - Acceptance Criteria:
    - Functional: Linux allows at most one in-flight file dialog and one folder dialog; duplicates are ignored/focused and state clears on success, cancel, send failure, and shutdown; clipboard behavior uses one backend only if Wayland/X11/macOS/Windows validation proves parity.
    - Performance: Repeated keypresses create no unbounded threads/portal requests; clipboard changes do not add polling or hot-path reads.
    - Code Quality: Use two explicit in-flight flags/generations in existing driver state; add no dialog manager/executor/custom UI component. Remove `arboard` only when existing Masonry/copypasta access covers Clay read/write fallback.
    - Security: Dialog capability/token flow remains server-authorized; stale dialog results cannot act on a newer generation; clipboard remains explicit user-command authority only.
  - Approach:
    - Documentation Reviewed:
      - `npx ui-skills start`; selected `wshobson/interaction-design` guidance for clear non-blocking feedback/state, translated to native Clay without web animation.
      - `.agents/skills/clay-ui/SKILL.md`, `references/{components,tokens}.md`.
      - `docs/wiki/modules/{client-file-dialog,multi-document-sessions}.md` and authority pattern clipboard rules.
      - Current `masonry_winit`/clipboard sources at locally resolved versions during execution.
    - Options Considered:
      - Dialog manager abstraction: unnecessary.
      - Disable command globally while any dialog runs: blocks independent file/folder intent.
      - Two in-flight states with generation checks. Chosen.
      - Remove `arboard` solely for dependency count: rejected without platform evidence.
    - Chosen Approach:
      - Add minimal driver state and completion event metadata; make no visual/catalog/token changes. Run backend matrix, then retain or remove `arboard` based on behavior.
    - API Notes and Examples:
      ```text
      file_dialog_in_flight: Option<Generation>
      folder_dialog_in_flight: Option<Generation>
      completion only clears/applies matching generation
      ```
    - Files to Create/Edit:
      - `src/main.rs`: in-flight dialog state and tests.
      - `src/client/clipboard.rs`, `Cargo.toml`, `Cargo.lock`: only if one-backend parity passes.
      - `docs/development/{linux,windows}.md` and platform smoke docs: validation results.
      - No `.agents/skills/clay-ui/references/*` update expected because no component/token/layout changes.
    - References:
      - P2-5, P3-1; deletion-pass item 7.
  - Test Cases to Write:
    - Repeated file/folder command tests prove one thread/request per kind and proper cancel/error reset.
    - Stale generation result ignored.
    - Linux Wayland/X11 blocking/nonblocking read-write-paste smoke; macOS/Windows checks non-blocking unless task explicitly runs on those hosts.

- [ ] Run full review closure and performance verification
  - Acceptance Criteria:
    - Functional: Every ledger row has passing closure evidence; Plan 056's P1-6 outcome is reverified rather than reimplemented; all changed protocol/package/filesystem/lifecycle behavior passes adversarial and regression suites.
    - Performance: Confirm one syntax parse per document/version/window, bounded queues/stores, non-blocking listing/LSP behavior, and measured build artifact improvement; regressions require fix or explicit user-approved compromise.
    - Code Quality: Linux `fmt`, `check`, `clippy`, all-target tests, doc registry freshness, and `git diff --check` pass; no broad lint allowances or unrelated cleanup.
    - Security: `cargo audit` policy passes; Plan 061 cross-domain/provenance/composition closure and this plan's multi-client isolation tests are blocking; no finding closes on documentation alone where executable behavior exists.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
      - `docs/wiki/modules/maintenance-validation.md`.
      - Completion ledger from first task.
    - Options Considered:
      - One monolithic final test only: poor failure localization.
      - Focused checks per task plus full Linux gate here. Chosen.
    - Chosen Approach:
      - Run focused suites during each task, then one clean all-target closure; record exact commands/results and before/after build metrics in this plan.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo audit
      git diff --check
      ```
    - Files to Create/Edit:
      - `plans/060-Comprehensive-Codebase-Review-Remediation.md`: final ledger and measured evidence.
      - CI files only if missing gates were identified by prior tasks.
    - References:
      - All P0–P3 findings; P1-6 specifically maps to completed Plan 056 verification.
  - Test Cases to Write:
    - Full Linux blocking command set above.
    - Focused hostile two-client, post-Plan-061 package-domain/provenance, filesystem race, queue saturation, LSP isolation, sandbox overflow, and dialog tests.
    - Compare test count/names and artifact measurements to baseline.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Audit all changed server-side Rust visibility and user/package-facing behavior after Plan 061; avoid duplicating its package API work, expose remaining genuine public capabilities through stable facade/op APIs and docs, and keep runtime-domain/package-context IDs, routing, lifecycle, queue, filesystem, and scheduler mechanics private or `pub(crate)`.
    - Performance: No API exposes per-keystroke callbacks, queue capacities, raw package-context/domain handles, arbitrary process/file handles, or paint-path execution.
    - Code Quality: Every public API has stable ID, concise authority-aware callable, user-facing name, key bindings, custom properties, docs, inventory, op/facade paths, generated registry, lookup, and tests; raw `Deno.core.ops` remains private.
    - Security: Public package-control, close/save, or authorization changes cannot bypass provenance, grants, connection identity, leases, versions, roots, payload limits, or revocation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API requirements.
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,doc-registry-tests}.md`.
      - Existing package/document/workspace/runtime API docs and inventory.
    - Options Considered:
      - Expose internal package-context/domain/routing controls: rejected.
      - Preserve existing public shapes where behavior can harden transparently; document only real shape/behavior changes. Chosen.
    - Chosen Approach:
      - Inventory changed `pub` items after implementation, map each to existing/new API or reduce visibility, update authoritative Markdown and regenerate only when public behavior changed.
    - API Notes and Examples:
      ```text
      Expected internal-only: PackagePrincipalId, dispatcher subscriptions, queue capacities, file identity.
      Potential public update: serverSaveDocument errors/version semantics; package-control authorization behavior.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/index.md`, `docs/reference/clay-js-api/api-inventory.toml`: only for public changes.
      - `runtime/js/*.js|*.ts|*.d.ts`, op modules: only for approved public shapes.
      - Generated registry via `cargo run --bin update-doc-registry`.
      - `tests/{clay_js_api_inventory,clay_js_doc_registry,clay_js_facade_layout,rust_visibility_api_mapping}.rs`.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
  - Test Cases to Write:
    - Rust visibility/API mapping fails for unmapped public server functions.
    - Facade/docs/index/registry/lookup freshness tests.
    - Internal runtime-domain/package-context/queue/file-identity names are absent from public exports.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review changed behavior for legitimate user choices after Plan 061; avoid duplicating its package adoption/replacement configuration, and default to no new settings for security ceilings, package-context validation, connection identity, queue sizes, atomic save rules, routing, or internal concurrency.
    - Performance: No hidden tuning keys for hot-path parse, queue, actor, target-directory, or dialog internals; measured build profiles remain repository policy rather than runtime config.
    - Code Quality: Any genuine configurable behavior is a documented `~/.config/clay/init.js` Clay JS API with complete schema, registry, and tests; otherwise record no API needed.
    - Security: Configuration cannot mint package contexts, promote runtime domains, weaken grants/identity/lease/version/path checks, increase security ceilings, or silently grant package/process/filesystem/network authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `docs/reference/clay-js-api/configuration.md` and relevant package authority APIs.
    - Options Considered:
      - User-tunable safety limits: rejected.
      - Fixed compiled budgets with diagnostics and evidence-based later decision. Chosen.
    - Chosen Approach:
      - Audit after implementation; add no setting unless behavior represents a real user policy choice rather than an implementation/safety invariant.
    - API Notes and Examples:
      ```text
      Expected additions: none.
      Explicitly non-configurable: runtime-domain/package-context binding, identity checks, queue/frame/file/connection ceilings.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`, `docs/index.md`, inventory/generated registry, and configuration tests only if a real setting is approved.
      - `plans/060-Comprehensive-Codebase-Review-Remediation.md`: no-new-setting audit result.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - Closed configuration/property allowlists reject internal/security limit names.
    - Existing configuration docs/registry tests remain green.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, external authority, and remaining limitations without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: project wiki workflow and quality bar.
      - `.agents/skills/project-wiki/references/page-template.md` when creating/substantially rewriting pages.
    - Options Considered:
      - Update after each task: noisy and likely to drift.
      - Update once after tests pass: aligns docs with final code. Chosen.
    - Chosen Approach:
      - Update the Markdown wiki once after implementation/API/configuration verification, including master index and relevant module/flow pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/package-principal-and-result-routing-primitive-review.md
      docs/wiki/modules/server-ipc-skeleton.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation for changed/new pages.
      - `docs/wiki/**`: post-Plan-061 package-domain verification, result routing, workspace I/O, document lifecycle, LSP actors, runtime facades, build/maintenance, and UI dialog implementation pages as changed.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Wiki index coverage test/manual review confirms every page is linked and changed pages document source paths, flow, limits, authority, tests, and extension guidance.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, owner, expiry, and priority.
