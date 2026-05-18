# Phase 8 Configuration Foundation

## Objectives
- Establish Clay's documented user configuration foundation on top of the existing IPC, synchronization, behavior-manifest, and Clay JS API inventory work.
- Close Phase 8 prerequisites that are not fully in place yet: Phase 7 configuration API verification and the Phase 3 generated registry/lookup coverage needed for configuration discoverability.
- Define `~/.config/clay/init.js` and modular local configuration loading as public Clay JS API contracts without enabling arbitrary client-side JavaScript or broad runtime permissions.
- Make key binding management and editor customization discoverable as configuration APIs with default key binding metadata, empty key binding lists where applicable, custom properties, permissions/security notes, generated registry entries, lookup access, and deterministic tests.

## Expected Outcome
- Phase 8 starts by reconciling the remaining Phase 7 configuration prerequisites: planned key binding, cursor customization, and configuration facade APIs are documented, indexed, inventoried, and tested consistently.
- A non-mutating generated documentation registry/check path exists for Clay JS API Markdown linked from `docs/index.md`, with a developer update command for checked-in artifacts and tests that fail with actionable output when artifacts are stale.
- App/help/agent lookup can query generated Clay JS API registry data for configuration entry point semantics, configuration APIs, default key bindings including empty defaults, and custom properties.
- `~/.config/clay/init.js` is documented as the user-facing entry point, and modular local loading semantics are represented as Clay JS APIs while actual server-side JavaScript execution remains deferred to Phase 11 unless a minimal non-executing parser/metadata path is needed for tests.
- Configuration remains no-authority-by-default: no filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript execution authority is granted implicitly.
- `cargo fmt --check`, `cargo test`, and `cargo check` pass.

## Tasks

