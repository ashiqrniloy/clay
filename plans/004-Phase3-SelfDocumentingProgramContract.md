# Phase 3 Self-Documenting Program Contract

## Objectives
- Make Clay JS API documentation part of the code contract before IPC, protocol, behavior manifest, command, extension, and AI-tool surfaces grow.
- Make Markdown files the authoritative documentation source of truth, organized by a master Markdown index.
- Generate app/programmatic documentation registries and lookup artifacts from the indexed Markdown files rather than authoring registry data separately.
- Add validation so server-side Rust public functions cannot be added without corresponding Clay JS APIs, Markdown documentation, master-index inclusion, generated registry coverage, and lookup-accessible documentation.
- Document the existing Clay JS API contract and current editor-facing concepts enough that future Phase 4 protocol work has a pattern to follow.
- Keep the system local, build/test driven, and independent of server, extension, AI, file IO, remote, or network authority.

## Expected Outcome
- The repository contains authoritative Markdown documentation files for Clay JS APIs and associated public programmatic capabilities.
- A master Markdown index enumerates and links the Clay JS API documentation set used for registry generation.
- A generated registry artifact exists for direct lookup by stable ID, kind, owner, and capability tags.
- The generated registry exposes or feeds a programmatic lookup API that future app help, command palette, key binding UI, configuration UI, extension tooling, and AI tools can use without scraping Markdown.
- Tests fail when server-side Rust public functions lack corresponding Clay JS APIs, or when Clay JS APIs lack required Markdown documentation, master-index inclusion, generated registry output, and programmatic lookup coverage.
- Tests fail when the checked-in generated registry is stale relative to the Markdown source of truth and instruct the user to update it with `cargo run --bin update-doc-registry`.
- Current editor-facing programmatic concepts, the Clay JS API exposure pattern, and the `~/.config/clay/init.js` configuration model are documented in Markdown and available through the generated registry where applicable.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- Phase 4 IPC/protocol work can add protocol messages and behavior manifest entries with documentation requirements already established.

## Tasks

- [x] Define the Markdown documentation schema and master index
  - Acceptance Criteria:
    - Functional: The project has a documented Markdown/frontmatter schema for Clay JS APIs, including at least stable ID, kind, JS module/export, backing Rust function path, `deno_core` op wrapper path/name, name, searchable user-facing name, default key bindings or an empty key binding list, custom properties for behavior-changing settings, summary, detailed description, why/when to use it, JavaScript usage, code example, configuration/options, return/async behavior, errors, owner component, phase, visibility, permissions/security notes, agent guidance, lookup tags, and app/help visibility. A master Markdown index links every authored Clay JS API documentation file that participates in registry generation.
    - Performance: Markdown parsing and registry generation are offline or test-time work and do not add runtime cost to the editor paint/input path.
    - Code Quality: The Markdown schema is small, typed by the parser where practical, and avoids a sprawling documentation framework before Clay has public extension APIs.
    - Security: The schema can record permissions/security notes but does not grant permissions, execute scripts, load extensions, or expose filesystem/network authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Documentation-as-code applies to Clay JS APIs and must include what/why/when, JavaScript usage, examples, options, backing Rust/op paths, generated registries, and lookup APIs.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Server-side Rust public functions must be surfaced through Clay JS APIs or made private/`pub(crate)`.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Registry freshness and coverage checks must be non-mutating and fail with Cargo update instructions.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Clay JS APIs must include user-facing names, key binding metadata, and custom properties for behavior-changing settings.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration options are Clay JS APIs loaded from `~/.config/clay/init.js` and documented through the same Markdown registry contract.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Public programmatic behavior must be exposed and documented through Clay JS APIs; internal implementation details belong in the project wiki.
      - `roadmap.md`: Phase 3 requires a self-documenting program contract before IPC and extension surfaces grow.
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`: Future protocol, command, manifest, permission, and AI tool surfaces must stay inspectable.
    - Options Considered:
      - Free-form Markdown with no parser/index: easy to write, but it drifts from public interfaces and is hard for agents or the app to query reliably.
      - Separately authored Rust/static registry plus generated docs: convenient for app lookup, but creates drift risk between human Markdown and programmatic docs.
      - Markdown as source of truth plus generated registry: keeps the human-readable docs authoritative while still giving the app, tools, and agents structured lookup data.
    - Chosen Approach:
      - Define a minimal Markdown/frontmatter schema and a master `docs/index.md` that links all Clay JS API documentation files included in registry generation. Derive `DocEntry`-style registry records from those Markdown files instead of hand-authoring a separate registry source.
    - API Notes and Examples:
      ````markdown
      ---
      id: clay.editor.serverInsertText
      kind: clay-js-api
      js_module: clay:editor
      js_export: serverInsertText
      backing_rust: src/server/editor.rs::insert_text
      deno_op: op_clay_editor_insert_text
      name: serverInsertText
      owner: server
      phase: Phase 3
      visibility: public
      app_visible: true
      lookup_tags: [editor, js-api, text]
      security: Requires document edit authority.
      agent_guidance: Use this API when a script needs to request an authoritative text insertion.
      ---

      # serverInsertText

      Inserts text through the server-authoritative edit path.

      ## When to use

      Use from JavaScript when an extension or AI tool needs to request text insertion with document authority.

      ## JavaScript usage

      ```ts
      import { serverInsertText } from "clay:editor";

      await serverInsertText({ documentId, offset, text: "hello" });
      ```

      ## Options

      - `documentId`: target document.
      - `offset`: byte or protocol-defined text offset.
      - `text`: inert text to insert.
      ````
    - Files Created/Edited:
      - `docs/index.md`: Master Markdown documentation index used by users, agents, and registry generation.
      - `docs/reference/clay-js-api/schema.md`: Authoritative Markdown/frontmatter schema for Clay JS API docs, including required metadata, user-facing names, key bindings, custom properties, body sections, parser expectations, and a complete example.
      - `docs/reference/clay-js-api/configuration.md`: Initial configuration model documentation for `~/.config/clay/init.js` and configuration options as Clay JS APIs.
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`: Mark this task complete.
    - Deferred to later tasks:
      - `src/docs.rs` or `src/docs/mod.rs`: Markdown parser, generated registry types, and validation helpers.
      - `src/lib.rs` if the project is split into a library for shared registry/tests.
      - `docs/reference/clay-js-api/**/*.md`: Per-API documentation files authored by the next documentation task.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `roadmap.md` Phase 3.
  - Test Cases to Write:
    - `markdown_docs_reject_empty_required_fields`: Validation fails when required Markdown/frontmatter fields are empty.
    - `markdown_doc_entry_ids_are_unique`: Duplicate stable IDs are rejected.
    - `master_index_includes_all_public_doc_files`: Clay JS API Markdown files must be linked from `docs/index.md`.
    - `doc_registry_entries_have_lookup_metadata`: Every generated registry entry has tags and app/help visibility metadata.
    - `markdown_docs_include_discovery_and_customization_metadata`: Every Clay JS API doc includes user-facing name, key binding metadata, and custom properties metadata.

