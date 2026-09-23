# 155 — In-App Capability Grant Surface: Package Panel and Capability Presets

Source: `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md`
→ `## Further Actions` items 3 and 6 (recorded 2026-09-23). Plan 136 shipped the
grant surface for the CLI and `~/.clay/init.js` (`clay package authorize`, the
`authorize` Clay JS API, durable provenance-bound grants), but the desktop app has
no package surface at all, so a GUI-only user cannot inspect, adopt, grant, or
revoke a third-party package. The capability-preset model was already approved
(`decision-logs/2026-08-18-1758-package-capability-presets.md`); once grants are
visible in the app, a one-click preset removes most per-capability friction.

**UI prototype gate: applies.** This plan adds a new app surface (a package
panel/overlay with lists, per-capability controls, provenance text, and
destructive actions), so it carries the ordered prototype → freeze/approval →
implementation tasks below. `design-artifacts/prototypes/` and
`design-artifacts/approved/` currently hold only `agent-lane-palette`,
`composer-palette-stages`, `quiet-instrument-language`, and
`quiet-instrument-migration` — none covers a package/capability surface, so a new
slug (`package-capability-grants`) is required and no implementation task may
start before its approval task closes. The landing/IA baseline is fixed
(`DESIGN.md` §12: the launcher is the landing surface, a tab is one workspace plus
one agent): this panel is a transient surface over that model, not a second
landing.

## Objectives

- G1: the app shows the installed/adopted packages with their provenance,
  adoption state, declared capabilities, granted capabilities, runtime profile,
  and grant attribution, and lets the user perform the same authority changes the
  CLI can — adopt, grant/ungrant a capability set, set the runtime profile,
  enable/disable, and revoke — with the same refusals and fail-closed behavior.
- G2: the panel never shows or claims more authority than the server has
  recorded: every mutation goes through the existing `PackageService` calls
  (`approve_package`, `authorize_package`, `enable`, `disable`,
  `revoke_package_approval`), and the UI cannot grant what the manifest does not
  declare.
- G3: a capability preset turns the common case into one action (e.g. "adopt a
  completion provider" grants the declared completion/parse set with the default
  profile), defined as inert data over declared capabilities, never as a
  hard-coded authority bundle a package can request for itself.

## Expected Outcome

- With a third-party package installed and pending, the app shows it with its
  provenance and declared capabilities; adopting and granting from the panel
  produces exactly the same durable store record the CLI produces
  (`grant { capabilities, runtime_profile, granted_by, granted_at }`), and
  `clay package inspect` from a separate process shows the same `Grants:` line.
- Granting only part of the declared set leaves the package failing closed on
  enable, with the panel showing which declared capabilities are ungranted and
  the same `MissingCapabilityGrant` reason the CLI reports.
- Revoking from the panel returns the package to fail-closed
  (`AdoptionRequired { code: "package_approval.revoked" }`) and the panel reflects
  it; a package whose provenance changed shows the grant as inert.
- A preset action grants the preset's declared set in one step and is visible in
  the same review trail (`granted_by: user`), with the panel listing exactly which
  capabilities it will grant before applying.
- The visual/accessibility review compares the running panel against the approved
  artifact and records per-surface deviations; `scripts/capture-ui-review.sh`
  captures the new surface in its states across the shipped content themes.
