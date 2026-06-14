# Phase 20 Markdown Mode End-User Loading and UI Cleanup

## Objectives
- Make the first-party Markdown mode usable through an end-user configuration surface instead of a smoke-fixture-only script.
- Remove the default Markdown left-side SDUI/status panel that was useful for fixture validation but is irrelevant in ordinary editing.
- Reduce Markdown setup in `~/.config/clay/init.js` to a one-line default load path, while preserving a package-owned customization path for later options.
- Keep `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` as the user-configured Windows file-open binding.
- Preserve Clay's primitive-first package architecture: Rust may expose generic package/module-loading primitives, but Markdown-specific registration, parser, decoration, and editor-rule data remain in `@clay/markdown`.

## Expected Outcome
- A normal Windows 11 user can run `cargo run` with a small `~/.config/clay/init.js` that loads Markdown defaults and binds the open-file command.
- Default Markdown loading requires no copied smoke fixture, no inline package manifest object, no explicit facade plumbing imports for commands/decorations/modes/packages/parse/SDUI, and no manual decoration publication from user config.
- The default Markdown editing UI shows the editor only; no Markdown preview/status side panel is published unless a future explicit customization API asks for one.
- Opening a Markdown file through the configured `Ctrl+O` path still installs Markdown behavior/decorations for the selected document when the Markdown package is loaded.
- Existing package/security/performance boundaries remain intact: no client-side package JavaScript, no parser work on keypress/paint, no broad filesystem authority, no shell/network/AI/WASM/raw-op authority, and no Markdown-specific Rust parser or editor branches.

## Tasks

- [ ] Confirm the end-user Markdown UX contract and remove fixture behavior from the product baseline
  - Acceptance Criteria:
    - Functional: The plan starts from a written baseline distinguishing smoke-only behavior from the desired end-user behavior: one-line Markdown loading, explicit `Ctrl+O` binding, no default side panel, and edit-only selected-file open until selected-file save is implemented later.
    - Performance: The baseline states that Markdown loading and selected-file activation may run at configuration/open time, while ordinary typing, paint, scroll, layout, and text-event handling stay local/non-blocking.
    - Code Quality: The baseline identifies duplicated fixture code and ad hoc user-config responsibilities that must move behind package-owned APIs or generic loader primitives.
    - Security: The baseline records that simplifying `init.js` must not broaden package, filesystem, workspace, shell, network, AI, WASM, raw-op, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`: Original Markdown package POC scope and fixture-driven setup.
      - `plans/021-Phase18.5-Large-File-Markdown-Performance-and-Memory.md`: Large-file Markdown parser/decor cache constraints.
      - `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`: Current Windows open-dialog smoke path and selected-file save exclusion.
      - `docs/reference/clay-js-api/configuration.md`: `init.js` configuration model and Phase 19 review noting the fixture binding.
      - `docs/wiki/modules/first-party-markdown-package.md`: Current package behavior, fixture usage, and selected-file activation path.
      - `.agents/skills/project-patterns/references/planning-checklist.md`, `.agents/skills/project-patterns/references/configuration-system.md`, `.agents/skills/project-patterns/references/authority-boundaries.md`.
    - Options Considered:
      - Keep the current large fixture as the documented setup: rejected because it is not usable or maintainable for end users.
      - Hard-code Markdown mode activation in Rust startup: rejected because it violates primitive-first package ownership and makes future modes worse.
      - Define a concise package-owned default loader and keep only the explicit file-open key binding in user config: selected.
    - Chosen Approach:
      - First document the target product behavior and test contract, then make implementation tasks move fixture responsibilities into reusable package/module-loading surfaces.
    - API Notes and Examples:
      ```js
      // Target default user setup, exact load API spelling to be finalized by the loader task.
      import { bindKey } from "clay:keybindings";
      import { loadPackage } from "clay:packages";

      await loadPackage("@clay/markdown");
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`: Update manual Markdown/open-file instructions to separate fixture validation from actual app setup.
      - `docs/reference/packages/markdown.md`: Record the intended default UX and no-default-panel behavior.
      - `plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md`: Keep plan status and verification notes current during execution.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
  - Test Cases to Write:
    - Documentation guard: actual-app Markdown instructions show a minimal package load plus explicit `bindKey`, not the smoke fixture manifest block.
    - Documentation guard: selected-file save remains out of scope and default Markdown mode does not require a side panel.

- [ ] Review existing editor primitives and plan generic primitive gaps before package work
  - Acceptance Criteria:
    - Functional: Existing primitives are inventoried before implementation: configuration module loading, Clay facade imports, package loading/validation, mode pattern registration, major-mode activation, command registration, behavior manifests, parse handler registration, decoration publication, selected-file open activation, and SDUI publication.
    - Performance: The review classifies work as configuration-time, package/module-resolution time, explicit selected-file-open time, background parse/decor time, or hot-path editor work.
    - Code Quality: Any new Rust work is generic, such as first-party package module resolution or package load helpers; no Markdown-specific Rust editor/parser/render branches are planned.
    - Security: The review documents allowed imports, package provenance, permission validation, module-loader deny-by-default behavior, and why simplified user config does not grant extra authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md` and `docs/reference/primitives/registry.md`: Primitive taxonomy and package-owned configuration/module boundaries.
      - `docs/wiki/modules/primitive-architecture.md`: Internal primitive architecture and hot-path constraints.
      - `docs/wiki/modules/package-loading.md`: Package validation, conflict, and load-time/runtime boundary.
      - `docs/wiki/modules/markdown-mode-activation.md`: Generic behavior-manifest mode activation and Markdown-owned editor rules.
      - `docs/wiki/modules/parse-coordinator.md` and `docs/wiki/modules/decoration-transport.md`: Background parser and inert decoration boundaries.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `.agents/skills/project-patterns/references/package-distribution.md`, `.agents/skills/project-patterns/references/behavior-manifests.md`.
    - Options Considered:
      - Add a `clay:markdown` Rust facade that performs all Markdown setup: rejected unless proven to be a thin generic module alias, because package logic should remain in the package.
      - Let config import arbitrary npm packages directly: rejected for this phase because it expands package-manager/module-loader authority too broadly.
      - Add a constrained first-party package specifier resolver for `@clay/markdown` entries: selected if the review confirms it can remain deny-by-default and package-owned.
    - Chosen Approach:
      - Produce or update a primitive review/wiki note before implementation, then implement only generic module-loader/package-loader gaps needed to make first-party packages importable from user config.
    - API Notes and Examples:
      ```text
      init.js -> package module resolver -> @clay/markdown package JS -> documented Clay facades -> behavior/decorations for the active document.
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase20-markdown-end-user-loading-primitive-review.md`: New primitive review artifact.
      - `docs/wiki/index.md`: Link the review.
      - `tests/primitives_docs.rs`: Add deterministic coverage for the review, index link, hot-path classification, and generic-only Rust guidance.
    - References:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
  - Test Cases to Write:
    - `phase20_markdown_loading_primitive_review_records_existing_inventory`: Review lists current primitives and gaps.
    - `phase20_markdown_loading_primitive_review_rejects_mode_specific_rust_branches`: Review records generic module/package gaps only.

