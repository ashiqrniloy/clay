# Current Implementation Code Review

**Review date:** 2026-06-21  
**Scope:** Rust application/server/client/editor/runtime/package implementation under `src/`, first-party Markdown package under `packages/markdown/`, runtime JS facades under `runtime/js/`, and project configuration.  
**Reviewer:** AI coding assistant

## Executive summary

Clay has a strong direction: Rust owns the trusted client/server/editor core, package contributions are meant to be inert and validated, and several hot paths already have explicit budgets. The highest-risk gaps are not in typing/rendering code; they are in authority boundaries and lifecycle integration.

Do not ship this to untrusted package/configuration inputs yet. The package loader currently records a package `loadEntry` path before enforcing that it stays inside the package root, `pnpm add` can run lifecycle scripts before Clay validates metadata, and the JS runtime has no evaluation timeout or heap budget. Separately, the workspace protocol still sends full document text over a 1 MiB frame and holds the workspace mutex across disk I/O, so large files and concurrent file operations will degrade or fail.

## Verification performed

| Check | Result |
|---|---|
| `cargo fmt --check` | Passed |
| `cargo check --locked` | Passed with 32 warnings, mostly dead code in SDUI/package UI/server SDUI areas |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Failed: 157 errors, including dead code, `result_large_err` 84x, `large_enum_variant`, collapsible `if`, unnecessary casts/conversions |
| `cargo test --locked` | Timed out after 240s in this environment |
| `cargo test --locked --lib` | Timed out after 180s in this environment |
| `npm audit --prefix packages/markdown --omit=dev --audit-level=moderate` | Failed because no package lock exists (`ENOLOCK`) |

## Priority key

- **P0:** security/data-integrity blocker before any untrusted distribution.
- **P1:** high impact; fix before scaling usage or enabling broader package/config workflows.
- **P2:** important maintainability/architecture issue.
- **P3:** cleanup/quality improvement.

---

## P0 findings

### P0-1: Package `loadEntry` path validation does not enforce package-root confinement

**Category:** Security, architecture  
**Evidence:** `src/packages/manifest.rs:94-99`, `src/packages/manifest.rs:296-309`, `src/server/ops/packages.rs:339-361`, `src/server/js_runtime.rs:739-759`

`validate_entry_path()` only rejects empty strings, HTTP(S), and strings containing `Deno.core.ops`. It does not require a relative `./...` path, a `.js` extension, or reject absolute paths / `..` traversal. Later, `op_clay_packages_load_package_by_specifier()` joins `loadEntry` to `package_root`, canonicalizes with `unwrap_or`, and records that path into the loader allowlist without checking `starts_with(canonical_package_root)`. The loader then reads the allowlisted path directly.

**Why this should change:** The comments say the allowlist is the single gate that confines first-party package code to a validated package root. The current validation does not guarantee that invariant. A malformed or compromised package manifest could point `loadEntry` outside its package and have Clay read/execute that file as runtime JS.

**Recommended fix:** Before recording the allowlist entry, require `loadEntry` to be an explicit relative `.js` path, reject absolute paths and `..`, require canonicalization to succeed, and verify `canonical_load_entry.starts_with(canonical_package_root)`. Do not use `unwrap_or` for security-sensitive canonicalization.

---

### P0-2: Package installation can execute third-party lifecycle scripts before Clay validation

**Category:** Security, supply chain  
**Evidence:** `src/packages/manager.rs:149-153`, `src/packages/manager.rs:166-178`, `src/packages/service.rs:140-151`

`PnpmBackend::install()` runs `pnpm add <spec>` with the user environment and default pnpm behavior. Clay validates package metadata only after installation/discovery.

**Why this should change:** npm/pnpm lifecycle scripts (`preinstall`, `install`, `postinstall`, prepare hooks) can execute during installation. That means remote package code can run before Clay's manifest/permission validator ever sees the package. The validation/enable split does not protect install-time execution.

