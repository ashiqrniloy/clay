# Phase 18 Markdown Primitive Review

## Source

- `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`
- `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/mode-registry.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/rendering-primitives.md`
- `docs/wiki/modules/first-party-markdown-package.md`

## Overview

This page records the primitive-first review that must happen before the Phase 18 `@clay/markdown` package rewrite continues. The review confirms that Markdown behavior should be built on existing generic Clay primitives where possible, and that any Rust-side follow-up must be reusable by future modes such as Python, Org, AsciiDoc, RST, or other language packages.

The active parser decision is to replace the mdast adapter with a package-owned `markdown-it` token-stream adapter. Rust must not learn about Markdown syntax, markdown-it token names, headings, fences, list markers, or parser-specific range recovery. Rust remains responsible for package validation, scheduling, protocol validation, and native rendering of inert data.

## Existing Primitive Inventory

| Primitive area | Current source paths | What Markdown can already do | Timing / hot-path policy | Permissions and validation boundary |
| --- | --- | --- | --- | --- |
| Package identity, permissions, and provenance | `src/packages/manifest.rs`, `src/packages/record.rs`, `src/packages/service.rs`, `docs/reference/primitives/package-security.md` | Validate `@clay/markdown`, prefix `markdown`, entries, docs path, declared modes, contribution descriptors, and API dependencies before load/enable. | Install/enable/load/configuration time only; no keypress, paint, scroll, layout, or text-event work. | Declared permissions only: `mode-registration`, `mode-activation`, `command-registration`, `parse-document`, and `render-decorations`; prohibited filesystem/network/shell/AI/WASM/raw-op/client-JS authorities remain rejected. |
| Document classification | `src/packages/modes.rs`, `runtime/js/modes.ts`, `docs/wiki/modules/mode-registry.md` | Classify `.md`, `.markdown`, `.mdown`, and `text/markdown` from static package metadata without package JavaScript or filesystem scans. | Open/reload/explicit reclassification only; not per keypress. | Requires `mode-registration`; duplicate or malformed mode patterns are rejected with package provenance. |
| Major-mode activation | `src/packages/modes.rs`, `src/server/ops/modes.rs`, `runtime/js/modes.ts` | Activate one Markdown major mode for a document and publish package-declared editor rules, commands, and keymaps as a generic behavior manifest. | Open/reload/configuration time; installed manifest then drives local behavior without server/package work in paint. | Requires `mode-activation`; one-major-mode invariant and behavior-version metadata are server-owned. |
| Command declaration and key routing | `src/packages/commands.rs`, `src/server/ops/commands.rs`, `runtime/js/commands.ts`, `runtime/js/keybindings.ts` | Register `markdown.*` commands and package-owned key bindings as discoverable server-routed intents. | Load/activation time for metadata; command execution is server-routed and not a local paint prerequisite. | Requires `command-registration`; duplicate commands, ambiguous key bindings, undeclared permissions, and client-first package command authority are rejected. |
| Inert text transforms | `src/protocol/mod.rs`, `src/editor/surface.rs`, `src/server/ops/modes.rs`, `docs/wiki/modules/markdown-mode-activation.md` | Declare generic `EnterRule::ContinueLineMarkers`, `EnterRule::PreserveFenceBodyIndent`, and `PairRule` data from package JSON. Pair rules already execute locally; list/fence variants are generic transform data and must stay language-neutral. | `ClientFirstPredictable` manifest data only; no package JavaScript or IPC before normal local paint. | No extra permission for inert transform declarations; executable `callback`, `code`, `javascript`, or `hook` fields are rejected. |
| Parse handler registration and scheduling | `src/protocol/parse.rs`, `src/server/parse_coordinator.rs`, `src/server/ops/parse.rs`, `runtime/js/parse.ts`, `docs/wiki/modules/parse-coordinator.md` | Register a server-side parse handler for `markdown`, schedule cancellable per-document background work, prioritize invalidated ranges that intersect the viewport, and reject stale/oversize results. | Background only; scheduling records metadata and spawns work after edits/viewport changes are accepted. It does not block edit acknowledgement, keypress, text events, scroll, or paint. | Requires `parse-document`; validates package provenance, mode ID, viewport ranges, invalidated ranges, stale versions, payload budget, timeout bounds, and executable callback fields. |
| Decoration publication and rendering | `src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/server/ops/decorations.rs`, `runtime/js/decorations.ts`, `src/editor/surface.rs`, `src/editor/layout.rs`, `docs/wiki/modules/decoration-transport.md` | Publish viewport-bounded `DecorationSpan` records for generic syntax/style tokens such as `markup.heading.1`, `markup.strong`, `markup.emphasis`, `markup.inline-code`, `markup.code-block`, `markup.list-marker`, `keyword.control`, `string.quoted`, `comment.line`, and `punctuation.definition`; render them locally as native inert editor decorations. | Publication is background/server-side; client applies validated updates before paint and paint consumes local cached spans only. | Requires `render-decorations`; validates document version, viewport bounds, byte ranges, known inert style tokens/kinds, package provenance, and `DECORATION_PAYLOAD_BUDGET_BYTES`. |
| SDUI preview/status | `src/protocol/sdui.rs`, `src/server/sdui.rs`, `runtime/js/sdui.ts`, `packages/markdown/dist/sdui.js`, `docs/wiki/modules/server-driven-ui.md` | Publish Markdown preview/status panels, labels, editor bindings, lists, and command-targeting buttons as inert server-validated SDUI trees. | Load/configuration/background UI update path only; native SDUI paint renders already-validated nodes. | Inert SDUI grants no extra authority; actions must target already registered package commands and payloads must fit SDUI budgets. |
| Configuration surfaces | `runtime/js/configuration.ts`, `src/server/configuration.rs`, `docs/reference/primitives/registry.md`, `docs/wiki/modules/configuration-runtime.md` | Keep current Markdown settings fixed for the POC unless real user-visible settings are introduced later; planned APIs (`setPackageOption`, `setModePreference`, `setDecorationTheme`, `setParsePolicy`) are generic Clay JS configuration surfaces. | Startup/reload/explicit configuration work only; no keypress/paint/text-event work. | Configuration cannot grant package enable/disable, filesystem, network, shell, AI, workspace mutation, raw ops, WASM, or client-side JavaScript authority. |
| Documentation and registry coverage | `docs/reference/primitives/**`, `docs/wiki/index.md`, `tests/primitives_docs.rs`, `tests/package_primitive_gate.rs` | Keep primitive reference docs, implementation wiki pages, API inventory, generated registry, and deterministic tests discoverable before package-specific work proceeds. | Test/docs maintenance only; no runtime work. | Tests must fail instead of mutating docs silently when primitive coverage or index links are stale. |

