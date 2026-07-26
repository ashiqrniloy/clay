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

User configuration is documented by [`clay.theme.setTypography`](../clay-js-api/theme/set-typography.md). Its `monospace`, `proportional`, and `ui` profiles are complete user-owned values. No package permission allows overriding them.

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

The seven semantic variants (`display`, `title`, `section`, `body`, `status`, `detail`, `caption`) are scale ratios over the selected role's base size. The complete hierarchy is user-owned and travels atomically with `ActiveTypography` via [`clay.theme.setTypography`](../clay-js-api/theme/set-typography.md). Defaults preserve the legacy scales (`title = 14/12`, `body/status = 1`, `detail = 10/12`) and add restrained defaults for `display` (1.5), `section` (13/12), and `caption` (0.75). Each scale must be finite, positive, and at most 4.

Packages and components select a semantic variant name only; they cannot supply concrete scale ratios. A `clay.contributions.designTokens` entry targeting any `typography.*` token is rejected as a typography (variant) override, not a scale value. A changed hierarchy increments the typography revision and invalidates editor/UI layout once; an unchanged hierarchy does not churn layout.

## Fallback and Invalidation

Each user profile is an ordered family stack ending in a generic fallback. Named families resolve on the client; Clay retains the role-appropriate generic fallback when names are unavailable. Packages cannot inspect installed fonts or react to resolution results.

A changed complete typography snapshot increments the server revision and invalidates client layout once. Document cache keys include typography revision, style revision, document role, text/viewport revisions, and width. Mixed inline roles use the larger active document profile as a conservative geometry baseline; visible Parley layout provides shaped line/caret metrics. UI profile changes reset SDUI geometry and scale paint, hit, row, and accessibility bounds together.

## Performance Contract

- Mode/package validation and configuration run at load, activation, configuration, or background decoration-publication time.
- Style-run normalization is viewport-bounded and cached outside paint.
- Client paint, input, layout, pointer, scroll, and text-event paths use installed `TypographyRegistry` state only.
- Those hot paths perform no package JavaScript, IPC, filesystem/network access, font download, or server-side installed-font discovery.
- Typography and decoration envelopes remain bounded by `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES` and `DECORATION_PAYLOAD_BUDGET_BYTES`.

## Security Contract

Package declarations are inert semantic names. Validators reject concrete or executable font authority including `fontFamily`, `fontFamilies`, `fontSize`, `fontStack`, font paths, font bytes, URLs, downloads, raw CSS, raw Parley properties, renderer callbacks, native widget handles, client-side JavaScript, and raw `Deno.core.ops`.

Semantic role declaration grants no filesystem, network, shell, package-manager, extension loading, AI, WASM, workspace mutation, native UI, or client-runtime authority. Invalid roles fail closed before publication/activation; they never fall through as concrete family names.

## Implementation and Tests

Primary source paths:

- Protocol and validation: `src/protocol/mod.rs`, `src/protocol/decorations.rs`, `src/packages/record.rs`, `src/server/ui.rs`
- Mode/range parsing: `src/packages/modes.rs`, `src/server/ops/modes.rs`, `src/server/ops/decorations.rs`, `src/server/syntax.rs`
- Client resolution/layout: `src/editor/typography.rs`, `src/editor/layout.rs`, `src/editor/surface.rs`
- Native UI/components: `src/masonry_editor.rs`, `src/masonry_sdui.rs`, `src/shell/package_ui.rs`, `src/shell/theme.rs`

Deterministic coverage lives in `tests/typography_protocol.rs`, `tests/markdown_mode.rs`, `tests/decoration_transport.rs`, `tests/editor_performance_invariants.rs`, `tests/package_loading.rs`, `tests/package_primitive_gate.rs`, and module tests beside the source paths above. Manual coverage is the Phase 18.16.5 matrix in `docs/development/launch-and-gui-smoke.md`.

Future modes must reuse `defaultFontRole`, decoration/style-map `fontRole`, and component `style.fontRole`. Do not add mode-specific Rust rendering branches or new concrete font settings.