- [ ] Add a package-owned one-line Markdown loader for user configuration
  - Acceptance Criteria:
    - Functional: User config can load default Markdown mode without inlining the package manifest, editor rules, keymaps, parse handler registration, command registration, SDUI helpers, or representative decoration spans.
    - Performance: Default loading runs at configuration/document-open time only; it does not run package JavaScript from keypress, paint, layout, scroll, or text-event handlers.
    - Code Quality: The default loader lives in `packages/markdown` and imports required Clay facades internally; public package exports use the registered `markdown` prefix for named APIs.
    - Security: The loader uses documented Clay facades and existing server-side validators; it does not expose raw Deno ops or grant filesystem, network, shell, AI, WASM, client-side JavaScript, package enable/disable, or workspace expansion authority.
  - Approach:
    - Documentation Reviewed:
      - `packages/markdown/dist/load.js` and `packages/markdown/dist/index.js`: Existing package-owned manifest/rules/commands/policy helpers.
      - `tests/fixtures/configuration/markdown-mode/init.js` and `tests/fixtures/configuration/windows-markdown-open/init.js`: Current duplicated setup to collapse.
      - `docs/reference/packages/markdown.md`: Public package contract and current smoke fixture description.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Package APIs should use package prefix/provenance.
    - Options Considered:
      - `loadPackage("@clay/markdown")`: preferred end-user convention because package loading is explicit in `init.js` and defaults remain concise.
      - `import { markdownLoadMode } from "@clay/markdown"; await markdownLoadMode();`: useful as a package-owned customization hook, but not the preferred ordinary setup if a generic loader can call it.
      - `import "@clay/markdown/auto";`: possible fallback, but less clear than an explicit load command and should be used only if the generic loader cannot be implemented cleanly in this phase.
    - Chosen Approach:
      - Implement or route through a generic explicit load API so users can write one default load line such as `await loadPackage("@clay/markdown")`. Keep `markdownLoadMode(options = {})` or equivalent as the package-owned implementation/customization hook behind that default. Any fallback requiring more code must be documented as a temporary Clay primitive/API limitation.
    - API Notes and Examples:
      ```js
      // Preferred one-line default.
      await loadPackage("@clay/markdown");

      // Package-owned form for future customization, if exposed.
      import { markdownLoadMode } from "@clay/markdown";
      await markdownLoadMode({});
      ```
    - Files to Create/Edit:
      - `packages/markdown/src/index.js` and `packages/markdown/dist/index.js`: Export explicit package-prefixed loader or re-export from load entry.
      - `packages/markdown/src/load.js` and `packages/markdown/dist/load.js`: Make loader import Clay facades internally and keep existing lower-level helpers reusable for tests.
      - `packages/markdown/src/auto.js` and `packages/markdown/dist/auto.js`: Add only if selected as a temporary fallback rather than the preferred generic load command.
      - `packages/markdown/package.json`: Add exports and Clay metadata needed by the generic load command/customization hook.
      - `src/server/js_runtime.rs`: Add constrained module-loader support for first-party `@clay/markdown` package specifiers if the primitive review selects this route.
      - `tests/markdown_mode.rs`, `src/server/js_runtime.rs` tests, and/or `tests/package_loading.rs`: Cover package-owned loader behavior.
    - References:
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
      - `docs/wiki/modules/first-party-markdown-package.md`
  - Test Cases to Write:
    - Runtime config test: `loadPackage("@clay/markdown")` loads the package, registers Markdown mode, commands, parse handler, and behavior manifest for document `1` without user-supplied facade plumbing.
    - Runtime config test: package-owned customization hook, if exposed, produces the same default package/mode/parse registration when called with no options.
    - Static guard: Markdown default loader source imports documented `clay:*` facades, not raw `Deno.core.ops`.
    - Static guard: package/module resolver allows only the intended first-party package specifiers and continues rejecting arbitrary external imports.

