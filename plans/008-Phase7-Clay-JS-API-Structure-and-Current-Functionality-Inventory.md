# Phase 7 Clay JS API Structure and Current Functionality Inventory

## Objectives
- Define the initial Clay JavaScript/TypeScript facade source tree and module layout without exposing raw Rust functions or raw `Deno.core.ops.op_*` calls as the public API.
- Inventory current editor, protocol, behavior, key binding, configuration, and application actions that should become public Clay JS APIs, and classify everything else as internal implementation detail.
- Apply Clay's authority, naming, documentation-as-code, configuration-as-API, and behavior-manifest constraints to concrete planned API names, stable registry IDs, documentation paths, backing Rust owners, and future op wrappers.
- Prepare Phase 3 deferred documentation, registry, lookup, and coverage work to resume against real API names instead of invented placeholders.
- Preserve the no-client-JavaScript and no-synchronous-keypress-to-JavaScript performance boundary.

## Expected Outcome
- A checked-in Clay JS API source-tree skeleton exists for stable user-facing facades, with domain modules such as `clay:editor`, `clay:keybindings`, `clay:configuration`, `clay:documents`, and `clay:behavior` represented by source files and exports or documented planned stubs.
- Current functionality has a durable inventory that records public/internal classification, authority owner, runtime path, JS module/export, stable registry ID, `user_facing_name`, key binding metadata requirement, custom property metadata requirement, permissions/security notes, backing Rust owner/path where known, future `deno_core` op wrapper name where applicable, documentation path, and implementation status.
- Existing Rust visibility is reviewed so server-side public functions that are not intended public programmatic surfaces are made private or `pub(crate)`, while intended public capabilities are mapped to future explicit op wrappers and stable JS/TS facades.
- Planned public Clay JS API Markdown pages or inventory-backed reference records exist under `docs/reference/clay-js-api/` and are linked from `docs/index.md` according to the current schema.
- Configuration and key binding/customization surfaces are represented as documented Clay JS APIs, including empty default key binding lists and `custom_properties` for behavior-changing settings.
- Tests or deterministic checks cover inventory/schema consistency, naming rules, index links, Rust visibility/API mapping, and registry-readiness as far as the current Phase 3 infrastructure supports.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- This phase does not introduce arbitrary JavaScript execution in the Rust client, does not put JavaScript in the ordinary typing path, does not implement the package system, does not add file/workspace authority, and does not expand runtime permissions beyond documented planned metadata.

## Tasks

