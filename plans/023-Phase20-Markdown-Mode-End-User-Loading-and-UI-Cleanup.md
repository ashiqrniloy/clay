# Phase 20 Markdown Mode End-User Loading and UI Cleanup

> **Phase 18.5 Replan (2026-06-15):** This plan was rewritten by `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md` so that every task consumes the generic shell/package UI primitives promoted in Phases 18.1–18.4 (`WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, `PackageOwnedConfiguration`) instead of driving Markdown UI from fixture-only behavior. The mapping of every Markdown need to an existing generic primitive is recorded in `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md`. The one-line loading target (`loadPackage("@clay/markdown")`) and the explicit `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` binding remains separate from package loading (the one-line load and Ctrl+O separation is preserved). No Markdown-specific Rust editor/parser/render/shell branch is introduced by any task below.
>
> **Phase 18.6 dependency (2026-06-15):** Task 3 (one-line package loader) requires the Phase 18.6 `loadPackage("@clay/*")` resolver. The generic `loadPackage` was deferred from Phase 18.5 with a decision-log-backed rationale (`decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`). Until Phase 18.6 ships, the package-owned fallback (`import { markdownLoadMode } from "@clay/markdown"; await markdownLoadMode();`) is the documented temporary end-user loading path. Task 3 should be revisited once Phase 18.6 is complete. Task 4 (remove default side panel) was deferred from Phase 18.5 because the Markdown package load path already does not publish a default side panel; the panel only exists in test fixtures and the `connection.rs` selected-file-open evaluation. This task now owns the full `clay:ui` `PanelContribution` conversion, fixture simplification, and `connection.rs` cleanup.

## Objectives
- Make the first-party Markdown mode usable through an end-user configuration surface that consumes generic shell/package primitives, instead of a smoke-fixture-only script that inlines a package manifest object and manually calls per-facade registration helpers.
- Publish **no** default Markdown side panel on load. The Markdown editor occupies the mandatory `main` slot of `PaneSlotLayout`; an optional Markdown preview/status panel is a package `PanelContribution` (via `clay:ui`) targeting a Clay slot such as `right`, with `defaultVisibility: "hidden"`, and is shown only when a user/package option explicitly enables it through `setPackageOption` or `serverSetLayoutOverride`.
- Reduce Markdown setup in `~/.config/clay/init.js` to a one-line default load path through a generic `loadPackage("@clay/markdown")` specifier resolver (or a documented package-owned fallback that consumes generic primitives internally), while preserving a package-owned customization path for later options.
- Keep `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` as the user-configured Windows file-open binding, separate from package loading. Selected-file open activation reuses generic `MajorModeActivation` + `DocumentClassification`, not a Markdown-specific Rust branch.
- Preserve Clay's primitive-first package architecture: Rust exposes only generic package-loading and shell/package-UI primitives; Markdown-specific registration, parser, decoration, behavior-manifest, and panel data remain in `@clay/markdown`.

## Expected Outcome
- A normal Windows 11 user can run `cargo run` with a small `~/.config/clay/init.js` that loads Markdown defaults and binds the open-file command.
- Default Markdown loading requires no copied smoke fixture, no inline package manifest object, no explicit facade plumbing imports for commands/decorations/modes/packages/parse/SDUI, no manual `publishTree(...)` panel publication, and no root-level `EditorWidget` ownership assumption. The main editor is placed through the mandatory `main` slot of `PaneSlotLayout`.
- The default Markdown editing UI shows the editor only; no Markdown preview/status `PanelContribution` is published unless a future explicit `setPackageOption` / `serverSetLayoutOverride` customization enables it.
- Opening a Markdown file through the configured `Ctrl+O` path still installs Markdown behavior/decorations for the selected document through generic `MajorModeActivation` + `DocumentClassification` when the Markdown package is loaded.
- Existing package/security/performance boundaries remain intact: no client-side package JavaScript, no parser work on keypress/paint, no broad filesystem authority, no shell/network/AI/WASM/raw-op authority, and no Markdown-specific Rust parser, editor, or shell branch.

## Rejected Implementation Shapes
- Do not add `MarkdownLoader`, `MarkdownSidebar`, `MarkdownPreviewPanel`, `MarkdownPaneLayout`, `MarkdownModeDefault`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust editor/parser/render/shell branch. The one-line loader must be a generic `loadPackage(specifier)`; Markdown only publishes a `loadEntry`.
- Do not keep the inline package manifest object (`const markdownPackage = { ... }`), the manual per-facade registration imports, or the fixture-only `publishTree(...)` call as the documented end-user `init.js` path.
- Do not publish a default fixed or transient Markdown panel on load. The optional preview is a `PanelContribution` with `defaultVisibility: "hidden"`, invoked only by an optional helper or explicit option.
- Do not treat hidden config keys, raw CSS, raw style strings, raw colors, or arbitrary JSON state blobs as package authoring APIs. All overrides flow through `setPackageOption` / `serverSetLayoutOverride`.

## Tasks

- [ ] Confirm the end-user Markdown UX contract and remove fixture behavior from the product baseline
  - Acceptance Criteria:
    - Functional: The plan starts from a written baseline distinguishing smoke-only behavior from the desired end-user behavior: one-line Markdown loading through a generic `loadPackage("@clay/markdown")` resolver (or documented package-owned fallback), explicit `Ctrl+O` binding, no default `PanelContribution`, main editor placement through the `PaneSlotLayout` `main` slot, and edit-only selected-file open until selected-file save is implemented later.
    - Performance: The baseline states that Markdown loading, contribution descriptor validation, and selected-file activation run at configuration/open time, while ordinary typing, paint, scroll, layout, and text-event handling stay local/non-blocking and read only installed inert shell/contribution state.
    - Code Quality: The baseline identifies duplicated fixture code (inline `markdownPackage` manifest object, manual `serverLoadPackage`/`serverRegisterModePattern`/`serverActivateMajorMode`/`serverRegisterCommand`/`serverRegisterParseHandler`/`publishTree` calls) and ad hoc user-config responsibilities that must move behind a generic `loadPackage` resolver and package-owned contribution helpers.
    - Security: The baseline records that simplifying `init.js` must not broaden package, filesystem, workspace, shell, network, AI, WASM, raw-op, native-widget, raw-CSS, renderer-callback, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`: Phase 18.5 replan scope and entry-gate gaps.
      - `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md`: Markdown-to-generic-primitive mapping and the `loadPackage` gap.
      - `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`, `plans/021-Phase18.5-Large-File-Markdown-Performance-and-Memory.md`, `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`: Prior Markdown POC, large-file, and Windows open-dialog context.
      - `docs/reference/primitives/shell-layout-strategy.md` and `docs/reference/primitives/package-loading.md`: Canonical shell/layout and package-loading contracts.
      - `.agents/skills/project-patterns/references/planning-checklist.md`, `configuration-system.md`, `authority-boundaries.md`, `package-ui-layout.md`.
    - Options Considered:
      - Keep the current large fixture as the documented setup: rejected because it inlines a package manifest object and manual facade plumbing, which is not usable or maintainable for end users.
      - Hard-code Markdown mode activation and a default panel in Rust startup: rejected because it violates primitive-first package ownership and the no-default-panel contract.
      - Define a concise package-owned default loader reachable through a generic `loadPackage("@clay/markdown")` resolver, keep only the explicit file-open key binding in user config, and publish no default `PanelContribution`: selected.
    - Chosen Approach:
      - First document the target product behavior and test contract in generic-primitive terms, then make implementation tasks move fixture responsibilities behind generic package-loading and shell/package-UI primitives.
    - API Notes and Examples:
      ```js
      // Target default user setup. Exact load API spelling is finalized by the loader task.
      import { bindKey } from "clay:keybindings";
      import { loadPackage } from "clay:packages";

      await loadPackage("@clay/markdown");
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`: Update manual Markdown/open-file instructions to separate fixture validation from actual app setup.
      - `docs/reference/packages/markdown.md`: Record the intended default UX, `PaneSlotLayout.main` editor placement, and no-default-`PanelContribution` behavior.
      - `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md`: Keep plan status and verification notes current during execution.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
      - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
  - Test Cases to Write:
    - Documentation guard: actual-app Markdown instructions show a minimal package load plus explicit `bindKey`, not the smoke fixture manifest block.
    - Documentation guard: selected-file save remains out of scope and default Markdown mode does not require a `PanelContribution`.

- [ ] Confirm the generic primitive inventory and Markdown-to-generic mapping before package work
  - Acceptance Criteria:
    - Functional: Existing generic primitives are confirmed before implementation: `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, `PackageOwnedConfiguration`, `serverLoadPackage`, `MajorModeActivation`, `DocumentClassification`, `CommandDeclaration`, behavior manifests, command registry, parse coordinator, decoration transport, configuration runtime, and selected-file open activation. Every Markdown need maps to one of these; the only generic gap is the one-line `loadPackage("@clay/markdown")` specifier resolver.
    - Performance: The review classifies work as configuration/load time, package validation time, explicit command/UI update time, background parse/decor time, behavior-manifest update time, or editor hot-path work, and preserves the no-hot-path-package-JS rule.
    - Code Quality: Any new Rust work is generic (a `loadPackage` specifier resolver or package-service bridge). No Markdown-specific Rust editor/parser/render/shell branch is planned.
    - Security: The review documents allowed imports, package provenance, permission validation, deny-by-default specifier resolution, and why simplified user config does not grant extra authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md`: The Phase 18.5 primitive review that already inventories primitives (`serverPublishDecorations` and decoration transport included), maps Markdown needs, and identifies the `loadPackage` gap.
      - `docs/reference/primitives/index.md`, `registry.md`, `backlog.md`, `shell-layout-strategy.md`, `package-security.md`, `package-loading.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `package-distribution.md`, `package-ui-layout.md`, `behavior-manifests.md`.
    - Options Considered:
      - Re-derive a new Phase 20 primitive review: rejected because the Phase 18.5 review already covers the Markdown-to-generic mapping.
      - Add a `clay:markdown` Rust facade that performs all Markdown setup: rejected unless it is a thin generic module alias, because package logic should remain in the package.
      - Consume the existing Phase 18.5 primitive review as the authoritative mapping and implement only the generic `loadPackage` gap: selected.
    - Chosen Approach:
      - Treat `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md` as the authoritative primitive inventory and Markdown-to-generic mapping for this plan. Add deterministic coverage so the mapping cannot drift.
    - API Notes and Examples:
      ```text
      init.js -> loadPackage("@clay/markdown") -> PackageService validation ->
        package loadEntry (./dist/load.js) -> documented Clay facades ->
        MajorModeActivation + behavior manifest + parse handler + decorations for the active document.
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md`: Already authoritative; keep current as implementation proceeds.
      - `tests/primitives_docs.rs`: Keep deterministic coverage for the review, the Markdown-to-generic mapping, and the `loadPackage` gap.
    - References:
      - `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
  - Test Cases to Write:
    - `phase18_5_markdown_replan_primitive_review_records_existing_inventory`: Review lists current generic primitives.
    - `phase18_5_markdown_replan_primitive_review_maps_markdown_to_generic_primitives`: Review maps Markdown needs to generic primitives, not Markdown-specific Rust branches.
    - `phase18_5_markdown_replan_primitive_review_identifies_load_package_gap`: Review identifies `loadPackage` as the only generic gap and rejects a Markdown-specific loader.

- [ ] Add a generic one-line package loader reachable from user configuration
  - Acceptance Criteria:
    - Functional: User config can load default Markdown mode through a generic `loadPackage("@clay/markdown")` specifier resolver that resolves the installed package, reads its declared `loadEntry`, validates it through the existing `PackageService` path, and executes the package-owned load entry. If a safe generic resolver cannot be implemented in this phase, the gap is documented in a decision log and a package-owned fallback (`import { markdownLoadMode } from "@clay/markdown"; await markdownLoadMode();`) consumes generic primitives (`serverLoadPackage`, `serverRegisterModePattern`, `serverActivateMajorMode`, `serverRegisterCommand`, `serverRegisterParseHandler`) internally. Either way, the end-user `init.js` is concise.
    - Performance: Default loading runs at configuration/document-open time only; it does not run package JavaScript from keypress, paint, layout, scroll, or text-event handlers.
    - Code Quality: The resolver is generic (a `loadPackage(specifier)` for any first-party `@clay/*` package), deny-by-default for arbitrary external specifiers, package-manager execution, registry fetching, and arbitrary specifier expansion. The Markdown package only publishes a `loadEntry`; no `MarkdownLoader` or `if package == "@clay/markdown"` Rust branch exists. Public package exports use the registered `markdown` prefix.
    - Security: The loader uses documented Clay facades and existing server-side validators; it does not expose raw Deno ops or grant filesystem, network, shell, AI, WASM, client-side JavaScript, package enable/disable, or workspace expansion authority.
  - Approach:
    - Documentation Reviewed:
      - `packages/markdown/dist/load.js` and `packages/markdown/dist/index.js`: Existing package-owned manifest/rules/commands/policy helpers and `loadMarkdownPackage(clay, options)` entry.
      - `tests/fixtures/configuration/markdown-mode/init.js` and `tests/fixtures/configuration/windows-markdown-open/init.js`: Current duplicated fixture setup to collapse.
      - `runtime/js/packages.ts`, `src/server/ops/packages.rs`, `src/packages/service.rs`: Current `serverLoadPackage(packageJson)` surface to extend with a specifier resolver.
      - `docs/reference/packages/markdown.md` and `docs/reference/primitives/package-loading.md`: Public package contract and loading primitive contract.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`, `package-distribution.md`, `configuration-system.md`.
    - Options Considered:
      - `loadPackage("@clay/markdown")`: preferred end-user convention; package loading is explicit in `init.js` and defaults remain concise.
      - Constrained first-party `@clay/*` resolver only, deny-by-default for external packages: selected scope for a safe implementation.
      - `import { markdownLoadMode } from "@clay/markdown"; await markdownLoadMode();`: package-owned fallback, selected only if the generic resolver cannot be implemented safely in this phase.
      - `import "@clay/markdown/auto";`: possible last-resort fallback, used only if neither generic nor explicit package-owned forms can be implemented cleanly.
    - Chosen Approach:
      - Implement a constrained first-party `loadPackage("@clay/*")` resolver that reuses `PackageService` validation and executes the declared `loadEntry`. If authority, validation, or scope constraints prevent a safe implementation, document the gap in a decision log and ship the package-owned fallback that consumes generic primitives internally.
    - API Notes and Examples:
      ```js
      // Preferred one-line default.
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/markdown");

      // Package-owned fallback if the generic resolver is deferred.
      import { markdownLoadMode } from "@clay/markdown";
      await markdownLoadMode({});
      ```
    - Files to Create/Edit:
      - `runtime/js/packages.ts`: Add or promote the `loadPackage` facade if implemented.
      - `src/server/ops/packages.rs`: Add `op_clay_packages_load_package_by_specifier` or equivalent if implemented.
      - `src/packages/service.rs`: Add specifier resolution and load-entry execution if implemented.
      - `packages/markdown/dist/load.js` and `packages/markdown/dist/index.js`: Ensure the package-owned default load entry is correct and imports Clay facades internally.
      - `packages/markdown/package.json`: Ensure exports and Clay metadata (declared `loadEntry`) support the chosen load path.
      - `tests/markdown_mode.rs`, `tests/package_loading.rs`, `src/server/js_runtime.rs` tests: Add load-path coverage.
    - References:
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
      - `docs/wiki/modules/first-party-markdown-package.md`
      - `docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md`
  - Test Cases to Write:
    - Runtime config test: `loadPackage("@clay/markdown")` (or documented fallback) loads the package, registers Markdown mode, commands, parse handler, and behavior manifest for document `1` without user-supplied facade plumbing.
    - Runtime config test: package-owned fallback, if used, produces the same default package/mode/parse registration when called with no options.
    - Static guard: Markdown default loader source imports documented `clay:*` facades, not raw `Deno.core.ops`.
    - Static guard: package/module resolver allows only the intended first-party `@clay/*` package specifiers and continues rejecting arbitrary external imports.

- [ ] Remove the default Markdown side panel and make the optional preview a `PanelContribution`
  - Acceptance Criteria:
    - Functional: Loading Markdown mode by default does not publish a `PanelContribution` (or any `publishTree(...)` side panel). The optional Markdown preview/status panel is implemented as a package `PanelContribution` via `clay:ui` targeting a slot such as `right`, with `kind: "fixed"` and `defaultVisibility: "hidden"`, invoked only by an optional `registerMarkdownPreview()` helper or an explicit `setPackageOption` / `serverSetLayoutOverride`. The main editor remains in the mandatory `main` slot of `PaneSlotLayout`. The `connection.rs` selected-file-open evaluation (`evaluate_markdown_open` / `markdown_open_init_source`) does not publish a default preview/status side panel; it publishes behavior/decorations only.
    - Performance: Removing the default panel avoids unnecessary SDUI snapshot work during ordinary Markdown loading and selected-file activation. The optional panel, if enabled, uses bounded inert `clay:ui` declarations and does not run package JavaScript in paint/layout.
    - Code Quality: Package code uses generic `clay:ui` APIs and `PaneSlotLayout` concepts rather than hard-coded `SIDEBAR_WIDTH` or root-level `EditorWidget` assumptions. Test fixtures (`tests/fixtures/configuration/markdown-mode/init.js` and `tests/fixtures/configuration/windows-markdown-open/init.js`) are simplified to use the default load path without `publishTree(...)` panel publication. Tests that previously depended on default panel labels are rewritten to assert mode/decor/behavior state or moved to optional-panel tests.
    - Security: The package `PanelContribution` is validated by the server-side `PackageUiRegistry` for prefix, slot, action targets, permissions, and payload budgets before client publication.
  - Approach:
    - Documentation Reviewed:
      - `src/shell/package_ui.rs`, `src/server/ui.rs`, `src/masonry_sdui.rs`: Package UI runtime and native rendering paths.
      - `runtime/js/ui.ts`: `serverRegisterPanelContribution` and `ComponentContribution` APIs.
      - `docs/reference/primitives/shell-layout-strategy.md`: Fixed/transient panel rules and slot behavior.
      - `docs/reference/packages/creating-packages.md`: Package author anti-patterns and `clay:ui` examples.
      - `tests/fixtures/configuration/markdown-mode/init.js` and `tests/fixtures/configuration/windows-markdown-open/init.js`: Current fixture-published panels that must be simplified.
      - `packages/markdown/dist/sdui.js` and `packages/markdown/dist/load.js`: Package-owned SDUI and load behavior — `loadMarkdownPackage` / `markdownLoadMode` already do not publish a default side panel.
      - `src/server/connection.rs::markdown_open_init_source` and `evaluate_markdown_open`: Selected-file activation currently publishes Markdown preview/status SDUI including a side panel; this must be converted to behavior/decorations only unless an explicit option enables it.
      - `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`: Deferred task 5 rationale.
    - Options Considered:
      - Delete all Markdown SDUI/panel code: rejected because the optional preview capability is still useful for future customization.
      - Keep the panel code but make it an optional `PanelContribution` registered through `clay:ui` with `defaultVisibility: "hidden"`: selected.
      - Hide the panel client-side only: rejected because the server should not publish irrelevant default UI state in the first place.
      - Move panel logic into a separate Markdown sub-package: rejected because it adds unnecessary complexity for a single optional feature.
    - Chosen Approach:
      - Update the Markdown package so it does not call `publishTree(...)` or register a default panel. Add an optional `registerMarkdownPreview()` helper that uses `serverRegisterPanelContribution` targeting the `right` slot with `defaultVisibility: "hidden"`. Simplify test fixtures to use the default load path without panel publication. Convert the `connection.rs` selected-file-open path to publish behavior/decorations only (no default side panel SDUI tree) unless an explicit option enables the preview.
    - API Notes and Examples:
      ```ts
      // Inside package load entry (not user config)
      import { serverRegisterPanelContribution } from "clay:ui";

      // Optional preview panel, not published by default
      export function registerMarkdownPreview() {
        serverRegisterPanelContribution(manifest, {
          id: "markdown.preview",
          slot: "right",
          kind: "fixed",
          defaultVisibility: "hidden",
          component: { kind: "panel", id: "markdown.preview.root", title: "Preview", children: [] },
          actionTargets: ["markdown.togglePreview"]
        });
      }
      ```
    - Files to Create/Edit:
      - `packages/markdown/dist/load.js`: Remove default panel publication; add optional preview helper if not already present.
      - `packages/markdown/dist/index.js`: Export the default load entry and optional helpers.
      - `packages/markdown/dist/sdui.js`: Narrow or document as optional only.
      - `tests/fixtures/configuration/markdown-mode/init.js`: Simplify to default load path without `publishTree(...)` panel publication.
      - `tests/fixtures/configuration/windows-markdown-open/init.js`: Simplify to default load path + `bindKey` without `publishTree(...)` panel publication.
      - `src/server/connection.rs`: Stop publishing Markdown preview/status SDUI side panel during selected-file open (`evaluate_markdown_open` / `markdown_open_init_source`) unless an explicit option enables it. Publish behavior manifests and decorations only by default.
      - `tests/markdown_mode.rs`, `tests/manual_smoke_docs.rs`: Update assertions away from default panel labels.
    - References:
      - `docs/wiki/modules/server-driven-ui.md`
      - `docs/wiki/modules/slot-aware-package-ui.md`
      - `docs/wiki/modules/first-party-markdown-package.md`
      - `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md` (deferred task 5 rationale)
  - Test Cases to Write:
    - `markdown_default_load_does_not_publish_side_panel`: Verifies fixture and package load paths produce no default panel.
    - `markdown_optional_preview_is_valid_panel_contribution`: Verifies the optional preview helper, if called, produces a valid `clay:ui` `PanelContribution` with provenance.
    - `selected_markdown_file_opens_without_default_panel`: Verifies selected-file activation (`connection.rs`) does not publish the side panel SDUI by default.
    - `markdown_fixture_simplification_no_publishTree`: Verifies the simplified fixtures do not contain `publishTree(...)` panel code.

- [ ] Keep the Windows open-file binding explicit and verify selected-file Markdown activation uses generic mode activation
  - Acceptance Criteria:
    - Functional: `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` remains the documented way to open the Windows file explorer, and selected Markdown files still activate Markdown behavior/decorations through generic `MajorModeActivation` + `DocumentClassification` when the package has been loaded.
    - Performance: The `Ctrl+O` route remains a client-local manifest lookup followed only by explicit modal native UI and server selected-file open work.
    - Code Quality: Selected-file Markdown activation reuses the generic package-owned loader/helper path and generic mode activation rather than generating a second divergent Markdown init script or a Markdown-specific Rust branch.
    - Security: A selected path remains a single-file server grant after explicit user selection; loading Markdown does not expand workspace roots or parent-directory authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/documents/client-open-file-dialog.md`: Client UI command ID contract.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: User keybinding API.
      - `docs/wiki/modules/client-file-dialog.md`: Windows file dialog backend and selected-path authority boundary.
      - `docs/wiki/flows/client-behavior-routing.md`: Client UI command routing through behavior manifests.
      - `docs/wiki/modules/markdown-mode-activation.md`: Generic `MajorModeActivation` + `DocumentClassification` path.
    - Options Considered:
      - Add `Ctrl+O` as a default Rust shortcut: rejected because the user explicitly wants to keep the `bindKey` command.
      - Require Markdown loader to bind `Ctrl+O`: rejected because file-open is app/editor configuration, not Markdown mode behavior.
      - Keep file-open binding separate from Markdown load and route selected-file activation through generic mode activation: selected.
    - Chosen Approach:
      - Simplify Markdown loading independently, keep the file-open binding as an explicit user config line, and update selected-file activation to reuse generic `MajorModeActivation` + `DocumentClassification` through the shared package-owned defaults.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      import { loadPackage } from "clay:packages";

      await loadPackage("@clay/markdown");
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: Reuse shared Markdown load/default activation path for selected-file opens through generic mode activation.
      - `tests/fixtures/configuration/windows-markdown-open/init.js`: Simplify to default Markdown load plus `bindKey`.
      - `docs/development/windows.md` and `docs/development/launch-and-gui-smoke.md`: Update actual-app Windows 11 manual instructions.
      - `src/masonry_editor.rs` and client routing tests: Verify no regression to manifest-routed control-character shortcuts.
    - References:
      - `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`
      - `docs/wiki/modules/client-file-dialog.md`
      - `docs/wiki/modules/markdown-mode-activation.md`
  - Test Cases to Write:
    - Existing/updated test: Windows open fixture binds `Ctrl+O` through `bindKey` and does not hard-code the shortcut in Rust.
    - Selected-file open test: loaded Markdown package causes selected `.md` open to install Markdown behavior/decorations through shared loader defaults and generic mode activation.
    - Regression test: `Ctrl+O` control-character key event is routeable by the manifest.

- [ ] Simplify fixtures, package docs, and manual test instructions for actual app usage
  - Acceptance Criteria:
    - Functional: Development fixtures remain deterministic, but they no longer teach users to paste large manifest/SDUI/decorations scripts into `init.js`. The Markdown main editor is placed through `PaneSlotLayout.main`; no default `PanelContribution` is published.
    - Performance: Docs preserve large-file/windowed parse expectations and no-hot-path-JS invariants.
    - Code Quality: Documentation clearly separates default user setup, optional future customization (through `setPackageOption` / `serverSetLayoutOverride`), and smoke fixture internals.
    - Security: Docs state that package import/loading is constrained and does not grant install/network/shell/filesystem authority beyond documented selected-file/workspace operations.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/markdown.md`
      - `packages/markdown/docs/index.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/development/launch-and-gui-smoke.md`
      - `docs/development/windows.md`
    - Options Considered:
      - Keep fixture documentation as the main setup guide: rejected because it caused the current confusion.
      - Document only package auto-import side effects: insufficient because the preferred convention is an explicit load command and users still need the separate file-open binding.
      - Provide concise default and explicit forms: selected.
    - Chosen Approach:
      - Update user-facing docs to make the actual app setup obvious (one-line `loadPackage("@clay/markdown")` plus explicit `bindKey`), then keep fixture docs framed as deterministic development validation only.
    - API Notes and Examples:
      ```powershell
      cargo run
      ```
      ```js
      import { bindKey } from "clay:keybindings";
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/markdown");
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/markdown.md`: Default loading docs, `PaneSlotLayout.main` placement, and no-default-`PanelContribution` note.
      - `packages/markdown/docs/index.md`: Package-owned user-facing setup.
      - `docs/reference/clay-js-api/configuration.md`: Replace or supplement Phase 19 fixture-heavy guidance.
      - `docs/development/launch-and-gui-smoke.md` and `docs/development/windows.md`: Updated manual Windows 11 actual-app and fixture smoke paths.
      - `tests/manual_smoke_docs.rs` and/or docs tests: Guard simplified instructions.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Documentation guard: Markdown package docs include the one-line default load path and explicit `Ctrl+O` binding.
    - Documentation guard: User-facing docs do not include copied package manifest/setup boilerplate as the recommended actual-app path.
    - Documentation guard: no-default-`PanelContribution` behavior is documented.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The plan verifies that Markdown default loading, the Windows file-open binding, the optional preview visibility, theme-token remaps, and any package layout overrides are represented as documented configuration-through-Clay-JS-API behavior (`setPackageOption`, `serverSetLayoutOverride`), not hidden config keys.
    - Performance: Configuration evaluation remains startup/load/configuration-change work only and does not move Markdown parser/decorator work into hot input/render paths.
    - Code Quality: Any implemented configuration options have documented defaults, types, allowed values, `custom_properties`, and tests; if no options beyond default loading are implemented, docs say customization is future work.
    - Security: Configuration cannot implicitly grant filesystem, network, shell, extension loading, package enable/disable, AI mutation, workspace mutation, WASM, raw ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration options are Clay JS APIs.
      - `docs/reference/clay-js-api/configuration.md`: Current configuration docs.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Existing file-open key binding API.
    - Options Considered:
      - Add ad hoc `markdown = true` config keys: rejected by the configuration-system decision.
      - Treat `loadPackage("@clay/markdown")` as the explicit package-loading configuration behavior, with `bindKey` and `setPackageOption` / `serverSetLayoutOverride` as the separate configuration APIs: selected unless a concrete option is added.
      - Add package-owned options now: deferred unless required by implementation, because the user asked for the default no-customization version first.
    - Chosen Approach:
      - Verify docs/registry accurately represent the package import/default loader, the explicit `bindKey`, and any Markdown-relevant `setPackageOption` / `serverSetLayoutOverride`; add no behavior-changing customization options unless implementation genuinely needs them.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
      ```ts
      // Optional Markdown-relevant configuration (only if implemented).
      setPackageOption({
        packagePrefix: "markdown",
        option: "layout.preview.defaultVisibility",
        value: "hidden",
        source: "init-js"
      });
      serverSetLayoutOverride({ slot: "right", visibility: "visible" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Update configuration guidance if default Markdown package import or package options become supported.
      - `docs/reference/clay-js-api/configuration/set-package-option.md` and `docs/reference/clay-js-api/ui/server-set-layout-override.md`: Verify/create per-API docs.
      - `docs/reference/clay-js-api/api-inventory.toml` and generated registry artifacts: Update only if Clay JS API docs change.
      - `tests/clay_js_doc_registry.rs`, `tests/clay_js_api_inventory.rs`, `tests/primitives_docs.rs`: Update if docs/registry metadata changes.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `plans/027-Phase18.4-Package-Input-Actions-State-Data-and-Configuration-Integration.md`
  - Test Cases to Write:
    - Registry/docs test: any new or changed configuration/public API docs remain linked and registry-current.
    - Static guard: no undocumented Markdown-specific config keys are introduced.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Any public programmatic surface introduced or changed by this plan (e.g., `loadPackage` or its fallback, Markdown package public exports, updated `clay:ui` usage patterns such as `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterThemeToken`) is documented through the Clay/package JS API contract, and all changed server-side Rust public functions are either private/`pub(crate)` or mapped to documented facades.
    - Performance: API docs and metadata preserve no-hot-path-JS and bounded payload expectations for Markdown mode loading, parse, decorations, contribution publication, and selected-file activation.
    - Code Quality: Public callable names distinguish module specifiers, callable exports, stable IDs, and user-facing names; package-owned exports use the `markdown` prefix.
    - Security: API docs include authority notes, permissions, failure modes, and explicit non-authorities.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
      - `docs/reference/clay-js-api/api-inventory.toml` and existing package/ui docs.
    - Options Considered:
      - Treat package exports as undocumented internals: rejected because this is now the end-user configuration surface.
      - Add a Clay-owned `clay.markdown.*` core API: rejected unless implementation proves it is only a documented first-party package alias and does not move Markdown logic into core.
      - Document the generic package load command plus any package-owned customization hook in package docs; update core Clay API docs if the load command is new or changed: selected.
    - Chosen Approach:
      - Document package-level public behavior in `packages/markdown/docs/index.md` and `docs/reference/packages/markdown.md`; update Clay JS API registry only for changed core facades or module-loader APIs.
    - API Notes and Examples:
      ```js
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `packages/markdown/docs/index.md`: Package API docs for the default `loadPackage("@clay/markdown")` path and customization hook if implemented.
      - `docs/reference/packages/markdown.md`: Reference package setup and security notes.
      - `docs/reference/clay-js-api/**`, `docs/index.md`, generated registry artifacts: Update if core Clay APIs change.
      - `tests/rust_visibility_api_mapping.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`: Update/verify as needed.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`
  - Test Cases to Write:
    - Registry/docs test: changed core Clay APIs have Markdown docs, index links, registry entries, and lookup metadata.
    - Package docs test/static guard: public package loader docs include examples, defaults, errors, permissions, and security notes.
    - Rust visibility test: no new server-side Rust public function lacks an intended Clay JS API mapping or explicit internal visibility.

- [ ] Run automated and manual verification for the end-user Markdown path
  - Acceptance Criteria:
    - Functional: Automated tests cover simplified config loading, no default `PanelContribution`, selected-file Markdown activation through generic mode activation, `Ctrl+O` manifest routing, package UI contribution validation, and configuration docs/registry coverage; manual Windows 11 `cargo run` verifies actual app behavior.
    - Performance: Relevant tests preserve no-hot-path parser/JS/IPC invariants, bounded decoration/parse/contribution payload checks, no full-document IPC for ordinary edits, and no package JavaScript in Masonry paint/layout/input handlers.
    - Code Quality: Focused tests are deterministic and avoid relying on interactive OS dialogs except for the documented manual Windows smoke.
    - Security: Tests continue to reject arbitrary imports, raw-op package code, unsafe config/module paths, stale document versions, selected-file authority expansion, unregistered actions, and hidden configuration keys.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`: Manual smoke workflow.
      - `docs/development/windows.md`: Windows-specific launch and smoke notes.
      - `docs/wiki/modules/first-party-markdown-package.md`: Existing focused test list.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`, `protocol-and-performance.md`.
    - Options Considered:
      - Rely only on manual Windows testing: rejected because simplified config/module loading and no-panel behavior should be deterministic.
      - Run full `cargo test` only: useful at final verification but too slow/noisy for each task.
      - Use focused tests during implementation plus final broad checks and manual Windows smoke: selected.
    - Chosen Approach:
      - Add focused regression tests per task, run targeted Cargo tests during execution (using `CARGO_TARGET_DIR=target/pi-verify` to avoid Windows target-directory locks), then perform final `cargo fmt --check`, relevant integration tests, and manual Windows `cargo run` validation.
    - API Notes and Examples:
      ```text
      cargo fmt --check
      CARGO_TARGET_DIR=target/pi-verify cargo test --test markdown_mode --quiet
      CARGO_TARGET_DIR=target/pi-verify cargo test --test package_loading --quiet
      CARGO_TARGET_DIR=target/pi-verify cargo test --test primitives_docs --quiet
      CARGO_TARGET_DIR=target/pi-verify cargo test --test clay_js_api_inventory --quiet
      CARGO_TARGET_DIR=target/pi-verify cargo test --test clay_js_doc_registry --quiet
      CARGO_TARGET_DIR=target/pi-verify cargo test --test performance_budgets --quiet
      ```
      ```powershell
      cargo run
      # Click editor, press Ctrl+O, select a UTF-8 .md file, verify editor-only Markdown behavior/decorations in the main slot.
      ```
    - Files to Create/Edit:
      - `tests/markdown_mode.rs`: Simplified loader/no-panel/package-boundary tests.
      - `src/server/js_runtime.rs` tests: Runtime module-loader and config evaluation tests.
      - `src/server/connection.rs` tests: Selected-file activation/no-panel tests.
      - `tests/manual_smoke_docs.rs`: Documentation smoke guards.
      - `docs/development/launch-and-gui-smoke.md`: Manual verification checklist.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `markdown_default_load_activates_mode_without_fixture_plumbing`
    - `markdown_default_load_does_not_publish_side_panel`
    - `markdown_optional_preview_is_valid_panel_contribution`
    - `windows_markdown_open_fixture_is_minimal_package_load_plus_bind_key`
    - `selected_markdown_file_uses_shared_default_loader_without_side_panel`
    - `arbitrary_external_package_imports_remain_denied`
    - Manual Windows 11: `cargo run`, configured real `~/.config/clay/init.js`, `Ctrl+O`, select Markdown file, verify editor-only view, decorations, responsive editing, and no save expectation.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, reflecting the generic-primitive consumption, the `loadPackage` status, the no-default-`PanelContribution` behavior, and the optional preview as a `PanelContribution` targeting the `right` slot.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details (contribution validation timing, no package JavaScript in Masonry hot paths, bounded contribution/theme payloads, client-first editor preservation).
    - Code Quality: Wiki pages explain what changed code does, how Markdown consumes generic primitives, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, raw-op/native-widget/CSS/client-JS prohibitions, theme token constraints, observability privacy, and external authority exclusions without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
      - `.agents/skills/create-plan/references/wiki-task.md`: Final wiki task template.
      - `docs/wiki/index.md`, `docs/wiki/modules/first-party-markdown-package.md`, `docs/wiki/modules/markdown-mode-activation.md`, `docs/wiki/modules/package-loading.md`, `docs/wiki/modules/configuration-runtime.md`, `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/slot-aware-package-ui.md`, `docs/wiki/modules/server-driven-ui.md`.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/first-party-markdown-package.md
      docs/wiki/modules/markdown-mode-activation.md
      docs/wiki/modules/package-loading.md
      docs/wiki/modules/configuration-runtime.md
      docs/wiki/modules/primitive-architecture.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/modules/first-party-markdown-package.md`: Update default loader, no-default-`PanelContribution`, optional preview, and generic primitive consumption.
      - `docs/wiki/modules/markdown-mode-activation.md`: Update shared loader and behavior-manifest activation notes if changed.
      - `docs/wiki/modules/package-loading.md`: Update default loading/customization status.
      - `docs/wiki/modules/configuration-runtime.md`: Update public configuration/override boundary for Markdown.
      - `docs/wiki/modules/client-file-dialog.md`: Update selected-file Markdown activation/no-panel notes if changed.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- This plan was rewritten by Phase 18.5 (`plans/028`) to consume generic shell/package UI primitives. The original fixture-centric task language (inline `markdownPackage` manifest object, manual per-facade registration imports, `publishTree(...)` side panel, hard-coded `SIDEBAR_WIDTH`, root-level `EditorWidget` ownership) is replaced by generic `clay:ui` contribution and `PaneSlotLayout` language. See the Phase 18.5 header note at the top.
- The one-line `loadPackage("@clay/markdown")` target is preserved as the preferred end-user convention. If a safe generic resolver cannot be implemented within the phase, a documented package-owned fallback that consumes generic primitives internally is acceptable and must be recorded in a decision log.

## Further Actions
- Close the `loadPackage` generic resolver gap (or document the fallback) as part of Phase 18.5 task 4.
- Remove default `PanelContribution` publication from the Markdown package and fixtures as part of Phase 18.5 task 5.
- Close the Phase 18.4 configuration API audit gap as part of Phase 18.5 task 6.
- Update the package authoring guide and wiki after implementation verification passes.