- [x] Reconcile Phase 8 prerequisites from earlier plans
  - Acceptance Criteria:
    - Functional: Review checked implementations in `plans/001` through `plans/008`, identify only prerequisite gaps that block Phase 8, and bring those gaps into this plan rather than modifying intentionally unchecked earlier-plan tasks.
    - Performance: The review confirms that ordinary typing/rendering remains client-local or client-first and does not wait on configuration loading, registry generation, JavaScript, IPC, AI, file IO, or full-document serialization.
    - Code Quality: The prerequisite list is explicit, scoped, and avoids duplicating work already implemented in Phase 4-7 tests, docs, facades, behavior manifests, leases, and API inventory.
    - Security: The review confirms no prior phase introduced implicit configuration authority or arbitrary JavaScript execution in the Rust client.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 8 and prerequisites from Phases 3, 6, and 7.
      - `plans/001-Phase0-NativeTextCanvas.md` through `plans/008-Phase7-Clay-JS-API-Structure-and-Current-Functionality-Inventory.md`: Checked tasks and intentionally unchecked leftovers.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Decision alignment, authority boundary, client hot path, documentation-as-code, configuration, security, and performance checks.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration entry point and configuration-as-API rule.
    - Options Considered:
      - Reopen older plan tasks: preserves original phase labels, but conflicts with the user's instruction that unchecked items there are intentionally incomplete.
      - Ignore older unchecked tasks: simpler, but Phase 8 needs some deferred Phase 3/7 registry and configuration prerequisites.
      - Pull only blocking prerequisites into Phase 8: keeps plan history intact and makes Phase 8 executable.
    - Chosen Approach:
      - Treat earlier checked tasks as the implemented baseline. Carry forward the Phase 7 configuration verification gap and Phase 3 generated registry/lookup gap as explicit Phase 8 prerequisite tasks.
    - API Notes and Examples:
      ```text
      Phase 8 prerequisites to include here:
      - verify configuration APIs from the Phase 7 inventory
      - generate/check registry artifacts from Markdown/index
      - expose lookup over generated registry data
      ```
    - Files to Create/Edit:
      - `plans/009-Phase8-Configuration-Foundation.md`: Record prerequisite scope and phase boundary.
      - No source edits expected for the review itself unless gaps are found during execution.
    - References:
      - `plans/008-Phase7-Clay-JS-API-Structure-and-Current-Functionality-Inventory.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
  - Test Cases to Write:
    - Manual prerequisite review: Confirm Phase 4 IPC, Phase 5 synchronization/leases, Phase 6 behavior manifests, and Phase 7 API inventory/facades have passing checked-task tests, and list any blocking Phase 8 gaps before implementation proceeds.
  - Completion Notes:
    - Reviewed task status from `plans/001` through `plans/008`: Phase 0/1 checked tasks are complete; Phase 2 completed editor interaction while its early Clay JS facade tasks remained intentionally deferred; Phase 3 completed schema/index/naming and left per-API docs, generated registry, lookup, configuration, public API coverage, and wiki tasks deferred; Phase 4, Phase 5, and Phase 6 completed IPC, synchronization/leases, and behavior manifests while leaving recurring Clay JS/configuration verification tasks deferred; Phase 7 completed facade layout, inventory, Rust visibility audit, planned API docs/indexing, readiness validation, and public API verification while leaving configuration verification, final verification, and wiki tasks unchecked.
    - Confirmed current Phase 8 blocking gaps are already represented in this plan: generated registry artifact/update command, read-only registry lookup, `clay:configuration` API docs/inventory/index entries for `loadConfigurationModule` and `getConfigurationState`, keybinding configuration metadata/lookup, cursor customization metadata/lookup, no-authority validation, final configuration API audit, public-surface audit, phase verification, and wiki update.
    - Confirmed non-blocking earlier leftovers should remain in earlier plans and not be reopened here except where Phase 8 explicitly carries them forward: broad Phase 3 workflow/docs/wiki cleanup, older final verification tasks, and recurring Clay JS/configuration checks from Phases 4-7.
    - Confirmed performance boundary from checked tests and roadmap: ordinary typing/rendering remains client-local or client-first and does not synchronously wait on configuration loading, registry generation, JavaScript, IPC, AI, file IO, or full-document serialization.
    - Confirmed security boundary: current code/docs/tests do not introduce implicit configuration authority or arbitrary JavaScript execution in the Rust client; configuration remains no-authority-by-default and server-side runtime execution is deferred to Phase 11.
    - Verification run: `cargo test` passed (161 lib tests, 6 main tests, 10 Clay JS API inventory tests, 2 facade layout tests, 1 Rust visibility mapping test, and doc tests).

- [x] Implement generated Clay JS API registry artifacts and stale-check command
  - Acceptance Criteria:
    - Functional: A deterministic generator reads `docs/index.md` registry source links and Clay JS API Markdown/frontmatter, writes checked-in generated registry artifacts, and a non-mutating check mode fails when generated artifacts are stale or malformed.
    - Performance: Registry generation and checking run only in developer/test commands and add no runtime work to Masonry paint/input, IPC frame handling, or ordinary edit hot paths.
    - Code Quality: The generator has focused parsing, actionable errors, stable output ordering, and tests that do not silently mutate files during `cargo test`.
    - Security: Generated entries preserve permissions/security notes and fail if configuration or public APIs omit no-authority-by-default language.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Require non-mutating checks, update command, stale artifact failures, and lookup coverage.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown plus `docs/index.md` is authoritative.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Checks should be deterministic and actionable.
      - `docs/reference/clay-js-api/schema.md`: Required frontmatter and body fields.
    - Options Considered:
      - Keep only Phase 7 readiness tests: already useful, but Phase 8 explicitly needs generated registry entries and lookup access.
      - Generate at build time: convenient, but risks hidden mutation and build-script complexity.
      - Add an explicit update binary plus non-mutating test/check path: aligns with project patterns and keeps artifacts reviewable.
    - Chosen Approach:
      - Add a small Rust registry module and `cargo run --bin update-doc-registry` command. The command rewrites checked-in generated artifacts; tests invoke the same logic in check mode and print the update command when stale.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test clay_js_doc_registry
      ```
      ```rust
      // check mode must compare expected generated bytes with checked-in bytes
      // and return an actionable error instead of writing during tests.
      ```
    - Files to Create/Edit:
      - `src/docs/mod.rs`: Registry generation/check helpers, parsing helpers, and lookup data structures.
      - `src/docs/registry.rs`: Deterministic registry builder from indexed Markdown.
      - `src/bin/update-doc-registry.rs`: Developer update command for checked-in artifacts.
      - `src/lib.rs`: Export internal docs module as needed for tests/bin use.
      - `docs/generated/clay-js-api-registry.json`: Checked-in generated registry artifact.
      - `tests/clay_js_doc_registry.rs`: Stale-artifact, schema, security, and update-command checks.
      - `Cargo.toml`: Add dependencies only if necessary; prefer std-only parsing if practical.
    - References:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - `generated_registry_is_current`: Fails if `docs/generated/clay-js-api-registry.json` differs from Markdown-derived output and prints `cargo run --bin update-doc-registry`.
    - `generated_registry_contains_all_indexed_public_apis`: Confirms every `docs/index.md` registry source link appears exactly once.
    - `generated_registry_preserves_configuration_metadata`: Confirms key bindings, custom properties, permissions, security notes, JS module/export, Rust owner, op name, facade path, and lookup tags survive generation.
  - Completion Notes:
    - Added `src/docs/registry.rs` and `src/docs/mod.rs` with std-only Clay JS API Markdown/frontmatter parsing, deterministic registry generation, duplicate-ID and schema validation, no-authority security validation, and non-mutating stale checks.
    - Added `src/bin/update-doc-registry.rs` as the explicit developer update command and generated the checked-in artifact at `docs/generated/clay-js-api-registry.json` from `docs/index.md` registry source links.
    - Added `tests/clay_js_doc_registry.rs` covering stale artifact detection with the `cargo run --bin update-doc-registry` repair command, exact index coverage, unique IDs, and preservation of configuration/key binding/custom property/security/facade/op/Rust-owner/lookup metadata.
    - Updated the implementation wiki with `docs/wiki/modules/clay-js-doc-registry.md` and linked it from `docs/wiki/index.md`.
    - Verification run: `cargo fmt --check`, `cargo test`, and `cargo check` passed.