- [x] Define Clay JS API naming convention as a project pattern
  - Acceptance Criteria:
    - Functional: Before any new Clay JS API facade is written, the project has an explicit naming convention for Clay JS APIs under `.agents/skills/project-patterns/references/` that balances maximum clarity with minimal redundant typing. The convention distinguishes callable JavaScript names, import/module names, registry stable IDs, and English `user_facing_name` metadata. It rejects redundant user-call patterns such as `clay.editor.*` when the surrounding Clay context already provides that meaning, defines flat logical naming for editor-core APIs that wrap Rust behavior, requires server/client authority segregation to be visible in editor-core API names, and requires pure-JS package APIs to start with the package name so users and AI agents can identify the source package.
    - Performance: Naming convention work is documentation/pattern-only and adds no runtime work, JavaScript execution, IPC, registry generation, or editor input/render cost.
    - Code Quality: The convention is concise, example-driven, and reusable by humans and AI agents through the existing `project-patterns` skill; the project-patterns skill description/routes are reviewed against all existing `references/` patterns and updated so Clay JS API naming work reliably selects the new pattern without weakening routing for the existing patterns. The convention prevents implementation-focused names, raw op names, and repetitive namespace words from leaking into user-facing JavaScript facades.
    - Security: The convention preserves authority-boundary clarity by making server/client ownership explicit for editor-core APIs and by not disguising authority-bearing package APIs behind generic names.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/SKILL.md`: Existing pattern router; updated so Clay JS API naming/design/refactor work and package distribution planning are routed through relevant project patterns explicitly.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: The public programmatic surface is the Clay JS/TS facade, not raw Rust functions or raw Deno ops.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Clay JS API docs already include English `user_facing_name`, lookup metadata, key binding metadata, and custom properties.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown docs and generated lookup must make Clay JS APIs discoverable by users and AI agents.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Public programmatic behavior must be exposed and documented through Clay JS APIs while preserving authority and performance boundaries.
      - `docs/reference/clay-js-api/schema.md`: Existing schema examples and naming fields updated to distinguish callable exports from stable IDs and `user_facing_name` metadata.
      - `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`: Approved naming convention and npm-compatible package distribution direction.
    - Options Considered:
      - Create a separate `write-clay-api` skill: focused, but unnecessary because this is stable project guidance that belongs with existing project patterns.
      - Keep naming rules only in the plan: easy for this phase, but future API work and agents may miss the convention.
      - Add a dedicated project pattern and update the `project-patterns` skill routing description: chosen because API naming is reusable project policy and should be selected with other Clay planning/API patterns.
    - Chosen Approach:
      - Added `.agents/skills/project-patterns/references/clay-js-api-naming.md` as the reusable naming pattern. Reviewed the current `project-patterns` routing against existing reference patterns, then updated `.agents/skills/project-patterns/SKILL.md` so Clay JS API naming/design/refactor work loads the naming pattern alongside Clay JS API boundary/schema/documentation patterns.
      - Accepted convention captured in the pattern:
        - Distinguish JS module specifiers, JS callable/export names, stable registry IDs, and English `user_facing_name` metadata.
        - Keep user-callable editor-core APIs flat and logically named around behavior, not Rust modules or op wrappers.
        - Do not repeat `clay`, broad module/domain names, or implementation categories in callable names when the import/runtime context already establishes Clay.
        - Make server/client segregation explicit in Clay-owned editor-core callable names, for example `serverInsertText(...)` for authoritative document mutation and `clientSetCursorStyle(...)` for local UI behavior.
        - Use English `user_facing_name` metadata for natural-language search/help, so callable names can stay concise and do not need to read like full sentences.
        - For pure-JS package APIs, begin the callable/export name with the package name or registered package prefix so provenance is obvious, for example `vimEnableMode(...)`.
        - Keep raw `op_*`, Rust module names, and generated registry IDs out of user-facing callable names unless a reviewed exception is documented.
    - API Notes and Examples:
      ```ts
      // Editor-core facade examples to validate in the pattern before implementation:
      await serverInsertText({ documentId, offset, text: "hello" });
      clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });

      // Pure-JS package facade example pattern:
      vimEnableMode({ mode: "normal" });
      ```
    - Files to Create/Edit:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: New project pattern defining Clay JS API naming, authority/source naming rules, examples, exceptions, and iterative update workflow.
      - `.agents/skills/project-patterns/references/package-distribution.md`: New project pattern recording the approved npm/pnpm package distribution direction discovered during naming/package provenance review.
      - `.agents/skills/project-patterns/SKILL.md`: Updated description/routing text so Clay JS API naming/design work selects relevant project patterns.
      - `docs/reference/clay-js-api/schema.md`: Updated naming guidance and examples to distinguish callable names from `user_facing_name` and stable registry IDs.
      - `docs/reference/clay-js-api/configuration.md`: Aligned configuration examples with the accepted naming convention.
      - `docs/index.md`: Aligned the future example API entry with the accepted naming convention.
      - `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`: Logged the approved decision.
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`: Marked this task complete after the pattern, routing description, docs, and decision log were updated.
    - References:
      - `.agents/skills/project-patterns/SKILL.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`
  - Test Cases to Write:
    - Manual routing review: Confirm `project-patterns` skill guidance still routes plan updates through all relevant existing reference patterns and makes Clay JS API naming/design/refactor tasks load the new naming pattern along with boundary/schema patterns.
    - Manual naming review: Confirm no new Clay JS API facade or per-API Markdown doc is added before the naming pattern exists and the schema examples follow it.
    - `clay_js_api_names_follow_convention`: Coverage gate or parser validation rejects documented callable names that repeat banned boilerplate, omit required server/client segregation for editor-core APIs, or omit the package prefix for pure-JS package APIs.
    - `clay_js_api_user_facing_name_is_not_used_as_callable_name`: Validation confirms English help/search names remain metadata and do not force verbose JavaScript callable names.

