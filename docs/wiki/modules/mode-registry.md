# Mode Registry

## Source

- `src/packages/modes.rs`
- `tests/package_primitive_gate.rs`

## Overview

The mode registry is the Phase 16.5 server-side primitive gate for document classification and major-mode activation. It accepts package-declared, static mode metadata after the package manifest validator has already approved package identity, prefix, declared modes, and permissions.

## Responsibilities

- Register package-owned major mode declarations with provenance.
- Classify open-document metadata by extension, MIME hint, exact filename, or a bounded basename wildcard pattern.
- Activate one server-owned major mode per document and assign behavior-version metadata.
- Reject undeclared modes, duplicate mode IDs, malformed static patterns, and missing `mode-registration` or `mode-activation` permissions.

It does not execute package JavaScript, scan the filesystem, install package managers, or make the Rust client authoritative for mode selection.

## How It Works

`ModeRegistry::register_mode` takes a validated `ClayPackageManifest` and a `ModeDeclaration`. Registration checks that:

1. The package declared `mode-registration`.
2. Declaration provenance matches the manifest name, version, and `apiPrefix`.
3. The mode ID is package-owned and appears in `clay.modes`.
4. Static patterns are well formed and unique.
5. No enabled package already registered the same mode ID.

`ModeRegistry::classify` receives `DocumentClassificationInput` for an open document. It uses only the supplied path basename/extension, optional MIME hint, optional shebang line, and an optional bounded leading-content slice — never the filesystem. The full Phase 18.9 precedence ladder is: **exact filename > wildcard filename > extension > MIME > shebang line > bounded leading-content probe > `core.code` > `core.text`**. Package-declared signals are always consulted before built-in fallbacks, so any package match (even the weakest content probe) wins over `core.code`/`core.text`. Among package-declared signals, equal-priority matches from two different package modes are rejected as ambiguous (`AmbiguousClassification`); a package-declared mode and a built-in mode never conflict because built-ins are a separate, lower fallback tier.

Shebang and content probes are declarative metadata validated like other mode patterns: a mode declares `shebang_patterns` (interpreter globs such as `python*`, `bash`, or `*` for any interpreter) and `content_probes` (literal markers such as `<?xml` matched at the start of the leading slice). The open path supplies the shebang line and a bounded prefix of the already-open document text (`MAX_LEADING_CONTENT_BYTES = 512`); oversize slices are rejected (treated as absent) so probes can never read unbounded content, perform no filesystem scan, directory walk, or arbitrary package predicate, and introduce no new filesystem/network/shell authority. `core.code` declares the `*` shebang pattern so any shebang-marked script resolves to generic code rather than plain text when no language package claims it. There are no language-specific Rust branches: matching is generic glob/literal-prefix comparison over declarative metadata.

Clay registers two always-on built-in Clay-owned fallback modes at server startup through `ModeRegistry::new()`: `core.text` (universal plain-text fallback with no static patterns, selected via `ModePatternKind::Fallback` when nothing else matches) and `core.code` (code-oriented fallback claiming a curated declarative set of common programming extensions, plus the `*` shebang pattern). Built-in modes require no package, no `~/.config/clay/init.js` line, and no `loadPackage` step, so any document remains editable even when every language package is absent or disabled. The `core.` mode-ID prefix is reserved for Clay-owned built-in modes; `ModeRegistry::register_mode` (package path) and `register_minor_mode` reject package-declared `core.*` IDs, and `register_builtin_mode` rejects non-`core.*` IDs. Built-in modes are activated with `ModeRegistry::activate_builtin_major_mode`, which skips package provenance checks because built-in modes have no owning package.

`ModeRegistry::activate_major_mode` requires `mode-activation`, verifies that the classification still belongs to the package manifest, and writes `MajorModeActivation` into server-owned registry state. Each declaration also stores one validated semantic `defaultFontRole` (`monospace` or `proportional`), copied into the selected `BehaviorManifest`; `core.code` uses monospace while `core.text` uses proportional. Packages never supply family names or sizes. Re-activating a document replaces the previous major mode deterministically and increments the behavior version. Stale behavior-version rejection is enforced downstream at the connection layer: `ActiveBehaviorManifest::validate_message_version` rejects edits whose `behavior_version` is not the current server version, returning `EditRejection::InvalidBehaviorVersion`; the registry-level precondition is that every re-activation produces a strictly greater `MajorModeActivation::behavior_version` for the same document, so any edit built against a prior manifest is provably stale.

## Active syntax grammar is separate from active major mode (Phase 18.10)

Phase 18.10 adds syntax grammar selection in `src/server/syntax.rs` without changing the major-mode registry contract. `ModeRegistry` still owns exactly one active major mode per document and still owns behavior-version changes. `SyntaxGrammarRegistry::select_for_document` receives the already-selected `MajorModeActivation` plus the same open-document classification metadata, records an optional `active_syntax_grammar`, and copies the active mode/behavior version only for diagnostics.

