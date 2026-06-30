---
date: 2026-07-01 03:50
status: approved
decision_about: "Phase 18.9 fallback mode authority and generic key behavior"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Phase 18.9 fallback mode authority and generic key behavior

## Decision

Clay ships always-on Clay-owned built-in fallback major modes `core.text` and `core.code`, registered at server startup through `ModeRegistry::register_builtin_mode` with no `~/.config/clay/init.js` line and no `loadPackage` step. Language packages *extend* these built-in fallbacks through the existing generic primitives (`DocumentClassification`, `MajorModeActivation`, `TextTransform`, `KeyRoutingOverride`, `CommandDeclaration`); the Phase 18.9 authority-adjacent changes — built-in always-on modes, bounded shebang/content probing of open-document bytes, generic client transform-engine kinds (electric characters), and the mode-discovery command contract — introduce **no new package authority** and no new client-side JavaScript execution path.

The generic transform kinds shipped by `core.code` (pair insertion, comment continuation, electric characters, Tab, Enter) are declarative manifest data executed by Rust-known engines, and are documented as reusable across future modes — not Markdown-only or Python-only.

## Context

Phase 18.9 (Plan 037, "Generic Text/Code Modes, Key Behavior, and Mode Discovery Fallbacks") needed every document to remain editable even when no language package is installed, disabled, or invalid, and needed generic key-behavior primitives packages can reuse — without weakening the deny-by-default authority model established in:

- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` (Clay-owned shell/layout and the package-contributes-parameters-only contract), and
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` (unified user-authorized package authority; install source affects trust prompts but not the capability ceiling).

The decision records how Phase 18.9 stays inside that boundary: built-in modes are Clay-owned data (not a real package), classification probing reads only a bounded prefix of an already-open document supplied by the open path (not a filesystem scan), generic transform kinds are manifest data executed by Rust engines (not package-supplied code), and discovery commands are read-only ServerFirst built-ins with empty permissions.

## Approval

- Proposed by: agent
- Approved by user: Yes
- Approval evidence: User said, "Yes approved" in response to the proposed decision log for "fallback mode authority and generic key behavior".

## Alternatives Considered

1. **No decision log** — rejected. Phase 18.9 changes authority-adjacent surfaces (always-on built-in modes that can be active with no package, open-document byte probing for classification, a new client transform kind active on the typing hot path, and a new command contract), so the authority boundary must be recorded per the create-decision-log workflow even though no new authority is granted.

2. **Built-in modes as a real Clay package** (`@clay/core-text`) loaded through `loadPackage` — rejected. Built-ins are registered at server startup via `register_builtin_mode` in `ModeRegistry::new()`, not via the package module loader; `core.*` IDs are Clay-reserved and unregisterable by packages; built-in manifests ship without an owning package record (`select_behavior_manifest_for_document` bypasses package-record lookup on the `core.` prefix). A package shape would imply packages can register `core.*` and would couple always-on editing to package load ordering.

3. **Parallel `FallbackModeRegistry` / a new "built-in authority" primitive** — rejected. Built-ins reuse the existing `MajorModeActivation` / `DocumentClassification` data surfaces with an `is_builtin` provenance marker and a `ModePatternKind::Fallback` lowest-precedence variant, rather than introducing a new registry or a new primitive category. `register_mode` / `register_minor_mode` reject `core.*` and `clay.*` prefixes so package-provided and built-in modes share one classification path.

4. **Language-specific Rust branches / client-side JavaScript for electric characters** — rejected. Electric characters (`ElectricCharacterRule` + `ElectricEffect::OutdentOneLevel`) are declarative manifest data; only Rust-known engines (`insert_electric_with_event`, `dedent_leading_one_level` in `src/editor/surface.rs`) execute them. Package JSON parsing accepts only the `outdent-one-level` effect; unknown effects are dropped. No language-specific Rust branches, no raw `Deno.core.ops`, no native handles, no raw CSS, no callbacks, no client-side JavaScript.