**Recommended fix:** Install with scripts disabled by default (`--ignore-scripts` equivalent), use a locked store, and make script execution an explicit, scary opt-in if ever needed. Also create/validate the store directory before invoking pnpm and avoid passing sensitive environment variables where possible.

---

## P1 findings

### P1-1: `OpenSelectedFile` trusts a raw client-supplied path

**Category:** Security, authorization  
**Evidence:** `src/protocol/mod.rs:488-490`, `src/server/connection.rs:204-210`, `src/server/connection.rs:386-395`, `src/server/workspace.rs:220-245`

The protocol accepts `ClientMessage::OpenSelectedFile { selected_path: String }`. The server canonicalizes and validates regular UTF-8 file content, then creates a single-file grant, but it has no proof that the path came from the native file picker or from a trusted GUI action.

**Why this should change:** If any untrusted local process can reach the IPC endpoint, it can request any server-readable UTF-8 file by sending a path. This bypasses configured workspace roots by design. Same-user local deployments reduce the risk, but the code already models workspace authority, so this path should uphold that boundary too.

**Recommended fix:** Replace raw selected paths with a capability flow: server issues a short-lived request/capability, the trusted UI fulfills it, and the server accepts only that capability. Simpler safe interim: disable `OpenSelectedFile` on IPC and require workspace-root opens until a capability exists.

---

### P1-2: Local IPC endpoints are not explicitly owner-restricted or authenticated

**Category:** Security  
**Evidence:** `src/ipc.rs:112-132`, `src/server/mod.rs:262-307`

Default Unix endpoints use `XDG_RUNTIME_DIR/clay.sock` or temp-dir fallback. Binding validates parent existence and removes stale sockets, but does not set/check owner-only permissions. Windows named pipes are created with default security settings and no application-level authentication.

**Why this should change:** IPC carries edit authority and file-open/save operations. Relying on default OS behavior is fragile across temp directories, service launchers, container mounts, and Windows pipe security defaults.

**Recommended fix:** Enforce owner-only Unix socket permissions and parent ownership, avoid `/tmp` fallback for sensitive operations when possible, and set a Windows pipe security descriptor for the current user. Add a lightweight authenticated hello/session token if the server can ever be contacted outside a strictly private desktop session.

---

### P1-3: JavaScript runtime evaluation has no timeout, cancellation, or heap budget

**Category:** Security, performance  
**Evidence:** `src/server/js_runtime.rs:270-315`, `src/server/js_runtime.rs:542-583`

Configuration/package JS is evaluated in `spawn_blocking`, then a fresh current-thread Tokio runtime runs `load_main_es_module`, `run_event_loop`, and `mod_evaluate`. There is no wall-clock timeout, operation budget, heap limit, or isolate termination path.

**Why this should change:** A user config or package `loadEntry` can hang startup with `while (true) {}` or allocate heavily. `spawn_blocking` avoids blocking the async reactor but does not stop the work; hung evaluations can accumulate and prevent configuration/package loading from completing.

**Recommended fix:** Add a hard evaluation timeout and terminate the V8 isolate on expiry. Add memory/heap limits if supported by the runtime layer. Long term, run package/config JS in a separate process so the OS can enforce CPU/memory and reliable cancellation.

---

### P1-4: Full-document snapshots conflict with the 1 MiB IPC frame limit

**Category:** Performance, architecture  
**Evidence:** `src/protocol/codec.rs:12-13`, `src/protocol/mod.rs:547-550`, `src/protocol/mod.rs:579-588`, `src/server/workspace.rs:206-216`, `src/server/workspace.rs:234-243`, `src/server/connection.rs:380-382`

The codec has a default 1 MiB maximum frame. `InitialDocument`, `ResyncSnapshot`, `DocumentOpened`, and `DocumentReloaded` carry full `text: String`. Workspace open/read paths read entire files into memory, convert to `String`, and then send the full text in one message.

**Why this should change:** Files above roughly 1 MiB will fail to encode/send, despite large-file Markdown plans and budgets elsewhere. Even below the limit, opening/saving clones the full document multiple times and creates latency spikes.