This lets fallback documents stay editable through `core.code` or `core.text` while a grammar-only package supplies highlighting:

```text
active_major_mode: core.code
active_syntax_grammar: rust from @clay/rust, selected by extension rs
edit behavior: core.code behavior manifest
```

If the grammar package is absent, disabled, or invalid, `active_syntax_grammar` is `None`; the active major mode, command routing, file authority, and behavior manifest remain unchanged. Syntax grammar selection runs on open/reload/reclassification/package load or reload inputs, not from keypress, paint, layout, scroll, pointer, or text-event handlers.

## Package disable / mid-session reclassification (Phase 18.9)

`ModeRegistry::unregister_mode(mode_id)` drops a mode declaration from the candidate set — used when an owning package is disabled mid-session. It is symmetric with `register_mode`/`register_builtin_mode` (not a new primitive), grants no new authority (in-process registry mutation only), and always preserves built-in `core.*` modes (returns `false`) so the always-available fallback guarantee holds. It does **not** by itself reclassify open documents: the centralized activation path (`classify` + `activate_major_mode`/`activate_builtin_major_mode`) must be re-run for each affected document so it reclassifies deterministically and gets a strictly greater behavior version. The prior active activation for the removed mode is deliberately retained until reclassification replaces it — it cannot bypass validation because `select_behavior_manifest_for_document` errors for an unregistered mode (its owning package is no longer in the enabled list), forcing reclassification. Thus disabling/removing a language package never blocks open/edit: the affected document reclassifies to `core.code` (shebang/code extensions) or `core.text` (everything else) deterministically.

## Default init.js loading experience (Phase 18.9)

`core.text` and `core.code` are **always-on built-in Clay-owned modes** registered at server startup through `ModeRegistry::new()` — they require **no `~/.config/clay/init.js` line and no `loadPackage` step**. A fresh runtime with an absent or empty `init.js` still classifies and activates an editable built-in fallback for every file through the centralized `classify` + `activate_builtin_major_mode` + `select_behavior_manifest_for_document` path (the same path the open path uses). This is the always-available guarantee: missing/disabled/invalid language packages never block open or edit, and first open needs no JavaScript round trip for fallback editing because the built-in modes are registered before any configuration/package evaluation runs.

Language packages remain **explicit opt-in** through the established one-line default loading convention — they *extend* `core.code`, they do not replace it:

```javascript
// ~/.config/clay/init.js — optional: enable a language package on top of core.code
import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
```

With that one line, a `.md` file activates the Markdown package mode (package-declared pattern wins precedence over the built-in fallback), while a `.rs` file still resolves to `core.code` because no package claims it. With the line removed, `.md` falls back to `core.text` — still editable, just without Markdown-specific behavior/decorations. No copied package manifests, low-level facade plumbing, manual primitive registration, or test-only wiring is required for fallback editing to work in a user config: the built-in modes ship their own default behavior manifests (`minimal_text_editing` for `core.text`, `core_code_editing` for `core.code`) without an owning package. Built-in modes grant only existing manifest/classification authority — "always-available" implies no new filesystem, network, or shell authority.

## Per-mode movement and caret settings (Plan 071)

Plan 071 (tasks 4/6/11) extends the manifest's `editorRules` with two optional declarative blocks validated by `parse_movement_rules` / `parse_caret_style` (`src/server/ops/modes.rs`): `movement` (`wordSeparators` code/prose/custom, `treatUnderscoreAsWord`, `camelCaseSubWord`, `paragraphStyle`, `stopAtEolWordEnd`, `lineMovement`, `stickyColumn`) and `caretStyle` (shape/blink/width/height/hollow/stopBlinkOnTyping). Built-in fallbacks (`core_code_editing`/`minimal_text_editing`) ship code-style movement and no caret override; `@clay/markdown` declares prose movement through `buildCodeEditingManifest`, and the code packages declare explicit code movement. Absent fields fall back deny-by-default to server defaults — no silent behavior change. Ligatures are deliberately not a manifest field; they follow the mode's font-role typography profile. See [Editor Movement, Selection, Caret, Ligatures, and Text Objects](editor-movement-selection-caret.md).

## Code Examples

```rust
let mut registry = ModeRegistry::new();
registry.register_mode(&manifest, markdown_mode_declaration)?;
let classification = registry.classify(&DocumentClassificationInput {
    document_id: 7,
    path: Some("README.md".to_string()),
    mime_type: None,
})?;
let activation = registry.activate_major_mode(&manifest, classification)?;
```

## Invariants and Constraints

