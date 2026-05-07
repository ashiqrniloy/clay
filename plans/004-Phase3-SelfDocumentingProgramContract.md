# Phase 3 Self-Documenting Program Contract

## Objectives
- Make Clay documentation part of the code contract before the IPC, protocol, behavior manifest, command, extension, and AI-tool surfaces grow.
- Create a small documentation registry that can produce both human-readable Markdown and machine-readable agent references from the same source of truth.
- Add validation so new public Clay surfaces cannot be added without inspectable documentation.
- Document the existing editor-facing public concepts enough that future Phase 4 protocol work has a pattern to follow.
- Keep the system local, build/test driven, and independent of server, extension, AI, file IO, remote, or network authority.

## Expected Outcome
- The repository contains a focused documentation registry module or structured registry data for public Clay surfaces.
- Human-readable generated documentation exists under a project documentation/reference directory.
- Machine-readable agent documentation exists in a predictable file that AI agents can inspect without guessing from source.
- Tests fail when registered public surfaces are missing required documentation fields.
- Current editor commands and user-facing editor behavior are documented through the registry.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- Phase 4 IPC/protocol work can add protocol messages and behavior manifest entries with documentation requirements already established.

## Tasks

- [ ] Define the documentation registry scope and schema
  - Acceptance Criteria:
    - Functional: The project has a documented schema for registered public surfaces, including at least surface kind, stable ID, name, summary, detailed description, owner component, phase, visibility, security notes, and agent guidance.
    - Performance: Registry lookup/generation is offline or test-time work and does not add runtime cost to the editor paint/input path.
    - Code Quality: The schema is small, typed where practical, and avoids a sprawling documentation framework before Clay has public extension APIs.
    - Security: The schema can record permissions/security notes but does not grant permissions, execute scripts, load extensions, or expose filesystem/network authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`: Documentation must be part of the code contract and inspectable by users and AI agents.
      - `.agents/skills/clay-patterns/references/planning-checklist.md`: New public surfaces require a registry/documentation path and tests or acceptance criteria.
      - `roadmap.md`: Phase 3 requires a self-documenting program contract before IPC and extension surfaces grow.
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`: Future protocol, command, manifest, permission, and AI tool surfaces must stay inspectable.
    - Options Considered:
      - Free-form Markdown only: easy to write, but it drifts from code and is hard for agents to query reliably.
      - Rust-only doc comments: close to code, but not enough for command/permission/manifest registries and agent-readable indexes.
      - Small typed registry plus generated Markdown/agent index: slightly more upfront work, but creates one source of truth for humans, users, and AI agents.
    - Chosen Approach:
      - Define a minimal `DocEntry`-style schema in Rust or structured data with explicit owner and security metadata. Start with enough fields to document current editor commands and future protocol/behavior entries without introducing macros unless duplication becomes painful.
    - API Notes and Examples:
      ```rust
      pub enum DocSurfaceKind {
          EditorCommand,
          ProtocolMessage,
          BehaviorManifestEntry,
          Permission,
          ExtensionApi,
          AiTool,
      }

      pub struct DocEntry {
          pub id: &'static str,
          pub kind: DocSurfaceKind,
          pub name: &'static str,
          pub summary: &'static str,
          pub details: &'static str,
          pub owner: &'static str,
          pub phase: &'static str,
          pub security: &'static str,
          pub agent_guidance: &'static str,
      }
      ```
    - Files to Create/Edit:
      - `src/docs.rs` or `src/docs/mod.rs`: Registry types and validation helpers.
      - `src/lib.rs` if the project is split into a library for shared registry/tests.
      - `plans/004-Phase3-SelfDocumentingProgramContract.md`: Track this plan.
    - References:
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`
      - `.agents/skills/clay-patterns/references/authority-boundaries.md`
      - `roadmap.md` Phase 3.
  - Test Cases to Write:
    - `doc_registry_rejects_empty_required_fields`: Validation fails when required fields are empty.
    - `doc_registry_entry_ids_are_unique`: Duplicate stable IDs are rejected.

- [ ] Register current editor commands and user-facing editor capabilities
  - Acceptance Criteria:
    - Functional: Existing user-facing editor commands/capabilities such as text insertion, newline, Backspace/Delete, cursor movement, Home/End, selection, drag selection, scrolling, resize behavior, and Escape exit are represented in the documentation registry.
    - Performance: Registration is static data or cheap construction and does not require inspecting editor buffers, layout caches, or runtime UI state.
    - Code Quality: Documentation entries are close enough to command definitions that future command changes are unlikely to forget documentation updates.
    - Security: Entries explicitly describe that current editor commands mutate only inert local text/UI state and do not introduce file IO, IPC, network, script execution, AI mutation, or extension authority.
  - Approach:
    - Documentation Reviewed:
      - `plans/003-Phase2-EditorInteractionModel.md`: Defines current editor behavior and completed interaction model.
      - `src/editor.rs`, `src/editor/surface.rs`, and `src/masonry_editor.rs`: Current command and event routing boundaries.
      - `.agents/skills/clay-patterns/references/authority-boundaries.md`: Client owns rendering/input and transient local state.
    - Options Considered:
      - Document only modules: insufficient for user/agent discovery of actual editor capabilities.
      - Document every private helper: too noisy and likely to slow development.
      - Document public/user-facing command and capability surfaces first: matches the self-documenting requirement while keeping the phase small.
    - Chosen Approach:
      - Add registry entries for the user-facing editor command/capability layer, not every private helper. Keep entries grouped by stable IDs such as `editor.command.insert-text` and `editor.capability.drag-selection`.
    - API Notes and Examples:
      ```rust
      DocEntry {
          id: "editor.command.insert-text",
          kind: DocSurfaceKind::EditorCommand,
          name: "Insert text",
          summary: "Insert printable text at the caret or replace the active selection.",
          owner: "client",
          phase: "Phase 2",
          security: "Mutates local inert text only.",
          agent_guidance: "Use as the local editor precedent for Phase 4 edit-operation documentation.",
          details: "...",
      }
      ```
    - Files to Create/Edit:
      - `src/docs.rs` or `src/docs/editor.rs`: Editor command/capability documentation entries.
      - `src/editor/surface.rs`: Add references or tests connecting `EditorCommand` variants to docs if practical.
      - `src/masonry_editor.rs`: Add references or tests for UI actions like Escape exit if practical.
    - References:
      - `plans/003-Phase2-EditorInteractionModel.md`
      - `src/editor/surface.rs`
      - `src/masonry_editor.rs`
  - Test Cases to Write:
    - `all_editor_commands_have_doc_entries`: Every public `EditorCommand` variant has a registry entry.
    - `editor_doc_entries_state_security_scope`: Editor command docs mention local-only/no external authority constraints.

- [ ] Generate human-readable and agent-readable documentation artifacts
  - Acceptance Criteria:
    - Functional: A generation path produces human-readable Markdown and a machine-readable agent index from the same registry entries.
    - Performance: Generation is deterministic and suitable for tests or a developer command; generated output size remains proportional to registered surfaces.
    - Code Quality: Generated files have stable ordering to avoid noisy diffs and are not hand-edited as the source of truth.
    - Security: Generated docs do not include secrets, environment data, local paths beyond project-relative references, or runtime user content.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`: Human-readable docs and machine-readable agent indexes should share a source of truth.
      - `roadmap.md`: AI agents must inspect app capabilities from structured documentation.
    - Options Considered:
      - Generate only Markdown: good for users, but less reliable for agents and tools.
      - Generate only JSON/RON/TOML: good for agents, less readable for users.
      - Generate both from one registry: best satisfies the strict self-documenting requirement.
    - Chosen Approach:
      - Add a deterministic generation function or small binary/test helper that writes Markdown and a simple machine-readable format. Prefer no new dependency in Phase 3; if JSON serialization would require adding a crate, use a simple deterministic text format first or explicitly add a small dependency only if justified.
    - API Notes and Examples:
      ```bash
      cargo test doc_registry_generated_outputs_are_current
      ```
      ```text
      docs/reference/clay-reference.md
      docs/agent-index/clay-capabilities.txt
      ```
    - Files to Create/Edit:
      - `src/docs.rs` or `src/docs/generate.rs`: Rendering/generation helpers.
      - `docs/reference/clay-reference.md`: Generated or checked-in human reference.
      - `docs/agent-index/clay-capabilities.txt`: Generated or checked-in agent-readable index.
      - `tests/` or module tests: Current-output validation.
    - References:
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`
      - `roadmap.md` Phase 3.
  - Test Cases to Write:
    - `doc_registry_generates_markdown_in_stable_order`: Output order is deterministic.
    - `doc_registry_generates_agent_index`: Agent-readable output includes IDs, kinds, owner, summary, and security notes.
    - `generated_docs_are_current`: Checked-in generated docs match registry output.

- [ ] Add documentation coverage gates for new public surfaces
  - Acceptance Criteria:
    - Functional: Tests fail when a registered surface is malformed, a documented enum has undocumented variants, or generated docs are stale.
    - Performance: Coverage tests run as normal unit/integration tests without launching the GUI or doing expensive filesystem scans beyond generated-doc comparison.
    - Code Quality: Coverage rules are explicit and easy to extend for Phase 4 protocol messages, behavior manifest entries, permissions, extension APIs, and AI tools.
    - Security: Coverage gates prevent undocumented authority-bearing surfaces from being introduced silently.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-patterns/references/planning-checklist.md`: Tests or acceptance criteria are required for documentation paths.
      - `.agents/skills/clay-patterns/references/protocol-and-performance.md`: Phase 4 protocol surfaces should include final-compatible metadata and tests.
      - `roadmap.md`: Documentation coverage gates are required as Clay hardens.
    - Options Considered:
      - Manual review only: too easy to miss, especially for AI-generated changes.
      - Strict lint for all Rust public items immediately: too broad for this early codebase and may distract from product architecture.
      - Targeted coverage gates for registered public Clay surfaces: focused and expandable.
    - Chosen Approach:
      - Start with explicit lists/mappings for current editor commands and generated docs. Add extension points so Phase 4 can require protocol message documentation without redesigning the registry.
    - API Notes and Examples:
      ```rust
      #[test]
      fn all_documented_surfaces_are_valid() {
          clay::docs::validate_registry(clay::docs::registry()).unwrap();
      }
      ```
    - Files to Create/Edit:
      - `src/docs.rs`: Validation API.
      - `src/editor/surface.rs` or tests: Editor command coverage mapping.
      - `tests/docs_contract.rs` if integration tests are preferred after adding `src/lib.rs`.
    - References:
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`
      - `.agents/skills/clay-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `all_documented_surfaces_are_valid`: Registry validation succeeds for current docs.
    - `editor_command_doc_coverage_is_complete`: Missing command docs fail the coverage test.
    - `generated_docs_are_not_stale`: Generated docs match checked-in artifacts.

