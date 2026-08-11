# Phase 18.9 Generic Text/Code Modes, Key Behavior, and Mode Discovery Fallbacks Primitive Review

## Source

- `plans/037-Phase18.9-Generic-Text-Code-Modes-Key-Behavior-and-Mode-Discovery-Fallbacks.md`
- `roadmap.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/wiki/modules/mode-registry.md`
- `docs/wiki/modules/behavior-manifests.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/control-center.md`
- `docs/wiki/modules/phase18.7-persistent-runtime-bridge-primitive-review.md`
- `docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md`
- `src/packages/modes.rs`
- `src/protocol/mod.rs` (`EditorBehaviorRules`, `EnterRule`, `TabRule`, `PairRule`, `CommentContinuationRule`, `AutocompleteTrigger`)
- `src/behavior/manifest.rs`
- `src/server/connection.rs`
- `tests/primitives_docs.rs`
- `tests/package_primitive_gate.rs`

## Overview

Phase 18.9 should install durable generic editing behavior before language-specific modes so every file remains editable even when no specialized package is installed, disabled, or invalid. This review completes the primitive-first gate before implementation. It inventories current document classification, major-mode activation, behavior-manifest `TextTransform`, keybinding, command/execution, SDUI/status, and parse primitives; records the classification vs generic key behavior vs discovery execution hot-path split; identifies the genuine generic gaps; and states the authority boundary Phase 18.9 must preserve.

The headline finding of this review is that most generic key behavior already exists as manifest data in `EditorBehaviorRules` — `Enter`/`Tab`, pair insertion (`PairRule`), comment continuation (`CommentContinuationRule`), list/fence continuation (`ContinueLineMarkers`/`PreserveFenceBodyIndent`), and autocomplete triggers (`AutocompleteTrigger`) are all generic, mode-parameterizable, and executed by Rust-known transform engines. The real Phase 18.9 gaps are therefore narrower than "expose generic key behavior": they are (1) built-in always-on `core.text`/`core.code` fallback major modes, (2) classification shebang and bounded leading-content probes plus a documented precedence ladder, (3) electric-character handling as an extension of existing `EnterRule`/`PairRule`, and (4) mode discovery/listing commands consuming the active-mode registry through the Phase 18.8 `CommandExecution` path rather than a new primitive.

## Existing Primitive Inventory

### Document classification and major-mode activation

- `src/packages/modes.rs::ModeRegistry` is the server-side source of truth for package-owned major-mode declarations. `register_mode` validates package provenance, package-prefixed mode IDs, declared `mode-registration` and `mode-activation` permissions, well-formed/unique static patterns, and one-mode-per-document activation.
- `DocumentClassificationInput` carries `document_id`, an optional `path` (basename/extension), and an optional `mime_type`. `ModeRegistry::classify` matches by exact filename, filename wildcard, extension, then MIME type; equal-priority matches from different modes are rejected as ambiguous. There is currently no shebang or leading-content probe, and no fallback when no declared pattern matches.
- `MajorModeActivation` records the active major mode per document, the package provenance, and publishes a behavior version. Re-activating a document replaces the active major mode deterministically and increments the behavior version.
- `core.*` mode ID ownership is not currently enforced: a package could attempt to register a mode whose ID collides with the planned `core.text`/`core.code` built-ins. Phase 18.9 must reserve the `core.` prefix for Clay-owned built-in modes.
- `docs/runtime/modules/mode-registry.md`, `docs/wiki/modules/mode-registry.md`, and `docs/reference/primitives/registry.md` are the canonical docs; `tests/package_primitive_gate.rs` is the canonical classification/activation test.

### Behavior manifest and generic text transforms

- `src/protocol/mod.rs::EditorBehaviorRules` is the manifest-data shape for generic key/text behavior. It already contains:
  - `text_edits`: declared `Insert`/`Delete`/`Replace` capabilities.
  - `enter`: `EnterRule` variants `PreserveLeadingWhitespace`, `InsertNewlineOnly`, `ContinueLineMarkers { markers, exit_on_empty_item }`, and `PreserveFenceBodyIndent { fence_markers }`. List and code-fence continuation are already generic and mode-parameterizable.
  - `tab`: `TabRule { mode: TabMode::InsertSpaces | InsertTabCharacter, spaces_per_tab }`.
  - `pairs`: `Vec<PairRule>` with `open`/`close` strings and a `PairRuleContext`. Pair insertion for `() [] {} "" ''` and any mode-declared pair is already a generic manifest rule.
  - `comments`: `Vec<CommentContinuationRule>` with `line_prefix` and `continue_prefix`. Comment continuation on `Enter` is already a generic manifest rule.
  - `autocomplete_triggers`: `Vec<AutocompleteTrigger>` with a trigger character and a `UiReactivePriority` routing policy.
  - `EditorBehaviorRules::default_text()` already produces a sensible default text-mode ruleset (indent-preserving Enter, 4-space Tab, common pairs, `//` comment continuation, `.` autocomplete).
