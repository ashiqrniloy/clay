# Plan 140 — User-Config Programmability: init.js Contribution Authority

Source: the 2026-09-20 configurability review ("modern day Emacs, AI native"
goal). Deviation D1: the user can `setq` but never `defun`. `~/.clay/init.js`
is trusted user configuration everywhere in the security model, yet every
contribution-registration facade (`commands.serverRegisterCommand`,
`modes.serverRegisterModePattern`, `ui.serverRegisterPanelContribution`,
parse/decoration/completion registration) requires an active **package**
context via `require_current_package_capability`
(`src/server/ops/mod.rs:583`), so init.js fails with
`packages.no_active_package` on every call. `bindKey` then validates against a
closed registry of built-in command IDs — Clay became a menu, not a
programmable surface.

Binding prior decisions:

- `decision-logs/2026-08-18-1758-single-manifest-package-loading.md`: manifest
  `clay.contributions` is the only *package* registration data path, and
  imperative registration APIs ("`serverRegisterModePattern`, etc.") are
  explicitly **"reserved for user `init.js` configuration and runtime
  contributions"** — the recorded intent this plan implements.
- `decision-logs/2026-08-03-1859-editor-control-trust-boundary-for-editor-ops.md`:
  "trusted callers without a package context (user configuration) are the only
  gate-free path" — the same trusted-config identity this plan generalizes.
- `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`:
  every configuration option is a Clay JS API; init.js is the entry point.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`: two
  trust domains; classification by compiled inventory, never naming.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`:
  contributions stay inert validated declarations; Clay owns composition.

Roadmap position: prerequisite for the roadmap's
"Third-Party Workflow/Command Package Platform" exit gate making sense for
*users* too, and for plan 142 (the agent edits init.js — it must be able to
*do something* there).

## Objectives

- Give configuration-root evaluation a host-stamped **user-config identity**
  so the existing imperative registration facades work from `init.js` and
  `loadConfigurationModule` modules, with contributions carried in the normal
  generation lifecycle (registered fresh each generation per Phase 19).
- Contributions registered from init.js carry `user-config` provenance in the
  conflict indices, behavior manifests, and diagnostics — visible, revocable
  by removing the config lines, and never attributable to a package.
- Keep the third-party runtime exactly as closed as today: the contribution
  ops stay absent from `init_package_extension`; a third-party isolate
  calling them still gets `TypeError: undefined is not a function`.
- No new authority classes: user-config registration uses the same inert
  declaration validation, budgets, conflict pass, and keybinding-overlay
  precedence (`configuration keymaps survive mode activation`) as package
  contributions. init.js already runs in the trusted domain.
- Document the unlocked surface: example config, API docs, `api-inventory`
  custom properties, and the generated registry.

Out of scope: new UI component kinds or slots (existing catalog only), local
package installation (plan 141), agent-side config editing (plan 142).

## Expected Outcome

- An `init.js` containing e.g. `commands.serverRegisterCommand(...)` +
  `keybindings.bindKey(...)` produces a working, bindable user command after
  reload; the same for one user mode pattern and one inert panel declaration,
  each validated exactly as a package's would be (budgets, prefixed-ID rules
  waived only where core IDs are legal for core callers, conflict pass).
- `clay` reload (watcher or `runtime.reloadConfiguration`) rebuilds user
  contributions per generation; removing the lines withdraws them.
