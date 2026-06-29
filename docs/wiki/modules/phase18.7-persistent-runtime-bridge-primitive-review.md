# Phase 18.7 Persistent Runtime and Parse Bridge Primitive Review

## Source

- `roadmap.md` Phase 18.7
- `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
- `decision-logs/2026-06-23-1823-defer-remove-hardcoded-markdown-open-path-to-phase-18-7-persistent-runtime-and-parse-bridge.md`
- `decision-logs/2026-06-16-1526-generic-first-party-package-loadentry-module-bridge.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/package-loading.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/parse-task-lifecycle.md`
- `docs/wiki/modules/embedded-js-runtime.md`
- `docs/wiki/modules/server-file-workspace.md`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
- `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
- `src/server/js_runtime.rs`
- `src/server/parse_coordinator.rs`
- `src/server/ops/parse.rs`
- `src/server/connection.rs`
- `src/server/mod.rs`
- `src/packages/service.rs`
- `packages/markdown/dist/load.js`
- `packages/markdown/dist/parser.js`

## Purpose

Phase 18.7 removes the temporary Markdown open path by building the missing generic path it should have used: a persistent server-side JS runtime plus a constrained JS-backed `ParseHandler` bridge into `ParseCoordinator`.

This review inventories existing primitives, what they already cover, and the smallest generic gaps that remain. It rejects new Markdown-specific Rust logic.

## Existing Generic Primitives to Reuse

### Package loading and package module authority

- Phase 18.6 implemented `clay:packages.loadPackage` for explicit one-line loading from `~/.config/clay/init.js`.
- The resolver currently accepts constrained `@clay/*` specifiers as an implementation limit, validates package metadata through `PackageService`, records resolver-validated `loadEntry` modules in the package load-entry allowlist, and keeps imports confined to recorded package/configuration/facade entries. Plan 035 generalizes this path to source-aware user-authorized packages.
- `PackageService` owns package metadata validation, enable/load checks, prefix/provenance, mode declarations, permissions, and conflict diagnostics.
- `serverLoadPackage` remains a lower-level validation helper; `loadPackage("@clay/markdown")` is the end-user default.

Reuse: open-time activation should depend on the package already loaded by `loadPackage` or invoke the same resolver-backed package load path. Do not add a separate file-open resolver, package copying path, or Markdown-specific package loader.

### Mode and behavior primitives

- `DocumentClassification` and `MajorModeActivation` exist as server-owned package/mode primitives for static open-document metadata and one active major mode per document.
- Behavior manifest selection and publication are already inert server-to-client data. The client runs only Rust-known client-first rules.
- `apply_runtime_outputs` centralizes behavior/SDUI/decorations output application and validation for startup and the current selected-file-open path.

Reuse: generic open-time activation should classify the document, activate the owning mode, install validated behavior output through `apply_runtime_outputs`, and never branch on Markdown in connection handling.

### Parse primitives

- `ParseCoordinator` exists and is generic:
  - `register_handler` accepts a `PackageRecord`, mode ID, and `impl ParseHandler` after checking `PackagePermission::ParseDocument`.
  - `schedule_parse`/`schedule_parse_with_windows` enqueue background work for `(document_id, package_prefix, mode_id)`.
  - Superseded work is aborted per document/package/mode.
  - `next_update` emits validated `IncrementalParseUpdate` values.
  - `validate_update` rejects stale versions, provenance mismatch, decoration version mismatch, invalid ranges, serialization over `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, and invalid decoration payloads.
- `ParseWindowSnapshot`, `ParsePolicy`, and `SyntaxMemoryBudget` provide bounded server-canonical text windows; `SYNTAX_CACHE_BUDGET_BYTES` caps retained parser input.
- `op_clay_parse_register_parse_handler` validates package identity, mode, parse unit, viewport-priority flag, timeout bounds, max-window/guard/memory budgets, and rejects executable public fields (`handler`, `callback`, `onParse`, `function`).

Reuse: do not redesign scheduling, range validation, budget checks, stale-result rejection, viewport priority, or payload validation. The missing piece is only a JS-backed adapter that implements existing `ParseHandler` for a resolver-validated package; Plan 035 later broadens which package sources can become resolver-validated.

### Decoration/render output primitives

- `DecorationRange`/`DecorationSet` are inert renderer/decorator data validated by `src/server/decorations.rs` and bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`.
- Parse-produced decorations can already be carried through `IncrementalParseUpdate` and validated by the coordinator.
- Client rendering consumes validated decorations; no package JavaScript or parser callbacks enter paint/layout/text handlers.

