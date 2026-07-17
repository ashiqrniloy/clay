---
date: 2026-07-16 18:25
status: approved
decision_about: "Phase 19 runtime-generation transaction, client installation, stale-edit grace, reload trigger, and lock semantics"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Atomic runtime-generation replacement with bounded stale-edit grace

## Decision

Clay will prepare each hot-reload generation in a fresh server-owned JavaScript runtime while the current generation remains active, validate one complete candidate without mutating live state, and commit all generation-owned server state under a behavior-scoped compare-and-swap lock. Live clients receive one bounded complete runtime-state snapshot, validate it, install it atomically, and acknowledge the installed runtime generation.

Clay will temporarily retain only the immediately previous generation's inert behavior manifest and version metadata—not its JavaScript runtime, workers, sessions, grants, or executable handlers—to accept a narrowly bounded set of already-rendered edits. The initial trigger is the explicit built-in `clay.runtime.reloadConfiguration` command; no filesystem watcher or reload-specific IPC path will be added.

## Exact Semantics

### Server transaction and rollback boundary

1. A single reload attempt prepares generation G2 while G1 remains fully active. A concurrent trigger returns `ReloadInProgress`; it does not queue another evaluation.
2. G2 uses a fresh `deno_core::JsRuntime` and the existing deny-by-default loader, timeout, heap, package provenance, permission, and process-grant checks. Reload reads only configured sources and resolver-authorized packages; evaluation cannot create authority absent an existing user-approved grant.
3. Candidate preparation stages and validates every generation-owned contribution and every affected connection snapshot without mutating active server/client-visible state. Any evaluation, validation, permission, serialization, or size failure drops G2 and leaves G1 unchanged.
4. JavaScript evaluation, package loading, connection snapshot construction, and document refresh preparation occur outside the behavior lock. Ordinary typing, paint, and canonical edit acceptance do not wait for candidate evaluation or background reparsing.
5. Final commit acquires `LockScope::Behavior`, confirms that G1 is still current, and swaps all generation-owned server state once. The runtime generation ID is the common identity for server contributions and client snapshots; independently monotonic behavior/document versions remain explicit within that generation.
6. The server commit is the single rollback boundary. Before it, failure preserves G1. After it, G2 remains authoritative: a broadcast, acknowledgement, or physical cleanup failure cannot roll visible state back after any client may have observed G2.
7. Old executable authority is logically revoked at commit. Old coordinator registrations reject new work immediately; workers, language-server sessions, and child processes then receive existing cancellation and bounded shutdown handling. Package-analysis/process cleanup retains the existing 2-second graceful and 5-second total shutdown ceilings. Cleanup failure emits a sanitized diagnostic and cannot restore G1 authority.

### Client snapshot and acknowledgement

- Each affected connection receives one complete `RuntimeStateSnapshot` scoped to state/documents visible to that connection. It contains the runtime generation ID and all mutually dependent client-visible behavior/rendering state needed for one installation; it does not contain document source text.
- Serialized snapshots must fit Clay's 1 MiB frame ceiling. Snapshot construction or validation above that ceiling fails the candidate before commit rather than sending partial state.
- Live fan-out uses a bounded `tokio::sync::broadcast` channel with capacity 16. A lagged receiver discards missed generations and receives the latest complete snapshot; intermediate generations are not replayed.
- A client validates the entire snapshot into a candidate, swaps all included behavior/rendering state, invalidates affected layout/render caches once, and only then sends `RuntimeGenerationInstalled(G2)`. It never installs individual snapshot fields incrementally.
- Invalid client snapshots receive no acknowledgement and cause the connection to fail closed and reconnect through normal bootstrap. Rebootstrap receives the latest complete authoritative state; no partial candidate remains installed.
- Server commit never waits synchronously for client acknowledgements. Acknowledgement controls stale-edit eligibility and retention only.

### Previous-generation stale-edit grace

For a connection that has not acknowledged G2, an edit stamped with G1 is eligible only when all these conditions hold:

- G1 is the immediately previous generation; older generations are never eligible.
- The operation is an already locally rendered canonical `Edit` or `EditorIntent` allowed by G1's retained inert manifest.
- Normal client identity, editable lease, base document version, transaction ordering, range, region/document lock, payload, and operation validation still pass.
- The edit arrives before two seconds have elapsed since G2 commit.
- Fewer than 256 G1 transactions have been accepted during the generation-wide grace window.
- The connection has not acknowledged G2, disconnected, or received a newer committed generation.

The grace window for a connection closes immediately when it acknowledges G2. Global G1 metadata is discarded when no affected connection remains eligible, or at the first global ceiling: two seconds, 256 accepted G1 transactions, another generation commit, or relevant shutdown. The ceilings are fixed implementation budgets, not user configuration.