- `src/behavior/manifest.rs` validates these rules: non-empty pair open/close, non-zero `spaces_per_tab`, autocomplete trigger characters must use `UiReactivePriority`. Validation rejects ambiguous key bindings and executable/side-effect authority on commands.
- The existing `TextTransform` primitive (`behavior.getActiveBehaviorManifest`) is exactly the right surface for generic key behavior: packages declare parameters and rule declarations; the client executes only Rust-known transform engines; no client-side JavaScript and no IPC before local paint.

### Key routing and commands

- `src/protocol/mod.rs::KeyBindingRule` plus `KeyRoutingOverride` (`keybindings.bindKey`) record package-declared keybindings routed through behavior manifests. `ClientFirstPredictable` and `ClientFirstRequiresAck` are Rust-known client edit authorities; server-first/UI-reactive/background policies route intents to the server.
- `src/packages/commands.rs::CommandRegistry` and the Phase 18.8 `src/server/command_execution.rs::CommandExecutor`/`CommandExecutionRequest` provide one server-owned command execution boundary used by SDUI actions, package UI actions, keybindings, and transient-menu selections. Unknown commands, mismatched provenance, undeclared permissions, malformed/oversize arguments, and unauthorized targets are rejected.
- `src/server/control_center.rs` already lists commands from the registry snapshot through `TransientMenuSession`; this is the right consumer for Phase 18.9 mode discovery commands.

### SDUI, status, and decoration surfaces

- `SduiPanelStatusContribution` and the slot-aware package UI primitives remain inert server-validated UI state; status items can surface mode provenance/discovery output when introduced.
- `DecorationRange`/`IncrementalParseUpdate` run as background server-side work and are out of Phase 18.9 scope except that fallback `core.text`/`core.code` documents must remain editable with no syntax decorations, and a disabled language package must not leave stale decorations blocking editing.

### Document open path

- `src/server/connection.rs` selected-file open currently provides `path`/`mime_type` into classification. The open path already reads document content for the parse window; a bounded leading-content slice (shebang + first N bytes) can be supplied to `DocumentClassificationInput` without new file IO authority, reusing the existing read.

## Generic Phase 18.9 Primitive Gaps

### Built-in always-on `core.text` and `core.code` fallback major modes

The fallback modes are the core gap. Required approach:

- Register `core.text` and `core.code` as real built-in Clay-owned modes through the existing `ModeRegistry::register_mode`/equivalent path at server startup, with a documented built-in provenance marker (e.g., provenance `core`) and lowest precedence.
- Reserve the `core.` mode-ID prefix so packages cannot register or shadow `core.*` IDs; reject package-declared `core.*` modes at registration.
- `core.text` ships the `EditorBehaviorRules::default_text()` ruleset: generic `Enter` (indent-preserving), `Tab`, common pairs, comment continuation, autocomplete trigger. No language-specific branches.
- `core.code` ships a code-oriented variant: generic indentation, pair insertion, comment continuation hooks, and electric-character handling. It is the deterministic fallback when classification cannot match a language package but the content or shebang looks code-like.
- Built-in modes require no `~/.config/clay/init.js` line and no package load step; they are always available even when every language package is disabled or absent. Language packages remain opt-in `loadPackage("@clay/*")` that extend/override `core.code`.

No new primitive column is required: built-in modes are `MajorModeActivation`/`DocumentClassification` data registered by Clay itself. The plan's `FallbackModeDeclaration` notion folds into built-in mode registration, not a separate primitive.

### Classification shebang and bounded leading-content probes

`DocumentClassificationInput` should carry an optional bounded leading-content slice plus declarative shebang/content-probe pattern kinds:

```text
precedence: user override > package-declared pattern (exact filename > wildcard > extension > MIME)
            > shebang line > bounded leading-content probe
            > core.code > core.text
```

