# Semantic Typography Roles

Phase 18.16.5 defines one reusable typography contract for document modes, syntax/semantic ranges, Clay shell UI, SDUI, and package components. Packages select semantic roles; user configuration owns concrete font families and sizes.

## Ownership

| Surface | Package declaration | Default | Concrete resolution owner |
| --- | --- | --- | --- |
| Document/mode text | `defaultFontRole: "monospace" \| "proportional"` | `core.code`: monospace; `core.text`: proportional | User `monospace` or `proportional` profile |
| Syntax/semantic range | `fontRole: "monospace" \| "proportional"` | Inherit document role | Same user profile selected by role |
| Clay/package component text | `style.fontRole: "ui" \| "monospace" \| "proportional"` | UI | Selected user profile |
| UI text scale | semantic `typography.body`, `typography.title`, `typography.status`, `typography.display`, `typography.section`, `typography.detail`, or `typography.caption` | Kind-specific semantic variant | Clay scales selected profile via the user-owned [`UiTypographyHierarchy`](#ui-typography-hierarchy); package supplies no pixels |

One role selects both ordered family fallback stack and logical-pixel size. Packages cannot split family and size ownership.

User configuration is documented by [`theme.setTypography`](../clay-js-api/theme/set-typography.md). Its `monospace`, `proportional`, and `ui` profiles are complete user-owned values. No package permission allows overriding them.

## Ligature Policy

Each `FontProfile` carries a `ligatures` policy (Plan 071 task 7) that resolves to editor font-feature settings at render time. Ownership mirrors the role table above: ligatures are **user-owned typography baseline**, not mode- or package-owned.

- **Semantic toggles first**: `enableStandard` (maps to `liga` + `clig`) and `enableContextual` (maps to `calt`) cover the ordinary ligature decision. `discretionaryFeatures`/`disableFeatures` accept bounded OpenType tag lists, and `rawFeatures` accepts a bounded CSS-font-feature string as the escape hatch for stylistic alternates (`ss0X`, `cv0X`, `zero`, `onum`, ...).
- **Default behavior**: a `setTypography` profile that omits `ligatures`, and every profile before the first `setTypography` call, resolves to `LigaturePolicy::default()` — standard and contextual ligatures enabled. Disabling ligatures is explicit user configuration (`enableStandard: false, enableContextual: false`), never an implicit default.
- **Mode/package surface**: a mode's `defaultFontRole` selects which profile's policy applies to its document text (Markdown → proportional; code modes → monospace). Packages never set ligature policy directly, and no package capability grants that authority — role selection is the only package-side lever, per the [authoring contract](ui-chrome-primitives.md#package-authoring-contract).
- **Cache correctness**: feature resolution is part of the layout cache key; a `setTypography` ligature change invalidates cached layout for the affected role without a typography-revision bump.

## Mode Default

Declare document-wide intent when registering a mode:

```ts
serverRegisterModePattern(packageManifest, {
  modeId: "example-code",
  displayName: "Example Code",
  extensions: ["example"],
  defaultFontRole: "monospace",
});
```

Allowed values are `monospace` and `proportional`. Omission inherits the base behavior manifest. Clay's built-in defaults are:

- `core.code`: `monospace`
- `core.text`: `proportional`
- first-party Markdown: `proportional`
- first-party Rust, TypeScript, and JavaScript: `monospace`

Classification and activation propagate the role through `ModeDeclaration` → `MajorModeActivation` → `BehaviorManifest.document_font_role`; rendering contains no language-name branches.

## Range Override

Only validated syntax and semantic decoration spans may override document text role. Diagnostic, search-match, stale, out-of-bounds, and invalid UTF-8-boundary spans cannot alter it.

A parser may publish a semantic role with a decoration:

```js
serverPublishDecorations({
  // document/version/viewport/provenance fields omitted
  spans: [{
    byteStart: 12,
    byteEnd: 18,
    layer: "syntax",
    styleToken: "markup.inline-code",
    fontRole: "monospace",
  }],
});
```

A syntax grammar `styleMap` may attach the same role to a capture:

```json
{
  "code": { "styleToken": "markup.code-block", "fontRole": "monospace" }
}
```

Allowed range values are `monospace` and `proportional`; omission means inherit. Overlaps normalize into non-overlapping viewport-bounded runs. Font-role precedence is priority, eligible layer (`semantic` above `syntax`), package provenance, then deterministic role rank. Text attributes compose independently.

## Component Text

Text-bearing `panel`, `label`, `button`, `list`, and `statusItem` components may use `style.fontRole`. Default is `ui`. Structural components and `editorView` cannot declare a text font role.

```json
{
  "kind": "label",
  "id": "example.command",
  "text": "cargo test",
  "style": {
    "typography": "typography.body",
    "fontRole": "monospace"
  }
}
```

`typography.body`, `typography.title`, and `typography.status` are semantic scale variants, not absolute sizes. Phase 20.1 adds `typography.display`, `typography.section`, `typography.detail`, and `typography.caption` as additive semantic variants. Native paint, row height, hit testing, scrolling, status geometry, and accessibility bounds derive from the same resolved metrics.

## UI Typography Hierarchy

The seven semantic variants (`display`, `title`, `section`, `body`, `status`, `detail`, `caption`) are scale ratios over the selected role's base size. The complete hierarchy is user-owned and travels atomically with `ActiveTypography` via [`theme.setTypography`](../clay-js-api/theme/set-typography.md). Defaults preserve the legacy scales (`title = 14/12`, `body/status = 1`, `detail = 10/12`) and add restrained defaults for `display` (1.5), `section` (13/12), and `caption` (0.75). Each scale must be finite, positive, and at most 4.

Packages and components select a semantic variant name only; they cannot supply concrete scale ratios. A `clay.contributions.designTokens` entry targeting any `typography.*` token is rejected as a typography (variant) override, not a scale value. A changed hierarchy increments the typography revision and invalidates editor/UI layout once; an unchanged hierarchy does not churn layout.

## Document size ladder (Phase 26.4)

Document text uses the resolved profile size multiplied by a per-`TokenType` scale from `StyleRegistry`. This is not a new font role and not a package pixel: packages keep declaring `defaultFontRole` / `fontRole` only.

| TokenType | Default scale |
| --- | --- |
| Heading1 | 1.50 |
| Heading2 | 1.33 |
| Heading3 | 1.17 |
| Heading4 | 1.08 |
| Heading5 | 1.00 |
| Heading6 | 0.92 |
| CodeSpan | 0.90 |
| all other token types | 1.00 |

Theme `textStyles[].scale` overrides clamp to the UI hierarchy range `(0, 4]`. Diagnostic and SearchMatch layers stay at 1.0 so they do not split runs. Line/scroll geometry still uses the body `document_line_height()`; per-line mixed-heading scroll is deferred.

## Fallback and Invalidation

Each user profile is an ordered family stack ending in a generic fallback. Named families resolve on the client; Clay retains the role-appropriate generic fallback when names are unavailable. Packages cannot inspect installed fonts or react to resolution results.

A changed complete typography snapshot increments the server revision and invalidates client layout once. Document cache keys include typography revision, style revision, document role, text/viewport revisions, and width. Mixed inline roles use the larger active document profile as a conservative geometry baseline; the editor's shaped layout provides line/caret metrics. UI profile changes reset SDUI geometry and scale paint, hit, row, and accessibility bounds together.

## Performance Contract

- Mode/package validation and configuration run at load, activation, configuration, or background decoration-publication time.
- Style-run normalization is viewport-bounded and cached outside paint.
- Client paint, input, layout, pointer, scroll, and text-event paths use installed `TypographyRegistry` state only.
- Those hot paths perform no package JavaScript, IPC, filesystem/network access, font download, or server-side installed-font discovery.
- Typography and decoration envelopes remain bounded by `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES` and `DECORATION_PAYLOAD_BUDGET_BYTES`.

## Security Contract

Package declarations are inert semantic names. Validators reject concrete or executable font authority including `fontFamily`, `fontFamilies`, `fontSize`, `fontStack`, font paths, font bytes, URLs, downloads, raw CSS, raw renderer properties, renderer callbacks, native widget handles, client-side JavaScript, and raw `Deno.core.ops`.

Semantic role declaration grants no filesystem, network, shell, package-manager, extension loading, AI, WASM, workspace mutation, native UI, or client-runtime authority. Invalid roles fail closed before publication/activation; they never fall through as concrete family names.

## Implementation and Tests

Primary source paths:

- Protocol and validation: `src/protocol/mod.rs`, `src/protocol/decorations.rs`, `src/packages/record/mod.rs`, `src/server/ui.rs`
- Mode/range parsing: `src/packages/modes.rs`, `src/server/ops/modes.rs`, `src/server/ops/decorations.rs`, `src/server/syntax.rs`
- Client resolution/layout: `src/editor/typography.rs`, `src/editor/layout.rs`, `src/editor/surface/mod.rs`
- Native UI/components: `src/shell/package_ui.rs`, `src/shell/theme.rs`; rendered by the React theme adapter (`frontend/src/theme`)

Deterministic coverage lives in `tests/typography_protocol.rs`, `tests/markdown_mode.rs`, `tests/decoration_transport.rs`, `tests/editor_performance_invariants.rs`, `tests/package_loading.rs`, `tests/package_primitive_gate.rs`, and module tests beside the source paths above. Manual coverage is the Phase 18.16.5 matrix in `docs/development/launch-and-gui-smoke.md`.

Future modes must reuse `defaultFontRole`, decoration/style-map `fontRole`, and component `style.fontRole`. Do not add mode-specific Rust rendering branches or new concrete font settings.