5. **New gating/executing authority for discovery commands** — rejected. `clay.modes.listActiveModes` and `clay.modes.explainActiveMode` are read-only `ServerFirst` built-in commands (in the same `builtin_server_command` list as `clay.controlCenter.open` / `workspace.refresh`) with empty permissions, resolved via `CommandExecutor::execute_discovery` by reading installed registry state. They carry no execution/document/workspace authority and have no op wrapper or Clay JS facade.

6. **Runtime configuration knobs for fallback mode / electric / pair / comment toggles** — rejected as YAGNI. `core.preferredFallbackMode`, `core.electricCharacters`, `core.pairInsertion`, `core.commentContinuation` have zero consumers; the fallback is hardcoded in `ModeRegistry::classify`, electric chars are hardcoded in `EditorBehaviorRules::default_code()`. The package system (declaring a mode with patterns + editor rules) is the override escape hatch, and `setPackageOption`'s closed suffix allowlist rejects those keys as unsupported package options, so built-in defaults cannot be overridden to grant authority.

Also considered and rejected during Phase 18.9 design: making built-in modes package-shaped, introducing any new authority primitive for discovery, and exposing `list_active_modes`/`explain_active_mode` as direct Rust embedder API. The resolver methods are `pub(crate)`; the user-facing surface is the built-in command.

## Rationale and Evidence

Concrete source evidence:

- **Built-in modes (Task 3):** `src/packages/modes.rs` — `CORE_TEXT_MODE_ID = "core.text"`, `CORE_CODE_MODE_ID = "core.code"` (lines ~288-290); `core_text_mode()`/`core_code_mode()` constructors; `register_builtin_mode()` (line ~503); `is_builtin` field on `RegisteredMode`; `ModePatternKind::Fallback` (line ~82) as the lowest-precedence variant for universal `core.text` fallback. `register_mode` (line ~429) and `register_minor_mode` (line ~819) reject `core.*` and `clay.*` prefixes (lines ~467 and ~863). `activate_builtin_major_mode` activates a built-in and bumps `behavior_version`. Classification tie-breaking: on equal `ModePatternKind`, a package-declared mode beats a built-in; only same-provenance ties raise `AmbiguousClassification`.

- **Two-phase classification (Task 3/4):** `ModeRegistry::classify` runs Phase 1 package-declared matching (best package match across all signal kinds where any package match beats built-ins) then Phase 2 built-in fallbacks where `core.code` claims extensions/shebang and `core.text` serves as universal fallback. `src/server/connection.rs` `classify_open_document` computes a shebang from the first line (if `#!`) and bounded `leading_content`, injects both into the JS classification input.

- **Bounded probing (Task 4):** `src/packages/modes.rs` — `MAX_LEADING_CONTENT_BYTES = 512` (line ~57). `ModePatternKind::ContentProbe` (ordinal 1) and `ModePatternKind::Shebang` (ordinal 2) added below `MimeType` in precedence. `ModeDeclaration` extended with `shebang_patterns: Vec<String>` (exact match or single-wildcard glob via `wildcard_match`) and `content_probes: Vec<String>` (non-empty literal markers, no wildcards/separators/whitespace, must fit within `MAX_LEADING_CONTENT_BYTES`). `core.code` declares `shebang_patterns: ["*"]` so any shebang routes to code mode. Oversize slices are treated as absent so classification still succeeds via the remaining ladder. The open path is the sole authority supplying these slices; packages cannot provide probe data.

- **Generic key behavior (Task 5):** `src/behavior/manifest.rs` — `electric_characters` field on `EditorBehaviorRules`; `ElectricEffect::OutdentOneLevel`; `EditorBehaviorRules::default_code()` ships electric outdent for `}`/`)`/`]`; `BehaviorManifest::core_code_editing()` and `BehaviorManifest::minimal_text_editing()` constructors. `src/editor/surface.rs` — `insert_electric_with_event` (line ~1008) and `dedent_leading_one_level` (line ~1268) Rust-known engines, wired into `route_key_with_event`. Package JSON parsing accepts only `outdent-one-level`; unknown effects dropped. `select_behavior_manifest_for_document` ships the `core.code` manifest when a `core.*` mode is activated.