- [ ] Author Markdown docs for current Clay JS API concepts and editor-facing capabilities
  - Acceptance Criteria:
    - Functional: Existing editor-facing programmatic concepts and planned Clay JS APIs for editor capabilities such as text insertion, newline, Backspace/Delete, cursor movement, selection, scrolling, resize behavior, cursor style, key binding configuration, and Escape exit are documented in Markdown files linked from the master Markdown index where they are intended to become programmatic JS APIs.
    - Performance: Documentation authoring and Markdown parsing do not require inspecting editor buffers, layout caches, or runtime UI state.
    - Code Quality: Documentation files use stable public interface IDs close enough to command definitions/tests that future command changes are unlikely to forget documentation updates.
    - Security: Entries explicitly describe that current editor commands mutate only inert local text/UI state and do not introduce file IO, IPC, network, script execution, AI mutation, or extension authority.
  - Approach:
    - Documentation Reviewed:
      - `plans/003-Phase2-EditorInteractionModel.md`: Defines current editor behavior and completed interaction model.
      - `src/editor.rs`, `src/editor/surface.rs`, and `src/masonry_editor.rs`: Current command and event routing boundaries.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Client owns rendering/input and transient local state.
    - Options Considered:
      - Document only modules: insufficient for user/agent discovery of actual editor capabilities.
      - Document every private helper: too noisy and likely to slow development.
      - Document Clay JS API concepts for command and capability surfaces first in Markdown: matches the self-documenting requirement while keeping the phase small.
    - Chosen Approach:
      - Add Markdown files for Clay JS API concepts around the editor command/capability layer, not every private helper. Keep entries grouped by stable IDs such as `clay.editor.serverInsertText` and `clay.editor.clientDragSelection`, and include them in `docs/index.md` so generated registries can discover them.
    - API Notes and Examples:
      ```markdown
      - [serverInsertText](reference/clay-js-api/editor/server-insert-text.md) — `clay.editor.serverInsertText`
      ```
    - Files to Create/Edit:
      - `docs/index.md`: Link Clay JS editor API documentation.
      - `docs/reference/clay-js-api/editor/*.md`: Authoritative Markdown docs for Clay JS editor APIs and planned capabilities.
      - `src/editor/surface.rs`: Add references or tests connecting editor capabilities to Clay JS API docs if practical.
      - `src/masonry_editor.rs`: Add references or tests for UI actions like Escape exit if practical.
    - References:
      - `plans/003-Phase2-EditorInteractionModel.md`
      - `src/editor/surface.rs`
      - `src/masonry_editor.rs`
  - Test Cases to Write:
    - `all_clay_js_editor_api_concepts_have_markdown_docs`: Every editor capability intended as a Clay JS API has a linked Markdown doc entry.
    - `editor_markdown_docs_state_security_scope`: Editor command docs mention local-only/no external authority constraints.

