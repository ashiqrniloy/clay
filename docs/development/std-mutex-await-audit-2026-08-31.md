# std::sync::Mutex guard-across-`.await` audit (2026-08-31)

Review P3-2 (plan 105 task 11). Scope: every `std::sync::Mutex` acquisition
reachable from async code, checking whether a guard can be held across an
`.await` (including `?`/early-return paths). No violations found; no code
changed. This is verification, not re-architecture.

## Mechanical argument (why most trees need no per-site audit)

Every async tree that runs server/client work is spawned through a
Send-bound spawn site:

| Tree | Spawn site | Send bound |
| --- | --- | --- |
| Connection family (`handle_connection_loop`, tabs/menus/runtime/documents/workspace handlers) | `src/server/mod.rs:1639` `JoinSet::spawn(async move ...)` | yes |
| Configuration watcher | `src/server/mod.rs:858` `JoinSet::spawn` | yes |
| Git refresh | `src/server/git.rs:143` `JoinSet::spawn` | yes |
| Agent daemon / stderr drain / watcher | `src/server/agent.rs:207,693,700` `tokio::spawn` | yes |
| Client connection (`run_connection`, read pump) | `src/client/mod.rs:1337,1669` `tokio::spawn` | yes |
| Parse sessions | `src/server/parse_coordinator.rs:767` `tokio::spawn` | yes |

`std::sync::MutexGuard` is `!Send`, so a guard held across any `.await` in a
Send-bound future is a compile error. Those trees are therefore safe by
construction; rustc is the enforcement mechanism and CI compiles all targets.

The only non-Send surfaces are the Deno worker's `block_on` calls
(`src/server/js_runtime/worker.rs:238,270,298,340,369` — the mini runtime
that drives async ops) and every async `op_*` function. Those were audited
site by site.

## Per-site table

| Site | Guard | Held across `.await`? | Action |
| --- | --- | --- | --- |
| `RuntimeDiagnosticStore::publish` (`connection/mod.rs:80`) | std `OutputRouter` | No — `broadcast` is sync (`try_send`), no await in scope | safe |
| Connection setup (`connection/mod.rs:518-519`) | tokio store lock + std router lock | No — `live_router()` clones the Arc (guard dropped); subscribe is statement-scoped sync, dropped before `read_client_message(...).await` | safe |
| `ConnectionOutputSubscriptions::drop` (`connection/mod.rs:118`) | std router | No — sync `Drop`, cannot await | safe |
| `bound_state` locks (`connection/tabs.rs:201,343`) | std `Mutex<Option<TabServerState>>` | No — statement-level temporaries (`*x.lock().unwrap() = v;`) | safe |
| `sweep_expired_tabs` (`server/mod.rs:1700`) | std `live_clients` | No — `lock().unwrap().clone()` temporary dropped before the adjacent `.lock().await` | safe |
| `LiveClientGuard::drop` (`server/mod.rs:1722`) | std `live_clients` | No — sync `Drop` | safe |
| Parse coordinator `inner` (`parse_coordinator.rs:331-471,665,861`) | std `ParseCoordinatorInner` | No — sync methods; in async fns the guard is `{ … }`-block-scoped and dropped before any await/spawn | safe |
| `next_update` / `next_diagnostic` (`parse_coordinator.rs:1089-1093`) | tokio mutex around `mpsc::Receiver` | Yes, by design — tokio mutex, await-safe | safe |
| Configuration registry (`configuration.rs:125-289`) | std (`loaded_modules`, `module_errors`, `package_options`) | No — all sync methods, statement/block-scoped | safe |
| Op preference stores (`ops/mod.rs:196-743`, CaretStyle/WrapPolicy/ShellPreferences) | std nested | No — statement-scoped | safe |
| `execute_command` (`ops/mod.rs:1430`): modes branch | std `modes` | No — `execute_discovery` is sync | safe |
| `execute_command`: standard branch | std `commands` | No — `CommandExecutor::execute` is sync (borrow of temporary, no await) | safe |
| `execute_command`: git/workspace branches | tokio `WorkspaceState` | Yes, by design — tokio mutex | safe |
| Async op `op_clay_language_server_authorize` (`ops/language_server.rs:44`) | std `package_service` | No — `{ … }` block returns cloned data, guard dropped before the `.await` | safe |
| Async op `op_clay_language_server_start_session` (`ops/language_server.rs:226`) | std `package_service` | No — same block-scoped pattern; dropped before `workspace.lock().await` | safe |
| Async op `op_clay_packages_load_in_package_domain` (`ops/packages.rs:69`) | std `package_service` | No — block-scoped; dropped before await | safe |
| Async ops documents×5, workspace×3, git×2 | tokio `WorkspaceState`/`DocumentState` | Yes, by design — tokio mutex | safe |
| `LoadEntryAllowlist` methods (`ops/packages.rs:60-120`) | std | No — sync methods, statement-scoped | safe |
| Worker `block_on` sites (`worker.rs:238-369`) | — | No lock held into `block_on`: `begin_evaluation` takes and drops all locks synchronously | safe |

22 async op functions exist; all are covered above (the only std-lock users
are the three package-service sites; the rest are tokio mutexes).

## Methodology

1. AST-ish scan (brace-matched fn bodies) over `src/` for functions
   containing both `.lock()` and `.await` — 168 candidates, of which 146 are
   test functions; production candidates triaged per file.
2. Enumerated every `std::sync::Mutex` type in `src/server` + `src/client`
   (22 distinct fields); traced each acquisition to its guard scope.
3. Verified every spawn site is Send-bound (`JoinSet::spawn` /
   `tokio::spawn`), making guard-across-await a compile error in those trees.
4. Manually read every async op that runs under the worker's non-Send
   `block_on` (22 ops; 5 with plain `.lock()` calls).

## Conclusion

No guard is held across an `.await` anywhere. No lock was added or widened;
no scope-shrinking fixes were needed. The plan's options remain evaluated:
switching to `tokio::sync::Mutex` everywhere was rejected — short critical
sections with std locks are correct and faster, and the Send-bound spawn
model already makes the dangerous pattern unrepresentable.