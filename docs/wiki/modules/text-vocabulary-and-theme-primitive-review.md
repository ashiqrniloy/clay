# Phase 18.15 Text Vocabulary, Two-Axis Decorations, and Theme Registry Primitive Review

## Source

- Plan: `plans/046-Phase-18.15-Text-Vocabulary-Two-Axis-Decorations-and-Theme-Registry.md` (task 2).
- Decision: `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` (components 2–4: vocabulary, two-axis decorations, theme registry).
- Locked vocabulary contract: `docs/reference/primitives/syntax-vocabulary.md` (Plan 046 task 1).
- Skill gate: `.agents/skills/create-plan/references/clay.md` — Primitive-First Mode and Package Task, Package-Provided Grammar Task, Package UI/Layout and Authoring Documentation Task.

## Overview

Phase 18.15 replaces the free-form `style_token: String` on `DecorationSpan` with a two-axis `TokenType` + `Modifiers` model, introduces a `StyleRegistry` as the single source of editor/syntax color, and adds text-style theme contributions plus two shipped Gruvbox Material themes. This review inventories the existing decoration and theme primitives, states exactly what the refactor achieves by reusing them versus which generic primitives it must add, and records the rejected non-generic approaches so later tasks (3–11) cannot drift into per-language branches, validation relaxation, or client-side styling.

## Existing Primitive Inventory

### Decoration transport protocol

`DecorationSpan { byte_start, byte_end, kind, style_token: String, priority, provenance }`, `DecorationKind { Syntax, Semantic, Diagnostic, SearchMatch }`, and `DecorationSet { document_id, document_version, viewport_byte_start, viewport_byte_end, spans }` in `src/protocol/decorations.rs`. All `rkyv::Archive/Serialize/Deserialize`. `DecorationSet::sorted_viewport_first` orders viewport-intersecting spans first by priority then byte range; `chunk_key`/`package_prefix` support chunked caching. `kind` is the decoration **layer** (orthogonal to the upcoming `TokenType`). Source/test: `src/protocol/decorations.rs`, `src/protocol/codec.rs`.

### Decoration payload budget

`DECORATION_PAYLOAD_BUDGET_BYTES = 8192` (advisory) in `src/perf/budgets.rs`, referenced by codec payload checks. Bounds per-message decoration size independent of content.

### Editor decoration state and paint path

`EditorSurface::apply_decoration_set` stores `EditorDecorationState` gated by `document_id` + `document_version` match (mismatches render zero spans). `visible_decoration_ranges(&VisibleSnapshot)` is the single paint-time consumer: it filters spans to the visible byte range and maps each via `decoration_color(span.kind, &span.style_token)` to a `(Range<usize>, Color)`. Fourteen hardcoded `Color` constants (`PANEL_COLOR`, `TEXT_COLOR`, `PLACEHOLDER_COLOR`, `SELECTION_COLOR`, five `SYNTAX_*`, `SEMANTIC_DECORATION_COLOR`, `DIAGNOSTIC_DECORATION_COLOR`, `SEARCH_DECORATION_COLOR`, `CARET_COLOR`, `SCROLLBAR_COLOR`, `SCROLLBAR_TRACK_COLOR`) live in `src/editor/surface.rs`. The locked baseline colors are asserted by `free_form_style_token_decoration_colors_baseline_locked` (Plan 046 task 1). Source/test: `src/editor/surface.rs`.

### SDUI theme token system (typed scalars)

`ThemeTokenType { ColorRole, Spacing, Radius, Typography, Opacity }`, `PackageThemeToken`, `ThemeTokenResolver`, `ResolvedThemeToken`, and `SduiThemeStyle` in `src/shell/theme.rs`. `core_theme_value` holds the core token table (`surface.panel`, `text.primary`, `spacing.row`, `typography.body`, `opacity.disabled`, …). The resolver maps a package-prefixed token to a same-typed core fallback. This system is **SDUI-component oriented**: colors/spacing/opacities resolve as native values, while `typography.body`/`title`/`status` resolve to semantic UI variants. `TypographyRegistry` supplies the configured profile size/family stack, so package tokens never become absolute sizes. It does not model text-decoration styling. `core_token_type` / `core_fallback_matches_type` enforce type agreement. Source/test: `src/shell/theme.rs`.

### Theme token contribution declaration and validation

`ThemeTokenContributionDescriptor { token, token_type, fallback, estimated_payload_bytes }` in `src/packages/record.rs`, parsed by `parse_theme_token_contributions`. Each declaration is bounded by `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` and passed through `reject_ui_prohibited_authority`, which recursively bans `rawOps`, `nativeHandle`, `nativeWidget`, `masonryWidget`, `widgetCallback`, `rendererCallback`, `drawCallback`, `clientHook`, `clientJavaScript`, `javascript`, `code`, `rawCss`, `cssText`, and any string containing `Deno.core.ops` or `op_clay_`. `theme_resolver_for_package_tokens` builds the resolver from declared tokens. Source/test: `src/packages/record.rs`.

