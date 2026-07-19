# Comprehensive Codebase Review

**Review date:** 2026-07-19  
**Scope:** Rust client/editor/server/protocol/package/runtime code, first-party packages, runtime JS facades, tests, dependencies, and build layout.  
**Focus:** Performance, security/data integrity, elegance, and maintainability.  
**Method:** Static flow review of trust boundaries and hot paths, repository/size analysis, dependency graph inspection, lint/test execution, RustSec audit, and an over-engineering/deletion pass.

## Executive summary

Clay has substantially improved since the 2026-06-21 review: package install scripts are disabled by default, package paths are confined, IPC endpoint permissions are restricted, JavaScript evaluation has termination controls, workspace disk I/O is split out of the global lock, saves use temp-file rename, SDUI/protocol payloads are bounded, Clippy is clean, and the full Linux suite passes.

Highest remaining risks sit at identity and lifecycle boundaries:

1. Package-facing ops trust caller-supplied package manifests/permissions instead of an authenticated package principal. This is a blocker before hostile third-party package execution.
2. Post-handshake IPC messages inconsistently trust embedded `client_id` values, and save ignores client identity entirely.
3. Parse, document-analysis, and completion outputs are consumed from shared single-receiver queues by every connection; multiple clients can steal or receive another client's result.
4. File size checks and atomic-save path handling still have time-of-check/time-of-use and temp-file safety gaps.
5. Several process-lifetime collections and queues are unbounded, while open documents and syntax trees lack complete close/eviction lifecycle.

Largest performance issue is already captured by Plan 056: one edit clones whole document text and schedules up to sixteen same-window Tree-sitter parses. Largest developer-productivity issue is build shape: 32 integration-test binaries link a V8/GPU-heavy monolithic crate, and this checkout's ignored `target/` is 118 GiB.

## Priority key

- **P0 — blocker:** Fix before enabling untrusted package execution or claiming multi-client isolation.
- **P1 — high:** Material security, data-integrity, latency, resource-exhaustion, or developer-productivity risk.
- **P2 — medium:** Important correctness/maintainability issue; fix after P0/P1.
- **P3 — low:** Simplification or efficiency improvement with limited immediate risk.

## Verification performed