Old-version commands are rejected; old-version completion and language-intelligence requests/results, parse/decorative/diagnostic output, and all executable callbacks are stale-dropped. Grace never revives old provider or process authority.

After grace expiry, an old edit receives `EditRejection::InvalidBehaviorVersion`. The server republishes the latest runtime snapshot, and the client requests the existing canonical document resync; document resync clears/reconciles its pending optimistic edits. This corrects both behavior routing and document text/version state.

### Trigger and lock ownership

- Register `clay.runtime.reloadConfiguration` as a Clay-owned built-in global command named **Reload Configuration and Packages** with `RoutingPolicy::ServerFirstWithLock { lock_scope: LockScope::Behavior }`, empty package permissions, and no default keybinding.
- User configuration may bind or discover the command through existing command metadata, Control Center, SDUI action, and key-routing paths. Package JavaScript receives no facade that directly invokes or self-authorizes reload.
- The existing developer/test helper delegates to the same command/service implementation. No `ClientMessage::ReloadRuntime`, second command dispatcher, filesystem watcher, debounce policy, or automatic reload loop is introduced.
- Command routing declares the maximum mutation scope, but candidate evaluation runs outside the behavior lock. The lock is acquired only for the bounded final compare-and-swap. RAII/cancellation cleanup releases it on success, validation failure, cancellation, disconnect, or panic unwinding.

### Snapshot-to-diff upgrade condition

Complete snapshots remain the initial and recovery protocol. Revisit chunking or generation diffs only after instrumentation shows either runtime-snapshot payload size at or above 768 KiB p95 or atomic client installation above 16 ms p95 on representative workloads. Any upgrade requires a separate reviewed protocol/budget decision and must preserve complete-snapshot resynchronization.

## Context

Plan 033 established generation-based replacement of one persistent server runtime, cancellation of older coordinator registrations, rollback on JavaScript evaluation failure, a developer-only trigger, and open-document refresh. Its current reload path still applies portions of `ClayRuntimeEvaluation` to shared live state before `RuntimeGenerationStore::swap`; behavior, theme, typography, SDUI/package UI, decorations/diagnostics, command/mode/provider registries, and live connections therefore lack one transaction and one client install boundary.

Phase 19 requires reload-time JavaScript reevaluation, versioned behavior/rendering updates, live fan-out, atomic client installation, stale-version correction, and scoped locking without moving JavaScript or IPC into typing/paint paths. `ActiveBehaviorManifest` already validates replacement manifests and advances behavior versions only after successful validation. `ClientBehaviorState` already validates a replacement before assignment. The client pending-edit queue is bounded at 256, and invalid document-version/lease/lock failures already use canonical resync flow; `InvalidBehaviorVersion` does not yet request that recovery. `ActiveTypographyState` already demonstrates bounded Tokio broadcast with latest-state recovery after lag.

Fresh-runtime replacement is required because in-place ES-module mutation cannot reliably undo cached modules, closures, token registries, or Rust-side registrations after partial evaluation. The project resolves `deno_core 0.400.0`; its lifecycle supports a fresh `JsRuntime` with a custom `ModuleLoader`, `load_main_es_module`, `mod_evaluate`, `run_event_loop`, and isolate termination.

## Approval

- Proposed by: agent
- Approved by user: Yes
- Approval evidence: After receiving the exact recommended transaction, 1 MiB/capacity-16 snapshot, atomic install acknowledgement, two-second/256-transaction stale-edit grace, explicit command, cleanup, and measured diff-upgrade semantics, the user replied: **"I agree with the recommended decision fully. Create the decision log with @.agents/skills/create-decision-log/ and update plan accordingly"**.

## Alternatives Considered

1. **Mutate live Rust registries and V8 globals in place.** — Rejected. Cached modules, closures, executable tokens, process registrations, and partially applied Rust snapshots cannot be rolled back reliably.
2. **Restart the server after configuration changes.** — Rejected. It drops connections, open documents, leases, pending edits, transient UI, and process state instead of defining hot reload.
3. **Use multi-message client prepare/commit with per-part acknowledgements.** — Rejected initially. It adds distributed transaction state and failure modes while bounded complete snapshots fit the current frame budget. A complete snapshot is also required for lag/reconnect recovery.
4. **Reject every G1 edit immediately at G2 commit.** — Rejected. Clients may already have rendered valid optimistic edits before receiving G2; immediate rejection creates avoidable correction and lost-pending-edit pressure.
5. **Accept stale behavior indefinitely or retain the old runtime.** — Rejected. It prolongs executable/package/process authority, permits unbounded mixed semantics, and pins resources. The selected grace retains inert validation data only and has time/count/ack ceilings.
6. **Hold one global mutex across JavaScript evaluation and refresh.** — Rejected. It would block unrelated server work and violate the typing/paint performance boundary. The selected behavior lock covers only commit.
7. **Add a reload-specific IPC request.** — Rejected. It bypasses existing command validation, discovery, keybinding, Control Center, SDUI, and provenance paths.
8. **Start with a filesystem watcher.** — Rejected. The roadmap permits a watcher or trigger; an explicit command avoids a dependency, debounce/coalescing policy, platform edge cases, and accidental reload loops. A watcher may later invoke the same command/service after a separate product decision.
9. **Start with diffs or chunked snapshots.** — Rejected until measurements cross the approved thresholds. Diffs require base-generation negotiation and still need a full recovery snapshot.