- [x] Expose documentation registry lookup APIs for app/help/agent discovery
  - Acceptance Criteria:
    - Functional: Generated registry data can be queried by stable ID, JS module/export, user-facing name, kind/owner, lookup tag, key binding, and custom property name, including configuration APIs.
    - Performance: Lookup operates on generated/static registry data and is suitable for app/help/agent queries without reading full source files or regenerating docs during normal use.
    - Code Quality: Lookup APIs are separated from generator mutation logic, have typed results, and return clear empty/error states.
    - Security: Lookup is read-only and exposes documentation metadata only; it does not execute configuration files, call ops, or grant permissions.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 8: Users and AI agents can discover configurable behavior, key bindings, missing key bindings, and custom properties through generated documentation registry.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required lookup/discoverability metadata.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Generated app/agent registries and lookup APIs derive from indexed Markdown.
    - Options Considered:
      - Expose lookup only through tests: validates artifacts but does not create reusable app/agent API.
      - Add a runtime server API now: closer to final UI help, but risks expanding protocol/runtime scope too early.
      - Add a Rust lookup module over checked-in generated data: reusable and testable without runtime JavaScript execution.
    - Chosen Approach:
      - Implement a read-only Rust lookup layer over generated registry entries. Keep protocol/app UI exposure minimal or deferred, but ensure tests exercise the lookup methods expected by future app/help/agent surfaces.
    - API Notes and Examples:
      ```rust
      let registry = ClayJsApiRegistry::load_generated()?;
      let cursor_style = registry.by_id("clay.editor.clientSetCursorStyle")?;
      let configurable = registry.by_custom_property("color");
      ```
    - Files to Create/Edit:
      - `src/docs/registry.rs`: Add read-only lookup indexes and query helpers.
      - `tests/clay_js_doc_registry.rs`: Add lookup coverage tests.
      - `docs/reference/clay-js-api/configuration.md`: Link or describe lookup semantics if user-facing docs need clarification.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - `lookup_finds_api_by_stable_id_and_export`: Validates ID and module/export lookup.
    - `lookup_finds_configuration_by_custom_property`: Finds cursor style APIs by `color`, `blinking`, and `type` custom properties.
    - `lookup_lists_empty_default_key_bindings`: Confirms APIs with no defaults still expose empty `key_bindings` lists.
    - `lookup_is_read_only`: Confirms lookup does not attempt to load `~/.config/clay/init.js` or execute JavaScript.
  - Completion Notes:
    - Added `ClayJsApiRegistry::from_generated` and `from_generated_json` so app/help/agent discovery can load the checked-in generated JSON artifact as typed registry entries without reading source Markdown or regenerating docs in normal lookup paths.
    - Added read-only lookup helpers for stable ID, JS module/export, user-facing name, kind/owner, lookup tag, default key binding, and custom property name. Unique lookups return `Option<&RegistryEntry>` and multi-match lookups return `Vec<&RegistryEntry>` for clear empty states.
    - Added generated-registry lookup tests covering ID/export/name/kind-owner/tag lookup, cursor custom property discovery for `color`, `blinking`, and `type`, empty default key-binding lists, default key-binding lookup, and metadata-only/no-execution behavior.
    - Updated `docs/wiki/modules/clay-js-doc-registry.md` with the generated-data lookup flow, APIs, invariants, and tests.
    - Verification run: `cargo fmt --check`, `cargo test --test clay_js_doc_registry`, `cargo check`, and full `cargo test` passed.