## Generic Primitive Gaps Before Package Work

No Markdown-specific Rust parser, renderer, token mapper, heading/list/fence branch, or style map should be added for the `markdown-it` rewrite. The existing primitives are enough for the parser adapter to parse package-provided text, build a package-owned source/line index, convert token-derived source ranges to UTF-8 byte ranges, and publish viewport-bounded decoration spans.

The review found only generic follow-up gaps:

1. **Complete generic text-transform engines for declared list/fence rules.** `ContinueLineMarkers` and `PreserveFenceBodyIndent` are already language-neutral manifest variants, but the current editor path still falls back to leading-whitespace preservation for those variants. If Phase 18 needs the actual Markdown list/fence editing behavior, implement it as reusable Rust-known transform engines driven entirely by the existing generic fields (`markers`, `exit_on_empty_item`, and `fence_markers`). Do not add `MarkdownTransformRules`, Markdown parser calls, or mode-name branches.
2. **Keep parse input metadata generic if runtime handler execution needs more than current scheduling metadata.** `ParseEditNotification` currently carries document/version/behavior/package/mode, viewport, and invalidated ranges. If the runtime-backed handler path needs source delivery, range snapshots, inserted-text previews, or line-start metadata, add a language-neutral parse-input/range-snapshot/line-index primitive that can serve Markdown, Python, and other packages. Do not add a `markdown-it` or Markdown-only request shape. The first `markdown-it` adapter should compute line starts inside package JavaScript from the supplied text unless profiling proves a generic line-index primitive is required.
3. **Remove mode-specific fallback defaults from generic package ops when touched.** Generic op helpers should require explicit package/mode provenance or derive it from a validated package manifest. Existing examples/defaults that silently assume `markdown` should be cleaned up when the package integration path is edited, but the replacement must be package-neutral. The Phase 18 token-stream primitive verification changed decoration publication fallback metadata to derive the mode from the package API prefix when an explicit mode is not provided; it no longer defaults to `markdown`.
4. **Defer style-token expansion to a generic decoration/theme registry.** The current known tokens cover Markdown POC syntax spans. Future language packages may need more tokens, but that should be a package-neutral style-token/decoration-theme registry or configuration surface, not a Rust branch for a single language.