- [ ] Remove the default Markdown side panel from normal and smoke paths
  - Acceptance Criteria:
    - Functional: Loading Markdown mode by default does not publish the `Markdown Preview`, `Windows Markdown Open Dialog Smoke`, or similar left-side SDUI panel.
    - Performance: Removing the panel avoids unnecessary SDUI snapshot work during ordinary Markdown loading and selected-file activation.
    - Code Quality: Tests that previously depended on panel labels are rewritten to assert meaningful mode/decor/behavior state or moved to package SDUI helper tests when the helper remains optional.
    - Security: Removing default SDUI publication does not add new UI action authority and does not leave stale package command buttons in normal editing.
  - Approach:
    - Documentation Reviewed:
      - `tests/fixtures/configuration/markdown-mode/init.js` and `tests/fixtures/configuration/windows-markdown-open/init.js`: Current fixture-published side panels.
      - `packages/markdown/dist/sdui.js`: Optional package-owned SDUI status helper.
      - `src/server/connection.rs::markdown_open_init_source`: Selected-file activation currently publishes Markdown preview/status SDUI.
      - `docs/wiki/modules/server-driven-ui.md`: SDUI is inert server-published UI state, not required for decorations.
    - Options Considered:
      - Delete all Markdown SDUI helper code: possible, but may remove useful optional preview/status test coverage.
      - Keep helper code but stop publishing it by default: selected because it removes user-facing clutter without forcing a broad package cleanup.
      - Hide the panel client-side only: rejected because the server should not publish irrelevant default UI state in the first place.
    - Chosen Approach:
      - Remove default fixture/runtime publication of the panel. Keep package SDUI helpers only as optional/internal capability if tests/docs make clear they are not part of default Markdown mode.
    - API Notes and Examples:
      ```text
      Default Markdown load: behavior manifest + parse handler + decoration publication; no publishTree(...) side panel.
      ```
    - Files to Create/Edit:
      - `tests/fixtures/configuration/markdown-mode/init.js`: Replace manual panel publication with default loader and editor-only state.
      - `tests/fixtures/configuration/windows-markdown-open/init.js`: Keep only Markdown default load plus `bindKey`.
      - `src/server/connection.rs`: Stop publishing Markdown preview/status SDUI during selected-file open unless an explicit future option exists.
      - `packages/markdown/src/sdui.js` and `packages/markdown/dist/sdui.js`: Keep, narrow, or document as optional according to implementation results.
      - `tests/markdown_mode.rs`, `src/server/js_runtime.rs` tests, `tests/manual_smoke_docs.rs`: Update assertions away from default panel labels.
    - References:
      - `docs/wiki/modules/first-party-markdown-package.md`
      - `docs/wiki/modules/server-driven-ui.md`
  - Test Cases to Write:
    - Runtime smoke fixture test: Markdown fixture loads mode/decorations without publishing a left-side Markdown preview/status panel.
    - Runtime selected-file test: opening a Markdown file publishes behavior/decorations for the selected document without publishing the side panel by default.
    - Static guard: fixture sources do not contain `Windows Markdown Open Dialog Smoke`, `Markdown Preview`, or default `publishTree` panel code.