**Recommended fix:** Add an explicit file-size gate now with a clear error before reading/sending oversized files. Then move to chunked document snapshots or viewport-first loading. Keep full snapshots only for small files.

---

### P1-5: Workspace mutex is held across async disk I/O

**Category:** Performance, concurrency, maintainability  
**Evidence:** `src/server/connection.rs:360-364`, `src/server/connection.rs:391-395`, `src/server/connection.rs:421`, `src/server/connection.rs:437-441`, `src/server/workspace.rs:206-216`, `src/server/workspace.rs:342-349`, `src/server/workspace.rs:390-397`

Connection handlers call `workspace.lock().await.open_existing_file(...).await`, `open_selected_file(...).await`, `save_document(...).await`, and `reload_document(...).await`. Those workspace methods perform filesystem reads/writes/metadata calls while the global workspace mutex is still held.

**Why this should change:** One slow disk read/write serializes unrelated document operations for all clients and runtime ops. It also makes lock-ordering bugs harder to reason about because workspace operations lock documents while retaining or reacquiring workspace state.

**Recommended fix:** Split workspace operations into short metadata/state phases and I/O phases. Clone the path/handles under the lock, release the lock for disk I/O, then reacquire to commit state if metadata/version still matches.

---

### P1-6: Save is non-atomic and can corrupt files on crash/interruption

**Category:** Data integrity  
**Evidence:** `src/server/workspace.rs:330-363`

`save_document()` clones the whole document text, checks stale metadata, then calls `tokio_fs::write(&canonical_path, text.as_bytes())` and marks clean if the document version still matches.

**Why this should change:** `write` truncates/replaces the destination in place. If the process crashes, disk fills, or the machine loses power mid-write, the user's file can be left partially written or empty. Editors should treat saving as a data-integrity boundary.

**Recommended fix:** Write to a temp file in the same directory, flush/fsync it, then atomically rename over the original. Preserve permissions where needed. If that is too much for the next increment, at least keep the stale metadata check and add a temporary-file rename for the common path.

---

### P1-7: Runtime SDUI `publishTree` lacks payload/node/depth budgets

**Category:** Security, performance  
**Evidence:** `src/server/ops/sdui.rs:73-105`, `src/server/ops/sdui.rs:210-220`, `src/server/sdui.rs:315-332`, compare `src/server/ui.rs:26` and `src/server/ui.rs:312-320`

Package UI contribution registration enforces payload budgets and `MAX_COMPONENT_NODES`. Runtime `publishTree` parses arbitrary JSON into `serde_json::Value`, recursively builds nodes, and validates only graph shape/editor bindings. There is no pre-parse size check, maximum node count, maximum text length, or recursion/depth bound.

**Why this should change:** A malicious or buggy config/package can allocate a huge tree and consume memory/CPU before validation rejects anything. This undermines the otherwise good SDUI budget discipline.

**Recommended fix:** Apply the same budget style as package UI: reject `tree_json.len()` over a snapshot budget before parsing, cap node count/depth/list items/text length, and return a typed runtime diagnostic.

---

## P2 findings

### P2-1: Package CLI/service state is in-memory only, so separate CLI invocations cannot list/enable prior installs

**Category:** Architecture, maintainability  
**Evidence:** `src/main.rs:655-675`, `src/packages/service.rs:274-285`, `src/packages/service.rs:288-290`

Every `clay package ...` command creates a fresh `PackageService`. `list()` and `inspect()` read only the service's in-memory `installed` map. There is no initial discovery step from the package store.

**Why this should change:** `clay package add foo` can install, but a later `clay package list` starts from an empty service and reports no packages. `enable` similarly cannot operate on packages installed in a previous process unless something repopulates `installed`.

**Recommended fix:** Add a small `refresh_installed()`/constructor path that calls `backend.list_installed()` at CLI startup and populates `installed`. Persist enabled package state separately if enable/disable must survive process restart.

---

### P2-2: Runtime evaluation returns data that the server does not apply consistently