- All seven Linux gates stay green; new tests prove the third-party-domain
  denial and the user-config provenance stamps.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: record commit + gate results (`audit`, `fmt`, `check`,
      `clippy`, `test`, `bench-compile`, `bindings`) under
      `code-reviews/<date>-plan137-baseline/logs/`.
    - Performance: no perf claims; baseline is correctness-only.
    - Code Quality: baseline captured before any edit.
    - Security: reproduce the gap — an `init.js` snippet calling
      `commands.serverRegisterCommand` logs `packages.no_active_package`.
  - Approach:
    - Documentation Reviewed: AGENTS.md (platform validation: Linux blocking);
      `.agents/skills/clay-execution/references/planning-checklist.md`.
    - Options Considered: skip fresh baseline — rejected: this plan changes
      the runtime gate every contribution op shares.
    - Chosen Approach: run the seven-stage gate; capture the failing
      registration diagnostic as the reproduced gap.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings
      cargo test --lib -- --test-threads=1
      ```
    - Files to Create/Edit: `code-reviews/<date>-plan137-baseline/README.md`.
    - References: `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md`
      (baseline task shape).
  - Test Cases to Write: none (evidence capture).

- [ ] Decision log: user-config executing identity for contribution facades
  - Acceptance Criteria:
    - Functional: `decision-logs/<date>-user-config-contribution-identity.md`
      records: configuration-root evaluation carries a host-stamped
      `user-config` provenance; contribution facades accept it as a trusted
      caller; capability checks treat it as first-party-equivalent (same
      trust as the editor-control gate-free path); conflict indices and
      diagnostics stamp `user-config`; third-party domain unchanged.
    - Performance: no runtime cost beyond one provenance lookup per
      registration call (registration is load/reload-time only).
    - Code Quality: decision follows the create-decision-log skill
      (alternatives, approval statement quoted).
    - Security: the decision states explicitly what is **not** granted: no
      new authority over filesystem/network/shell/AI/workspace; user-config
      identity cannot be forged by package code (it exists only while the
      server evaluates the configuration root).
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/create-decision-log/SKILL.md`;
      `decision-logs/2026-08-18-1758-single-manifest-package-loading.md`;
      `decision-logs/2026-08-03-1859-editor-control-trust-boundary-for-editor-ops.md`.
    - Options Considered:
      - Synthesize a real `PackageRecord` for the config root and run it
        through `PackageService` — rejected: drags install/enable/adoption
        lifecycle into config evaluation for no validation benefit.
      - A boolean `trusted_config_caller` flag beside `current_package` —
        rejected: provenance strings are already threaded through conflict
        diagnostics; a named identity composes better.
    - Chosen Approach: explicit user approval for the decision text (the task
      stays unchecked without it), then the implementation tasks cite it.
    - API Notes and Examples:
      ```rust
      // Conceptual: the executing-caller enum the gate consults.
      enum ExecutingCaller { Package(PackageProvenance), UserConfig }
      ```
    - Files to Create/Edit: `decision-logs/<date>-user-config-contribution-identity.md`
      (new); `.agents/skills/clay-execution/references/packages.md`
      (smallest-relevant-reference update per SKILL.md integration rule).
    - References: `create-decision-log` skill; clay-execution SKILL.md →
      Plan Creation and Decision-Log Integration.
  - Test Cases to Write: none (document).

- [ ] Review contribution-gate primitives before implementation
  - Acceptance Criteria:
    - Functional: inventory with exact paths — `ClayOpState` +
      `require_current_package_capability` (`src/server/ops/mod.rs:580`),
      the per-family ops (`src/server/ops/{commands,modes,ui,parse,decorations,completion,folding,sdui}.rs`),
      provenance flow into `PackageRecord`-keyed registries, the conflict pass
      (`src/packages/conflict.rs`), keybinding overlay ordering
      (`configuration keymaps survive mode activation` decision), Phase 19
      generation rebuild (`src/server/mod.rs::reload_runtime_generation`),
      and the third-party extension boundary (`init_package_extension`).
      State which validation paths assume a package record (ID prefixing,
      payload budgets) and how a user-config caller maps onto each.
    - Performance: confirm registration stays load/reload-time only; no gate
      work on typing/paint/layout/scroll/text-event paths.
    - Code Quality: the review rejects any new per-family special case; the
      change lands once in the shared gate.
    - Security: the review names every op that must remain unreachable from
      the third-party domain and the test that proves it.
  - Approach:
    - Documentation Reviewed:
      `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`,
      `docs/wiki/modules/primitive-architecture.md`,
      `docs/wiki/modules/package-loading.md`,
      `.agents/skills/clay-execution/references/packages.md`,
      `references/js-api.md`.
    - Options Considered: per-facade `Option<caller>` parameters — rejected:
      ~10 call sites × drift risk; one identity in `ClayOpState` is smaller.
    - Chosen Approach: single executing-caller identity consulted by the
      shared gate; family validators receive provenance the same way they do
      today, with `user-config` as the provenance string.
    - API Notes and Examples:
      ```rust
      // Today (fails for init.js):
      let package = state.borrow::<Arc<ClayOpState>>()
          .require_current_package_capability("commands")?;
      // After: gate answers Ok with UserConfig provenance when the
      // executing context is the configuration root.
      ```
    - Files to Create/Edit:
      `code-reviews/<date>-plan137-baseline/primitive-review.md` (new).
    - References: `src/server/ops/mod.rs`, `src/packages/conflict.rs`,
      `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`.
  - Test Cases to Write: none (review verified by later tasks).