- Leading-content probes read only a bounded constant prefix (e.g., 512 bytes) of an already-open document; no filesystem scan, directory walk, or arbitrary package predicate.
- Probe rules are declarative metadata validated like other patterns; no language-specific Rust branches (no `if path.ends_with(".py")`).
- Oversize content slices are rejected and classification falls back to `core.code`/`core.text`.
- Ambiguous equal-priority shebang/content probes from different modes are rejected deterministically, matching the existing ambiguity policy.

This extends the existing `DocumentClassification` primitive (no new primitive) and reuses existing classification validation.

### Electric characters

Electric characters are the one genuinely new manifest kind. The review recommends modeling them as an extension of the existing `EnterRule`/`PairRule` family rather than a brand-new primitive, so they stay `ClientFirstPredictable` manifest data executed by Rust-known engines:

- An electric-character rule declares: a trigger character (e.g., `:`/`{`/`<`/`=>`), an optional preceding-context predicate, and the deterministic local transform (e.g., auto-outdent on `}` in C-like languages, automatic indent reflow). No callbacks, no JavaScript.
- Where an "electric" effect is really pair/character insertion, the existing `PairRule` already covers it. Phase 18.9 should add electric-character handling only for effects that `EnterRule`/`PairRule` cannot already express, and keep it generic so any future language package can declare parameters.

This is a `TextTransform` extension (new manifest kind in `EditorBehaviorRules`), not a new primitive category.

### Mode discovery/listing commands

Phase 18.9 adds two built-in commands, registered through the existing `CommandDeclaration` primitive and resolved through the Phase 18.8 `CommandExecutor`:

```text
modes.listActiveModes   -> [{documentId, modeId, provenance, classificationSource}]
modes.explainActiveMode -> {documentId, activeMode, why, fallbackUsed}
```

- Commands read already-installed `ModeRegistry` state only; they never trigger filesystem scans, package evaluation, or parse work.
- They grant no execution/document/workspace authority; they are informational and route through Control Center for visibility.
- Existing `CommandDeclaration`/`CommandExecutor` validation (ID, routing policy, permissions, arguments, target) applies unchanged. No new primitive.

## Hot-Path Classification

Phase 18.9 classifies work explicitly:

| Work | Classification | Allowed path |
| --- | --- | --- |
| Document classification (extension/filename/MIME/shebang/content probe) | Open/reload-time, no-hot-path | `ModeRegistry::classify` on open/reload/explicit reclassification only |
| Built-in fallback mode activation | Server-first activation on open/reload; `MODE_ACTIVATION_P95_BUDGET_MS` | No work per keypress after activation |
| Generic key behavior (Tab/Enter/pair/comment/list continuation/electric) | `ClientFirstPredictable` manifest data | Client executes Rust-known transform engines; no IPC, no JavaScript before local paint |
| Mode discovery/listing | Server-first explicit command | `CommandExecutor` reads registry metadata; bounded, cancellable; not in paint/text hot paths |
| Reclassification after package reload/enable/disable | Deterministic reactivation + behavior-version bump | Reuses centralized activation path, no parallel state |

Ordinary typing, caret movement, local edit application, scroll, paint, layout, pointer hit testing, keypress dispatch, and text-event handling must not synchronously classify documents, execute mode discovery commands, run package JavaScript, wait on IPC, read files, call shell/network/AI, or serialize full documents. Generic `Enter`/`Tab`/pair/comment-continuation transforms must apply entirely from installed manifest data on the client, preserving `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` and `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`.

## Rejected Implementation Shapes

- Do not add `PlainTextMode`, `CodeMode`, `PythonFallbackSelector`, `MarkdownFallback`, or any `if mode == "code"` / `if extension == ".py"` Rust classification/transform branch. Generic probes are declarative metadata validated by the existing engine.
- Do not implement `core.text`/`core.code` as first-party `@clay/core-text`/`@clay/core-code` JS packages requiring `loadPackage`. Built-in fallback modes must be always-on and must not depend on init.js loading or package enable state; "missing packages never block editing" supersedes the one-line load convention for built-in behavior.
- Do not add a parallel `FallbackModeRegistry` or a separate `FallbackModeContribution` primitive. Built-in modes are `MajorModeActivation`/`DocumentClassification` data registered by Clay itself with a `core.` provenance marker.
- Do not invent a `CommentContinuation` or `PairInsertion` primitive — both already exist as `CommentContinuationRule` and `PairRule` manifest rules. Electric characters are the only new manifest kind, and they extend `EditorBehaviorRules` rather than introducing a new primitive category.
- Do not run filesystem scans, directory walks, arbitrary package predicates, package JavaScript, configuration evaluation, blocking IPC, or full-document serialization inside `classify` or in Masonry paint/layout/pointer/scroll/key/text-event handlers.
- Do not implement mode discovery as a one-off SDUI panel or Control-Center-only widget; register `modes.listActiveModes`/`explainActiveMode` as commands routed through the shared `CommandExecutor`, consumable by Control Center and any future diagnostics UI.
- Do not expose Masonry `Widget`/`WidgetId`/`WidgetPod`, native handles, Vello/Parley callbacks, raw op names, or raw CSS as package or discovery APIs.
- Do not treat a public Clay JS API as implemented by adding only a raw op or inventory row; public APIs require facade, op, docs, registry, tests, security notes, and naming metadata.