**Category:** Architecture, correctness  
**Evidence:** `src/server/js_runtime.rs:381-389`, `src/server/mod.rs:196-223`, `src/server/connection.rs:500-537`

`ClayRuntimeEvaluation` contains published decorations, parse handler metadata, and package UI contributions. Server startup `apply_runtime_evaluation()` applies only SDUI tree and behavior manifest. Selected Markdown open has a separate special path that sends decorations/tree for that one flow.

**Why this should change:** The runtime boundary looks generic, but application is partial and flow-specific. That makes package loading hard to reason about: a package can register parse/UI/decorations successfully in runtime tests, yet the live server may ignore those outputs unless it is in a hardcoded Markdown selected-file path.

**Recommended fix:** Add one central `apply_runtime_evaluation()` that handles all evaluation outputs or explicitly remove/defer unused fields until wired. Avoid special-casing Markdown inside connection handling once generic package/mode activation exists.

---

### P2-3: Markdown open path is hardcoded and expensive

**Category:** Architecture, performance  
**Evidence:** `src/server/connection.rs:574-609`, `packages/markdown/dist/load.js:109-160`, `packages/markdown/dist/parser.js:424-445`

On selected Markdown open, the connection layer creates a temp runtime root, copies Markdown dist JS files, writes an init module, evaluates a fresh JS runtime, and removes the temp directory.

**Why this should change:** This bypasses the generic package loader/parse coordinator path and adds disk I/O plus JS runtime startup to file open. It will be hard to extend to other modes without copying the same pattern.

**Recommended fix:** Route open-time mode activation and parse decoration publication through the generic package/mode/parse primitives. Keep the Markdown-specific parser logic inside the package, not in `server/connection.rs`.

---

### P2-4: Clippy cannot pass with warnings denied

**Category:** Maintainability, performance hygiene  
**Evidence:** `cargo clippy --all-targets --all-features --locked -- -D warnings` failed with 157 errors.

Main failure groups:

- Dead code in `src/masonry_sdui.rs`, `src/shell/package_ui.rs`, `src/server/sdui.rs`, `src/server/ui.rs`.
- `result_large_err` 84x for package/service/mode diagnostics.
- `large_enum_variant` for `src/masonry_editor.rs:32-36`.
- Minor issues: collapsible `if`, redundant closures, unnecessary casts, useless conversions.

**Why this should change:** A project this dependent on safety boundaries should have a clean lint gate. Otherwise real regressions are hidden inside known-warning noise.

**Recommended fix:** Fix simple lints directly. For intentionally staged/dead primitives, add local `#[expect(..., reason = "...")]` or feature-gate them. For large diagnostics, box large variants or return `Box<Diagnostic>` at cold error boundaries.

---

### P2-5: Large diagnostic error types are copied through many `Result` paths

**Category:** Performance, maintainability  
**Evidence:** `src/packages/record.rs:294-301`, `src/packages/conflict.rs:11-18`, `src/packages/service.rs:47-53`, clippy `result_large_err` 84x

Package diagnostics carry multiple `String` fields and are returned directly as `Err` variants from many functions.

**Why this should change:** This increases `Result` size and stack movement on success paths. These are cold diagnostics; paying heap allocation for errors is usually better than inflating every result.

**Recommended fix:** Use `Box<PackageRecordError>`, smaller codes plus detail structs, or a shared `Arc<str>`/structured diagnostic storage at service boundaries.

---

### P2-6: First-party Markdown dependency state is not reproducible/auditable

**Category:** Supply chain, maintainability  
**Evidence:** `packages/markdown/package.json:11-12`, absence of `package-lock.json`/`pnpm-lock.yaml`, `npm audit` failed with `ENOLOCK`.

The package uses `"markdown-it": "^14.1.0"` and has a checked-in `node_modules` tree, but there is no lockfile to reproduce or audit exact transitive versions.

**Why this should change:** Reviewers cannot tell whether `node_modules` matches `package.json`, and automated audit tooling cannot run. A caret range also permits future installs to resolve different code.