- [x] Finalize configuration entry point and modular loading API contract
  - Acceptance Criteria:
    - Functional: `~/.config/clay/init.js` is documented as the configuration entry point, modular local loading semantics are documented as Clay JS APIs, and facade/inventory/docs agree on API names and status.
    - Performance: Configuration loading remains server-side and outside ordinary input/rendering hot paths; Phase 8 does not introduce synchronous keypress-to-JavaScript behavior.
    - Code Quality: Configuration APIs follow Clay JS naming rules, have stable IDs, examples, custom properties, key binding metadata, permissions/security notes, backing owner/op/facade metadata, and generated registry entries.
    - Security: Modular loading is constrained to local configuration semantics and does not implicitly grant arbitrary filesystem, network, shell, extension/package loading, AI mutation, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 8 and Phase 11: Phase 8 establishes the public contract; Phase 11 evaluates `init.js` in the server-side JavaScript runtime.
      - `.agents/skills/project-patterns/references/configuration-system.md`: `init.js` entry point, modular local files, configuration-as-API, and no-authority-by-default security.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Domain module and lower-camel-case exports.
      - Context7 `/denoland/deno_core`: Future server runtime should use `JsRuntime`, `extension!`, and explicit `#[op2]` wrappers; Phase 8 should not expose raw ops.
    - Options Considered:
      - Implement full `deno_core` configuration execution now: would satisfy runtime loading, but belongs to Phase 11 and expands authority too early.
      - Document only prose without API entries: safer, but fails configuration-as-API and registry lookup requirements.
      - Define planned facade/docs/inventory entries and optionally add non-executing path validation: preserves the public contract while deferring runtime execution.
    - Chosen Approach:
      - Add or update `clay:configuration` planned APIs for configuration state and modular loading, likely `loadConfigurationModule` and `getConfigurationState`, with docs and registry coverage. State clearly that actual JavaScript evaluation is deferred to Phase 11.
    - API Notes and Examples:
      ```ts
      // ~/.config/clay/init.js
      import { loadConfigurationModule } from "clay:configuration";
      import { bindKey } from "clay:keybindings";

      await loadConfigurationModule({ path: "./keys.js" });
      bindKey("Ctrl+I", "clay.editor.serverInsertText");
      ```
      ```rust
      // Future Phase 11 shape from deno_core docs; not a Phase 8 runtime requirement.
      deno_core::extension!(clay_configuration, ops = [op_clay_configuration_load_module]);
      ```
    - Files to Create/Edit:
      - `runtime/js/configuration.ts`: Verify or extend planned facade types/exports.
      - `docs/reference/clay-js-api/configuration.md`: Clarify entry point, modular loading, and phase boundary.
      - `docs/reference/clay-js-api/configuration/load-configuration-module.md`: Public API doc for modular local configuration loading if accepted.
      - `docs/reference/clay-js-api/configuration/get-configuration-state.md`: Public API doc for configuration state lookup if accepted.
      - `docs/reference/clay-js-api/api-inventory.toml`: Add configuration API entries with status `planned` and registry metadata.
      - `docs/index.md`: Link configuration API docs under registry source files.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs change.
      - `tests/clay_js_facade_layout.rs`: Include `clay:configuration` exports in facade checks.
      - `tests/clay_js_api_inventory.rs` and/or `tests/clay_js_doc_registry.rs`: Validate configuration API metadata.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - Context7 `/denoland/deno_core` `JsRuntime`, `extension!`, and `op2` documentation.
  - Test Cases to Write:
    - `configuration_entrypoint_is_documented_and_indexed`: Confirms docs mention `~/.config/clay/init.js` and are linked/generated.
    - `configuration_facade_exports_match_inventory`: Confirms configuration facade exports match public inventory/docs.
    - `configuration_module_loading_is_planned_no_authority`: Confirms docs/security notes state local modular semantics and no implicit filesystem/workspace/package authority.
  - Completion Notes:
    - Added public Phase 8 Clay JS API docs for `clay.configuration.loadConfigurationModule` and `clay.configuration.getConfigurationState`, linked them from `docs/index.md`, and regenerated `docs/generated/clay-js-api-registry.json`.
    - Updated `docs/reference/clay-js-api/configuration.md` to document `~/.config/clay/init.js` as the configuration entry point, local modular loading through `loadConfigurationModule`, configuration-state lookup through `getConfigurationState`, and the Phase 8/Phase 11 runtime boundary.
    - Added matching public inventory entries for both `clay:configuration` exports with stable IDs, facade paths, future op names, custom properties, empty key-binding metadata, hot-path policies, and no-authority security notes.
    - Added registry and inventory test coverage for documented/indexed configuration entry point APIs, facade/inventory/docs consistency, generated lookup metadata, and planned local modular loading without implicit filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
    - Updated implementation wiki pages for the documentation registry and Clay JS facade skeleton to explain the new configuration entry point contract.
    - Verification run: `cargo fmt --check`, `cargo check`, and full `cargo test` passed.