- [ ] Generate and update the documentation registry from Markdown
  - Acceptance Criteria:
    - Functional: A generation path reads `docs/index.md` and linked Clay JS API Markdown files, validates required metadata, and produces a generated registry artifact for programmatic/app/agent lookup.
    - Performance: Generation is deterministic and suitable for tests or a developer command; generated registry size remains proportional to indexed Markdown docs, and lookup indexes are precomputed rather than built in editor paint/input paths.
    - Code Quality: Generated registry files have stable ordering to avoid noisy diffs, are not hand-edited as the source of truth, and use stable IDs so users and agents can link to exact public surfaces.
    - Security: Registry generation does not include secrets, environment data, local paths beyond project-relative references, or runtime user content.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Clay JS API Markdown is authoritative; generated registries and lookup APIs are derived from the master Markdown index.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Tests must detect stale generated registry artifacts without mutating them.
      - `roadmap.md`: AI agents must inspect app capabilities from structured documentation.
      - `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`: Records Markdown-authoritative registry generation.
    - Options Considered:
      - Generate Markdown from a code registry: good for app lookup, but makes Markdown secondary and risks human-doc drift.
      - Use Markdown only with no generated registry: good for humans, but less reliable for app/agent/tool lookup.
      - Generate a registry from indexed Markdown: best satisfies the strict self-documenting requirement while keeping Markdown authoritative.
    - Chosen Approach:
      - Add deterministic functions for checking/generating the registry from `docs/index.md` and linked Markdown files. Provide one non-mutating function used by tests and one update function/command that rewrites the checked-in generated registry when the Markdown source changes.
    - API Notes and Examples:
      ```bash
      cargo test generated_doc_registry_is_current
      cargo run --bin update-doc-registry
      ```
      ```text
      docs/index.md
      docs/reference/clay-js-api/editor/server-insert-text.md
      generated/doc-registry.txt
      ```
    - Files to Create/Edit:
      - `src/docs.rs` or `src/docs/generate.rs`: Markdown parsing, registry generation, and registry update helpers.
      - `src/bin/update-doc-registry.rs` or equivalent developer command: Writes the checked-in generated registry from Markdown.
      - `generated/doc-registry.txt` or equivalent generated artifact: Checked-in app/agent lookup registry derived from Clay JS API Markdown.
      - `tests/` or module tests: Current-output validation.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `roadmap.md` Phase 3.
  - Test Cases to Write:
    - `doc_registry_generates_from_master_markdown_index`: Generator reads `docs/index.md` and linked Markdown docs.
    - `doc_registry_generation_is_stable`: Generated registry output order is deterministic.
    - `doc_registry_supports_lookup_keys`: Generated registry includes stable ID, kind, owner, tags, summary, and security notes.
    - `generated_doc_registry_is_current`: Checked-in generated registry matches output generated from Markdown; on mismatch, the failure instructs the user to run `cargo run --bin update-doc-registry`.