Reuse: Markdown parser output should stay as generic decoration spans with package provenance. No Markdown token or syntax branch belongs in Rust.

### Server document/workspace primitives

- `WorkspaceState` owns file/workspace authority, selected-file grants, open-document registry, canonical paths, duplicate-open reuse, dirty state, and UTF-8/size validation.
- `DocumentState` owns canonical rope text, versions, accepted edits, leases, and bounded parse-window snapshots.
- Ordinary edits never do file IO, JavaScript execution, full-document IPC, or rendering work.

Reuse: open-time activation can read only already-open canonical document text/windows through server-owned document state. It must not grant package filesystem access or re-read/copy package files per open.

### Runtime boundary primitives

- `ClayJsRuntimeService` owns constrained `deno_core` execution, curated `clay:*` facades, timeout diagnostics, raw-op hiding, and sanitized runtime errors.
- `ClayModuleLoader` accepts curated facades, configuration-root relative modules, and resolver-validated package load entries recorded in the allowlist.
- `JS_RUNTIME_EVALUATION_TIMEOUT_MS` and `clay.runtime.timeout` already bound runaway evaluation.
- Current runtime evaluations run on `spawn_blocking`, keeping V8 work away from async protocol tasks and UI paths.

Reuse: the persistent runtime should preserve `ClayModuleLoader`, facades, diagnostics, timeout behavior, and blocking-worker isolation. It should change lifecycle, not authority.

## What Existing Primitives Already Achieve

With the current primitives, Clay can already:

- Validate and load `@clay/markdown` through the generic first-party package resolver.
- Register package metadata, modes, commands, key bindings, SDUI, UI/state/configuration, and permissions with provenance.
- Publish inert behavior manifests and SDUI from server runtime output.
- Validate and publish decoration payloads.
- Represent bounded parse notifications, parse windows, policies, memory budgets, and incremental updates.
- Schedule cancellable parse work and validate results in tests.
- Keep parse/decorations out of client paint/text-event handlers.

## Generic Primitive Gaps

### Gap 1: Persistent server-side runtime lifecycle

Current `ClayJsRuntimeService::evaluate_module_on_runtime` builds and drops a fresh `deno_core::JsRuntime` for each evaluation. Startup configuration and selected-file Markdown open therefore lose any JS closures or handler state after evaluation.

Required generic primitive:

- A server-owned persistent runtime instance, with lifecycle documented as per-server/per-configuration-generation until Phase 19 hot reload adds explicit reload/recreate semantics.
- Runtime work remains on a blocking worker or equivalent serialized owner, not in Masonry or protocol hot paths.
- Existing timeout, diagnostics, facade allowlist, and first-party `loadEntry` allowlist remain in force.

Rejected alternatives:

- Per-document runtime: reintroduces the per-open spawn cost.
- Per-open temp runtime root: the thing Phase 18.7 removes.
- Separate Markdown runtime: mode-specific branch.

### Gap 2: JS-backed `ParseHandler` adapter

`ParseCoordinator::register_handler` can accept a real handler, but production JS registration is metadata-only. `op_clay_parse_register_parse_handler` intentionally rejects executable callback fields.

Required generic primitive:

- A `ParseHandler` implementation backed by the persistent runtime and a server-owned handler token/registry.
- Registration must be allowed only for resolver-validated package `loadEntry` execution with `parse-document` permission.
- Public user config still cannot pass executable `handler`, `callback`, `onParse`, or `function` values to `serverRegisterParseHandler`.
- Handler failures map to `ParseCoordinatorError::HandlerFailed` or typed runtime diagnostics.

Rejected alternatives:

- Accepting callback functions from `~/.config/clay/init.js`: violates existing `reject_executable_handler` boundary.
- Embedding Markdown parser calls in Rust: violates primitive-first mode rules.
- Making `ParseCoordinator` know about JS: unnecessary; the adapter can satisfy existing `ParseHandler`.

