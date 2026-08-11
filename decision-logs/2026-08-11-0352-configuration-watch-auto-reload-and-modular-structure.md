---
date: 2026-08-11 03:52
status: approved
decision_about: "Configuration modularity, fault-isolated modules, automatic reload, default reload chord, and core/package ID ownership"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Modular configuration with fault isolation, automatic reload, and owned IDs

## Decision

Clay will keep `~/.config/clay/init.js` as the minimal base configuration entry point and support any user-chosen local module layout beneath the configuration root. The shipped example will use `init.js` plus optional `packages/first-party.js` and `packages/third-party.js` modules; optional module failures produce bounded diagnostics without preventing functional base configuration from loading.

Clay will automatically detect relevant configuration-root changes with a bounded Tokio polling watcher and delegate each reload to the existing serialized `runtime.reloadConfiguration` service. The command will ship with a default global `Ctrl+Shift+R` binding. Core dotted identifiers use bare `<domain>.<name>` names, package-owned identifiers use the package's own `<package>.<name>` prefix, and reserved core domains cannot be claimed by third-party package `apiPrefix` values.

This decision supersedes only the earlier trigger/keybinding deferrals. The prior runtime-generation transaction, candidate validation, compare-and-swap commit, rollback boundary, client snapshot, stale-edit, worker cleanup, and authority semantics remain in force.

## Context

The earlier Phase 19 decision selected an explicit reload command as the initial trigger and deferred both a filesystem watcher and a default reload chord. Plan 080 addresses the later requirement that configuration changes apply while Clay runs and that users have a shipped reload shortcut. It also makes the existing modular loading capability useful for separating base, first-party, and third-party configuration while ensuring a broken package module cannot sink the base configuration.

The identifier decision is included because core IDs were renamed during this work: `runtime.reloadConfiguration` is the current core command spelling, not the retired `clay.runtime.reloadConfiguration`. Package IDs remain visibly owned by their package prefix.

## Approval

- Proposed by: User requirements and agent implementation plan.
- Approved by user: Yes.
- Approval evidence: The user explicitly instructed: **“Complete first task Record the configuration auto-reload and default-chord decision log and update the plan once done.”**

## Alternatives Considered

1. **Keep explicit reload only and provide no default chord** — rejected. It preserves the older Phase 19 scope but does not satisfy automatic application or the required shipped keybinding.
2. **Use an event-driven `notify` watcher** — rejected for this phase. It adds a dependency, platform-specific event behavior, watcher lifecycle complexity, and debounce/coalescing edge cases. Tokio polling is sufficient for a small configuration tree and uses existing dependencies.
3. **Expose watcher interval, debounce, or enable/disable as configuration APIs** — rejected. Fixed bounded behavior avoids a new configuration surface and prevents configuration from controlling its own reload loop.
4. **Watch only modules already loaded by `init.js`** — rejected. It would miss newly created modules and relevant deletions under the configuration root.
5. **Use an example-only `try/catch` helper for broken modules** — rejected as the primary contract. It would provide inconsistent behavior and would not reliably feed failures into Clay's diagnostic store. The facade gets an explicit `optional: true` mode; required modules retain fail-the-evaluation behavior.
6. **Mutate the active runtime or add a reload-specific IPC path** — rejected. The watcher must call the existing serialized reload service so candidate evaluation, locking, rollback, command routing, and authority cleanup remain centralized.
7. **Keep the `clay.<domain>.*` core namespace** — rejected. Bare core domains make ownership clear alongside package-owned prefixes; the old spelling remains rejected. `clay:` module specifiers and `package.json` `clay.*` manifest keys are structural exceptions, not public dotted IDs.

## Rationale and Evidence

### Reload trigger and watcher