**Recommended fix:** Commit a lockfile or replace checked-in `node_modules` with a generated vendored bundle plus an integrity hash/source script. Pin exact dependency versions for first-party runtime packages.

---

### P2-7: Public Rust API surface is wider than the implementation maturity

**Category:** Architecture, maintainability  
**Evidence:** `src/lib.rs:1-12`

Most modules are exported as `pub mod`, including client, editor, package, perf, protocol, Masonry widgets, and docs.

**Why this should change:** Public modules become semver/API commitments and make internal refactors harder. They also expose implementation details that downstream users may depend on before the API is stable.

**Recommended fix:** Keep only intentional public API modules public. Move internal implementation modules to `pub(crate)` and re-export narrow, documented types/functions from stable facades.

---

## P3 findings

### P3-1: Direct dependency list includes likely unused or over-broad dependencies

**Category:** Build performance, maintainability  
**Evidence:** `Cargo.toml:14`, `Cargo.toml:17-20`; `git grep` found no direct source usage for `pollster`, `taffy`, or `wgpu`, and `vello` is referenced through `masonry::vello` rather than the direct crate path.

**Why this should change:** Extra direct dependencies increase compile time, audit surface, and update burden.

**Recommended fix:** Remove unused direct dependencies unless they are intentionally part of the public API. If Tokio does not need every feature, replace `features = ["full"]` with the minimal feature set actually used.

---

### P3-2: Unsafe Windows COM calls need complete safety comments

**Category:** Maintainability, safety reviewability  
**Evidence:** `src/client/file_dialog.rs:62-112`, `src/client/file_dialog.rs:125-158`

Some unsafe COM calls have comments, but others (`CoCreateInstance`, `dialog.Show`, `GetDisplayName`, `SetFileTypes`, `CoInitializeEx`, `CoUninitialize`) rely on surrounding context.

**Why this should change:** Unsafe blocks are maintenance hotspots. Each should state the exact invariant that makes it safe.

**Recommended fix:** Add short `// SAFETY:` comments immediately before each unsafe block, or group calls into small safe wrappers with one documented invariant.

---

### P3-3: Test/lint runtime is too slow for a tight feedback loop

**Category:** Maintainability  
**Evidence:** `cargo test --locked` timed out after 240s; `cargo test --locked --lib` timed out after 180s in this environment.

**Why this should change:** If the default test command is too slow, contributors stop running it and regressions escape.

**Recommended fix:** Define a fast CI/local command that excludes heavyweight GUI/runtime tests, and keep heavier tests behind explicit names/features. At minimum document the expected test matrix in one place.

---

## Strengths worth keeping

- Protocol frames are length-prefixed and bounded (`src/protocol/codec.rs:12-13`) with rkyv byte-checking on decode.
- Workspace-root opens canonicalize paths and reject paths outside configured roots (`src/server/workspace.rs:499-541`).
- Configuration module loading is strongly confined to relative `.js` files under the config root (`src/server/configuration.rs:481-500`).
- Decoration and parse pipelines have explicit byte budgets and viewport/window validation (`src/server/decorations.rs`, `src/server/parse_coordinator.rs`).
- Client edit queue is bounded (`EDIT_QUEUE_CAPACITY = 256`) and uses `try_send`, avoiding unbounded typing backpressure.
- Package UI contribution validation has useful payload/provenance/action-target checks (`src/server/ui.rs:304-397`).

## Suggested fix order

1. Fix package-root confinement for `entry`/`loadEntry` and disable package install lifecycle scripts.
2. Lock down IPC/file-open authority: owner-only endpoint permissions plus capability-gated selected-file opens.
3. Add JS runtime timeout/termination and runtime SDUI budgets.
4. Add file-size gate/chunked snapshots and stop holding workspace lock across disk I/O.
5. Make saves atomic.
6. Wire or remove unused runtime evaluation outputs; remove hardcoded Markdown open path when generic path is ready.
7. Make clippy clean with warnings denied.
8. Add package lock/integrity and trim public/dependency surface.