- [x] Create or verify initial key binding configuration APIs
  - Acceptance Criteria:
    - Functional: `bindKey`, `unbindKey`, and `listKeyBindings` are documented and generated as configuration APIs, including default key binding behavior, empty defaults where applicable, scopes, `when` conditions, and command/API ID validation expectations.
    - Performance: Key binding changes update future server-owned behavior manifests and never install arbitrary JavaScript into the Rust client keypress hot path.
    - Code Quality: Key binding API docs, facade stubs, inventory records, registry entries, and lookup results agree on stable IDs, names, options, custom properties, and security notes.
    - Security: Binding keys does not grant file, network, shell, extension loading, package, workspace, AI mutation, WASM, or client-side JavaScript authority; future command IDs must be documented/registered before binding.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/keybindings/bind-key.md`, `unbind-key.md`, and `list-key-bindings.md`: Existing planned API docs from Phase 7.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Key routing is through inert server-issued behavior manifests.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Key bindings are configuration APIs.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Key binding metadata and custom properties are mandatory.
    - Options Considered:
      - Implement live manifest mutation from config now: useful, but runtime config execution is deferred.
      - Keep Phase 7 docs unchanged: may miss Phase 8 registry/lookup and entry point semantics.
      - Verify and tighten docs/tests around generated registry and lookup: satisfies Phase 8 foundation without premature runtime behavior.
    - Chosen Approach:
      - Treat key binding APIs as planned server-side configuration APIs. Ensure registry entries expose empty default key binding lists, `custom_properties` for `key`, `command`, `scope`, and `when`, and behavior-manifest phase boundaries.
    - API Notes and Examples:
      ```ts
      import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";

      bindKey("Ctrl+I", "clay.editor.serverInsertText", { scope: "editor" });
      unbindKey("Escape", { scope: "global" });
      const bindings = listKeyBindings("editor");
      ```
    - Files to Create/Edit:
      - `runtime/js/keybindings.ts`: Verify planned facade API shape.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Update Phase 8 configuration and registry notes if needed.
      - `docs/reference/clay-js-api/keybindings/unbind-key.md`: Update Phase 8 configuration and registry notes if needed.
      - `docs/reference/clay-js-api/keybindings/list-key-bindings.md`: Update Phase 8 configuration and registry notes if needed.
      - `docs/reference/clay-js-api/api-inventory.toml`: Verify key binding configuration classification.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs changes.
      - `tests/clay_js_doc_registry.rs`: Add key binding lookup and metadata assertions.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - `keybinding_configuration_apis_have_empty_defaults`: Confirms management APIs expose `key_bindings: []` in generated registry.
    - `keybinding_configuration_custom_properties_are_queryable`: Confirms `key`, `command`, `scope`, and `when` are generated and lookup-visible.
    - `keybinding_docs_reject_undocumented_authority`: Confirms docs/security notes do not imply permissions beyond documented command routing.
  - Completion Notes:
    - Verified `bindKey`, `unbindKey`, and `listKeyBindings` are documented/indexed/generated as planned `clay:keybindings` configuration/query APIs with empty default key-binding metadata, behavior-manifest routing boundaries, custom property metadata, no-authority security notes, and command/API ID validation expectations.
    - Tightened the key binding API docs to describe Phase 8 configuration semantics, server-owned/inert behavior-manifest routing, `scope`/`when` handling, and documented/registered command ID requirements without introducing live manifest mutation or client-side JavaScript hooks.
    - Updated `runtime/js/keybindings.ts` so `listKeyBindings` accepts the documented `"all" | "global" | "editor"` scope filter while preserving planned no-runtime-op facade behavior.
    - Added generated-registry tests for empty key binding defaults, queryable `key`/`command`/`scope`/`when` custom properties, and static key binding authority denial/command-registration documentation.
    - Updated implementation wiki pages for the documentation registry and Clay JS facade skeleton with the verified key binding configuration contract.
    - Verification run: `cargo fmt --check`, `cargo test --test clay_js_doc_registry`, `cargo check`, and full `cargo test` passed.

- [x] Create or verify initial editor customization configuration APIs
  - Acceptance Criteria:
    - Functional: Initial editor customization configuration APIs, starting with cursor style and any documented viewport/customization settings already in the inventory, have docs, registry entries, lookup coverage, and custom properties for every behavior-changing setting.
    - Performance: Customization changes remain UI metadata or manifest-delivered behavior and do not route ordinary typing through JavaScript or block paint/input on server work.
    - Code Quality: Custom properties include types, defaults, allowed values where relevant, descriptions, examples, and consistency between Markdown, inventory, facade types, and generated registry.
    - Security: Editor customization does not grant document mutation authority or external authority unless explicitly documented and server-validated.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/editor/client-set-cursor-style.md`: Existing planned cursor style API.
      - `docs/reference/clay-js-api/editor/client-set-viewport.md`: Existing planned viewport API if retained as configurable behavior.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Behavior-changing settings must be configuration APIs with `custom_properties`.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: UI state changes must remain off the blocking hot path.
    - Options Considered:
      - Add many theme/editor settings now: broad coverage, but risks inventing APIs before implementation needs are clear.
      - Limit Phase 8 to cursor style and existing Phase 7 inventory settings: smaller and anchored to current code/docs.
      - Defer customization entirely: conflicts with Phase 8 focus areas.
    - Chosen Approach:
      - Start with `clientSetCursorStyle` as the primary editor customization API, and verify existing viewport/customization entries only where already planned. Defer broader theme APIs to later product hardening unless roadmap or docs already require them.
    - API Notes and Examples:
      ```ts
      import { clientSetCursorStyle } from "clay:editor";

      clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
      ```
    - Files to Create/Edit:
      - `runtime/js/editor.ts`: Verify `ClientSetCursorStyleOptions` reflects documented custom properties.
      - `docs/reference/clay-js-api/editor/client-set-cursor-style.md`: Tighten configuration examples, defaults, allowed values, and security notes.
      - `docs/reference/clay-js-api/editor/client-set-viewport.md`: Verify whether viewport options are configuration, command, or internal UI state.
      - `docs/reference/clay-js-api/api-inventory.toml`: Verify custom property metadata.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs changes.
      - `tests/clay_js_doc_registry.rs`: Add custom property lookup assertions.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - `cursor_style_custom_properties_are_complete`: Confirms `color`, `blinking`, and `type` include type/default/allowed-value metadata in docs and generated registry.
    - `editor_customization_has_no_external_authority`: Confirms generated security notes deny external authority and document client-local UI state.
    - `configuration_lookup_finds_cursor_customization`: Confirms lookup by custom property and tag returns `clay.editor.clientSetCursorStyle`.
  - Completion Notes:
    - Verified `runtime/js/editor.ts` already exposes `ClientSetCursorStyleOptions` with `color`, `blinking`, and `type` matching the documented cursor customization API; no facade runtime wiring was added.
    - Tightened `docs/reference/clay-js-api/editor/client-set-cursor-style.md` with Phase 8 configuration/customization semantics, custom property descriptions with types/defaults/allowed cursor shape values, no blocking paint/input behavior, and denial of document mutation plus external authority.
    - Verified `clientSetViewport` as a retained planned client-local viewport/layout API rather than user configuration from `~/.config/clay/init.js`, and tightened its no-authority/document-mutation security notes.
    - Updated matching inventory security notes and regenerated `docs/generated/clay-js-api-registry.json` from Markdown.
    - Added generated-registry tests for complete cursor custom property metadata, editor customization authority denial, and cursor customization lookup by tag and custom property.
    - Updated implementation wiki pages for the documentation registry and Clay JS facade skeleton with the editor customization configuration contract.
    - Verification run: `cargo fmt --check`, `cargo test --test clay_js_doc_registry`, `cargo test --test clay_js_api_inventory`, `cargo test --test clay_js_facade_layout`, and `cargo check` passed.

