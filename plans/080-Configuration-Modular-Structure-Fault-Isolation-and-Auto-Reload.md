# Configuration Modular Structure, Fault Isolation, and Auto-Reload

## Analysis (pre-plan findings)

Current state, verified in code:

- **Modular config already exists mechanically.** `~/.config/clay/init.js` can
  statically `import "./any/name.js"` or `await loadConfigurationModule({ path })`;
  `ConfigurationRuntime::resolve_module` / `canonical_local_file`
  (`src/server/configuration.rs`) confine modules to the config root, require
  explicit `.js`, reject URLs/absolute/package specifiers. Any folder layout
  (e.g. `packages/`) already works. Nothing to build here.
- **Reload pipeline already exists and is transactional.**
  `clay.runtime.reloadConfiguration` is a built-in global command
  (`src/server/command_execution.rs:695`) with
  `RoutingPolicy::ServerFirstWithLock { lock_scope: Behavior }`.
  `IpcServer::reload_runtime_generation_inner` (`src/server/mod.rs:954`)
  evaluates a candidate generation and swaps only after successful config load;
  failure preserves the previous working generation and records a
  `clay.runtime.*` diagnostic (tests at `src/server/mod.rs:2827,2995`).
  Third-party runtime/LSP sessions survive trusted reload (Plan 061).
- **Fault-isolation gap.** A throwing or unparseable module rejects the whole
  init.js evaluation: at reload the entire new generation is discarded (all
  good config lost with the bad), at startup the server falls back to defaults.
  `loadConfigurationModule` propagates import errors; there is no optional
  module mode. This violates "broken package config must not sink functional
  core config".
- **No watcher.** Decision log
  `2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`
  rejected a filesystem watcher *for that phase* and explicitly allowed one
  later "after a separate product decision", delegating to the same serialized
  command/service. This plan is that product decision.
- **No default reload chord.** Same decision log (item 57) and test
  `configuration_can_explicitly_bind_reload_without_default_binding`
  (`src/server/js_runtime.rs:7829`) pin "bindable, no default binding".
  User now requires a shipped keybinding — a deliberate amendment.
- **Example is monolithic.** `examples/init.js` (~450 lines, 11 sections)
  mixes base Clay config with first-party package loading. `node --check
  examples/init.js` passes today and must keep passing.

## Objectives

- Segregate configuration into three components — base Clay config in
  `init.js`, first-party package config, third-party package config — as the
  shipped, documented folder structure under `~/.config/clay`, while keeping
  the existing any-file/any-name module freedom.
- Guarantee fault isolation: a broken package-config module degrades to a
  diagnostic; functional core config and the app launch/reload are unaffected.
- Auto-apply configuration changes while Clay runs (watch config root, reload
  through the existing serialized reload service), plus a shipped default
  keybinding that triggers the same reload manually.
- Restructure `examples/` to mirror the three-component structure.

## Expected Outcome

- `~/.config/clay/init.js` stays minimal (core Clay APIs + two optional
  module imports); `packages/first-party.js` and `packages/third-party.js`
  hold package config; users may equally invent their own layout.
- Editing any file under `~/.config/clay` triggers a debounced runtime reload
  with no restart; a broken edit keeps the previous generation and reports a
  diagnostic; `Ctrl+Shift+R` triggers the same reload on demand.
- `examples/` ships `init.js` + `packages/first-party.js` +
  `packages/third-party.js`, all `node --check`-clean, comprehensive across
  the set.
- Docs, API inventory, doc registry, manual test plan, and wiki reflect the
  new structure and semantics.

## Tasks