### Theme token registration op and facade

`op_clay_ui_register_theme_token` in `src/server/ops/ui.rs` (wired in `src/server/ops/mod.rs`) backs the `serverRegisterThemeToken(manifest, declaration)` facade in `runtime/js/ui.js`. This is the inert runtime registration path packages use to declare typed theme tokens. Source: `src/server/ops/ui.rs`, `src/server/ops/mod.rs`, `runtime/js/ui.js`.

### Decoration production (server side)

The tree-sitter/native handler `decorations_for_window` in `src/server/syntax.rs` emits `DecorationSpan` entries with `style_token` strings (e.g. `keyword.control`) clamped to the viewport and capped at `MAX_SYNTAX_HIGHLIGHT_SPANS`. `schedule_open_parse` drives parse/decoration through `ParseCoordinator`; `open_document_followup_messages` in `src/server/connection.rs` ships the resulting `DecorationSet` to the client. Source: `src/server/syntax.rs`, `src/server/parse_coordinator.rs`, `src/server/connection.rs`.

## Generic Phase 18.15 Primitive Gaps

### Two-axis `TokenType` + `Modifiers` vocabulary

Closed `TokenType` enum (LSP base 23 + Clay prose 12) and `Modifiers` bitflag set (LSP base 10 + Clay text-attribute 4) with an optional open `scope: Option<String>`. Locked in `docs/reference/primitives/syntax-vocabulary.md`. Generic across every language; no per-language variants. New protocol-level primitive added to `src/protocol/decorations.rs` (task 3).

### `StyleSpec` and `StyleRegistry` (text-rendering single source of color)

`StyleSpec { color, bold, italic, underline, strike }` and a `StyleRegistry` mapping `TokenType + Modifiers → StyleSpec` plus base UI colors. This is **distinct from** the SDUI `ThemeTokenResolver` (which resolves typed scalars, not text-style contracts); overloading it would conflate two domains. The registry is the only thing paint reads for color. New `src/editor/theme.rs` primitive (task 4).

### Text-style theme contribution and active-theme application

A text-style contribution kind (token → `StyleSpec` override) validated through the existing inert-contribution plumbing (`ThemeTokenContributionDescriptor` shape, `reject_ui_prohibited_authority`, payload budgets), resolved into the `StyleRegistry` for one active theme at load/reload. Reuses the declaration/validation infrastructure without overloading the SDUI typed-scalar resolver. New contribution kind + resolution path (task 5).

### Free-form `style_token` compatibility mapper

`TokenType::from_style_token(&str) -> (TokenType, Modifiers)` mapping the current families (`keyword.*`→Keyword, `string.*`→String, `comment.*`→Comment, `punctuation.*`→Operator, `markup.heading.N`→HeadingN, `markup.bold`→Paragraph+Bold, default→Variable) so first-party packages keep rendering. Generic prefix mapper, not language-specific. New helper in `src/protocol/decorations.rs` (task 3); baseline colors locked by task 1.

## What the Refactor Achieves with Existing Primitives

- **Transport unchanged:** `DecorationSpan` / `DecorationKind` / `DecorationSet` / rkyv framing / `sorted_viewport_first` / chunk keys are reused as-is; task 3 adds fields, it does not replace the message.
- **Paint gating unchanged:** `EditorDecorationState` document_id/version gating and `visible_decoration_ranges` viewport filter stay; only the inner `decoration_color(kind, style_token)` call becomes `registry.style_for(token_type, modifiers)`.
- **Inert-contribution validation reused:** text-style theme contributions flow through `ThemeTokenContributionDescriptor` + `parse_theme_token_contributions` + `reject_ui_prohibited_authority` + `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` — the identical security boundary SDUI tokens already use.
- **Registration path reused:** the `op_clay_ui_register_theme_token` op / `serverRegisterThemeToken` facade shape is the template for a text-style registration op (parallel op or extended op).
- **Production path unchanged:** server-side `decorations_for_window` / `schedule_open_parse` / `open_document_followup_messages` keep emitting `DecorationSet`; task 3 only changes which fields each span carries.

## Rejected Approaches

