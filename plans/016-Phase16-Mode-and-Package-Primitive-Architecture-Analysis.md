# Phase 16: Mode and Package Primitive Architecture Analysis

## Objectives

- Analyze and define the full set of primitive categories packages need to control editor behavior and rendering in Clay without hard-coding mode-specific logic into Rust.
- Produce an exhaustive-but-iterative **Clay primitive registry** where every primitive has an owner, authority boundary, hot-path policy, Clay JS API shape, documentation metadata, test expectations, and performance budget.
- Define how packages customize rendering through inert, server-validated declarations (decorator/syntax spans, layout hints, block/inline render intents, SDUI nodes) that the Rust client renders locally without executing package JavaScript in paint or text-event handlers.
- Define Markdown mode proof-of-concept requirements: file-extension detection, major-mode activation, syntax decoration, list continuation, heading/code-block behavior, key binding set, and command set.
- Identify which primitives already exist across behavior manifests, SDUI, configuration, file/workspace APIs, and Phase 14/15 observability, and which new primitives must be added before the Markdown POC in Phase 18.
- Define package-controlled rendering and parsing update strategies with bounded payloads, incremental parse/update units, cancellable background parsing, viewport-prioritized results, and fallback behavior when package work lags behind local edits.
- Define security and provenance requirements for package-provided primitives: package prefix, permissions, no raw ops, no client-side JavaScript, no shell/network/filesystem authority unless explicitly documented and validated.

## Expected Outcome

- Clay has a concrete, documented package/mode primitive architecture that makes the Markdown mode in Phase 18 implementable as a JavaScript package rather than hard-coded Rust.
- A versioned `docs/reference/primitives/index.md` primitive registry document exists, listing every identified primitive with owner, authority, hot-path policy, Clay JS API shape stub, performance budget reference, and prerequisite status.
- A `docs/reference/primitives/markdown-mode-requirements.md` document specifies exactly which primitives and Clay JS APIs the Markdown mode POC needs, and which are deferred or out of scope.
- The roadmap has a concrete, prioritized primitive backlog usable by Phase 17 package loading and Phase 18 Markdown POC implementation.
- Future packages can extend Clay by registering documented primitives while preserving client hot-path performance, visual observability, and server authority.
- No new runtime code is written in this phase; all deliverables are architecture analysis documents and Clay JS API stub definitions verified by coverage tests.

## Tasks