- `src/server/mod.rs::reload_runtime_generation` and `execute_reload_command` already serialize attempts, evaluate a fresh candidate, preserve the active generation on pre-commit failure, and publish existing diagnostics. Reusing this path avoids a second reload implementation.
- The watcher will poll approximately once per second with `MissedTickBehavior::Skip`, scan at most 256 files to depth 8, skip dotfiles and temporary files, and wait for a 300 ms quiet period before reloading. It will track relevant `.js` files and `preferences.json`, re-baseline after each attempt, and stop with the server.
- Watcher work stays off typing, paint, and other editor hot paths. It reads only the canonical configuration root and adds no filesystem, network, shell, package, workspace, AI, or client-JavaScript authority.
- A watcher-triggered reload and a manual reload use the same `reload_attempt` serialization. A concurrent trigger reports the existing in-progress outcome rather than queuing unbounded work.

### Default command binding

- `runtime.reloadConfiguration` remains a Clay-owned global command with server-first behavior locking and no package-JavaScript self-authorization.
- `Ctrl+Shift+R` is the default global chord because it is already the documented reload example and does not conflict with the global default map. Existing overlay semantics remain: users can unbind it or replace it in `init.js`.
- An editor-scope use of the same physical chord remains independent from the global binding under existing scope resolution rules.

### Modular and fault-isolated configuration

- `src/server/configuration.rs::ConfigurationRuntime` already canonicalizes the configuration root, resolves relative local modules, requires explicit local paths, and confines modules to the root. `optional: true` must validate containment before catching import failure.
- `runtime/js/configuration.js` will catch only module resolution/parse/evaluation failures requested as optional, record a root-relative bounded diagnostic through a Clay-owned op, and return a status object. Missing or broken optional package modules therefore do not discard successful base configuration. Required modules continue to throw.
- The example's first-party module preserves grant-before-`loadPackage` ordering. The third-party module remains a safe commented template and cannot silently add third-party behavior.

### Identifier ownership

- Core command/API/diagnostic IDs are bare `<domain>.<name>` values such as `runtime.reloadConfiguration` and `editor.serverInsertText`.
- Package commands, options, and contributions begin with the package's own `apiPrefix`; `setPackageOption` continues to reject `clay.` and non-package-owned options.
- `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs` prevents third-party `apiPrefix` squatting. Bundled first-party packages are exempt only through the compiled bundled inventory and remain protected by exact manifest integrity checks.

## References

- `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md` — prior approved runtime-generation transaction; its explicit-watcher and no-default-chord scope is superseded here.
- `plans/080-Configuration-Modular-Structure-Fault-Isolation-and-Auto-Reload.md` — implementation plan, alternatives, budgets, tests, and file-level approach.
- `src/server/mod.rs` — serialized reload command/service and runtime-generation commit path.
- `src/server/configuration.rs` — configuration-root canonicalization and local-module containment.
- `runtime/js/configuration.js` — current configuration facade and optional-module extension point.
- `src/protocol/mod.rs::default_keymaps` — default keymap source of truth.
- `src/server/ops/keybindings.rs` and `src/server/command_execution.rs` — reload routing and command ownership.
- `src/packages/manifest.rs` — `RESERVED_CORE_API_DOMAINS` and package-prefix validation.
- `.agents/skills/project-patterns/references/configuration-system.md` — configuration, authority, and keymap overlay patterns.
- `.agents/skills/project-patterns/references/extensions-and-ai.md` — generation-safe reload and watcher reuse pattern.
- Tokio API review recorded in plan 080: `tokio::time::interval`, `MissedTickBehavior::Skip`, and bounded `tokio::fs` metadata polling.

## Consequences

- Editing supported files below `~/.config/clay` will apply through the same transactional reload path without restarting Clay.
- A broken optional package module is visible as a diagnostic while the previous working generation or functional base configuration remains usable.
- Polling has a small periodic filesystem cost and may produce one extra idempotent reload after a settings preference write; this is accepted for the bounded, dependency-free implementation.
- The watcher intentionally has no user-facing toggle or tuning API in this phase. Revisit that only if measured scan cost, reload storms, or deployments with unusually large configuration trees justify a separate decision.
- Existing users with retired `clay.<domain>.*` IDs must migrate those IDs; the runtime keeps the app launch/fallback safety behavior rather than granting compatibility aliases.