- [x] Define the Clay JS facade source tree and module layout
  - Acceptance Criteria:
    - Functional: The repository contains an initial JS/TS facade source-tree skeleton for domain modules, with public exports represented as typed planned stubs or documented placeholders rather than raw op calls.
    - Performance: The facade layout introduces no runtime work in Masonry paint/input handlers and no synchronous keypress -> IPC -> JavaScript -> IPC -> paint path.
    - Code Quality: Module names and exports follow Clay naming rules, separate user APIs from raw ops, and leave implementation internals behind Rust/private module boundaries.
    - Security: The skeleton does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, or client-side JavaScript execution authority.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 7: Define initial Clay JS API source tree and facade/module layout after IPC, synchronization, and behavior manifests.
      - `docs/reference/clay-js-api/schema.md`: Required Clay JS API metadata and body schema.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Clay JS/TS facades are the public programmatic surface, not raw Rust functions or raw ops.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Domain modules, lower-camel-case exports, stable registry IDs, and server/client authority markers.
      - Context7 `/denoland/deno_core`: Use `#[op2]` functions registered through `extension!` for Rust ops when runtime binding is implemented later.
    - Options Considered:
      - Implement `deno_core` runtime execution now: closer to final extension support, but belongs to Phase 11 and risks expanding authority too early.
      - Create only Markdown prose: low risk, but leaves no concrete source-tree target for future facades and tests.
      - Create typed facade skeleton files with planned APIs and explicit unimplemented/runtime-placeholder behavior: gives future phases concrete paths while preserving Phase 7 boundaries.
    - Chosen Approach:
      - Add a source-tree skeleton under a stable facade directory, tentatively `runtime/js/`, with domain files and an index/import-map note. Export planned APIs as typed stubs or declarations that do not call raw ops until Phase 11 wires server-side `deno_core`.
    - API Notes and Examples:
      ```ts
      // runtime/js/editor.ts
      export async function serverInsertText(options: ServerInsertTextOptions): Promise<EditResult> {
        throw new Error("clay.editor.serverInsertText is planned; runtime op wiring is not implemented yet");
      }
      ```
      ```rust
      // Future Phase 11 binding shape from deno_core docs.
      #[deno_core::op2(async)]
      async fn op_clay_editor_insert_text(/* state and args */) -> Result<(), deno_core::error::AnyError> {
          todo!("call server-authoritative edit implementation")
      }
      ```
    - Files to Create/Edit:
      - `runtime/js/editor.ts`: Planned editor/document/UI behavior facade exports.
      - `runtime/js/keybindings.ts`: Planned key binding facade exports.
      - `runtime/js/configuration.ts`: Planned configuration facade exports.
      - `runtime/js/documents.ts`: Planned document authority facade exports.
      - `runtime/js/behavior.ts`: Planned behavior manifest/query facade exports.
      - `runtime/js/mod.ts`: Re-export or document module entry points.
      - `runtime/js/README.md`: Explain facade status, module specifiers, and why raw ops are not user-facing.
      - `tests/clay_js_facade_layout.rs`: Deterministic facade layout, export, and naming/boundary checks.
      - `docs/wiki/modules/clay-js-facade-skeleton.md` and `docs/wiki/index.md`: Implementation wiki coverage for the new skeleton.
      - `docs/reference/clay-js-api/schema.md`: No update needed; the existing schema already covered the skeleton boundary.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - Context7 `/denoland/deno_core` op/extension documentation.
  - Test Cases Written:
    - `tests/clay_js_facade_layout.rs::clay_js_facade_modules_exist_with_expected_exports`: Confirms initial domain facade paths and planned exports exist.
    - `tests/clay_js_facade_layout.rs::clay_js_facade_exports_follow_naming_and_boundary_rules`: Rejects raw op-shaped exports, redundant names, and raw `Deno.core.ops` calls from facade files.
  - Verification:
    - `cargo fmt --check`
    - `cargo test clay_js_facade --test clay_js_facade_layout`
    - `cargo test`
    - `cargo check`