## Security and Authority Boundary

The Phase 18.9 review introduces no new filesystem, network, shell, AI, WASM, native-widget, raw-op, client-side JavaScript, package-manager, package-install, or package-enable/disable authority.

Allowed authority remains narrow:

- Built-in `core.text`/`core.code` modes grant only existing manifest/classification authority; they are always-on expressions of `MajorModeActivation`/`DocumentClassification` and cannot be overridden to grant package authority.
- Classification shebang/content probes read only from already-open document bytes supplied by the open path; no new file-read authority, no filesystem scan, no directory walk, no arbitrary predicate. Oversize slices are rejected.
- Generic key behavior remains manifest data; packages contribute parameters and rule declarations only. No client-side JavaScript, raw ops, native widget handles, raw CSS, renderer callbacks, or arbitrary callbacks. Executable callback fields stay rejected by the existing `TextTransform`/command authorities.
- Mode discovery commands are read-only over installed registry state; they grant no execution/document/workspace authority and never trigger scans or package evaluation.
- `core.*` mode ID ownership prevents packages from shadowing built-in fallback modes; built-in provenance is recorded so discovery can explain `core.code` vs a package mode.
- Disabled/invalid language packages must never leave stale active modes that bypass validation; reclassification reuses the centralized activation path and increments the behavior version.

## Planned Documentation and Test Coverage