- [ ] Record the configuration auto-reload and default-chord decision log
  - Acceptance Criteria:
    - Functional: A decision log under `decision-logs/` records: (1) config-root
      watch triggers the existing serialized reload service (amends the "no
      filesystem watcher" phase scoping), (2) `clay.runtime.reloadConfiguration`
      gains a default global chord (amends the "no default keybinding" item),
      (3) the shipped example adopts the base / first-party / third-party
      module structure, (4) `loadConfigurationModule` gains an `optional`
      fault-isolation mode.
    - Performance: No runtime work.
    - Code Quality: Log follows the `create-decision-log` skill format and
      references the amended log
      `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
    - Security: Log restates that the watcher reads only the configuration
      root and grants no new authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md` (items 57, 59, 88, 126: watcher/default-chord deferral)
    - Options Considered:
      - Amend in place: rejected; decision logs are append-only history.
      - New log citing the old: preserves history, records the product decision.
    - Chosen Approach:
      - New decision log via the `create-decision-log` skill, explicitly
        superseding the two deferred items with user-requirement evidence.
    - API Notes and Examples:
      ```text
      decision-logs/<timestamp>-configuration-watch-auto-reload-and-modular-structure.md
      ```
    - Files to Create/Edit:
      - `decision-logs/<timestamp>-configuration-watch-auto-reload-and-modular-structure.md`: new log.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - None (documentation); reviewer confirms the log links the amended decision.

- [ ] Review existing configuration primitives and plan generic primitive gaps before implementation
  - Acceptance Criteria:
    - Functional: Written inventory (in this plan's task notes or a wiki
      comment) of the existing configuration primitives — module resolution
      (`ConfigurationRuntime`), reload service (`IpcServer::reload_runtime_generation`),
      diagnostic store/fanout, keybinding overlay — stating exactly what each
      new requirement reuses; any new Rust is generic (no package-specific
      branches).
    - Performance: Confirms watch/reload stays off keystroke, paint, and IPC
      hot paths (existing configuration contract).
    - Code Quality: No mode/package-specific Rust; reuse-first justification
      for the watcher and fault-isolation mechanisms.
    - Security: Confirms module containment and deny-by-default authority are
      unchanged by the new `optional` flag and watcher.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`
      - `docs/wiki/modules/configuration-runtime.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
    - Options Considered:
      - Skip review (mechanisms exist): rejected; the `optional` flag changes
        a public primitive's semantics and the watcher adds a server lifecycle
        component — both warrant the gate.
      - Primitive-review task before implementation: chosen.
    - Chosen Approach:
      - One review pass over the four primitives above; record reuse mapping;
      only new generic surfaces: a module-error collection channel on
      `ConfigurationRuntime` and a config-root watch loop on `IpcServer`.
    - API Notes and Examples:
      ```rust
      // src/server/configuration.rs — existing primitives to reuse:
      // resolve_module / canonical_local_file (containment)
      // record_loaded_module / state_json (module tracking, diagnostics shape)
      // src/server/mod.rs — reload_runtime_generation_inner (serialized swap)
      ```
    - Files to Create/Edit:
      - None (review output recorded in plan/wiki).
    - References:
      - `src/server/configuration.rs`, `src/server/mod.rs:889-1000`,
        `src/server/ops/configuration.rs`
  - Test Cases to Write:
    - None new; review confirms existing tests cover reused primitives.

- [ ] Add fault-isolated optional configuration modules to `loadConfigurationModule`
  - Acceptance Criteria:
    - Functional: `loadConfigurationModule({ path, optional: true })` catches
      module resolution/parse/evaluation failures, records a
      `clay.configuration.module_failed` warning diagnostic naming the
      root-relative path, and resolves `{ loaded: false, error }` so
      evaluation continues; `optional` omitted/false keeps today's
      fail-the-evaluation behavior; success resolves `{ loaded: true }`.
    - Performance: No work added outside startup/reload evaluation; error
      messages bounded (e.g. 1 KiB) before storage.
    - Code Quality: Failures collected on `ConfigurationRuntime` (mirroring
      `loaded_modules`), drained into the evaluation/reload diagnostics so CLI
      and clients see them through the existing `clay.runtime.*` fanout; no
      swallow-all anywhere else in init.js semantics.
    - Security: Containment validation (`validate_module_path`) runs BEFORE
      the optional catch — escaping paths still hard-fail; diagnostics use
      root-relative display paths (`display_relative_to`); no new authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/configuration/load-configuration-module.md`
      - `runtime/js/configuration.js` (facade contract)
      - `src/server/configuration.rs` (`record_loaded_module` precedent)
    - Options Considered:
      - Example-only try/catch helper around `loadConfigurationModule`: zero
        API change, but errors surface as opaque thrown strings, nothing
        reaches the diagnostic store, and every user reinvents it — rejected
        as the primary mechanism (helper still shown in example).
      - Server-side op performs the import: rejected; ops here are sync,
        module import is async JS — keep import in the facade.
      - Facade catches + records via runtime: chosen — small, typed,
        testable, feeds existing diagnostics.
    - Chosen Approach:
      - Extend the facade: validate via existing op, `try { await import } catch`
        when `optional`, record through the runtime, return a status object.
        Rust: add a bounded `module_errors: Mutex<Vec<…>>` on
        `ConfigurationRuntime` plus a recording op; evaluation/reload paths
        drain it into their diagnostic lists (exact drain point verified
        against `ClayRuntimeEvaluation` assembly during implementation).
    - API Notes and Examples:
      ```js
      // runtime/js/configuration.js — facade shape after change:
      export async function loadConfigurationModule(options) {
          // ...existing { path: string } validation...
          const path = configurationOps().op_clay_configuration_load_module(options.path);
          try {
              await import(path);
              return { loaded: true };
          } catch (error) {
              if (options?.optional !== true) throw error;
              const message = String(error?.message ?? error).slice(0, 1024);
              configurationOps().op_clay_configuration_record_module_error(path, message);
              return { loaded: false, error: message };
          }
      }
      ```
    - Files to Create/Edit:
      - `runtime/js/configuration.js`: optional-mode facade + return status.
      - `src/server/configuration.rs`: `module_errors` store, bounded record
        method, drain accessor.
      - `src/server/ops/configuration.rs`: new
        `op_clay_configuration_record_module_error` (bounded inputs).
      - `src/server/js_runtime.rs` and/or `src/server/mod.rs`: drain module
        errors into evaluation/reload diagnostics (tentative exact location;
        depends on where evaluation diagnostics are assembled).
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - Optional module with syntax error: evaluation succeeds, subsequent
      init.js statements applied, `clay.configuration.module_failed`
      diagnostic recorded with root-relative path.
    - Optional module missing: same isolation behavior.
    - Required (non-optional) broken module: evaluation fails; reload keeps
      previous generation (regression guard on
      `reload_reruns_init_js_package_load_in_fresh_generation_and_preserves_old_on_failure`).
    - Path escape (`../outside.js`) with `optional: true`: still hard-fails
      (containment precedes isolation).
    - Message over 1 KiB: truncated before storage.

- [ ] Watch the configuration root and auto-reload through the serialized reload service
  - Acceptance Criteria:
    - Functional: When the server runs with a configuration root, a background
      task detects created/modified/deleted `.js` files (and
      `preferences.json`) anywhere under the root and triggers
      `reload_runtime_generation()` after a quiet-period debounce; no reload
      loops (a completed reload re-baselines the snapshot); watcher starts only
      when a config root exists and stops with the server.
    - Performance: Poll interval ~1 s with
      `MissedTickBehavior::Skip`; scan bounded (≤ 256 files, depth ≤ 8,
      skip dotfiles/tmp); zero work on editor hot paths; debounce collapses
      edit storms into one reload.
    - Code Quality: Single small module; reuses
      `IpcServer::reload_runtime_generation` (already serialized by the
      `reload_attempt` lock — a watcher trigger during a manual reload yields
      the existing `clay.runtime.reload_in_progress` outcome, no new
      coordination); watcher-triggered diagnostics reach connected clients
      through the existing runtime-diagnostic fanout (verify, wire if gaps).
    - Security: Reads only the configuration root (canonicalized, same root
      `ConfigurationRuntime` enforces); follows no symlinks outside the root;
      no new authority, IPC surface, or client-visible attack vector.
  - Approach:
    - Documentation Reviewed:
      - Tokio docs (`/tokio-rs/tokio` via ctx7): `tokio::time::interval`,
        `Interval::tick`, `MissedTickBehavior`, `tokio::fs` metadata.
      - `decision-logs/2026-07-16-1825-...hot-reload...md` item 59/126
        (watcher must delegate to the same serialized service with reviewed
        debounce — this task is that review).
    - Options Considered:
      - `notify` crate (event-driven inotify/FSEvents): rejected — new
        dependency, platform edge cases, watcher-thread lifecycle; decision
        log previously cited exactly these costs.
      - Tokio polling of the config root: chosen — no new dependency (tokio
        `fs`/`time` features already enabled), trivially cross-platform, and
        correct at config-directory scale; handles new/deleted files a
        loaded-modules-only watch would miss.
      - Watch only `loaded_modules`: rejected — misses newly created files the
        user is about to import and deletions of never-imported files.
    - Chosen Approach:
      - Polling watcher: snapshot `BTreeMap<path, (mtime, len)>` via bounded
        recursive walk; on diff, wait a 300 ms quiet period (re-scan until
        stable, capped), call `reload_runtime_generation()`, then re-baseline.
      - Known ceiling: settings-panel persist writes `preferences.json` then
        self-reloads; the watcher may fire one extra idempotent reload after
        it. Accepted (reloads are cheap and generation-guarded); mark with a
        `ponytail:` comment naming the upgrade path (server-side write
        suppression window) if it ever matters.
    - API Notes and Examples:
      ```rust
      // sketch — src/server/config_watch.rs
      let mut interval = tokio::time::interval(Duration::from_secs(1));
      interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
      loop {
          interval.tick().await;
          let snapshot = scan_config_root(&root); // bounded walk, mtime+len
          if snapshot == baseline { continue; }
          // debounce: re-scan until quiet for 300ms (bounded iterations)
          baseline = server.reload_runtime_generation().await.rebaseline(snapshot);
      }
      ```
    - Files to Create/Edit:
      - `src/server/config_watch.rs`: new watcher task (~100 lines).
      - `src/server/mod.rs`: spawn watcher in `IpcServer::run` when
        `configuration_root.is_some()`; shutdown handling.
      - `src/main.rs`: no change expected (root already threaded through).
    - References:
      - `src/server/mod.rs:926` (`reload_runtime_generation`),
        `src/server/mod.rs:889` (`trigger_developer_hot_reload` precedent)
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - Modify an imported module file → reload occurs without command intent;
      new binding/config active in the new generation.
    - Introduce a syntax error, save → previous generation preserved,
      `clay.runtime.*` diagnostic recorded; fix the file, save → recovery
      reload succeeds.
    - Create a NEW file under a subfolder and import it → detected (covers
      creation, not just modification).
    - Rapid successive saves (edit storm) → exactly one reload after quiet
      period (debounce).
    - Watcher trigger during an in-flight manual reload → `reload_in_progress`
      outcome, no crash, no queue pile-up.
    - Server without a configuration root → no watcher task spawned.

- [ ] Ship a default keybinding for configuration reload
  - Acceptance Criteria:
    - Functional: `Ctrl+Shift+R` (global scope) binds
      `clay.runtime.reloadConfiguration` in `default_keymaps()`; users can
      still `unbindKey`/`bindKey` to override (last-wins overlay semantics
      unchanged); Control Center listing shows the chord.
    - Performance: No runtime cost (static rule entry).
    - Code Quality: Updates the pinning test
      `configuration_can_explicitly_bind_reload_without_default_binding` into
      a default-present + still-overridable test; example config re-declares
      the chord idempotently like other shipped defaults.
    - Security: Command keeps `ServerFirstWithLock` routing and
      `UnauthorizedTarget` rejection from package JavaScript — unchanged.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/keybindings/bind-key.md` (Phase 19 reload
        note stating "no default chord exists" — must be revised).
      - `src/protocol/mod.rs::default_keymaps` (global_client_ui precedent).
    - Options Considered:
      - Keep no-default, document `bindKey` only: rejected — explicit user
        requirement ("there should be a keybinding").
      - Default chord `Ctrl+Shift+R`: chosen — already the documented example
        chord for this command; no conflict in `default_keymaps()`.
    - Chosen Approach:
      - Add one `KeyBindingRule` global entry; note the editor-scope
        `clientRequestResync` doc example shares the chord in a different
        scope (scopes resolve independently; call it out in docs).
    - API Notes and Examples:
      ```rust
      // src/protocol/mod.rs::default_keymaps()
      KeyBindingRule::global(
          "clay.runtime.reloadConfiguration",
          ctrl_shift_key(KeyCode::Character("r".to_string())),
      ),
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: default rule.
      - `src/server/js_runtime.rs`: replace the no-default-binding test.
      - `examples/init.js`: idempotent re-declaration + comment.
    - References:
      - `src/server/ops/keybindings.rs:429` (routing policy),
        `src/server/command_execution.rs:695`
  - Test Cases to Write:
    - Fresh startup: `Ctrl+Shift+R` resolves to the reload command in the
      global scope without any init.js binding.
    - `unbindKey("Ctrl+Shift+R", { scope: "global" })` removes the default;
      re-`bindKey` on another chord wins.
    - Package JavaScript calling the command via `clay:commands` remains
      rejected (existing test keeps passing).

- [ ] Restructure the canonical example configuration into base / first-party / third-party modules
  - Acceptance Criteria:
    - Functional: `examples/` ships: `init.js` (base Clay config: header,
      ground rules, import map, theme+appearance, typography, caret, key
      bindings incl. reload chord, shell policy, syntax preference,
      programmatic editor control, planned-API section; ends with
      `await loadConfigurationModule({ path: "./packages/first-party.js", optional: true })`
      then the third-party equivalent), `examples/packages/first-party.js`
      (LSP grant helper + grants + `loadPackage("@clay/…")` lines +
      settings/git), `examples/packages/third-party.js` (commented template;
      no third-party packages ship). Every previously documented surface
      appears exactly once across the set; `node --check` passes on all three.
    - Performance: Example evaluation cost unchanged (same statements,
      two extra bounded module imports).
    - Code Quality: Base `init.js` is fully functional standalone (app usable
      with package modules absent/broken — the "use Clay to fix Clay"
      requirement); ordering constraint (grants before first `loadPackage`)
      preserved inside `first-party.js` and noted in `init.js` comments;
      per-module fault-isolation helper pattern shown for package loads.
    - Security: No new authority in examples; commented third-party template
      references the adoption/approval policy instead of implying raw loading.
  - Approach:
    - Documentation Reviewed:
      - `examples/init.js` (current 11-section monolith).
      - `.agents/skills/create-plan/references/clay.md` (Example Configuration
        Maintenance + Package Default Loading requirements).
      - `test-plan/02-configuration-init-js.md` (setup copies the example).
    - Options Considered:
      - Single-file example + prose suggesting splits: rejected — user
        requires the shipped structure to embody the segregation.
      - `config/` folder name instead of `packages/`: rejected — `packages/`
        matches the user's stated mental model and the contents are package
        configs.
      - Three files as above: chosen; minimal, mirrors the three components.
    - Chosen Approach:
      - Split sections 2–3 (grants, packages) into `packages/first-party.js`;
      everything core stays in `init.js`; static-import users remain supported
        but the example uses `optional: true` module loads to demonstrate the
        fault-isolation contract.
    - API Notes and Examples:
      ```js
      // examples/init.js (tail)
      // Package configuration is segregated and fault-isolated: a broken
      // module records clay.configuration.module_failed and never blocks
      // the base configuration above or app launch.
      await loadConfigurationModule({ path: "./packages/first-party.js", optional: true });
      await loadConfigurationModule({ path: "./packages/third-party.js", optional: true });
      ```
    - Files to Create/Edit:
      - `examples/init.js`: slim to base config + module imports.
      - `examples/packages/first-party.js`: new (sections 2–3 content).
      - `examples/packages/third-party.js`: new (commented template).
      - `README.md` / docs referencing "copy examples/init.js": copy-the-tree
        instructions (tentative list; grep for references during task).
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
  - Test Cases to Write:
    - `node --check` passes on all three example files.
    - Copy tree to a temp config root, boot server fixture: base config
      applied, first-party packages loaded, no diagnostics.
    - Delete/break `packages/first-party.js`: app launches, base config
      active, `clay.configuration.module_failed` diagnostic recorded.
    - Existing doc tests referencing `examples/init.js` content updated and
      passing.

- [ ] Define and verify the package default init.js loading experience under the new structure
  - Acceptance Criteria:
    - Functional: One-line `loadPackage("@clay/markdown")`-style loads still
      work verbatim inside `packages/first-party.js` with no manifest copying,
      facade plumbing, or extra setup; package defaults apply after the line.
    - Performance: Package load latency unchanged under module indirection.
    - Code Quality: The three-component structure is documented as convention,
      not requirement — any local module layout keeps working (regression
      test with an alternate layout, e.g. `keys.js` + `editor.js`).
    - Security: Module loading grants no package authority beyond the existing
      configuration trust domain.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Package Default
        Loading Task requirements).
      - `docs/reference/packages/creating-packages.md` (loading section).
    - Options Considered:
      - Only test the shipped layout: rejected — user requires layout freedom
        to be guaranteed, so an alternate-layout regression test is required.
      - Shipped layout + alternate-layout tests: chosen.
    - Chosen Approach:
      - Extend existing package-loading integration tests with (a) shipped
        three-file layout, (b) arbitrary alternate layout, both driving the
        same one-line `loadPackage` calls.
    - API Notes and Examples:
      ```js
      // any file, any folder depth within ~/.config/clay:
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `tests/package_loading*.rs` or `src/server/js_runtime.rs` tests:
        layout matrix coverage (tentative location; match existing fixtures).
      - `docs/reference/packages/creating-packages.md`: configuration-layout
        section update.
    - References:
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
  - Test Cases to Write:
    - Shipped layout: all `@clay/*` one-line loads succeed from
      `packages/first-party.js`.
    - Alternate layout (`./a/b/c.js` chain of static imports): identical
      package outcomes.
    - Third-party template file left fully commented: zero third-party
      activity on boot.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: `loadConfigurationModule` doc gains the `optional` option
      and `{ loaded, error? }` return; reload command doc gains the default
      chord; watcher behavior documented as server behavior (no new JS API —
      verify none is needed; if a watch-status surface proves necessary, add
      it through the full schema instead of a hidden key).
    - Performance: Registry/docs checks stay deterministic and offline.
    - Code Quality: Every touched API doc carries stable ID, user-facing
      name, key bindings, custom properties, errors, permissions, backing
      Rust path, op, facade path, lookup tags; master index links updated;
      `cargo run --bin update-doc-registry` run and stale-registry test green.
    - Security: Docs restate configuration authority boundaries; no raw-op
      exposure as user surface.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`,
        `clay-js-api-naming.md`, `doc-registry-tests.md`
      - `docs/reference/clay-js-api/schema.md`, `api-inventory.toml`
    - Options Considered:
      - New `watchConfiguration` JS API: rejected (YAGNI) — watcher is
        server-automatic; a toggle can ride the existing package-option or a
        future documented API if users ask.
      - Docs-only updates for changed surfaces: chosen.
    - Chosen Approach:
      - Update existing docs in place; run the registry generator; rely on
        `tests/clay_js_doc_registry.rs` / `clay_js_api_inventory.rs` gates.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test clay_js_doc_registry --test clay_js_api_inventory
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration/load-configuration-module.md`: `optional`, return shape, isolation semantics.
      - `docs/reference/clay-js-api/configuration.md`: watcher + default chord + three-component convention.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: revise "no default chord" Phase 19 note.
      - `docs/reference/clay-js-api/api-inventory.toml`: custom properties for changed APIs.
      - `docs/index.md`: link updates if any.
      - Generated registry artifacts via the update command.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
  - Test Cases to Write:
    - Registry-current test passes after regeneration.
    - Inventory test covers `optional` custom property on
      `loadConfigurationModule`.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Configuration review for the whole phase: watcher behavior,
      `optional` modules, default chord, and example structure are all
      expressed through documented Clay JS APIs or documented server behavior
      — zero hidden keys/flags.
    - Performance: Configuration remains startup/reload-only.
    - Code Quality: Review notes appended to
      `docs/reference/clay-js-api/configuration.md` in the established
      per-phase review style.
    - Security: Explicit statement that watch/reload/isolation grant no
      filesystem/network/shell/package-install/AI/workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Clay Configuration
        Task), `docs/reference/clay-js-api/configuration.md` phase-review sections.
    - Options Considered:
      - Configurable watcher toggle in this phase: rejected (YAGNI) — record
        as rejected hidden-key candidate in the review section.
      - Fixed automatic watcher + documented semantics: chosen.
    - Chosen Approach:
      - One review section documenting accepted surfaces and rejected
        configuration keys (watcher interval, debounce, enable/disable stay
        compiled constants this phase).
    - API Notes and Examples:
      ```text
      docs/reference/clay-js-api/configuration.md#phase-23-configuration-structure-and-auto-reload-review
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: review section.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Attempt `setPackageOption` with watcher keys (e.g.
      `core.watch.intervalMs`) → rejected by the closed allowlist (extend the
      existing rejection test pattern in `src/server/configuration.rs`).

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: `test-plan/02-configuration-init-js.md` updated: setup copies
      the example tree; C1/C7/C8 reflect the three-file layout; new numbered
      steps cover watcher auto-reload (edit → live apply), broken-module
      isolation at boot, default `Ctrl+Shift+R` reload, and debounce
      sanity; executed on a real Linux build with pass/fail recorded.
    - Performance: New steps assert no config evaluation on keystroke/paint
      (existing negative check extended to the watcher).
    - Code Quality: `test-plan/index.md` coverage matrix updated if module
      scope changes; no existing step weakened.
    - Security: Negative checks retain the no-new-authority assertions.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/02-configuration-init-js.md`, `test-plan/index.md`
      - `.agents/skills/create-plan/references/clay.md` (Manual Test Plan Task)
    - Options Considered:
      - New module file for watcher steps: rejected — same configuration
        domain; extend module 02.
      - Extend module 02: chosen.
    - Chosen Approach:
      - Amend setup + steps in place; run them on the final build.
    - API Notes and Examples:
      ```bash
      cp -r examples/. ~/.config/clay/   # new setup shape
      ```
    - Files to Create/Edit:
      - `test-plan/02-configuration-init-js.md`: updated + new steps.
      - `test-plan/index.md`: matrix touch-up if needed.
    - References:
      - `docs/development/` deep references where applicable.
  - Test Cases to Write:
    - Manual steps as above; results recorded in the file per convention.

