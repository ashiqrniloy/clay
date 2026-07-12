# Clay Text Vocabulary and Two-Axis Decoration Contract

Status: **Locked contract** — implemented by Phase 18.15 (Plan 046 tasks 1, 3, 4, 5, 6, and 7). Package/theme authoring guidance is also covered in [Creating Clay Packages](../packages/creating-packages.md#phase-1815-theme-authoring-textstyles-and-settheme).

Decision source: `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` (components 2–4).

## Purpose

Clay decorations use a two-axis vocabulary so parsers, LSP bridges, themes, and the native renderer agree on stable text categories without per-language renderer branches.

A decoration says **what text is** (`TokenType`) separately from **which attributes apply** (`Modifiers`). A theme then resolves that inert metadata through `StyleRegistry` into a `StyleSpec` (`color`, `bold`, `italic`, `underline`, `strike`).

```rust
DecorationSpan {
    token_type: TokenType::Function,
    modifiers: Modifiers::DECLARATION | Modifiers::BOLD,
    scope: None,
    kind: DecorationKind::Syntax,
    // byte range, priority, provenance...
}
```

`kind` (`Syntax`, `Semantic`, `Diagnostic`, `SearchMatch`) is the **decoration layer**. It is orthogonal to the token vocabulary: diagnostics/search/semantic layers may color by layer, while syntax uses the token table.

## Axis 1: `TokenType`

`TokenType` is a closed Rust/protocol enum in `src/protocol/decorations.rs`. First-party packages and future LSP packages should map grammar/LSP output to these names.

### LSP base `SemanticTokenType` set

Clay mirrors the Language Server Protocol `SemanticTokenType` base set:

`Namespace, Type, Class, Enum, Interface, Struct, TypeParameter, Parameter, Variable, Property, EnumMember, Event, Function, Method, Macro, Keyword, Modifier, Comment, String, Number, Regexp, Operator, Decorator`

Use these for code modes:

| TokenType | Use for |
| --- | --- |
| `Keyword` | reserved words and control keywords |
| `String` | string literals |
| `Comment` | comments/doc comments; add `Documentation` when appropriate |
| `Function` / `Method` | callable identifiers |
| `Class` / `Struct` / `Enum` / `Interface` | type declarations/references |
| `Variable` / `Parameter` / `Property` | values, function parameters, object/member fields |
| `Number` / `Regexp` / `Operator` / `Decorator` | literal/operator/decorator forms |

### Clay prose extension

Clay owns 12 prose token types for Markdown/rich-text structures not covered by the LSP code vocabulary:

`Heading1, Heading2, Heading3, Heading4, Heading5, Heading6, ListItem, Quote, CodeBlock, CodeSpan, Link, Paragraph`

Use these for text modes. Do not encode bold/italic/underline/strike in the token type; those are `Modifiers`.

## Axis 2: `Modifiers`

`Modifiers` is a fixed `u16` bitfield in `src/protocol/decorations.rs`.

### LSP base `SemanticTokenModifiers` set

Clay mirrors the Language Server Protocol `SemanticTokenModifiers` base set:

`Declaration, Definition, Readonly, Static, Deprecated, Abstract, Async, Modification, Documentation, DefaultLibrary`

### Clay text-attribute extension

Clay adds rich-text attributes as modifiers:

`Bold, Italic, Underline, Strikethrough`

This is the two-axis payoff:

- `TokenType::Function + Modifiers::Bold | Declaration` — bold function declaration.
- `TokenType::Heading1 + Modifiers::Bold` — heading text that is also bold.
- `TokenType::Link + Modifiers::Underline` — link styling without a separate `UnderlinedLink` token.

## Optional `scope` escape

`DecorationSpan.scope: Option<String>` is the open escape for third-party grammar scopes and compatibility strings.

Implemented today:

- Existing free-form `styleToken` strings are preserved in `scope` by `DecorationSpan::from_style_token`.
- Validation still rejects unknown unsafe first-party scope strings at the publication boundary.
- Closed `TokenType` and `Modifiers` drive rendering when no future scope-specific theme rule exists.

Reserved contract:

- A future third-party scope resolver may apply TextMate-style **longest-prefix fallback** (`meta.function-call.arguments` → `meta.function-call` → `meta`), then fall back to `token_type`.
- First-party themes authored in Phase 18.15 target closed `TokenType` names and base UI keys, not open scope patterns.

## Compatibility mapping

Existing `styleToken` producers keep working through `TokenType::classify_style_token` and `DecorationSpan::from_style_token`.

The exact baseline colors are locked by `free_form_style_token_decoration_colors_baseline_locked` in `src/editor/surface.rs`.

| styleToken input | TokenType | Modifiers |
| --- | --- | --- |
| `keyword.control` | `Keyword` | none |
| `string.quoted` | `String` | none |
| `comment.line` | `Comment` | none |
| `punctuation.definition` | `Operator` | none |
| `markup.heading.1`..`markup.heading.6` | `Heading1`..`Heading6` | none |
| `markup.strong` | `Paragraph` | `Bold` |
| `markup.emphasis` | `Paragraph` | `Italic` |
| `markup.inline-code` | `CodeSpan` | none |
| `markup.code-block` | `CodeBlock` | none |
| `markup.list-marker` | `ListItem` | none |
| `text` | `Paragraph` | none |
| `diagnostic.*` / `search.match` | `Variable` | layer color from `kind` |
| unknown valid fallback | `Variable` | none |

## Theme binding

Theme packages declare inert `clay.contributions.textStyles` entries. Each entry targets either:

- a base UI key: `shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`; or
- a `TokenType` variant name such as `Keyword`, `String`, `Function`, `Heading1`, or `Paragraph`.

Each entry may set any subset of:

```json
{ "token": "Keyword", "color": "#d3869b", "bold": true }
```

`color` accepts `#rgb`, `#rrggbb`, or `#rrggbbaa`. Boolean fields are `bold`, `italic`, `underline`, and `strike`.

Theme resolution happens at configuration/package-load time. `setTheme("@clay/theme-gruvbox-material-dark")` selects one active first-party theme, the server sends an inert `ActiveTheme` snapshot, and the client builds `StyleRegistry` before first paint. Paint reads the resolved registry only.

## Invariants

- **Single source of color:** editor/shell paint-path color resolves through `StyleRegistry`. Source-guard tests reject `Color::from_rgb8`/`Color::from_rgba8` literals outside theme-definition modules.
- **Inert data:** tokens, modifiers, scopes, and themes are byte-range/style metadata only. No code, widgets, ops, raw CSS, filesystem, network, or shell authority attaches to a token or theme.
- **Hot path:** `StyleRegistry` lookup is a cheap per-span read during paint — no allocation, no per-glyph map lookup, no package JavaScript.
- **Layered model:** `DecorationKind` chooses the layer; `TokenType` and `Modifiers` choose style inside the layer.
- **Closed first-party vocabulary:** Rust, TypeScript, JavaScript, and Markdown first-party modes map into the LSP base set plus Clay prose extension. Third-party-specific scopes use `scope` as an escape, not a core rebuild.

## References

- `src/protocol/decorations.rs` — `TokenType`, `Modifiers`, `DecorationSpan`, compatibility mapping.
- `src/editor/theme.rs` — `StyleSpec`, `StyleRegistry`, theme override resolution.
- `src/packages/record.rs` — `TextStyleOverrideDescriptor` validation for `clay.contributions.textStyles`.
- `runtime/js/theme.ts` and `docs/reference/clay-js-api/theme/set-theme.md` — `setTheme()`.
- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`.