## Rationale and Evidence

- `src/server/mod.rs::{reload_runtime_generation,apply_runtime_evaluation,RuntimeGenerationStore}` shows the existing fresh-generation baseline and the pre-swap live-state mutations that Phase 19 must replace with candidate preparation.
- `src/server/behavior.rs::ActiveBehaviorManifest` validates manifests, reports `InvalidBehaviorVersion`, and increments the behavior version only on accepted replacement.
- `src/client/behavior.rs::ClientBehaviorState` validates a complete manifest before assignment; behavior routing reads one active manifest.
- `src/client/mod.rs` bounds pending edits/events at 256 and already requests canonical resync for stale/future document versions, leases, read-only state, and region locks.
- `src/server/mod.rs::ActiveTypographyState` and `src/server/connection.rs` provide the existing capacity-16 Tokio broadcast/latest-state recovery model.
- Parse, completion, language-intelligence, and document-analysis coordinators expose generation cancellation, stale-result rejection, and provider/worker removal. These are reused after commit rather than duplicated.
- Existing package-analysis/process authority already requires exact grants and bounded revocation. Hot reload may reevaluate configured sources but cannot create filesystem/network/shell/process/workspace authority outside those current approvals.
- Clay's codec rejects frames above the 1 MiB default maximum before allocation; complete runtime snapshots compose with that existing transport boundary.
- The prior Phase 18.7 decision requires persistent runtime state to remain generation-scoped and identifies hot reload as the point where handler invalidation and behavior update semantics must be defined.
- The Phase 18.21 authority decision requires runtime-generation replacement to cancel package workers and trusted child sessions within the approved two-second graceful/five-second total ceilings.

## References

- `plans/054-Phase19-Hot-Reload-and-Behavior-Update-Semantics.md` — Phase 19 implementation sequence and acceptance criteria.
- `docs/wiki/modules/phase19-hot-reload-behavior-update-primitive-review.md` — entry-gate, current-flow, primitive, authority, and budget inventory.
- `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md` — completed fresh-generation baseline.
- `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md` — persistent runtime authority and generation-scoping requirement.
- `decision-logs/2026-07-15-1750-lsp-document-sync-and-package-worker-authority.md` — package worker identity, exact grants, and shutdown ceilings.
- `src/server/{mod,behavior,connection,command_execution}.rs` — current reload, behavior, live connection, and command paths.
- `src/client/{mod,behavior}.rs` — bootstrap/live message handling, optimistic edit queue, resync, and behavior installation.
- `src/protocol/{mod,codec}.rs` and `src/perf/budgets.rs` — lock/routing/version message shapes and bounded transport/work queues.
- `.agents/skills/project-patterns/references/{authority-boundaries,behavior-manifests,extensions-and-ai,protocol-and-performance}.md` — reusable architecture constraints.
- `deno_core 0.400.0` local resolved source/rustdoc and Cargo metadata/tree — exact runtime/module lifecycle APIs.
- Context7 `/denoland/deno_core` documentation for custom `ModuleLoader`, `JsRuntime::new`, `load_main_es_module`, `mod_evaluate`, and `run_event_loop`, consulted 2026-07-16.

## Consequences

- Clients and server components observe complete generations, never a mixture of old behavior with new rendering/provider state.
- Reload failure before commit is reversible by dropping the candidate; failure after commit is handled by latest-state recovery, reconnect, stale-drop, and bounded cleanup rather than unsafe rollback.
- Locally rendered G1 edits get a short compatibility window without retaining G1 executable authority. Expired edits use explicit behavior and document resynchronization.
- Implementation must add a complete runtime-state snapshot/ack protocol, candidate validation/commit boundary, previous inert-manifest retention, live broadcast, scoped lock manager, command integration, and deterministic lag/oversize/rollback/security tests.
- Full snapshots add bounded connection-specific serialization and atomic-install work. Diffs/chunking remain deferred until the approved p95 thresholds are measured.
- Explicit reload means configuration changes do not apply automatically. Add a watcher only if users need automatic reload and it can delegate to the same serialized service with reviewed debounce/coalescing semantics.
- Revisit this decision if normal snapshots approach 768 KiB p95, client installation exceeds 16 ms p95, the two-second/256-transaction grace causes measurable corrections or authority pressure, multiple concurrent reload sources need coalescing, or third-party package authority changes the trust model.
