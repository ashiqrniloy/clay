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

- [x] Complete prerequisite Plan 061 before remaining review remediation
  - Completion Evidence (2026-07-21): Plan 061 fully closed at 16/16 tasks. Two-domain runtime (trusted 67 ops / third-party 35 ops, 21/13 facade allowlists, fail-closed by type absence), host-stamped `PackageContext` provenance across all package-facing ops with caller-manifest functions deleted, durable `PackageApprovalStore` adoption gate (no code before approval), extension points with dual consent, atomic first-party replacement with rollback, cross-domain bridge with poison-replay, `production_reload` preserving third-party state across config reload, CLI adopt/revoke/rollback, adversarial tests (cross-domain denial, A-as-B, no-preapproval execution, stale/revoked approvals, malformed stores, replacement escalation, grant non-transfer), two-runtime resource cost recorded (+5 MiB RSS / +2 threads), wiki updated. Post-061 rebaseline: Plan 060 T4 (connection identity/routing), T6 (bounds), T8 (concurrency), T10 (facade orchestration) resume against the final two-domain architecture; `cargo test --all-targets` 1748 passed / 0 failed at gate close.
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

- [x] Make connection identity canonical and route outputs to authorized clients
  - Completion Evidence (2026-07-21):
    - **P0-2 central identity boundary**: `client_message_identity` in `src/server/connection.rs` extracts the legacy caller-supplied ID from all 16 post-`Hello` families; one pre-dispatch gate rejects any mismatch with `Error(InvalidMessage)` before any arm runs. Redundant per-arm checks (`DecorationViewportRequest`, `SduiAction`, `CommandIntent`) removed; downstream arms only see the canonical connection identity.
    - **Connection-owned document access**: `DocumentState` gains an `access_holders` set populated only by authorized open paths (welcome acquire, OpenDocument, OpenSelectedFile) with `has_access` gating. `WorkspaceState::document_metadata`/`list_documents`/`save_document`/`reload_document` now fail closed with `UnknownDocument` for documents the connection never opened (no existence/metadata oracle). Save requires the editable lease (`ReadOnlySave`/`AccessDenied` for read-only connections) and validates `known_version` (`StaleSaveVersion`/`StaleFileMetadata` for future-version claims; `known_version <= current` accepted because only the lease holder can edit, so any gap is the caller's own in-flight edits). `RequestResync` gated identically — a guessed workspace document id can no longer pull full text. Server-internal runtime identity (0) retains full authority for trusted config document ops.
    - **P0-3 authorized output routing**: new `src/server/output_router.rs` (`OutputRouter<T>`) holds per-client bounded senders (capacity 64) plus a document→client subscription index. `ParseCoordinator` routes `IncrementalParseUpdate` only to connections subscribed to that document and broadcasts sanitized diagnostics to all subscribers; `DocumentAnalysisCoordinator` routes Decorations/Diagnostics by document and broadcasts worker-failure diagnostics via a worker-facing `AnalysisOutputSink`. Legacy global coordinator channels retained but bounded (4096) for tests/internal tooling. Connection subscribes at welcome (default doc) and every `DocumentOpened` arm; a `ConnectionOutputSubscriptions` drop guard withdraws all subscriptions on every exit path (clean close, IO error, disconnect).
    - **Completion oneshot**: `schedule_completion` now returns a request-scoped `oneshot::Receiver<CompletionResultSet>` (mirroring language-intelligence); shared results channel, cross-request/cross-connection result stealing, and `drain_pending_results` removed. ~20 call sites in `tests/completion_provider.rs` and `src/server/js_runtime.rs` converted.
    - **Edit/EditorIntent normalization**: both arms now share `dispatch_edit_operation` (one apply/ack/follow-up path); intent→operation conversion is the only per-arm code.
    - **Protocol shape**: legacy post-`Hello` IDs retained on the wire (rkyv compatibility) but equality with the handshake identity is now enforced, making the field vestigial; removal deferred to a protocol version bump.
    - New tests (7): `forged_client_identity_is_rejected_for_every_message_family` (table-driven, all 16 families, no-side-effect assertion), `two_client_parse_updates_are_isolated_to_the_subscribed_connection`, `save_reload_status_list_enforce_connection_owned_access` (resync/status/list unknown-doc, read-only save, future-version save, lease-holder save), 4 `output_router` unit tests (document isolation, unsubscribe, broadcast, full-channel drop). `editor_performance_invariants` static guards updated to follow the extracted dispatcher.
    - Validation: `cargo test --all-targets` 1757 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `git diff --check` clean.
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

- [x] Harden bounded file reads and atomic save replacement
  - Completion Evidence (2026-07-22):
    - **P1-2 bounded handle-based reads**: new `read_file_bounded` in `src/server/workspace.rs` is the single read path for open (`open_io`) and reload (`reload_io`): one `tokio::fs::File::open` handle, handle-based metadata validation (type + size fast-reject), then `take(MAX_OPENABLE_FILE_BYTES + 1).read_to_end` so allocation is capped at the ceiling plus one byte and post-validation growth rejects with typed `FileTooLarge`. Registry now records the metadata of the handle actually read (open + selected-open + reload), so the first save's staleness baseline matches the bytes in the document; the separate post-read `tokio_fs::metadata` call in `reload_io` is gone. `.gitignore` reads use the sync sibling `read_auxiliary_file_bounded` (new `MAX_AUXILIARY_READ_BYTES = 1 MiB` budget) and fail closed to file-absent behavior. Config/package source readers keep their own error types and were not converted (trusted startup-local reads; the tentative ledger item).
    - **P1-3 atomic save hardening**: temp names are now unpredictable (process-random `RandomState` seed mixed with a unique counter — std OS-entropy keys, no new dependency) and created exclusively via `OpenOptions::create_new` with up to 8 bounded `AlreadyExists` retries (`create_exclusive_temp`), so a pre-created file or symlink at a guessed temp path is never truncated or followed; Unix temp mode starts `0o600` (new files keep it, existing files get their original mode restored). Permission restore on Unix is now fail closed (restore error → temp removed → save fails). New `TargetIdentity` ((dev, ino) on Unix, volume serial + file index on Windows, plus len + modified) is captured by `reauthorize_open_file` from the same `fs::metadata` call as before (zero added syscalls, none under the workspace mutex beyond the pre-existing stat) and revalidated immediately before the rename: external edit, same-length edit, atomic replace, symlink swap, or target removal during the temp write fails with typed `StaleFileMetadata` (`AtomicSaveError::TargetChanged`) and preserves the external bytes.
    - **Test hooks**: three path-scoped `#[cfg(test)]` hooks (`TEST_TEMP_NAMES` queue, `BEFORE_REVALIDATE_HOOKS`, `BETWEEN_METADATA_AND_READ_HOOKS`) keyed by target path so parallel tests cannot consume each other's staged hazards.
    - New tests (9): grow-between-validation-and-read rejects bounded; pre-created temp file ignored (retries, sentinel untouched); pre-created temp symlink not followed; bounded collision exhaustion fails closed with no litter; target replaced before rename fails closed preserving replacement; same-length external edit fails closed; end-to-end `save_document` reports typed `StaleFileMetadata` when target changes during write; Unix `0o600` start + mode preservation across replace; `.gitignore` bounded read. UTF-8/oversized/at-limit/concurrent-edit/regression coverage already existed and still passes.
    - Not staged: permission-copy failure (as same user on Linux, `chmod` on an owned temp always succeeds; the fail-closed path is implemented and its cleanup is shared with the collision-exhaustion coverage).
    - Docs: `docs/development/file-open-save-reload-workflow.md` updated (bounded single-handle open/reload read, exclusive unpredictable temp, identity revalidation, platform table).
    - Validation: `cargo test --all-targets` 1766 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `git diff --check` clean.
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

- [x] Add document close lifecycle, bounded stores, and connection ceilings
  - Completion Evidence (2026-07-22):
    - **P1-4 close lifecycle + ceilings**: new `ClientMessage::CloseDocument { client_id, document_id, force }` (identity-gated by the T4 boundary) and `ServerMessage::DocumentClosed { document_id, closed }` ack. `WorkspaceState::close_document` fails closed as `UnknownDocument` for non-holders, requires `force` for dirty documents (explicit policy), releases the caller's access, and on final-holder close removes the registry + path index entries. Final close (explicit or disconnect via `cleanup_connection_documents`) tears down all document-scoped state: `ParseCoordinator::remove_document` (versions, native-edit acceptance, active parse tasks), `CompletionCoordinator::remove_document` and `LanguageIntelligenceCoordinator::remove_document` (versions, generations, active tasks), `DocumentAnalysisCoordinator::close_document` (routes + workers), and the connection's parse/analysis subscriptions. Disconnect now finalizes documents whose last holder left (`release_client_access` returns finals) instead of leaking them; documents still held by other connections keep all state. Ceilings: `MAX_DOCUMENTS_PER_CLIENT = CLIENT_DOCUMENT_SESSION_MAX (64)` and `MAX_SERVER_DOCUMENTS = MAX_ACTIVE_CONNECTIONS × 64 (4096)` enforced at every acquire/register path with typed `WorkspaceLimitExceeded`. Client LRU eviction now sends `CloseDocument(force: true)` per evicted session (`SessionEviction` reports evicted IDs; `ClientEditQueue::enqueue_close_document`).
    - **P1-8 bounded lanes**: per-connection completion/language-intelligence result lanes converted from unbounded to bounded `mpsc` (`CONNECTION_RESULT_LANE_CAPACITY = 64`) with `try_send` drop + counter + log on saturation; server-side `runtime_diagnostics` Vec replaced by `RuntimeDiagnosticStore` (bounded deduplicating `VecDeque`, capacity `RUNTIME_DIAGNOSTIC_CAPACITY = 32` aligned with the snapshot publication cap, consecutive-duplicate collapse, retained drop count) so welcome/runtime snapshots never grow past frame budgets. Remaining process-lifetime lanes were already bounded: parse channels (4096, T4), analysis input/output (64), metrics recorder (4096), request lanes are per-request oneshots (T4), session store (64).
    - **P1-10 endpoint + connection caps**: `MAX_ACTIVE_CONNECTIONS = 64` enforced by a semaphore permit owned by each connection task in the shared `spawn_connection` (covers Unix and Windows accept loops); excess connections are refused at accept time and released permits let later connections in. `validate_parent_directory_mode` rejects group/world-writable socket parents unless the sticky bit marks a sanctioned shared-temp policy (`/tmp`-style `0o1777`); default owner-only `$XDG_RUNTIME_DIR` behavior unchanged (tested at `0o700`/`0o777`/`0o1777`).
    - New tests (7): per-client ceiling open (65th fails, other client unaffected); close lifecycle (dirty/force, non-holder, shared, final registry removal, disconnect finalize); connection-level close ack + teardown + access loss; disconnect finalization over the wire; diagnostic store dedup/cap/drop-count; socket parent mode policy; connection-limit refuse-and-recover.
    - Validation: `cargo test --all-targets` 1773 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `git diff --check` clean.
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

- [x] Remediate RustSec advisories and enforce expiring audit policy
  - Completion Evidence (2026-07-22):
    - **Resolved (5 findings):** `crossbeam-epoch` 0.9.18→0.9.20 (RUSTSEC-2026-0204 closed); `anyhow` 1.0.102→1.0.104 (RUSTSEC-2026-0190 closed); `memmap2` 0.9.10→0.9.11 (RUSTSEC-2026-0186 closed); `quick-xml` 0.39.3→0.39.4 + `wayland-protocols` 0.32.12→0.32.13 lockfile bumps (cleared the stale yanked/unmaintained `fuchsia-cprng`/`instant`/wayland warnings). Vulnerabilities went 3→2, warnings 5→3; `cargo audit` exits 0.
    - **Upstream-blocked (2 vulnerabilities):** RUSTSEC-2026-0194/0195 (`quick-xml` 0.39.4, newest in the 0.39 line). Two dependency paths, both blocked by unreleased upstream fixes: (1) build-time proc-macro via `wayland-scanner 0.31.10` — the `quick-xml 0.41` bump is merged (Smithay/wayland-rs PR #938, 2026-07-08) but unpublished; (2) runtime via `zbus_xml 5.1.1` (AT-SPI/D-Bus accessibility chain) — `zbus_xml 5.2.1` dropped `quick-xml` for `winnow`, but reaching it needs `accesskit_winit 0.33`, which `masonry_winit 0.4.0` (newest release) does not permit. Reachability: path 1 parses only checked-in Wayland protocol XML at build time; path 2 parses AT-SPI session-bus introspection XML at runtime (same-UID peers; worst case local DoS via crafted XML panic — no memory-safety impact; Clay speaks no other D-Bus).
    - **Expiring exception policy:** `.cargo/audit.toml` ignores exactly the two quick-xml advisories; `docs/development/security.md` records each with dependency path, runtime reachability, upstream reference, owner, and expiry (2026-10-22), plus a classification table for the three remaining warnings (bincode via deno_core, paste via v8, ttf-parser via ab_glyph — all unmaintained, none runtime-reachable to untrusted input). New test `audit_exceptions_are_documented_and_unexpired` (in existing `tests/primitives_docs.rs`, no new binary) fails the test gate when any expiry passes or an ignored advisory lacks documentation.
    - **CI:** the project had no CI; created `.github/workflows/ci.yml` with an audit job (cargo-audit + the expiry test). Audit remains a local gate too.
    - **Performance:** patch-level updates only; all performance-invariant and benchmark-work-count suites pass unchanged; no duplicate XML stacks introduced (single `quick-xml` remains; no forced duplicates).
    - Validation: `cargo audit` exit 0; `cargo test --all-targets` 1774 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `git diff --check` clean.
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

- [x] Remove blocking work and head-of-line blocking from async services
  - Completion Evidence (2026-07-22):
    - **P1-5 workspace listing:** `WorkspaceState::prepare_directory_listing` now copies only the authorized canonical root plus clamped request ceilings while the workspace mutex is held. `op_clay_workspace_list_directory` releases that lock, acquires one of `DIRECTORY_LISTING_MAX_CONCURRENCY = 4` permits, and runs the synchronous `traverse_directory` through `tokio::task::spawn_blocking`; canonical target/type/root-containment revalidation, bounded `.gitignore`, child scans, recursion, sorting, and page construction all happen outside the lock. Cancellation is checked before filesystem I/O, after target validation, and during traversal. `ListingCancellationGuard` requests cancellation and removes the active token on success, error, dropped op future, or blocking-task panic. Token ID allocation no longer registers state before a listing starts, and re-registering an existing ID reuses rather than replaces its token (no lost cancel race).
    - **P1-7 LSP session actors:** `LanguageServerProcessService` retains one dedicated current-thread Tokio runtime and central bounded router, but the router now performs only exact package/contribution/fingerprint checks, session-table mutations, and non-blocking actor dispatch. Every session has its own task, child/stdin/stdout/stderr ownership, stop signal, and bounded `mpsc` queue (`LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY = 8`). A hung read/write blocks only that session; full ingress returns typed `SessionBusy` instead of blocking the router. Stop/revoke/shutdown remove table entries first, signal independently of queue fullness, then kill/reap actors concurrently. Existing fixed executable/argv/env/canonical-root grants, no-shell launch, 16-process cap, 1 MiB messages, 30-second read maximum, runtime-domain ownership, and per-op PackageService revalidation remain unchanged; the router revalidates identity again immediately before actor ingress.
    - **P2-8 stderr:** `StderrCapture` retains only the first `LANGUAGE_SERVER_STDERR_BUDGET_BYTES` (64 KiB) and records a truncation flag. `read_capped_stderr` continues reading/discarding through EOF after retention fills, preventing a verbose child from blocking on a full pipe; diagnostics still sanitize and cap output.
    - **P3-2 Git roots:** `GitDiscoveryService` owns a shared `GIT_ROOT_CONCURRENCY = 4` semaphore. Every root holds one permit for its complete ordered command sequence (repository root → branch/detached head → status), while `discover_workspace_statuses` fans roots through `JoinSet` and sorts snapshots back by workspace-root ID. Cache refresh paths share the same service/semaphore, so direct and cached discovery have one subprocess ceiling and result association remains authoritative.
    - **Tests (7 new, no new harness):** FIFO-backed slow listing proves open + save complete within their own deadline while traversal blocks, then verifies cancellation and token cleanup; separate error/unwind token-cleanup tests; hung LSP A while LSP B writes/reads/stops under one second; actor queue overflow returns `SessionBusy`; over-cap stderr drains a 72 KiB duplex stream to normal EOF while retaining exactly 64 KiB + truncation; three fake Git roots under a two-permit test budget prove only two start, each root's command order is preserved, and IDs/paths remain associated.
    - **Documentation/API:** updated existing `server-file-workspace`, `language-server-process-service`, and `git-discovery-service` wiki pages; no public Clay JS API or configuration shape changed. Tokio 1.52.2 was verified against local Cargo resolution/source and current Context7 `/websites/rs_tokio` guidance for `spawn_blocking`, bounded `mpsc`, `Semaphore`, and `JoinSet`; no executor, filesystem trait, actor framework, or dependency added.
    - **Performance/validation:** deterministic deadlines cover lock independence and cross-session HOL isolation; existing `performance_protocol` 19/19 and benchmark work-count suites pass unchanged. Full Linux gate: `cargo test --all-targets` 1781 passed / 0 failed (including all benchmark harness self-checks); `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo audit` (only the three documented allowed warnings), and `git diff --check` all clean.
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

- [x] Reduce test link/storage cost before considering crate splits
  - Completion Evidence (2026-07-22):
    - **Rebaseline + profile choice:** execution found 33 (not the stale planned 32) top-level integration sources. Installed Cargo/rustc 1.96.1 documentation and current Context7 `/websites/doc_rust-lang_cargo` confirm that `test` inherits `dev`, `line-tables-only` retains minimal source/line debug information, and explicit test targets plus `autotests = false` disable one-binary-per-file discovery. `Cargo.toml` now sets line tables explicitly for both routine `dev` and `test`, plus opt-in `[profile.debugging] inherits = "test"; debug = 2` for variable/type debugging; `debug = 0` was rejected. No dependency or production crate split was added.
    - **Harness consolidation:** 33 source files remain readable at their original `tests/*.rs` paths but are included by four plain roots: `tests/suites/security.rs` (7 package/process/visibility authority modules), `runtime.rs` (10 command/provider/LSP/parse/runtime/syntax modules), `editor.rs` (7 decoration/editor/Markdown/diagnostic/theme/typography modules), and `protocol.rs` (9 API/doc/facade/fixture/budget/protocol modules). Cargo all-target harnesses fall from 43 (5 unit/bin + 33 integration + 5 Criterion) to 14 (5 + 4 + 5). Focused invocation is now group + source-module filter, e.g. `cargo test --test security package_loading::`; active docs, fixture README, CI, and documentation-test needles were migrated. Historical completed plans retain their historical commands.
    - **Coverage proof/security:** pre-change `cargo test --tests -- --list` enumerated 1,782 names including the one ignored environment-gated smoke. Final enumeration has 1,783; after removing the expected source-module namespace and the new inventory guard, its multiset exactly equals all 1,782 prior names (zero missing/added). `integration_suite_inventory_assigns_every_source_once` reads all top-level test sources and four roots, failing on omission or duplicate assignment. Package/adoption/LSP/sandbox/visibility tests remain discoverable under `security`; multi-client/filesystem tests remain library tests; audit expiry is under `protocol`; all stay in `cargo test --all-targets`.
    - **Measured clean build/storage:** on the same Linux x86_64 host and normal repository `target/`, `cargo clean && cargo test --all-targets --no-run` improved 89.070 s → 61.724 s (-30.7%); `target/` 21,942,578,072 → 6,711,040,962 bytes (-69.4%); `target/debug/deps` 19,521,134,614 → 5,213,466,962 bytes (-73.3%); incremental 1,953,003,235 → 1,031,228,815 bytes (-47.2%); executable harnesses 43 → 14 (-67.4%). No-op warm check was 238 → 222 ms; the Plan T1 warm relink snapshot versus touching one consolidated source was 15.903 → 7.250 s (-54.4%). Timings remain snapshots, not gates.
    - **Debugger acceptance:** routine `protocol` binary contains DWARF line tables; `llvm-symbolizer` resolved the inventory guard symbol to `tests/suites/protocol.rs:21`. `cargo test --profile debugging --test protocol --no-run` built in 55.802 s and emitted full parameter/local-variable DWARF; those optional 7.6 GiB artifacts were removed with `cargo clean --profile debugging`. GDB/LLDB were absent on the measurement host, so interactive stepping was not claimed; docs state routine line-table limits and the full-debug command.
    - **Single target + CI/docs:** all active `target/pi-verify` command guidance was removed; `docs/development/build-and-test.md` documents profiles, suite mapping, focused/full commands, metrics, debugger boundary, security discovery, and cleanup. `docs/wiki/modules/maintenance-validation.md` records implementation/invariants. CI is one Linux job sharing normal `target/` across audit/fmt/check/clippy/all-target tests and reports deps/incremental/total sizes plus executable count; no custom runner, cache layer, or framework wrapper was introduced.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1,782 passed / 0 failed / 1 ignored; the one new guard accounts for the pass-count increase), `cargo audit` (only three documented allowed warnings), and `git diff --check` pass. Benchmark harness self-checks and all Linux platform gates remain included.
  - Acceptance Criteria:
    - Functional: Add measured dev/test debug-info profiles and consolidate 33 integration harness roots into a smaller coherent set without dropping test names, platform gates, or security coverage.
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

- [x] Make runtime facades, registration identifiers, and orchestration single-source and remove panic startup
  - Completion Evidence (2026-07-22):
    - **P2-1/D1 one executable facade source:** deleted all 21 `CLAY_FACADE_*` raw-string bodies from `src/server/js_runtime.rs` and replaced the 21 implementation-bearing `runtime/js/*.ts` mirrors with 21 checked-in executable `*.js` files plus 21 declaration-only `*.d.ts` files. New focused `src/server/facades.rs` is the only Rust facade inventory: each row owns one `clay:*` specifier, one compile-time `include_str!("../../runtime/js/<domain>.js")`, and one `TrustedOnly`/`Public` classification. `ClayModuleLoader` uses that table for both source resolution and domain admission; there is no runtime file read/transpilation/allocation or embedded second body. `js_runtime.rs` fell 12,060 → 11,423 lines; the extracted table is 129 lines.
    - **Two-domain security closure:** all 21 trusted facades and exactly 13 public-third-party rows remain unchanged from Plan 061. `facade_inventory_is_unique_and_domain_partitioned`, `runtime_facades_are_included_from_authoritative_js_files`, and `third_party_facade_allowlist_exactly_matches_plan_public_inventory` prove unique/exhaustive disk→include mapping, adjacent declarations, no raw-string constants, and exact independent Plan 061 security classification. Existing runtime/domain tests prove public imports execute and trusted-only imports/ops remain absent from third-party runtime. Facade implementations may call Clay-owned ops internally; exports still reject op-shaped names and no V8/native values cross domains.
    - **P3-3 identifier ownership:** one `builtin_commands!` declaration in `src/server/command_execution.rs` now owns every built-in command ID, display name, and routing category and emits the named constants, ordered discovery slice, and metadata table. Separate ID/display/routing matches and workspace/Git/mode arrays were deleted; `builtin_command_table_owns_unique_ids_and_registration_fields` independently validates uniqueness plus security-sensitive routing. Parse, completion, and language-intelligence provider ops now share one `ops::registration_token(api_prefix, contribution_id, index)` formatter with a pinned-format test instead of three `format!` copies. Cross-language facade/docs identifiers remain independently validated contracts; security fields were not generated away or merged with documentation.
    - **P2-2/D2-D3 orchestration:** T4's existing `dispatch_edit_operation` remains the single Edit/EditorIntent apply→ack→completion/intelligence/analysis/syntax path. New plain `write_document_open_response` is now called by direct workspace open, selected-file open, and SDUI/command open; it alone writes `DocumentOpened`, subscribes parse/analysis routes, captures the active runtime generation, emits mode/decorations follow-ups, and starts document analysis. No context object, trait, factory, dispatch layer, or added allocation. Existing direct/selected/file-browser parity and post-ack hot-path tests pass.
    - **P2-7/D6 fallible startup:** `src/main.rs` and `src/bin/clay-server.rs` now call `IpcServer::try_new` and propagate `ServerError` into typed launch results. `IpcServer::new` is compiled only for unit tests; production cannot call the panic wrapper. Existing invalid-root test proves `ServerError::InvalidWorkspaceRoot`; new `production_server_binaries_use_fallible_constructor` statically prevents either binary regressing to `new`.
    - **Docs/registry/wiki:** all implemented facade metadata paths now point to executable `runtime/js/*.js`; `cargo run --bin update-doc-registry` refreshed the checked-in registry. `runtime/js/README.md`, `clay-js-facade-skeleton`, `embedded-js-runtime`, `server-ipc-skeleton`, `command-registry`, and affected implementation/reference pages document executable/declaration ownership, static domain table, token/command identity, shared open flow, and fallible startup; wiki index remains linked. No public API shape or configuration option was added.
    - **Scope/deletions:** `src/server/facades.rs` is the only oversized-module split because facade ownership was a demonstrated `js_runtime.rs` responsibility. `packages/record.rs`, `editor/surface.rs`, crates, and unrelated validation were not reorganized. Node syntax checks pass for all 21 JavaScript files; source/declaration pairing is 21/21.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1,786 passed / 0 failed / 1 ignored), `cargo audit` (only three documented allowed warnings), generated-registry freshness, all 21 `node --check` runs, and `git diff --check` pass. Focused closure includes 149 JS-runtime tests (148 passed / 1 ignored), 62 API inventory, 34 doc registry, 52 package docs, 44 package loading, 133 primitive docs, 18 command execution, facade/domain/security tests, and all three open-origin regressions.
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

