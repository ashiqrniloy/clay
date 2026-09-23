# Plan 134 task 3 — D5 mutex poison policy (2026-09-22)

Baseline: `grep -rn 'expect("[^"]*poisoned")' src/` on the task-2 tree = **357
sites**; 20 of the original review's "~90" is a stale count (source review
absent, see task 1). Policy and helper: `src/lock_util.rs`
(`LockOrRecover::lock_or_recover`). Rule text lives in the module docs; the
kept-expect site and the two authority-adjacent locks carry inline policy
comments.

## Triage

| Class | Sites | Treatment |
| --- | --- | --- |
| Recoverable production state locks | **330** across 21 files | `lock_or_recover()` |
| Irrecoverable production lock | **1** | keep `expect` + comment |
| Test-only locks (`#[cfg(test)]` hooks/harness, test files) | **25** | keep `expect` (fail loud) |
| Condvar wait on a recovered-class mailbox | 1 inline | `unwrap_or_else(PoisonError::into_inner)` |

Recoverable sites per file:

```
133  src/server/ops/mod.rs             registry/snapshot/publisher/service-handle accessors
 31  src/server/document_analysis.rs   mailboxes, active_documents, output router, coordinator inner
 26  src/server/parse_coordinator.rs   coordinator inner (registry, task handles, versions, stats)
 21  src/server/language_intelligence.rs  coordinator inner
 21  src/server/completion.rs         coordinator inner
 15  src/server/js_runtime/mod.rs     worker slot, agent-host slot, package-service reads, caches
 15  src/client/mod.rs                sync state, capability slots, behavior state
 12  src/server/syntax/mod.rs         tree/decoration/embedded-layer caches
 10  src/server/ops/packages.rs       package loadEntry allowlist, first-party specifier cache
  6  src/server/runtime_reload.rs     package-service reads during reload
  6  src/server/configuration.rs      module state/error caches, package option state
  5  src/server/js_runtime/worker.rs  join hand-off (Drop), mailbox queue
  5  src/server/connection/mod.rs     diagnostic router, bound tab state
  5  src/perf/metrics.rs              perf recorder
  4  src/server/ops/theme.rs          package service / registries
  4  src/server/ops/language_server.rs  package service
  3  src/server/workspace/mod.rs      listing-cancellation registry
  3  src/server/mod.rs                live-client set
  3  src/server/js_runtime/source.rs  module loader state
  1  src/server/ops/document_analysis.rs
  1  src/server/fanout.rs             StateFanout current value
```

### Kept expect (irrecoverable)

- `src/server/syntax/mod.rs` embedded tree-sitter `Parser` (`layer.parser`):
  the parser owns mutable FFI state; a panic mid-parse means the C parser must
  not be reused. Comment added at the site.

### Test-only kept (fail loud, not reachable from a production path)

- `src/server/workspace/mod.rs`: `BETWEEN_METADATA_AND_READ_HOOKS`,
  `ATOMIC_WRITE_PAUSES`, `TEST_TEMP_NAMES`, `BEFORE_REVALIDATE_HOOKS` (all
  inside `#[cfg(test)]` blocks).
- `src/server/parse_coordinator.rs`: test worker gate (`#[cfg(test)]` module).
- Test files: `js_runtime/tests/*` (19), `syntax/tests.rs` (1).

### Condvar arm

`src/server/js_runtime/worker.rs` mailbox `ready.wait(state)`: `Condvar::wait`
also returns the guard on poison; recovered inline and the loop re-checks
queue/closed state, so no torn hand-off is served.

## Security: recovery never bypasses an authority check

- **`PackageLoadEntryAllowlist`** (`src/server/ops/packages.rs`): entries are
  inserted only after path validation, and a lookup miss denies; recovery
  leaves either no entry (fail closed) or a fully written validated entry.
  Policy comment added at the struct.
- **`ClayOpState::set_current_package`** (`src/server/ops/mod.rs`): provenance
  only — every use resolves the package through the host-enabled set, so a
  recovered stale context cannot expand authority. Policy comment added.
- **`PackageService`** (`src/server/ops/*`, `runtime_reload.rs`,
  `js_runtime/mod.rs`): enable/authorize mutates only after the grant checks;
  the allowlist/HMAC envelopes are unchanged, so recovery cannot enable a
  package that was not already authorized.
- **Client capability slots** (`src/client/mod.rs`): the client cache holds a
  server-issued token; the server re-validates it on use.
- **Mailbox/enqueue accounting** (`document_analysis.rs`,
  `js_runtime/worker.rs`): a torn write can lose or double-count one in-flight
  event; each queue stays entry-bounded and re-clamps on the next enqueue.
  Recorded as a ceiling, not an authority bypass.

## Verification

- `cargo clippy --all-targets -- -D warnings`: exit 0 (no unused imports from
  the migration).
- Poisons covered by tests: `poisoned_state_mutex_recovers_service` in
  `src/lock_util.rs` (helper), `src/server/fanout.rs` (live lane publish/read),
  and `src/server/tests.rs` (real `IpcServer`: poison `live_clients`, then run
  the connection-arrival `sweep_expired_tabs` path).
- `scripts/check.sh full`: see `task3-exit-codes.txt`.