- `cargo fmt --check`, `cargo check/clippy --all-targets -- -D warnings`,
  `cargo test --all-targets`, `cargo test -p clay-desktop --all-targets`,
  `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`, and
  `scripts/check.sh full` are green.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: tree state and a full `scripts/check.sh full` run are recorded
      before edits; the absence of any package surface is reproduced concretely
      (no frontend module renders package authority; `packages.inspect`,
      `packages.list`, `packages.enable`, `packages.disable` are
      `status = "planned"` rows in `docs/reference/clay-js-api/api-inventory.toml`;
      `clay package inspect` is the only way to see grants).
    - Performance: the baseline records the live `server.edit_ack` envelope and
      startup timings (plan 136: p50 231.8 µs / p95 282.1 µs; socket 425 ms,
      first config effect 493 ms, editor visible 1388 ms) so the new surface is
      measured against a known state; opening the panel must not touch the typing
      path.
    - Code Quality: the baseline inventories the existing surfaces the panel must
      reuse — `PackageInspection` (`src/packages/service.rs:282`), the
      `clay package …` verbs, the SDUI/package-UI snapshot path
      (`src/protocol/runtime.rs`), the shell pane/slot model, and the catalog
      components — and states which are missing (a server-owned package list
      snapshot for the app).
    - Security: the baseline records that the CLI's refusals (undeclared
      capability, unadopted package, revoked record, activation-scope self-grant)
      must be preserved verbatim by the GUI path, and that no new authority may be
      reachable from a package.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/packages.md`,
        `references/config.md`, `references/ui.md`, `references/components.md`,
        `references/tokens.md`, and `DESIGN.md` (§12 landing/tab model, §14
        retired patterns).
      - `docs/reference/clay-js-api/packages/authorize.md`,
        `docs/reference/packages/creating-packages.md`,
        `docs/wiki/modules/third-party-runtime-authority.md`.
      - `design-artifacts/README.md`: what prototypes and approved artifacts are
        and which is normative.
    - Options Considered:
      - CLI-only forever (status quo): rejected — a GUI-only user cannot adopt a
        powerful package, which is the blocker this plan exists to remove.
      - A settings-page table only (no per-capability control): cheaper, but the
        grant decision is per capability and the panel is where users make it.
    - Chosen Approach: inventory the data and authority surfaces first, then
      prototype, then implement the server snapshot and the panel on top of the
      existing service calls.
    - API Notes and Examples:
      ```bash
      scripts/check.sh full
      clay package inspect @fixture/lane    # the CLI truth the panel must match
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/155-capability-grant-ui/baseline.md`.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` items 3, 6.
      - `src/packages/service.rs:282` (`PackageInspection`),
        `docs/reference/clay-js-api/api-inventory.toml` (planned rows).
  - Test Cases to Write:
    - Baseline reproduction: the CLI is the only grant surface, recorded with the
      exact commands and their output.

