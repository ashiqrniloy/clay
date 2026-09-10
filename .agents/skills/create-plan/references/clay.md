# Clay Project Plan Requirements

Apply these requirements only when creating or updating plan documents for the Clay project.

## Primitive-First Mode and Package Task

Each Clay phase plan that implements or materially changes an editor mode, language mode, first-party JS package, package runtime capability, or reusable editor capability must include a separate primitive-review task before package/mode implementation tasks. The task should require:

- Read existing primitive reference docs and wiki pages before designing package behavior: `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, relevant strategy docs, `docs/wiki/modules/primitive-architecture.md`, and relevant primitive/module wiki pages.
- Inventory existing Rust-side primitives (document classification, major-mode activation, commands/key routing, inert text transforms, parse handlers, decoration transport, SDUI, configuration, folding, completions, diagnostics, other current surfaces); state what the new package/mode achieves with them before proposing new Rust code.
- New Rust primitives only when needed, generic/reusable across future modes — never named or shaped around one language (Markdown, Python).
- Build JS package functionality on those primitives; no mode-specific Rust server/client branches, parser logic, renderer callbacks, or package-specific client behavior.
- Documentation and tests keep every new/changed primitive recorded in reference docs, wiki pages, wiki index navigation, and deterministic primitive-documentation checks.

Recommended title: `- [ ] Review existing editor primitives and plan generic primitive gaps before package work` — placed after entry-gate/baseline tasks, before dependent implementation/cleanup tasks. Decision source: `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`.

## Package Runtime Trust-Domain Task

Each Clay plan that adds or materially changes package execution, package loading, first-party packages, package extension points, package graph relations, or package-facing ops must include acceptance criteria and/or a dedicated task preserving the two runtime trust domains. It must require:

- Trusted runtime classification comes from Clay's compiled bundled inventory and exact provenance/integrity — never `@clay/*` naming or normal user promotion.
- The adopted-package runtime installs only documented public package ops and narrow host state; Clay-internal ops and trusted module roots are absent.
- Cross-domain communication uses typed, bounded, inert Rust-mediated values with generation, payload, timeout, provenance, and revocation checks; no V8 objects/functions/globals/modules cross domains.
- Third-party changes to first-party behavior require both a first-party-declared extension point and explicit user approval; replacement preserves third-party provenance and never moves replacement code into the trusted runtime.
- Tests prove cross-domain internal-op/module denial, stale-generation rejection, adoption/revocation, replacement rollback, and the documented lack of hostile isolation among third-party packages.

Decision source: `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.

## Package Default Loading Task

Each Clay phase plan that implements or materially changes a JS package, package runtime capability, editor mode package, package loader, or package configuration surface must include acceptance criteria and/or a dedicated task for the end-user `init.js` loading experience. It must require:

- The package is explicitly loaded from `~/.clay/init.js`; packages never become behavior-changing defaults silently. Preferred setup: one-line explicit load (`loadPackage("@clay/markdown")` or equivalent).
- Normal package defaults work after the one-line load without copied manifests, low-level facade plumbing, manual primitive registration, test-only SDUI, or representative decoration publication in user config.
- Customization may use documented Clay/package JS APIs but stays optional for common use unless a package has a documented reason.
- If one-line default loading is impossible, the plan identifies the generic Clay primitive/API gap and documents longer setup as temporary fallback/limitation, not preferred convention.
- Tests and docs cover both the default load path and any supported customization path.

Recommended title when a separate task helps: `- [ ] Define and verify the package default init.js loading experience` — after the primitive-review task, before broad cleanup (fold into package tasks for small phases). Decision source: `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`.

## External Process Authority Task

Each Clay plan that introduces or materially changes a package-triggered external process must include an authority-decision task before implementation, requiring:

- A dedicated deny-by-default capability and explicit user approval bound to package provenance, a fixed contribution, canonical executable/literal argv, explicit inherited-environment names, and known workspace roots. No implicit grant from package load, bundled/first-party trust, `shell`, or `filesystem`; no runtime-selected executable, arguments, cwd, shell, or unrestricted environment.
- Bounded asynchronous I/O, timeout/concurrency budgets, sanitized diagnostics, revocation/reload/root-removal/runtime-replacement cleanup, and no process work in editor hot paths.
- Truthful containment language: cwd/root grants constrain Clay's API and audit record, not same-user OS filesystem/network/process access; never call the child sandboxed or workspace/filesystem confined without separately approved OS enforcement.
- Realistic alternatives, explicit user approval, a decision log, reusable project-pattern updates, and deny/revocation/lifecycle tests before process code starts.

Decision source: `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Package-Provided Grammar Task

Each Clay phase plan that implements or materially changes syntax highlighting, language grammar support, Tree-sitter integration, language packages, or language-mode expansion must include acceptance criteria and/or a dedicated task for package-provided grammar contributions, requiring:

- Grammar support through generic package primitives — no hard-coded Rust branches for Rust, TypeScript, JavaScript, or any later language.
- `@clay/rust`, `@clay/typescript`, `@clay/javascript` start grammar-only (grammar/query assets, language metadata, style-token mapping, docs, tests, provenance) until a later expansion phase.
- Active syntax grammar stays separate from active major mode so grammar packages attach highlighting to `core.code`/`core.text` fallback modes.
- Arbitrary third-party grammar/native artifact loading is out of scope unless a dedicated security/trust decision approves integrity, sandboxing, and user authorization rules.
- Tests cover package grammar resolution, disabled/invalid package fallback, query/decoration payload bounds, no client-side JavaScript or parser code in paint/text hot paths, no language-specific Rust branches.

Recommended title when a separate task helps: `- [ ] Review package-provided grammar primitives before language package work`. Decision source: `decision-logs/2026-06-29-2006-package-provided-grammar-and-capability-phases.md`.

## Package UI/Layout and Authoring Documentation Task

Each Clay phase plan that implements or materially changes package UI, mode UI, SDUI, layout, pane/window behavior, component primitives, input routing, package actions, package state/data, styling/theme tokens, or package configuration must include acceptance criteria and/or a dedicated task for the package authoring contract, requiring:

- Clay owns the working area, pane/split tree, fixed pane slots, React component registry, action routing, theme/style token model, and Tauri/webview security boundary.
- Packages declare inert UI/layout/input/action/data/style contributions through documented Clay/package JS APIs; no host-layout mutation, uncontrolled host CSS, direct Tauri IPC, or raw `Deno.core.ops`. First-party trusted UI modules may be compiled into the frontend; arbitrary third-party custom UI requires an isolated surface.
- Empty/new-tab `main` is a package pane-content contribution (one winner); core fallback is Open File / Open Folder only. No product-named pane kinds (`Agent`) or irreplaceable native landings — agent profiles register via first-party packages (`loadPackage`), not compiled stubs.
- New UI/layout primitives are generic and reusable across packages/modes, never package-specific Rust branching.
- Fixed vs transient panel behavior, slot ownership, package/user override precedence, action routing, focus/input routing, and style token mapping are documented and tested when introduced or changed.
- `docs/reference/packages/creating-packages.md` is updated in the same phase: implemented APIs, examples, limitations, migration notes, permissions, testing guidance, temporary fallbacks.

Recommended title when a separate task helps: `- [ ] Update the package UI/layout authoring contract and package guide` — near package UI/layout implementation, before final documentation/wiki verification. Decision sources: `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`, `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`.

## Clay JS API Task

Each Clay plan document must include a separate task near the end to create or verify Clay JavaScript APIs for public programmatic behavior and Rust public functions introduced or changed by the plan. The task should require:

- Review the phase implementation and propose the Clay JS APIs needed for extensibility, configuration, customization, user search/help, key binding, AI-agent discovery, and future public programmatic use.
- Follow the dotted-ID naming convention: core IDs are bare `<domain>.<name>` (e.g. `shell.clientClosePane`, `editor.clientUndo`, `runtime.reloadConfiguration`), never the retired `clay.<domain>.*`; package-owned IDs start with the package's `apiPrefix`; new core domains go into `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs`. `clay:` import specifiers and `package.json` `clay.*` manifest key paths are exempt. See `.agents/skills/clay-execution/references/js-api.md`.
- Inventory all server-side Rust public functions introduced or changed; expose each public programmatic capability through an explicit `deno_core` op wrapper and stable Clay JS/TS facade; never expose arbitrary Rust public functions directly to JavaScript or make raw `Deno.core.ops.op_*` calls the user-facing API. If a function should not be exposed, make it private or `pub(crate)`.
- Add or update Markdown docs for every Clay JS API: stable ID, searchable user-facing name, default key bindings or an empty list, custom properties for behavior-changing settings, what/why/when, JavaScript usage, example, configuration/options, return/async behavior, errors, permissions/security notes, backing Rust path, op wrapper, JS facade path, lookup tags; link from the master docs index; update the generated registry when docs change.
- Ensure `cargo test` fails when a required Clay JS API, Markdown doc, master-index link, generated registry entry, key binding/custom property field, or lookup entry is missing/stale.

Recommended title: `- [ ] Create or verify Clay JS APIs for public programmatic surfaces` — after implementation/verification, before the final wiki task. Decision sources: `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`, `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`.

## Clay Configuration Task

Each Clay plan document that adds or changes user-visible behavior, commands, key bindings, customization, extension points, server APIs, protocol capabilities, or public programmatic surfaces must include a separate configuration task, requiring:

- Review the phase implementation and propose configuration APIs for extensibility, customization, key binding, user/agent discovery; treat every configuration option as a Clay JS API, not an undocumented key.
- `~/.clay/init.js` is the user configuration entry point; `init.js` may load other local configuration files for modular configuration when implemented.
- Add or update Clay JS API docs for configuration APIs (user-facing name, key bindings, custom properties, examples, permissions/security notes, lookup tags); link from `docs/index.md`; update generated registry artifacts.
- Add tests/coverage gates that fail for undocumented configuration APIs or behavior-changing settings missing from `custom_properties`.
- Preserve security boundaries: configuration never implicitly grants filesystem, network, shell, extension loading, AI mutation, or workspace authority.

Recommended title: `- [ ] Create or verify Clay configuration APIs` — near the Clay JS API task, before the final wiki task. Decision source: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.

## Example Configuration Maintenance Task

Each Clay plan that introduces or materially changes a user-facing configuration surface — new `init.js`-callable APIs, new options/custom properties, new first-party packages users should load, new bindable command IDs, new theme/appearance/typography/caret/ligature options, or new trust-boundary declarations users must write (e.g. `clay.editorControl`) — must include a dedicated task updating the canonical example configuration. The task should require:

- Update `examples/config/init.js` so it stays comprehensive: every supported configuration surface appears exactly once, in its section, with all documented options annotated in comments and the same documentation style (section comment on purpose/ownership, every option name/type/default/allowed value, commented example for non-default variants).
- Keep the file valid JavaScript (`node --check examples/config/init.js`); the active (uncommented) part stays safe to copy verbatim — heavy or environment-specific setup (LSP grants, optional packages, behavior overrides) stays commented with instructions.
- Preserve documented ordering constraints (e.g. `authorizeLanguageServer` before the first `loadPackage`) and the planned-but-not-callable section when a facade is promoted from planned to implemented.
- Cross-check the example against the Clay JS API docs and `api-inventory.toml` custom properties for touched APIs: option names, enums, and defaults must match validated server-side parsers, not prose.

Recommended title: `- [ ] Update the canonical example configuration (examples/config/init.js)` — next to the Clay Configuration task, before the final wiki task. Decision source: user instruction 2026-08-03 (canonical example config + per-plan maintenance duty).

## Example Configuration Live Launch-Test Task

Each Clay plan that includes an Example Configuration Maintenance Task must also include a separate task that launch-tests the real app against a copy of the canonical example config — updating the file without running the app is not sufficient. The task should require:

- Copy `examples/config/init.js` (plus `examples/config/packages/` when present) to an isolated scratch config root (e.g. temp `HOME`/`.clay`); never launch against the developer's real profile.
- Launch a real Linux GUI build (server + client) with that config and verify healthy startup: client reaches Connected, configuration evaluation commits a generation with no `configuration failed` diagnostics, shell responds to interaction (open a pane, run a command, open a menu).
- Exercise the surfaces the plan changed as loaded from the example config: theme/appearance/typography apply visually, `bindKey` commands fire, `loadPackage`'d packages register contributions (profiles, commands, language modes), new option values take effect. Verify design-system/theme selections render as the selected system (e.g. `setDesignSystem("@clay/design-glass")` shows glass recipes — rounded 1px-border controls — not the neobrutal fallback; a failed activation silently falls back and users report it as "the wrong design system").
- Record the launch command, scratch config path, and observed results in the task evidence. A broken or degraded app under the example config is a product defect (or an explicitly prioritized follow-up), never a docs-only fix.
- If a headless/sandboxed environment blocks the GUI launch, record the blocker, run the strongest available automated check (start the server against the copied config, assert the runtime generation commits without diagnostics), and leave live interactive acceptance unresolved rather than claiming it passed.

Recommended title: `- [ ] Launch-test the app with the canonical example config` — immediately after the maintenance task, before the manual-test-plan task; both belong to the configuration-change phase so drift is caught in-phase. Decision source: user instruction 2026-09-07 (plan 109 review found the app broken under a copied example config that had never been launch-tested).

## Manual Test Plan Task

Each Clay plan document that changes user-visible behavior — editor features, UI, rendering, configuration, keybindings, packages/modes, file workflows, protocol/IPC, platform behavior — must include a dedicated task running and maintaining the manual test plan in `test-plan/`. The task should require:

- Identify affected `test-plan/` module files (module map and coverage matrix in `test-plan/index.md`) and execute relevant steps on a real Linux build, recording pass/fail against the numbered steps.
- Add new numbered steps (module + step IDs) for any new user-visible behavior, with expected results, negative checks, and known ceilings; update `test-plan/index.md` when a module file is added, the coverage matrix changes, or a deep-reference doc moves.
- If the change cannot be tested manually (pure internal refactor, automated-only surface), record that explicitly with the reason instead of silently dropping the task.
- Never weaken or delete existing steps to make a failing check pass; a failing step is a defect or a documented known ceiling (in the file's ceilings section), decided explicitly.
- Cross-link new module steps to deep-reference docs under `docs/development/` where they exist instead of duplicating them.

Recommended title: `- [ ] Execute and update the manual test plan (test-plan/)` — after implementation/verification (feature must be buildable), before the final wiki task. Decision source: user instruction 2026-08-04 (test-plan/ folder + per-plan manual verification duty).

## Clay UI Primitives-First Task

Each Clay plan that touches the app UI (components, panels, overlays, pop-ups, dropdowns, menus, text inputs, multi-selects, completion pop-ups, theme, typography, tokens, layout) routes UI skill loading through `.agents/skills/clay-execution/` before proposing new UI code. The plan should require:

- Before reviewing, designing, or implementing each UI task, read `.agents/skills/clay-execution/references/ui.md` (distilled binding rules, design-skill routing, shell layout model, client architecture) plus `references/components.md` and `references/tokens.md` (the catalogs) and list them under every UI task's `Approach -> Documentation Reviewed`; plan-level mention alone is insufficient. Substantial new-surface design tasks additionally load the four project-local design skills (`impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend`). Read `docs/reference/ui-components.md` as the navigation/contract entry point.
- Reconcile conflicting aesthetic guidance through the user brief, Clay product identity, accessibility, security, authority, catalog compatibility, and typed token ownership; adapt marketing-page guidance to Clay's Operate-mode desktop UI instead of forcing AIDA, hero sections, hardcoded palettes/fonts, or decorative motion.
- Reuse cataloged components, primitives, style variables, and theme tokens first; a custom component outside the catalog requires explicit justification in `Options Considered`.
- New components, primitives, tokens, or layout rules are generic and reusable across packages, token-driven (no raw colors, uncontrolled package CSS, concrete font families, or point sizes), and state-complete (hover/active/focus/disabled). Target web components consume host-generated CSS custom properties with the same semantic token ownership. Component kinds, style variables, and token names are additive-only so existing packages keep working.
- Keep the catalog current: update `.agents/skills/clay-execution/references/components.md` / `references/tokens.md` and `docs/reference/packages/creating-packages.md` for any UI surface change. Documentation drift across the catalog, `creating-packages.md`, `docs/reference/ui-components.md`, and `docs/index.md` fails `cargo test` (Phase 20.8).
- Preserve the shell layout contract: `main` slot plus optional `left`/`right`/`top`/`bottom` fixed panels whose sizes remain user-configurable (min/max/collapse/resize).
- Apply `.agents/skills/clay-execution/references/ui.md` (Client Architecture) and `references/packages.md` (Authority Boundaries) to all client/migration plans: separate server authority, CodeMirror-local typing, narrow Tauri capabilities, stable-ID React reconciliation, no permanent dual client.

Recommended title: `- [ ] Review Clay UI catalog and plan primitive/component reuse before UI work` — after entry-gate/baseline tasks, before UI implementation tasks.

## UI Design-System Package Task

Each Clay plan that adds or changes UI design-system packages, component recipes, design-system selection, or recipe-driven component styling must preserve the approved typed recipe boundary. The task should require:

- Keep content themes, user-owned typography, and UI design systems as separate configuration and invalidation layers; preserve `theme.setTheme`/`theme.setTypography` compatibility.
- Content themes stay the sole normal-rendering color authority. Recipes may map component slots/states to semantic active-theme color roles and apply typed opacity/effects, but must reject palettes, literals, and package-owned color values; browser/OS system colors are reserved for forced-colors mode.
- Design systems are versioned, inert `clay.contributions` data mapping host-owned component kinds, semantic slots, variants, and interaction states to typed non-color visual recipe properties and semantic theme-color-role references.
- Reject raw CSS, selectors, JSX, scripts, renderer callbacks, URLs, literal colors, color aliases/palettes, arbitrary transforms/filters, and direct Tauri APIs. React Aria and Clay retain behavior, focus, accessibility semantics, and DOM ownership.
- Validate exact package provenance, current generation, schema version, property/token types, color-role references, bounds, recipe completeness, contrast across representative themes, reduced-motion/transparency fallbacks, and revocation before atomic install.
- Keep component kinds, slots, recipe properties, tokens, and style variables additive and versioned; existing fixed non-color recipes remain fallback until migration completes; fallback colors always resolve through active-theme roles.
- Include both restrained Neobrutal and Glass conformance fixtures across at least two materially different content themes; the abstraction is incomplete if either requires host component source changes or declares a concrete color.
- Apply `.agents/skills/clay-execution/references/config.md` (UI Design-System Packages) and route UI skills through `.agents/skills/clay-execution/references/ui.md` (distilled rules; four design skills load only for substantial new-surface design tasks).

Recommended title: `- [ ] Review and implement the typed UI design-system recipe boundary`. Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.

## Mandatory UI Visual and Accessibility Review Task

Each Clay plan that touches app UI must include one post-implementation task reviewing the implemented interface visually and through accessibility tooling before final API/documentation/wiki work. The task must require:

- Launch a real Linux GUI build using representative data and exercise every changed state: default, interactive/focus, empty/error/loading/recovery when applicable, plus narrow and wide window layouts when layout is affected.
- Take and inspect screenshots for each exercised state; store review evidence under a clearly named artifact path and record the path and findings in the task completion evidence.
- When `computer-use-linux` is available, call `get_app_state` before UI interaction, inspect its accessibility tree, and verify keyboard-only flow, focus visibility/order, role/name/state exposure, modal containment, and announcements for changed controls; prefer semantic selectors and re-check state after each interaction.
- If GUI launch, screenshots, or computer use are unavailable, state the exact blocker, preserve automated structural/accessibility checks, and leave manual visual/a11y acceptance unresolved rather than claiming it passed.
- Treat a screenshot or accessibility failure as a product defect or an explicitly prioritized follow-up; never replace it with source inspection alone.

Recommended title: `- [ ] Perform visual screenshot and accessibility review of changed UI` — after UI implementation and automated verification, before Clay JS API/configuration/manual-test-plan/wiki finalization. Decision source: `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.

## Final Code Wiki Task

Each Clay plan whose wiki workflow applies (per `.agents/skills/clay-execution/references/docs-as-code.md`) includes exactly one final wiki task after implementation, verification, API/documentation maintenance, and project-specific maintenance tasks. The task must require:

- Update the code wiki after all implementation tasks complete, or explicitly verify it is unchanged for non-code work; updates add no runtime work and document performance-relevant implementation details the plan changed.
- Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, and examples where useful, linked from the master wiki index (`docs/wiki/index.md`); completed-phase review records go to `docs/wiki/archive/`, never `modules/`.
- Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
- Update once after tests pass (not per-task), using the `docs-as-code.md` wiki workflow, quality bar, and archive policy; edit `docs/wiki/index.md` navigation plus the relevant `docs/wiki/**` pages.

Template:

```markdown
- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow, quality bar, and archive policy.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code. (Chosen.)
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation links for changed implementation areas.
      - `docs/wiki/**`: implementation wiki pages for changed code.
  - Test Cases to Write:
    - Manual wiki review: the master index links relevant pages and updated pages explain what changed implementation does and how it works.
```
