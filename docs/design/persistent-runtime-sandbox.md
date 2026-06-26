# Persistent Runtime Sandbox Design

## Purpose

This document defines the separate-process JavaScript runtime sandbox gate required before Clay can execute any non-`@clay/*` package JavaScript. It is a design gate, not authority approval: third-party package execution remains blocked until an approved decision log grants that authority.

## Boundary

Clay splits runtime authority into a parent supervisor and a child JavaScript process.

- Parent process owns canonical documents, workspace/file authority, package metadata validation, permissions, runtime generation IDs, behavior publication, SDUI state, parse scheduling, diagnostics, package-manager execution, and restart policy.
- Child process owns only V8/`deno_core` evaluation for one runtime generation and receives bounded, validated requests.
- Protocol messages carry inert request/result data only. They never carry Rust internals, raw `Deno.core.ops` names, V8 handles, package JavaScript function references, native widget handles, open file descriptors, workspace roots, package-manager handles, or capability tokens.
- Client code never talks to the sandbox. Clients continue to receive validated behavior manifests, SDUI snapshots/updates, decorations, and protocol messages from the parent.

## Supervisor Lifecycle

1. Parent starts one sandbox child for the active runtime generation.
2. Parent sends a handshake containing protocol version, generation ID, compiled budgets, allowed request kinds, and denied authority markers.
3. Child replies with its protocol version and readiness; any mismatch kills the child and records a sanitized diagnostic.
4. Parent sends configuration/package-load/parse/evaluate requests only after package metadata, permissions, and budgets pass parent-side validation.
5. Parent enforces per-request timeout, payload budgets, and output validation. Timeout or protocol violation kills the child.
6. Parent restarts a fresh child for the next generation or explicit recovery path; old generation parse work is cancelled/ignored through `ParseCoordinator` generation checks.
7. Failed child startup, timeout, heap-limit, crash, or malformed response emits a stable sanitized diagnostic and keeps prior validated client state until replacement succeeds.

## Message Protocol

Initial protocol shape:

```text
RuntimeRequest::Handshake { protocol_version, generation_id, budgets }
RuntimeRequest::EvaluateModule { generation_id, module_specifier, source, package, permissions, document_context }
RuntimeRequest::LoadFirstPartyPackage { generation_id, specifier, load_entry_source, package_metadata }
RuntimeRequest::Parse { generation_id, handler_token, notification, windows, budgets }
RuntimeRequest::Shutdown { generation_id }

RuntimeResponse::Ready { generation_id }
RuntimeResponse::Evaluation { generation_id, inert_outputs }
RuntimeResponse::ParseUpdate { generation_id, inert_json }
RuntimeResponse::Diagnostic { generation_id, code, severity, safe_message }
RuntimeResponse::Exited { generation_id }
```

All frames are length-prefixed and bounded like the existing IPC codec direction. Parent rejects unknown variants, oversized frames, generation mismatches, stale handler tokens, invalid UTF-8/path-like authority payloads, and outputs that fail existing behavior/SDUI/decoration/parse validators.

## Allowed Requests

Allowed initial requests are deliberately small:

- Evaluate a parent-provided controlled module for the current generation.
- Load resolver-validated first-party `@clay/*` package `loadEntry` code.
- Invoke a registered parse handler token with bounded `ParseEditNotification` and parse-window data.
- Shut down the child.

Non-`@clay/*` package execution is not allowed by this design alone. A later approved authority decision must define package trust, registry/integrity policy, permissions, update/rollback behavior, and tests before enabling it.

## Cancellation and Restart

- Parent owns cancellation. Child requests cannot extend their own deadline.
- Per-request timeout kills the child process rather than trusting isolate reuse.
- Heap-limit or child crash marks the generation failed and cancels/ignores in-flight parse work for that generation.
- Restart creates a fresh child with a new generation or explicit recovery generation; package `loadEntry` modules must rerun and handlers must re-register.
- Parent keeps last validated behavior/SDUI/decorations until a fresh generation validates replacement output.

## Diagnostics

Diagnostics crossing the process boundary are sanitized:

- stable Clay error code
- severity
- generic safe message
- generation ID
- optional package prefix/name/version only after parent validation

Diagnostics must not include source text, absolute paths, environment variables, tokens, V8 stack internals, raw package-manager output, raw `Deno.core.ops`, hostnames, credentials, or workspace roots.

## Denied Authorities

Sandboxed JavaScript has no default authority for:

- filesystem outside parent-provided open document data
- network
- shell
- WASM
- AI mutation or tool orchestration
- package-manager execution or lifecycle scripts
- native-widget handles or direct Masonry mutation
- client-side JavaScript
- raw-op / raw `Deno.core.ops` public authority
- remote listeners
- workspace mutation outside declared parent-owned APIs

Any exception requires a later approved decision log, explicit permission, parent-side validation, docs, and tests.

## Performance Targets

Sandbox work is startup/package-load/parse/reload/background work only. It must not run in keypress, paint, layout, scroll, text-event, or edit-ack handlers.

Initial measurable targets for the harness:

- child startup + handshake: recorded in tests/bench notes, target under 250 ms on developer machines
- first-party package load request overhead over in-process runtime: record median; target under 2x before production migration
- parse request round trip with small windows: record median; target under 10 ms added overhead for small visible-window parses
- timeout kill + restart: record elapsed; target under 500 ms for kill acknowledgement and fresh handshake
- reload: no full-document IPC and no client hot-path dependency

These are design targets, not CI thresholds until stable runners exist.

## Migration Path

1. Keep current in-process runtime for first-party packages.
2. Add an internal sandbox supervisor harness that can start, handshake, evaluate a harmless fixture, kill on timeout, restart, and reject oversized output.
3. Route selected first-party smoke requests through the harness behind an internal test feature or test-only entrypoint.
4. Measure startup, package-load, parse, timeout, heap, and reload overhead.
5. Write and approve a third-party runtime authority decision log.
6. Only after approval, add non-`@clay/*` resolver/trust policy and execution path.

## Minimal Harness Status

`src/server/runtime_sandbox.rs` and `src/bin/clay-runtime-sandbox.rs` implement the current internal harness only. It proves child spawn/handshake, controlled evaluation, parent timeout kill, fresh restart, payload-budget rejection, and absence of filesystem/network/shell globals. It uses newline-delimited JSON over child stdio rather than the final production protocol and is not wired into package loading.

## Tests Required Before Implementation Completion

- Design doc guard requires process boundary, bounded protocol, restart policy, parent-side validation, hot-path exclusion, denied authorities, and decision-log gate language.
- `tests/runtime_sandbox_harness.rs` proves child start/evaluate, timeout kill/restart, oversized output rejection, and no filesystem/network/shell authority through the protocol.
- Package-loading tests must continue proving non-`@clay/*` execution is blocked without an approved authority decision.