- [ ] Expose programmatic and app-facing documentation lookup
  - Acceptance Criteria:
    - Functional: The generated registry exposes lookup helpers that can resolve Clay JS API documentation by stable ID and list documentation by kind, owner, JS module/export, backing Rust path, op name, user-facing name, key binding, custom property, and tags for future in-app help, command palette, extension tooling, configuration UI, and AI tool discovery.
    - Performance: Lookup uses static/precomputed registry data and does not scan source Markdown or rebuild indexes during editor input/rendering.
    - Code Quality: Lookup APIs return structured documentation entries or lightweight views generated from Markdown instead of requiring app UI and agents to parse Markdown at runtime.
    - Security: Lookup exposes only public documentation metadata and does not reveal runtime user content, local environment values, secrets, or private implementation-only APIs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Users and AI agents must be able to inspect Clay JS API capabilities from Markdown, the generated registry, and the app.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic lookup should resolve Clay JS facade APIs, not raw ops.
      - `roadmap.md`: User-facing help, extension author docs, and agent tool descriptions should come from the same Markdown-derived registry to prevent drift.
    - Options Considered:
      - Require users and agents to read Markdown directly: simple, but not enough for app UI, command palette search, or reliable tool lookup.
      - Require app/tooling to parse Markdown at runtime: keeps one source of truth, but adds unnecessary runtime parsing and error handling.
      - Expose lookup helpers over the generated registry derived from Markdown: keeps Markdown authoritative while avoiding runtime scraping/parsing.
    - Chosen Approach:
      - Add `get_doc_entry`, `entries_by_kind`, `entries_by_owner`, and `entries_by_tag`-style helpers over the generated registry. Future protocol/server phases can expose these through IPC or SDUI later, but Phase 3 only needs local lookup APIs and generated artifacts.
    - API Notes and Examples:
      ```rust
      pub fn get_doc_entry(id: &str) -> Option<&'static DocEntry>;
      pub fn entries_by_kind(kind: DocSurfaceKind) -> impl Iterator<Item = &'static DocEntry>;
      pub fn entries_by_tag(tag: &str) -> impl Iterator<Item = &'static DocEntry>;
      pub fn entry_by_js_export(module: &str, export: &str) -> Option<&'static DocEntry>;
      pub fn entry_by_backing_rust_path(path: &str) -> Option<&'static DocEntry>;
      ```
    - Files to Create/Edit:
      - `src/docs.rs` or `src/docs/lookup.rs`: Structured lookup helpers over the generated registry.
      - `tests/docs_contract.rs` or module tests: Lookup behavior and generated registry consistency tests.
      - `docs/documentation-workflow.md`: Explain Markdown authoring versus generated registry and app/programmatic lookup.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `roadmap.md` Documentation as Code Requirement.
  - Test Cases to Write:
    - `doc_lookup_finds_entry_by_stable_id`: Lookup resolves known public interface IDs.
    - `doc_lookup_lists_entries_by_kind_owner_and_tag`: Lookup supports app/help and agent discovery paths.
    - `doc_lookup_finds_entries_by_user_facing_name_key_binding_and_custom_property`: Lookup supports user search, key binding discovery, and configuration/customization discovery.
    - `doc_lookup_matches_generated_registry`: Programmatic lookup keys match the generated registry artifact.

- [ ] Add documentation coverage gates for server-side Rust public functions and Clay JS APIs
  - Acceptance Criteria:
    - Functional: `cargo test` fails when any server-side Rust public function lacks a corresponding Clay JS API, when a Clay JS API lacks required Markdown documentation, when a Clay JS API Markdown doc is missing from the master index, when a generated registry entry is malformed, when a documented API has undocumented options/configuration, when user-facing name/key binding/custom property metadata is missing or malformed, or when the generated registry is stale.
    - Performance: Coverage tests run as normal unit/integration tests without launching the GUI or doing expensive filesystem scans beyond generated-doc comparison.
    - Code Quality: Coverage rules are explicit and easy to extend for Phase 4 protocol messages, behavior manifest entries, permissions, extension APIs, and AI tools; the tests name the missing public interface IDs so humans and agents know what documentation to add.
    - Security: Coverage gates prevent undocumented authority-bearing surfaces from being introduced silently.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Public programmatic behavior must be exposed and documented through Clay JS APIs.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Server-side Rust public functions must have Clay JS facades or become non-public.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Cargo tests should fail for missing Clay JS APIs, docs, index links, stale registries, or lookup gaps.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Phase 4 protocol surfaces should include final-compatible metadata and tests.
      - `roadmap.md`: Documentation coverage gates are required as Clay hardens.
    - Options Considered:
      - Manual review only: too easy to miss, especially for AI-generated changes.
      - Allow server-side Rust public functions to remain undocumented internals: rejected by the Clay JS API decision; implementation details should be private or `pub(crate)` instead.
      - Targeted coverage gates for server-side Rust public functions, Clay JS APIs, Markdown docs, master-index inclusion, and generated registry freshness: focused and expandable while still failing when public programmatic surfaces are missing docs.
    - Chosen Approach:
      - Start with explicit lists/mappings for server-side Rust public functions, Clay JS facade exports, Markdown docs, master-index inclusion, and generated registry freshness. Add extension points so Phase 4 can require protocol and behavior-manifest JS APIs by writing coverage tests next to the server/protocol modules. The required pattern is: server Rust public function inventory -> Clay JS API -> Markdown doc -> `docs/index.md` link -> generated registry entry -> lookup API -> tests that fail if any link is missing. `cargo test` must run the non-mutating registry generation/check path; a separate update command writes regenerated artifacts.
    - API Notes and Examples:
      ```rust
      #[test]
      fn generated_doc_registry_is_current() {
          clay::docs::assert_generated_registry_is_current().unwrap();
      }
      ```
    - Files to Create/Edit:
      - `src/docs.rs`: Markdown validation, registry generation, stale-check, and lookup APIs.
      - `src/editor/surface.rs` or tests: Editor command coverage mapping.
      - `tests/docs_contract.rs` if integration tests are preferred after adding `src/lib.rs`.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `all_markdown_documented_surfaces_are_valid`: Markdown/frontmatter validation succeeds for current docs.
    - `server_public_rust_functions_have_clay_js_api`: Missing Clay JS API fails the coverage test.
    - `generated_doc_registry_is_current`: Generated registry matches checked-in artifact and tells the user to run `cargo run --bin update-doc-registry` when stale.
    - `clay_js_api_without_markdown_doc_fails`: Adding a Clay JS API without a matching Markdown doc fails a targeted test.
    - `markdown_doc_missing_from_master_index_fails`: Public documentation files must be linked from `docs/index.md`.
    - `doc_entry_without_generated_lookup_fails`: Clay JS API Markdown docs that do not appear in the generated registry and lookup API fail tests.
    - `clay_js_api_discovery_metadata_missing_fails`: Missing user-facing name, key binding metadata, or custom property metadata fails validation.