- [x] Replace brittle documentation needles with structured generic validators
  - Completion Evidence (2026-07-22):
    - **P2-3/D4 deletion:** rewrote the three reviewed documentation suites instead of preserving phase-by-phase phrase lists. `tests/{primitives_docs,clay_js_api_inventory,package_loading_docs}.rs` fell from 15,530 → 1,162 lines (-92.5%), 2,403 assertion/needle-like sites → 88 (-96.3%), and 247 tests → 22 generic tests (-225 superseded prose tests). Focused execution fell from the immediately preceding ~0.11 s aggregate (0.03/0.05/0.03) to ~0.03 s (0.01/0.02/<0.01); compilation also parses 14,368 fewer Rust source lines in the consolidated protocol harness.
    - **Structured primitive/package contracts:** new `docs/reference/documentation-contracts.json` schema v1 enumerates all 15 `docs/reference/primitives/*.md` pages and all 5 package reference pages exactly once, names every required index, binds the four documented language packages to exact package manifests, and carries only five narrow security-contract rows. Generic validators enumerate directory contents, reject missing/duplicate IDs/paths/index links with the exact page, require basic H1/H2 structure, bind package name/load entry/API prefix/default typography role to `package.json`, recursively ensure all 87 non-index wiki pages (88 Markdown files total) are indexed, and never mutate files.
    - **Primitive matrix:** one generic parser validates every row in the existing structured `docs/reference/primitives/registry.md` category matrix: 15 columns, unique primitive ID, non-empty owner/authority/hot-path/permissions/budget/docs/test fields, and closed status values. This replaces dozens of category/phase-specific registry substring loops without adding a second primitive registry.
    - **API matrix:** nine generic `clay_js_api_inventory` validators enumerate all 105 `[[api]]` rows and compare the exact registry-public ID set against `ClayJsApiRegistry::from_generated()` and the bounded `docs/index.md` source list. They validate required fields/status/visibility, Markdown/generated identity and implementation metadata, key bindings/permissions/custom-property names, required reference sections and TypeScript usage, facade exports, naming/raw-op boundaries, runtime-backed source paths, denied-authority fields, relative paths, and read-only behavior. T10's single built-in command table remains the authoritative command matrix; security-sensitive routing is still independently tested rather than inferred from docs.
    - **Security retained independently:** in the three reviewed suites, exact prose sensitivity is limited to 17 markers across package trust/adoption/provenance/root confinement, language-server no-shell/root/trusted-subprocess containment, inert rendering/raw-op/no-client-JS boundaries, and package-author hot-path/native-widget boundaries. Public API permission lists and all nine denied authorities remain structured generated-registry fields checked for every public entry. Plan 061's executable 67-op/21-facade/11-package source-to-plan inventory gate and the expiring RustSec exception gate remain intact by name for CI. Runtime/package/domain/filesystem/hot-path behavior stays covered by executable suites, not documentation assertions.
    - **Actionable failure/prose-rewrite proof:** synthetic tests prove missing fields and security markers report exact stable IDs/paths. Adversarial execution temporarily removed `clay.editor.serverInsertText.security_notes` (failed with that exact ID/field), removed one master-index API link (failed the exact inventory/index set comparison), then restored both. Rewriting an ordinary `serverInsertText` description paragraph passed all nine generic API validators plus generated-registry freshness, proving prose is not an accidental schema. All temporary mutations were restored.
    - **Real drift exposed/fixed:** exact generic comparisons found two pre-existing metadata defects hidden by selected phrase tests: `clay.language-server.authorizeLanguageServer.backing_rust` contained prose instead of source symbols, and Git refresh/theme custom properties used unsupported inline frontmatter so generated lookup silently omitted them. Inventory/Markdown now name `PackageService::authorize_language_server` + its op, Git `workspaceRootId`, and theme `specifier` as parser-supported structured objects; generated registry was refreshed and a second update run produced the same SHA-256.
    - **Wiki/maintenance:** updated `clay-js-doc-registry`, `maintenance-validation`, `package-loading`, affected primitive-review/package pages, and master wiki descriptions. Stale references/commands for deleted phase-specific tests now point to generic suites; historical completed plan evidence remains unchanged. `docs/index.md` links the machine-readable validation contract.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1,561 passed / 0 failed / 1 ignored), `cargo audit` (only three documented allowed warnings), all 120 protocol tests, 34 existing registry parser/lookup tests, generated-registry freshness/idempotence, mutation checks, and `git diff --check` pass. No dependency, production runtime path, public API shape, permission, configuration option, or test harness binary was added.
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