- [ ] Review Clay UI catalog and plan primitive/component reuse before UI work
  - Acceptance Criteria:
    - Functional: the review names the catalog components, slots, tokens, and
      patterns the panel uses (list/table, badges for adoption and trust state,
      switch/checkbox rows for capabilities, select for runtime profile, dialog
      for the destructive revoke confirmation, empty/loading/error states), and
      states which catalog additions (if any) are needed — additive only.
    - Performance: the review states the render budget (the panel renders on open
      and on package-state changes, never per keystroke) and confirms no
      package-authority read lands on a hot path.
    - Code Quality: reuse is checked before any custom component; a custom
      component requires explicit justification in `Options Considered`; no
      retired `DESIGN.md` §14 pattern is reintroduced; the catalog references and
      `docs/reference/ui-components.md` are updated if a component kind is added.
    - Security: the review states that the panel is a trusted shell surface (not
      package-contributed UI), that packages cannot inject rows or actions into
      it, and that every action maps to a server-side authority check.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` (normative), `.agents/skills/clay-execution/references/ui.md`,
        `references/components.md`, `references/tokens.md`,
        `docs/reference/ui-components.md`.
      - The four project-local design skills (`impeccable`,
        `full-output-enforcement`, `high-end-visual-design`,
        `design-taste-frontend`) for the substantial new surface.
    - Options Considered:
      - A package-contributed panel (SDUI from a first-party package): rejected as
        the primary shape — package authority is host-owned; a first-party
        package may later provide a richer view, but the authority surface itself
        stays host.
      - Host-owned transient panel (chosen): matches the Control Center palette
        and settings surfaces users already know.
    - Chosen Approach: catalog review first, then prototype with the chosen
      components and tokens.
    - API Notes and Examples:
      ```bash
      ls .agents/skills/clay-execution/references/   # ui.md, components.md, tokens.md
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/155-capability-grant-ui/ui-catalog-review.md`;
        catalog files only if a component kind is added.
    - References:
      - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
  - Test Cases to Write:
    - None (review task); catalog/documentation drift tests must stay green.

- [ ] Build the HTML prototype for the package capability panel in design-artifacts/prototypes/package-capability-grants/
  - Acceptance Criteria:
    - Functional: a self-contained artifact (or a shared stylesheet plus per-view
      pages) opens over `file://` with no build step and shows every in-scope
      state: package list (installed, pending adoption, approved, enabled,
      revoked, provenance-changed/inert grant), package detail (declared vs
      granted capabilities, runtime profile, attribution), the grant/ungrant
      flow, the enable/disable action, the revoke confirmation, and the preset
      chooser — each in rest/hover/active/focus/selected/disabled/invalid plus
      empty/loading/error/recovery where applicable, in narrow and wide layouts.
    - Performance: the prototype loads without console errors and without
      layout thrash; it states the intended render triggers (panel open,
      package-state change).
    - Code Quality: it reuses the catalog vocabulary
      (`component.variant.slot.state`, kind names, token names) so approval maps
      1:1 onto catalog entries; a needed catalog addition is named in the task
      evidence; a `README.md` states scope, variants, coverage, and how to open
      it; the prototype states explicitly that it has no authority.
    - Security: the prototype shows the authority facts a user needs to decide
      (provenance, declared vs granted, who granted, what enablement means) and
      never renders a package-controlled string as trusted UI text; destructive
      actions require an explicit confirmation state.
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/README.md`, `design-artifacts/prototypes/agent-lane-palette/`
        and `composer-palette-stages/` (existing prototype conventions).
      - `DESIGN.md` §4/§11/§12/§14; `references/components.md`,
        `references/tokens.md`.
      - `docs/reference/clay-js-api/packages/authorize.md` and the
        `clay package inspect` output shape (`Grants:`, `Granted by:`,
        `Ungranted:`).
    - Options Considered:
      - One page with a variant switcher: chosen — the panel is one surface with
        state variants.
      - Separate pages per state: more files, slower to compare; rejected.
    - Chosen Approach: one self-contained page rendering the four shipped content
      themes with a theme switcher, all component states, and both widths.
    - API Notes and Examples:
      ```bash
      # open the prototype directly; no build step
      xdg-open design-artifacts/prototypes/package-capability-grants/index.html
      ```
    - Files to Create/Edit:
      - `design-artifacts/prototypes/package-capability-grants/index.html`,
        `README.md`, plus any per-state asset.
    - References:
      - user instruction 2026-09-11 (UI prototype and explicit approval gate).
  - Test Cases to Write:
    - Manual prototype leg: open in a browser, screenshot each state/theme/width,
      keyboard-only pass over interactive controls, zero console errors.

- [ ] Obtain explicit user approval and freeze design-artifacts/approved/package-capability-grants/
  - Acceptance Criteria:
    - Functional: the prototype is presented with the decisions that need a
      choice (placement, list vs detail layout, per-capability control shape,
      preset presentation, confirmation flow) called out; approval is a real user
      statement, never inferred.
    - Performance: the frozen artifact records the render-trigger expectations so
      the implementation review can compare behavior, not only looks.
    - Code Quality: on approval the chosen files are copied to
      `design-artifacts/approved/package-capability-grants/` with a `README.md`
      recording approval date, the approving user statement (quoted), chosen
      variant, requested changes, superseded variants, and the exact
      surface/state/theme/width coverage; losing variants stay under
      `prototypes/` marked not approved; the artifact set is append-only.
    - Security: the approved artifact states the authority facts it must show
      (provenance, declared vs granted, attribution, revocation) so the
      implementation cannot quietly drop them.
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/README.md`, `design-artifacts/approved/quiet-instrument-migration/README.md`
        (the approval record format).
      - `.agents/skills/create-decision-log/SKILL.md` (if approval changes the
        design language, which it must not).
    - Options Considered:
      - Infer approval from the agent's own review: rejected — the gate requires a
        user statement.
    - Chosen Approach: present, record, freeze. Without the user's approval this
      task stays unchecked and blocks the implementation tasks.
    - API Notes and Examples:
      ```bash
      cp -r design-artifacts/prototypes/package-capability-grants/* \
            design-artifacts/approved/package-capability-grants/
      ```
    - Files to Create/Edit:
      - `design-artifacts/approved/package-capability-grants/` (frozen copies +
        `README.md`).
    - References:
      - user instruction 2026-09-11.
  - Test Cases to Write:
    - Approval record completeness: date, quoted user statement, chosen variant,
      coverage list, and binding artifact paths present.