- **Discovery commands (Task 6):** `src/server/command_execution.rs` — `clay.modes.listActiveModes` and `clay.modes.explainActiveMode` registered in `builtin_server_command_ids()`/`builtin_server_command()` with `RoutingPolicy::ServerFirst` and empty permissions; `MODE_DISCOVERY_COMMAND_IDS` and `is_mode_discovery_command()` are `pub(crate)`; `execute_discovery()` (line ~139) reads installed registry state via `list_active_modes()`/`explain_active_mode()` (both `pub(crate)`, same-crate only). `src/server/ops/mod.rs` `ClayRuntimeOpState::execute_command` routes discovery commands through `execute_discovery`. Return types `ActiveModeSummary` and `ModeExplanation` carry provenance (`CoreBuiltIn`/`Package`) and `classification_source`.

- **Performance budgets:** `src/perf/budgets.rs` — `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES = 2048`, `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS = 16`, `MODE_ACTIVATION_P95_BUDGET_MS = 100`, `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS = 25`.

- **No config in hot paths (Task 10):** `src/server/configuration.rs` `validate_package_option_name` closed allowlist only accepts suffixes `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, `fallback`; Phase 18.9 behavior keys are rejected. `src/packages/modes.rs` classify/open path contains zero runtime configuration reads.

Project evidence reviewed:
- `plans/037-Phase18.9-Generic-Text-Code-Modes-Key-Behavior-and-Mode-Discovery-Fallbacks.md` — Tasks 3-10 acceptance criteria and chosen approaches.
- `docs/reference/packages/creating-packages.md` — Phase 18.9 authoring contract section (extension mechanism, classification ladder, discovery contract, reusable transform kinds, budgets, security boundaries).
- `docs/reference/primitives/registry.md` — `DocumentClassification`/`MajorModeActivation`/`TextTransform`/`KeyRoutingOverride` primitive notes.

## Performance record

Generic key behavior is `ClientFirstPredictable`: the Rust client executes inert behavior-manifest data for Tab/Enter/pair/comment/electric transforms with no synchronous JavaScript, IPC, or server round trip before local paint. Modes and classification defaults are compile-time constants (no configuration-evaluation cost at paint/text time). Configuration evaluation is bounded to init.js/package load or explicit setting change and stays off the typing/paint/text-event hot paths.

Budgets enforced: `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS = 16` (no sync JS before local paint), `MODE_ACTIVATION_P95_BUDGET_MS = 100`, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES = 2048` (oversize manifests rejected with `PayloadBudgetExceeded` at record time), `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS = 25`.

## Security record

- **`core.*` / `clay.*` ID ownership reserved.** The `core.` mode-ID prefix is reserved for Clay-owned built-ins and cannot be registered by a package (`register_mode`/`register_minor_mode` reject `core.*` and `clay.*`).
- **Deny-by-default authority.** Built-in fallback modes require no package and grant no package authority. Packages cannot grant themselves filesystem, network, shell, AI mutation, WASM, package-manager, package installation/enable-disable, native widget, raw-op, or client-side-JavaScript authority. The powerful authorities remain user-authorized via `clay.capabilities` per the unified-authority model.
- **Packages contribute parameters/declarations only.** Electric characters, pair insertion, and comment continuation are declarative manifest data; Rust-known engines execute them. Unknown electric effects are dropped (not executed). No raw `Deno.core.ops`, native handles, raw CSS, callbacks, or client-side JavaScript are exposed to packages.
- **Built-in defaults cannot be overridden to grant authority.** `setPackageOption` uses a closed suffix allowlist and rejects Phase 18.9 behavior-changing keys (`core.preferredFallbackMode`, `core.electricCharacters`, `core.pairInsertion`, `core.commentContinuation`) as unsupported package options.
- **Bounded leading-content probing.** Shebang/content probes read only a bounded constant prefix (`MAX_LEADING_CONTENT_BYTES = 512`) of an already-open document supplied by the open path; they perform no filesystem scan, directory walk, or arbitrary package predicate, and oversize slices are rejected and classified to a fallback mode. The open path is the sole authority supplying these slices; packages cannot provide probe data.
- **Discovery commands carry no authority.** `clay.modes.listActiveModes`/`explainActiveMode` are read-only `ServerFirst` built-ins with empty permissions; `CommandExecutor::execute_discovery` reads installed registry state and performs no execution/document/workspace mutation.