- A document has at most one active major mode in `active_major_modes`.
- A document may also have an optional active syntax grammar in `SyntaxGrammarRegistry`; it never replaces the active major mode or changes behavior version.
- Mode activation references `MODE_ACTIVATION_P95_BUDGET_MS` through the registry API, but Phase 16.5 does not add a hard latency CI gate.
- Patterns and document font defaults are static metadata only: no callbacks, concrete font values, client predicates, filesystem scans, or raw ops.
- The Rust client receives future validated behavior/protocol data; it does not select or execute package modes itself.

## Tests

- `tests/package_primitive_gate.rs`: validates Markdown extension classification, duplicate mode rejection, malformed pattern rejection, one-major-mode activation replacement, behavior-version increments, and required permissions.
- `tests/markdown_mode.rs::core_and_markdown_modes_publish_semantic_document_font_defaults`: verifies `core.code` monospace plus `core.text`/Markdown proportional manifests.
- `tests/command_execution.rs`: Phase 18.9 mode-discovery commands — `modes.explainActiveMode` reports `core.code` built-in fallback rationale when no language package matched (and `core.text` universal fallback), `modes.listActiveModes` reports package vs `core` built-in provenance with classification source, unknown documents return `None`, and discovery commands reject no-authority violations (invalid arguments, unauthorized workspace target, non-discovery/bogus command IDs).
- `tests/package_primitive_gate.rs`: Phase 18.9 Task 8 default loading experience — with a fresh `ModeRegistry::new()` (absent/empty `init.js`), `.txt` activates `core.text` (Fallback) and `.rs` activates `core.code` (Extension), both remaining editable (`core.text` ships `minimal_text_editing` with no electric chars; `core.code` ships `core_code_editing` with electric outdent rules for `}`/`)`/`]`) with no `loadPackage` step; with markdown registered (simulating `loadPackage("@clay/markdown")`), `.md` activates the Markdown package mode while `.rs` still uses `core.code` (packages extend, not replace, the built-in fallback).
- `tests/package_primitive_gate.rs`: Phase 18.9 Task 7 reclassification + stale behavior-version + payload budget — disabling a language package mid-session reclassifies an open document to `core.text` fallback with a strictly greater behavior version (no stale active mode bypasses validation); reactivation serves a strictly newer manifest version (registry-level stale-version precondition, paired with the connection-layer `EditRejection::InvalidBehaviorVersion` test); fallback (`core.text`/`core.code`) behavior-manifest payloads stay within `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`.
- `tests/editor_performance_invariants.rs`: Phase 18.9 Task 7 advisory budget alignment — `ModeRegistry::activation_budget_ms()` equals `MODE_ACTIVATION_P95_BUDGET_MS` (single source of truth) and `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` orders below mode-activation latency (no sync work before local paint).
- `tests/syntax_grammar.rs`: Phase 18.10 active syntax grammar selection — `.rs` keeps active major mode `core.code` while selecting the `rust` syntax grammar when loaded, exact filenames can attach grammars to `core.text`, unloaded registries fall back to no syntax grammar, grammar selection cannot override mode ID or behavior version, and the deterministic manual-smoke flow verifies `.rs`/`.ts`/`.js` fixtures stay editable under `core.code` while syntax decorations refresh after edits.

Run focused coverage with:

```text
cargo test --test security package_primitive_gate::
cargo test --test runtime command_execution::
cargo test --test editor editor_performance_invariants::
```

## Phase 18.9 mode discovery

`ModeRegistry` exposes two crate-internal read-only discovery entrypoints (`pub(crate)`) used by the built-in `modes.listActiveModes` and `modes.explainActiveMode` commands (reachable through the [Control Center](control-center.md) and resolved through `CommandExecutor::execute_discovery`). They are intentionally not part of the public Rust embedder API and have no Clay JS facade/op wrapper — the commands are the user-facing surface, not these methods:

- `list_active_modes() -> Vec<ActiveModeSummary>` — one entry per open document with an active major mode: `document_id`, `mode_id`, owning `package_name`/`api_prefix`, `provenance` (`CoreBuiltIn` for `core.text`/`core.code` or `Package`), and the `classification_source` (`ModePatternKind`) recorded at activation.
- `explain_active_mode(document_id) -> Option<ModeExplanation>` — detailed explanation: active mode, display name, full package provenance, classification source, `fallback_used` (`true` only for the `core.text` universal fallback), and a human-readable `why` rationale derived generically from the signal + provenance (e.g. "no language package matched; built-in core.code claimed the document via extension").

These read installed registry state only: no filesystem scan, package evaluation, network, shell, AI, WASM, raw ops, or client-side JavaScript. The classification source is recorded on `MajorModeActivation::matched_by` at activation time so discovery never recomputes classification or opens files.

## Related

- [Package Primitive Gate](package-primitive-gate.md)
- [Primitive Architecture](primitive-architecture.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Package Loading](package-loading.md)
- `docs/reference/primitives/registry.md#DocumentClassification`
- `docs/reference/primitives/registry.md#MajorModeActivation`
- `docs/reference/primitives/markdown-mode-requirements.md`