- [ ] Implement the user-config executing identity in the shared gate
  - Acceptance Criteria:
    - Functional: during configuration-root evaluation (init.js and
      `loadConfigurationModule` modules) the contribution facades succeed and
      stamp `user-config` provenance; outside configuration evaluation the
      identity does not exist (package runtime callers unchanged); a
      generation reload re-registers from scratch (Phase 19 semantics hold).
      Default-loading duty: the user experience is editing `init.js` directly
      — one import + one registration call, no copied manifests, no
      low-level facade plumbing, no test-only SDUI (decision 0219's bar
      applied to the user path).
    - Performance: one identity lookup per registration call at
      load/reload time; zero hot-path cost.
    - Code Quality: one gate change in `ClayOpState`; no per-family branches;
      `pub(crate)` visibility for new internals.
    - Security: identity is set by the trusted evaluation driver and cannot
      be influenced by evaluated JS; third-party extension still lacks the
      ops (existing absence test keeps passing, plus a new explicit one).
  - Approach:
    - Documentation Reviewed: primitive-review task output;
      `src/server/js_runtime/` evaluation driver (where `current_package` is
      set for `loadEntry` execution).
    - Options Considered: pass `trusted=true` through each facade — rejected
      (per-family drift, see review task).
    - Chosen Approach: mirror how `loadEntry` execution installs package
      provenance: the config evaluation driver installs `UserConfig` for the
      duration of evaluation; the shared gate maps it to
      first-party-equivalent capability answers.
    - API Notes and Examples:
      ```js
      // init.js — works after this task:
      import { serverRegisterCommand } from "clay:commands";
      serverRegisterCommand({
        id: "user.toggleSidebar",
        title: "Toggle my sidebar default",
        run: () => import("clay:shell").then((m) => m.clientToggleLeftPanel?.()),
      });
      ```
    - Files to Create/Edit: `src/server/ops/mod.rs` (gate),
      `src/server/js_runtime/evaluation.rs` (driver installs identity),
      affected validators where provenance strings are constructed.
    - References: decision log from task 2; `src/server/ops/mod.rs:580`.
  - Test Cases to Write:
    - `init_js_registers_command_and_bind_key_works`: end-to-end config
      evaluation registers a command; `bindKey` accepts it; dispatch fires.
    - `third_party_isolate_still_lacks_registration_ops`: unchanged denial.
    - `user_config_contributions_rebuild_per_generation`: reload withdraws
      removed lines, re-registers kept ones.

- [ ] Carry user-config provenance through conflicts, manifests, and diagnostics
  - Acceptance Criteria:
    - Functional: a user-registered contribution colliding with a package
      contribution produces the existing `PackageConflictDiagnostic` shape
      with `user-config` provenance; user-vs-user duplicates conflict with
      themselves; keybinding overlay precedence (config beats mode, decision
      2026-07-19-0328) unchanged.
    - Performance: conflict pass cost unchanged (same BTreeMap insertion).
    - Code Quality: provenance enum/struct reused, no stringly-typed new
      paths.
    - Security: no contribution from user config can claim a `clay.*` or
      reserved core prefix it isn't entitled to (same rules as core callers).
  - Approach:
    - Documentation Reviewed: `src/packages/conflict.rs`;
      `docs/wiki/modules/package-loading.md` (conflict section).
    - Options Considered: skip conflict integration and let user
      registrations win silently — rejected: silent wins are the ambiguity
      the conflict pass exists to remove.
    - Chosen Approach: extend provenance types with the `user-config` variant
      where `PackageProvenance` is currently required; sort order places
      `user-config` deterministically (alphabetical by provenance string).
    - API Notes and Examples:
      ```text
      conflict: commands id "user.toggleSidebar" claimed by user-config and @acme/pkg@1.0.0
      ```
    - Files to Create/Edit: `src/packages/conflict.rs`,
      behavior-manifest assembly (`src/server/behavior*` / `src/packages/record/behavior.rs`
      as located by the review task), diagnostic rendering.
    - References: `plans/019-Phase17-Package-System-and-Mode-Loading-Foundation.md`.
  - Test Cases to Write:
    - `user_config_vs_package_conflict_diagnostic_names_both_provenances`.
    - `config_keymap_overlay_beats_mode_binding` (regression guard).

