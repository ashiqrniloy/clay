# Persistent Runtime Sandbox Design

## Purpose

This document defines the separate-process JavaScript runtime sandbox as a hardening primitive and optional runtime profile. It is not a first-party/third-party dividing line: any Clay-shipped or user-installed package may use a sandboxed, restricted, or native-trust profile when the user grants that profile.

## Boundary

Clay splits runtime authority into a parent supervisor and a child JavaScript process.

- Parent process owns canonical documents, workspace/file authority, package metadata validation, capabilities, runtime generation IDs, behavior publication, SDUI state, parse scheduling, diagnostics, package-manager execution, and restart policy.
- Child process owns only V8/`deno_core` evaluation for one runtime generation and receives bounded, validated requests.
- Protocol messages carry inert request/result data only. They never carry Rust internals, raw `Deno.core.ops` names unless an explicit raw-ops profile is granted, V8 handles, native widget handles, open file descriptors, workspace roots, package-manager handles, or capability tokens.
- Client code never talks to the sandbox. Clients continue to receive validated behavior manifests, SDUI snapshots/updates, decorations, and protocol messages from the parent unless a future `client-runtime` capability explicitly changes that path.

## Supervisor Lifecycle

1. Parent starts one sandbox child for the active runtime generation/profile.
2. Parent sends a handshake containing protocol version, generation ID, compiled budgets, allowed request kinds, and granted capability profile.
3. Child replies with its protocol version and readiness; any mismatch kills the child and records a sanitized diagnostic.
4. Parent sends configuration/package-load/parse/evaluate requests only after package metadata, capabilities, user authorization, and budgets pass parent-side validation.
5. Parent enforces per-request timeout, payload budgets, and output validation. Timeout or protocol violation kills the child.
6. Parent restarts a fresh child for the next generation or explicit recovery path; old generation parse work is cancelled/ignored through `ParseCoordinator` generation checks.
7. Failed child startup, timeout, heap-limit, crash, or malformed response emits a stable sanitized diagnostic and keeps prior validated client state until replacement succeeds.

## Message Protocol

Initial protocol shape:

```text
RuntimeRequest::Handshake { protocol_version, generation_id, budgets, runtime_profile }
RuntimeRequest::EvaluateModule { generation_id, module_specifier, source, package, capabilities, document_context }
RuntimeRequest::LoadPackage { generation_id, specifier, load_entry_source, package_metadata, capabilities }
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

Allowed requests are profile-dependent and parent-built:

- Evaluate a parent-provided controlled module for the current generation.
- Load resolver-validated package `loadEntry` code from any enabled user-authorized package source.
- Invoke a registered parse handler token with bounded `ParseEditNotification` and parse-window data.
- Shut down the child.

## Production Enforcement Contract

The current `RuntimeSandboxSupervisor` newline-delimited JSON harness is evidence only, not production API. Production sandbox routing requires a bounded typed protocol shaped like the main IPC `Codec`: length-prefixed frames, maximum frame size, typed request/response variants, decode validation, generation IDs, stable error codes, and metrics for frame-too-large/protocol-failure cases.

Required request flow:

```text
parent validates package metadata + user-authorized capabilities + runtime profile + budgets
-> child evaluates/load/parse request for one runtime generation
-> parent validates bounded inert outputs
-> parent publishes behavior/SDUI/decorations/folding/completion/parse updates
```

Parent validation is mandatory before every request: source record, manifest validation, user grant match, entry path confinement, payload budget, timeout/heap budget, runtime generation, handler token, document version, and stale-generation rejection. Parent validation is mandatory after every response: allowed response kind, generation match, package provenance match, payload size, inert JSON shape, behavior/SDUI/decorations/folding/completion/parse validators, no executable callbacks unless explicitly supported by the granted API, no ungranted raw op names, no ungranted client JavaScript, and no path-like authority payloads outside granted scopes.

Timeout, heap-limit, malformed response, oversized output, protocol mismatch, unknown variant, stale generation, stale handler token, or invalid output kills the child process and starts a fresh child for a later generation or recovery path. Parent keeps the last validated client state until replacement output validates.

The child receives no workspace roots, absolute source paths, file descriptors, package-manager handles, V8 handles, Rust internals, capability tokens, client handles, native widget handles, environment variables, credentials, registry auth tokens, or direct client authority unless a future explicit capability/API deliberately passes a bounded substitute.

Performance evidence is required before production routing: startup plus handshake target under 250 ms on developer machines, first package load overhead recorded against in-process runtime, small parse request round trip target under 10 ms added overhead, timeout kill plus fresh handshake target under 500 ms, and no keypress, paint, layout, scroll, text-event, or edit-ack dependency.

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
- package prefix/name/version/source when parent validated

Diagnostics must not include source text, environment variables, tokens, V8 stack internals, raw package-manager output, raw `Deno.core.ops`, hostnames, credentials, or workspace roots unless the user explicitly enabled a development/debug profile that documents the leak.

## Runtime Profiles

```text
native-trust | sandboxed | restricted
```

Profiles are user/config choices for any package source. `sandboxed` removes ambient host authority and routes requests through the parent. `restricted` is stricter and may allow only inert outputs. `native-trust` uses the in-process server runtime and relies on user-approved capabilities plus Clay API boundaries. WASM, network, shell, filesystem, client JavaScript, native widget handles, and package-manager handles remain unavailable in sandboxed/restricted profiles unless a documented capability/API passes a bounded substitute.

## Performance Targets

Sandbox work is startup/package-load/parse/reload/background work only. It must not run in keypress, paint, layout, scroll, text-event, or edit-ack handlers.

Initial measurable targets for the harness:

- child startup + handshake: recorded in tests/bench notes, target under 250 ms on developer machines
- package load request overhead over in-process runtime: record median; target under 2x before production migration
- parse request round trip with small windows: record median; target under 10 ms added overhead for small visible-window parses
- timeout kill + restart: record elapsed; target under 500 ms for kill acknowledgement and fresh handshake
- reload: no full-document IPC and no client hot-path dependency

## Migration Path

1. Keep current in-process runtime for bundled packages while source-aware package loading is implemented.
2. Keep internal sandbox supervisor harness that can start, handshake, evaluate a harmless fixture, kill on timeout, restart, and reject oversized output.
3. Add runtime profile selection to package authorization records.
4. Route selected smoke requests through the harness behind tests.
5. Measure startup, package-load, parse, timeout, heap, and reload overhead.
6. Generalize production routing for any package source once Plan 035 implements user authorization and package graph support.

## Minimal Harness Status

`src/server/runtime_sandbox.rs` and `src/bin/clay-runtime-sandbox.rs` implement the current internal harness only. It proves child spawn/handshake, controlled evaluation, parent timeout kill, fresh restart, payload-budget rejection, and absence of filesystem/network/shell globals. It uses newline-delimited JSON over child stdio rather than the final production protocol and is not wired into package loading.

## Tests Required Before Implementation Completion

- Design doc guard requires process boundary, bounded protocol, restart policy, parent-side validation, hot-path exclusion, runtime profiles, and user-authorized package source language.
- `tests/runtime_sandbox_harness.rs` proves child start/evaluate, timeout kill/restart, oversized output rejection, and no filesystem/network/shell authority through the protocol by default.