- **Per-language Rust branches** for Rust/TypeScript/JavaScript/Markdown colors or token mappings — rejected (Plan 044 no-per-language-Rust-branches constraint; decision 0352 generic pipeline). The closed vocabulary plus the open `scope` escape cover all languages without core branching.
- **Relaxing `StaticSduiState` or decoration validation** to admit theme data — rejected. Theme/text-style data stays inert and budget-bounded; no live objects, callbacks, or op references enter the validated payload.
- **Client-side JavaScript or raw CSS for styling** — rejected. `reject_ui_prohibited_authority` already bans `clientJavaScript`, `rawCss`, `cssText`, `rendererCallback`, `drawCallback`, and `Deno.core.ops`/`op_clay_` strings; the two-axis model keeps styling server-side and inert.
- **Overloading the SDUI `ThemeTokenType` typed-scalar resolver** to emit `StyleSpec` — rejected. It resolves typed scalars (color/spacing/radius/typography/opacity) for SDUI components; folding text-style into it conflates two resolved contracts and breaks the clean separation between SDUI theming and text-decoration theming.
- **Hidden JSON/TOML configuration keys for themes** — rejected. Decision `2026-05-08-1841` requires every configuration option to be a Clay JS API; theme selection is `setTheme("@clay/...")` from `init.js`, and themes are first-party packages.

## Hot-Path Classification

`StyleRegistry` lookup is a cheap per-visible-span read during layout normalization — array-indexed or a small map, no per-glyph lookup. `normalize_visible_text_style_runs` resolves syntax/semantic foreground colors and `LayoutState` caches ranged Parley brushes with the shaped layout. Active-theme resolution happens at package load/reload, never in package JavaScript during typing/rendering. Span production (`decorations_for_window`) keeps its viewport clamp and transport-safe `MAX_SYNTAX_HIGHLIGHT_SPANS` cap; overflow truncates rather than dropping the entire set.

## Security and Authority Boundary

Every theme and text-style contribution is inert data only. The reused `reject_ui_prohibited_authority` boundary bans raw ops, native handles/widgets, renderer/draw/widget callbacks, client hooks, client-side JavaScript, code, and raw CSS at declaration parse time, and rejects any string containing `Deno.core.ops` or `op_clay_`. Payload budgets are enforced (`SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` per declaration, `DECORATION_PAYLOAD_BUDGET_BYTES` per message). `DecorationProvenance` (package name/version/prefix) is retained on every span. Theme selection via `setTheme` grants no filesystem, network, shell, or extension authority beyond loading a first-party `@clay/*` package; arbitrary non-`@clay/*` theme specifiers are denied by default.

## Implementation Follow-up

Plan 046 completed the generic primitives described by this review:

- `DecorationSpan` now carries `token_type: TokenType`, `modifiers: Modifiers`, and optional `scope` compatibility metadata. Legacy `styleToken` producers use `DecorationSpan::from_style_token`.
- `src/editor/theme.rs::StyleRegistry` is the paint-time style source. It stores base UI colors, layer colors, a per-`TokenType` `[Color; 35]` syntax table, and per-token text-attribute defaults.
- `clay.contributions.textStyles` is parsed as inert `TextStyleOverrideDescriptor` data in `src/packages/record.rs`; `reject_ui_prohibited_authority` rejects raw CSS, raw ops, callbacks, native handles, and client JavaScript.
- `@clay/theme-gruvbox-material-dark` and `@clay/theme-gruvbox-material-light` ship as first-party inert packages with full 48-entry mappings (13 base UI keys + 35 token types).
- `theme.setTheme` selects one active first-party theme from `init.js`, stores an `ActiveTheme` snapshot, and the client converts it to `StyleRegistry` before first paint.

Implementation details live in [Editor Theme Registry](editor-theme-registry.md). Public authoring docs live in [Text Vocabulary and Two-Axis Decoration Contract](../../reference/primitives/syntax-vocabulary.md) and [Phase 18.15 theme authoring](../../reference/packages/creating-packages.md#phase-1815-theme-authoring-textstyles-and-settheme).

## References

- `docs/wiki/modules/editor-theme-registry.md` — final Phase 18.15 implementation wiki.
- `docs/reference/primitives/syntax-vocabulary.md` — locked two-axis vocabulary contract.
- `src/protocol/decorations.rs`, `src/protocol/codec.rs` — decoration transport and codec.
- `src/editor/surface.rs` — `decoration_color`, the 14 color constants, `visible_decoration_ranges`, baseline lock test.
- `src/shell/theme.rs` — SDUI `ThemeTokenType` / `ThemeTokenResolver` / `SduiThemeStyle`.
- `src/packages/record.rs` — `ThemeTokenContributionDescriptor`, `parse_theme_token_contributions`, `reject_ui_prohibited_authority`.
- `src/server/ops/ui.rs`, `runtime/js/ui.js` — `op_clay_ui_register_theme_token` / `serverRegisterThemeToken`.
- `src/perf/budgets.rs` — `DECORATION_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.
- Plan 046 tasks 3–11 — implementation tasks that consume this review.