- [ ] Linux platform verification gate
  - Acceptance Criteria:
    - Functional: `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`, and `cargo test` all pass
      on Linux.
    - Performance: No regressions in existing perf-invariant tests.
    - Code Quality: Zero new warnings; all new code covered by the tasks'
      tests.
    - Security: Trust-domain, containment, and authority test suites green.
  - Approach:
    - Documentation Reviewed:
      - `AGENTS.md` (platform-validation: Linux gates blocking).
    - Options Considered:
      - Windows smoke: not required (Linux is the blocking host; keep Windows
        paths from regressing where practical only).
      - Linux-only gate: chosen.
    - Chosen Approach:
      - Run the four commands; fix fallout in-place.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && \
      cargo clippy --all-targets -- -D warnings && cargo test
      ```
    - Files to Create/Edit:
      - Any file needing lint/format fixes.
    - References:
      - `AGENTS.md`
  - Test Cases to Write:
    - The gate itself is the check.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages (notably `docs/wiki/modules/configuration-runtime.md` for watcher, isolation, and structure).
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/configuration-runtime.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made

- To be filled after tasks are completed and tests pass. (Known accepted
  trade-off going in: polling watcher instead of event-driven `notify`, and a
  possible extra idempotent reload after settings-panel preference persists —
  both recorded in the watcher task.)

## Further Actions

- To be filled after task completion with improvements, rationale, and
  priority. (Candidates already visible: user-facing watcher enable/disable
  toggle if requested; event-driven watching via `notify` if poll latency or
  battery cost ever matters; per-module reload granularity.)