- [x] Correct ignore matching and bound sandbox framing
  - Completion Evidence (2026-07-22):
    - **P2-4 truthful bounded ignore grammar:** Cargo metadata confirmed Clay has no directly reusable glob/ignore matcher, so no dependency was added. `src/server/workspace.rs` now compiles only root-level filename-component rules: literal Unicode scalars, `?` for one scalar, backtracking `*` for zero or more scalars, and one optional trailing `/` for directory-only matching; blank lines and column-zero comments are skipped. The standard greedy-star fallback fixes the reviewed `*ab` vs `aab` false negative. Entry metadata is read before matching, so `build/` ignores a directory named `build` but not a regular file with that name. Rules apply independently to visited filename components and never receive path separators.
    - **Unsupported syntax fails visibly:** negation, escaping, character classes, `/` path/anchoring semantics, `**`, control characters, empty directory rules, invalid UTF-8, non-regular/unreadable files, and malformed input abort the listing with one bounded `WorkspaceDiagnostic`, an empty entry set, and `truncated = true`; they are not silently skipped into a broader traversal. A missing `.gitignore` still means compiled defaults only. Cancellation is rechecked after the auxiliary read, preserving T8's blocked-read cancellation behavior.
    - **Ignore resource ceilings:** the existing single-handle read retains at most `MAX_AUXILIARY_READ_BYTES + 1` (1 MiB + 1). New compiled non-configurable budgets cap parsing at 4,096 lines, 1,024 retained rules, and 256 Unicode scalars per rule; listing remains capped at 1,000 entries/depth 8/child scan 100 and runs under T8's four-permit blocking semaphore. Pattern work is therefore bounded and remains outside workspace locks and editor hot paths.
    - **P2-6 bounded sandbox framing:** `RuntimeSandboxSupervisor::read_frame` no longer calls unbounded `read_line(String)`. A small `read_bounded_frame` loop uses Tokio `AsyncBufRead::fill_buf`/`consume`, retains at most negotiated `max_payload_bytes + 1`, excludes the newline delimiter from the payload budget, accepts an exact-max payload, and rejects as soon as overflow is observable. Overflow, EOF before a delimiter, read failure, and malformed JSON kill and `wait` the child before returning; timeout behavior remains unchanged. No protocol/API/dependency/configuration shape changed—the evidence-only harness remains newline JSON, while production migration still requires the documented typed length-prefixed protocol.
    - **Adversarial tests:** three workspace tests cover `*` backtracking, `?`, Unicode, directory-only behavior, every unsupported syntax family, byte/line/rule/rule-length ceilings, and unsupported/oversized `.gitignore` pages proving hidden files are not exposed. The Linux sandbox harness creates hostile child scripts for newline-terminated and unterminated `max + 1` output; both reject immediately below the request timeout with exact `PayloadTooLarge { len: max + 1 }`, and `/proc/<pid>` absence proves kill/reap. Existing valid oversized JSON, timeout/restart, controlled evaluation, and ambient-authority-denial tests remain green.
    - **Documentation:** updated `workspace-file-browser`, `server-file-workspace`, `persistent-runtime-hardening`, and `persistent-runtime-sandbox.md` with exact grammar, ceilings, diagnostic behavior, bounded framing, reaping, source/test paths, and the harness-vs-production protocol boundary. Wiki pages remain master-indexed; no public Clay JS reference change was needed.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1,564 passed / 0 failed / 1 ignored), `cargo audit` (only three documented allowed warnings), 65 workspace tests, 3 lock/cancellation listing tests, 5 sandbox harness tests, 19 performance protocol tests, generic documentation/index checks, generated-registry freshness, and `git diff --check` pass.
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