- [x] Audit existing primitives across behavior manifests, SDUI, configuration, file/workspace, and observability
  - Acceptance Criteria:
    - Functional: Every primitive category already present in Phase 1–15 work is identified by name, owner (server/client), authority boundary, current Clay JS API ID or stub, hot-path classification, and existing test/doc coverage status. The audit is written to `docs/reference/primitives/audit.md` and linked from `docs/index.md`.
    - Performance: Audit is a static analysis deliverable; it does not introduce any runtime changes or new code paths.
    - Code Quality: The audit references actual source paths (`src/behavior/manifest.rs`, `src/server/document.rs`, `src/protocol/sdui.rs`, etc.) and existing API IDs from `docs/reference/clay-js-api/api-inventory.toml`. Cross-references must be accurate.
    - Security: The audit explicitly notes which existing primitives carry permissions (e.g., `document-edit`, `workspace-read`) and which are inert declarations that require no permissions.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 16: Identify primitives already existing through manifests, SDUI, configuration, file/workspace APIs, and observability.
      - `docs/reference/clay-js-api/api-inventory.toml`: Existing Clay JS API IDs and their authority/hot-path classifications.
      - `src/behavior/manifest.rs`: Current behavior manifest schema.
      - `src/protocol/sdui.rs`: Current SDUI schema (panels, labels, buttons, lists, editor views, flex).
      - `src/perf/budgets.rs`: `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, and latency budgets.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Routing policies and versioning rules.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server vs. client ownership.
    - Options Considered:
      - Audit as inline comments in source code: harder to read as a standalone design artifact and not discoverable by AI agents.
      - Audit as a structured TOML inventory alongside `api-inventory.toml`: consistent with existing tooling direction but premature without finalized primitive schema.
      - Audit as a Markdown document first, TOML later when primitives stabilize: preferred because it produces a readable design artifact immediately and defers schema commits until Phase 17.
    - Chosen Approach:
      - Write `docs/reference/primitives/audit.md` as a Markdown table/section document covering existing primitive categories (behavior-manifest entries, SDUI node types, configuration APIs, file/workspace APIs, observability hooks) with owner, authority, hot-path policy, existing Clay JS API ID, and Phase 14/15 performance budget reference.
    - API Notes and Examples:
      ```text
      Existing primitives (partial list, to be verified in audit):
      - KeyBinding (clay.keybindings.bindKey) — manifest data, client-first routing
      - IndentRule (behavior manifest field) — manifest data, client-first predictable
      - SDUI Panel (clay.sdui.definePanel) — server-first, SDUI snapshot/update path
      - DocumentEdit (clay.editor.serverInsertText) — server-authoritative, async ack
      - BehaviorManifest (clay.behavior.getActiveBehaviorManifest) — server-issued, client-executed
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/audit.md`: New — structured audit of existing primitives.
      - `docs/index.md`: Add link to `docs/reference/primitives/audit.md` under the primitives/package section.
    - References:
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `src/behavior/manifest.rs`, `src/protocol/sdui.rs`, `src/perf/budgets.rs`
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `roadmap.md` Phase 16
  - Test Cases to Write:
    - `primitives_audit_doc_linked_from_index`: `cargo test` verifies `docs/reference/primitives/audit.md` is linked from `docs/index.md`.
    - `primitives_audit_cites_valid_api_ids`: Each Clay JS API ID referenced in the audit exists in `api-inventory.toml` (static text check via test helper).

- [x] Define the primitive category taxonomy and Clay primitive registry schema
  - Acceptance Criteria:
    - Functional: A complete taxonomy of package-controllable primitive categories is defined and written to `docs/reference/primitives/registry.md`. Every category has a name, description, owner (server/client), authority boundary, hot-path classification, Clay JS API shape stub (module, export name, stable ID, user-facing name), performance budget pointer, and whether it is manifest data, server-first command, SDUI state, or renderer/decorator data. At minimum the following categories must be covered: document classification/mode detection, major-mode activation, minor-mode activation, key routing overrides, text transforms (auto-indent, pair, continuation), incremental parse/syntax tree updates, decoration ranges (syntax spans, semantic spans, diagnostic spans), folding ranges, completion triggers and result shapes, command declarations, SDUI panel/status contributions, package-owned configuration, and package-owned permission declarations.
    - Performance: All registry entries must reference a budget constant from `src/perf/budgets.rs` or explicitly record `no-hot-path` with rationale. New budget constants for decoration/parse payloads and mode activation latency must be proposed and documented (constants added to `src/perf/budgets.rs` in this task, advisory only, not hard CI failures per Phase 14 carry-forward policy).
    - Code Quality: The registry schema must be consistent with the existing `api-inventory.toml` field vocabulary (`authority`, `hot_path_policy`, `js_module`, `js_export`, `user_facing_name`, `permissions`, `security_notes`). Deviations require documented rationale.
    - Security: Every primitive category must explicitly state the permission scope it requires (or `none` for inert declarations), state that no raw ops or client-side JavaScript are permitted, and confirm it cannot grant file/network/shell/AI authority unless the permission is explicitly declared and server-validated.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 16: Full primitive category list including document classification, major/minor mode activation, key routing, text transforms, incremental parsing, decoration ranges, semantic spans, folding, diagnostics, completion triggers, commands, SDUI panels, status items, and package-owned configuration.
      - `roadmap.md` Phase 17: Package manifest format, per-document/per-mode behavior manifest selection, deterministic conflict handling.
      - `roadmap.md` Phase 18: Markdown mode requirements as a concrete primitive consumer.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Routing policy vocabulary.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required schema fields.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Naming rules including `server*`/`client*` authority markers and package prefix rules.
      - `.agents/skills/project-patterns/references/package-distribution.md`: Package identity, prefix, permissions, and load/runtime separation.
    - Options Considered:
      - TOML primitive registry matching `api-inventory.toml` immediately: consistent tooling but premature for primitives with no code yet — schema needs review first.
      - Markdown registry document first: readable design artifact, usable by Phase 17 plan without tooling changes.
      - Full code generation immediately: premature; generates churn if primitive shapes change during Markdown POC iteration.
    - Chosen Approach:
      - Write the primitive registry as `docs/reference/primitives/registry.md` with per-category sections using a consistent Markdown table layout. Budget constants for new categories (decoration payload, incremental parse payload, mode activation latency) are added to `src/perf/budgets.rs` as advisory-only constants. A `PRIMITIVES_REGISTRY_VERSION` string constant is defined in `src/perf/budgets.rs` or a new `src/primitives/registry_version.rs` to allow future freshness tests.
    - API Notes and Examples:
      ```text
      Primitive Registry Entry Shape (Markdown table row):

      | Primitive | Owner | Authority | Hot-Path Policy | JS Module | JS Export | Stable ID | User-Facing Name | Budget Ref | Prerequisite |
      |---|---|---|---|---|---|---|---|---|---|
      | MajorModeDeclaration | server | server-first-on-activate | ClientFirstManifestInstall | clay:modes | serverActivateMajorMode | clay.modes.serverActivateMajorMode | Activate Major Mode | BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES | New |
      | DecorationRange | server→client | server-produced, client-rendered | Background | clay:decorations | serverPublishDecorations | clay.decorations.serverPublishDecorations | Publish Decorations | DECORATION_PAYLOAD_BUDGET_BYTES | New |
      | KeyRoutingOverride | server→manifest | manifest-data | ClientFirstPredictable | clay:keybindings | bindKey (package-prefixed) | package-prefixed | Bind Key | BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES | Exists |
      ```

      New budget constants to propose:
      ```rust
      // src/perf/budgets.rs
      pub const DECORATION_PAYLOAD_BUDGET_BYTES: usize = 8192;        // advisory
      pub const INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES: usize = 4096;  // advisory
      pub const MODE_ACTIVATION_P95_BUDGET_MS: u64 = 100;             // advisory
      pub const COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES: usize = 4096; // advisory
      pub const FOLDING_RANGE_PAYLOAD_BUDGET_BYTES: usize = 2048;     // advisory
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/registry.md`: New — comprehensive primitive category registry.
      - `docs/index.md`: Add link to `docs/reference/primitives/registry.md`.
      - `src/perf/budgets.rs`: Add advisory budget constants for new primitive categories.
      - `tests/primitives_docs.rs`: Add static coverage for the registry link, required category taxonomy, schema field vocabulary, and advisory budget constants.
    - References:
      - `roadmap.md` Phases 16, 17, 18
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `src/perf/budgets.rs`
  - Test Cases to Write:
    - `primitives_registry_linked_from_index`: `cargo test` verifies `docs/reference/primitives/registry.md` is linked from `docs/index.md`.
    - `primitives_budget_constants_compile`: `cargo check` verifies new budget constants in `src/perf/budgets.rs` compile without warnings.
    - `primitives_registry_categories_cover_required_list`: Static test verifies the registry document contains sections for all required categories listed in Phase 16 roadmap focus areas (text check).

- [x] Define rendering customization strategy: inert declarations and client-side rendering
  - Acceptance Criteria:
    - Functional: `docs/reference/primitives/rendering-strategy.md` defines how packages produce rendering customizations without arbitrary client-side JavaScript. The document covers: decoration span shapes (byte range, kind, style token, priority), layout hints (block/inline intent, margin, emphasis level), SDUI node contributions from packages, render intent versioning and viewport filtering, the server-side compilation step that validates and bounds declaration payloads, and the client rendering path that applies validated declarations without calling package code. The document explicitly states which rendering primitives are new and which reuse existing SDUI or behavior manifest paths.
    - Performance: All rendering primitive shapes must be bounded by `DECORATION_PAYLOAD_BUDGET_BYTES` or the existing `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` / `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` constants. The document must describe viewport-prioritized incremental update semantics so large documents do not require full decoration repaint on every edit.
    - Code Quality: The rendering strategy document must cross-reference `src/masonry_sdui.rs` (existing SDUI client application), `src/masonry_editor.rs` (editor paint path), and the Parley/Vello rendering pipeline, with explicit notes on where new rendering hooks would attach if new client-side rendering primitives are added in Phase 18.
    - Security: The document must confirm packages cannot inject arbitrary GPU draw calls, native widget mutations, or synchronous JavaScript into Masonry paint handlers. All package rendering flows through validated server-produced declarations.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 16: "Define how rendering customization works without arbitrary client-side JavaScript: packages produce inert declarations such as syntax/decorator spans, layout hints, block/inline render intents, or SDUI nodes; the Rust client renders validated declarations locally."
      - `roadmap.md` Phase 14: Performance budget constants for SDUI payloads.
      - `src/masonry_sdui.rs`, `src/masonry_editor.rs`: Existing client rendering integration points.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Viewport-bounded rendering, bounded queues, no full-document IPC.
    - Options Considered:
      - Extend SDUI for all rendering customizations: SDUI handles panels and widgets; applying it to inline text decorations (span-level syntax highlighting) would be over-engineered and likely exceed SDUI payload budgets.
      - Custom `DecorationUpdate` protocol message: clean separation from SDUI, bounded by its own payload constant, maps directly to Parley/Vello attribute ranges.
      - Mixed approach using SDUI for block-level/panel contributions and a separate `DecorationUpdate` for inline span decorations: preferred because it respects existing SDUI authority model for layout while keeping inline text rendering efficient.
    - Chosen Approach:
      - Document the mixed rendering approach: packages contribute SDUI node subtrees for block-level UI (panels, status bars, preview panes) through existing `clay:sdui` APIs, and a new `DecorationUpdate` message shape for inline span decorations (syntax highlighting, diagnostic underlines, semantic emphasis). Both paths are server-validated before client delivery. `DecorationUpdate` is bounded by `DECORATION_PAYLOAD_BUDGET_BYTES` and contains only viewport-relevant spans (server filters by visible byte range before sending). The document specifies that no new rendering code is written in this phase; the shapes and attachment points are defined here for Phase 17/18 implementation.
    - API Notes and Examples:
      ```rust
      // Proposed DecorationUpdate shape (for documentation only in this phase)
      // src/protocol/decorations.rs (to be created in Phase 17/18)
      pub struct DecorationSpan {
          pub byte_start: u64,
          pub byte_end: u64,
          pub kind: DecorationKind,        // Keyword, Comment, String, Diagnostic, etc.
          pub style_token: String,          // e.g. "keyword.control", "diagnostic.error"
          pub priority: u8,
      }

      pub struct DecorationUpdate {
          pub document_id: DocumentId,
          pub document_version: u64,
          pub behavior_version: u64,
          pub package_prefix: String,       // provenance
          pub viewport_byte_start: u64,
          pub viewport_byte_end: u64,
          pub spans: Vec<DecorationSpan>,   // bounded by DECORATION_PAYLOAD_BUDGET_BYTES
      }
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/rendering-strategy.md`: New — rendering customization strategy document.
      - `docs/index.md`: Link `rendering-strategy.md` from the primitives section.
      - `tests/primitives_docs.rs`: Add static coverage for the rendering strategy link, inert/client-rendering contract, and budget references.
    - References:
      - `roadmap.md` Phase 16
      - `src/masonry_sdui.rs`, `src/masonry_editor.rs`
      - `src/perf/budgets.rs`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `rendering_strategy_doc_linked_from_index`: `cargo test` verifies `docs/reference/primitives/rendering-strategy.md` is linked from `docs/index.md`.
    - `decoration_budget_constant_exists`: `cargo check` verifies `DECORATION_PAYLOAD_BUDGET_BYTES` is defined in `src/perf/budgets.rs`.
    - `rendering_strategy_covers_inert_client_rendering_contract`: Static test verifies the strategy documents decoration span shapes, layout hints, render intent versioning, server validation, client attachment points, Parley/Vello usage, and security prohibitions.
    - `rendering_strategy_references_payload_budgets`: Static test verifies the strategy references decoration and SDUI payload budgets plus viewport-prioritized updates.

- [x] Define incremental parsing and background parse update strategy
  - Acceptance Criteria:
    - Functional: `docs/reference/primitives/parse-update-strategy.md` defines how package-provided parsers produce incremental syntax trees and decoration updates without blocking the typing hot path. The document covers: parse unit boundaries (file-level, region-level, or line-group-level), incremental edit notification from the server to background parse tasks, parse task lifecycle (spawn, cancel, timeout, result publication), viewport-prioritized result delivery, fallback behavior when parse results lag behind edits, and the server validation step that bounds and verifies parse-produced decoration payloads before client delivery.
    - Performance: The document must reference `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` and `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`. It must explicitly state that parse tasks are background-priority (`Background` routing policy) and must not block `ClientFirstPredictable` keypress-to-paint paths. Parse result delivery must be viewport-bounded and cancellable.
    - Code Quality: The document must identify where background parse task spawning would attach in the existing server architecture (`src/server/document.rs`, `src/server/js_runtime.rs`, or a new `src/server/parse_coordinator.rs`) without introducing new code in this phase.
    - Security: Background parse tasks run server-side through the `deno_core` runtime, not in the Rust client. Parse results are validated and stripped of arbitrary code before being packaged as `DecorationUpdate` payloads. Parse tasks cannot access filesystem, network, shell, or AI authority unless explicitly declared.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 16: "Define package-controlled rendering and parsing update strategies: bounded decoration payloads, incremental parse/update units, cancellable background parsing, viewport-prioritized results, and fallback behavior when package work lags behind local edits."
      - `roadmap.md` Phase 14: Performance budget constants, `Background` routing policy.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: `Background` routing policy classification.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Cancellable background work, bounded queues.
    - Options Considered:
      - Synchronous parse per edit in the JavaScript runtime: would violate the no-synchronous-JavaScript-in-keypress-path rule.
      - Full file re-parse on every server-accepted edit: too expensive for large files; known from Phase 14 benchmarks.
      - Incremental parse task with cancellation and viewport prioritization: preferred because it matches the `Background`/`UiReactivePriority` routing policies already in the manifest system.
    - Chosen Approach:
      - Define an incremental parse task model where the server dispatches small edit notifications to a package's background parse handler after each acknowledged edit. Parse handlers run on the `deno_core` runtime as `Background`-priority tasks, produce `DecorationUpdate` payloads bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, and are cancelled if a newer edit arrives before the result is published. Viewport-prioritized delivery means the server sends only spans within the current client viewport byte range, caching the rest for scroll events. Fallback: if a parse result is not ready within two edit cycles, the client retains the last known decoration set; the server sends a `no-decoration-update` acknowledgement to avoid client state drift.
    - API Notes and Examples:
      ```text
      Parse task lifecycle (server-side):
      1. Server accepts edit → increments document_version.
      2. Server enqueues ParseEditNotification { document_id, edit, base_version } to background task.
      3. Background task calls package.onEdit(notification) → returns Promise<DecorationUpdate>.
      4. Server cancels pending promise if newer version arrives.
      5. Server validates DecorationUpdate payload size ≤ INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES.
      6. Server filters spans to viewport byte range.
      7. Server sends DecorationUpdate to connected clients with document_id and document_version.
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/parse-update-strategy.md`: New — incremental parse and background parse task strategy.
      - `docs/index.md`: Link `parse-update-strategy.md` from the primitives section.
    - References:
      - `roadmap.md` Phases 16, 18
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src/server/document.rs`, `src/server/js_runtime.rs`
  - Test Cases to Write:
    - `parse_strategy_doc_linked_from_index`: `cargo test` verifies `docs/reference/primitives/parse-update-strategy.md` is linked from `docs/index.md`.
    - `incremental_parse_budget_constant_exists`: `cargo check` verifies `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` is defined in `src/perf/budgets.rs`.