- [ ] Keep the Windows open-file binding explicit and verify selected-file Markdown activation uses the shared loader
  - Acceptance Criteria:
    - Functional: `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` remains the documented way to open the Windows file explorer, and selected Markdown files still activate Markdown behavior/decorations when the package has been loaded.
    - Performance: The `Ctrl+O` route remains a client-local manifest lookup followed only by explicit modal native UI and server selected-file open work.
    - Code Quality: Selected-file Markdown activation reuses the package-owned loader/helper path rather than generating a second divergent Markdown init script.
    - Security: A selected path remains a single-file server grant after explicit user selection; loading Markdown does not expand workspace roots or parent-directory authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/documents/client-open-file-dialog.md`: Client UI command ID contract.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: User keybinding API.
      - `docs/wiki/modules/client-file-dialog.md`: Windows file dialog backend and selected-path authority boundary.
      - `docs/wiki/flows/client-behavior-routing.md`: Client UI command routing through behavior manifests.
    - Options Considered:
      - Add `Ctrl+O` as a default Rust shortcut: rejected because the user explicitly wants to keep the bindKey command.
      - Require Markdown loader to bind `Ctrl+O`: rejected because file-open is app/editor configuration, not Markdown mode behavior.
      - Keep file-open binding separate from Markdown load: selected.
    - Chosen Approach:
      - Simplify Markdown loading independently, keep the file-open binding as an explicit user config line, and update selected-file activation to reuse the same package-owned defaults.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      import { loadPackage } from "clay:packages";

      await loadPackage("@clay/markdown");
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: Reuse shared Markdown load/default activation path for selected-file opens.
      - `tests/fixtures/configuration/windows-markdown-open/init.js`: Simplify to default Markdown load plus `bindKey`.
      - `docs/development/windows.md` and `docs/development/launch-and-gui-smoke.md`: Update actual-app Windows 11 manual instructions.
      - `src/masonry_editor.rs` and client routing tests: Verify no regression to manifest-routed control-character shortcuts.
    - References:
      - `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`
      - `docs/wiki/modules/client-file-dialog.md`
  - Test Cases to Write:
    - Existing/updated test: Windows open fixture binds `Ctrl+O` through `bindKey` and does not hard-code the shortcut in Rust.
    - Selected-file open test: loaded Markdown package causes selected `.md` open to install Markdown behavior/decorations through shared loader defaults.
    - Regression test: `Ctrl+O` control-character key event is routeable by the manifest.

- [ ] Simplify fixtures, package docs, and manual test instructions for actual app usage
  - Acceptance Criteria:
    - Functional: Development fixtures remain deterministic, but they no longer teach users to paste large manifest/SDUI/decorations scripts into `init.js`.
    - Performance: Docs preserve large-file/windowed parse expectations and no-hot-path-JS invariants.
    - Code Quality: Documentation clearly separates default user setup, optional future customization, and smoke fixture internals.
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
      - Update user-facing docs to make the actual app setup obvious, then keep fixture docs framed as deterministic development validation only.
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
      - `docs/reference/packages/markdown.md`: Default loading docs and no-default-panel note.
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
    - Documentation guard: no-default-side-panel behavior is documented.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The plan verifies that Markdown default loading and the Windows file-open binding are represented as documented configuration-through-Clay-JS-API behavior, not hidden config keys.
    - Performance: Configuration evaluation remains startup/load-time work and does not move Markdown parser/decorator work into hot input/render paths.
    - Code Quality: Any implemented configuration options have documented defaults, types, allowed values, and tests; if no options beyond default loading are implemented, docs say customization is future work.
    - Security: Configuration cannot implicitly grant filesystem, network, shell, extension loading, package enable/disable, AI mutation, WASM, raw ops, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration options are Clay JS APIs.
      - `docs/reference/clay-js-api/configuration.md`: Current configuration docs.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Existing file-open key binding API.
    - Options Considered:
      - Add ad hoc `markdown = true` config keys: rejected by the configuration-system decision.
      - Treat `loadPackage("@clay/markdown")` as the explicit package-loading configuration behavior, with `bindKey` remaining the separate file-open configuration API: selected unless a concrete option is added.
      - Add package-owned options now: deferred unless required by implementation, because the user asked for the default no-customization version first.
    - Chosen Approach:
      - Verify docs/registry accurately represent the package import/default loader and existing `bindKey` behavior; add no behavior-changing customization options unless implementation genuinely needs them.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Update configuration guidance if default Markdown package import becomes supported.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Verify no change needed for `Ctrl+O`.
      - `docs/index.md` and generated registry artifacts: Update only if Clay JS API docs change.
      - `tests/clay_js_doc_registry.rs`, `tests/clay_js_api_inventory.rs`: Update if docs/registry metadata changes.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - Registry/docs test: any new or changed configuration/public API docs remain linked and registry-current.
    - Static guard: no undocumented Markdown-specific config keys are introduced.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Any public programmatic surface introduced or changed by this plan is documented through the Clay/package JS API contract, and all changed server-side Rust public functions are either private/`pub(crate)` or mapped to documented facades.
    - Performance: API docs and metadata preserve no-hot-path-JS and bounded payload expectations for Markdown mode loading, parse, decorations, and selected-file activation.
    - Code Quality: Public callable names distinguish module specifiers, callable exports, stable IDs, and user-facing names; package-owned exports use the `markdown` prefix.
    - Security: API docs include authority notes, permissions, failure modes, and explicit non-authorities.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `.agents/skills/project-patterns/references/clay-js-api-naming.md`, `.agents/skills/project-patterns/references/clay-js-api-schema.md`.
      - `.agents/skills/project-patterns/references/documentation-as-code.md` and `.agents/skills/project-patterns/references/doc-registry-tests.md`.
      - `docs/reference/clay-js-api/api-inventory.toml` and existing package docs.
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
    - Functional: Automated tests cover simplified config loading, no default side panel, selected-file Markdown activation, and `Ctrl+O` manifest routing; manual Windows 11 `cargo run` verifies actual app behavior.
    - Performance: Relevant tests preserve no-hot-path parser/JS/IPC invariants and bounded decoration/parse payload checks.
    - Code Quality: Focused tests are deterministic and avoid relying on interactive OS dialogs except for the documented manual Windows smoke.
    - Security: Tests continue to reject arbitrary imports, raw-op package code, unsafe config/module paths, stale document versions, and selected-file authority expansion.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`: Manual smoke workflow.
      - `docs/development/windows.md`: Windows-specific launch and smoke notes.
      - `docs/wiki/modules/first-party-markdown-package.md`: Existing focused test list.
    - Options Considered:
      - Rely only on manual Windows testing: rejected because simplified config/module loading and no-panel behavior should be deterministic.
      - Run full `cargo test` only: useful at final verification but too slow/noisy for each task.
      - Use focused tests during implementation plus final broad checks and manual Windows smoke: selected.
    - Chosen Approach:
      - Add focused regression tests per task, run targeted Cargo tests during execution, then perform final `cargo fmt --check`, relevant integration tests, and manual Windows `cargo run` validation.
    - API Notes and Examples:
      ```powershell
      cargo run
      # Click editor, press Ctrl+O, select a UTF-8 .md file, verify editor-only Markdown behavior/decorations.
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
    - `markdown_auto_import_loads_default_mode_without_facade_plumbing`
    - `markdown_default_load_does_not_publish_side_panel`
    - `windows_markdown_open_fixture_is_minimal_package_load_plus_bind_key`
    - `selected_markdown_file_uses_shared_default_loader_without_side_panel`
    - `arbitrary_external_package_imports_remain_denied`
    - Manual Windows 11: `cargo run`, configured real `~/.config/clay/init.js`, `Ctrl+O`, select Markdown file, verify editor-only view, decorations, responsive editing, and no save expectation.

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
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/modules/first-party-markdown-package.md`: Update default loader/no-panel behavior.
      - `docs/wiki/modules/markdown-mode-activation.md`: Update shared loader and behavior-manifest activation notes if changed.
      - `docs/wiki/modules/embedded-js-runtime.md` or `docs/wiki/modules/configuration-runtime.md`: Update first-party package module-resolution behavior if changed.
      - `docs/wiki/modules/client-file-dialog.md`: Update selected-file Markdown activation/no-panel notes if changed.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