- [ ] Expose the package inventory and grant state to the app
  - Acceptance Criteria:
    - Functional: the server publishes the data the panel needs — installed and
      bundled packages with provenance, adoption state, declared capabilities,
      granted capabilities, runtime profile, grant attribution, and enablement —
      derived from the same `PackageInspection`/service reads the CLI uses, and
      pushed on package-state changes (adopt/grant/enable/disable/revoke/reload)
      without requiring a manual refresh.
    - Performance: the snapshot is built on package-state change and panel open
      only; it never runs per keystroke, and its payload stays within the existing
      snapshot budgets (no unbounded package list in a hot message).
    - Code Quality: one server-owned shape serves the panel (no per-view queries),
      the DTO is validated like the other snapshots, and the CLI printer and the
      panel are shown to read the same fields (no second source of truth).
    - Security: the snapshot carries only what the user is entitled to see, marks
      grant provenance explicitly (so an inert grant is visible as inert), and
      exposes no filesystem path, environment value, or secret.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/runtime.rs` (`PackageUiSnapshot` and friends) as the
        package-UI precedent; `src/server/fanout.rs` (Plan 132 lanes) for the
        push shape.
      - `docs/wiki/modules/server-state-fanout.md`,
        `docs/wiki/modules/server-ipc-skeleton.md`.
      - `src/packages/service.rs` (`PackageInspection`, `capability_granted`,
        `refresh_installed`).
    - Options Considered:
      - SDUI tree for the whole panel: rejected as the data source — authority
        data should be a typed snapshot the shell renders with catalog
        components; SDUI stays for package-contributed content.
      - A typed package-authority snapshot pushed on change (chosen): matches the
        existing snapshot/fanout patterns and keeps the client dumb.
    - Chosen Approach: add the typed snapshot (server) and its client store, with
      a test that the panel data equals the CLI's inspect output for the same
      store.
    - API Notes and Examples:
      ```rust
      // the same reads the CLI printer uses
      let inspection = service.inspect_package(&name)?;   // PackageInspection
      let granted = inspection.approved_capabilities;      // granted view (union)
      ```
    - Files to Create/Edit:
      - `src/protocol/` (snapshot type + validation), `src/server/` (builder +
        fanout push), `frontend/src/shell/` (store), plus tests.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` (snapshot lanes),
        `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` task 5 outcome (CLI inspect lines).
  - Test Cases to Write:
    - `package_authority_snapshot_matches_cli_inspect`: for a fixture store, the
      snapshot fields equal the CLI's `Grants:`/`Granted by:`/`Ungranted:` view.
    - `package_authority_snapshot_pushes_on_state_change`: adopt/grant/enable/
      disable/revoke each produce a new snapshot without a manual refresh.

