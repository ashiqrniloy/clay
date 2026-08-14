# Clay Project Plan Requirements

Apply these requirements only when creating or updating plan documents for the Clay project.

## Primitive-First Mode and Package Task

Each Clay phase plan that implements or materially changes an editor mode, language mode, first-party JS package, package runtime capability, or reusable editor capability must include a separate primitive-review task before package/mode implementation tasks.

The task should require:

- Read existing primitive reference docs and implementation wiki pages before designing package behavior: `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, relevant strategy docs, `docs/wiki/modules/primitive-architecture.md`, and relevant primitive/module wiki pages.
- Inventory existing Rust-side primitives such as document classification, major-mode activation, commands/key routing, inert text transforms, parse handlers, decoration transport, SDUI, configuration, folding, completions, diagnostics, or other current surfaces.
- State what the new package/mode can achieve with existing primitives before proposing new Rust code.
- Plan new Rust primitives only when needed, and require them to be generic/reusable across future modes instead of named or shaped around a single language such as Markdown or Python.
- Build JS package functionality on top of those primitives; do not add mode-specific Rust server/client branches, parser logic, renderer callbacks, or package-specific client behavior.
- Add documentation and tests that keep every new/changed primitive recorded in reference docs, code wiki pages, wiki index navigation, and deterministic primitive-documentation checks.

Recommended task title:

```markdown
- [ ] Review existing editor primitives and plan generic primitive gaps before package work
```

Place this task after entry-gate/baseline tasks and before implementation or cleanup tasks that depend on the primitive assessment.

Decision source: `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`.

## Package Runtime Trust-Domain Task

Each Clay plan that adds or materially changes package execution, package loading, first-party packages, package extension points, package graph relations, or package-facing ops must include acceptance criteria and/or a dedicated task that preserves the two runtime trust domains.

The task should require:

- Trusted runtime classification comes from Clay's compiled bundled inventory and exact provenance/integrity, never `@clay/*` naming or normal user promotion.
- The adopted-package runtime installs only documented public package ops and narrow host state; Clay-internal ops and trusted module roots are absent.
- Cross-domain communication uses typed, bounded, inert Rust-mediated values with generation, payload, timeout, provenance, and revocation checks; no V8 objects/functions/globals/modules cross domains.
- Third-party changes to first-party behavior require both a first-party-declared extension point and explicit user approval. Replacement preserves third-party provenance and never moves replacement code into the trusted runtime.
- Tests prove cross-domain internal-op/module denial, stale-generation rejection, adoption/revocation, replacement rollback, and the documented lack of hostile isolation among third-party packages.

Decision source: `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.

## Package Default Loading Task

Each Clay phase plan that implements or materially changes a JS package, package runtime capability, editor mode package, package loader, or package configuration surface must include acceptance criteria and/or a dedicated task for the package's end-user `init.js` loading experience.

The task should require:

- The package is explicitly loaded from `~/.config/clay/init.js`; packages should not become behavior-changing defaults silently.
- The preferred default setup is a one-line explicit load command, such as `loadPackage("@clay/markdown")` or the implemented equivalent.
- Normal package defaults should work after the one-line load command without requiring copied package manifests, low-level Clay facade plumbing, manual primitive registration, test-only SDUI, or representative decoration publication in user config.
- Package/mode customization may be exposed through documented Clay/package JS APIs, but customization is optional for common use unless a specific package has a documented reason.
- If one-line default loading is not currently possible, the plan must identify the generic Clay primitive/API gap and document any longer setup as a temporary fallback or limitation, not as the preferred convention.
- Tests and docs should cover both the default package-load path and any supported customization path.

Recommended task title when a separate task is useful:

```markdown
- [ ] Define and verify the package default init.js loading experience
```

Place this task after the primitive-review task and before broad implementation cleanup tasks, or fold the requirements into package implementation tasks when the phase is small.

Decision source: `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`.

## External Process Authority Task

Each Clay plan that introduces or materially changes a package-triggered external process must include an authority-decision task before implementation. The task must require:

- A dedicated deny-by-default capability and explicit user approval bound to package provenance, a fixed contribution, canonical executable/literal argv, explicit inherited-environment names, and known workspace roots.
- No implicit grant from package load, bundled/first-party trust, `shell`, or `filesystem`; no runtime-selected executable, arguments, cwd, shell, or unrestricted environment.
- Bounded asynchronous I/O, timeout/concurrency budgets, sanitized diagnostics, revocation/reload/root-removal/runtime-replacement cleanup, and no process work in editor hot paths.
- Truthful containment language: cwd/root grants constrain Clay's API and audit record, not same-user OS filesystem/network/process access. Do not call the child sandboxed or workspace/filesystem confined without separately approved OS enforcement.
- Realistic alternatives, explicit user approval, a decision log, reusable project-pattern updates, and deny/revocation/lifecycle tests before process code starts.

Decision source: `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Package-Provided Grammar Task

Each Clay phase plan that implements or materially changes syntax highlighting, language grammar support, Tree-sitter integration, language packages, or language-mode expansion must include acceptance criteria and/or a dedicated task for package-provided grammar contributions.

The task should require:

- Syntax grammar support is expressed through generic package primitives, not hard-coded Rust branches for Rust, TypeScript, JavaScript, or any later language.
- `@clay/rust`, `@clay/typescript`, and `@clay/javascript` start as grammar-only packages when first introduced: grammar/query assets, language metadata, style-token mapping, docs, tests, and provenance, with no full mode behavior until a later expansion phase.
- Active syntax grammar remains separate from active major mode so grammar packages can attach highlighting to `core.code`/`core.text` fallback modes.
- Arbitrary third-party grammar/native artifact loading is out of scope unless a dedicated security/trust decision approves integrity, sandboxing, and user authorization rules.
- Tests cover package-provided grammar resolution, disabled/invalid package fallback, query/decoration payload bounds, no client-side JavaScript or parser code in paint/text hot paths, and no language-specific Rust server/client branches.

Recommended task title when a separate task is useful:

```markdown
- [ ] Review package-provided grammar primitives before language package work
```

Decision source: `decision-logs/2026-06-29-2006-package-provided-grammar-and-capability-phases.md`.

## Package UI/Layout and Authoring Documentation Task

Each Clay phase plan that implements or materially changes package UI, mode UI, SDUI, layout, pane/window behavior, component primitives, input routing, package actions, package state/data, styling/theme tokens, or package configuration must include acceptance criteria and/or a dedicated task for the package authoring contract.

The task should require:

- Clay remains the owner of the working area, pane/split tree, fixed pane slots, component catalog, action routing, theme/style token model, and native Masonry widget implementation.
- Packages declare inert UI/layout/input/action/data/style contributions through documented Clay/package JS APIs; they must not directly create Masonry widgets, mutate native layout, provide raw CSS, run client-side JavaScript, or call raw `Deno.core.ops`.
- Any new UI/layout primitive is generic and reusable across packages/modes, not Markdown-specific or package-specific Rust branching.
- Fixed vs transient panel behavior, slot ownership, package/user override precedence, action routing, focus/input routing, and style token mapping are documented and tested when introduced or changed.
- `docs/reference/packages/creating-packages.md` is updated in the same phase with implemented APIs, examples, limitations, migration notes, permissions, testing guidance, and any temporary fallback paths.

Recommended task title when a separate task is useful:

```markdown
- [ ] Update the package UI/layout authoring contract and package guide
```

Place this task near package UI/layout implementation tasks and before final documentation/wiki verification.

Decision source: `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.

## Clay JS API Task

Each Clay plan document must include a separate task near the end of the plan to create or verify Clay JavaScript APIs for public programmatic behavior and Rust public functions introduced or changed by the plan.

The task should require:

- Review the phase implementation and propose the Clay JS APIs needed for extensibility, configuration, customization, user search/help, key binding, AI-agent discovery, and future public programmatic use.
- Follow the dotted-ID naming convention: core Clay command/API/option IDs are bare `<domain>.<name>` (e.g. `shell.clientClosePane`, `editor.clientUndo`, `runtime.reloadConfiguration`) and must never use the retired `clay.<domain>.*` spelling; package-owned IDs always start with the package's own `apiPrefix` (`<package>.<name>`); new core domains must be added to `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs`. `clay:` import specifiers and `package.json` `clay.*` manifest key paths are exempt. See `.agents/skills/project-patterns/references/clay-js-api-naming.md`.
- Inventory all server-side Rust public functions introduced or changed by the plan.
- For each server-side Rust public function that is a public programmatic capability, expose it through an explicit `deno_core` op wrapper and stable Clay JS/TS facade API.
- Do not expose arbitrary Rust public functions directly to JavaScript.
- Do not make raw `Deno.core.ops.op_*` calls the user-facing API.
- If a server-side function should not be exposed to JavaScript, make it private or `pub(crate)` instead of public.
- Add or update Markdown documentation for every Clay JS API with: stable ID, searchable user-facing name, default key bindings or an empty key binding list, custom properties for behavior-changing settings, what it does, why/when to use it, JavaScript usage, code example, configuration/options, return/async behavior, errors, permissions/security notes, backing Rust path, op wrapper, JS facade path, and lookup tags.
- Link every Clay JS API doc from the master Markdown documentation index.
- Update the generated documentation registry using the project command when docs change.
- Ensure `cargo test` fails when a required Clay JS API, Markdown doc, master-index link, generated registry entry, key binding/custom property field, or lookup entry is missing/stale.

Recommended task title:

```markdown
- [ ] Create or verify Clay JS APIs for public programmatic surfaces
```

Place this task after implementation/verification tasks and before the final project-wiki task when both are present.

Decision sources:

- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
- `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`

## Clay Configuration Task

Each Clay plan document that adds or changes user-visible behavior, commands, key bindings, customization, extension points, server APIs, protocol capabilities, or public programmatic surfaces must include a separate configuration task.

The task should require:

- Review the phase implementation and propose configuration APIs needed for extensibility, customization, key binding, and user/agent discovery.
- Treat every configuration option as a Clay JS API, not as an undocumented configuration key.
- Use `~/.config/clay/init.js` as the user configuration entry point.
- Allow `init.js` to load other local configuration files for modular configuration when configuration loading is implemented.
- Add or update Clay JS API docs for configuration APIs, including user-facing name, key bindings, custom properties, examples, permissions/security notes, and lookup tags.
- Link configuration API docs from `docs/index.md` and update generated registry artifacts.
- Add tests or coverage gates that fail for undocumented configuration APIs or behavior-changing settings missing from `custom_properties`.
- Preserve security boundaries: configuration must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority.

Recommended task title:

```markdown
- [ ] Create or verify Clay configuration APIs
```

Place this task near the Clay JS API task and before the final project-wiki task when present.

Decision source: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.

## Example Configuration Maintenance Task

Each Clay plan document that introduces or materially changes a user-facing configuration surface — new `init.js`-callable Clay JS APIs, new options/custom properties on existing configuration APIs, new first-party packages users should load, new bindable command IDs, new theme/appearance/typography/caret/ligature options, or new trust-boundary declarations users must write (e.g. `clay.editorControl`) — must include a dedicated task that updates the canonical example configuration.

The task should require:

- Update `examples/init.js`, the canonical example configuration users copy to `~/.config/clay/init.js`. It must stay comprehensive: every supported configuration surface appears exactly once, in its section, with all documented options annotated in comments.
- Add the new option/API with the same documentation style: section comment explaining purpose and ownership, every option name/type/default/allowed value, and a commented example for non-default variants.
- Keep the file valid JavaScript (`node --check examples/init.js`) and keep the active (uncommented) part safe to copy verbatim: heavy or environment-specific setup (LSP grants, optional packages, behavior overrides) stays commented with instructions.
- Preserve the documented ordering constraints (e.g. `authorizeLanguageServer` before the first `loadPackage`) and the planned-but-not-callable section at the end when a facade is promoted from planned to implemented.
- Cross-check the example against the Clay JS API docs and `api-inventory.toml` custom properties for the touched APIs; option names, enums, and defaults must match the validated server-side parsers, not prose.

Recommended task title:

```markdown
- [ ] Update the canonical example configuration (examples/init.js)
```

Place this task next to the Clay Configuration task and before the final project-wiki task when present.

Decision source: user instruction 2026-08-03 (canonical `examples/init.js` + per-plan maintenance duty).

## Manual Test Plan Task

Each Clay plan document that changes user-visible behavior — editor features, UI, rendering, configuration, keybindings, packages/modes, file workflows, protocol/IPC behavior, or platform behavior — must include a dedicated task that runs and maintains the manual test plan in `test-plan/`.

The task should require:

- Identify which `test-plan/` module files the change affects (module map and coverage matrix live in `test-plan/index.md`) and execute the relevant steps on a real Linux build, recording pass/fail against the numbered steps.
- Add new numbered steps (module + step IDs) to the affected module file(s) for any new user-visible behavior the plan ships, with expected results, negative checks, and known ceilings.
- Update `test-plan/index.md` when a new module file is added, when the coverage matrix changes, or when a deep-reference doc moves.
- If the change cannot be tested manually (pure internal refactor, automated-only surface), the task records that explicitly with the reason instead of being silently dropped.
- Never weaken or delete existing steps to make a failing check pass; a failing step is a defect or a documented known ceiling (in the file's ceilings section), decided explicitly.
- Cross-link new module steps to the deep-reference docs under `docs/development/` where they exist instead of duplicating them.

Recommended task title:

```markdown
- [ ] Execute and update the manual test plan (test-plan/)
```

Place this task after implementation/verification tasks (the feature must be buildable) and before the final project-wiki task when present.

Decision source: user instruction 2026-08-04 (test-plan/ folder + per-plan manual verification duty).

## Clay UI Primitives-First Task

Each Clay plan that touches the app UI (components, panels, overlays, pop-ups, dropdowns, menus, text inputs, multi-selects, completion pop-ups, theme, typography, tokens, or layout) must route through UI skill selection and reuse the established UI catalog before proposing new UI code.

The plan should require:

- Before reviewing existing UI or planning, designing, or implementing any UI task, run `npx ui-skills start`; inspect the relevant category and load the smallest useful skill set (prefer 1, max 3). Repeat this per independently executed UI task and record the selected category/slugs in plan evidence.
- Load the `clay-ui` skill (`.agents/skills/clay-ui/`) and read its `references/components.md` and `references/tokens.md` before writing UI tasks. Read `docs/reference/ui-components.md` for the navigation/contract entry point that links the catalog, token tables, chrome primitives, package authoring guide, and Phase 20.7 conformance rules.
- Reuse cataloged components, primitives, style variables, and theme tokens first; a custom component outside the catalog requires explicit justification in the task's `Options Considered`.
- New components, primitives, tokens, or layout rules must be generic and reusable across packages, token-driven (no raw colors, CSS, concrete font families, or point sizes), and state-complete (hover/active/focus/disabled).
- Component kinds, style variables, and token names are additive-only so existing packages keep working.
- Keep the catalog current: the plan must include updating `.agents/skills/clay-ui/references/components.md` / `references/tokens.md` and `docs/reference/packages/creating-packages.md` for any UI surface change. Documentation drift across the catalog, `creating-packages.md`, `docs/reference/ui-components.md`, and `docs/index.md` fails `cargo test` (Phase 20.8).
- Preserve the shell layout contract: `main` slot plus optional `left`/`right`/`top`/`bottom` fixed panels whose sizes remain user-configurable (min/max/collapse/resize).

Recommended task title:

```markdown
- [ ] Review Clay UI catalog and plan primitive/component reuse before UI work
```

Place this task after entry-gate/baseline tasks and before UI implementation tasks.

## Mandatory UI Visual and Accessibility Review Task

Each Clay plan that touches app UI must include one post-implementation task that reviews the implemented interface visually and through accessibility tooling before final API/documentation/wiki work.

The task must require:

- Launch a real Linux GUI build using representative data and exercise every changed state: default, interactive/focus, empty/error/loading/recovery states when applicable, plus narrow and wide window layouts when layout is affected.
- Take and inspect screenshots for each exercised state. Store review evidence under a clearly named review artifact path, and record the path and findings in the task completion evidence.
- When `computer-use-linux` is available, call `get_app_state` before UI interaction, inspect its accessibility tree, and verify keyboard-only flow, focus visibility/order, role/name/state exposure, modal containment, and announcements for changed controls. Prefer semantic selectors; re-check state after each interaction.
- If GUI launch, screenshot capture, or computer use is unavailable, state the exact blocker, preserve automated structural/accessibility checks, and leave manual visual/a11y acceptance unresolved rather than claiming it passed.
- Treat a screenshot or accessibility failure as a product defect or an explicitly prioritized follow-up; do not replace it with source inspection alone.

Recommended task title:

```markdown
- [ ] Perform visual screenshot and accessibility review of changed UI
```

Place this task after UI implementation and automated verification, before Clay JS API/configuration/manual-test-plan/wiki finalization.

Decision source: `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