## Decision for the markdown-it Rewrite

Proceed with package work using existing generic primitives first:

- Build the `markdown-it` token-stream adapter entirely in `packages/markdown/dist/parser.js` and matching source files.
- Use package-owned source scanning and line-start tables for token-to-byte-range mapping.
- Publish only Clay `DecorationSpan` data through `clay.decorations.serverPublishDecorations`.
- Register parse behavior through `clay.parse.serverRegisterParseHandler` without executable callback fields in registration payloads.
- Keep SDUI and commands package-prefixed and inert.
- Update primitive docs/tests only if a generic primitive contract changes.

## Token-Stream Adapter Primitive Verification

The markdown-it token stream does not require a Markdown-specific Rust primitive. Context7 `/markdown-it/markdown-it` documentation confirms that markdown-it exposes block token streams, inline child token streams, and Token fields such as `map`, `markup`, `content`, `nesting`, `block`, and `hidden`; the package adapter can interpret those tokens in JavaScript and publish only generic Clay decoration spans.

Phase 18 verified the generic primitives needed by token-stream adapters:

- `ParseEditNotification` and `ParseScheduleRequest` carry document ID, document version, behavior version, package prefix, mode ID, viewport byte range, and invalidated byte ranges. A Python-mode fixture proves the metadata is package-neutral and viewport-intersecting invalidated ranges are delivered first.
- `DecorationRange` validation accepts inert syntax spans from a non-Markdown language package (`@clay/python`) using generic code style tokens such as `keyword.control` and `string.quoted`; Markdown spans still use `markup.*` tokens.
- Generic package fallback metadata in decoration publication derives the default mode from the package API prefix rather than a hard-coded Markdown mode.
- Rust parser/decoration primitives remain free of markdown-it token branches such as `heading_open`, `list_item_open`, `strong_open`, `em_open`, and `code_inline`.

Any future Rust-side implementation added because of this review must be named for the reusable primitive, not for Markdown. Acceptable names include `ParseRangeSnapshot`, `ParseLineIndex`, `StyleTokenRegistry`, or a completed `ContinueLineMarkers` engine. Rejected names include `MarkdownParser`, `MarkdownHeading`, `MarkdownFence`, `MarkdownItToken`, or any `if mode == "markdown"` parser/rendering path.

## Verification

- Inventory reviewed: package loading, mode registry, command/key routing, behavior manifests/text transforms, parse coordinator, decoration transport/rendering, SDUI, configuration, package permissions, and documentation coverage.
- Hot-path review: package validation and mode activation run at load/open/reload/configuration time; parsing and decoration publication are background/viewport-prioritized; SDUI updates are out-of-band; paint/text-event/key routing consume inert local state and must not invoke package JavaScript or IPC before local paint.
- Security review: Markdown package work remains limited to declared package permissions (`mode-registration`, `mode-activation`, `command-registration`, `parse-document`, `render-decorations`) and cannot gain filesystem, network, shell, AI, WASM, raw-op, native-widget, package enable/disable, workspace mutation, or client-side JavaScript authority.
- Primitive gap review: no new Markdown-specific Rust primitive is required before the `markdown-it` package rewrite. Generic follow-ups are recorded above and must be implemented with reference docs, wiki coverage, and deterministic tests if selected by later tasks.

## Tests

- `tests/primitives_docs.rs::phase18_markdown_primitive_review_records_existing_inventory`
- `tests/primitives_docs.rs::phase18_markdown_primitive_review_records_generic_gaps_only`
- `cargo test --test primitives_docs`
- `cargo test --test package_primitive_gate`
- `cargo test --test decoration_transport`
- `cargo test --test parse_coordinator`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Rendering Primitives](rendering-primitives.md)
- [First-Party Markdown Package](first-party-markdown-package.md)
