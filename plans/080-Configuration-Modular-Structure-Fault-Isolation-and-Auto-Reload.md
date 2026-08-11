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
  `runtime.reloadConfiguration` is a built-in global command
  (`src/server/command_execution.rs:695`) with
  `RoutingPolicy::ServerFirstWithLock { lock_scope: Behavior }`.
  `IpcServer::reload_runtime_generation_inner` (`src/server/mod.rs:954`)
  evaluates a candidate generation and swaps only after successful config load;
  failure preserves the previous working generation and records a
  `runtime.*` diagnostic (tests at `src/server/mod.rs:2827,2995`).
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

- [x] Record the configuration auto-reload and default-chord decision log
  - Acceptance Criteria:
    - Functional: A decision log under `decision-logs/` records: (1) config-root
      watch triggers the existing serialized reload service (amends the "no
      filesystem watcher" phase scoping), (2) `runtime.reloadConfiguration`
      gains a default global chord (amends the "no default keybinding" item),
      (3) the shipped example adopts the base / first-party / third-party
      module structure, (4) `loadConfigurationModule` gains an `optional`
      fault-isolation mode, (5) core dotted IDs drop the `clay.` prefix in
      favor of bare `<domain>.<name>` with package IDs staying
      `<package>.<name>`, and core domains become reserved against
      third-party `apiPrefix` squatting.
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
      decision-logs/2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md
      ```
    - Files to Create/Edit:
      - `decision-logs/2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md`: approved decision log created for the watcher, default chord, modular structure, optional module isolation, and ID ownership decisions.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `decision-logs/2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md`
  - Test Cases to Write:
    - None (documentation); reviewer confirms the log links the amended decision.

- [x] Review existing configuration primitives and plan generic primitive gaps before implementation
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
      - One review pass over the existing configuration primitives; record the
        reuse map and keep all additions generic. New surfaces are limited to
        a bounded module-error collection/drain on `ConfigurationRuntime`, a
        config-root polling task, and a server-level runtime-diagnostic fanout
        that reuses the generic bounded `OutputRouter` connection broadcast.
        No watcher-specific IPC or watcher configuration API is added.
    - API Notes and Examples:
      ```rust
      // src/server/configuration.rs — existing primitives to reuse:
      // resolve_module / canonical_local_file (containment)
      // record_loaded_module / state_json (module tracking, diagnostics shape)
      // src/server/mod.rs — reload_runtime_generation_inner (serialized swap)
      // src/server/output_router.rs — bounded connection-scoped diagnostic fanout
      // src/server/connection.rs — bootstrap retention plus live diagnostic subscription
      ```
    - Files to Create/Edit:
      - None (review output recorded in plan/wiki).
    - References:
      - `src/server/configuration.rs`, `src/server/mod.rs:889-1000`,
        `src/server/ops/configuration.rs`
      - `src/server/connection.rs`, `src/server/output_router.rs`,
        `docs/wiki/modules/configuration-runtime.md`
  - Test Cases to Write:
    - None new for the review; later implementation tasks must cover the
      module-error drain, watcher lifecycle/debounce, and live diagnostic
      fanout identified below.
  - Review Findings (completed 2026-08-11):
    - **Module resolution and containment — reuse.**
      `ConfigurationRuntime::from_config_root` canonicalizes the root and
      entry point; `resolve_module`/`resolve_from_entry` and
      `canonical_local_file` enforce explicit local `.js` paths and the
      root boundary; `load_module_source` repeats the boundary check and
      records loaded modules; `state_json` exposes deterministic module
      state. `op_clay_configuration_load_module` validates before the JS
      facade imports. The optional mode therefore needs only a bounded,
      configuration-owned module-error store/drain; it must not weaken path
      validation or create a second resolver.
    - **Evaluation and generation replacement — reuse.**
      `ClayJsRuntimeService::load_configuration_from_root*` runs constrained
      configuration evaluation on the existing runtime worker. `IpcServer`
      `reload_runtime_generation`/`execute_reload_command` already serialize
      attempts with `reload_attempt`; `production_reload` builds the fresh
      candidate; `prepare_runtime_generation_candidate` validates it; and
      commit swaps only after successful preparation. The watcher calls this
      service and adds no reload IPC, generation protocol, lock, or package
      branch.
    - **Diagnostics and fanout — reuse plus one generic gap.**
      `RuntimeDiagnosticStore` is bounded, deduplicating retention used by
      bootstrap; `IpcServer::record_runtime_diagnostic` stores diagnostics in
      both the retention store and the active generation; manual reloads
      return diagnostics to their initiating command connection. The generic
      `OutputRouter<T>` already provides bounded per-connection subscription
      and broadcast semantics, but runtime diagnostics currently have no
      server-wide live subscription in the connection select loop (parse and
      analysis routers cover other diagnostic classes). Add one
      connection-scoped `RuntimeDiagnostic` router using that generic
      primitive; record once, retain for reconnect/bootstrap, and broadcast
      to live clients. Do not make the watcher send client messages directly.
    - **Keybinding overlay — reuse; no new primitive.**
      `default_keymaps()` is the static default source; `ClayOpState::bind_key`
      and `unbind_key` validate and publish behavior manifests; configuration
      bindings are retained in `configured_keymaps`; mode activation reapplies
      those bindings after mode/package keymaps; `ActiveBehaviorManifest` and
      the client behavior router consume only inert validated rules. The
      reload chord is one static default rule plus the existing override path.
    - **Lifecycle and performance boundary — reuse plus one task.**
      `IpcServer::run` performs initial configuration load before entering its
      accept loop. A small generic config-root task can be spawned only when an
      effective root exists: the explicit fixture/CLI root takes precedence,
      otherwise the same default-root resolver used by configuration loading
      supplies `~/.config/clay` when `init.js` exists. It must be stopped with
      the server. Polling and quiet debounce remain background work;
      configuration evaluation already has runtime timeout/heap limits, and
      candidate evaluation remains outside the behavior commit lock. No
      watcher or module-error work enters
      keypress, text edit acknowledgement, paint, layout, or parse hot paths.
    - **Security boundary — preserved.**
      Optional failure handling happens after `validate_module_path`; escape,
      URL, absolute, extensionless, and package specifiers still hard-fail.
      The watcher scans only the canonical configuration root with bounded
      depth/file count and rejects canonical paths outside it. Existing
      constrained facades and package authority checks remain unchanged; the
      watcher grants no filesystem, network, shell, package, workspace, AI,
      WASM, native-widget, raw-op, or client-JavaScript authority.
    - **Generic-gap decision.**
      Implement only (1) bounded module-error collection/drain,
      (2) bounded root polling/debounce lifecycle, and (3) reusable live
      runtime-diagnostic fanout. Example-file organization, the default
      chord, and ID renames are composition/static/documentation work and do
      not justify new Rust primitives. No package-specific or mode-specific
      Rust branch is permitted.

- [x] Rename core dotted IDs to bare `<domain>.<name>` across the codebase
  - Acceptance Criteria:
    - Functional: Every core Clay command/JS-API/diagnostic identifier drops
      the `clay.` prefix: `shell.*`, `editor.*`, `documents.*`, `workspace.*`,
      `runtime.*`, `language.*`, `controlCenter.*`, `ui.*`, `sdui.*`, `modes.*`,
      `commands.*`, `keybindings.*`, `theme.*`, `configuration.*`, `packages.*`,
      `syntax.*`, `parse.*`, `decorations.*`, `diagnostics.*`, `behavior.*`,
      `completion.*`, … (e.g. `shell.clientSplitPaneVertical`,
      `runtime.reloadConfiguration`). Package-owned IDs keep the package
      prefix (`markdown.togglePreview`, `settings.open`). Surviving `clay`
      prefixes: `clay:` import specifiers, `@clay/*` package specifiers,
      `package.json` `clay.*` manifest key paths (`clay.apiPrefix`,
      `clay.contributions.*`, `clay.editorControl.modes`, `clay.performance.*`,
      `clay.extensionPoints`), and Rust `manifest.clay.<field>` struct access.
    - Performance: Pure identifier rename; no runtime behavior change.
    - Code Quality: One mechanical codemod pass plus targeted validation
      edits; doc registry validation enforces the bare-domain convention for
      new API docs (`src/docs/registry.rs`).
    - Security: Namespace ownership stays unambiguous — first segment names
      the owner (core domain or package prefix); package contribution
      validation still rejects `clay.*` claims.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `src/packages/manifest.rs` (`is_valid_api_prefix`),
        `src/packages/record.rs` (reserved-namespace checks),
        `src/docs/registry.rs` (`validate_entry`).
    - Options Considered:
      - Rename only command IDs, keep `clay.*` JS-API stable IDs: rejected —
        mixed spelling defeats the core-vs-package distinction the change
        exists to create.
      - Rename `clay:` import specifiers too: rejected — the `clay:` scheme
        is the import-map brand distinguishing built-in modules from package
        specifiers (`@clay/markdown`); users do not type it as a dotted ID.
      - Full rename with manifest-key-path exemptions: chosen.
    - Chosen Approach:
      - Regex codemod `clay\.<domain>.` → `<domain>.` over `src`, `runtime`,
        `packages`, `tests`, `benches`, `docs`, `examples`, `test-plan`,
        `tools`, `scripts`, `windows`, and root Markdown, with: lookbehind
        excluding `manifest.clay.<field>`/variable access, an exclusion list
        for package.json manifest key paths, and `packages/markdown/dist/load.js`
        excluded (uses a local `const clay` alias object — verified it
        contains no `clay.*` string literals). Historical docs
        (`decision-logs/`, `plans/`, `code-reviews/`) intentionally untouched.
      - Regenerated bundled-manifest FNV-1a fingerprints in
        `src/packages/bundled.rs` after the package.json edits.
    - API Notes and Examples:
      ```text
      shell.clientSplitPaneVertical   (was clay.shell.clientSplitPaneVertical)
      runtime.reloadConfiguration     (was clay.runtime.reloadConfiguration)
      configuration.loadConfigurationModule (stable ID, was clay.configuration.*)
      markdown.togglePreview          (package-owned; unchanged)
      ```
    - Files to Create/Edit:
      - 351 files across the tree (codemod); `src/packages/bundled.rs`
        fingerprints; `src/docs/registry.rs` validation rule.
    - References:
      - User instruction (this phase): verbosity flaw report and
        `<package>.<name>` package convention.
  - Test Cases to Write:
    - Full `cargo test` run: every assertion referencing renamed IDs updated
      by the codemod; doc-registry tests validate the new ID rule.
    - `node --check examples/init.js` passes with renamed command IDs.

- [x] Reserve core API domains against third-party `apiPrefix` squatting
  - Acceptance Criteria:
    - Functional: `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs`
      lists the core domains; manifest validation rejects a third-party
      package whose `clay.apiPrefix` claims one (e.g. a third-party `shell`
      package) while bundled first-party packages from the compiled inventory
      stay exempt (`@clay/git` keeps `git`).
    - Performance: Const lookup at manifest validation only.
    - Code Quality: Single const + one predicate
      (`is_reserved_core_domain_for_package`); existing `is_package_owned_id`
      contribution checks unchanged.
    - Security: Closes the post-rename squatting vector where a third-party
      package prefix could become indistinguishable from a core domain;
      exemption derives from the pinned bundled inventory, never `@clay/*`
      naming.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/manifest.rs` validation flow,
        `src/packages/bundled.rs` (`bundled_entry`, trust domains decision).
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
    - Options Considered:
      - Reserve only lowercase prefix-shaped domains: camelCase domains
        (e.g. `controlCenter`) can never be valid apiPrefixes; included anyway
        so the list doubles as the canonical core-domain registry for the doc
        registry validation.
      - Universal rejection without bundled exemption: rejected — breaks
        `@clay/git` (core git JS APIs share the `git` domain).
    - Chosen Approach:
      - Exemption keyed on `bundled_entry(package_name).is_some()` per the
        trust-domain decision (compiled inventory + fingerprint provenance).
    - API Notes and Examples:
      ```rust
      if is_reserved_core_domain_for_package(&api_prefix, &package_name) {
          // reject: InvalidPrefix
      }
      ```
    - Files to Create/Edit:
      - `src/packages/manifest.rs`: const + predicate + validation branch.
    - References:
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
  - Test Cases to Write:
    - Third-party manifest with `apiPrefix: "shell"`/`"editor"` rejected with
      InvalidPrefix naming the reserved domain. (covered by manifest tests;
      full suite run)
    - Bundled `@clay/git` manifest still validates (existing bundled-inventory
      tests).

- [x] Update agent skills and project patterns for the naming convention
  - Acceptance Criteria:
    - Functional: `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      documents the bare-domain rule, the owner-segment rule, reserved-domain
      list location, and the `clay:`/manifest-key exemptions;
      `configuration-system.md` carries the convention for configuration
      work; `create-plan/references/clay.md` requires the convention in the
      Clay JS API plan task; `project-wiki/SKILL.md` forbids the retired
      `clay.<domain>.*` spelling in wiki prose; stale pattern lines
      (`mode-primitive-first.md`, `authority-boundaries.md`,
      `extensions-and-ai.md`) use renamed IDs.
    - Performance: No runtime work.
    - Code Quality: Convention stated once authoritatively (naming pattern)
      and referenced elsewhere.
    - Security: Pattern restates that reserved-domain enforcement derives
      from the bundled inventory.
  - Approach:
    - Documentation Reviewed:
      - The four skill files listed above.
    - Options Considered:
      - New pattern file: rejected — naming pattern file already exists.
      - In-place updates: chosen.
    - Chosen Approach:
      - Edit `clay-js-api-naming.md` as the canonical statement; reference it
        from the configuration pattern, plan requirements, and wiki skill.
    - API Notes and Examples:
      ```text
      .agents/skills/project-patterns/references/clay-js-api-naming.md
      ```
    - Files to Create/Edit:
      - The four skill/reference files above.
    - References:
      - User instruction (this phase): keep AI agents on the convention.
  - Test Cases to Write:
    - None (agent-instruction docs); reviewer confirms consistency.

- [x] Add fault-isolated optional configuration modules to `loadConfigurationModule`
  - Acceptance Criteria:
    - Functional: `loadConfigurationModule({ path, optional: true })` catches
      module resolution/parse/evaluation failures, records a
      `configuration.module_failed` warning diagnostic naming the
      root-relative path, and resolves `{ loaded: false, error }` so
      evaluation continues; `optional` omitted/false keeps today's
      fail-the-evaluation behavior; success resolves `{ loaded: true }`.
    - Performance: No work added outside startup/reload evaluation; error
      messages bounded (e.g. 1 KiB) before storage.
    - Code Quality: Failures collected on `ConfigurationRuntime` (mirroring
      `loaded_modules`), drained into the evaluation/reload diagnostics so CLI
      and clients see them through the existing `runtime.*` fanout; no
      swallow-all anywhere else in init.js semantics.
    - Security: Containment validation (`validate_module_path`) runs BEFORE
      the optional catch and permits only an in-root missing final path for
      optional imports — escaping paths still hard-fail; diagnostics use
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
      - Extend the facade: validate through the existing op with an explicit
        optional flag, `try { await import } catch` only when `optional === true`,
        record through the runtime, and return a status object. Rust adds a
        bounded `module_errors: Mutex<VecDeque<…>>` on `ConfigurationRuntime`
        plus a recording op; `evaluate_loaded_module` drains it into
        `ClayRuntimeEvaluation.configuration_diagnostics`, and the server
        records those warnings in reload/bootstrap diagnostics.
    - API Notes and Examples:
      ```js
      // runtime/js/configuration.js — facade shape after change:
      const result = await loadConfigurationModule({
          path: "./packages/first-party.js",
          optional: true,
      });
      // result: { loaded: true } or { loaded: false, error: string }
      // The path op validates root containment before import is attempted.
      ```
    - Files to Create/Edit:
      - `runtime/js/configuration.js`: optional-mode facade + return status.
      - `runtime/js/configuration.d.ts`: optional option and result union.
      - `src/server/configuration.rs`: `module_errors` store, bounded record
        method, optional missing-path containment validation, drain accessor.
      - `src/server/ops/configuration.rs`: optional-aware validation and new
        `op_clay_configuration_record_module_error` (bounded inputs).
      - `src/server/ops/mod.rs`: trusted-op registration and strict-subset
        inventory.
      - `src/server/js_runtime.rs`: evaluation diagnostic assembly and runtime
        tests.
      - `src/server/mod.rs`: bootstrap/reload diagnostic retention and test.
      - `src/server/connection.rs`: return optional reload warnings to the
        initiating client instead of hiding them behind success.
      - `tests/primitives_docs.rs` and
        `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`:
        trusted-op inventory count/list.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/wiki/modules/configuration-runtime.md`
      - `src/server/ops/mod.rs` trusted/package extension inventories
  - Test Cases to Write:
    - Optional syntax-error module: `configuration_optional_module_failure_isolated_and_reported`; evaluation succeeds, subsequent init.js statements apply, and the root-relative warning is returned.
    - Optional missing module: `configuration_optional_missing_module_failure_isolated_and_reported`; same isolation behavior.
    - Required broken module: `configuration_required_module_failure_still_fails_evaluation`; required failure remains fatal and reload preservation stays covered by the existing generation test.
    - Optional path escape: `configuration_optional_module_path_escape_still_fails_before_catch`; containment precedes isolation.
    - Message over 1 KiB: `module_error_storage_is_relative_bounded_and_drained`; stored detail is capped at 1 KiB and drained once.
    - Successful reload with optional warning: `optional_configuration_module_warning_survives_successful_reload`; warning enters reload outcome and runtime diagnostic retention.

- [x] Watch the configuration root and auto-reload through the serialized reload service
  - Acceptance Criteria:
    - Functional: When the server runs with an effective configuration root, a
      background task detects created/modified/deleted `.js` files (and
      `preferences.json`) anywhere under the root and triggers
      `reload_runtime_generation()` after a quiet-period debounce; no reload
      loops (a completed reload re-baselines the snapshot); watcher starts only
      when an effective root exists and stops with the server. The effective
      root is the explicit `ServerConfig.configuration_root`, or the existing
      `ConfigurationRuntime::default_config_root()` when the default
      `~/.config/clay/init.js` is present.
    - Performance: Poll interval ~1 s with
      `MissedTickBehavior::Skip`; scan bounded (≤ 256 files, depth ≤ 8,
      skip dotfiles/tmp); zero work on editor hot paths; debounce collapses
      edit storms into one reload.
    - Code Quality: Single small module; reuses
      `IpcServer::reload_runtime_generation` (already serialized by the
      `reload_attempt` lock — a watcher trigger during a manual reload yields
      the existing `runtime.reload_in_progress` outcome, no new
      coordination). `record_runtime_diagnostic` retains diagnostics in the
      bounded store and sends them through the generic connection-scoped
      `OutputRouter<RuntimeDiagnostic>` fanout; no watcher-specific client
      message path is added.
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
        Reload diagnostics flow through the generic server diagnostic router:
        `RuntimeDiagnosticStore` retains them for bootstrap and
        `OutputRouter<RuntimeDiagnostic>` broadcasts them to live clients.
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
      - `src/server/config_watch.rs`: new bounded watcher task (~100 lines).
      - `src/server/mod.rs`: route `record_runtime_diagnostic` through the
        server-level diagnostic store/router, spawn the watcher in
        `IpcServer::run` when an effective root exists, and handle shutdown.
      - `src/server/connection.rs`: subscribe each connection to live
        runtime diagnostics and select the bounded output lane; keep
        `RuntimeDiagnosticStore` for bootstrap/reconnect retention.
      - `src/main.rs`: no change expected (root already threaded through).
    - References:
      - `src/server/mod.rs:926` (`reload_runtime_generation`),
        `src/server/mod.rs:889` (`trigger_developer_hot_reload` precedent)
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - `configuration_watcher_reloads_changed_root_without_command_intent`:
      modify `init.js` and verify a new generation and configuration result.
    - `configuration_watcher_preserves_generation_on_failure_and_recovers`:
      introduce a syntax error, verify the old generation and diagnostic, then
      fix the file and verify recovery.
    - `configuration_watcher_detects_new_optional_module`: create a new module
      under the root, import it through the existing optional loader, and
      verify the watcher reloads it.
    - `watcher_debounces_rapid_saves_into_one_reload`: rapid successive saves
      produce exactly one callback after the quiet period.
    - `watcher_detects_new_and_deleted_watched_files`: creation and deletion
      both change the bounded snapshot.
    - `server_sends_runtime_diagnostics_after_bootstrap`: a reload diagnostic
      reaches a connected client once and remains available in bootstrap
      retention.
    - Existing `concurrent_reload_commands_commit_at_most_one_candidate_at_a_time`
      covers a watcher-equivalent trigger during an in-flight reload; no
      second reload is queued.
    - Effective-root path review: explicit roots are canonicalized directly;
      the default-root branch checks for `~/.config/clay/init.js` before
      spawning.

- [x] Ship a default keybinding for configuration reload
  - Acceptance Criteria:
    - Functional: `Ctrl+Shift+R` (global scope) binds
      `runtime.reloadConfiguration` in `default_keymaps()`; users can
      still `unbindKey`/`bindKey` to override (last-wins overlay semantics
      unchanged); Control Center listing shows the chord.
    - Performance: No runtime cost (static rule entry).
    - Code Quality: Replaces the old no-default pinning test with
      `configuration_default_reload_binding_is_present_and_overridable`;
      built-in command metadata exposes the same chord to Control Center; the
      example config re-declares the chord idempotently like other shipped
      defaults.
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
      - Add one `KeyBindingRule::default_reload_configuration` global entry
        with `ServerFirstWithLock { lock_scope: Behavior }`; reuse that same
        rule in built-in command metadata so Control Center displays the
        shipped chord. Note the editor-scope `clientRequestResync` doc example
        shares the chord in a different scope (scopes resolve independently;
        call it out in docs).
    - API Notes and Examples:
      ```rust
      // src/protocol/mod.rs::default_keymaps()
      KeyBindingRule::default_reload_configuration(),
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: default rule and routing pinning test.
      - `src/server/command_execution.rs`: expose the same default in built-in
        command metadata for Control Center.
      - `src/server/control_center.rs`: verify the listing displays the chord.
      - `src/server/js_runtime.rs`: replace the no-default-binding test.
      - `tests/command_execution.rs`: update built-in reload metadata assertions.
      - `examples/init.js`: idempotent re-declaration + comment.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`,
        `docs/reference/clay-js-api/configuration.md`,
        `docs/reference/clay-js-api/commands/server-register-command.md`, and
        `docs/reference/packages/creating-packages.md`: default/override docs.
      - `docs/wiki/modules/{configuration-runtime,command-registry,persistent-runtime-hot-reload}.md`
        and the Phase 19 primitive review: implementation notes.
    - References:
      - `src/server/ops/keybindings.rs:429` (routing policy),
        `src/server/command_execution.rs:695`
  - Test Cases to Write:
    - `default_keymaps_contain_configuration_reload_binding`: fresh startup
      contains global `Ctrl+Shift+R` with the behavior lock routing policy.
    - `configuration_default_reload_binding_is_present_and_overridable`:
      `unbindKey("Ctrl+Shift+R", { scope: "global" })` removes the default;
      `bindKey` on another chord wins while preserving server-first locking.
    - `control_center_includes_built_in_commands`: the reload item detail
      displays `Ctrl+Shift+R`.
    - `reload_command_is_server_first_behavior_locked_and_discoverable`:
      built-in metadata exposes exactly that global chord with no permissions.
    - `package_javascript_cannot_directly_execute_reload_command`: package
      JavaScript remains rejected (existing test keeps passing).

- [x] Restructure the canonical example configuration into base / first-party / third-party modules
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
      - Implemented exactly as planned: `examples/init.js` renumbered to 10
        sections with the grants/packages content moved verbatim into
        `examples/packages/first-party.js` (grant-before-loadPackage ordering
        preserved inside that module, stated in its header and in init.js
        section 10), `examples/packages/third-party.js` is a commented
        template referencing the `clay package add`/`adopt` host CLI flow,
        and `examples/init.js` section 1 now imports
        `loadConfigurationModule`/`getConfigurationState` for real and the
        tail loads both modules with `optional: true`.
      - Copy instructions updated to copy the tree
        (`cp -r examples/. ~/.config/clay/`) in `docs/reference/.../configuration.md`,
        `test-plan/index.md`, and `test-plan/02-configuration-init-js.md`
        (setup + new rows C9–C11 for broken/missing package modules; rows
        renumbered C12–C14 for live reload); section references in
        `test-plan/09` and `test-plan/13` adjusted to the new numbering.
      - `src/server/mod.rs` gained `temp_example_config_root` (copies the
        whole examples tree) and the existing
        `example_configuration_loads_cleanly_and_applies_effects` test now
        boots from the tree, asserts no `configuration.module_failed`
        diagnostics, asserts a behavior manifest from the package loads, and
        cleans up; new test
        `example_configuration_survives_broken_package_module` breaks
        `packages/first-party.js` and asserts reload still succeeds, the
        diagnostic is recorded, and base typography still applies.
    - API Notes and Examples:
      ```js
      // examples/init.js (tail)
      // Package configuration is segregated and fault-isolated: a broken
      // module records configuration.module_failed and never blocks
      // the base configuration above or app launch.
      await loadConfigurationModule({ path: "./packages/first-party.js", optional: true });
      await loadConfigurationModule({ path: "./packages/third-party.js", optional: true });
      ```
    - Files to Create/Edit:
      - `examples/init.js`: slim to base config + module imports (DONE).
      - `examples/packages/first-party.js`: new (sections 2–3 content) (DONE).
      - `examples/packages/third-party.js`: new (commented template) (DONE).
      - `README.md` / docs referencing "copy examples/init.js": copy-the-tree
        instructions (tentative list; grep for references during task) —
        updated `docs/reference/clay-js-api/configuration.md`, `test-plan/index.md`,
        `test-plan/02-configuration-init-js.md`; other references (bind-key.md,
        wiki modules) name the file for Ctrl+B/defaults and remain accurate.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
  - Test Cases to Write:
    - `node --check` passes on all three example files (DONE).
    - Copy tree to a temp config root, boot server fixture: base config
      applied, first-party packages loaded, no diagnostics —
      `example_configuration_loads_cleanly_and_applies_effects` (DONE).
    - Delete/break `packages/first-party.js`: app launches, base config
      active, `configuration.module_failed` diagnostic recorded —
      `example_configuration_survives_broken_package_module` (DONE).
    - Existing doc tests referencing `examples/init.js` content updated and
      passing (Ctrl+B count assertion still passes verbatim).

- [x] Define and verify the package default init.js loading experience under the new structure
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
      - Implemented: `example_configuration_loads_cleanly_and_applies_effects`
        now asserts the shipped tree produces the same package outcomes as a
        flat init.js (markdown JS parse handler; rust/typescript/javascript/
        markdown syntax grammars; all handlers `@clay/*`-prefixed; zero
        `packages.*` diagnostics — the fully commented third-party template
        causes no package activity). New test
        `alternate_configuration_layout_loads_identical_packages` drives
        `init.js → loadConfigurationModule("./a/b.js") → static import
        "./c.js" → one-line loadPackage` in each module and asserts identical
        parse-handler and syntax-grammar outcomes. `docs/reference/packages/
        creating-packages.md` gained an "Any local module layout works"
        block: the examples/ tree is a convention, not a requirement; module
        indirection does not change loads, ordering, or diagnostics.
    - API Notes and Examples:
      ```js
      // any file, any folder depth within ~/.config/clay:
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `tests/package_loading*.rs` or `src/server/js_runtime.rs` tests:
        layout matrix coverage (tentative location; match existing fixtures)
        — DONE in `src/server/mod.rs` runtime_generation_tests
        (`alternate_configuration_layout_loads_identical_packages` + the
        extended shipped-tree assertions).
      - `docs/reference/packages/creating-packages.md`: configuration-layout
        section update (DONE).
    - References:
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
  - Test Cases to Write:
    - Shipped layout: all `@clay/*` one-line loads succeed from
      `packages/first-party.js` — DONE (parse handlers + syntax grammars +
      no `packages.*` diagnostics).
    - Alternate layout (`./a/b/c.js` chain of static imports): identical
      package outcomes — DONE (`alternate_configuration_layout_loads_identical_packages`).
    - Third-party template file left fully commented: zero third-party
      activity on boot — DONE (all handlers `@clay/*`-prefixed, no
      `packages.*` diagnostics in the shipped-tree test).

- [x] Create or verify Clay JS APIs for public programmatic surfaces
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
      - Implemented: `configuration/load-configuration-module.md` rewritten
        for the `optional` option and `{ loaded, error? }` result (Options,
        Custom properties, Return and async behavior, Errors,
        Permissions and security, Lookup metadata; containment validation
        runs before the optional catch, 1 KiB error truncation,
        `configuration.module_failed` bounded warning); `api-inventory.toml`
        custom_properties now `["path:string", "optional:boolean"]`;
        `configuration.md` Phase 19 gained an "Automatic configuration-root
        watch (server behavior, no JS API)" subsection (polling watcher,
        quiet-period debounce, failed reloads keep previous generation, no
        `watch*` API — no hidden keys); default chord, explicit-binding
        override, and three-component convention were already present from
        the keybinding and example-restructure tasks; bind-key.md and
        server-register-command.md already carry the shipped global chord.
        No new JS API was needed.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test clay_js_doc_registry --test clay_js_api_inventory
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration/load-configuration-module.md`: `optional`, return shape, isolation semantics. (DONE)
      - `docs/reference/clay-js-api/configuration.md`: watcher + default chord + three-component convention. (DONE)
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: revise "no default chord" Phase 19 note. (DONE in keybinding task)
      - `docs/reference/clay-js-api/api-inventory.toml`: custom properties for changed APIs. (DONE)
      - `docs/index.md`: link updates if any. (unchanged — link already present)
      - Generated registry artifacts via the update command. (DONE)
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
  - Test Cases to Write:
    - Registry-current test passes after regeneration. (DONE — `generated_registry_is_current`)
    - Inventory test covers `optional` custom property on
      `loadConfigurationModule`. (DONE — added
      `by_custom_property("optional")` assertion to
      `configuration_module_loading_is_runtime_backed_no_external_authority`
      in `tests/clay_js_doc_registry.rs`; all 50 protocol registry/inventory
      tests pass)

- [x] Create or verify Clay configuration APIs
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
      - Implemented: appended "Phase 23 configuration structure and auto-reload
        review" to `docs/reference/clay-js-api/configuration.md` in the
        established style (What changed / Configuration surfaces / Rejected
        hidden configuration keys / Security / Performance). It documents the
        three shipped surfaces — modular structure via `loadConfigurationModule`
        (`optional: true`, examples/ tree as convention, any layout works),
        automatic config-root watch as server behavior with no `watch*` API,
        and the global `Ctrl+Shift+R` reload chord with override/unbind —
        names the rejected watcher-toggle option (YAGNI, arrives as a
        documented API if ever needed), states the no-new-authority security
        position, and pins the startup/reload-only performance contract with
        the watcher scan bounds. New test
        `configuration_rejects_watcher_control_keys` in
        `src/server/configuration.rs` proves `core.watch.intervalMs` /
        `core.watch.debounceMs` / `core.watch.enabled` all fail closed with
        `unsupported package option` on the closed allowlist.
    - API Notes and Examples:
      ```text
      docs/reference/clay-js-api/configuration.md#phase-23-configuration-structure-and-auto-reload-review
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: review section. (DONE)
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Attempt `setPackageOption` with watcher keys (e.g.
      `core.watch.intervalMs`) → rejected by the closed allowlist (extend the
      existing rejection test pattern in `src/server/configuration.rs`).
      (DONE — `configuration_rejects_watcher_control_keys` passes for all
      three watcher keys)

- [x] Execute and update the manual test plan (test-plan/)
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
      - Implemented: module 02 setup now names the repeatable fixture
        `tests/fixtures/configuration/plan080-manual/` (verbatim `examples/`
        tree: init.js + packages/first-party.js + packages/third-party.js)
        with the exact headless server command; C11 reworded (missing
        optional module records `configuration.module_failed`, no fatal
        failure); new C12 boot-time broken-module isolation row; live-reload
        table renumbered C13–C19 with three new watcher rows: C16 watcher
        auto-reload on a plain init.js save (no reload key), C17 debounce
        sanity (5 rapid saves → 1 reload), C18 invalid-bindKey watcher
        reload failure preserving the previous generation, C19 default
        `Ctrl+Shift+R` chord + Control Center listing; negative checks
        extended to the watcher (bounded polling, never on keypress/paint,
        no new authority). A "Recorded results (Linux, 2026-08-11)"
        section records the real-binary run.
      - Execution (real Linux debug build, `target/debug/clay server
        /tmp/clay-ipc/clay-plan080.sock --config-fixture plan080-manual`):
        C1/C2 PASS (clean startup, zero diagnostics); C3 PASS via watcher
        (`keybindings.unknown_command` reload failure, previous generation
        preserved); C9 PASS (broken first-party.js → `configuration.module_failed`,
        server alive); C10 PASS (fix → clean reload); C11 PASS (missing
        optional module → warning only); C12 PASS (boot with broken module:
        server starts, warning recorded); C16 PASS (valid init.js edit →
        auto-reload ≤ ~2 s); C17 PASS (5 rapid writes → exactly ONE reload);
        C18 PASS (invalid bindKey → failed reload, fix → clean). GUI steps
        C4–C8/C13–C15/C19 recorded NOT RUN headless and mapped to the
        automated integration tests that cover them. `test-plan/index.md`
        row 02 scope updated (watcher auto-reload, default reload chord,
        fixture path).
    - API Notes and Examples:
      ```bash
      cp -r examples/. ~/.config/clay/   # new setup shape
      ```
    - Files to Create/Edit:
      - `test-plan/02-configuration-init-js.md`: updated + new steps. (DONE)
      - `test-plan/index.md`: row 02 scope + fixture reference. (DONE)
      - `tests/fixtures/configuration/plan080-manual/`: repeatable fixture
        (verbatim copy of the examples/ tree). (ADDED)
      - `test-plan/index.md`: matrix touch-up if needed.
    - References:
      - `docs/development/` deep references where applicable.
  - Test Cases to Write:
    - Manual steps as above; results recorded in the file per convention.

- [x] Linux platform verification gate
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
      - Executed 2026-08-11 on the final tree (debug profile):
        `cargo fmt --check` clean; `cargo check --all-targets` clean;
        `cargo clippy --all-targets -- -D warnings` clean (zero warnings);
        `cargo test` green: lib 1388 passed / 0 failed / 2 ignored, editor
        158, protocol 149, runtime 197, security 125, plus 62 aggregated
        integration — 0 failures. Zero fallout, so no files needed fixes.
        Perf-invariant, trust-domain, containment, and authority suites are
        inside those green targets (e.g. security suite incl.
        package_manifest_rejects_third_party_api_prefix_squatting_core_domain;
        runtime suite incl. reload/keybinding/concurrent-reload invariants).
        Supplementary: `git diff --check` clean and `node --check` passes on
        examples/init.js, examples/packages/first-party.js, and
        examples/packages/third-party.js.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && \
      cargo clippy --all-targets -- -D warnings && cargo test
      ```
    - Files to Create/Edit:
      - Any file needing lint/format fixes. (none needed)
    - References:
      - `AGENTS.md`
  - Test Cases to Write:
    - The gate itself is the check. (DONE)

- [x] Update or verify the code wiki after implementation
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
      - Implemented: `docs/wiki/modules/configuration-runtime.md` gained a
        canonical-`examples/`-tree paragraph in How It Works (three-component
        structure as convention not requirement, `packages/first-party.js`
        grants + one-line loads, commented third-party template, host-CLI
        adoption, `tests/fixtures/configuration/plan080-manual/` verbatim
        fixture + headless `--config-fixture` command), and its Tests
        section now names the three example-tree integration tests
        (`example_configuration_loads_cleanly_and_applies_effects`,
        `example_configuration_survives_broken_package_module`,
        `alternate_configuration_layout_loads_identical_packages`) plus
        `configuration_rejects_watcher_control_keys` (watcher has no hidden
        tuning keys). `docs/wiki/index.md` Configuration Runtime bullet
        extended with the modular `examples/` tree convention, default
        global `Ctrl+Shift+R` chord, and fixture. Watcher/optional-module/
        chord content already present from earlier per-task updates
        (configuration-runtime.md, persistent-runtime-hot-reload.md,
        phase19-hot-reload-behavior-update-primitive-review.md). All pages
        use bare `<domain>.<name>` core IDs; no retired `clay.*` spellings
        introduced. Manual review confirms the master index links every
        relevant page and updated pages explain what the changed code does
        and how it works, with source/test paths and invariants.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/configuration-runtime.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas. (DONE)
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code. (DONE — configuration-runtime.md; watcher/optional-module/chord pages already current)
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works. (DONE)

## Compromises Made

- **Polling watcher instead of event-driven `notify`** (accepted, task 4):
  the config-root watcher polls at 1 s with a 300 ms quiet debounce rather
  than depending on the `notify` crate. Cost: up to ~1 s latency before an
  edit auto-applies, and a tiny bounded per-second directory scan (≤ 256
  files, depth ≤ 8). Benefit: zero new dependency, no inotify/watch
  lifecycle complexity, deterministic behavior on every platform the
  server runs on. Upgrade path: `notify`-based watching if poll latency or
  battery cost ever matters.
- **Possible extra idempotent reload after settings-panel preference
  persists** (accepted, task 4): `preferences.json` self-reloads may cause
  the watcher to fire one extra idempotent reload (settings writes the file
  and the watcher notices it). Harmless — the reload is a no-op state-wise
  — and documented with a `ponytail:` comment naming a server-side
  write-suppression-window upgrade path.
- **Watcher tuning stays compiled constants** (accepted, configuration
  APIs task): interval, debounce, and enable/disable are not configurable;
  `core.watch.*` keys fail closed on the setPackageOption allowlist.
  Rejected as YAGNI; a documented API can arrive later if users ask.
- **Three-component example layout is a convention, not a requirement**
  (accepted, example-restructure task): the shipped `examples/` tree splits
  into base `init.js` + `packages/first-party.js` + `packages/third-party.js`,
  but any local module layout works and is regression-tested
  (`alternate_configuration_layout_loads_identical_packages`).
- **Manual test plan GUI steps not run headless** (accepted, test-plan
  task): server-side steps (watcher, debounce, isolation, boot) were
  executed on the real Linux binary with pass/fail recorded; GUI steps are
  covered by automated integration tests and flagged NOT RUN headless for
  a desktop session.

## Further Actions

- **User-facing watcher toggle** (if requested): interval/debounce/
  enable-disable arrive as a fully documented Clay JS API through the
  schema, never a hidden key. Low priority — the watcher grants no
  authority, so there is no safety reason to disable it.
- **Event-driven watching via `notify`** (if poll latency or battery cost
  ever matters): replaces the bounded polling task with inotify/ReadDirectoryChangesW
  events while keeping the same quiet-period debounce and re-baseline
  semantics. Medium priority, currently unnecessary.
- **Per-module reload granularity** (if large config trees grow): today any
  change reloads the whole runtime generation; a per-module cache or
  partial-evaluation path could avoid re-running unrelated modules. Low
  priority — reloads are startup-only work and fast in practice.
- **Doc-registry refresh for loadConfigurationModule**: the `optional`
  custom property and `{ loaded, error? }` return shape are now in the
  inventory and generated registry (DONE in the Clay JS APIs task); keep
  them in sync whenever the facade changes.