### Gap 3: Generic open-time activation orchestration

`src/server/connection.rs` currently checks Markdown file extensions and loaded Markdown commands, then evaluates a copied temp Markdown runtime root. No generic open-time path ties classification, mode activation, persistent runtime handlers, bounded parse windows, and output application together.

Required generic primitive:

- A generic `open_time_activate_document` flow that:
  1. Uses existing document classification/major-mode activation metadata.
  2. Ensures the owning package is loaded through the Phase 18.6 `loadPackage` path.
  3. Ensures the package parse handler is registered on the persistent runtime.
  4. Builds bounded server-canonical `ParseWindowSnapshot` input for initial viewport/open.
  5. Calls `ParseCoordinator::schedule_parse_with_windows`.
  6. Applies behavior/SDUI/decoration outputs through `apply_runtime_outputs` or existing publication primitives.

Rejected alternatives:

- `if is_markdown_path` in Rust.
- A helper named for Markdown, markdown-it, headings, fences, or lists.
- Copying `packages/markdown/dist` into temp dirs.

## Security Boundary

Phase 18.7 must preserve these boundaries:

- Only resolver-validated packages can register live parse handlers; current `@clay/*` package resolution is an implementation limit superseded by Plan 035 source-aware loading.
- `serverRegisterParseHandler` remains a Clay JS facade, not raw `Deno.core.ops` user API.
- User config cannot register arbitrary executable callbacks.
- Packages cannot access filesystem outside already-open document content, network, shell, AI, WASM, raw ops, native widgets, client-side JavaScript, package-manager execution, or package-control authority merely by loading; those capabilities require explicit user approval when implemented.
- Persistent runtime module loading remains confined to curated facades, configuration-root modules, and resolver-recorded package entries.
- Parse windows expose only validated slices of already-open server-canonical document text.
- Runtime diagnostics remain sanitized.

## Performance Boundary

- Open-time activation may run JS asynchronously on the server, but ordinary typing/rendering must not wait on JS.
- Parse scheduling/result publication remains `Background` and cancellable.
- `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` remains protected by client-first behavior manifests.
- `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `SYNTAX_CACHE_BUDGET_BYTES`, and `JS_RUNTIME_EVALUATION_TIMEOUT_MS` remain the relevant guards.
- No full-document IPC for ordinary edits.
- No full-document parse snapshots for large ordinary edits; use bounded windows.

## Planned Generic Implementation Direction

1. Introduce a persistent runtime owner for the server/configuration generation.
2. Keep `ClayModuleLoader` confined to recorded package entries and reuse the package `loadEntry` allowlist.
3. Add a JS-backed `ParseHandler` adapter that stores server-owned handler tokens tied to package prefix/mode and invokes package JS on the persistent runtime.
4. Keep public `serverRegisterParseHandler` callback-free for user config; bridge registration only succeeds under validated package load authority.
5. Add generic open-time activation orchestration and route Markdown through it.
6. Delete the Markdown-specific branch and helpers from `src/server/connection.rs` once equivalent behavior/decorations are produced.
7. Update docs/tests/registry around the changed `clay:parse` contract.

## Test Implications

Minimum tests for later implementation tasks:

- Persistent runtime retains package registration across two document opens.
- Arbitrary user config callback registration remains rejected.
- Non-`@clay/*` and unallowlisted imports remain denied.
- Package without `parse-document` cannot register a live handler.
- Bounded parse windows are the only document text visible to the handler.
- Timeout/looping handler surfaces typed failure, not hang.
- Markdown open produces behavior/decorations through generic path with no temp runtime root.
- Rust source has no Markdown open/parser branch after cleanup.

## Conclusion

No new Markdown-specific primitive is needed. Phase 18.7 needs exactly three generic reusable additions:

1. Persistent server-side JS runtime lifecycle.
2. JS-backed `ParseHandler` adapter for resolver-validated packages.
3. Generic open-time activation orchestration tying mode classification, package load, parse scheduling, and output application together.

Everything else should reuse existing package loading, mode activation, behavior manifest, parse coordinator, parse-window, decoration, workspace/document, runtime diagnostics, and Clay JS API documentation primitives.