- [x] Define Markdown mode POC requirements and primitive prerequisite mapping
  - Acceptance Criteria:
    - Functional: `docs/reference/primitives/markdown-mode-requirements.md` specifies the Markdown mode POC scope for Phase 18 in terms of primitives only (no implementation). The document covers: file-extension/MIME-type mode detection rules, major-mode declaration and activation API shape, Enter/list-continuation behavior manifest rule shape, heading/bold/italic/code-span/code-block decoration span kinds, fenced code block indent behavior, key binding set (heading insertion, list toggle, preview toggle), command set (toggle Markdown preview, insert heading), SDUI panel for preview/decoration toggle, minimum performance expectations referenced from `src/perf/budgets.rs`, and minimum Clay JS API stubs needed before Phase 18 can begin. For each required primitive it must state: exists/new/deferred, which Phase 16 registry entry covers it, and estimated risk.
    - Performance: The document must include a table of Phase 18 Markdown mode performance targets for: startup parse cost (`MODE_ACTIVATION_P95_BUDGET_MS`), incremental edit decoration cost (`INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`), decoration payload per update (`DECORATION_PAYLOAD_BUDGET_BYTES`), and scroll/render latency (`SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`), all referencing constants from `src/perf/budgets.rs`.
    - Code Quality: The document must be organized as a Phase 18 readiness checklist so the Phase 18 plan author can directly reference it for task derivation. Each primitive row must map to a registry entry in `docs/reference/primitives/registry.md`.
    - Security: The Markdown mode is declared as a first-party package with `@clay/markdown` as the package identity and `markdown` as the API prefix. The document must state that the package cannot access filesystem beyond document content already open, cannot use network/shell/AI authority, and uses only documented Clay JS APIs with declared permissions.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 16: "Decide the Markdown mode POC requirements: Markdown syntax highlighting/rendering target, list continuation, heading emphasis, code block behavior, preview/decorated editor behavior, command/key binding set, and minimum file-extension/mode detection rules."
      - `roadmap.md` Phase 18: Detailed Markdown mode implementation scope.
      - `.agents/skills/project-patterns/references/package-distribution.md`: Package identity, prefix, permission model.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Manifest rules for list continuation, Enter handling.
      - `docs/reference/primitives/registry.md` (produced by previous task): Registry entries to reference.
    - Options Considered:
      - Specify Markdown mode requirements in the Phase 18 plan only: defers prerequisite discovery to Phase 18, increasing risk.
      - Specify requirements in Phase 16 as a separate requirements document: enables Phase 17 to prioritize exactly the primitives Markdown mode needs, reducing Phase 18 scope risk.
    - Chosen Approach:
      - Write `docs/reference/primitives/markdown-mode-requirements.md` as a cross-reference document that maps each Markdown mode capability to a primitive in the registry, marks each primitive as exists/new/deferred, and records the Phase 18 prerequisite status. This document becomes the readiness gate for entering Phase 18.
    - API Notes and Examples:
      ```markdown
      ## Markdown Mode Primitive Prerequisites (excerpt)

      | Capability | Primitive Category | Registry Entry | Status | Risk |
      |---|---|---|---|---|
      | .md file → Markdown major mode | DocumentClassification | clay.modes.registerModePattern | New | Medium |
      | List continuation on Enter | TextTransform/ManifestRule | clay.behavior.* (extend manifest) | Exists (extend) | Low |
      | Heading decoration spans | DecorationRange | clay.decorations.serverPublishDecorations | New | Medium |
      | Fenced code block indent | TextTransform/ManifestRule | clay.behavior.* | Exists (extend) | Low |
      | Markdown preview SDUI panel | SduiContribution | clay.sdui.definePanel | Exists | Low |
      | Toggle Markdown preview command | CommandDeclaration | clay.commands.registerCommand | New | Medium |
      | Background syntax parse task | IncrementalParse | clay.parse.registerParseHandler | New | High |
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/markdown-mode-requirements.md`: New — Markdown mode POC primitive prerequisite map.
      - `docs/index.md`: Link `markdown-mode-requirements.md` from the primitives section.
      - `tests/primitives_docs.rs`: Add static coverage for the Markdown requirements link, registry-entry prerequisite mapping, and Phase 18 POC contract content.
      - `docs/wiki/modules/primitive-architecture-docs.md`: Document the primitive architecture documentation/test coverage flow.
      - `docs/wiki/index.md`: Link the primitive architecture documentation wiki page.
    - References:
      - `roadmap.md` Phases 16, 18
      - `docs/reference/primitives/registry.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `src/perf/budgets.rs`
  - Test Cases to Write:
    - `markdown_mode_requirements_doc_linked_from_index`: `cargo test` verifies `docs/reference/primitives/markdown-mode-requirements.md` is linked from `docs/index.md`.
    - `markdown_mode_prerequisites_reference_registry_entries`: Static test verifies each primitive row in `markdown-mode-requirements.md` references a section name present in `registry.md` (text check).
    - `markdown_mode_requirements_cover_phase18_poc_contract`: Static test verifies the Markdown POC detection, editing, decoration, command, SDUI, performance budget, and security requirements stay documented.

- [x] Define security and provenance requirements for package-provided primitives
  - Acceptance Criteria:
    - Functional: `docs/reference/primitives/package-security.md` defines the security and provenance model for all package-provided primitives. The document covers: required package prefix declaration, how package primitive contributions are scoped to the package prefix in the Clay JS API registry, permission declaration requirements for each primitive category, what server-side validation Clay performs before accepting primitive contributions (schema validation, payload bound checks, permission scope checks, no raw op calls, no client-side JavaScript references), conflict handling for duplicate primitive registrations (same mode name, same key binding, same decoration range overlap), and the prohibition on primitives that grant file/network/shell/AI authority without explicit declared permissions.
    - Performance: The document specifies that server-side primitive validation runs at package load time (not on every edit) and that validation failures produce a load-time error, not a runtime panic. Validation cost must not appear on the typing hot path.
    - Code Quality: The document must reference `.agents/skills/project-patterns/references/package-distribution.md` and `.agents/skills/project-patterns/references/extensions-and-ai.md` as the authoritative decision sources. All security rules must be traceable to an existing decision log or roadmap directive.
    - Security: This is the primary security deliverable of Phase 16. The document must enumerate every authority that packages may NOT claim by default: filesystem outside open documents, network, shell, AI mutation, remote listeners, WASM execution, raw `Deno.core.ops`, direct Masonry/widget mutation, and client-side JavaScript. Any future exception requires a new decision log before a plan can implement it.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 16: "Define security and provenance requirements for package-provided primitives: package prefix, permissions, no raw ops, no client JS, no shell/network/filesystem authority unless explicitly documented and validated."
      - `.agents/skills/project-patterns/references/package-distribution.md`: Package identity, prefix, permissions, load vs. runtime separation.
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`: JavaScript extension security boundaries.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: No raw ops, no direct Rust public function exposure.
      - `decision-logs/` (any relevant security decisions from prior phases).
    - Options Considered:
      - Embed security requirements inline in each primitive registry entry: granular but scattered; hard to audit as a complete security surface.
      - Separate security document: provides a single audit target, easy to cross-reference from each primitive, and aligns with the `documentation-as-code` pattern of making security inspectable.
    - Chosen Approach:
      - Write `docs/reference/primitives/package-security.md` as the canonical security surface document for package primitives. Reference it from the primitive registry (`registry.md`) header section and from the Markdown mode requirements. The document is a living artifact that must be reviewed and updated for any new primitive category added in Phases 17+.
    - API Notes and Examples:
      ```text
      Required security checks at package load time:
      1. Package declares clay.apiPrefix matching pattern /^[a-z][a-z0-9-]{1,31}$/
      2. All contributed Clay JS API IDs start with `{apiPrefix}.`
      3. Each primitive contribution declares a non-empty permissions list or []
      4. Payload size of all contributed manifests/declarations ≤ category budget constant
      5. No api IDs beginning with `clay.` (reserved for first-party)
      6. No contributed init code that calls Deno.core.ops.* directly
      7. Mode name uniqueness checked against active mode registry
      8. Key binding conflicts resolved by declared priority or rejected with actionable error

      Prohibited authorities (no package may claim without a new approved decision log):
      - filesystem (beyond document content already open)
      - network
      - shell
      - ai-mutation
      - wasm-execution
      - raw-deno-ops
      - client-js-execution
      - native-widget-mutation
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/package-security.md`: New — package primitive security and provenance requirements.
      - `docs/index.md`: Link `package-security.md` from the primitives section.
      - `docs/reference/primitives/registry.md`: Reference the package security document from the shared security baseline.
      - `docs/reference/primitives/markdown-mode-requirements.md`: Reference the package security document for the Markdown package baseline.
      - `tests/primitives_docs.rs`: Add static coverage for the package security link, source references, validation requirements, conflict handling, and prohibited authorities.
    - References:
      - `roadmap.md` Phase 16
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
  - Test Cases to Write:
    - `package_security_doc_linked_from_index`: `cargo test` verifies `docs/reference/primitives/package-security.md` is linked from `docs/index.md`.
    - `package_security_doc_references_decision_sources`: Static text check verifies the document references `package-distribution.md` and `extensions-and-ai.md` pattern files.
    - `package_security_doc_covers_validation_conflicts_and_prohibitions`: Static text check verifies the document covers prefix validation, load-time validation, conflict handling, typing-hot-path isolation, prohibited authorities, and decision-log requirements for future exceptions.

