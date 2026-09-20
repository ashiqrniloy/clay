# Plan 141 — Config Discoverability: help.lookup, Palette Help Rows, openConfigurationFile

Source: the 2026-09-20 configurability review, deviation D4. Clay compiles a
full Clay JS documentation registry (`docs/generated/clay-js-api-registry.json`,
`src/docs/registry.rs` — with `by_id`, `by_user_facing_name`,
`by_lookup_tag`, `by_key_binding` lookups) explicitly "for future app/help/
agent discovery", but nothing consumes it at runtime: no `apropos`, no
in-app help surface, and the Emacs self-documentation pillar is absent.
Likewise there is no command that opens the user's own `init.js` — users
must find the Path Browser themselves. The registry also cannot serve the
agent (plan 139's skill points at "live lookup when available").

Binding prior decisions:

- `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`:
  `user_facing_name` + lookup tags exist for "help, command search,
  configuration UIs, and AI-agent discovery" — this plan is that decision's
  runtime half.
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`:
  public programmatic behavior = Clay JS APIs over explicit ops.
- `decision-logs/2026-08-11-1711-command-centre-surface-path-mode-and-sequence-keybindings.md`:
  one shared surface for pickers; new session kinds, not new overlay
  systems; built-in browse→SingleFile grant precedent for file opening.
- `decision-logs/2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md`:
  init.js is the edit target; watcher reloads it.

Roadmap position: completes the self-documenting pillar for users and
agents; feeds plan 139's skill with non-drifting lookup once exposed to the
daemon (recorded there as a further action).

## Objectives

- A `help` core API domain with a read-only `help.lookup` facade
  (`clay:help`) backed by the compiled registry: query by free text (IDs,
  user-facing names, lookup tags, key bindings), bounded result rows with
  stable IDs, docs paths, and custom-property summaries.
- A Command Centre help session kind: typing `? <terms>` (exact trigger
  fixed by the control-centre task) in the composer palette lists matching
  APIs/commands with their key bindings; selecting an entry shows its
  summary and opens its docs page path (and can invoke `help.lookup` for
  details). Rendered entirely by the existing shared transient-menu
  surface — no new overlay system.
- A `shell.openConfigurationFile` command (ServerFirst, no default binding)
  that opens `~/.clay/init.js` in the active editor pane through the
  existing SingleFile-grant document path, creating the file from the
  canonical example template when absent.
- Registry consumption is read-only and hot-path-free; the generated JSON
  stays the single source (no second registry).

## Expected Outcome

- A user can discover every configurable surface without leaving the app:
  `? theme` in the palette lists the theme APIs with bindings; running
  `shell.openConfigurationFile` (via palette search) drops them into their
  init.js with the watcher making edits live.
- `help.lookup("keybinding:ctrl+shift+r")`-style queries work from trusted
  JS and are documented for agent use; results cite docs paths that exist.
- All gates green; drift between docs and runtime lookup fails `cargo test`
  via the existing registry gates.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: seven-stage gate results recorded; inventory note that
      `src/docs/registry.rs` has no server-runtime consumer today (grep
      evidence) and no `help` reserved domain exists
      (`src/packages/manifest.rs:522`).
    - Performance: registry size + parse cost recorded (it becomes a
      runtime-loaded artifact).
    - Code Quality: baseline before edits.
    - Security: lookup is read-only; no trust change.
  - Approach:
    - Documentation Reviewed: AGENTS.md platform validation;
      planning-checklist.md.
    - Options Considered: skip — cheap and establishes the perf note.
    - Chosen Approach: gates + inventory evidence.
    - API Notes and Examples:
      ```bash
      rg -l "docs::registry|DocsRegistry" src   # expect tests/tooling only
      ```
    - Files to Create/Edit: `code-reviews/<date>-plan141-baseline/README.md`.
    - References: `src/docs/registry.rs`.
  - Test Cases to Write: none (evidence capture).

- [ ] Review Clay JS API and UI catalog primitives before implementation
  - Acceptance Criteria:
    - Functional: inventory with paths — registry types and lookups
      (`src/docs/registry.rs:89-208`), generated-artifact embedding options
      (compile-time include vs read-on-first-use), the control-centre
      session-kind registry (`src/server/control_center.rs`),
      transient-menu snapshot budget, command catalogue +
      `CommandExecutor::execute_discovery` precedent (read-only ServerFirst,
      empty permissions), `documents.clientOpenFileDialog` / browse→grant
      flow for the open-config command, `RESERVED_CORE_API_DOMAINS`
      (`src/packages/manifest.rs:522`) for the new `help` domain.
    - Performance: lookup must be off typing/paint paths; palette query
      work bounded (result cap, prebuilt index at generation load).
    - Code Quality: no second registry structure; session kind reuses
      `TransientMenuSession`.
    - Security: read-only discovery mirrors `modes.listActiveModes` (no
      execution authority).
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/js-api.md`,
      `references/ui.md`, `references/components.md`,
      `references/protocol-perf.md`,
      `docs/wiki/modules/command-centre.md` (or nearest),
      `docs/wiki/modules/clay-js-doc-registry.md`.
    - Options Considered:
      - Client-side registry fetch (frontend parses JSON) — rejected: the
        contract is server-authoritative data; SDUI/menu sessions already
        push snapshots.
      - A separate help microservice/index — rejected: second source of
        truth.
    - Chosen Approach: server-side read-only lookup over the compiled
      artifact; one new session kind.
    - API Notes and Examples:
      ```rust
      pub const RESERVED_CORE_API_DOMAINS: &[&str] = &[ /* … */ "help", /* … */ ];
      ```
    - Files to Create/Edit:
      `code-reviews/<date>-plan141-baseline/primitive-review.md` (new).
    - References: decision `2026-05-08-1840`.
  - Test Cases to Write: none (review verified by later tasks).

- [ ] Implement the `help.lookup` op and facade
  - Acceptance Criteria:
    - Functional: `help.lookup({ query, tags?, limit? })` returns bounded
      entries (stable ID, user-facing name, module/export, key bindings,
      docs path, one-line summary, custom-property names) matching free
      text against IDs/names/tags/key bindings; registered only in the
      trusted extension; query cost bounded (cap default 20, index built at
      generation load, not per query).
    - Performance: index build at load/reload only; per-query work linear
      in index with early exit; no hot-path contact.
    - Code Quality: op wrapper + facade + `.d.ts` per the boundary rules;
      new Rust internals `pub(crate)`.
    - Security: read-only; no environment, path, or credential data in
      results; callable from user config (trusted) and, via plan 139's
      follow-up, the daemon — not from the third-party runtime unless a
      later decision adds it.
  - Approach:
    - Documentation Reviewed: primitive-review output; `references/js-api.md`.
    - Options Considered: expose raw registry JSON — rejected: schema churn
      becomes API.
    - Chosen Approach: typed lookup facade.
    - API Notes and Examples:
      ```js
      import { lookup } from "clay:help";
      const rows = await lookup({ query: "theme", limit: 10 });
      ```
    - Files to Create/Edit: `src/server/ops/help.rs` (new),
      `src/server/ops/mod.rs` (registration), `src/packages/manifest.rs`
      (`help` domain), `runtime/js/help.js` + `help.d.ts` (new).
    - References: decision `2026-05-08-1509`.
  - Test Cases to Write:
    - `help_lookup_finds_by_id_name_tag_and_keybinding`.
    - `help_lookup_results_bounded_and_cite_existing_docs_paths`.

- [ ] Add the palette help session kind
  - Acceptance Criteria:
    - Functional: a new Command Centre session kind (trigger and listing
      behavior fixed with the control-centre conventions — e.g. `?`
      prefix) queries `help.lookup`, renders rows through the existing
      transient-menu snapshot pipeline, shows key bindings inline, and on
      selection presents the entry summary + docs path (copyable), with
      invoke offered only for entries that are real commands; cancel/back
      behaves like other session kinds.
    - Performance: per-keystroke query bounded by the lookup cap; snapshot
      budgets enforced as for other sessions.
    - Code Quality: no new overlay system, no new component kinds — the
      shared session surface and catalog components only.
    - Security: session is read-only; invoking a listed command routes
      through normal command execution (no new authority).
  - Approach:
    - Documentation Reviewed: `references/ui.md`,
      `references/components.md`, `references/protocol-perf.md`,
      `src/server/control_center.rs`; approved artifact
      `design-artifacts/approved/composer-palette-stages/` (palette
      surface + states; help rows are a new session kind on that surface —
      coverage check in the UI-gate task below; no in-place artifact
      edits).
    - Options Considered: extend the command-list session with a mode flag —
      rejected: session kinds are the recorded extension unit.
    - Chosen Approach: new session kind over shared machinery.
    - API Notes and Examples:
      ```text
      ? theme
      theme.setTheme            Theme: Select Content Theme      (unbound)
      theme.setTypography       Typography: Set User Typography  (unbound)
      runtime.reloadConfiguration  Reload Configuration          Ctrl+Shift+R
      ```
    - Files to Create/Edit: `src/server/control_center.rs` (+ session
      kind), protocol snapshot types if the shared kind enum requires it,
      `frontend/src/control-center/*` as located (rendering only).
    - References: decision `2026-08-11-1711`.
  - Test Cases to Write:
    - `help_session_lists_entries_with_bindings`.
    - `help_session_selection_shows_summary_and_docs_path`.
    - `help_session_snapshot_within_budget`.

- [ ] Implement `shell.openConfigurationFile`
  - Acceptance Criteria:
    - Functional: ServerFirst command with empty permissions (discovery
      precedent) that resolves `<config-root>/init.js`, creates it from
      the canonical example template when absent (bounded copy, no secrets),
      records the SingleFile grant through the existing path, and opens it
      in the active editor pane; appears in palette search by its
      user-facing name; no default key binding.
    - Performance: one file resolve + open on explicit command.
    - Code Quality: reuses the browse→grant→open machinery; template ships
      as a checked-in asset (single source with `examples/config/init.js`
      documented relationship — either copy-on-write with a header comment
      or a build-time sync, decided in-task and recorded).
    - Security: command opens exactly the config-root init.js — no
      parameterized paths; template contains no credentials or environment
      assumptions.
  - Approach:
    - Documentation Reviewed: primitive-review output;
      `src/server/documents.rs` open path; decision `2026-08-11-1711`
      (SingleFile grant).
    - Options Considered: parameterize the path (`shell.openConfigFile(path)`) —
      rejected: parameterized host-file opening is a new surface needing
      its own decision; the fixed target is the user need.
    - Chosen Approach: fixed-target command.
    - API Notes and Examples:
      ```js
      // bindable like any command ID:
      keybindings.bindKey("Ctrl+Alt+,", "shell.openConfigurationFile");
      ```
    - Files to Create/Edit: `src/server/shell.rs` or command-execution
      module per inventory, command catalogue registration, docs page.
    - References: decision `2026-05-08-1841`.
  - Test Cases to Write:
    - `open_configuration_file_opens_existing_init_js`.
    - `open_configuration_file_seeds_template_when_absent`.

- [ ] UI prototype gate coverage check (prototype only if uncovered)
  - Acceptance Criteria:
    - Functional: verify `design-artifacts/approved/composer-palette-stages/`
      covers the palette surface including a help/session-kind listing
      state and narrow layout; if the help rows' states (long doc paths,
      many results, empty result, unbound/bound badge variants) are not
      covered, build the missing pages under
      `design-artifacts/prototypes/help-discovery/` and freeze approval
      into `design-artifacts/approved/help-discovery/` before the palette
      task's implementation is considered complete; record the coverage
      decision with artifact paths.
    - Performance: n/a.
    - Code Quality: no in-place approved-artifact edits.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → UI
      Prototype and Explicit User Approval Task.
    - Options Considered: assume coverage — rejected: session-kind content
      rows are a new state set on a shipped surface.
    - Chosen Approach: coverage audit, prototype the uncovered set only.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: coverage note under
      `code-reviews/<date>-plan141-baseline/`; conditional prototype +
      approved slugs.
    - References: user instruction 2026-09-11.
  - Test Cases to Write: none (gate evidence).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: `help.lookup` and `shell.openConfigurationFile` have
      schema-complete docs (stable IDs `help.lookup`,
      `shell.openConfigurationFile`; user-facing names; key bindings `[]`
      + documented bindability; `custom_properties` for `query`, `tags`,
      `limit`; security notes; lookup tags); master index + generated
      registry updated; drift fails `cargo test`.
    - Performance: n/a.
    - Code Quality: naming per `references/js-api.md` (`help` domain added
      to the reserved list, first segment ownership respected).
    - Security: docs state read-only boundaries.
  - Approach:
    - Documentation Reviewed: `references/js-api.md`;
      `docs/reference/clay-js-api/schema.md`.
    - Options Considered: put lookup under `runtime.*` — rejected: a
      `help` domain is cleaner ownership and the reserved list is additive.
    - Chosen Approach: two doc pages + inventory entries.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      ```
    - Files to Create/Edit: `docs/reference/clay-js-api/help/lookup.md`,
      `docs/reference/clay-js-api/shell/open-configuration-file.md`,
      `api-inventory.toml`, `docs/index.md`, regenerated registry JSON.
    - References: decisions `2026-05-08-1509`, `2026-05-08-1840`.
  - Test Cases to Write: registry coverage gates for the new entries.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: no new configuration keys; the new command is bindable
      through the documented `bindKey` path and the example config shows it
      commented; `help.lookup` itself documented as agent/config-callable.
    - Performance: n/a.
    - Code Quality: no undocumented behavior-changing settings.
    - Security: no new authority.
  - Approach:
    - Documentation Reviewed: `references/config.md`.
    - Options Considered: a settings-panel "Open config" button — recorded
      as a further action instead (own UI prototype loop; the palette
      command already covers the need).
    - Chosen Approach: command + docs only this phase.
    - API Notes and Examples:
      ```js
      // examples/config/init.js (commented):
      // keybindings.bindKey("Ctrl+Alt+,", "shell.openConfigurationFile");
      ```
    - Files to Create/Edit: `docs/reference/clay-js-api/configuration.md`
      (pointer only).
    - References: decision `2026-05-08-1841`.
  - Test Cases to Write: existing bindKey validation test gains the new
    command ID as a valid target.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: keybindings section gains the commented
      `shell.openConfigurationFile` binding with annotation; help/discovery
      mentioned in the file's section comments where the palette `?`
      trigger is documented; `node --check` passes.
    - Performance: n/a.
    - Code Quality: canonical style, one annotation per item.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Example
      Configuration Maintenance Task.
    - Options Considered: active binding — rejected: default-unbound is
      the recorded posture for new commands.
    - Chosen Approach: commented example.
    - API Notes and Examples: see previous task.
    - Files to Create/Edit: `examples/config/init.js`.
    - References: user instruction 2026-08-03.
  - Test Cases to Write: `node --check` gate.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: scratch HOME; GUI launch; Connected; clean generation;
      palette `?` query returns rows (spot-check theme APIs and
      `runtime.reloadConfiguration` with its binding); running
      `shell.openConfigurationFile` opens the seeded init.js in the pane;
      an edit + save triggers watcher reload (theme change visible).
    - Performance: palette query latency observed and recorded (no numeric
      gate this phase; evidence for a later budget).
    - Code Quality: commands + results in evidence.
    - Security: scratch profile; no real config harmed.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Live
      Launch-Test Task.
    - Options Considered: headless-only — fallback on GUI blocker with
      server-side lookup assertions.
    - Chosen Approach: full GUI launch.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: evidence in plan file.
    - References: plan 109 lesson (config never launch-tested).
  - Test Cases to Write: manual evidence.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: real Linux GUI; exercise palette help session (query,
      empty result, selection, cancel), the open-config flow, and narrow
      layout; screenshots stored; compare against the covering approved
      artifacts with per-deviation disposition; keyboard-only pass over
      the session (focus order, announcements, role/name/state of rows).
    - Performance: n/a.
    - Code Quality: evidence paths recorded.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Mandatory
      UI Visual and Accessibility Review Task; `references/ui.md`.
    - Options Considered: skip (reused surface) — rejected: duty is
      mandatory for UI-touching plans.
    - Chosen Approach: `computer-use-linux` + screenshots when available;
      else record blocker, leave unresolved.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `code-reviews/<date>-plan141-visual/`.
    - References: decision `2026-08-14-0200`.
  - Test Cases to Write: manual evidence.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: control-centre/palette module steps for the help session
      and the open-config command; results recorded on Linux.
    - Performance: n/a.
    - Code Quality: no weakened steps.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed: `test-plan/index.md`.
    - Options Considered: automated-only — rejected (user-visible).
    - Chosen Approach: extend the palette module.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `test-plan/control-centre*.md` (or nearest),
      `test-plan/index.md`.
    - References: user instruction 2026-08-04.
  - Test Cases to Write: the added steps.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki documents registry consumption at runtime, the help
      session kind, and the open-config command; index updated.
    - Performance: lookup/index build costs documented.
    - Code Quality: docs-as-code bar.
    - Security: read-only boundary noted.
  - Approach:
    - Documentation Reviewed: `references/docs-as-code.md`.
    - Options Considered: per-task updates — rejected (churn).
    - Chosen Approach: one post-pass.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `docs/wiki/modules/clay-js-doc-registry.md`,
      `docs/wiki/modules/command-centre.md` (or nearest), `docs/wiki/index.md`.
    - References: docs-as-code reference.
  - Test Cases to Write: manual wiki review.

## Compromises Made
- To be filled after execution.

## Further Actions
- To be filled after execution. Known candidates: settings-panel "Open
config" button (own prototype loop); exposing `help.lookup` to the daemon
for plan 139's skill (reverse-RPC mirror); an interactive config eval
surface (`*scratch*`-style) — needs its own security pass.