- [x] Inventory current functionality and classify API authority/runtime paths
  - Acceptance Criteria:
    - Functional: Current functionality is inventoried for text insertion, newline, Backspace/Delete, cursor movement, selection, scrolling, resize/viewport behavior, cursor style/customization, key binding management, behavior manifest routing, lease/read-only state, and Escape/quit/application actions.
    - Performance: The inventory classifies hot-path client-first behavior separately from server-first or background work and records that ordinary typing remains local and asynchronous to the server.
    - Code Quality: Each record has a stable schema with no ambiguous ownership; internal-only entries are clearly excluded from public registry generation.
    - Security: Each public/planned API record states permissions and explicitly notes authority not granted, including filesystem, network, shell, extension loading, AI mutation, and workspace access where absent.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 7: Inventory and classify current Clay functionality by authority and runtime path.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns canonical documents/leases/locks; client owns rendering/input/viewport/caret/selection transient state.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Client executes inert server-issued manifests for hot-path predictable behavior.
      - `docs/wiki/modules/behavior-manifests.md` and `docs/wiki/flows/client-behavior-routing.md`: Current implemented behavior-routing details.
      - `src/editor/**`, `src/client/**`, `src/server/**`, `src/protocol/**`, `src/behavior/**`: Current implementation owners to audit.
    - Options Considered:
      - Use one free-form Markdown table only: readable, but harder to validate by tests.
      - Use machine-readable TOML/JSON/YAML only: easy to validate, but less helpful to humans.
      - Use a machine-readable inventory plus generated or companion Markdown explanation: better for tests and planning, with acceptable maintenance cost.
    - Chosen Approach:
      - Create a machine-readable inventory, tentatively `docs/reference/clay-js-api/api-inventory.toml`, plus a human Markdown overview. Use the inventory as the deterministic source for Phase 7 validation while per-API Markdown remains the public documentation source.
    - API Notes and Examples:
      ```toml
      [[api]]
      id = "clay.editor.serverInsertText"
      js_module = "clay:editor"
      js_export = "serverInsertText"
      user_facing_name = "Insert Text"
      authority = "server-authoritative-document-mutation"
      runtime_path = "server-first-op-wrapper"
      backing_rust = "src/server/document.rs::DocumentState::apply_edit"
      deno_op = "op_clay_editor_insert_text"
      documentation_path = "docs/reference/clay-js-api/editor/server-insert-text.md"
      key_bindings = []
      permissions = ["document-edit"]
      status = "planned"
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: Machine-readable planned/current API inventory.
      - `docs/reference/clay-js-api/inventory.md`: Human overview of classifications and phase boundaries.
      - `docs/reference/clay-js-api/editor/*.md`: Planned editor API docs or records for inventoried public editor APIs.
      - `docs/reference/clay-js-api/keybindings/*.md`: Planned key binding API docs or records.
      - `docs/reference/clay-js-api/configuration/*.md`: Planned configuration/customization API docs or records.
      - `docs/reference/clay-js-api/behavior/*.md`: Planned behavior manifest/query API docs or records.
      - `docs/index.md`: Link public API docs under **Clay JS API Registry Source Files**.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/schema.md`
  - Test Cases Written:
    - `tests/clay_js_api_inventory.rs::api_inventory_has_required_fields`: Every public inventory entry has ID, module/export, user-facing name, authority, runtime path, docs path, permissions, key binding metadata, security notes, and status; IDs are unique.
    - `tests/clay_js_api_inventory.rs::api_inventory_classifies_current_editor_behavior`: Required Phase 7 functionality categories are present and hot-path entries record asynchronous server behavior.
    - `tests/clay_js_api_inventory.rs::api_inventory_does_not_mark_internal_details_public`: Internal implementation entries are classified as internal, excluded from public registry generation, and have no JS module/export.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --test clay_js_api_inventory`

- [x] Audit Rust visibility and map intended public capabilities to facades and future ops
  - Acceptance Criteria:
    - Functional: Existing Rust public items are reviewed, server-side public programmatic capabilities are mapped to Clay JS facade exports and future `deno_core` op names, and non-public implementation details are made private or `pub(crate)` where practical without breaking intended crate tests.
    - Performance: Visibility and mapping changes do not alter edit hot-path behavior or introduce new synchronization/serialization work.
    - Code Quality: Public Rust API exposure is intentional, documented, and minimized; future op wrapper names are stable and derived from Clay JS API names rather than leaking Rust internals.
    - Security: No raw Rust public function or raw op becomes the user-facing API; server-side authority remains validated on the server.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public server Rust functions require explicit op wrappers and stable JS/TS facades, or should become private/`pub(crate)`.
      - `src/lib.rs`: Current crate module exports.
      - `src/server/mod.rs`, `src/server/document.rs`, `src/server/behavior.rs`, `src/server/connection.rs`: Current server-side owners.
      - `src/protocol/mod.rs`: Current protocol data types and public wire DTOs.
      - `src/editor.rs` and `src/editor/**`: Client/editor public re-exports and internal modules.
    - Options Considered:
      - Make all modules private immediately: strong encapsulation, but may break integration tests and future internal reuse.
      - Leave all current visibility unchanged and only document later: easy, but conflicts with the approved API boundary.
      - Perform a surgical visibility audit: convert clear internals to `pub(crate)`, keep protocol DTOs and necessary crate-facing types public, and record unresolved exposure decisions in the inventory.
    - Chosen Approach:
      - Used `rg`/targeted review plus deterministic tests to identify externally visible server/editor items. Existing server document/behavior/connection implementation details were already `pub(crate)`, so no Rust visibility reductions were required for this task; public server process infrastructure is classified as internal/non-JS inventory or allowlisted by validation, while public capabilities remain mapped through facade/op/docs metadata.
    - API Notes and Examples:
      ```text
      Rust owner: src/server/document.rs::DocumentState::apply_edit
      Future op: op_clay_editor_insert_text
      JS facade: runtime/js/editor.ts::serverInsertText
      Stable ID: clay.editor.serverInsertText
      Documentation: docs/reference/clay-js-api/editor/server-insert-text.md
      ```
    - Files to Create/Edit:
      - `src/lib.rs`: Adjust crate module visibility only when an exported module is not intended as a public API boundary.
      - `src/server/*.rs`: Make internal server helpers private/`pub(crate)` as appropriate.
      - `src/editor.rs` and `src/editor/**`: Keep UI/editor internals private unless intentionally crate-facing.
      - `src/protocol/mod.rs`: Keep wire DTOs public where needed, but document that they are not the Clay JS user API.
      - `docs/reference/clay-js-api/api-inventory.toml`: Record Rust owner/op/facade mapping and classify public server IPC runtime types as internal/non-JS infrastructure.
      - `tests/clay_js_api_inventory.rs`: Add future-op/user-facing-export boundary validation.
      - `tests/rust_visibility_api_mapping.rs`: Add deterministic server Rust visibility mapping/allowlist validation.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases Written:
    - `tests/rust_visibility_api_mapping.rs::server_public_items_have_api_inventory_entries_or_are_allowlisted`: Deterministic check over server Rust files catches unclassified public server items.
    - `tests/clay_js_api_inventory.rs::inventory_future_ops_are_not_user_facing_exports`: Ensures JS exports do not start with raw `op_` names and future ops remain explicit `op_clay_*` wrappers.
    - Existing unit/integration behavior was unaffected because no Rust visibility reductions were necessary.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --test clay_js_api_inventory`
    - `cargo test --test rust_visibility_api_mapping`

- [x] Author planned Clay JS API reference docs and link them from the master index
  - Acceptance Criteria:
    - Functional: Every inventoried public/planned API has a Markdown reference path or explicitly documented deferred status, with required frontmatter/body fields where full API docs are authored.
    - Performance: Documentation and registry preparation remain offline developer/test work and add no runtime editor hot-path cost.
    - Code Quality: Docs use the schema consistently, include concrete JS usage examples, and avoid duplicating internal wiki implementation details.
    - Security: Each API doc states authority boundaries, permissions, and what authority the API does not grant.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/schema.md`: Required frontmatter fields and required Markdown headings.
      - `docs/index.md`: Master Markdown index and Clay JS API Registry Source Files section.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown is authoritative; generated registries derive from indexed Markdown.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Coverage tests should fail for missing/malformed docs, links, registry entries, and lookup access.
    - Options Considered:
      - Defer all per-API docs after inventory: smaller Phase 7, but does not satisfy the self-documenting contract for planned public APIs.
      - Author complete planned docs for the initial public inventory: more work, but gives Phase 3 registry/lookup tasks concrete input.
      - Author one aggregate inventory page only: useful overview, but not enough for per-API discoverability.
    - Chosen Approach:
      - Author planned per-API Markdown pages for the initial inventory where API shape is clear, using `stability: planned` where runtime is not implemented. For uncertain APIs, keep them internal or mark them as inventory candidates until names/options are stable.
    - API Notes and Examples:
      ```markdown
      ---
      id: clay.editor.clientSetCursorStyle
      kind: clay-js-api
      js_module: clay:editor
      js_export: clientSetCursorStyle
      key_bindings: []
      custom_properties:
        - name: color
          type: string
          default: inherited
          description: Cursor color override.
      ---
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/editor/server-insert-text.md`: Planned server-authoritative insertion API.
      - `docs/reference/clay-js-api/editor/server-delete-range.md`: Planned server-authoritative deletion API.
      - `docs/reference/clay-js-api/editor/client-move-cursor.md`: Planned client-local cursor movement API.
      - `docs/reference/clay-js-api/editor/client-set-selection.md`: Planned client-local selection API.
      - `docs/reference/clay-js-api/editor/client-scroll-to.md`: Planned client-local scrolling API.
      - `docs/reference/clay-js-api/editor/client-set-cursor-style.md`: Planned cursor customization API.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Planned key binding management API.
      - `docs/reference/clay-js-api/behavior/get-active-behavior-manifest.md`: Planned behavior discovery/query API.
      - `docs/reference/clay-js-api/application/quit.md`: Planned quit/application action API, if classified public.
      - `docs/reference/clay-js-api/editor/server-insert-newline.md`, `editor/client-set-viewport.md`, `keybindings/unbind-key.md`, `keybindings/list-key-bindings.md`, `behavior/list-behavior-routes.md`, `documents/server-get-document-snapshot.md`, and `documents/server-get-document-lease.md`: Additional public inventory API docs authored because the inventory classified them as public planned APIs.
      - `docs/index.md`: Add every public API page to the registry source list.
    - References:
      - `docs/reference/clay-js-api/schema.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases Written:
    - `tests/clay_js_api_inventory.rs::clay_js_api_docs_have_required_frontmatter_and_body_sections`: Validates required schema fields, required body headings, security notes, lookup tags, and TypeScript usage examples for all public API docs.
    - `tests/clay_js_api_inventory.rs::docs_index_links_all_public_inventory_docs`: Fails when public inventory docs are not linked from `docs/index.md`.
    - `tests/clay_js_api_inventory.rs::api_docs_match_inventory_ids_and_exports`: Fails when docs and inventory disagree on stable IDs, modules, exports, facade paths, Rust owners, op names, or user-facing names.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --test clay_js_api_inventory`

- [x] Create deterministic inventory, naming, and registry-readiness validation
  - Acceptance Criteria:
    - Functional: `cargo test` or a deterministic repository check fails when inventory entries, facade exports, docs, master-index links, key binding metadata, custom property metadata, or naming rules are missing or stale.
    - Performance: Validation runs only in tests/developer commands, not in editor runtime, IPC handlers, or Masonry paint/input handlers.
    - Code Quality: Failures are actionable and name the file, API ID, and expected repair; checks do not silently mutate generated artifacts.
    - Security: Validation catches missing permission/security metadata for public APIs and rejects docs that imply unauthorized authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer automated checks over instruction-only maintenance, with actionable repair commands.
      - `docs/reference/clay-js-api/schema.md`: Parser and registry expectations.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Naming rules to validate.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required metadata fields.
    - Options Considered:
      - Rely on manual review: quick, but not durable for a self-documenting program.
      - Build the full generated registry and lookup API now: ideal eventually, but may exceed Phase 7 if Phase 3 generator infrastructure is absent.
      - Add focused validation tests now and leave full registry generation to the resumed Phase 3 tasks: good coverage without overbuilding.
    - Chosen Approach:
      - Added focused non-mutating validation tests for inventory/docs/index/facade consistency, naming rules, security metadata, key binding metadata, and custom property metadata. Full generated registry/lookup artifact validation remains deferred to resumed Phase 3 generator work because no checked-in generator exists yet; the new tests validate registry readiness from the authoritative Markdown/index inputs without mutating artifacts.
    - API Notes and Examples:
      ```rust
      #[test]
      fn clay_js_api_inventory_docs_and_index_are_consistent() {
          // Parse docs/reference/clay-js-api/api-inventory.toml,
          // parse docs/index.md registry source links,
          // validate required fields and matching IDs.
      }
      ```
    - Files to Create/Edit:
      - `tests/clay_js_api_inventory.rs`: Deterministic validation of inventory/docs/index/facade consistency, naming rules, and metadata completeness.
      - `tests/clay_js_facade_layout.rs`: Extended facade export smoke coverage for newly validated editor/application exports.
      - `runtime/js/editor.ts`: Added planned facade stubs for inventory-listed editor APIs that validation now checks.
      - `runtime/js/application.ts`: Added the planned application lifecycle facade stub used by the public `quit` API inventory entry.
      - `runtime/js/mod.ts`: Re-exported the application facade namespace.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Updated implementation wiki coverage for the application facade and stricter validation.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `docs/reference/clay-js-api/schema.md`
  - Test Cases Written:
    - `tests/clay_js_api_inventory.rs::clay_js_api_inventory_docs_and_index_are_consistent`: Cross-validates public inventory entries, per-API docs, exact `docs/index.md` registry-source links, backing implementation metadata, and facade export paths.
    - `tests/clay_js_api_inventory.rs::clay_js_api_names_follow_project_conventions`: Enforces `clay.<module>.<export>` stable IDs, lower-camel-case exports, server/client authority markers for editor/document state APIs, and no raw op/Rust/project-shaped callable names.
    - `tests/clay_js_api_inventory.rs::public_api_docs_include_security_keybinding_and_custom_properties`: Validates required discoverability/security fields, declared key bindings, custom property metadata in frontmatter/body content, and explicit denial of unauthorized authority.
    - `tests/clay_js_facade_layout.rs::clay_js_facade_modules_exist_with_expected_exports`: Extended to include the application facade plus newly validated editor facade exports.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --test clay_js_facade_layout`
    - `cargo test --test clay_js_api_inventory`

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: The phase's public programmatic surfaces and changed Rust public functions are represented by stable Clay JS API facade exports, future op wrapper mappings, Markdown docs, master-index links, and inventory entries.
    - Performance: API verification does not add synchronous JavaScript/runtime execution to the editor hot path and keeps ordinary typing client-first/asynchronous according to behavior manifests.
    - Code Quality: User-facing APIs are stable facades with clear names, examples, lookup tags, and tests; raw Rust functions and raw `Deno.core.ops.op_*` calls remain implementation details.
    - Security: Every API has permissions/security notes and does not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace access, or arbitrary client JavaScript execution.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API verification task for each Clay plan.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic surface is Clay JS/TS, not raw Rust or ops.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required user-facing name, key binding metadata, custom properties, docs, registry, and lookup coverage.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown plus master index is authoritative.
    - Options Considered:
      - Treat the previous implementation tasks as sufficient: they create most artifacts, but a separate verification pass is required by the Clay plan workflow.
      - Add a final verification task: slightly repetitive, but catches drift between source tree, Rust visibility, docs, and inventory.
    - Chosen Approach:
      - Perform an explicit API verification pass after implementation tasks. Reconcile any public Rust changes with the inventory and docs; make non-public Rust internals private/`pub(crate)` rather than documenting them as public APIs.
    - API Notes and Examples:
      ```text
      For each public capability:
      JS module/export -> stable ID -> user_facing_name -> docs path -> facade path -> future deno_op -> backing Rust owner -> tests.
      ```
    - Files to Create/Edit:
      - `runtime/js/**`: Verify facade exports for planned public APIs.
      - `docs/reference/clay-js-api/**`: Verify Markdown docs for public APIs.
      - `docs/index.md`: Verify links under Clay JS API Registry Source Files.
      - `docs/reference/clay-js-api/api-inventory.toml`: Verify inventory coverage.
      - `src/**/*.rs`: Verify server-side Rust public functions are either mapped or made private/`pub(crate)`.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases Written:
    - API verification tests from earlier tasks pass under `cargo test --test clay_js_facade_layout --test clay_js_api_inventory --test rust_visibility_api_mapping`.
    - Manual API audit: `rg '^pub ' src/server -n` plus `tests/rust_visibility_api_mapping.rs` confirmed public server Rust items are inventoried or explicitly allowlisted as non-JS server infrastructure.
    - Documentation audit: `tests/clay_js_api_inventory.rs` confirmed each public API has user-facing name, key bindings, custom properties, examples, permissions/security notes, backing Rust path, future op name, facade path, lookup tags, and `docs/index.md` linkage.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --test clay_js_facade_layout --test clay_js_api_inventory --test rust_visibility_api_mapping`

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Configuration-relevant behavior from the inventory, including key binding management and editor customization such as cursor style, has planned Clay JS API docs and inventory entries using `~/.config/clay/init.js` as the user configuration entry point.
    - Performance: Configuration APIs remain planned/server-side and do not add client-side arbitrary JavaScript or runtime configuration evaluation in the editor hot path.
    - Code Quality: Configuration is modeled as documented Clay JS APIs, not undocumented key/value settings, and every behavior-changing option is listed in `custom_properties`.
    - Security: Configuration APIs do not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace access, or package loading authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required configuration task when behavior, commands, key bindings, customization, APIs, or public surfaces change.
      - `docs/reference/clay-js-api/configuration.md`: Configuration entry point and configuration-as-API model.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Project configuration pattern.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: `key_bindings` and `custom_properties` requirements.
    - Options Considered:
      - Defer configuration APIs entirely to Phase 8: matches roadmap sequencing, but Phase 7 must inventory current configuration/customization surfaces.
      - Implement runtime configuration loading now: too early; belongs to Phase 8/11.
      - Create planned configuration API inventory/docs now: preserves phase boundary while giving Phase 8 concrete APIs to implement.
    - Chosen Approach:
      - Document planned configuration and key binding APIs as part of the inventory and reference docs, with runtime status clearly marked `planned`. Do not evaluate `init.js` or load user files in this phase.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js, future Phase 8/11 behavior.
      import { bindKey } from "clay:keybindings";
      import { clientSetCursorStyle } from "clay:editor";

      bindKey("Ctrl+I", "clay.editor.serverInsertText");
      clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Update only if Phase 7 inventory clarifies configuration status.
      - `docs/reference/clay-js-api/configuration/*.md`: Planned configuration API docs, if separate from key binding/editor docs.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Planned key binding API docs.
      - `docs/reference/clay-js-api/editor/client-set-cursor-style.md`: Planned cursor customization API docs.
      - `docs/reference/clay-js-api/api-inventory.toml`: Mark configuration API entries and custom properties.
      - `docs/index.md`: Link public configuration API docs.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - `configuration_api_docs_include_init_js_security_notes`: Confirms configuration docs mention `~/.config/clay/init.js` and no-authority-by-default security.
    - `configuration_api_custom_properties_are_documented`: Confirms cursor/key binding customization APIs list behavior-changing properties.
    - `configuration_apis_have_empty_or_explicit_key_bindings`: Confirms key binding metadata is always present.