- [ ] Implement the package panel against the approved artifact
  - Acceptance Criteria:
    - Functional: the panel renders from the snapshot and performs adopt, grant,
      ungrant (set replacement), runtime-profile selection, enable, disable, and
      revoke through the existing service calls, matching the approved artifact
      (`design-artifacts/approved/package-capability-grants/`) for every state it
      covers; refusals (undeclared capability, unadopted record, revoked record,
      provenance-changed inert grant) surface the server's own reason.
    - Performance: opening the panel and performing an action do not regress the
      live `server.edit_ack` envelope or startup timings; the panel performs no
      per-keystroke work and no polling.
    - Code Quality: the panel is host-owned shell code using catalog components and
      tokens; no raw colors or literals; every action maps to one server call
      (no duplicated authority logic in the client); each deviation from the
      approved artifact is either fixed or re-approved and recorded.
    - Security: the client cannot widen authority — it sends the requested
      capability set and the server validates declaration, ceiling, and record
      state; destructive actions require confirmation; no package-supplied string
      is rendered as trusted UI text; the panel is unreachable from package code.
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/approved/package-capability-grants/README.md` and the
        frozen artifacts (binding).
      - `DESIGN.md`, `references/ui.md`, `references/components.md`,
        `references/tokens.md`.
      - `docs/reference/clay-js-api/packages/authorize.md` (the authority rules
        the panel must mirror).
    - Options Considered:
      - Reimplement authority checks client-side for instant feedback: rejected —
        the server is the only authority; the client renders the server's answer.
      - Mirror the CLI's refusal messages in the panel (chosen): one vocabulary
        for the same failure, already covered by tests.
    - Chosen Approach: render from the snapshot, mutate through the service calls,
      reuse the CLI's error text, and compare against the approved artifact in the
      review task.
    - API Notes and Examples:
      ```tsx
      // one action → one server call; the panel renders the server's reason
      await authorizePackage({ package: name, capabilities, runtimeProfile, approvedBy: "user" });
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/` (panel component + store wiring + tests),
        `src/server/ops/` if a thin op is needed for the client action path.
    - References:
      - `src/cli.rs` (`clay package authorize|revoke|inspect` semantics),
        `src/packages/service.rs` (`authorize_package`, `approve_package`,
        `enable`, `disable`, `revoke_package_approval`).
  - Test Cases to Write:
    - `granting_from_the_panel_writes_the_same_store_record_as_the_cli`: compare
      the durable record after a panel grant with the CLI's record shape.
    - `panel_refuses_an_undeclared_capability_and_shows_the_server_reason`.
    - Component tests for each state in the approved artifact (loading, empty,
      pending, granted-partial, revoked, provenance-changed, error).

- [ ] Add capability presets over the grant surface
  - Acceptance Criteria:
    - Functional: a preset action grants a named, declared capability set with the
      default runtime profile in one step (`granted_by: user`), shows exactly
      which capabilities it will grant before applying, and is refused if the
      package does not declare the preset's capabilities; presets cover the common
      cases (e.g. completion provider, document analysis, mode + parse) and remain
      inert data.
    - Performance: applying a preset is one grant write, no different from the
      per-capability path.
    - Code Quality: presets are a documented, additive table (no package-specific
      Rust branch, no hard-coded authority for a named package), defined once and
      shared by the panel and (optionally) a CLI flag, with tests asserting the
      preset set is a subset of the declared capabilities.
    - Security: a preset never grants beyond the adoption ceiling or the manifest's
      declarations, never bypasses the undeclared-capability refusal, and cannot be
      requested by package code — the preset model is user-side convenience, not
      package-side authority.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-08-18-1758-package-capability-presets.md` (approved
        model), `docs/reference/clay-js-api/packages/authorize.md`.
      - `src/packages/permissions.rs` (`PackagePermission` names),
        `src/packages/service.rs` (`authorize_package` subset check).
    - Options Considered:
      - Presets as manifest-declared bundles requested by the package: rejected —
        a package must not propose its own authority bundle as a one-click.
      - User-side preset table over declared capabilities (chosen): the decision
        log's model, and it reuses the existing subset validation.
    - Chosen Approach: one shared preset definition consumed by the panel (and a
      CLI flag if the review shows it helps), with the pre-apply summary listing
      the exact capabilities.
    - API Notes and Examples:
      ```js
      // CLI shape if the flag ships
      clay package authorize @vendor/pkg --preset completion-provider
      ```
    - Files to Create/Edit:
      - `src/packages/` (preset table + validation), panel wiring,
        `src/cli.rs` (only if the flag ships), tests.
    - References:
      - `decision-logs/2026-08-18-1758-package-capability-presets.md`.
  - Test Cases to Write:
    - `preset_grants_exactly_its_declared_subset`: for each preset, the granted
      set equals the preset and never exceeds the manifest declarations.
    - `preset_is_refused_when_the_package_does_not_declare_it`.