- [ ] Document the Clay JS API documentation workflow for future phases
  - Acceptance Criteria:
    - Functional: Future contributors and agents can find clear instructions for adding documented Clay JS APIs for protocol messages, commands, behavior manifest entries, permissions, extension APIs, and AI tools, including how to author Markdown docs, add them to the master index, update the generated registry, and verify app/programmatic lookup results.
    - Performance: The workflow does not require running expensive generation steps during normal editing; verification remains part of normal Cargo checks/tests.
    - Code Quality: The workflow points to Markdown plus `docs/index.md` as the source of truth and discourages hand-editing generated registry outputs.
    - Security: The workflow requires security/authority notes for public surfaces and explicitly calls out permission-bearing APIs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/SKILL.md`: Plans must include documentation/manifest/protocol coverage when new public surfaces are added.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Planning guidance for Markdown-authoritative documentation, registry generation, tests, and agent discovery.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Public programmatic APIs are Clay JS facades backed by explicit ops.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Tests cover registry freshness and lookup access.
      - `roadmap.md`: Later phases add IPC, behavior manifests, file/workspace server, SDUI, JavaScript extensions, hot reload, and AI tools.
    - Options Considered:
      - Keep workflow only in the skill: useful for agents, but users/contributors need project-local docs too.
      - Keep workflow only in generated registry output: generated output should not be the source of authoring rules.
      - Add a concise source-controlled workflow document plus skill references: readable by users and discoverable by agents, and explicit about Markdown authoring, master index inclusion, generated registry updates, and app-lookup documentation paths.
    - Chosen Approach:
      - Add a short authoring guide under `docs/` that explains Markdown source files, `docs/index.md`, registry generation/update commands, lookup API, tests, and rules for adding new public surfaces in later phases.
    - API Notes and Examples:
      ```bash
      cargo test generated_doc_registry_is_current
      cargo run --bin update-doc-registry
      cargo fmt
      cargo check
      ```
    - Files to Create/Edit:
      - `docs/documentation-workflow.md`: Authoring workflow and rules.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Update only if implementation discovers a reusable pattern change.
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md`: No required edit unless Phase 3 changes the documentation workflow expected by Phase 4.
    - References:
      - `.agents/skills/project-patterns/SKILL.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `roadmap.md` Phases 4-13.
  - Test Cases to Write:
    - `documentation_workflow_mentions_required_clay_js_api_surfaces`: Guide includes Clay JS API documentation requirements for protocol, command, behavior manifest, permission, extension API, AI tool, and SDUI surfaces.
    - `documentation_workflow_explains_markdown_index_registry_and_lookup`: Guide explains Markdown authoring, master-index inclusion, generated registry updates, app/programmatic lookup, and failing tests for missing docs.
    - Manual documentation review: Confirm a future agent can identify where to add docs for a new Phase 4 protocol message.


- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Phase 3 defines the configuration model around `~/.config/clay/init.js`, documents that `init.js` may load modular local configuration files, and treats every configuration option as a documented Clay JS API with user-facing name, key binding metadata, custom properties, Markdown docs, master-index inclusion, generated registry coverage, and lookup access.
    - Performance: Configuration documentation and registry generation remain offline/test-time work; no configuration loading or JavaScript execution is added to the editor paint/input path in Phase 3.
    - Code Quality: Configuration APIs reuse the Clay JS API schema rather than creating a separate undocumented configuration key registry.
    - Security: Configuration does not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority; permission-bearing configuration APIs require explicit docs and future server-side validation.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`: Approved `~/.config/clay/init.js` configuration entry point and configuration-as-Clay-JS-API model.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration options are Clay JS APIs and must be documented through the Markdown registry contract.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: APIs must include user-facing name, key bindings, and custom properties.
      - `docs/reference/clay-js-api/configuration.md`: Initial configuration model documentation.
    - Options Considered:
      - Separate configuration registry: rejected because it would drift from Clay JS API docs and generated lookup.
      - Undocumented configuration keys in `init.js`: rejected because configuration is a public user/agent surface.
      - Configuration options as documented Clay JS APIs: chosen to keep customization, key binding, help, and AI-agent discovery on one contract.
    - Chosen Approach:
      - In Phase 3, document and validate the configuration contract without implementing full runtime configuration loading. Later implementation phases should add `init.js` loading and modular imports while preserving the Markdown-derived registry and lookup requirements.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { bindKey } from "clay:keybindings";
      import { clientSetCursorStyle } from "clay:editor";

      bindKey("Ctrl+I", "clay.editor.serverInsertText");
      clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Configuration model documentation.
      - `docs/reference/clay-js-api/**/*.md`: Configuration API docs as APIs are introduced.
      - `docs/index.md`: Link configuration docs and future configuration API docs.
      - `src/docs.rs` or tests: Validate configuration API metadata when registry parsing is implemented.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - `configuration_docs_describe_init_js_entry_point`: Documentation mentions `~/.config/clay/init.js` and modular configuration loading.
    - `configuration_options_are_documented_as_clay_js_apis`: Configuration APIs must have Markdown docs, master-index links, generated registry entries, and lookup access.
    - `configuration_api_custom_properties_are_required`: Behavior-changing configuration options must appear in `custom_properties`.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: The Phase 3 implementation is reviewed and the Clay JS APIs needed for extensibility, configuration, customization, user search/help, key binding, AI-agent discovery, and future public programmatic use are proposed or created. All server-side Rust public functions introduced or existing in this phase are inventoried; each public programmatic capability has a stable Clay JS API facade backed by an explicit `deno_core` op wrapper. Functions that should not have JavaScript exposure are made private or `pub(crate)` instead of public.
    - Performance: Clay JS API facade setup does not add work to the client editor input/render path and keeps JS execution server-side.
    - Code Quality: The implementation separates Rust implementation functions, op wrappers, and JS/TS facade exports with stable names suitable for documentation and versioning.
    - Security: Arbitrary Rust public functions and client-side Rust functions are not exposed directly to JavaScript; authority checks happen at the server/API boundary.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`: Approved Clay JS API facade boundary.
      - `.agents/skills/create-plan/references/clay.md`: Clay plans require a JS API task for public programmatic surfaces and Rust public functions.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Clay JS APIs are the documentation-as-code public programmatic surface.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Clay JS APIs include user-facing names, key bindings, and custom properties.
      - Context7 `/denoland/deno_core` docs: Rust functions are exposed through extensions and ops.
    - Options Considered:
      - Expose Rust public functions directly: rejected due to unsafe coupling and accidental authority exposure.
      - Make raw ops the public API: rejected because raw ops are implementation details.
      - Stable Clay JS facade over explicit ops: chosen for versioning, docs, and authority boundaries.
    - Chosen Approach:
      - Review Phase 3 implementation surfaces, propose needed Clay JS APIs for extensibility/configuration/customization, add an inventory/check path for server-side Rust public functions, and create Clay JS facade exports for each public programmatic capability. In Phase 3, this may establish the structure and tests before Phase 4 server implementation adds real authority-bearing APIs.
    - API Notes and Examples:
      ```rust
      #[deno_core::op2]
      #[string]
      fn op_clay_text_normalize_line_endings(#[string] input: String) -> String {
          clay::text::normalize_line_endings(&input)
      }
      ```
      ```ts
      import { normalizeLineEndings } from "clay:text";

      const normalized = normalizeLineEndings("a\r\nb");
      ```
    - Files to Create/Edit:
      - `src/docs.rs` or `src/docs/coverage.rs`: Server public Rust function to Clay JS API coverage checks.
      - `src/server/js/**` or equivalent future server JS-op boundary files if introduced in this phase.
      - `runtime/js/**` or equivalent Clay JS facade files if introduced in this phase.
      - `docs/reference/clay-js-api/**/*.md`: Clay JS API docs for exposed functions, including user-facing names, key bindings, and custom properties.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - `server_public_rust_functions_have_clay_js_api`: Fails when a server-side Rust public function lacks a Clay JS API.
    - `clay_js_api_has_backing_op_and_rust_function`: Fails when a Clay JS API doc/facade lacks backing Rust/op metadata.

- [ ] Create the project code wiki structure
  - Acceptance Criteria:
    - Functional: A Markdown code wiki exists with `docs/wiki/index.md` as the master index and an initial navigable structure for architecture, modules, flows, and concepts.
    - Performance: Wiki creation adds no runtime application work and is maintained as source-controlled Markdown.
    - Code Quality: The wiki structure follows `.agents/skills/project-wiki/SKILL.md`, links pages from the master index, and distinguishes code-wiki implementation education from public Clay JS API registry docs.
    - Security: Wiki pages do not include secrets, local user data, or sensitive environment values; security boundaries are documented where relevant.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Code wiki scope, master index, public-reference linking boundary, and quality bar.
      - `.agents/skills/project-wiki/references/page-template.md`: Default template for substantial wiki pages.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Internal Rust implementation details belong in the project code wiki unless exposed through Clay JS APIs.
    - Options Considered:
      - Put implementation education into `docs/index.md`: rejected because public API registry docs and internal code wiki have different audiences and metadata needs.
      - Create a separate `docs/wiki/` tree with its own master index: chosen to keep implementation education navigable and separate from generated public API registry docs.
    - Chosen Approach:
      - Create `docs/wiki/index.md` and initial pages such as architecture, editor modules, rendering/layout flow, input flow, and documentation/JS API boundaries.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/architecture.md
      docs/wiki/modules/editor.md
      docs/wiki/flows/input-to-edit.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Master code wiki index.
      - `docs/wiki/architecture.md`: Initial architecture overview.
      - `docs/wiki/modules/*.md`: Module implementation pages.
      - `docs/wiki/flows/*.md`: Flow implementation pages.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki structure review: Confirm every created wiki page is linked from `docs/wiki/index.md` and explains what code it covers.

- [ ] Update or verify the code wiki after Phase 3 implementation
  - Acceptance Criteria:
    - Functional: After Phase 3 implementation and tests pass, existing Clay code and Phase 3 documentation/registry/Clay JS API machinery are documented or verified in the code wiki deeply enough that a developer unfamiliar with the project can learn what the implementation does and how it works by reading the wiki.
    - Performance: Performance-sensitive implementation details, especially bounded rendering/layout, local editor hot paths, and no synchronous JS/client round trips, are explicitly documented.
    - Code Quality: Wiki pages include source paths, test paths, implementation flow, examples where useful, invariants/tradeoffs, and links to related pages.
    - Security: Wiki pages document local-only editor behavior and current absence of filesystem, network, IPC, extension, and script authority where relevant.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Required wiki content, public-reference linking boundary, and quality bar.
      - `.agents/skills/project-wiki/references/page-template.md`: Default page template for substantial wiki pages.
      - `plans/001-003`: Existing implementation history and accepted compromises/follow-up work.
      - `src/editor.rs`, `src/editor/**`, `src/masonry_editor.rs`, and `src/main.rs`: Existing code to document.
    - Options Considered:
      - Document only public APIs: rejected because the code wiki must teach internal implementation.
      - Document every trivial helper separately: rejected because it is noisy and hard to maintain.
      - Document implementation units and flows at educational depth: chosen to make the wiki useful for onboarding and AI agents.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once. Write or verify pages for the current editor buffer/cursor/selection/layout/viewport/surface/widget/main flow and the new Phase 3 docs/registry/Clay JS API boundary, including what each unit does, how it works, important invariants, examples, and tests. Link public programmatic API usage to `docs/reference/` instead of duplicating it.
    - API Notes and Examples:
      ```markdown
      ## Source

      - `src/editor/buffer.rs`
      - `src/editor/cursor.rs`

      ## How It Works

      Explain byte-offset edits over `crop::Rope` and cursor boundary invariants.
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/editor-buffer.md`: Buffer implementation details.
      - `docs/wiki/modules/editor-cursor-selection.md`: Cursor and selection implementation details.
      - `docs/wiki/modules/editor-layout-viewport.md`: Layout/cache/viewport implementation details.
      - `docs/wiki/flows/editor-input-to-render.md`: Input-to-edit-to-render flow.
      - `docs/wiki/concepts/documentation-registry.md`: Phase 3 Markdown-to-registry implementation and Clay JS API boundary.
      - `docs/wiki/index.md`: Link all implementation pages.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `plans/003-Phase2-EditorInteractionModel.md`
  - Test Cases to Write:
    - Manual wiki completeness review: Confirm existing source modules have corresponding wiki coverage or an explicit reason for omission.

- [ ] Run verification and update the plan status
  - Acceptance Criteria:
    - Functional: Clay JS API Markdown documentation, master index, generated registry, app/programmatic lookup, configuration API contract, public Rust-to-JS API coverage, code wiki, and coverage tests are implemented and pass.
    - Performance: Verification does not require launching the GUI or server and remains fast enough for normal `cargo test` use.
    - Code Quality: `cargo fmt`, `cargo test`, and `cargo check` pass; generated registry files are stable across repeated runs.
    - Security: No new runtime authority is introduced; docs do not leak secrets or inspect local user content.
  - Approach:
    - Documentation Reviewed:
      - Rust/Cargo standard workflow used by prior Clay phases: `cargo fmt`, `cargo test`, and `cargo check`.
      - `plans/001-003`: Prior plans require preserving local editor behavior and Cargo verification.
    - Options Considered:
      - Verify only documentation output: insufficient because code changes add registry and tests.
      - Run full Cargo verification: preserves confidence before Phase 4 IPC work begins.
    - Chosen Approach:
      - Run the standard verification commands, update completed task checkboxes only after tests pass, and fill compromises/further actions after implementation.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test
      cargo check
      ```
    - Files to Create/Edit:
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`: Mark tasks complete and record compromises/further actions after implementation.
      - Any registry, docs, or test files added by earlier tasks.
    - References:
      - `plans/001-Phase0-NativeTextCanvas.md`
      - `plans/002-Phase1-TextCanvasFoundation.md`
      - `plans/003-Phase2-EditorInteractionModel.md`
  - Test Cases to Write:
    - `phase3_documentation_contract_verification`: `cargo fmt`, `cargo test`, and `cargo check` all pass.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