| Check | Result |
|---|---|
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test --all-targets` | Passed, including integration tests and benchmark harness smoke executions |
| `cargo audit` | Failed: 3 vulnerabilities; 5 unmaintained/unsound warnings |
| `git diff --check` | Passed before report creation |
| Rust source inventory | 129,705 lines total: ~47,376 production lines before inline test modules, ~46,014 inline-test lines, ~35,706 integration-test lines |
| Test/build shape | 32 integration-test binaries; local ignored `target/` measured 118 GiB |

`cargo audit` details:

- `RUSTSEC-2026-0204`: `crossbeam-epoch 0.9.18`; fixed in `>=0.9.20`; dev-only path through Criterion/Rayon.
- `RUSTSEC-2026-0194` and `RUSTSEC-2026-0195`: `quick-xml 0.39.3`; quadratic duplicate-attribute checking and unbounded namespace allocation; fixed in `>=0.41.0`. Current `wayland-scanner ^0.39` dependency constraint blocks a direct lockfile-only update.
- Warnings: unmaintained `bincode 1.3.3`, `paste 1.0.15`, `ttf-parser 0.25.1`; unsound `anyhow 1.0.102`, `memmap2 0.9.10`.

## Prioritized findings

### P0-1: Package permissions and provenance are self-asserted by JavaScript callers

**Category:** Security, authority architecture  
**Confidence:** High  
**Evidence:**

- `src/server/ops/decorations.rs::package_from_options`
- `src/server/ops/{decorations,diagnostics,parse,document_analysis,language_intelligence}.rs`
- `src/server/ops/completion.rs::package_value_from_options`
- `src/server/ops/completion.rs::op_clay_completion_disable`
- `src/server/ops/language_server.rs`

`package_from_options` accepts a caller-provided `packageManifest`, or synthesizes a package record from caller-provided name, prefix, and permission strings. Validation proves that the supplied manifest is internally well-formed; it does not prove that it is the installed/enabled package executing the call. Completion registration follows the same pattern. Completion disable accepts any provider/package prefix without binding the operation to a caller principal.

All package modules share one Deno isolate and the same global Clay facades. Deno ops do not derive identity from the importing module. A malicious loaded package can therefore claim another package's prefix/permissions, construct a more privileged manifest, publish under forged provenance, register providers it did not declare, or disable another package's completion provider. Language-server operations improve this with exact grant/fingerprint checks, but the package name is still caller-supplied rather than bound to an authenticated caller.

Current first-party-only runtime use limits immediate exploitability. This becomes a security blocker the moment untrusted third-party package code can execute.

**Suggested change:**

1. Immediately stop accepting permission lists and arbitrary manifests on publication/registration ops. Require exact match against an installed, enabled, current-generation `PackageRecord` held by `PackageService`.
2. Mint an opaque, generation-scoped `PackagePrincipalId` during package enable/load. Derive package name/version/prefix/permissions/provenance from that principal in Rust.
3. Bind facade calls to an unforgeable capability/closure or execute packages in principal-specific isolates/processes. A string token exposed to all modules is not sufficient unless it is unavailable to sibling packages.
4. Require explicit `package-control` authority for cross-package disable/replacement operations.
5. Add adversarial tests where package A submits package B's manifest/prefix, adds undeclared permissions, publishes decorations, starts an LSP session, and disables B.

### P0-2: Connection identity is not centrally enforced after `Hello`

**Category:** Security, correctness  
**Confidence:** High  
**Evidence:** `src/server/connection.rs:292-722`

The server assigns `client_id` per accepted connection, but many post-handshake messages carry another caller-supplied `client_id`. Enforcement is inconsistent:

- `DecorationViewportRequest`, `SduiAction`, and `CommandIntent` compare embedded ID with connection ID.
- Completion and language-intelligence requests overwrite embedded ID with connection ID.
- `Edit`, `EditorIntent`, `RequestResync`, `OpenDocument`, `OpenSelectedFile`, `AddSelectedWorkspaceRoot`, `ReloadDocument`, `GetDocumentStatus`, and `ListDocuments` trust or forward the embedded value without a central equality check.
- `SaveDocument { client_id: _ }` explicitly discards identity, and workspace save does not verify the requesting client's lease/access.

This creates cross-client spoofing and ownership inconsistencies. Guessing another client/lease ID may permit actions under another session, while save can persist another client's dirty document without proving caller ownership.

**Suggested change:**

1. Remove `client_id` from all post-`Hello` client message variants in the next protocol version; connection context is canonical identity.
2. Until protocol migration, reject every mismatched embedded ID in one pre-dispatch validator, then pass only the connection-owned ID to handlers.
3. Make save/reload/status/list authorization explicit. Save should require current document access/lease and validate `known_version`; never ignore either field.
4. Add two-client tests for every message family, including forged IDs, guessed document IDs, read-only save, stale known version, and disconnect/reconnect.

### P0-3: Shared coordinator receivers misroute results across connections

**Category:** Security, correctness, concurrency  
**Confidence:** High  
**Evidence:**

- `src/server/connection.rs:180-275`
- `src/server/parse_coordinator.rs::{updates_rx,diagnostics_rx,next_update,next_diagnostic}`
- `src/server/document_analysis.rs::next_output`
- `src/server/completion.rs::{results_rx,next_result}`

Every connection loop competes to drain the same parse and document-analysis receivers. A source comment acknowledges that one connection drains the shared parse channel because desktop use is assumed single-client. Completion requests also spawn per-connection tasks that race on one global `next_result()` receiver.

With two clients, the wrong connection can receive another document's decorations, diagnostics, or completion result; the intended client can miss it. Result structs contain document/client metadata in some paths, but routing happens before a connection-specific ownership/subscription check. This is both an information leak and an immediate correctness bug in the advertised multi-client/lease model.

**Suggested change:**

1. Give request-scoped work a `oneshot` reply, as language intelligence already does, or route all coordinator outputs through one server-owned dispatcher keyed by `client_id`, document subscription, request ID, and generation.
2. Use bounded per-connection channels. Do not let connection loops drain global work queues directly.
3. Broadcast document-level syntax/diagnostic updates only to clients currently subscribed/authorized for that document.
4. Add deterministic two-client tests proving no stolen, duplicated, or leaked result under concurrent parse/completion/analysis work.

### P1-1: Current dependency graph fails RustSec audit

**Category:** Security, supply chain  
**Confidence:** High  
**Evidence:** `Cargo.lock`, `cargo audit` output recorded above.

Two `quick-xml` vulnerabilities are rated high (7.5). The crate reaches Clay through Wayland scanner/runtime UI dependencies and `zbus_xml`. `crossbeam-epoch` is dev-only but still makes CI/test tooling vulnerable. Unsound/unmaintained warnings are transitive through Deno, Masonry/Winit, and V8.

**Suggested change:**

1. Update `crossbeam-epoch` to `0.9.20` immediately; a lockfile-only update is currently resolvable.
2. Track/upgrade upstream Winit/Masonry/Wayland and zbus chains that permit `quick-xml >=0.41`. Do not force an incompatible duplicate solely to silence the audit.
3. Assess whether Clay invokes vulnerable runtime `quick-xml` paths on untrusted D-Bus/introspection data; document temporary exposure if upstream blocks remediation.
4. Add `cargo audit` to Linux CI with explicit, expiring ignores only for proven non-reachable advisories.
5. Track the five warnings as upstream upgrade blockers rather than silently allowing them forever.

### P1-2: Open/reload size gates can be bypassed by filesystem races

**Category:** Security, performance, data integrity  
**Confidence:** High  
**Evidence:**

- `src/server/workspace.rs::{check_openable_size,open_io,reload_io}`
- `src/server/workspace.rs::canonical_selected_file`

Clay checks metadata size before `tokio_fs::read`, then later reads the entire path into a `Vec`. A local process can grow or replace the file between check and read. The documented memory-exhaustion guard therefore does not guarantee the allocation ceiling. Path-based metadata/canonicalization also leaves symlink/file-replacement races between authorization and I/O.

**Suggested change:**

1. Open a file handle first, verify handle metadata/type, then read through `take(MAX_OPENABLE_FILE_BYTES + 1)` into a bounded buffer and reject the extra byte.
2. Reuse the opened handle rather than reopening by path after validation.
3. Apply the same bounded read to reload, package/config source reads where applicable, and `.gitignore` loading.
4. Add race-oriented tests using a replace/grow hook between metadata and read; assert bounded allocation and rejection.

### P1-3: Atomic-save temp handling is predictable, follows symlinks, and treats permission restoration as best effort

**Category:** Security, data integrity  
**Confidence:** High  
**Evidence:** `src/server/workspace.rs::{atomic_temp_path,atomic_write_file,prepare_save}`

The temp name is deterministic from PID plus a process-local counter. `tokio_fs::File::create` follows an existing symlink and truncates an existing file. In a writable workspace directory, a pre-created temp symlink can redirect the write. If restoring original permissions fails, Clay still renames the temp over the target; a previously private file can therefore be replaced with default/umask permissions. Stale metadata is checked during `prepare_save`, but an external edit after that check and before rename can still be overwritten.

**Suggested change:**

1. Create temp files with `create_new(true)` and unpredictable names; retry bounded collisions. On Unix use mode `0o600` at creation and reject symlinks/existing paths.
2. Treat required permission-copy failure as save failure; clean up temp and leave target unchanged.
3. Revalidate target identity/metadata immediately before rename and include stable file identity (Unix dev/inode, Windows file ID where available), not only length/mtime.
4. Add adversarial tests for pre-created temp symlink/file, permission-copy failure, target replacement, and same-length external edits.

### P1-4: Server open-document and related per-document state have no complete lifecycle ceiling

**Category:** Security, performance, lifecycle correctness  
**Confidence:** High  
**Evidence:**

- `src/server/workspace.rs::WorkspaceState::documents`
- `src/server/syntax.rs::TreeSitterSyntaxHandler::trees`
- `src/server/{parse_coordinator,completion,language_intelligence}.rs::current_versions/current_generations`
- Absence of a protocol `CloseDocument` message
- `CLIENT_DOCUMENT_SESSION_MAX = 64` applies only to client retention.

Each file is individually capped, but a connection can open an unbounded number of files under a workspace root. Workspace documents, Rope text, syntax trees, coordinator version maps, and some provider state can remain for process lifetime. Client LRU eviction does not notify server to release corresponding state.

**Suggested change:**

1. Add `CloseDocument`/unsubscribe semantics and send them on client session eviction/close.
2. Enforce a server-wide and per-client open-document ceiling aligned with the existing 64-document snapshot/session budget.
3. On final close, cancel document work and remove syntax trees, decoration caches, coordinator versions/generations, analysis routes, and leases.
4. Add churn tests that open/evict thousands of document IDs and assert bounded retained bytes/map lengths.

### P1-5: Directory listing performs blocking filesystem traversal under the workspace mutex

**Category:** Performance, concurrency  
**Confidence:** High  
**Evidence:**

- `src/server/ops/workspace.rs::op_clay_workspace_list_directory`
- `src/server/workspace.rs::{list_directory,list_directory_recursive,read_root_gitignore_patterns}`

The async op acquires the Tokio workspace mutex and then performs synchronous recursive `std::fs` traversal while holding it. Slow disks/network mounts block the persistent runtime thread and serialize unrelated workspace operations. Cancellation is cooperative, but the same runtime thread is blocked in synchronous traversal, making a package's cancellation request unable to run promptly. Root `.gitignore` is read without a byte cap.

**Suggested change:**

1. Under the lock, validate/snapshot root authority and request metadata only.
2. Drop the lock and run traversal via `spawn_blocking`, returning a bounded page.
3. Use an RAII token guard so cancellation entries are removed on all exits/panics.
4. Cap `.gitignore` bytes/lines/patterns and check cancellation during slow metadata work.
5. Test cancellation latency and concurrent save/open while a deliberately slow listing runs.

### P1-6: Syntax refresh performs O(document size) cloning and up to sixteen duplicate parses per edit

**Category:** Performance  
**Confidence:** High  
**Evidence:**

- `src/server/connection.rs::{refresh_native_syntax_after_edit,schedule_parse_window}`
- `src/server/document.rs::text`
- `src/server/syntax.rs::TreeSitterSyntaxHandler::parse_sync`
- `plans/056-Low-Latency-Incremental-Syntax-Decoration.md`

After each accepted edit, Clay clones the whole Rope into a `String`, creates a bounded 4 KiB snapshot, splits it into 256-byte viewports, and schedules one handler task per chunk. Each task parses the same 4 KiB window through a shared parser mutex. Cached-tree reuse uses a full-window `InputEdit` and requires unchanged `window_start`.

This explains the visible base-color-to-syntax-color lag and makes typing cost scale with document size despite bounded parse windows.

**Suggested change:** Execute Plan 056. Key non-negotiable outcomes are bounded Rope slicing without full-document clone, one parse per document/version/window, exact `InputEdit`, stable windows, changed-range queries, bounded output fan-out after parsing, and provisional client span interpolation.

### P1-7: Language-server router serializes all sessions behind one blocking read/write loop

**Category:** Performance, concurrency, maintainability  
**Confidence:** High  
**Evidence:** `src/server/language_server.rs::{router_loop,handle_read,handle_write}`

One current-thread runtime owns all sessions and processes one `SessionCommand` at a time. `handle_read` waits on one child's stdout until data or timeout; during that interval no other session can start, read, write, stop, revoke, or shut down. A slow server can head-of-line block every language and document. `handle_write` can similarly wait on a backed-up child pipe.

**Suggested change:**

1. Give each session its own async actor/task owning child stdio and a bounded command queue.
2. Keep only session table/start/revoke routing centralized; never await child I/O in the table router.
3. Preserve exact package/contribution/fingerprint validation at actor ingress.
4. Add tests where one server's read hangs while another responds/stops within its own deadline.

### P1-8: Process-lifetime queues and diagnostic/metric stores are unbounded

**Category:** Security, performance, observability  
**Confidence:** High  
**Evidence:**

- `src/server/parse_coordinator.rs`: unbounded updates and diagnostics channels.
- `src/server/completion.rs`: unbounded result channel.
- `src/server/connection.rs`: per-connection unbounded completion/language-intelligence channels.
- `src/server/mod.rs`: `runtime_diagnostics: Vec<RuntimeDiagnostic>` and generation diagnostics.
- `src/perf/metrics.rs`: enabled recorder stores every `MetricSnapshot` forever.

Backpressure is absent at several producer/consumer boundaries. Repeated failed hot reloads grow diagnostics and enlarge every welcome snapshot; enough diagnostics can eventually exceed the IPC frame ceiling and prevent new clients from connecting. Long profiling sessions retain every metric event. Parse/provider bursts can allocate behind a slow/disconnected client.

**Suggested change:**

1. Replace unbounded channels with capacity derived from existing event/payload budgets; latest-version syntax/status should coalesce rather than queue indefinitely.
2. Store runtime diagnostics in a bounded `VecDeque`, deduplicate repeated codes/messages, and cap snapshot payload before publication.
3. Aggregate metrics by name/metadata or use a bounded ring plus streaming sink; expose dropped-event counts.
4. Add saturation tests proving producer behavior remains non-blocking where required and memory remains bounded.

### P1-9: Build/test layout causes extreme disk and link overhead

**Category:** Developer performance, maintainability  
**Confidence:** High  
**Evidence:**

- 32 top-level integration-test files, each a separate Rust test binary.
- Clay's one crate links Deno/V8, Masonry/Vello/WGPU, Tree-sitter grammars, and platform UI into many test binaries.
- This checkout's `target/` measured 118 GiB: 59 GiB in `target/debug/deps`, 18 GiB incremental, and another 37 GiB in `target/pi-verify`.
- Largest individual debug test binaries exceed 500 MiB.

**Suggested change:**

1. Add `[profile.dev]`/`[profile.test]` with `debug = "line-tables-only"` or `debug = 1`; measure debugger needs before using `debug = 0`.
2. Consolidate related integration files under fewer test harness roots (`tests/security.rs`, `tests/runtime.rs`, etc. with modules), reducing repeated links without changing production architecture.
3. Reuse one target directory for normal verification; reserve alternate `CARGO_TARGET_DIR` only for genuine lock conflicts and clean it automatically.
4. Document `cargo clean`/target retention guidance and record target-size deltas in CI.
5. Consider crate splitting only after the cheaper profile/harness changes are measured; do not create speculative abstraction crates solely for organization.

### P1-10: IPC endpoint checks omit parent-directory write-permission and connection-count ceilings

**Category:** Security, resource management  
**Confidence:** Medium-high  
**Evidence:** `src/server/mod.rs::{validate_socket_path,validate_parent_directory_ownership,accept_unix_loop}`

Unix bind validates parent ownership and sets the socket to `0o600`, which is good. A custom parent owned by the user but group/world writable is still accepted; another user with directory write authority can unlink/replace the socket despite socket mode. The accept loop has no active-connection ceiling, so a same-user process can allocate unbounded connection tasks and per-connection channels.

**Suggested change:** Reject group/world-writable endpoint parents unless a narrowly justified sticky-directory policy exists, and guard accepts with a semaphore/connection cap. Keep the default `$XDG_RUNTIME_DIR` path.

## P2 findings

### P2-1: Runtime JS facades have two manually maintained implementations

**Category:** Maintainability, correctness  
**Confidence:** High  
**Evidence:**

- `runtime/js/*.ts`
- 21 `CLAY_FACADE_*` raw strings in `src/server/js_runtime.rs:67-628`
- Recent missing `clientOpenFileDialog` embedded export caused complete `init.js` load failure.

Tests compare selected exports, but every API change still requires editing two implementations plus inventory/docs/allowlists. The recent production failure demonstrates that tests are not a substitute for one source of truth.

**Suggested change:** Store executable facades as checked-in `runtime/js/*.js` and `include_str!` them from Rust; keep types in paired `.d.ts`/TypeScript declarations. Generate both from the Markdown/API inventory only if generation remains simple and checked in. Delete embedded raw-string copies.

### P2-2: Several core modules are too large and duplicate orchestration logic

**Category:** Elegance, maintainability  
**Confidence:** High  
**Evidence:**

- Production code before test modules: `packages/record.rs` ~4,506 lines, `editor/surface.rs` ~2,427, `server/workspace.rs` ~2,075, `server/ui.rs` ~1,738, `packages/modes.rs` ~1,624, `main.rs` ~1,524, `masonry_editor.rs` ~1,414, `server/syntax.rs` ~1,342, `server/document_analysis.rs` ~1,299.
- `ClientMessage::Edit` and `EditorIntent` duplicate acknowledgement, analysis, completion, and syntax follow-up logic in `server/connection.rs`.
- Workspace and selected-file open branches duplicate mode activation/analysis follow-ups.

Large files are not inherently wrong, but these mix validation, state, I/O, transport, orchestration, tests, and diagnostics. Small fixes require broad context and have already produced regressions such as behavior-manifest replacement and facade drift.

**Suggested change:** Split by existing responsibility, not new interfaces: contribution-family validators from `packages/record.rs`; open/save/listing from workspace; shared `apply_edit_and_schedule_followups`; shared `finish_document_open`; client status/session/recovery handlers. Avoid factories/traits where plain functions/modules suffice.

### P2-3: Documentation tests rely on thousands of brittle prose needles

**Category:** Maintainability, test quality  
**Confidence:** High  
**Evidence:**

- `tests/primitives_docs.rs`: 6,993 lines and roughly 1,207 needle/assertion occurrences.
- `tests/clay_js_api_inventory.rs`: 4,341 lines and roughly 662 occurrences.
- `tests/package_loading_docs.rs`: 3,045 lines and roughly 556 occurrences.
- Exact prose edits have repeatedly broken tests despite preserving meaning.

These tests provide valuable documentation coverage but couple prose wording to Rust compilation, duplicate registry facts, and dominate maintenance volume.

**Suggested change:** Move enforceable facts to structured frontmatter/inventory schemas and write generic validators over all entries. Keep a small set of semantic markers for genuinely required security statements. Generate command/API matrices from the source-of-truth registry instead of asserting every phrase individually. Delete phase-specific needles once replaced by stable generic invariants.

### P2-4: Hand-written `.gitignore` glob matching is incorrect and can defeat listing exclusions

**Category:** Correctness, performance  
**Confidence:** High  
**Evidence:** `src/server/workspace.rs::{build_ignore_set,gitignore_pattern_matches,glob_matches}`

The matcher does not backtrack after `*`: pattern `*ab` incorrectly fails against `aab` because it commits to the first `a`. `build_ignore_set` classifies `[` patterns as globs, but `glob_matches` implements no character classes. Negation, escaping, and path semantics are also not implemented.

Incorrect ignores can traverse directories users expected excluded, increasing listing cost and exposing names in the file browser.

**Suggested change:** Either implement and test the explicitly documented minimal `*`/`?` matcher correctly while rejecting unsupported syntax, or use an already-present proven ignore/glob implementation if dependency cost is truly zero. Do not claim partial `.gitignore` compatibility silently. Cap pattern input as noted in P1-5.

### P2-5: Linux native-dialog commands can spawn unlimited simultaneous dialog threads

**Category:** Performance, UX, lifecycle  
**Confidence:** High  
**Evidence:** `src/main.rs::Driver::spawn_native_dialog_command`

Every matching action starts a detached OS thread. Repeated key presses can create multiple portal requests/dialogs and threads; there is no in-flight flag, cancellation, or result generation check.

**Suggested change:** Keep one in-flight file dialog and one folder dialog at most, ignore/focus duplicate requests, and clear state when the event-loop result returns. No executor or dialog-manager abstraction is needed.

### P2-6: Sandbox frame reading allocates before enforcing its payload limit

**Category:** Security, performance  
**Confidence:** High  
**Evidence:** `src/server/runtime_sandbox.rs::read_frame`

`BufReadExt::read_line` grows an unbounded `String` until newline/EOF, then compares the final byte count with `max_payload_bytes`. A malicious child can emit an arbitrarily long unterminated line and exhaust parent memory before rejection. This supervisor is currently a test/harness surface, not the active package runtime, but its comments describe a future security boundary.

**Suggested change:** Use bounded `fill_buf`/`take(max + 1)` framing and kill the child immediately on overflow. Add an unterminated oversized-stream test before promoting the sandbox to production.

### P2-7: Production startup uses a panic wrapper despite a fallible constructor

**Category:** Code quality, error handling  
**Confidence:** High  
**Evidence:**

- `src/server/mod.rs::IpcServer::{new,try_new}`
- Production calls in `src/main.rs` and `src/bin/clay-server.rs` use `IpcServer::new`.

Invalid configured workspace roots panic with `expect("configured workspace roots must be valid")` instead of returning a typed startup diagnostic.

**Suggested change:** Use `try_new` in binaries and reserve `new` for tests or remove it. Startup input is a trust boundary and should not panic.

### P2-8: LSP stderr capping stops draining the pipe

**Category:** Correctness, process management  
**Confidence:** Medium-high  
**Evidence:** `src/server/language_server.rs::read_capped_stderr`

Once the retained stderr budget is full, the task exits and drops the read pipe. A verbose language server can receive broken pipe/SIGPIPE or alter behavior merely because Clay stopped collecting diagnostics.

**Suggested change:** Retain only the capped prefix but continue reading/discarding until EOF/cancellation. Record truncation separately.

## P3 findings

### P3-1: Clay ships two Linux clipboard backends

**Category:** Dependency/build simplification  
**Confidence:** Medium  
**Evidence:**

- Direct `arboard` usage in `src/client/clipboard.rs`.
- `masonry_winit` already uses transitive `copypasta` and intercepts Ctrl/Cmd+V.

This duplication contributed to divergent paste paths and the recent Ctrl+V bug. Evaluate using the same clipboard backend as Masonry for Clay's write/read fallback, then remove `arboard` if behavior and platform coverage remain equivalent. Do not change merely to reduce a dependency without Wayland/X11/macOS/Windows smoke coverage.

### P3-2: Git status discovery processes workspace roots sequentially

**Category:** Performance  
**Confidence:** High  
**Evidence:** `src/server/git.rs::discover_workspace_statuses`

Each root runs repository-root, branch/detached-head, and status commands before the next root starts. Several slow repositories multiply refresh latency.

**Suggested change:** Run roots concurrently with a small semaphore (for example 2–4), preserving per-root command ordering and current output/time budgets.

### P3-3: API/command registration remains spread across many closed allowlists

**Category:** Maintainability  
**Confidence:** High  
**Evidence:** API additions currently touch Rust routing allowlists, embedded facade strings, TypeScript facades, docs, `api-inventory.toml`, generated registry, and multiple exact-list tests.

Some separation is security-positive, but manually duplicating the same command ID/name/routing policy caused real omissions. Generate checked-in routing/facade/doc registry tables from the authoritative Markdown/inventory where feasible. Keep independent validation of security fields rather than independent copies of identifiers.

## Over-engineering/deletion pass

Ranked simplifications:

1. **shrink:** Replace embedded `CLAY_FACADE_*` implementations with `include_str!` executable facade files; delete duplicated JS bodies. `[src/server/js_runtime.rs, runtime/js/]`
2. **shrink:** Normalize `EditorIntent` into `EditOperation` and call one edit/ack/follow-up function. `[src/server/connection.rs]`
3. **shrink:** Share one document-open follow-up path for workspace, selected-file, and command opens. `[src/server/connection.rs]`
4. **delete:** Remove phase-specific documentation needle assertions after generic schema validators cover the same invariant. `[tests/primitives_docs.rs, tests/clay_js_api_inventory.rs, tests/package_loading_docs.rs]`
5. **shrink:** Consolidate integration tests into fewer harness binaries; same tests, less linking/storage. `[tests/]`
6. **delete:** Remove `IpcServer::new` panic wrapper once production/tests use `try_new` or an explicit test helper. `[src/server/mod.rs]`
7. **shrink:** Use one clipboard backend if cross-platform validation passes. `[Cargo.toml, src/client/clipboard.rs]`

Estimated net after these targeted changes: several thousand test/duplicate facade lines, one direct clipboard dependency, fewer test binaries, and materially smaller build artifacts. Exact line/dependency counts should be measured during implementation rather than promised here.

## Suggested execution order

1. **Package principal boundary:** P0-1.
2. **IPC identity and result routing:** P0-2 and P0-3 together; protocol/multi-client tests must land with them.
3. **Filesystem data integrity:** P1-2 and P1-3.
4. **Resource lifecycle:** P1-4, P1-8, P1-10.
5. **Dependency advisories:** P1-1; crossbeam immediately, upstream UI/XML chain as soon as compatible.
6. **Visible editing performance:** execute Plan 056 / P1-6.
7. **Blocking concurrency:** P1-5 and P1-7.
8. **Build feedback loop:** P1-9.
9. **Single-source/elegance cleanup:** P2-1 through P2-4, then remaining P2/P3 items.

## Strengths to preserve

- IPC frames are length-prefixed, byte-checked, and capped before allocation.
- Workspace disk I/O no longer holds the global workspace lock across heavy read/write operations.
- Client edit queue and document-analysis worker queues are bounded.
- Package installation suppresses lifecycle scripts by default and package module paths are canonicalized/confined.
- JavaScript evaluation has timeout/termination machinery and generation replacement rejects stale output.
- Decorations, diagnostics, completions, language intelligence, SDUI, and runtime snapshots have explicit validation/payload budgets.
- File saves use same-directory temp plus fsync/rename rather than in-place truncation; hardening is needed, but direction is correct.
- Linux formatting, Clippy with denied warnings, and full all-target test gates pass.
- Unsafe platform code has deterministic safety-comment coverage.
- Paint/text-event paths are guarded against package JavaScript, filesystem, process, clipboard re-read, and parser work.

## Review limitations

- Linux was the execution host; Windows/macOS findings are static-review only.
- No live hostile multi-client or filesystem-race exploit was executed; findings follow directly from current dispatch/I/O structure and should receive focused regression tests.
- Benchmark harnesses ran as `cargo test --all-targets` smoke executions, not statistically sampled Criterion benchmark runs.
- Transitive advisory reachability was classified from dependency paths and source use; upstream fixes may require Masonry/Winit/zbus releases rather than direct Clay version changes.
