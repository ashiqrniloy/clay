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