- [ ] Run Phase 7 verification
  - Acceptance Criteria:
    - Functional: Phase 7 source tree, inventory, docs, visibility, and validation artifacts are complete and consistent.
    - Performance: No new runtime path causes ordinary typing, rendering, or manifest-declared client-first behavior to block on IPC, server work, JavaScript, AI, file IO, or full-document serialization.
    - Code Quality: `cargo fmt`, `cargo test`, and `cargo check` pass; validation failures are actionable and deterministic.
    - Security: Verification confirms no new filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority has been introduced.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 7 expected outcome and phase boundaries.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Decision alignment, authority, hot path, documentation, configuration, security, performance, and phase-boundary checks.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Preserve no full-document IPC and no blocking UI handlers.
    - Options Considered:
      - Run only new validation tests: faster, but may miss regressions in IPC/editor behavior.
      - Run full Rust checks: slower, but appropriate for a phase-level plan.
    - Chosen Approach:
      - Run `cargo fmt --check`, `cargo test`, and `cargo check`. If validation commands are added, run them as part of `cargo test` or document the exact command.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo test
      cargo check
      ```
    - Files to Create/Edit:
      - No new files expected; update failing docs/inventory/tests only as needed.
    - References:
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Full verification command set: `cargo fmt --check`, `cargo test`, and `cargo check` pass.
    - Manual phase-boundary review: Confirm runtime JavaScript execution and configuration loading remain deferred.

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
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