## Code Quality: built-in vs package-provided modes, primitive-data vs new-primitive

- **Built-in vs package-provided modes.** Built-in `core.text`/`core.code` are Clay-owned data registered by `register_builtin_mode` at `ModeRegistry::new()` with an `is_builtin` provenance marker and lowest precedence; any package-declared pattern at equal or higher precedence beats them. Built-ins ship their own default behavior manifests without an owning package (`select_behavior_manifest_for_document` bypasses package-record lookup on the `core.` prefix). Package-provided modes are user-authorized, extend the built-ins, and win on precedence — not on load order. There is intentionally no runtime configuration knob to flip the fallback (YAGNI); declaring a package mode with a matching pattern is the override.
- **Primitive-data vs new-primitive.** Electric characters are the only new manifest *kind* added (`ElectricCharacterRule` + `ElectricEffect`), implemented as an extension of the `EnterRule`/`PairRule` family for effects those rules cannot already express, and reused for `core.code`'s default outdent set. No new primitive *category* and no new *authority* primitive were introduced: built-ins reuse `MajorModeActivation`/`DocumentClassification`; discovery reuses `CommandDeclaration`/`CommandExecution`; generic transforms reuse `TextTransform`. This keeps the primitive surface small and the new kind reusable across future language modes.

## References

- `plans/037-Phase18.9-Generic-Text-Code-Modes-Key-Behavior-and-Mode-Discovery-Fallbacks.md`
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` — Clay-owned shell/layout and package-contributes-parameters-only contract.
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` — unified user-authorized package authority; source affects trust prompts, not capability ceiling.
- `src/packages/modes.rs` — built-in modes, `core.*` guards, two-phase classify, bounded probing, `MAX_LEADING_CONTENT_BYTES`.
- `src/behavior/manifest.rs` — `electric_characters`, `ElectricEffect::OutdentOneLevel`, `core_code_editing`/`minimal_text_editing` constructors.
- `src/editor/surface.rs` — `insert_electric_with_event`, `dedent_leading_one_level` Rust engines.
- `src/server/command_execution.rs` — discovery commands, `execute_discovery`, empty permissions, `ServerFirst` routing.
- `src/server/ops/mod.rs` — `execute_command` routing to `execute_discovery`.
- `src/server/configuration.rs` — closed suffix allowlist rejecting behavior-changing core.* keys.
- `src/perf/budgets.rs` — payload/latency budget constants.
- `docs/reference/packages/creating-packages.md` — Phase 18.9 authoring contract section.
- `docs/reference/primitives/registry.md` — primitive category notes.

## Consequences

Positive:
- Any file opens into a predictable editable mode (`core.text`/`core.code`) with no package, no `init.js`, and no synchronous JS before local paint.
- Generic transform kinds (electric/pair/comment/Tab/Enter) are reusable across future modes; language packages add capabilities incrementally without becoming required for basic editing.
- The authority boundary stays deny-by-default: built-ins grant no package authority, probing introduces no filesystem-scan authority, and discovery commands carry empty permissions.

Risks and follow-up:
- The `MAX_LEADING_CONTENT_BYTES = 512` bound may need revisiting for exotic shebangs/multibyte leading markers; oversize slices currently fall back to the remaining ladder rather than failing.
- Future electric effects beyond `outdent-one-level` will each need a Rust-known engine; the package-declarable JSON parser is the extension point but only known effects execute.
- Adding any new client transform kind beyond Rust-known effect types in a future phase would require revisiting the "packages contribute parameters only" boundary.

Conditions that would cause revisiting this decision:
- A phase requires packages to inject new client transform kinds not backed by a Rust-known engine.
- A phase requires packages to register `core.*` mode IDs (would violate the reserved-prefix rule).
- A phase requires unbounded or package-supplied document probing (would violate the bounded, open-path-only probe rule).
- A phase requires discovery commands to mutate document/workspace state (would violate the read-only/empty-permission rule).
