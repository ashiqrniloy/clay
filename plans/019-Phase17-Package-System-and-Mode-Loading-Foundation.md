# Phase 17: Package System and Mode Loading Foundation

## Objectives

- Make Clay load installable or local JavaScript packages that contribute documented modes, commands, configuration, SDUI, and behavior manifests through the Phase 16/16.5 primitive registry, without hard-coding mode-specific behavior in the Rust app.
- Define the Clay package manifest format, package identity/prefix, entry points, metadata, permissions, documented Clay JS API dependencies, and primitive contributions on top of the existing Phase 16.5 `ClayPackageManifest` validator.
- Separate package install (delegated to an npm-compatible manager), enable/load validation (Clay-owned), runtime execution (server-side `deno_core`), and load-time behavior contribution.
- Add per-document/per-mode behavior manifest selection with package provenance, plus deterministic conflict handling across enabled packages.
- Implement the Phase-18-required Markdown foundations that Phase 17 must leave ready: bounded `DecorationRange` transport/client render hook and the `IncrementalParseUpdate` server background parse coordinator, with `FoldingRange` only if Markdown POC scope promotes it.
- Preserve Clay's authority boundaries and hot-path rules: packages declare inert primitives, the server owns validation/activation/execution, the Rust client never executes package JavaScript, and ordinary typing/paint/scroll/text-event handlers never block on package work, synchronous IPC, or package-manager calls.

## Expected Outcome