- [ ] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` documents what a
      package author must do for the panel to be useful — declare capabilities
      truthfully, expect the host to own the authority surface, and never rely on
      a UI path for grants; the panel itself is documented as a host surface, not
      a package contribution.
    - Performance: no authoring-path change adds runtime work; the guide states
      the panel's data is a snapshot on state change.
    - Code Quality: the guide's examples, permission lists, and limitations match
      the shipped behavior (including the preset model and the refusal cases);
      catalog references stay current if a component kind was added.
    - Security: the guide states the trust boundary explicitly — packages cannot
      render into the panel, cannot request a preset, and cannot observe other
      packages' grants.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`,
        `docs/reference/primitives/package-security.md`,
        `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`,
        `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`.
    - Options Considered:
      - Leave the guide untouched because the panel is host-owned: rejected — the
        guide is where authors learn what the host will do with their declared
        capabilities.
    - Chosen Approach: add a short "how grants appear in the app" section plus the
      authority statements.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol primitives_docs:: documentation_coverage::
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`,
        `.agents/skills/clay-execution/references/components.md` /
        `references/tokens.md` (only if the catalog changed).
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` task 11 outcome (package authoring docs precedent).
  - Test Cases to Write:
    - Primitive/documentation coverage tests stay green; markers pin the new
      authority statements if the guide's contract test exists.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: the panel's actions are backed by documented Clay JS APIs —
      the plan promotes the inventory rows it actually uses
      (`packages.inspect`, `packages.list`, `packages.enable`, `packages.disable`,
      and any new preset surface) from `status = "planned"` to runtime-backed with
      real op paths, or records explicitly why a row stays planned.
    - Performance: no facade adds hot-path work; the panel's reads are snapshot
      reads.
    - Code Quality: each promoted API has Markdown docs with stable ID, options,
      defaults, return shape, errors, permissions, backing Rust path, op wrapper,
      facade path, and lookup tags; `docs/index.md`, the generated registry, and
      the parity ledger are updated; `cargo test` fails on missing/stale entries.
    - Security: every promoted API documents its trusted-only position (package
      code cannot import `clay:packages`), the refusal cases, and that grants stay
      provenance-bound; the denied-authority list stays accurate.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`;
        `docs/reference/clay-js-api/packages/authorize.md` (page template).
      - `docs/reference/clay-js-api/api-inventory.toml` (planned rows),
        `src/server/ops/packages.rs` (op conventions), `src/server/facades.rs`
        (`clay:packages` is `Facade::trusted`).
    - Options Considered:
      - Keep the panel on a private client action path with no public API:
        rejected — the same capability should be scriptable, and the inventory
        already declares these IDs.
      - Promote the rows the panel uses (chosen): keeps the inventory honest.
    - Chosen Approach: promote exactly the rows the panel needs, with docs and
      registry regeneration in the same change.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test protocol clay_js_api_inventory:: primitives_docs::
      ```
    - Files to Create/Edit:
      - `runtime/js/packages.js`, `runtime/js/packages.d.ts`,
        `docs/reference/clay-js-api/api-inventory.toml`,
        `docs/reference/clay-js-api/packages/*.md`, `docs/index.md`,
        `docs/generated/clay-js-api-registry.json`,
        `docs/development/tauri-react-parity-ledger.json`.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` tasks 3, 9, 10 (promotion pattern and its gate
        failures), `src/server/ops/mod.rs` (op-count and admin-denial-list
        updates).
  - Test Cases to Write:
    - `every_promoted_package_api_is_runtime_backed_and_documented`; the existing
      inventory/registry/parity tests stay green.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: any configuration surface this plan adds (e.g. a preset
      definition override or a panel-related preference) is a documented Clay JS
      API; unchanged surfaces are verified rather than restated.
    - Performance: configuration work happens at configuration/panel-open time,
      not per keystroke.
    - Code Quality: docs, `custom_properties`, the generated registry, and the
      example config agree on names, enums, and defaults.
    - Security: configuration never grants package authority implicitly; granting
      remains an explicit user action in the panel or `authorize` call.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`;
        `docs/reference/clay-js-api/configuration.md`.
    - Options Considered:
      - Make presets configurable in `init.js`: only if the review shows a real
        need; otherwise presets stay a fixed documented table.
    - Chosen Approach: verify; add a configuration surface only for a shipped
      option.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js_api_inventory::plan136
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md` and the touched API pages.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` task 10 outcome.
  - Test Cases to Write:
    - `plan155_configuration_documents_the_grant_surface` (markers), or an
      explicit no-change verification in the task evidence.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: the example shows the grant surface a user needs — the
      adopt → authorize → `loadPackage` order stays, and a commented preset
      example is added if presets ship; every option name/enum/default matches
      the validated parsers.
    - Performance: `node --check` passes; the active part stays copy-safe.
    - Code Quality: the third-party module
      (`examples/config/packages/third-party.js`) keeps its section style, with
      the new surface annotated exactly once.
    - Security: no active grant for a powerful capability ships uncommented; the
      example states that a grant is an explicit user decision.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/packages/third-party.js`,
        `examples/config/README.md`, `docs/reference/clay-js-api/configuration.md`.
      - `tests/clay_js_doc_registry.rs::canonical_example_active_configuration_is_copy_safe`.
    - Options Considered:
      - Skip because the panel is GUI-only: rejected — the preset/CLI shape is a
        configuration surface users read.
    - Chosen Approach: extend the third-party module with the preset example and
      verify the copy-safe test.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js && node --check examples/config/packages/third-party.js
      ```
    - Files to Create/Edit:
      - `examples/config/packages/third-party.js`, `examples/config/README.md`.
    - References:
      - user instruction 2026-08-03 (example config maintenance duty).
  - Test Cases to Write:
    - `plan136_configuration_documents_the_capability_grant_surface` and the
      copy-safe test stay green (extend markers for presets).

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: a real Linux build launched against a copy of `examples/config/`
      in an isolated scratch root reaches Connected with no `configuration failed`
      diagnostics, and the panel opens, lists the fixture package with its
      provenance and grants, and performs one grant and one revoke through the
      live server (verified by a separate `clay package inspect` process).
    - Performance: startup timings are recorded and compared against the plan-136
      baseline (socket 425 ms, first config effect 493 ms, editor visible
      1388 ms); opening the panel does not disturb the editor's typing path.
    - Code Quality: the launch command, scratch root, and observations are
      recorded; a degraded app under the example config is a defect, not a docs
      fix.
    - Security: mode-700 scratch root with its own HOME/XDG/TMPDIR and socket; the
      panel's mutations are verified to land in that scratch store only; teardown
      is PID-based.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/artifacts/136-capability-grants/live-example-config/README.md`
        (launch recipe, isolation, timing baseline).
      - `test-plan/artifacts/127-lane-scheduling/run-live.sh` (`example-config`
        mode), `probe.py`.
    - Options Considered:
      - Drive the panel through AT-SPI: the reliable path (the palette precedent);
        screenshots plus AT-SPI trees as evidence.
    - Chosen Approach: launch, open the panel, grant, verify from the CLI,
      revoke, verify fail-closed again.
    - API Notes and Examples:
      ```bash
      CLAY_LIVE_ROOT=/tmp/clay-plan155-ui test-plan/artifacts/127-lane-scheduling/run-live.sh start example-config
      clay package inspect @fixture/lane    # against the scratch store
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/155-capability-grant-ui/live-example-config/`.
    - References:
      - user instruction 2026-09-07 (launch-test duty).
  - Test Cases to Write:
    - Manual launch leg: panel opens, lists, grants, revokes, and the CLI agrees.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: every surface/state in the approved artifact is captured from
      the running app and compared; each deviation is recorded as either fixed or
      re-approved through the prototype loop with the reason.
    - Performance: captures record the observed render timing on open and on
      state change, confirming no polling or per-keystroke work.
    - Code Quality: captures cover the shipped content themes, narrow and wide
      widths, keyboard-only operation (focus order, activation, confirmation),
      and the empty/loading/error states; `scripts/capture-ui-review.sh` fixtures
      are added for the new surface.
    - Security: the review confirms destructive actions require confirmation, that
      provenance/attribution facts are visible before a grant, and that no
      package-controlled text is rendered as trusted UI.
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/approved/package-capability-grants/README.md` (binding),
        `DESIGN.md` review checklist, `references/ui.md`.
      - `scripts/capture-ui-review.sh` and the `ui-review-*` fixture conventions.
    - Options Considered:
      - Rely on component tests only: rejected — the gate requires a running-app
        comparison against the approved artifact.
    - Chosen Approach: capture with the existing harness, compare per surface,
      record deviations and their resolution.
    - API Notes and Examples:
      ```bash
      scripts/capture-ui-review.sh --fixture ui-review-package-panel
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/155-capability-grant-ui/visual-review/` (captures +
        comparison table), `scripts/` fixture entry if added.
    - References:
      - user instruction 2026-09-11 (visual/accessibility review duty).
  - Test Cases to Write:
    - Review record: per-surface deviation table with resolution, plus the
      keyboard-only pass result.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: new numbered steps cover the panel path on a real Linux build —
      a pending package adopted and granted from the panel (positive), the same
      package failing closed with an ungranted declared capability (negative),
      revoke returning to fail-closed, a preset granting its exact set, and the
      provenance-changed grant showing as inert — with pass/fail recorded.
    - Performance: the steps record the startup timings and the `server.edit_ack`
      envelope from the live run, compared with the plan-136 baseline.
    - Code Quality: `test-plan/index.md` (module map, coverage matrix, execution
      record) and the parity ledger are updated in the same task; no existing step
      is weakened or deleted.
    - Security: the negatives prove fail-closed behavior and that the panel cannot
      grant beyond declarations or the adoption ceiling; the steps note that
      `clay:packages` stays trusted-domain-only.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/09-packages-and-modes.md` (P56-P58 from plan 136),
        `test-plan/index.md`, `tests/documentation_coverage.rs` (step-ID ledger).
      - `test-plan/artifacts/136-capability-grants/manual-plan/README.md`.
    - Options Considered:
      - Add a new module for package management UI: rejected — module 09 owns
        packages and modes; module 02/13 own shell surfaces if the panel is
        reached from there.
    - Chosen Approach: extend module 09 with the panel steps and record the
      execution.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage::parity_ledger
      ```
    - Files to Create/Edit:
      - `test-plan/09-packages-and-modes.md`, `test-plan/index.md`,
        `docs/development/tauri-react-parity-ledger.json`.
    - References:
      - plan 136 task 13 outcome (manual-plan execution pattern).
  - Test Cases to Write:
    - Manual steps with expected results, negatives, and ceilings; ledger and
      documentation-coverage tests stay green.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/third-party-runtime-authority.md` documents
      the in-app grant surface (what the panel shows, which service calls it
      makes, what it refuses), the preset model, and the snapshot that feeds it;
      the shell/UI page that owns the panel documents its state machine and the
      host-owned authority boundary; `docs/wiki/index.md` links them.
    - Performance: the wiki states the panel's render triggers and that no
      package-authority read is on a typing path.
    - Code Quality: pages follow the wiki template, link the authoritative API
      pages instead of duplicating them, and list source/test paths.
    - Security: the pages state that the panel is host-owned, that packages cannot
      inject rows/actions, that grants remain provenance-bound, and that revoke
      returns the system to fail-closed.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md` (wiki workflow
        and template), `tests/documentation_coverage.rs` (contract tests).
    - Options Considered:
      - Update per task: rejected — update once after tests pass (plan-136
        precedent).
    - Chosen Approach: update the authority page plus the shell page that owns the
      panel, add a contract test with markers, and update the index descriptions.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage::
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/wiki/modules/` (the shell surface page that owns the panel),
        `docs/wiki/index.md`, `tests/documentation_coverage.rs`.
    - References:
      - plan 136 task 14 outcome (wiki contract-test pattern).
  - Test Cases to Write:
    - `plan155_wiki_pages_describe_the_in_app_grant_surface`: marker test plus a
      stale-claim scan for "no package-manager surface"/"the GUI has no grant
      surface".

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