- [x] Produce prioritized primitive backlog and Phase 17 prerequisite checklist
  - Acceptance Criteria:
    - Functional: `docs/reference/primitives/backlog.md` contains a prioritized list of all new primitives identified in the registry that must be implemented before the Phase 18 Markdown POC, plus primitives deferred to later phases. Each entry states: primitive name, category, priority tier (Phase-17-required / Phase-18-required / Deferred), estimated implementation location (new Clay JS API stub, server module, protocol message, or behavior manifest extension), Clay JS API ID target, and which plan document (Phase 17 or 18) should implement it. The backlog is sortable by priority tier and referenced from `docs/reference/primitives/index.md`.
    - Performance: No runtime changes; this is a planning document deliverable only.
    - Code Quality: Every backlog entry must trace to a primitive category in `registry.md` and a Phase 16 analysis document (audit, rendering strategy, parse strategy, or Markdown requirements). Orphan entries without a registry trace are not permitted.
    - Security: Backlog entries for permission-bearing primitives must include the permission name from `package-security.md` and a note that the permission must be declared and server-validated before the primitive is executable.
  - Approach:
    - Documentation Reviewed:
      - All Phase 16 analysis documents produced by prior tasks: `audit.md`, `registry.md`, `rendering-strategy.md`, `parse-update-strategy.md`, `markdown-mode-requirements.md`, `package-security.md`.
      - `roadmap.md` Phase 17: "Define the package manifest format… use the approved npm-compatible package distribution direction… add per-document/per-mode behavior manifest selection… add deterministic conflict handling."
      - `roadmap.md` Phase 18: Markdown mode implementation scope — backlog must ensure Phase 17 implements the primitives Phase 18 needs.
    - Options Considered:
      - Inline backlog in the primitive registry: combines two concerns and makes the registry hard to scan.
      - Separate backlog document cross-referencing the registry: preferred for clarity; Phase 17 and 18 plan authors can directly consume it.
    - Chosen Approach:
      - Write `docs/reference/primitives/backlog.md` as a three-tier priority table derived from all prior Phase 16 analysis documents. Create `docs/reference/primitives/index.md` as a navigation index for the entire `docs/reference/primitives/` directory, linking all six analysis documents plus the backlog.
    - API Notes and Examples:
      ```markdown
      ## Phase-17-Required (must exist before Phase 17 package loading)
      | Primitive | Category | Clay JS API Target | Implementation Location |
      |---|---|---|---|
      | MajorModeDeclaration | ModeActivation | clay.modes.serverActivateMajorMode | New server module + protocol msg |
      | ModePatternRegistration | DocumentClassification | clay.modes.registerModePattern | New server registry |
      | CommandDeclaration | CommandRegistry | clay.commands.registerCommand | New Clay JS API + op |

      ## Phase-18-Required (must exist before Markdown POC)
      | Primitive | Category | Clay JS API Target | Implementation Location |
      |---|---|---|---|
      | DecorationRange | DecorationUpdate | clay.decorations.serverPublishDecorations | New protocol + client render hook |
      | IncrementalParseHandler | BackgroundParse | clay.parse.registerParseHandler | New server task coordinator |
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/index.md`: New — primitives directory navigation index.
      - `docs/reference/primitives/backlog.md`: New — prioritized primitive backlog.
      - `docs/index.md`: Replace individual primitive doc links with a single link to `docs/reference/primitives/index.md` (or add the index link if individual links should be preserved).
    - References:
      - All prior Phase 16 analysis documents
      - `roadmap.md` Phases 17, 18
  - Test Cases to Write:
    - `primitives_index_linked_from_docs_index`: `cargo test` verifies `docs/reference/primitives/index.md` is linked from `docs/index.md`.
    - `primitives_backlog_entries_trace_to_registry`: Static text check verifies each backlog entry primitive name appears as a section heading or table row in `registry.md`.
    - `primitives_phase17_required_entries_exist`: Static check verifies the backlog contains at minimum `MajorModeDeclaration`, `ModePatternRegistration`, and `CommandDeclaration` entries (names may differ, but a mode activation and command registration primitive must be present).

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Review all primitive categories identified in Phase 16 and propose Clay JS API stubs for new primitives that require public programmatic exposure. For each proposed stub: record the JS module, export name, stable ID, user-facing name, authority, hot-path policy, `deno_core` op name (stub, not yet implemented), documentation path (stub `docs/reference/clay-js-api/` entry), key binding metadata, custom properties, permissions, and security notes. Add stubs to `docs/reference/clay-js-api/api-inventory.toml` with `status = "planned"`. Existing API IDs must not be changed. Every new planned API must trace to a primitive in `docs/reference/primitives/registry.md`.
    - Performance: Phase 16 introduces no new runtime code; stubs carry `status = "planned"` and produce no op wrappers or facade code in this phase. Stub entries in `api-inventory.toml` must pass existing coverage tests (all required TOML fields present, no missing `user_facing_name`, `key_bindings`, `custom_properties`).
    - Code Quality: Apply `clay-js-api-naming.md` to all new stubs: `clay.modes.*`, `clay.commands.*`, `clay.decorations.*`, `clay.parse.*` modules; `server*` for server-authoritative calls; `client*` for client-local; package prefix for package-owned exports; no raw op names in export names.
    - Security: Every new stub must include `security_notes` stating which authorities it does and does not grant. Stubs for permission-bearing primitives must include the relevant permission name from `package-security.md`.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required fields for `api-inventory.toml` entries.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Naming rules.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Facade and op boundary.
      - `docs/reference/primitives/registry.md` and `backlog.md` (produced by prior tasks).
      - `docs/reference/clay-js-api/api-inventory.toml`: Existing entries as naming and field vocabulary reference.
    - Options Considered:
      - Add stubs only for Phase-17-required primitives: minimal scope, but Phase 18 prerequisite gaps discovered late.
      - Add stubs for all new primitives including deferred ones with `status = "deferred"`: creates inventory drift.
      - Add stubs for Phase-17-required and Phase-18-required primitives only, with `status = "planned"`; mark deferred primitives as separate comments: preferred.
    - Chosen Approach:
      - Add `status = "planned"` stubs to `api-inventory.toml` for all Phase-17-required and Phase-18-required new Clay JS APIs identified in the backlog. Each stub includes all required TOML fields. Stubs do not generate code, op wrappers, facade exports, generated-registry entries, or Markdown docs pages in this phase (docs stubs are deferred to Phase 17/18); the new stubs therefore use `registry_public = false` until those implementation artifacts exist. Existing coverage tests pass with the new stub entries.
    - API Notes and Examples:
      ```toml
      # Example new planned stubs in docs/reference/clay-js-api/api-inventory.toml

      [[api]]
      id = "clay.modes.serverActivateMajorMode"
      category = "mode-activation"
      visibility = "public"
      status = "planned"
      js_module = "clay:modes"
      js_export = "serverActivateMajorMode"
      user_facing_name = "Activate Major Mode"
      authority = "server-first-mode-activation"
      runtime_path = "server-first-op-wrapper"
      hot_path_policy = "Mode activation is server-first; the client receives a new behavior manifest after activation. Not on the typing hot path."
      facade_path = "runtime/js/modes.ts::serverActivateMajorMode"
      backing_rust = "src/server/modes.rs::ModeRegistry::activate_major_mode"
      deno_op = "op_clay_modes_activate_major_mode"
      deno_op_path = "src/server/ops/modes.rs::op_clay_modes_activate_major_mode"
      documentation_path = "docs/reference/clay-js-api/modes/server-activate-major-mode.md"
      key_bindings = []
      custom_properties = []
      permissions = ["mode-activation"]
      security_notes = "Activates a registered major mode for the target document; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority. Mode must be registered via registerModePattern before activation."
      registry_public = true

      [[api]]
      id = "clay.commands.serverRegisterCommand"
      category = "command-registry"
      visibility = "public"
      status = "planned"
      js_module = "clay:commands"
      js_export = "serverRegisterCommand"
      user_facing_name = "Register Command"
      authority = "server-first-command-registration"
      runtime_path = "server-first-op-wrapper"
      hot_path_policy = "Command registration occurs at package load time, not on the typing hot path."
      facade_path = "runtime/js/commands.ts::serverRegisterCommand"
      backing_rust = "src/server/commands.rs::CommandRegistry::register_command"
      deno_op = "op_clay_commands_register_command"
      deno_op_path = "src/server/ops/commands.rs::op_clay_commands_register_command"
      documentation_path = "docs/reference/clay-js-api/commands/server-register-command.md"
      key_bindings = []
      custom_properties = ["routingPolicy:manifest=ServerFirst", "requiresLock:boolean=false"]
      permissions = ["command-registration"]
      security_notes = "Registers a named command with a routing policy; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, WASM, or client-side JavaScript authority. Command handler permissions are declared separately."
      registry_public = true
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: Added new `status = "planned"` stubs for Phase-17-required and Phase-18-required primitives: `clay.modes.serverActivateMajorMode`, `clay.modes.serverRegisterModePattern`, `clay.commands.serverRegisterCommand`, `clay.commands.serverListCommands`, `clay.decorations.serverPublishDecorations`, `clay.parse.serverRegisterParseHandler`, `clay.folding.serverPublishFoldingRanges`, `clay.configuration.setPackageOption`, and `clay.packages.serverValidatePackagePermissions`.
      - `docs/reference/primitives/backlog.md`: Updated `CommandDeclaration` to trace the metadata-only `clay.commands.serverListCommands` query.
      - `docs/reference/primitives/registry.md`: Added the `serverListCommands` trace under `CommandDeclaration`.
      - `tests/primitives_docs.rs`: Added static coverage for planned primitive API stubs, required metadata, security notes, and registry/backlog traces.
      - `docs/wiki/modules/primitive-architecture-docs.md`: Documented the planned-stub inventory flow and test coverage.
    - References:
      - `docs/reference/primitives/registry.md`, `backlog.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases to Write:
    - `api_inventory_new_stubs_have_required_fields`: `cargo test` existing coverage gate verifies all new stubs have `user_facing_name`, `key_bindings`, `custom_properties`, `security_notes`, and `registry_public`.
    - `api_inventory_new_stub_ids_unique`: Existing uniqueness test covers new entries without changes.
    - `new_planned_stubs_trace_to_primitive_registry`: Static text check verifies the stable ID suffix (e.g., `serverActivateMajorMode`) appears in a section of `docs/reference/primitives/registry.md`.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review Phase 16 analysis documents for any user-configurable behavior introduced or planned (mode activation defaults, decoration style preferences, parse handler timeouts, package enable/disable). For each configuration surface: add or verify a Clay JS API stub in `api-inventory.toml` with `status = "planned"`, `js_module = "clay:configuration"` or the relevant domain module, required fields, permissions, and security notes. Verify no new configuration surface bypasses the `~/.config/clay/init.js` entry point model.
    - Performance: Configuration stubs are load-time only; none may appear on the typing hot path.
    - Code Quality: Apply `clay-js-api-naming.md` and `clay-js-api-schema.md`. Configuration APIs use `custom_properties` to declare behavior-changing settings with type, default, and allowed values.
    - Security: Configuration must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority. Permission-bearing configuration APIs must declare the permission explicitly.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Clay Configuration Task requirement.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`, `clay-js-api-naming.md`.
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
      - Phase 16 analysis documents for configuration surfaces.
    - Options Considered:
      - No new configuration APIs in Phase 16 (analysis only): possible if no new configurable behavior is surfaced.
      - Add stubs for configurable decoration styles and mode activation defaults: anticipated from Phase 16 analysis.
    - Chosen Approach:
      - After completing the primitive registry and Markdown mode requirements, identify user-configurable behavior surfaced by Phase 16: package-owned options, mode activation defaults, decoration style preferences, and parse handler timeout/policy. Add `status = "planned"` inventory stubs for concrete configuration APIs rooted in `~/.config/clay/init.js`: `clay.configuration.setPackageOption`, `clay.configuration.setModePreference`, `clay.configuration.setDecorationTheme`, and `clay.configuration.setParsePolicy`. Document package enable/disable as intentionally unexposed in Phase 16 because package-management authority requires a future approved decision log and explicit server-side permission model.
    - API Notes and Examples:
      ```toml
      # Example: if decoration style configuration is identified
      [[api]]
      id = "clay.configuration.setDecorationTheme"
      category = "decoration-configuration"
      visibility = "public"
      status = "planned"
      js_module = "clay:configuration"
      js_export = "setDecorationTheme"
      user_facing_name = "Set Decoration Theme"
      authority = "client-local-configuration"
      runtime_path = "configuration-api"
      hot_path_policy = "Configuration only; not on the typing hot path."
      facade_path = "runtime/js/configuration.ts::setDecorationTheme"
      backing_rust = "src/server/configuration.rs"
      deno_op = "op_clay_configuration_set_decoration_theme"
      deno_op_path = "src/server/ops/configuration.rs"
      documentation_path = "docs/reference/clay-js-api/configuration/set-decoration-theme.md"
      key_bindings = []
      custom_properties = ["theme:string=default", "contrastMode:boolean=false"]
      permissions = []
      security_notes = "Sets the active decoration token-to-color mapping; does not grant filesystem, network, shell, or AI authority."
      registry_public = true
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: Added/verified planned configuration stubs for package options, mode preferences, decoration theme preferences, and parse policy; documented why package enable/disable is not exposed in Phase 16.
      - `docs/reference/primitives/registry.md`: Added the configuration stub trace under `PackageOwnedConfiguration`.
      - `docs/reference/primitives/backlog.md`: Added configuration API IDs to the Phase 17 package configuration backlog and readiness checklist.
      - `tests/primitives_docs.rs`: Added static coverage for Phase 16 configuration API stubs and deferred package enable/disable authority.
      - `docs/wiki/modules/primitive-architecture-docs.md`: Documented the configuration inventory flow and test coverage.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - Phase 16 analysis documents
  - Test Cases to Write:
    - `configuration_stubs_pass_coverage_gate`: Existing `cargo test` coverage gate verifies all new configuration stubs have required fields and no configuration API bypasses the documented init.js model.
    - `phase16_configuration_api_stubs_cover_reviewed_surfaces`: Static test verifies planned stubs cover package options, mode preferences, decoration themes, parse policies, `~/.config/clay/init.js` security, custom properties, and the deferred package enable/disable decision.
  - Verification:
    - `cargo test --test primitives_docs` — passed (24 tests; warnings only for pre-existing unused SDUI observability structs/methods).
    - `cargo test --test clay_js_api_inventory` — passed (14 tests; same pre-existing warnings).

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all Phase 16 implementation tasks are complete. Because Phase 16 produces only design documents and stub inventory entries (no new Rust code), wiki updates cover: the new `docs/reference/primitives/` directory structure, the primitive registry schema and categories, the rendering strategy, the parse update strategy, the Markdown mode requirements map, the package security model, and the new planned API stubs. The wiki master index links any new or updated pages.
    - Performance: Wiki updates add no runtime work. Performance-relevant design decisions (new budget constants, `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `MODE_ACTIVATION_P95_BUDGET_MS`) are documented in the wiki with their purpose and advisory status.
    - Code Quality: Wiki pages explain what each Phase 16 analysis document covers, why design choices were made, cross-references to authoritative docs in `docs/reference/primitives/`, and links from `docs/wiki/index.md`. Internal implementation design details (routing policies, parse task lifecycle, rendering attachment points) belong in the wiki, not in the public API reference.
    - Security: The wiki page covering the package security model (`package-security.md`) cross-references the canonical security document without duplicating it; security boundaries and prohibited authorities are stated with a reference to the authoritative source.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular but likely to churn as documents evolve during Phase 16.
      - Update once after all tasks pass: keeps wiki aligned with final design decisions.
    - Chosen Approach:
      - After all Phase 16 tasks complete, update `docs/wiki/index.md` and add or update wiki pages for the primitive architecture introduction, rendering strategy internals, parse task lifecycle, and package security model. Cross-reference public docs in `docs/reference/primitives/` rather than duplicating them.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md  — add links to new primitive architecture pages
      docs/wiki/modules/primitive-architecture.md  — new
      docs/wiki/modules/rendering-primitives.md    — new
      docs/wiki/modules/parse-task-lifecycle.md    — new
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add navigation links for new Phase 16 wiki pages.
      - `docs/wiki/modules/primitive-architecture.md`: New — overview of primitive registry design and category taxonomy.
      - `docs/wiki/modules/rendering-primitives.md`: New — rendering strategy internals, `DecorationUpdate` shape, SDUI vs. inline span split.
      - `docs/wiki/modules/parse-task-lifecycle.md`: New — background parse task coordination, cancellation, viewport filtering.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - All Phase 16 analysis documents
      - `docs/wiki/modules/performance-fixtures.md` (existing, for style reference)
  - Test Cases to Write:
    - Manual wiki review: Confirm `docs/wiki/index.md` links all three new wiki pages and each page explains its subject with accurate cross-references to authoritative `docs/reference/primitives/` documents.
  - Verification:
    - Manual wiki review — passed. `docs/wiki/index.md` links `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/rendering-primitives.md`, and `docs/wiki/modules/parse-task-lifecycle.md`; each page cross-references the authoritative `docs/reference/primitives/` documents and explains Phase 16 internals.
    - `rg -n "primitive-architecture\\.md|rendering-primitives\\.md|parse-task-lifecycle\\.md" docs/wiki/index.md docs/wiki/modules/primitive-architecture-docs.md docs/wiki/modules/primitive-architecture.md docs/wiki/modules/rendering-primitives.md docs/wiki/modules/parse-task-lifecycle.md` — passed.
    - `cargo test --test primitives_docs` — passed (24 tests; warnings only for pre-existing unused SDUI observability structs/methods).

## Compromises Made

- Phase 16 intentionally kept primitive package/mode work as architecture analysis, reference documentation, planned API inventory stubs, advisory budget constants, and wiki coverage only; no runtime package loader, parse coordinator, decoration protocol, client rendering hook, Clay JS facade implementation, or op wrapper was added.
- Code wiki pages link to canonical `docs/reference/primitives/` documents instead of duplicating every table and requirement, preserving the reference docs as the source of truth while documenting internal implementation flow and attachment points.

## Further Actions

- Priority high: Phase 17 should implement package manifest loading/validation, mode pattern registration, major mode activation, command registration, deterministic conflict handling, and permission validation from `docs/reference/primitives/backlog.md`.
- Priority high: Phase 18 should implement Markdown mode against the primitive readiness checklist, including background parse handling, viewport-bounded decoration publication, manifest-based list/code-block transforms, package-prefixed commands, key bindings, and SDUI preview/toggle UI.
- Priority medium: Promote advisory primitive budgets to concrete payload/latency guard tests only after Phase 17/18 introduce representative protocol messages, fixtures, and benchmarks.
