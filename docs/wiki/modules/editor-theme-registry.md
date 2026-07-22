# Editor Theme Registry

## Source

- `src/editor/theme.rs`
- `src/editor/typography.rs`
- `src/editor/layout.rs`
- `src/editor/surface.rs`
- `src/protocol/decorations.rs`
- `src/protocol/mod.rs`
- `src/packages/record.rs`
- `src/server/ops/theme.rs`
- `src/server/ops/mod.rs`
- `src/server/js_runtime.rs`
- `src/server/mod.rs`
- `src/server/connection.rs`
- `src/client/mod.rs`
- `src/masonry_editor.rs`
- `runtime/js/theme.js`
- `packages/theme-gruvbox-material-dark/package.json`
- `packages/theme-gruvbox-material-light/package.json`
- `tests/theme_packages.rs`
- `tests/decoration_transport.rs`
- `tests/editor_performance_invariants.rs`
- `tests/primitives_docs.rs`

## Overview

Phase 18.15 moves editor text styling from scattered paint constants and free-form decoration style strings to one resolved `StyleRegistry`. The registry is the client-side single source of color and text attributes for editor chrome, syntax/prose decorations, diagnostics, search matches, caret, selection, scrollbars, and status chrome.

The public configuration surface is [`clay.theme.setTheme`](../../reference/clay-js-api/theme/set-theme.md). Theme packages are inert first-party packages that declare static `clay.contributions.textStyles` entries. `setTheme("@clay/theme-gruvbox-material-dark")` selects one active theme during `init.js`; server bootstrap sends an inert `ActiveTheme` snapshot; the client resolves that snapshot into a `StyleRegistry` before first paint.

The authoritative package authoring and vocabulary references are:

- [Text Vocabulary and Two-Axis Decoration Contract](../../reference/primitives/syntax-vocabulary.md)
- [Package styleMap authoring (vocabulary captures)](../../reference/primitives/syntax-vocabulary.md#package-stylemap-authoring)
- [Phase 18.15 theme authoring: `textStyles` and `setTheme`](../../reference/packages/creating-packages.md#phase-1815-theme-authoring-textstyles-and-settheme)

## Responsibilities

- Resolve two-axis decoration data (`TokenType` + `Modifiers`) into `StyleSpec { color, bold, italic, underline, strike }`.
- Store base editor UI colors (`shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`).
- Resolve range-diagnostic severity colors through `StyleRegistry::diagnostic_style(DiagnosticSeverity)` for native squiggle paint.
- Preserve legacy style-token compatibility through `DecorationSpan::from_style_token` and `TokenType::classify_style_token` while rendering through the new vocabulary.
- Apply static theme-package `textStyles` over the Clay default with last-wins ordering.
- Keep paint hot paths free of package JavaScript, raw IPC, package resolution, filesystem access, allocation-heavy maps, and raw color literals outside theme definitions.
- Keep SDUI component theme tokens separate from editor text theming. `src/shell/theme.rs::ThemeTokenResolver` styles SDUI typed scalars; `StyleRegistry` styles editor text/decorations/base UI.

## How It Works

### 1. Decoration spans carry vocabulary, not renderer colors

`src/protocol/decorations.rs` defines:

- `TokenType`: 23 LSP `SemanticTokenType` variants plus 12 Clay prose variants (`Heading1` through `Paragraph`).
- `Modifiers`: a `u16` bitfield for LSP modifiers plus Clay text attributes (`BOLD`, `ITALIC`, `UNDERLINE`, `STRIKETHROUGH`).
- `DecorationSpan`: byte range, `DecorationKind`, `token_type`, `modifiers`, optional compatibility `scope`, priority, and provenance.

Existing producers that still have a free-form `styleToken` use `DecorationSpan::from_style_token`. That constructor classifies known legacy strings such as `keyword.control`, `string.quoted`, `markup.heading.1`, `markup.strong`, and `markup.emphasis` into the closed vocabulary while preserving the original string in `scope` for validation/compatibility.

### 2. `StyleRegistry` resolves style once, then paint reads it cheaply

`src/editor/theme.rs` owns:

- `StyleSpec`: final per-span style result.
- `BaseUiColors`: editor shell/panel/text/status/chrome colors.
- `StyleRegistry`: default layer colors, per-token syntax color table, and per-token text-attribute defaults.

The registry stores syntax colors in a `[Color; 35]` table indexed by `TokenType::index()`. The Clay default table reproduces the old family mapping for code tokens (`Keyword`, `String`, `Comment`, `Operator`, default syntax); the prose palette was revised in Plan 059 task 3 from uniform muted green to differentiated styling — headings step through red/yellow/green/blue/purple/teal and are bold by default, `ListItem` is gray, `Quote` is italic gray, `CodeBlock` green, `CodeSpan` yellow, `Link` underlined blue — while active themes can override every token independently. `Paragraph` deliberately resolves to `base.text`, so Markdown's broad `(paragraph) @text` capture preserves normal editor text color instead of tinting every prose line green; headings, links, code, lists, and other constructs remain decorated. Active prose themes can still override every token independently.

`StyleRegistry::style_for(kind, token_type, modifiers)` is the paint-time lookup. `DecorationKind::Diagnostic`, `SearchMatch`, and `Semantic` use layer colors. `DecorationKind::Syntax` reads the per-token table. Text attributes are ORed from theme defaults and span modifiers. `StyleSpec::attributes()` exposes only those four inert booleans to the role-aware layout path; `StyleRegistry` still owns no font family, size, or font-role decision.

### 3. Theme packages contribute inert `textStyles`

The shipped first-party themes are `@clay/theme-gruvbox-material-dark` and `@clay/theme-gruvbox-material-light`.

`src/packages/record.rs` parses `clay.contributions.textStyles` into `TextStyleOverrideDescriptor` values. Validation rejects:

- unknown base UI keys or `TokenType` names;
- invalid hex color strings;
- duplicate token entries;
- entries with no override fields;
- raw CSS/color escape keys (`rawColor`, `value`, `css`, `rawCss`, `cssText`);
- executable/native authority fields through `reject_ui_prohibited_authority`.

Descriptors store color as `[u8; 4]` so package records remain `Eq`-friendly. `to_override()` converts descriptors into editor-side `TextStyleOverride` when a theme is resolved.

Theme packages require no special permission. They are inert manifest data plus no-op ESM entry/load files so package discovery/classification can load them without throwing.

### 4. `setTheme` resolves one active theme

`runtime/js/theme.js::setTheme` is the public facade. It calls `op_clay_theme_set_theme`, but raw `Deno.core.ops` is not the user-facing API.

`src/server/ops/theme.rs::op_clay_theme_set_theme`:

1. validates the request shape;
2. denies non-`@clay/*` specifiers;
3. reuses first-party package resolution from `src/server/ops/packages.rs`;
4. reads validated `record.contributions.text_styles`;
5. stores `ActiveTheme { specifier, overrides }` in `ClayOpState`.

`ClayRuntimeEvaluation` carries the active theme out of runtime evaluation. `IpcServer::apply_runtime_evaluation` stores it in shared server state. During connection bootstrap, `send_welcome_snapshot_and_manifest` sends `ServerMessage::ActiveTheme`; if no theme was selected, it sends `@clay/default` with zero overrides.

### 5. Client applies the registry before first paint and on reload

`src/client/mod.rs::load_initial_state_from_stream` expects `ActiveTheme` after the initial `BehaviorManifest` and stores it in `ClientInitialState`. `EditorWidget::with_initial_state` calls `EditorSurface::set_theme(StyleRegistry::from_active_theme(&initial_state.active_theme))` before first paint.

`run_connection` also maps later `ServerMessage::ActiveTheme` frames to `ClientConnectionEvent::ActiveTheme`. `EditorWidget::apply_connection_event` applies them by rebuilding the registry from the inert snapshot. That lets runtime reload/theme changes replace the active registry without introducing package JavaScript into paint/input.

### 6. Paint uses the registry

`src/editor/surface.rs` owns an `EditorSurface { theme: StyleRegistry, ... }`. Paint reads:

- `self.theme.base.panel_bg` for editor background;
- `self.theme.base.text` and `placeholder` for text;
- `self.theme.base.selection`, `caret`, `scrollbar`, `scrollbar_track` for chrome;
- `self.theme.style_for(span.kind, span.token_type, span.modifiers).color` for visible decoration rectangles.

Phase 18.16.5 also turns the same `StyleSpec` attributes into viewport-bounded `VisibleTextStyleRun` records. `EditorSurface` clips cached spans to UTF-8 boundaries, rejects invalid/out-of-document ranges locally, splits at visible boundaries, ORs bold/italic/underline/strike, chooses a font role only from syntax/semantic spans by priority, layer, then stable provenance, and merges adjacent equal records. `LayoutState` retains those normalized runs beside its cached Parley layout and pushes only Clay-resolved `FontStack`, `FontSize`, `FontWeight`, `FontStyle`, `Underline`, and `Strikethrough` properties. Diagnostics and search may still paint their rectangles and attributes but cannot select a font role. The cache key includes text/viewport revisions plus typography revision, layout-style revision, default document role, and width, so a paint cache hit does not rescan decoration spans.

`TypographyRegistry::document_line_height()` owns the editor's conservative logical geometry: it takes the larger configured monospace/proportional size and Clay's single multiplier for visible-line estimation, pixel-scroll progression, and logical scrollbar progress. Parley continues to supply exact visible layout height and caret/selection rectangles; the empty placeholder caret uses the same Parley geometry rather than a fixed size.

`src/masonry_editor.rs` reads `self.editor.theme().base.status_bg/status_text/shell_bg` for status and root background. Source guards in `tests/editor_performance_invariants.rs` reject new `Color::from_rgb8`/`Color::from_rgba8` literals in paint-path files.

Phase 18.17 adds severity-owned base UI keys `diagnosticError` / `diagnosticWarning` / `diagnosticInfo`. `StyleRegistry::diagnostic_style(severity)` supplies squiggle colors; `LayoutState::paint_text` strokes zig-zag marks from cached Parley rectangles without hardcoded paint-path colors. Details: [Range Diagnostics](range-diagnostics.md).

## Code Examples

Theme selection in `~/.config/clay/init.js`:

```js
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
```

A theme package `textStyles` entry:

```json
{ "token": "Keyword", "color": "#d3869b", "bold": true }
```

Client registry application:

```rust
let registry = StyleRegistry::from_active_theme(&active_theme);
editor_surface.set_theme(registry);
```

Decoration rendering path:

```rust
let style = self.theme.style_for(span.kind, span.token_type, span.modifiers);
let color = style.color;
```

## Primitive Coverage

- **Primitive/category:** editor text theme registry and two-axis decoration styling.
- **Owner:** `src/editor/theme.rs` plus protocol vocabulary in `src/protocol/decorations.rs`.
- **Public JS API:** `clay.theme.setTheme` in `runtime/js/theme.js`; authoritative docs in `docs/reference/clay-js-api/theme/set-theme.md`.
- **Deno op:** `op_clay_theme_set_theme` in `src/server/ops/theme.rs`.
- **Protocol shape:** `TextThemeOverride` and `ActiveTheme` in `src/protocol/mod.rs`; sent as `ServerMessage::ActiveTheme`.
- **Package contribution:** `clay.contributions.textStyles` parsed by `src/packages/record.rs`.
- **Permissions:** none for theme packages; `setTheme` only resolves bundled first-party `@clay/*` themes.
- **Validation:** known token/base key, valid hex, duplicate rejection, no-op rejection, executable/raw CSS/native authority rejection, manifest payload budget.
- **Hot path:** resolved once at configuration/reload/bootstrap; normalized visible presentation runs are retained with the Parley layout, so cache-hit paint does no package JavaScript, package loading, filesystem, server IPC, font-family parsing, or decoration-span rescan.
- **Reuse rule:** future modes emit `TokenType` + `Modifiers`; future themes declare `textStyles`; no per-language Rust color branches, paint branches, or raw CSS hooks.

## Invariants and Constraints

- `StyleRegistry` is the single color source for editor paint paths.
- `DecorationKind` is a layer; `TokenType` and `Modifiers` are text meaning/attributes. Do not encode diagnostic/search semantics as token names.
- `scope` is compatibility/escape metadata, not the first-party rendering contract.
- Theme packages are inert data. They cannot run renderer code, install native widgets, call raw ops, read files, access network/shell, or mutate workspaces.
- non-`@clay/*` specifiers are denied by `setTheme` until third-party theme installation/authority is designed.
- `TextStyleOverrideDescriptor` keeps protocol/package records independent of `peniko::Color`; wire data uses RGBA bytes.
- `ServerMessage::ActiveTheme` is part of bootstrap. Tests that wait for later capability or SDUI messages must skip or consume it intentionally.

## Tests

- `src/editor/theme.rs`: default baseline, kind/token dispatch, modifier attributes, hex parsing, override routing, last-wins merge, unknown-token no-op, and theme text-attribute defaults.
- `src/editor/surface.rs`: mixed Markdown-code role runs, deterministic overlap/attribute composition, and diagnostic/invalid-UTF-8 font-role rejection; `src/editor/layout.rs` covers typography/style/default-role cache invalidation.
- `tests/theme_packages.rs`: Gruvbox Material Dark/Light validate as inert full 48-entry mappings, produce distinct palettes, change the registry, make keywords bold, and preserve per-prose-token color overrides.
- `tests/decoration_transport.rs`: two-axis `DecorationSpan` protocol round trip and compatibility construction.
- `tests/editor_performance_invariants.rs`: `style_registry_is_single_source_of_color_for_paint_paths` and `paint_uses_cached_inert_spans_without_package_javascript`.
- `src/server/js_runtime.rs`: `set_theme_resolves_first_party_gruvbox_theme` validates runtime facade/op resolution.
- `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`: Clay JS API documentation, generated registry, and facade coverage.
- `tests/selected_file_markdown_smoke.rs`: selected-file bootstrap skips `ActiveTheme` while waiting for file-open capability.
- Commands: `cargo test --lib editor::theme`, `cargo test --test editor theme_packages::`, `cargo test --test editor decoration_transport::`, `cargo test --test editor editor_performance_invariants::`, `cargo test --test protocol`.

## Phase 20 verification

Roadmap Phase 20's "theme system" item is **satisfied by Phase 18.15**. Phase 20 only verified completeness and landed accessibility/theme polish: status/shell chrome already reads `StyleRegistry` base colors; `theme_display_label` / `SduiStatusObservation.theme_label` expose the active specifier; `status_chrome_meets_contrast` locks WCAG AA contrast for Clay default and Gruvbox Material status chrome. Do not invent a second theme registry.


## Related

- [Phase 18.15 Text Vocabulary, Two-Axis Decorations, and Theme Registry Primitive Review](text-vocabulary-and-theme-primitive-review.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Text Vocabulary and Two-Axis Decoration Contract](../../reference/primitives/syntax-vocabulary.md)
- [Package styleMap authoring (vocabulary captures)](../../reference/primitives/syntax-vocabulary.md#package-stylemap-authoring)
- [Theme authoring guide](../../reference/packages/creating-packages.md#phase-1815-theme-authoring-textstyles-and-settheme)
- [`clay.theme.setTheme`](../../reference/clay-js-api/theme/set-theme.md)