- [ ] Document the documentation workflow for future phases
  - Acceptance Criteria:
    - Functional: Future contributors and agents can find clear instructions for adding documented protocol messages, commands, behavior manifest entries, permissions, extension APIs, and AI tools.
    - Performance: The workflow does not require running expensive generation steps during normal editing; verification remains part of normal Cargo checks/tests.
    - Code Quality: The workflow points to the registry as source of truth and discourages hand-editing generated outputs.
    - Security: The workflow requires security/authority notes for public surfaces and explicitly calls out permission-bearing APIs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-patterns/SKILL.md`: Plans must include documentation/manifest/protocol coverage when new public surfaces are added.
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`: Planning guidance for documentation metadata, generated docs, tests, and agent discovery.
      - `roadmap.md`: Later phases add IPC, behavior manifests, file/workspace server, SDUI, JavaScript extensions, hot reload, and AI tools.
    - Options Considered:
      - Keep workflow only in the skill: useful for agents, but users/contributors need project-local docs too.
      - Keep workflow only in generated reference: generated output should not be the source of authoring rules.
      - Add a concise source-controlled workflow document plus skill references: readable by users and discoverable by agents.
    - Chosen Approach:
      - Add a short authoring guide under `docs/` that explains the registry, generated artifacts, tests, and rules for adding new public surfaces in later phases.
    - API Notes and Examples:
      ```bash
      cargo test generated_docs_are_current
      cargo fmt
      cargo check
      ```
    - Files to Create/Edit:
      - `docs/documentation-workflow.md`: Authoring workflow and rules.
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`: Update only if implementation discovers a reusable pattern change.
      - `plans/005-Phase4-IPC-Client-Server-Skeleton.md`: No required edit unless Phase 3 changes the documentation workflow expected by Phase 4.
    - References:
      - `.agents/skills/clay-patterns/SKILL.md`
      - `.agents/skills/clay-patterns/references/documentation-as-code.md`
      - `roadmap.md` Phases 4-13.
  - Test Cases to Write:
    - `documentation_workflow_mentions_required_public_surfaces`: Guide includes protocol, command, behavior manifest, permission, extension API, AI tool, and SDUI documentation requirements.
    - Manual documentation review: Confirm a future agent can identify where to add docs for a new Phase 4 protocol message.

- [ ] Run verification and update the plan status
  - Acceptance Criteria:
    - Functional: Documentation registry, generated docs, and coverage tests are implemented and pass.
    - Performance: Verification does not require launching the GUI or server and remains fast enough for normal `cargo test` use.
    - Code Quality: `cargo fmt`, `cargo test`, and `cargo check` pass; generated files are stable across repeated runs.
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