- Clay can enable a validated local or installed package whose `package.json` Clay metadata declares identity, `apiPrefix`, permissions, modes, commands, configuration, SDUI/behavior contributions, and Clay JS API dependencies; enabling fails with actionable diagnostics when any Clay-owned rule is violated.
- A `clay package ...` CLI surface and one shared package-management service can install (via delegated npm-compatible manager), enable/disable, list, and inspect packages; installation and execution remain separate.
- A document can activate exactly one package-provided major mode that selects a validated per-document behavior manifest with package provenance; minor modes declare compatible major modes and cannot silently override major-mode behavior.
- Conflicts across enabled packages (prefixes, modes, command IDs, key bindings, configuration keys, SDUI regions, decoration/render primitives, behavior manifest entries) are detected deterministically with provenance-preserving diagnostics.
- The Phase-18 handoff is satisfied: a bounded, versioned, viewport-prioritized decoration protocol with a client render hook and a cancellable server-side parse coordinator exist, validated against `DECORATION_PAYLOAD_BUDGET_BYTES` and `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, with stale-version rejection and package provenance.
- Users and AI agents can inspect installed packages, modes, commands, key bindings, configuration options, permissions, performance metadata, primitive contributions, and any approved SDUI panel/query surfaces through Clay JS API Markdown docs, the generated registry, and app/help/agent lookup.
- `cargo test` fails when packages omit required manifest fields, permission declarations, mode declarations, runtime/load-time separation, docs, registry entries, primitive metadata, performance metadata, or conflict metadata.

## Tasks

- [x] Define the package manifest format, package contract, and enable/load validator
  - Acceptance Criteria:
    - Functional: A server-side package contract validator extends the Phase 16.5 `ClayPackageManifest` to a full enable/load package record that accepts package identity, `clay.apiPrefix`, entry/loadEntry, `clay.permissions`, `clay.modes`, declared Clay JS API dependencies, primitive contribution descriptors (commands, key routing, text transforms, SDUI/status, configuration), documentation metadata path, and performance metadata; it rejects malformed/incomplete packages with structured diagnostics including package name/version/prefix, contribution ID, and failed rule.
    - Performance: Enable/load validation runs only at install/enable/reload time, never in keypress/paint/layout/scroll/text-event paths; static contribution payloads are checked against existing budget constants in `src/perf/budgets.rs`.
    - Code Quality: The package record/types live in `src/packages/` with typed structs, deterministic error variants, no package-manager process execution, and reuse of the existing manifest/permission/mode/command validators rather than duplicating them.
    - Security: Reject unknown permissions, reserved/first-party `clay.*` IDs claimed by packages, invalid/duplicate prefixes, raw `Deno.core.ops` exposure metadata, client-side JavaScript hooks, and the prohibited default authorities listed in `package-security.md`.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md`: Package identity/prefix/permission/provenance, server-side validation checklist, prohibited authorities.
      - `docs/reference/primitives/implementation-gate.md` and `docs/reference/primitives/backlog.md`: Phase 16.5 validated primitive contract and Phase 17 prerequisite checklist to reuse, not re-derive.
      - `src/packages/manifest.rs`, `src/packages/permissions.rs`, `src/packages/modes.rs`, `src/packages/commands.rs`: Existing typed validators to extend.
      - `.agents/skills/project-patterns/references/package-distribution.md`: npm-compatible distribution, install vs enable vs runtime separation, package prefix provenance.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md` and `clay-js-api-naming.md`: Package API prefix provenance and facade/op boundary.
      - Context7 pnpm docs (`/websites/pnpm_io`, fetched 2026-06-02): `package.json` is the standard manifest; `clay.*` is an additive namespaced section.
    - Options Considered:
      - One monolithic validator function: simpler but hard to test per-contribution-category and conflicts with existing modular validators.
      - Compose existing per-primitive validators behind a package record assembler: preferred; reuses Phase 16.5 validation invariants and keeps deterministic per-category diagnostics.
      - Define a new bespoke manifest schema unrelated to `package.json`: rejected; violates the npm-compatible distribution decision.
    - Chosen Approach:
      - Add a `PackageRecord`/`PackageContract` assembler in `src/packages/` that parses a `package.json`-shaped value, reuses `validate_manifest_value`, permission parsing, mode declarations, and command declarations, then validates declared primitive contribution descriptors and documentation/performance metadata into one typed enable/load record with provenance retained on every accepted contribution.
    - API Notes and Examples:
      ```json
      {
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
          "apiPrefix": "markdown",
          "entry": "./dist/index.js",
          "loadEntry": "./dist/load.js",
          "permissions": ["mode-registration", "mode-activation", "command-registration"],
          "modes": ["markdown"],
          "docs": "./docs/index.md",
          "apiDependencies": ["clay.modes.serverRegisterModePattern", "clay.commands.serverRegisterCommand"],
          "contributions": {
            "commands": [{ "id": "markdown.togglePreview", "displayName": "Toggle Markdown Preview", "routingPolicy": "server-first" }],
            "configuration": [{ "key": "markdown.preview.enabled", "type": "boolean", "default": false }]
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `src/packages/record.rs`: New package enable/load record, contribution descriptors, assembler, and error variants.
      - `src/packages/mod.rs`: Export the package record module.
      - `src/packages/manifest.rs`: Add any shared parsing helpers needed by the record assembler without changing Phase 16.5 behavior.
      - `tests/package_loading.rs`: New enable/load record validation coverage.
    - References:
      - `docs/reference/primitives/package-security.md`
      - `docs/reference/primitives/backlog.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
  - Test Cases to Write:
    - `package_record_accepts_full_markdown_contract`: a complete Markdown-style package record validates with provenance retained.
    - `package_record_rejects_missing_required_contract_fields`: missing entry, permissions, modes, docs, or performance metadata fails with actionable per-field diagnostics.
    - `package_record_rejects_package_claiming_clay_reserved_ids`: package-owned contribution IDs claiming `clay.*` are rejected.
    - `package_record_rejects_undeclared_permission_for_contribution`: a contribution requiring a permission not declared in `clay.permissions` fails before enable.

- [x] Implement the package install/enable/disable service and `clay package` CLI over a delegated package manager
  - Acceptance Criteria:
    - Functional: One shared package-management service supports install (delegating fetch/resolution/lockfile/integrity/caching to an npm-compatible manager), enable/load (Clay-owned validation), disable, list, and inspect; `clay package add|remove|list|enable|disable|inspect` CLI subcommands route through the same service. Install records a package without executing its runtime; enable validates the Clay package record before any server-side execution.
    - Performance: Install/enable/disable are explicit user/agent operations off the editing hot path; no package-manager process is spawned during typing/paint/scroll/text-event handling.
    - Code Quality: Package-manager invocation is isolated behind a typed boundary that captures stdout/exit codes and parses machine-readable output; the service does not embed editor state and returns typed results/diagnostics.
    - Security: Clay never grants packages filesystem/network/shell/AI authority by virtue of being installed; the package manager process runs with the user's environment and Clay only validates Clay-owned metadata; enable/disable mutation is an explicit privileged operation, not a configuration side effect.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-distribution.md`: `clay package ...` CLI + in-app UI backed by one shared service; delegate to npm-compatible manager; install vs enable vs runtime separation.
      - `docs/reference/primitives/package-security.md`: Install/execution separation, prohibited authorities, enable/disable mutation as privileged.
      - `src/main.rs` and existing launch/command parsing: Where to add the `package` subcommand surface.
      - Context7 pnpm docs (`/websites/pnpm_io`, fetched 2026-06-02): `pnpm add <pkg>`, `pnpm i --lockfile-only`, `pnpm i --frozen-lockfile`, `pnpm list --json`/`--long` for machine-readable installed metadata.
    - Options Considered:
      - Embed a Rust npm client: rejected; the decision delegates fetch/resolution/lockfile/integrity to an existing manager.
      - Shell out to a hard-coded `npm`: workable but less aligned with the pnpm-preferred direction and harder to get machine-readable output.
      - Delegate to a configurable npm-compatible manager (pnpm preferred) behind a typed `PackageManager` boundary using `pnpm add`/`pnpm list --json`: preferred; matches the decision and yields parseable metadata.
    - Chosen Approach:
      - Add a `PackageManagerBackend` trait with a pnpm-first implementation that runs install via `pnpm add` (and lockfile/frozen variants where appropriate) into a Clay-managed package store, lists installed packages via `pnpm list --json`, and feeds discovered `package.json` values into the Phase-17 enable/load validator. Add a `PackageService` that owns install/enable/disable/list/inspect and an `enabled` state record. Wire `clay package ...` subcommands to the service.
    - API Notes and Examples:
      ```bash
      clay package add @clay/markdown
      clay package list
      clay package enable @clay/markdown
      clay package inspect @clay/markdown
      clay package disable @clay/markdown
      clay package remove @clay/markdown
      ```
      ```bash
      # delegated package-manager calls (pnpm preferred)
      pnpm add @clay/markdown
      pnpm list --json --long
      pnpm i --frozen-lockfile
      ```
    - Files to Create/Edit:
      - `src/packages/service.rs`: New `PackageService` with install/enable/disable/list/inspect and enabled-state record.
      - `src/packages/manager.rs`: New `PackageManagerBackend` trait and pnpm-first implementation with typed process boundary.
      - `src/packages/mod.rs`: Export service and manager modules.
      - `src/main.rs`: Add `package` subcommand parsing routed to `PackageService`.
      - `tests/package_loading.rs`: Service enable/disable/list/inspect coverage with a fake `PackageManagerBackend`.
    - References:
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `docs/reference/primitives/package-security.md`
      - `src/main.rs`
  - Test Cases to Write:
    - `package_service_install_records_without_executing_runtime`: install records a package and does not run its runtime entry.
    - `package_service_enable_rejects_invalid_clay_metadata`: enable fails for a package whose Clay record is invalid, with actionable diagnostics.
    - `package_service_disable_removes_active_contributions`: disabling removes the package's contributions and frees its prefix/mode/command IDs.
    - `package_cli_subcommands_route_through_shared_service`: CLI subcommands invoke the same service paths as the in-app surface (fake backend).

- [x] Implement per-document/per-mode behavior manifest selection with package provenance and major/minor mode rules
  - Acceptance Criteria:
    - Functional: When a package-provided major mode activates for a document, the server composes and selects a validated per-document behavior manifest from the active mode's package-owned `KeyRoutingOverride`/`TextTransform` contributions plus base behavior, atomically installs it for the document's client, and records package/mode/behavior-version provenance. A document has at most one active major mode; minor modes declare compatible major modes and are rejected when incompatible and may not override major-mode entries.
    - Performance: Manifest selection/composition happens at mode activation/reload only; client keypress routing remains manifest-based and never calls JavaScript synchronously, preserving `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`.
    - Code Quality: Per-document manifest state is server-owned and keyed by document ID and behavior version; composition reuses existing `src/behavior/manifest.rs` validation and the Phase 16.5 command/mode registries.
    - Security: Composed manifests are inert and server-validated; no package JavaScript is delivered to the client, and minor-mode composition cannot silently replace major-mode behavior.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/registry.md`: `MajorModeActivation`, `MinorModeActivation`, `KeyRoutingOverride`, `TextTransform` entries.
      - `docs/reference/primitives/package-security.md`: Conflict handling table and per-document provenance requirements.
      - `src/behavior/manifest.rs`: Existing manifest validation, ambiguous-keybinding checks, behavior versioning.
      - `src/packages/modes.rs`, `src/packages/commands.rs`: Phase 16.5 mode/command registries and one-major-mode state.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Inert manifest, routing policy vocabulary, atomic per-document install.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns canonical behavior; client owns only execution of validated manifests.
    - Options Considered:
      - Keep one global behavior manifest and switch it on focus: rejected; cannot represent concurrent documents in different modes.
      - Add per-document manifest selection keyed by active major mode with minor-mode overlay composition: preferred; matches roadmap per-document/per-mode requirement and existing single-major-mode state.
      - Implement full minor-mode conflict policy now: scope to compatibility declaration + non-override enforcement; richer overlay precedence remains deferred unless Markdown POC needs it.
    - Chosen Approach:
      - Extend the mode registry to map an active major mode (and any compatible minor modes) to package-owned manifest contributions, compose them into an inert candidate, validate via the existing manifest validator, assign a per-document behavior version, and record the selected manifest per document ID with provenance. Reject incompatible/overriding minor modes deterministically.
    - API Notes and Examples:
      ```rust
      let selection = mode_registry.select_behavior_manifest_for_document(
          document_id,
          &enabled_packages,
      )?; // returns inert validated manifest + behavior version + provenance
      ```
    - Files to Create/Edit:
      - `src/packages/modes.rs`: Add per-document manifest selection, minor-mode compatibility/non-override enforcement, and provenance metadata.
      - `src/behavior/manifest.rs`: Add composition helpers if needed without changing existing validation semantics.
      - `tests/package_loading.rs`: Per-document selection, minor-mode compatibility, and provenance tests.
    - References:
      - `docs/reference/primitives/registry.md`
      - `docs/reference/primitives/package-security.md`
      - `src/behavior/manifest.rs`
  - Test Cases to Write:
    - `behavior_manifest_selected_per_document_with_provenance`: two documents in different modes get distinct validated manifests with correct provenance/version.
    - `minor_mode_rejected_when_incompatible_major_mode`: incompatible minor mode fails with actionable diagnostics.
    - `minor_mode_cannot_override_major_mode_entries`: overlay attempting to replace a major-mode entry is rejected.
    - `keypress_routing_uses_manifest_without_javascript`: composed manifest routing does not invoke server JavaScript on keypress (regression against hot-path rule).

- [x] Implement deterministic cross-package conflict handling and SDUI/status contribution provenance
  - Acceptance Criteria:
    - Functional: Enabling packages detects and reports deterministic conflicts for duplicate prefixes, duplicate mode names, duplicate command IDs, ambiguous key bindings, configuration-key collisions, SDUI region/slot collisions, decoration/render primitive collisions, and behavior manifest entry collisions, following the `package-security.md` conflict table. Inert package SDUI panel/status contributions are validated with package region/provenance metadata and bounded payloads.
    - Performance: Conflict checks run at enable/reload time; SDUI contribution payloads are checked against `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.
    - Code Quality: Conflict detection is a single deterministic pass over enabled-package contribution indices producing typed, provenance-preserving diagnostics; SDUI provenance reuses the existing SDUI validation path.
    - Security: Conflicts never silently override behavior; SDUI actions targeting commands inherit command permissions; no package widget code reaches the client.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md`: Conflict handling table and SDUI region rules.
      - `docs/reference/primitives/registry.md`: `SduiPanelStatusContribution` entry.
      - `src/server/ops/sdui.rs` and existing SDUI validation: Where to add package region/provenance metadata.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Deltas, bounded payloads, no hot-path package work.
    - Options Considered:
      - Detect conflicts lazily at first use: rejected; non-deterministic and unsafe.
      - Detect all conflicts at enable time across the enabled set: preferred; deterministic and produces load-time diagnostics.
    - Chosen Approach:
      - Build enabled-package contribution indices (prefix, mode, command ID, key binding, config key, SDUI region, decoration kind, manifest entry) and run a deterministic conflict pass on enable/reload, returning provenance-preserving diagnostics per the security table. Extend SDUI validation to attach package region/provenance and enforce budgets.
    - API Notes and Examples:
      ```rust
      let report = conflict::check_enabled_packages(&enabled)?; // Err carries package prefixes + conflict kind
      ```
    - Files to Create/Edit:
      - `src/packages/conflict.rs`: New deterministic conflict detection over enabled-package indices.
      - `src/packages/record.rs`: Added key-routing conflict metadata, decoration descriptors, and SDUI/status provenance plus snapshot/update budget validation.
      - `src/packages/service.rs`: Run conflict checks during enable and roll back failed candidate enables.
      - `src/packages/mod.rs`: Export the conflict module.
      - `tests/package_loading.rs`: Conflict and SDUI provenance/budget tests.
      - `docs/wiki/index.md`, `docs/wiki/modules/package-loading.md`: Document implementation details for the completed package-loading conflict/provenance work.
    - References:
      - `docs/reference/primitives/package-security.md`
      - `docs/reference/primitives/registry.md`
  - Test Cases to Write:
    - `enable_rejects_duplicate_prefix_and_mode_and_command`: each duplicate kind fails deterministically with both packages' provenance.
    - `ambiguous_keybinding_across_packages_rejected_without_priority`: ambiguous bindings without declared priority are rejected.
    - `package_sdui_contribution_carries_provenance_and_respects_budget`: SDUI/status contribution retains provenance and fails when exceeding the SDUI payload budget.

- [x] Implement the bounded decoration transport and client render hook (Phase-18 handoff: `DecorationRange`)
  - Acceptance Criteria:
    - Functional: A new bounded, versioned protocol message carries package-provided inert decoration spans (byte range, kind, style token, priority, package provenance) for a document/viewport; the server validates them and the client renders validated decorations through an editor render hook outside paint-time package code. Stale-document-version decorations are rejected.
    - Performance: Decoration payloads are bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`, viewport-prioritized, and never block typing/paint; the client render hook applies validated spans without running package JavaScript in paint/text-event handlers.
    - Code Quality: Decoration types are typed and `rkyv`-serializable behind the existing codec boundary; server validation is reused at edit-time only for cheap version/range/payload checks.
    - Security: Spans are inert data with known kind/style fields; `render-decorations` permission is required; no draw callbacks, widget handles, or client JavaScript.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/registry.md` (`DecorationRange`), `docs/reference/primitives/rendering-strategy.md`, `docs/reference/primitives/parse-update-strategy.md`: Bounded inert spans, viewport prioritization, versioning.
      - `docs/reference/primitives/package-security.md`: `render-decorations` permission and prohibited authorities.
      - `src/perf/budgets.rs`: `DECORATION_PAYLOAD_BUDGET_BYTES`.
      - Existing protocol/codec modules and editor paint path: Where the bounded message and render hook attach.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Deltas, per-document ordering, viewport-bounded rendering.
    - Options Considered:
      - Inline decorations into the behavior manifest: rejected; decorations are per-viewport render data, not hot-path routing rules.
      - Add a dedicated bounded decoration protocol message with a client render hook: preferred; matches rendering-strategy and keeps paint package-JS-free.
    - Chosen Approach:
      - Add a typed `DecorationSet`/`DecorationSpan` protocol message validated server-side (bounds, version, permission, provenance) and a client-side render hook that stores validated decorations keyed by document/version and applies them during normal editor rendering without invoking package code.
    - API Notes and Examples:
      ```rust
      // server -> client, viewport-prioritized, version-checked
      struct DecorationSpan { start: usize, end: usize, kind: DecorationKind, style_token: String, priority: u16 }
      ```
    - Files to Create/Edit:
      - `src/protocol/decorations.rs`, `src/protocol/mod.rs`: New bounded `DecorationSet`/`DecorationSpan` message behind the codec boundary.
      - `src/server/decorations.rs`, `src/server/mod.rs`: Server-side validation/publication helper with permission, version, range, provenance, viewport, and payload-budget checks.
      - `src/client/mod.rs`, `src/masonry_editor.rs`, `src/editor/surface.rs`, `src/editor/layout.rs`: Validated decoration event, store, and native render hook (no package JS in paint).
      - `tests/decoration_transport.rs`: New decoration transport/validation/render-hook coverage.
      - `docs/wiki/index.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/rendering-primitives.md`, `docs/wiki/modules/protocol-codec.md`: Code wiki updates for the implemented transport/render hook.
    - References:
      - `docs/reference/primitives/rendering-strategy.md`
      - `docs/reference/primitives/package-security.md`
      - `src/perf/budgets.rs`
  - Test Cases to Write:
    - `decoration_payload_rejected_when_exceeding_budget`: oversized decoration set fails validation.
    - `decoration_rejected_for_stale_document_version`: stale-version spans are rejected.
    - `decoration_render_hook_applies_validated_spans_without_package_js`: client render hook applies validated spans and never executes package JavaScript.

- [x] Implement the server-side background parse coordinator (Phase-18 handoff: `IncrementalParseUpdate`)
  - Acceptance Criteria:
    - Functional: A server-side parse coordinator accepts package-registered parse handlers, schedules cancellable background parse tasks per document with viewport-prioritized scheduling, applies stale-version rejection, and publishes bounded incremental parse results that downstream decoration/folding can consume. Parse handler registration requires `parse-document` permission.
    - Performance: Parsing runs asynchronously on a background lane separate from edit acknowledgement; results are bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`; ordinary typing/local paint never wait on parse completion.
    - Code Quality: The coordinator is isolated (`src/server/parse_coordinator.rs`), uses per-document ordering and cancellation tokens, and exposes a typed handler registration boundary; no parser reaches the client.
    - Security: Parser execution is server-side only and may read only Clay-provided open-document content/metadata; no filesystem/network/shell/AI/WASM/raw-op/client-JS authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/parse-update-strategy.md`, `docs/reference/primitives/registry.md` (`IncrementalParseUpdate`): Cancellable, viewport-prioritized, bounded, version-checked parse results.
      - `docs/reference/primitives/package-security.md`: `parse-document` permission and server-side-only execution.
      - `src/perf/budgets.rs`: `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.
      - `.agents/skills/rust-async-patterns/SKILL.md`: Tokio cancellation, background task lanes, per-document ordering.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Cancellable background work prioritized separately from background.
    - Options Considered:
      - Parse synchronously on edit acknowledgement: rejected; violates hot-path and acknowledgement-latency rules.
      - Add a cancellable background parse coordinator with viewport-prioritized scheduling: preferred; matches parse-update-strategy and async patterns.
    - Chosen Approach:
      - Implement `parse_coordinator.rs` with a per-document task model: register handler (permission-checked), enqueue parse on edit/viewport change, cancel superseded tasks, reject stale versions, and emit bounded incremental results consumable by the decoration path. Keep handler execution server-side via the runtime boundary.
    - API Notes and Examples:
      ```rust
      coordinator.register_handler(package_prefix, mode_id, handler_meta)?; // requires parse-document
      coordinator.schedule_parse(document_id, version, viewport); // cancellable, prioritized
      ```
    - Files to Create/Edit:
      - `src/server/parse_coordinator.rs`: New background parse coordinator.
      - `src/protocol/parse.rs`: New `rkyv`-serializable `ParseEditNotification`/`IncrementalParseUpdate` shapes.
      - `src/protocol/mod.rs`: Export parse protocol shapes without adding them to hot edit-ack IPC.
      - `src/server/mod.rs`: Wire the coordinator into server state.
      - `tests/parse_coordinator.rs`: New coordinator scheduling/cancellation/version/budget coverage.
      - `tests/rust_visibility_api_mapping.rs`: Allowlist the coordinator as non-JS server infrastructure until the later facade/API inventory task promotes or keeps APIs planned.
      - `docs/wiki/index.md`, `docs/wiki/modules/parse-task-lifecycle.md`, `docs/wiki/modules/parse-coordinator.md`: Document the implemented coordinator and link it from the wiki.
    - References:
      - `docs/reference/primitives/parse-update-strategy.md`
      - `docs/reference/primitives/package-security.md`
      - `.agents/skills/rust-async-patterns/SKILL.md`
  - Test Cases to Write:
    - `parse_handler_registration_requires_parse_permission`: registration without `parse-document` fails.
    - `superseded_parse_task_is_cancelled`: a newer edit cancels an in-flight parse for the same document.
    - `parse_result_rejected_for_stale_version_and_oversized_payload`: stale-version and over-budget results are rejected.
    - `stale_parse_result_is_not_published`: superseded stale results are dropped before downstream publication.
    - `parsing_does_not_block_edit_acknowledgement`: edit acknowledgement is not delayed by background parsing.
  - Verification:
    - `cargo test --test parse_coordinator --test performance_protocol` passed.
    - `cargo test` passed. During implementation, full-suite runs exposed an edit-ack payload regression from adding parse updates to `ServerMessage`; the parse update was kept server-side and `performance_protocol` passed. A later full run exposed the expected public-Rust visibility inventory gate, so the coordinator was allowlisted as server infrastructure pending the facade/API inventory task.

- [x] Wire package-loading runtime facades and add package/primitive verification coverage
  - Acceptance Criteria:
    - Functional: `clay:packages`, `clay:modes`, `clay:commands`, and the new decoration/parse facade surfaces expose package enable/load, mode activation, per-document manifest selection, decoration publication, and parse-handler registration as load/configuration-time Clay JS APIs (implemented or explicitly planned with actionable unavailable errors); the controlled server-side runtime can import them and route fixtures through the typed validators. A public reference document explains Phase 17 package loading scope, the install/enable/runtime boundary, conflict handling, and the Phase-18 handoff.
    - Performance: Facade calls are load/activation/configuration-time only and do not create a synchronous keypress-to-JavaScript path; verification includes static checks that package validation, mode activation, decoration, and parse work are documented outside typing-hot-path handlers.
    - Code Quality: User-facing exports never expose raw `Deno.core.ops.op_*`; tests fail when package-loading docs, primitive backlog handoff, or docs-index links go stale.
    - Security: Runtime calls cannot bypass typed Rust validators, request undeclared permissions, or expose filesystem/network/shell/AI/package-install authority to the client.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime.rs`, `src/server/ops/packages.rs|modes.rs|commands.rs`, `src/server/ops/planned.rs`: Existing facade loader and planned-op pattern.
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/backlog.md`, `docs/reference/primitives/implementation-gate.md`: Where to add the Phase 17 loading reference and update the prerequisite checklist.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`: Facade/op boundary and package-prefix provenance.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`, `doc-registry-tests.md`: Prefer automated freshness checks with actionable failures.
    - Options Considered:
      - Document loading only in the plan: rejected; not discoverable by Phase 18 implementers or AI agents.
      - Add a concise reference page + runtime facade wiring + verification tests: preferred and testable.
    - Chosen Approach:
      - Extend the runtime facade modules and ops to cover package enable/load, per-document manifest selection, decoration publication, and parse-handler registration (planned where deferred), add `docs/reference/primitives/package-loading.md` linked from the primitives and docs indexes, update the backlog checklist to mark Phase-17 prerequisites satisfied, and add verification tests for terms/links/hot-path policy.
    - API Notes and Examples:
      ```javascript
      import { serverEnablePackage } from "clay:packages";
      import { serverPublishDecorations } from "clay:decorations";
      import { serverRegisterParseHandler } from "clay:parse";
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime.rs`: Add package-loading/decoration/parse facade sources and runtime import/error coverage.
      - `src/server/ops/mod.rs`, `src/server/ops/packages.rs`: Register package enable/load and selection ops (or planned routes).
      - `runtime/js/packages.ts`, `runtime/js/modes.ts`, `runtime/js/decorations.ts`, `runtime/js/parse.ts`, `runtime/js/mod.ts`: Mirror runtime facade exports for source-tree verification.
      - `docs/reference/primitives/package-loading.md`: New Phase 17 package loading reference.
      - `docs/reference/primitives/index.md`, `docs/index.md`, `docs/reference/primitives/backlog.md`: Link the reference and update the prerequisite checklist.
      - `docs/wiki/modules/package-loading.md`, `docs/wiki/modules/clay-js-facade-skeleton.md`: Code wiki updates for package-loading runtime facade wiring.
      - `tests/package_loading_docs.rs`, `tests/clay_js_facade_layout.rs`: New docs/index/handoff/hot-path verification and facade source layout coverage.
    - References:
      - `src/server/js_runtime.rs`
      - `docs/reference/primitives/backlog.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - `runtime_imports_package_loading_facades`: controlled runtime imports the loading/decoration/parse facade modules and routes valid fixtures through typed validators.
    - `package_loading_doc_linked_from_indexes_and_marks_phase17_ready`: static test verifies index links and updated prerequisite checklist.
    - `package_loading_keeps_validation_and_parsing_out_of_typing_hot_path`: static test verifies docs keep enable/activation/decoration/parse off typing/paint/text-event paths.
    - `phase18_only_apis_remain_planned_or_documented`: decoration/parse/folding facade surfaces are either implemented for the handoff or explicitly planned without accidental raw-op exposure.
  - Verification:
    - `cargo test runtime_imports_modes_commands_and_packages_facades --lib` passed.
    - `cargo test phase18_primitive_facades_remain_explicitly_planned --lib` passed.
    - `cargo fmt` completed.
    - `cargo test --test package_loading --test package_loading_docs --test clay_js_facade_layout --test primitives_docs` passed.
    - `cargo test` passed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Review all package-loading behavior and server-side Rust public functions introduced or changed by this plan. Each public programmatic capability is exposed through a stable Clay JS facade API and explicit `deno_core` op wrapper, or the Rust item is private/`pub(crate)` with a documented reason. Update `api-inventory.toml` statuses/docs paths for implemented package-loading, mode-selection, decoration, and parse APIs; keep any still-deferred Phase-18 APIs planned.
    - Performance: API docs/inventory work adds no hot-path work; implemented entries identify load/activation/configuration-time routing, not typing-hot-path execution.
    - Code Quality: API IDs, modules, exports, `user_facing_name`, key bindings, custom properties, permissions, security notes, op paths, facade paths, and docs paths satisfy the Clay JS API schema and naming patterns.
    - Security: APIs do not expose raw `Deno.core.ops`, filesystem/network/shell/AI authority, or client-side JavaScript; permission-bearing APIs (`mode-registration`, `mode-activation`, `command-registration`, `render-decorations`, `parse-document`, `package-configuration`) document their required scopes.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`, `clay-js-api-naming.md`, `clay-js-api-boundary.md`: Inventory/doc metadata, naming, facade/op boundary.
      - `docs/reference/clay-js-api/api-inventory.toml`: Existing Phase 16/16.5 stubs and statuses.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`, `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`.
    - Options Considered:
      - Document every planned Phase 18 API as implemented now: premature where implementation remains deferred.
      - Update only Phase-17 implemented APIs and the implemented Phase-18 handoff APIs, keeping the rest planned: preferred.
    - Chosen Approach:
      - After implementation stabilizes, reconcile `api-inventory.toml`, Markdown API docs, docs-index links, the generated registry, and tests for implemented package-loading, mode-selection, decoration, and parse APIs; regenerate the registry with the project command and keep non-implemented APIs as planned stubs with no raw-op public surface.
    - API Notes and Examples:
      ```text
      clay.packages.serverEnablePackage
      clay.packages.serverListPackages
      clay.modes.serverSelectDocumentManifest
      clay.decorations.serverPublishDecorations
      clay.parse.serverRegisterParseHandler
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: Update package-loading/mode/decoration/parse API statuses, op/facade paths, permissions, and security notes.
      - `docs/reference/clay-js-api/packages/*.md`, `modes/*.md`, `decorations/*.md`, `parse/*.md`: Add/verify implemented API docs.
      - `runtime/js/packages.ts`, `modes.ts`, and new `decorations.ts`/`parse.ts`: Static facade skeletons mirroring runtime facades.
      - `docs/index.md`: Link new public API docs.
      - `docs/generated/clay-js-api-registry.json`: Regenerate with `cargo run --bin update-doc-registry`.
      - `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/rust_visibility_api_mapping.rs`: Add package-loading API inventory/docs/registry/mapping coverage.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
  - Test Cases to Write:
    - `api_inventory_package_loading_entries_are_implemented_or_planned`: statuses match runtime implementation scope.
    - `clay_js_api_docs_cover_package_loading_surfaces`: docs/index/registry coverage for implemented public APIs.
    - `rust_visibility_mapping_has_no_unmapped_public_package_functions`: public Rust/op/facade package-loading capabilities are mapped in the inventory.
  - Verification:
    - Promoted `clay.packages.serverLoadPackage` from planned to runtime-backed in `api-inventory.toml`, documented it under `docs/reference/clay-js-api/packages/server-load-package.md`, linked it from `docs/index.md`, and regenerated `docs/generated/clay-js-api-registry.json` with `cargo run --bin update-doc-registry`.
    - Added a planned inventory record for `clay.modes.serverSelectDocumentManifest` that maps the implemented Rust selector while keeping the public JS facade unavailable until a later op promotion; kept `clay.decorations.serverPublishDecorations`, `clay.parse.serverRegisterParseHandler`, and folding planned.
    - Updated API inventory, registry, and Rust visibility tests plus code wiki notes for the package-loading facade state.
    - `cargo test --test clay_js_api_inventory --test clay_js_doc_registry --test rust_visibility_api_mapping --test clay_js_facade_layout --test package_loading_docs` passed.
    - `cargo test` passed.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review package-loading behavior for user-visible configuration surfaces (package options, mode preferences, decoration theme/style-token preferences, parse policy, and any package-owned SDUI panel visibility/layout settings). Implement or preserve planned configuration APIs through `~/.config/clay/init.js`; if package-owned SDUI regions make panel visibility/layout user-facing, add documented Clay JS configuration APIs with `user_facing_name`, key bindings, custom properties, `docs/index.md` links, registry entries, and coverage tests; otherwise record the deferral explicitly. Decide whether package tooling/help/agent workflows need a `clay:sdui.queryUiState` API; if so define it with full docs/registry/tests, otherwise keep `SduiObservableSnapshot`/`SduiStatusObservation` internal with inventory tests.
    - Performance: Configuration APIs run at configuration/package load or explicit setting change time, never on the typing hot path.
    - Code Quality: Configuration API docs/inventory entries include `custom_properties` with type/default/allowed values for behavior-changing settings.
    - Security: Configuration cannot implicitly grant filesystem, network, shell, extension loading, package enable/disable mutation, AI mutation, or workspace authority; package enable/disable stays a privileged service operation, not a config side effect.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: `~/.config/clay/init.js` configuration-as-API.
      - `docs/reference/primitives/registry.md` (`PackageOwnedConfiguration`, `SduiPanelStatusContribution`): Package configuration and SDUI surfaces.
      - Phase 15/16 carried-forward `clay:sdui.queryUiState` go/no-go note in `roadmap.md`.
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
    - Options Considered:
      - Add package enable/disable as configuration: rejected; security model keeps enable/disable a privileged operation.
      - Implement concrete package/mode/decoration settings only where Phase 17 introduces real behavior-changing options, keeping others planned: preferred.
    - Chosen Approach:
      - Implement configuration APIs for any concrete behavior-changing package/mode/decoration settings introduced by Phase 17; otherwise keep `setPackageOption`/`setModePreference`/`setDecorationTheme`/`setParsePolicy` planned with documented custom properties, init.js scope, and server-validation requirements. Record the `clay:sdui.queryUiState` and SDUI panel/layout configuration go/no-go decisions explicitly.
    - API Notes and Examples:
      ```javascript
      import { setModePreference, setDecorationTheme } from "clay:configuration";
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: Update/verify package configuration API status, custom properties, init.js scope, and enable/disable deferral note.
      - `runtime/js/configuration.ts`: Add/verify facade exports for reviewed package/mode/decoration configuration setters.
      - `docs/reference/clay-js-api/configuration*`: Add/verify docs for any implemented configuration APIs.
      - `tests/clay_js_facade_layout.rs`, `tests/primitives_docs.rs`, `tests/clay_js_api_inventory.rs`: Configuration facade/metadata/authority coverage and `clay:sdui.queryUiState` go/no-go record.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/primitives/package-security.md`
  - Test Cases to Write:
    - `phase17_configuration_apis_cover_reviewed_package_surfaces`: documented configuration surfaces, custom properties, init.js/server-validation metadata, no hot-path work, and enable/disable deferral.
    - `sdui_query_ui_state_decision_is_recorded`: test asserts the explicit go/no-go outcome (implemented-with-docs or kept-internal-with-inventory-coverage).
    - `package_configuration_cannot_grant_prohibited_authority`: configuration APIs cannot grant filesystem/network/shell/enable-disable/AI/workspace authority.
  - Verification:
    - Verified Phase 17 added no concrete user-facing package/mode/decoration/parse configuration setting requiring a promoted public registry API; preserved `setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as planned `clay:configuration` facade/inventory entries with custom-property metadata and planned-unavailable runtime behavior.
    - Recorded package enable/disable as a privileged package service/CLI operation rather than configuration, and recorded `clay:sdui.queryUiState` as deferred while `SduiObservableSnapshot`/`SduiStatusObservation` remain internal.
    - Updated `docs/reference/clay-js-api/configuration.md`, `tests/clay_js_api_inventory.rs`, and `docs/wiki/modules/clay-js-facade-skeleton.md` to cover the Phase 17 configuration review.
    - `cargo fmt` completed.
    - `cargo test --test clay_js_api_inventory --test clay_js_facade_layout --test primitives_docs` passed.
    - `cargo test` passed.

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
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and pages for the package contract/record, package service/CLI/manager boundary, per-document manifest selection, conflict handling, decoration transport/render hook, parse coordinator, and runtime facades.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/package-loading.md
      docs/wiki/modules/parse-coordinator.md
      docs/wiki/modules/decoration-transport.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/modules/package-loading.md`: New/update package contract, service, CLI, manager-boundary, conflict handling, and per-document manifest selection page.
      - `docs/wiki/modules/decoration-transport.md`: New/update decoration protocol and client render hook page.
      - `docs/wiki/modules/parse-coordinator.md`: New/update background parse coordinator page.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Update runtime facade notes for package-loading/decoration/parse surfaces.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
    - Regression: `cargo test --test package_loading` and `cargo test --test package_loading_docs` pass after the wiki update.
  - Verification:
    - Reviewed `docs/wiki/index.md`, `docs/wiki/modules/package-loading.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/parse-coordinator.md`, and `docs/wiki/modules/clay-js-facade-skeleton.md` against the final implementation state.
    - Updated `docs/wiki/modules/package-loading.md` to cover the package manager boundary, `clay package` CLI routing, `PackageService` enable rollback, per-document major/minor manifest selection, hot-path constraints, and install-vs-enable authority boundaries.
    - Updated `docs/wiki/index.md`, `docs/wiki/modules/rendering-primitives.md`, `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/parse-task-lifecycle.md`, `docs/wiki/modules/primitive-architecture-docs.md`, and `docs/wiki/modules/clay-js-facade-skeleton.md` to remove stale wording and reflect implemented decoration transport, parse protocol shapes, and the promoted `serverLoadPackage` registry entry.
    - Confirmed the master wiki index already links the relevant package-loading, decoration, parse, facade, protocol, and rendering pages.
    - `cargo test --test package_loading --test package_loading_docs` passed.

## Compromises Made

- Phase 18 provider-facing decoration/parse public APIs remain planned/unavailable even though the Rust decoration transport and parse coordinator handoff foundations exist; `clay.packages.serverLoadPackage` is the Phase 17 package-loading API promoted to the generated registry.
- Package installation delegates to pnpm through a typed process boundary and does not implement an embedded npm client; this preserves the install/enable/runtime separation but depends on the user's package-manager environment for actual fetch/remove operations.

## Further Actions

- Phase 18 should promote the planned decoration and parse provider APIs only after their public op/facade contracts are finalized, then update the Clay JS inventory, generated registry, and wiki pages again.
- Persist and share package enable/disable state beyond the current in-memory service so the CLI, future in-app UI, and server runtime can observe the same package store state across processes.