- [x] Limit native dialogs and validate clipboard backend simplification
  - Completion Evidence (2026-07-22):
    - **P2-5 Linux dialog admission:** `Driver` now owns one monotonic `dialog_generation` plus explicit `file_dialog_in_flight: Option<u64>` and `folder_dialog_in_flight: Option<u64>`. The Linux portal arm reserves the matching generation before starting `clay-native-dialog`; repeated same-kind commands return before creating another thread/request, while one file and one folder picker remain independent. No dialog manager, executor, custom component, animation, or new thread pool was added.
    - **Completion/lifecycle correctness:** background workers always return `EditorAction::FileDialogCompleted` / `FolderDialogCompleted { generation, result }`, including selected, cancelled, unsupported, and failed outcomes. The GUI clears and applies only an exact matching generation; stale completions cannot clear a newer request or reach selected-path capability handling. Thread-spawn failure clears immediately. `ExitRequested` and `on_close_requested` clear both states; `EventLoopProxy::send_event` can fail only after event-loop shutdown, when `Driver` and its remaining state are dropped. Existing server-issued single-use selected-path capability and canonical file/root validation are unchanged.
    - **Interaction/UI boundary:** ran `npx ui-skills start`, applied the narrow `wshobson/interaction-design` non-blocking/interruptible feedback guidance, and reviewed the Clay UI catalog/tokens. Duplicate commands are deliberately ignored (no new visual surface); no component, token, layout, focus, accessibility, package UI, or authoring contract changed, so the catalog files correctly remain unchanged.
    - **P3-1/D7 backend decision—retain with evidence:** Context7 `/websites/rs_arboard`, exact local rustdoc/source for `arboard 3.6.1`, `masonry_winit 0.4.0`, and transitive `copypasta 0.10.2`, plus Cargo inverse trees, show Masonry's clipboard context is private. Widgets can emit writes and masonry_winit intercepts native paste, but `DriverCtx` exposes no clipboard read for the bindable `clientPasteClipboard` fallback. On Linux copypasta's public `ClipboardContext` aliases X11; its Wayland constructor requires Masonry's raw display pointer. A one-backend replacement therefore lacks pure-Wayland/API parity and would require unsafe event-loop coupling. `arboard` stays direct pending upstream access plus Linux pure-Wayland, X11, macOS, and Windows host parity.
    - **Small validated simplification and Linux fix:** `arboard` now uses `default-features = false` because Clay is text-only, deleting 11 image-codec packages (`image`, TIFF/JPEG helpers, etc.) and reducing the resolved lock graph from 582 to 571 packages without changing clipboard APIs. Live validation exposed that constructing/dropping one arboard provider per operation loses X11 ownership when no clipboard manager captures it. `SystemClipboard` now keeps one GUI-thread-lifetime backend in standard `thread_local`/`RefCell` storage (and drops it with the UI thread so backend shutdown/handoff still runs); explicit copy/cut/paste reuse it, with no polling, background reads, or ordinary typing/paint/layout/scroll work.
    - **Platform validation:** on this GNOME Wayland + XWayland host (`WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:0`), the ignored live UTF-8 set/get/restore smoke passes both with the normal session environment and with `WAYLAND_DISPLAY` removed (explicit X11 path). Pure Wayland without XWayland is explicitly not claimed because Masonry 0.4 itself constructs copypasta's X11 alias. macOS (`NSPasteboard`) and Windows (Win32 clipboard) source/API paths were reviewed but not executed, consistent with Linux-primary policy; host checklists remain mandatory before future deletion.
    - **Tests/documentation:** added `native_dialog_generations_limit_duplicates_and_reject_stale_results` (same-kind admission, independent kinds, completion reset, stale rejection, shutdown clear) and ignored `live_system_clipboard_round_trip`. Updated `client-file-dialog`, `masonry-editor`, Phase 20/workflow primitive wiki pages, `launch-and-gui-smoke.md`, `file-open-save-reload-workflow.md`, and `windows.md` with lifecycle, backend rationale, limits, authority, source/test paths, and truthful platform evidence. Existing wiki index already links all changed pages; public Clay JS command shapes and configuration remain unchanged.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1,565 passed / 0 failed / 2 ignored), both live clipboard invocations, 20 manual-smoke documentation tests, 8 generic primitive/wiki tests, generated registry freshness, `cargo audit` (571 crates; only three documented allowed warnings), and `git diff --check` pass.
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