- `docs/reference/primitives/registry.md` should record Phase 18.9 coverage on the `DocumentClassification` (shebang/content probe, fallback precedence), `TextTransform` (existing `enter`/`pairs`/`comments` kinds reused; electric-character kind added), and `KeyRoutingOverride` (fallback command routing for unmatched keybindings) rows, plus the `core.text`/`core.code` built-in mode note under `MajorModeActivation`.
- `docs/reference/primitives/backlog.md` should note Phase 18.9 reuses existing primitives rather than introducing new priority-tier rows.
- `tests/primitives_docs.rs` should require this review page to be linked from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`, and should assert it records inventory, genuine gaps (built-in modes, classification probes, electric characters, discovery commands), hot-path classification, rejected mode-specific shapes, and no-new-authority text. It should also assert the registry mentions the required Phase 18.9 terms.
- `tests/package_primitive_gate.rs` should later cover built-in mode registration, `core.*` ownership rejection, shebang/content-probe matching, precedence, bounded-prefix rejection, and fallback availability with no packages loaded (implementation tasks).

## Final Implementation Status

This review was the Phase 18.9 Task 1 primitive-first inventory. Its recommended approaches were implemented in Tasks 3-6 and verified in Tasks 7-10; this section reconciles the review's scope with what shipped so the page reflects the final implementation.

- **Built-in fallback modes (implemented).** `core.text`/`core.code` are registered at server startup via `ModeRegistry::register_builtin_mode` (not `register_mode`) with an `is_builtin` provenance marker and `ModePatternKind::Fallback` as the lowest-precedence variant for universal `core.text`. The `core.` and `clay.` mode-ID prefixes are reserved: `register_mode`/`register_minor_mode` reject them. Built-in modes ship their own default behavior manifests without an owning package (`select_behavior_manifest_for_document` bypasses package-record lookup on the `core.` prefix). Classification is two-phase: package-declared matches are consulted first (any package match beats built-ins), then `core.code` (code-like extensions and any shebang) and `core.text` (universal fallback).
- **Classification probes (implemented as scoped).** `DocumentClassificationInput` carries optional `shebang` and `leading_content` `Option<String>` fields supplied solely by the open path; packages cannot supply probe data. The bound is the constant `MAX_LEADING_CONTENT_BYTES = 512` in `src/packages/modes.rs`; oversize slices are treated as absent (classification falls to the remaining ladder rather than failing). `ModePatternKind::ContentProbe` (ordinal 1) and `ModePatternKind::Shebang` (ordinal 2) sit below `MimeType` in precedence. `ModeDeclaration` carries `shebang_patterns` (exact match or single-wildcard glob via `wildcard_match`) and `content_probes` (non-empty literal markers, no wildcards/separators/whitespace, must fit within the bound). `core.code` declares `shebang_patterns: ["*"]` so any shebang routes to code mode. No filesystem scan, directory walk, or package predicate runs.
- **Electric characters (implemented with a narrowed initial effect set).** The review's aspirational trigger examples (`:`/`{`/`<`/`=>`) and optional preceding-context predicate were **not** shipped in the initial implementation. What shipped: a new manifest *kind* `electric_characters: Vec<ElectricCharacterRule>` on `EditorBehaviorRules`, where each rule names a `trigger` character and a declarative `ElectricEffect`. Only `ElectricEffect::OutdentOneLevel` is implemented; package JSON parsing accepts only the `outdent-one-level` effect and **drops** unknown effects. `core.code` ships a default electric set for `}`/`)`/`]`. Rust-known engines execute: `insert_electric_with_event` and `dedent_leading_one_level` in `src/editor/surface.rs` (no client-side JavaScript, raw ops, native handles, raw CSS, callbacks, or preceding-context predicates). A future phase that needs additional electric effects (indent reflow, `:`/`=>`) must add a new `ElectricEffect` variant plus a Rust-known engine; the package-declarable JSON parser is the extension point but only known effects execute.
- **Mode discovery commands (implemented).** `modes.listActiveModes`/`modes.explainActiveMode` are read-only `ServerFirst` built-in server commands with empty permissions, resolved via `CommandExecutor::execute_discovery` (not a new primitive or op/facade). Return types `ActiveModeSummary` and `ModeExplanation` carry `ModeProvenance` (`CoreBuiltIn`/`Package`) and `classification_source`. See [Command Registry](command-registry.md) for the resolver contract.
- **Mid-session disable (added beyond the review).** `ModeRegistry::unregister_mode` removes a package-declared mode declaration mid-session while preserving the prior `active_major_modes` entry so reclassification bumps `behavior_version` (1→2); stale entries cannot bypass validation because `select_behavior_manifest_for_document` errors for unregistered modes. Always returns `false` for `core.*` built-ins.
- **No new authority (confirmed).** The review's no-new-authority conclusion held: built-in modes grant no package authority, probing introduces no filesystem-scan authority, electric engines are Rust-known (packages contribute parameters only), and discovery commands carry empty permissions. See `decision-logs/2026-07-01-0350-phase18-9-generic-text-code-fallback-modes-and-key-behavior.md`.

## Invariants and Constraints

- `ModeRegistry` remains the classification and activation source of truth.
- Built-in `core.text`/`core.code` are `MajorModeActivation` data, not a separate primitive.
- Generic key behavior is `ClientFirstPredictable` manifest data executed by Rust-known transform engines; the client never runs package JavaScript in paint/layout/pointer/scroll/key/text-event handlers.
- Classification runs only on open/reload/explicit reclassification, never per keypress.
- Mode discovery commands are read-only `CommandDeclaration` consumers routed through `CommandExecutor`.
- Any document is always editable even when every language package is disabled, invalid, or absent.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Implementation-time tests (later tasks): `tests/package_primitive_gate.rs` for built-in `core.*` mode registration/ownership, shebang/content-probe matching, precedence, bounded-prefix rejection, and fallback availability with no packages loaded; behavior-manifest tests for the electric-character manifest kind and `core.code` default rules; `tests/command_execution.rs` for mode discovery command results and no-authority invariants.
- Run focused documentation coverage with:

```text
cargo test --test protocol primitives_docs::
```

## Related

- [Mode Registry](mode-registry.md)
- [Behavior Manifests](behavior-manifests.md)
- [Command Registry](command-registry.md)
- [Control Center](control-center.md)
- [Transient Menu Session](transient-menu-session.md)
- [Phase 18.7 Persistent Runtime and Parse Bridge Primitive Review](phase18.7-persistent-runtime-bridge-primitive-review.md)
- [Phase 18.8 Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [Primitive Architecture](primitive-architecture.md)
- [Primitive Registry Reference](../../reference/primitives/registry.md)
- [Prioritized Primitive Backlog](../../reference/primitives/backlog.md)