- [x] Add no-authority-by-default configuration security validation
  - Acceptance Criteria:
    - Functional: Tests fail when configuration APIs imply or omit authority boundaries for filesystem, network, shell, extension loading, AI mutation, workspace, package loading, WASM, or client-side JavaScript execution.
    - Performance: Security validation is static/test-time and adds no runtime cost to editing or rendering.
    - Code Quality: Validation messages name the API ID, docs path, and missing/forbidden authority language.
    - Security: Permission-bearing future APIs must require explicit documented permissions and server-side validation notes before they can enter the public registry.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Configuration Requirement and Phase 8 security focus.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration cannot implicitly grant external authority.
      - `.agents/skills/project-patterns/references/extensions-and-ai.md` if touched by future configuration docs.
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`: Configuration through `init.js` and Clay JS APIs.
    - Options Considered:
      - Rely on manual security review: necessary but insufficient for recurring docs changes.
      - Validate only generated registry entries: catches public artifacts, but source docs should also be checked.
      - Validate both source docs/frontmatter and generated artifacts: best coverage with modest duplicated assertions.
    - Chosen Approach:
      - Extend existing Clay JS API inventory/doc tests and new registry tests to enforce no-authority-by-default language for all configuration-category APIs and any API with configuration tags/custom properties.
    - API Notes and Examples:
      ```text
      Required security language includes: does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
      ```
    - Files to Create/Edit:
      - `tests/clay_js_api_inventory.rs`: Add or tighten source doc security validation for configuration APIs.
      - `tests/clay_js_doc_registry.rs`: Validate generated registry security fields.
      - `docs/reference/clay-js-api/**/*.md`: Fix any missing security notes.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs changes.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - `configuration_docs_deny_implicit_external_authority`: Fails if a configuration API omits required no-authority language.
    - `permission_bearing_configuration_requires_validation_notes`: Fails if any configuration API lists permissions without server-side validation notes.
    - `generated_registry_security_matches_source_docs`: Confirms generated security metadata is not dropped.
  - Completion Notes:
    - Added shared static denied-authority validation in `tests/clay_js_api_inventory.rs` for configuration-relevant public APIs, including configuration entry points, key binding APIs, and APIs with behavior-changing custom properties. The validation now requires source Markdown frontmatter, Markdown body text, and inventory `security_notes` to deny filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, and client-side JavaScript authority with messages that name the API ID and docs path.
    - Added permission-bearing configuration-relevant validation in `tests/clay_js_api_inventory.rs` so any API that lists explicit permissions must also document permission/server validation notes before remaining in the public registry.
    - Added generated-registry security validation in `tests/clay_js_doc_registry.rs` to assert generated configuration-relevant entries retain the full denied-authority boundary and that generated security metadata matches source Markdown frontmatter exactly.
    - Updated `docs/wiki/modules/clay-js-doc-registry.md` with the new no-authority-by-default validation coverage and tests.
    - Verification run: `cargo fmt --check`, `cargo test --test clay_js_api_inventory --test clay_js_doc_registry`, `cargo check`, and full `cargo test` passed.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The phase's configuration APIs are represented by stable Clay JS facade exports, future op wrapper mappings where applicable, Markdown docs, master-index links, generated registry entries, lookup coverage, and tests.
    - Performance: Configuration verification confirms no ordinary keypress/render path depends synchronously on configuration loading, registry generation, JavaScript evaluation, or server work.
    - Code Quality: Configuration is consistently modeled as Clay JS APIs rather than an undocumented settings table; raw Rust functions and raw `Deno.core.ops.op_*` calls remain implementation details.
    - Security: Configuration APIs preserve no-authority-by-default and permission-bearing APIs, if any, include explicit docs and validation notes.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration pattern.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Registry/docs contract.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Generated registry and lookup checks.
    - Options Considered:
      - Treat individual implementation tasks as sufficient: likely misses cross-artifact drift.
      - Add an explicit final configuration verification pass: required by the Clay plan workflow and useful before Phase 9+ APIs multiply.
    - Chosen Approach:
      - After implementation tasks, audit every configuration-category API and every API with behavior-changing `custom_properties`. Reconcile facade, inventory, docs, generated registry, lookup, tests, and security notes.
    - API Notes and Examples:
      ```text
      Verify for each configuration API:
      stable ID -> JS module/export -> docs path -> docs/index.md link -> generated registry entry -> lookup by ID/tag/custom property -> tests.
      ```
    - Files to Create/Edit:
      - `runtime/js/**`: Verify configuration-relevant facade exports.
      - `docs/reference/clay-js-api/**`: Verify configuration docs and API pages.
      - `docs/index.md`: Verify registry source links.
      - `docs/reference/clay-js-api/api-inventory.toml`: Verify configuration classification and metadata.
      - `docs/generated/clay-js-api-registry.json`: Verify generated entries.
      - `tests/**/*.rs`: Verify coverage gates.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - Configuration verification suite: Run focused registry, inventory, facade, and security tests for configuration APIs.
    - Manual configuration API audit: Confirm no undocumented behavior-changing settings remain in public docs/inventory.
  - Completion Notes:
    - Audited configuration-relevant APIs across `runtime/js/**`, `docs/reference/clay-js-api/**`, `docs/index.md`, `docs/reference/clay-js-api/api-inventory.toml`, `docs/generated/clay-js-api-registry.json`, and the registry/inventory/facade tests.
    - Confirmed the Phase 8 configuration surfaces are represented consistently: `clay:configuration` entry point APIs, `clay:keybindings` management/query APIs, `clay.editor.clientSetCursorStyle`, and public APIs with behavior-changing `custom_properties` all have stable IDs, facade exports, future op metadata where applicable, Markdown docs, master-index links, generated registry entries, lookup coverage, and security tests.
    - Confirmed no ordinary keypress/render path depends synchronously on configuration loading, registry generation, JavaScript evaluation, or server work; configuration loading remains planned server-side metadata in Phase 8, and lookup uses checked-in generated registry data only.
    - Confirmed raw Rust functions and raw `Deno.core.ops.op_*` calls remain implementation details behind planned Clay JS facades, and configuration remains no-authority-by-default with static validation for filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, and client-side JavaScript authority.
    - Verification run: `cargo test --test clay_js_doc_registry --test clay_js_api_inventory --test clay_js_facade_layout`, `cargo fmt --check`, and `cargo check` passed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Public programmatic surfaces introduced or changed by Phase 8, including generated registry lookup and configuration facade APIs, are exposed or planned through Clay JS APIs where user-facing, or are kept private/`pub(crate)` when internal.
    - Performance: New public API/documentation infrastructure does not add synchronous JavaScript, registry generation, full-document IPC, or blocking server work to ordinary editor hot paths.
    - Code Quality: Rust public functions introduced for registry generation/checking are classified as internal infrastructure or documented with stable JS API metadata where appropriate.
    - Security: Public APIs have permissions/security notes and do not expose raw ops or Rust internals as user-facing JavaScript.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API verification task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic surface is Clay JS/TS, not raw Rust or raw ops.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required metadata and discoverability fields.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Internal docs infrastructure vs public API docs.
    - Options Considered:
      - Make registry generator public API: useful for tooling, but likely internal developer infrastructure for now.
      - Keep registry generator internals private and expose only documented lookup data: safer phase boundary.
      - Document new user-facing lookup APIs if app/help/agent access becomes public in this phase: acceptable if schema and security are complete.
    - Chosen Approach:
      - Audit Phase 8 Rust visibility and public surfaces. Keep generator/update-command implementation internal. If registry lookup is user/app/agent-facing, document it as a Clay JS API; otherwise record it in the code wiki as internal infrastructure for future app/help surfaces.
    - API Notes and Examples:
      ```text
      For every new public Rust item:
      - classify as internal infrastructure, or
      - map to stable Clay JS API + op + facade + docs + generated registry + lookup tests.
      ```
    - Files to Create/Edit:
      - `src/docs/**/*.rs`: Verify public visibility is intentional.
      - `src/bin/update-doc-registry.rs`: Keep as developer command, not user-facing Clay JS API.
      - `docs/reference/clay-js-api/api-inventory.toml`: Add entries only for public programmatic/user-facing APIs.
      - `tests/rust_visibility_api_mapping.rs`: Extend allowlist/mapping if new public Rust infrastructure appears.
      - `docs/wiki/**`: Document internal registry infrastructure if not public API.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - `server_public_items_have_api_inventory_entries_or_are_allowlisted`: Extend existing visibility test for any new server/docs public Rust items.
    - API verification tests: Confirm any new public Clay JS APIs have docs, generated registry entries, and lookup coverage.
  - Completion Notes:
    - Audited Phase 8 public programmatic surfaces and confirmed the user-facing additions are represented through Clay JS API docs/inventory/facades/generated registry entries: `clay:configuration` entry point APIs, `clay:keybindings` configuration APIs, and editor customization metadata. No new raw Rust function or raw `Deno.core.ops.op_*` path was promoted as a user-facing JavaScript API.
    - Kept `src/bin/update-doc-registry.rs` and `src/docs/registry.rs` generation/check/lookup helpers classified as internal documentation-registry infrastructure for developer tests, update commands, and future app/help/agent plumbing rather than public user-facing Clay JS APIs.
    - Extended `tests/rust_visibility_api_mapping.rs` with `docs_public_items_are_internal_registry_infrastructure`, which fails if new public `src/docs` Rust items appear without explicit internal classification or promotion through Clay JS API docs/inventory/registry coverage. Existing server public-item mapping coverage remains in place.
    - Confirmed the generated registry lookup path is read-only metadata, does not execute configuration or JavaScript, does not grant permissions, and does not add synchronous registry generation, JavaScript evaluation, full-document IPC, or blocking server work to editor hot paths.
    - Updated `docs/wiki/modules/clay-js-doc-registry.md` to document the public Rust visibility classification and added visibility-test coverage.
    - Verification run: `cargo fmt --check`, `cargo test --test clay_js_doc_registry --test clay_js_api_inventory --test clay_js_facade_layout --test rust_visibility_api_mapping`, and `cargo check` passed.

- [x] Run Phase 8 verification
  - Acceptance Criteria:
    - Functional: Phase 8 prerequisite reconciliation, generated registry artifacts, lookup APIs, configuration docs/facades/inventory, and security validation are complete and consistent.
    - Performance: Verification confirms no new runtime path causes ordinary typing, rendering, or manifest-declared client-first behavior to block on IPC, server work, JavaScript, AI, file IO, configuration loading, registry generation, or full-document serialization.
    - Code Quality: `cargo fmt --check`, `cargo test`, and `cargo check` pass; generated artifacts are current and tests fail actionably if they drift.
    - Security: Verification confirms configuration remains no-authority-by-default and no arbitrary JavaScript execution is introduced in the Rust client.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 8 expected outcome and phase boundaries.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Phase-level verification checklist.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Hot-path and protocol performance constraints.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Deterministic checks and actionable failures.
    - Options Considered:
      - Run only new registry/configuration tests: fast, but may miss regressions in behavior/IPC/editor hot paths.
      - Run full verification command set: slower, but appropriate for a phase-level foundation.
    - Chosen Approach:
      - Run focused tests during tasks, then final `cargo fmt --check`, `cargo test`, and `cargo check`. Run `cargo run --bin update-doc-registry` only when artifacts need regeneration, not as a substitute for stale checks.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo test
      cargo check
      ```
    - Files to Create/Edit:
      - No new files expected; update failing docs/generated artifacts/tests only as needed.
    - References:
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Full verification command set: `cargo fmt --check`, `cargo test`, and `cargo check` pass.
    - Manual phase-boundary review: Confirm Phase 8 did not implement arbitrary JavaScript execution, package loading, file/workspace authority, or client-side JS execution.
  - Completion Notes:
    - Ran the final Phase 8 verification command set: `cargo fmt --check`, `cargo test`, and `cargo check` all passed.
    - Confirmed generated registry verification is active and current through `generated_registry_is_current`, with the actionable repair command `cargo run --bin update-doc-registry` covered by the registry tests rather than silent test-time mutation.
    - Confirmed focused Phase 8 coverage passed inside the full suite: Clay JS API inventory/docs consistency and security tests, generated registry lookup/security tests, facade layout tests, and Rust visibility/API-boundary mapping tests.
    - Confirmed performance boundary through existing passing hot-path tests including ordinary typing/client-first behavior without server or JavaScript waits, bounded edit queues, no full-document IPC for ordinary edits, and manifest routing tests.
    - Confirmed security phase boundary: Phase 8 defines configuration contracts and metadata only; it does not introduce arbitrary JavaScript execution in the Rust client, package loading, filesystem/workspace/network/shell/WASM authority, extension loading, or AI mutation authority.
    - Verification run: `cargo fmt --check`, `cargo test`, and `cargo check` passed.

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
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/modules/clay-js-doc-registry.md`: Verify and update implementation details for generated registry, lookup, configuration metadata, security validation, visibility classification, and tests.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Verify and update Phase 8 configuration, key binding, and editor customization facade boundaries.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Completion Notes:
    - Reviewed `docs/wiki/index.md`, `docs/wiki/modules/clay-js-doc-registry.md`, and `docs/wiki/modules/clay-js-facade-skeleton.md` after Phase 8 implementation and final verification.
    - Confirmed the master wiki index links the relevant Phase 8 pages and describes the generated Clay JS documentation registry and facade skeleton for onboarding.
    - Confirmed `docs/wiki/modules/clay-js-doc-registry.md` documents the generated registry source/test paths, deterministic generation and stale checks, read-only generated-data lookup APIs, configuration entry point metadata, key binding and cursor customization metadata, no-authority-by-default validation, Rust visibility classification, invariants, examples, and test coverage.
    - Confirmed `docs/wiki/modules/clay-js-facade-skeleton.md` documents the Phase 8 `clay:configuration` entry point contract, key binding facade shape, cursor customization properties, planned-runtime boundary, hot-path/performance constraints, and denied external authority.
    - Added links from both relevant wiki pages back to `plans/009-Phase8-Configuration-Foundation.md` for traceability.
    - Manual wiki review passed, including a `python3` check that every `docs/wiki/**/*.md` page is linked from `docs/wiki/index.md`; no runtime verification was needed because only Markdown documentation and this plan were updated after the final `cargo fmt --check`, `cargo test`, and `cargo check` verification run.

## Compromises Made
- Phase 8 intentionally defines configuration contracts, generated registry metadata, and validation infrastructure without evaluating `~/.config/clay/init.js` or executing modular configuration JavaScript. Runtime execution remains deferred to Phase 11 to preserve the no-authority-by-default boundary.
- Generated registry lookup is implemented as internal Rust documentation infrastructure for future app/help/agent surfaces, not as a new public Clay JS API. This avoids exposing tooling internals before a product surface requires them.
- Key binding and editor customization APIs remain planned facade contracts and metadata validation in Phase 8; live behavior-manifest mutation and broader theme/settings APIs are deferred until later runtime/product phases.

## Further Actions
- Phase 11 should implement server-side configuration JavaScript evaluation only after explicit permission, module-loading, and validation boundaries are designed and tested.
- Future app/help/agent UI work should consume the generated registry lookup layer and decide whether any user-facing discovery API needs a formal Clay JS facade entry.
- Later customization phases should add additional settings only when backed by docs, inventory metadata, generated registry coverage, no-authority security notes, and behavior/performance tests.