- [ ] Create or verify Clay JS APIs for the unlocked facades
  - Acceptance Criteria:
    - Functional: every facade newly callable from init.js
      (`commands.serverRegisterCommand`, `modes.serverRegisterModePattern`,
      `ui.serverRegisterPanelContribution`, parse/decoration/completion
      registration as applicable) has Markdown docs with an init.js usage
      example, updated `permissions/security notes` stating the
      user-config/trusted-domain split, and `custom_properties` entries for
      behavior-changing options; master index + generated registry updated;
      `cargo run --bin update-doc-registry` rewrites artifacts; `cargo test`
      fails on drift.
    - Performance: n/a (docs).
    - Code Quality: naming follows `js-api.md` (no new exports needed —
      existing ones gain a documented caller class).
    - Security: docs state third-party domain cannot call these.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/js-api.md`;
      `docs/reference/clay-js-api/schema.md`.
    - Options Considered: separate "user API" doc pages — rejected: same
      facades, one page per API with a caller-class section.
    - Chosen Approach: extend existing pages + `api-inventory.toml` entries
      with an `agent_guidance`/security-note update.
    - API Notes and Examples:
      ```toml
      # api-inventory.toml entry gains:
      security_notes = "Callable from user configuration (trusted domain) and enabled packages with the commands capability; absent from the third-party runtime."
      ```
    - Files to Create/Edit: `docs/reference/clay-js-api/**`,
      `docs/reference/clay-js-api/api-inventory.toml`,
      `docs/index.md`, regenerated `docs/generated/clay-js-api-registry.json`.
    - References: decision `2026-05-08-1509`, `2026-05-08-1840`.
  - Test Cases to Write: registry/doc coverage gates (existing harness)
    extended to assert the caller-class note on the touched entries.

- [ ] Create or verify Clay configuration APIs (configuration task)
  - Acceptance Criteria:
    - Functional: the configuration story for user contributions is one
      documented path (init.js imperative registration) with no hidden keys;
      `getConfigurationState()` reports user-config contributions.
    - Performance: state snapshot stays bounded (existing budget).
    - Code Quality: no undocumented behavior-changing settings.
    - Security: configuration grants nothing beyond the trusted domain it
      already runs in (statement in docs).
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/config.md`.
    - Options Considered: a `~/.clay/commands.json` declarative file —
      rejected: a second registration path with weaker typing.
    - Chosen Approach: init.js is the single user path (matches decision
      2026-05-08-1841).
    - API Notes and Examples:
      ```js
      // examples/config/init.js — user commands section (annotated).
      ```
    - Files to Create/Edit: `docs/reference/clay-js-api/configuration.md`,
      `runtime/js/configuration.js` (state exposure if missing).
    - References: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
  - Test Cases to Write: `getConfigurationState_lists_user_contributions`.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: example gains a "User commands and modes" section showing
      one registered command + one `bindKey` to it (active-safe), with
      commented variants for a mode pattern and a panel declaration;
      `node --check examples/config/init.js` passes; ordering constraints
      preserved.
    - Performance: n/a.
    - Code Quality: same annotation style as existing sections; each option
      documented once.
    - Security: no example grants capabilities it doesn't need.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Example
      Configuration Maintenance Task.
    - Options Considered: ship docs-only examples — rejected: canonical file
      must exercise the surface.
    - Chosen Approach: minimal active example + commented variants.
    - API Notes and Examples: see task above.
    - Files to Create/Edit: `examples/config/init.js`.
    - References: user instruction 2026-08-03 (canonical example config).
  - Test Cases to Write: `node --check` gate (existing); example-config
    conformance test if present.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: scratch config root (temp HOME) with the copied example;
      real Linux GUI build; Connected state; generation commits with no
      `configuration failed` diagnostics; the example user command appears in
      the palette and its binding fires.
    - Performance: startup within existing budgets.
    - Code Quality: launch command + observed results recorded in task
      evidence.
    - Security: never launched against the developer profile.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Example
      Configuration Live Launch-Test Task.
    - Options Considered: server-only check — fallback only if GUI blocked
      (record blocker, assert generation commits headlessly).
    - Chosen Approach: full GUI launch on Linux.
    - API Notes and Examples:
      ```bash
      HOME=$(mktemp -d) cargo run --release --bin clay   # client+server per repo launch docs
      ```
    - Files to Create/Edit: evidence recorded in the plan file (task notes).
    - References: `plans/012-Developer-Friendly-Launch-and-GUI-Smoke.md`.
  - Test Cases to Write: manual evidence (this task is the test).

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: affected modules identified (configuration, keybindings,
      palette); numbered steps added for user-registered commands/modes;
      results recorded on a real Linux build.
    - Performance: n/a.
    - Code Quality: no weakened steps.
    - Security: negative check — third-party runtime denial still holds.
  - Approach:
    - Documentation Reviewed: `test-plan/index.md` (module map).
    - Options Considered: skip (internal-only surface) — rejected: user-visible
      behavior changed.
    - Chosen Approach: extend configuration/keybinding modules.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `test-plan/*.md`, `test-plan/index.md`.
    - References: user instruction 2026-08-04.
  - Test Cases to Write: the added numbered steps.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki documents the executing-caller identity, provenance
      flow, and Phase 19 rebuild for user contributions; index links updated.
    - Performance: no runtime work added.
    - Code Quality: follows `docs-as-code.md` bar.
    - Security: trust-boundary description updated without secrets.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Options Considered: per-task updates — rejected (churn).
    - Chosen Approach: one post-pass update.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `docs/wiki/modules/package-loading.md`,
      `docs/wiki/modules/configuration-runtime.md`, `docs/wiki/index.md`.
    - References: docs-as-code reference.
  - Test Cases to Write: manual wiki review.

## Compromises Made
- To be filled after execution.

## Further Actions
- To be filled after execution. Known candidates: per-mode keybinding scopes
  and hook points (Emacs `before-save-hook` analogues) are deliberately out
  of scope and need their own primitive review.