- [x] Run full review closure and performance verification
  - Completion Evidence (2026-07-22):
    - **Closure method:** re-read the original 24-row P0–P3 ledger plus D1–D7, enumerated current tests (`1,567` tests and `65` Criterion self-check benchmarks), ran focused hostile/authority/performance suites first, then deleted all build artifacts and performed a clean final-source all-target build and full Linux gate. No row closes on prose alone where executable behavior exists.
    - **Regression found and fixed during closure:** the 39-test connection suite exposed `disconnect_finalizes_documents_with_no_remaining_holders` intermittently returning `BrokenPipe`. Root cause: asynchronous parse/analysis/typography/result writes used `?` inside the connection loop and could return before `cleanup_connection_documents`, leaving document authority/state dependent on which `select!` branch won after peer close. `handle_connection_with_analysis` is now the single outer lifecycle boundary around `handle_connection_loop`: it normalizes EOF/reset/broken-pipe as ordinary peer disconnect, awaits cleanup on every inner-loop result, then returns non-disconnect codec failures only after cleanup. The regression test passed 20 consecutive runs, the complete 39-test connection suite, and both subsequent all-target runs. Updated `server-ipc-skeleton.md` and `multi-document-sessions.md` with final-close/eviction/output-failure lifecycle.

    | Ledger row | Final closure evidence |
    |---|---|
    | P0-1 | **Resolved by Plan 061 and reverified.** 149 JS-runtime tests (148 pass/1 environment smoke ignored), 6 cross-domain tests, 18 package-graph, 44 package-loading, and 28 package-primitive-gate tests cover trusted/public op/module absence, host-stamped provenance, A-as-B/caller-field rejection, adoption/revocation/stale approval, approved composition/replacement, replay, and root-confined load entries. |
    | P0-2 | **Resolved.** All 39 connection tests pass, including table-driven forged identity across every post-Hello family plus guessed-document/read-only/version/access denial. Connection identity remains handshake-owned. |
    | P0-3 | **Resolved.** Two-client parse isolation, request-owned completion/intelligence replies, bounded subscription routers, unsubscribe/drop cleanup, and the newly centralized every-exit cleanup pass. No live connection drains a shared global result receiver. |
    | P1-1 | **Resolved under enforced exception policy.** `cargo audit` exits 0 across 571 crates; crossbeam/anyhow/memmap advisories are fixed. Only the two documented quick-xml vulnerabilities remain under unexpired 2026-10-22 exceptions, with exact build/runtime paths and expiry test; three documented unmaintained warnings remain. |
    | P1-2 | **Resolved.** All 65 workspace tests pass, including single-handle `max + 1` read growth and exact-size/UTF-8/type boundaries. |
    | P1-3 | **Resolved.** Workspace suite passes exclusive unpredictable temp collision/symlink, `0o600`/permission restoration, same-length edit, target identity replacement, cleanup, and typed stale-save tests. |
    | P1-4 | **Resolved.** Workspace and connection close tests prove 64-per-client/4096-server ceilings, dirty/force policy, shared-holder survival, final-holder registry/coordinator teardown, LRU `CloseDocument`, disconnect cleanup, and queued-output peer-close cleanup. |
    | P1-5 | **Resolved.** Three workspace-op tests include FIFO-backed slow listing: open/save complete while traversal blocks; cancellation/error/unwind remove the token under the four-permit blocking traversal ceiling. |
    | P1-6 | **Resolved by Plans 056–058 and reverified, not rebuilt.** 69 syntax-grammar, 29 parse-coordinator, 23 editor-performance, 20 decoration-transport, 19 performance-protocol, and 16 performance-budget tests pass. `first_party_continuity_edits_keep_one_bounded_parse_and_query` proves one parser call/query/member; large-file tests prove bounded Rope windows, no full-document parser copy, exact edits, cancellation/stale rejection, bounded chunks, and typing independent of parse completion. |
    | P1-7 | **Resolved.** 16 language-server authority plus 4 service tests pass; hung-session isolation, bounded actor ingress/`SessionBusy`, exact identity/grants, revocation, stop, and child reap remain enforced. |
    | P1-8 | **Resolved.** Source scan finds no unbounded channel constructor in `src/`; compiled capacities remain 4096 parse/test lanes and metrics, 64 analysis/result lanes, 32 runtime diagnostics, 8 per-LSP-session commands, request `oneshot`s, and bounded document/session/process stores. Backpressure, diagnostic dedup/drop-count, connection-limit, payload, and queue saturation tests pass. |
    | P1-9 | **Resolved with measured improvement.** Four suite roots still assign all 33 integration sources exactly once; clean Cargo output remains 14 all-target harnesses. Final artifact measurements are below. No routine `target/pi-verify` exists. |
    | P1-10 | **Resolved.** Connection-limit refuse/recover and Unix `0o700`/reject-`0o777`/allow-sticky-`0o1777` endpoint tests pass; Windows/Unix share connection permits. |
    | P2-1 | **Resolved.** Four facade-layout tests, Rust visibility/domain tests, generated inventory, and `node --check` over all 21 authoritative JS files pass; `facades.rs` remains the sole include/access table (21 trusted, exactly 13 public). |
    | P2-2 | **Resolved to demonstrated duplication.** `dispatch_edit_operation` owns Edit/EditorIntent apply/ack/follow-up and `write_document_open_response` owns all three open origins; hot-path/order and origin-parity tests pass. No organization-only split was added. |
    | P2-3 | **Resolved.** 9 API-inventory, 34 registry, 8 primitive/wiki, and 5 package-doc generic validators pass; generated registry is fresh and validators remain read-only. |
    | P2-4 | **Resolved.** Workspace tests pass truthful Unicode `*` backtracking/`?`/directory-only grammar and visible failure for negation, escapes, classes, separators, `**`, malformed/oversized input. |
    | P2-5 | **Resolved.** Native dialog test proves one file plus one folder generation, same-kind duplicate rejection, stale completion rejection, reset, and shutdown clear; Linux portal work remains off the event loop. |
    | P2-6 | **Resolved.** Five sandbox harness tests include terminated/unterminated `max + 1` hostile streams with bounded retention and `/proc` child-reap proof. |
    | P2-7 | **Resolved.** Production constructor guard and invalid-root tests prove both binaries use fallible `IpcServer::try_new`; panic wrapper remains test-only. |
    | P2-8 | **Resolved.** Capped-stderr test proves exactly 64 KiB retained while excess output drains through EOF, preventing pipe backpressure. |
    | P3-1 | **Closed as evidence-based retention.** T13 proved Masonry/copypasta lacks the explicit paste-command read API and pure-Wayland parity needed to remove arboard. Text-only arboard remains with image features removed; live UTF-8 set/get/restore passed under GNOME Wayland+XWayland and explicit X11 during closure. |
    | P3-2 | **Resolved.** All 14 Git tests pass; the multi-root barrier test proves four-permit bounded root concurrency, ordered commands per root, deterministic root-ID association, and shared service ceiling. |
    | P3-3 | **Resolved.** Built-in command table uniqueness/routing, one provider registration-token formatter, facade/inventory/registry identity, and independent security-field tests pass. |

    | Deletion row | Final closure evidence |
    |---|---|
    | D1 | 21 embedded raw facade bodies deleted; authoritative `runtime/js/*.js` bodies compile into `facades.rs`. |
    | D2 | Edit/EditorIntent duplicate orchestration deleted in favor of `dispatch_edit_operation`. |
    | D3 | Direct, selected-file, and SDUI/file-browser opens all use `write_document_open_response`; origin regressions pass. |
    | D4 | 14,368 superseded prose-validator lines and 225 brittle tests removed in T11; structured generic validators retain schema/security coverage. |
    | D5 | 33 integration sources compile through four suite roots; all-target harnesses remain 14 versus the 43-row baseline. |
    | D6 | Production panic constructor callsites deleted; fallible startup guard passes. |
    | D7 | Duplicate clipboard backend remains intentionally retained because deletion failed API/platform parity; unused arboard image dependency subtree was deleted and the remaining limitation is documented/tested. |

    - **Focused closure results:** package/security suites passed 18 package graph + 44 loading + 28 primitive gate + 16 LSP authority + 5 sandbox; runtime authority passed 148/149 JS-runtime + 6 cross-domain; connection/workspace/process passed 39 connection + 65 workspace + 3 listing + 4 LSP service + 14 Git; syntax/performance passed 69 grammar + 29 coordinator + 23 editor invariants + 20 transport + 19 protocol + 16 budget; documentation/API passed 9 inventory + 34 registry + 4 facade + 8 primitive/wiki + 5 package-doc tests. Focused test counts overlap the full inventory by design.
    - **Final clean build/artifact comparison on the same Linux host:** `cargo clean && cargo test --all-targets --no-run` took 67.881 s versus the 89.070 s pre-T9 clean baseline (-23.8%). Final `target/` is 6,557,582,794 bytes versus 21,942,578,072 (-70.1%); `target/debug/deps` 5,073,605,781 versus 19,521,134,614 (-74.0%); incremental 1,017,688,774 versus 1,953,003,235 (-47.9%); Cargo emits 14 harnesses versus 43. Against T9's immediate post-change snapshot, storage improved another 1.3–2.7%; clean wall time was +10.0% (67.881 versus 61.724 s) while an earlier closure clean run was 57.333 s and warm no-op remained 0.233 s versus 0.222 s. Timings are advisory machine snapshots, not deterministic regressions; artifact/harness/work-count gates improved or stayed exact.
    - **Inventory continuity:** current `--list` contains 1,567 tests (1,565 pass + 2 explicit environment-gated ignores) and 65 benchmark self-checks. The difference from T9's 1,783 tests is fully accounted for by T10's four additions, T11's intentional deletion of 225 superseded prose tests, and T12/T13's five additions; `integration_suite_inventory_assigns_every_source_once` passes, so no source module silently disappeared.
    - **Blocking final gate:** final-source `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, post-clean `cargo test --all-targets` (1,565 passed / 0 failed / 2 ignored; 65 benchmark successes), `cargo audit`, generated-registry freshness, wiki-index coverage, all 21 `node --check` facade checks, both live clipboard invocations, and `git diff --check` pass. Linux is closed; macOS/Windows behavior remains accurately documented but non-blocking under project policy.
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

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Completion Evidence (2026-07-22):
    - **Public-surface audit result:** audited all Plan 060/061 additions against the 105-row `api-inventory.toml`, 21 authoritative executable facades, 21 declarations, 86 generated public registry entries, changed bare-public Rust declarations, package-domain allowlists, op registration, and generated lookup. No genuinely new callable is needed: close/unsubscribe, connection subscriptions, package/runtime provenance, worker routing/replay, queue/file/process ceilings, file identity, dialog generations, and scheduler permits are implementation/security mechanics rather than user policy. Existing `loadPackage`, document file APIs, workspace listing, language-server, provider-registration, and UI contribution APIs already cover real public behavior.
    - **Visibility closure:** reduced all 20 newly exposed internal Rust items found by the audit to `pub(crate)`: 12 queue/document/connection/listing/scheduler budget constants; client LRU `enqueue_close_document`; completion/language-intelligence teardown; and five parse subscription/teardown methods. Integration queue saturation now treats the compiled eight-command capacity as deliberately internal rather than importing it as public crate API. Plan 061's public package manifest/approval record value types remain host-library/test data required by public `PackageService` signatures; they do not identify the executing package, grant authority, or appear in JS facades. No changed tracked server item remains newly bare-`pub` after the reduction pass.
    - **Executable visibility guard:** expanded `tests/rust_visibility_api_mapping.rs` from the Plan 061 trust-only check to three independent tests: exact 13-facade public-domain inventory; no bare-public trust-domain, package-context, output-router, lifecycle, queue, filesystem, or scheduler declaration; and no internal Rust identifier/`serverCloseDocument` export in any `runtime/js/*.{js,d.ts}` facade. This prevents later test convenience from turning internal capacities or coordinator handles into accidental library/API surface.
    - **Real contract defect fixed:** `serverSaveDocument({ knownVersion })` advertised a version precondition but `op_clay_documents_save_document` always passed `0`, silently ignoring caller input. The op now parses an optional unsigned `knownVersion` and routes it through the existing `WorkspaceState::authorize_save` choke point. Values `<=` canonical server version remain valid; future values fail with typed `clay.documents.save_failed`/stale-file metadata before disk IO. A new JS-runtime test proves the authoritative facade rejects a future version. Trusted configuration retains server-internal lease authority by design; `clay:documents` is absent from third-party runtime, so packages cannot use that bypass or forge connection identity.
    - **Public behavior documentation refreshed without API proliferation:** updated `serverSaveDocument` for version semantics and exclusive temp/target-identity atomic save; `serverListDirectory` for lock-free bounded traversal, exact component ignore grammar/ceilings, cancellation, and visible malformed-ignore diagnostics; and `loadPackage` for exact bundled trust, mandatory durable third-party adoption, shared-third-party-cohort disclosure, dual-consent relations/replacements, stale/revoked denial, trusted-only package control, root confinement, and domain routing. Updated matching inventory security fields and package facade/declaration comments. No package adoption/authorization JS API was added: keeping approval in host/CLI authority prevents package JavaScript from approving itself.
    - **Documentation as code:** regenerated `docs/generated/clay-js-api-registry.json` twice with identical SHA-256. `docs/index.md` already links every one of the 86 public pages, so no index row changed. Updated `docs/wiki/modules/clay-js-doc-registry.md` with the 105→86 classification, visibility gate, trusted-facade boundaries, save/list/load behavior, and source/test paths; no new wiki page/index entry was needed.
    - **Security/performance closure:** no facade exposes per-keystroke callbacks, queue/concurrency capacities, package-context/domain handles, arbitrary file/process handles, close routing, native paint/layout work, or raw `Deno.core.ops`. `clay:packages`, `clay:documents`, and `clay:workspace` remain trusted-only; third-party package facades remain exactly 13. Public operations continue through existing provenance/grant/root/payload/version/revocation choke points, and this task adds no hot-path work—the only runtime branch parses `knownVersion` during explicit save.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` pass (`1,567` passed / `0` failed / `2` ignored plus `65` benchmark self-check successes). Focused gates pass: 34 registry, 9 generic API inventory, 4 facade-layout, 3 Rust visibility, future-save-version, and LSP queue saturation tests. `cargo audit` passes across 571 crates with only three documented allowed unmaintained warnings; all 21 facade `node --check` runs, registry idempotence, and `git diff --check` pass.
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

- [x] Create or verify Clay configuration APIs
  - Completion Evidence (2026-07-22):
    - **No-new-setting result:** reviewed every Plan 060 behavior and the final Plan 061 two-domain package architecture against `~/.config/clay/init.js`, all 6 `clay:configuration` exports/inventory rows, 86 generated public APIs, package CLI authority, package option/layout schemas, compiled budgets, and hot-path policy. No missing legitimate user choice requires a new setting. Existing APIs already cover package loading, key bindings, theme/typography/syntax-tier choice, exact language-server grants, local module composition, and typed package UI defaults.
    - **Package policy stays host-owned:** third-party adoption, inspection, revocation, replacement approval, and rollback use `clay package adopt|inspect|revoke|rollback`; `loadPackage` only consumes installed, authorized, currently adopted state. Configuration cannot self-approve package JavaScript, mint `PackageContext`, select/promote `RuntimeDomain`, expand relation/replacement scope, transfer first-party authority, disable consent checks, or move third-party code into trusted runtime. The stale Plan 035 single-runtime `RuntimeProfile` configuration narrative is now explicitly marked historical/superseded by Plan 061.
    - **Closed configuration surface verified:** `clay:configuration` remains trusted-only with exactly 3 runtime-backed APIs (`loadConfigurationModule`, `getConfigurationState`, `setPackageOption`) and 3 explicit planned/unavailable stubs (`setModePreference`, `setDecorationTheme`, `setParsePolicy`). Added `configuration_surface_is_closed_and_security_controls_are_not_properties`, which exact-compares facade exports and inventory rows, checks runtime-backed/public status, pins trusted facade classification, and rejects internal control names from `custom_properties`.
    - **Fail-closed property validation:** added `plan060_internal_security_and_performance_controls_are_not_configurable`, exercising the real `ConfigurationRuntime::set_package_option` boundary. Package-prefixed attempts to configure runtime domain/context, IPC identity, connection/document ceilings, queue/result lanes, atomic-save policy/retries, listing/git concurrency, ignore ceilings, LSP actor capacity, sandbox frames, dialog concurrency, clipboard backend, Cargo debug profile, or target directory all return `unsupported package option`; none enters configuration state. This reuses the existing seven-suffix typed package-option allowlist rather than adding a second denylist to production code.
    - **Fixed non-configurable controls:** connection identity/access/lease/version/result routing; document/connection/session/queue/actor/process/frame/payload/time/heap ceilings; atomic temp creation/permission/sync/identity replacement; listing ignore grammar/read/line/pattern/character budgets and worker/git concurrency; domain/provenance/envelope/generation/replay controls; process descriptors/revocation; dialog generations/platform backends/clipboard lifetime; sandbox/IPC/audit/CI/Cargo profile/test-suite/target policies remain compiled host/repository invariants. No environment, JSON/TOML, package option, or hidden facade key was added.
    - **Documentation/wiki:** added the authoritative Plan 060/061 closure to `docs/reference/clay-js-api/configuration.md`, including existing user choices, fixed-control inventory, representative rejected keys, exact facade status, hot-path/security boundary, and test paths. Updated `docs/wiki/modules/configuration-runtime.md` with implementation flow/invariants/tests and corrected stale `loadPackage` status. Existing `docs/index.md` and `docs/wiki/index.md` links already cover these pages; no new page/API/registry row was needed. Registry regeneration is byte-identical.
    - **Security/performance:** configuration remains startup/reload/explicit-setting work; no keypress, paint, layout, scroll, parser, filesystem traversal, IPC, process, or package execution hot path gained a lookup or branch. Configuration cannot weaken filesystem/path/save, network/process, identity/lease/version, package provenance/grant/revocation, payload/resource, raw-op, native-widget, or client-JavaScript boundaries.
    - **Validation:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and the final `cargo test --all-targets` rerun pass (`1,569` passed / `0` failed / `2` ignored plus `65` benchmark self-check successes). Focused closed-surface, Plan 061 pending/adopted/stale config-load, relation/replacement approval, 10 generic API inventory, 34 registry, and wiki-index tests pass. `cargo audit` passes across 571 crates with only 3 allowed unmaintained warnings; all 21 facade syntax checks, byte-identical registry regeneration, and `git diff --check` pass. The first all-target run encountered one transient Linux `ETXTBSY` while spawning the hostile sandbox fixture; its isolated rerun and the complete all-target rerun passed, with no configuration-related failure or code change required.
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

- [x] Update or verify the code wiki after implementation
  - Completion Evidence (2026-07-22):
    - **Final implementation audit:** reviewed all 88 Markdown wiki pages against the completed Plan 060/061 source, protocol, runtime, package, filesystem, concurrency, build, API, configuration, and UI changes. All 87 discoverable pages are linked exactly once from `docs/wiki/index.md`; an independent local-link scan reports zero missing targets.
    - **Current architecture corrected:** `package-principal-and-result-routing-primitive-review.md` now distinguishes its historical gap matrix from the implemented outcome: two domain-specific workers/op/facade sets, host-stamped package context, durable dual-consent approvals, typed bounded cross-domain replay, request-owned completion/intelligence replies, authorized bounded output subscriptions, canonical connection identity, and session-owned LSP actors. Historical Plan 035 unified-authority and Phase 18.21 LSP review pages are explicitly labeled superseded/baseline and link to current authority/runtime/process pages, preventing stale one-runtime/profile/unbounded-channel prose from being treated as security guidance.
    - **Routing/protocol lifecycle documented:** `parse-coordinator.md` now explains bounded dual publication (4096 internal test/tool lanes, 64 per-client `OutputRouter` lanes), access-scoped document subscriptions, non-blocking saturation, and close/disconnect teardown. `protocol-codec.md` now records `CloseDocument`/`DocumentClosed`, handshake-owned post-Hello identity validation, access-holder authority, and close/output cleanup tests. Master-index descriptions for protocol, IPC, workspace, sessions, and principal routing were refreshed.
    - **Package provenance examples fixed:** removed obsolete caller-supplied `packageManifest` from current examples in completion snippets, first-party language package loading, typography mode registration, and language-intelligence registration. The Rust package load-entry excerpt now matches `packages/rust/dist/load.js`: host context selects provenance/grants and registration calls carry only inert contribution data.
    - **Review/build/security closure indexed:** `maintenance-validation.md` now maps each Plan 060 implementation area to its detailed wiki owner and records the measured test-harness/artifact reduction, compiled-policy boundary, audit policy, and executable-evidence rule. Index entries identify historical reviews as historical and current implementation pages as authoritative.
    - **Wiki integrity repair:** fixed three pre-existing broken local links (two `src/perf/budgets.rs` paths from `embedded-js-runtime.md` and the renamed completion-provider review link). No new page was needed; existing focused module/flow pages already cover every changed implementation area with source paths, flow, invariants, security/performance constraints, tests, and related links.
    - **Runtime impact:** Markdown-only task; adds no runtime code, lookup, allocation, dependency, API, setting, hot-path branch, or authority surface.
    - **Validation:** all 8 primitive/wiki, 5 package-doc, 10 API-inventory, 34 registry, and 4 facade-layout tests pass; `wiki_index_links_every_wiki_page` passes; independent scans confirm `88` total pages / `87` indexed discoverable pages / `0` missing local links. Final `fmt`, `check`, `clippy`, audit, facade syntax, registry idempotence, whitespace, and the complete all-target rerun pass (`1,569` passed / `0` failed / `2` ignored plus `65` benchmark self-checks). The first all-target invocation again hit the known transient Linux `ETXTBSY` while spawning the generated hostile sandbox script; its isolated rerun and complete rerun passed. This task changed no runtime/test code to mask that separate test-fixture race.
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

- **Clipboard backend retained:** Clay still uses text-only `arboard` for explicit copy/cut/paste commands while Masonry owns a separate transitive copypasta context for event-loop paste/store. Masonry 0.4 exposes write but no arbitrary clipboard-read API to widgets, and pure-Wayland replacement parity was not available. Image features were removed; GUI-thread lifetime preserves X11 ownership. This is documented, not hidden behind a speculative adapter.
- **Two quick-xml advisories remain under expiring exceptions:** runtime/build dependency paths are blocked on compatible upstream Wayland/accessibility releases. `.cargo/audit.toml` exceptions expire 2026-10-22 and are enforced/documented; they are not treated as permanently resolved dependencies.
- **Legacy post-Hello client IDs remain on protocol v5:** one central pre-dispatch check makes the handshake identity canonical, but removing fields would require a protocol-version migration. Keeping validated vestigial fields avoided unrelated wire churn.
- **Coordinator test receivers remain:** parse/analysis retain bounded global lanes for existing internal/test tooling while production connections use bounded authorized subscriptions. This small dual-publish cost avoided rewriting test infrastructure into a second routing abstraction; both paths are fixed-capacity/non-blocking.
- **Platform closure is Linux-primary:** Windows/macOS behavior and smoke procedures were kept accurate, but native validation was not made blocking from a Linux host, per project policy.
- **Third-party packages share one trust cohort:** the approved two-domain architecture protects trusted Clay/bundled code but intentionally does not provide hostile sibling isolation among adopted third-party packages. Host APIs still enforce provenance, grants, relations, approvals, payloads, generations, and revocation.

## Further Actions

- **P1 — Dependency owner — by 2026-10-22:** track the first compatible `wayland-scanner`/`accesskit_winit`/`zbus_xml` release chain, update dependencies, remove both quick-xml audit exceptions, and rerun full Linux/audit gates. Do not extend expiry without a new documented reachability/upstream review.
- **P2 — Protocol owner — next intentional protocol-version bump:** remove vestigial post-Hello `client_id` fields from message variants and rely solely on connection context; retain forged-ID regression coverage during migration.
- **P2 — Client/platform owner — when Masonry exposes cross-platform clipboard read or upgrades its backend:** re-run native Wayland/X11/macOS/Windows parity and remove `arboard` only if explicit command paste, ownership lifetime, and error behavior remain equivalent.
- **P3 — Package/runtime owner — only if hostile third-party sibling isolation becomes a product requirement:** revisit per-package isolates/processes with measured heap/startup/IPC cost and a new approved decision; do not weaken the two-domain boundary or silently promote packages.
- **P2 — Test infrastructure owner — next maintenance pass:** eliminate intermittent Linux `ETXTBSY` in `sandbox_terminated_and_unterminated_overflow_are_bounded_and_reaped` under the first parallel all-target run. Preserve the max+1 framing/reap assertion; investigate fixture-file launch lifetime/concurrent test interaction rather than adding blind production spawn retries. Isolated and complete reruns pass, but repeated first-run occurrences in tasks 16–17 make this tracked flakiness rather than a product failure.
- **P3 — Build owner — during major dependency/test growth:** repeat clean artifact/harness measurements and split/consolidate suites only when inventory tests and measured link/storage cost justify it; current four-suite topology needs no further abstraction.
