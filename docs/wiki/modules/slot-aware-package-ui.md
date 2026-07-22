# Slot-Aware Package UI

## Source

- `runtime/js/ui.js`
- `src/server/ops/ui.rs`
- `src/server/ui.rs`
- `src/server/js_runtime.rs`
- `src/shell/components.rs`
- `src/shell/theme.rs`
- `src/shell/package_ui.rs`
- `src/masonry_sdui.rs`
- `src/packages/record.rs`
- `src/packages/conflict.rs`
- `docs/reference/clay-js-api/ui/server-register-panel-contribution.md`
- `docs/reference/clay-js-api/ui/server-register-component-contribution.md`
- `docs/reference/clay-js-api/ui/server-register-transient-overlay-contribution.md`
- `docs/reference/clay-js-api/ui/server-register-input-contribution.md`
- `docs/reference/clay-js-api/ui/server-register-ui-state-scope.md`
- `docs/reference/clay-js-api/ui/server-register-theme-token.md`
- `docs/reference/packages/creating-packages.md`
- `tests/clay_js_api_inventory.rs`
- `tests/clay_js_doc_registry.rs`
- `tests/clay_js_facade_layout.rs`
- `tests/package_loading.rs`
- `tests/package_primitive_gate.rs`
- `tests/primitives_docs.rs`
- `tests/rust_visibility_api_mapping.rs`

## Overview

Phase 18.3 implements Clay's first runtime-backed package UI contribution model. Packages can declare inert fixed panels, reusable component trees, transient overlays, and semantic theme tokens through documented `clay:ui` facade APIs. Phase 18.4 extends the same primitive family with `serverRegisterInputContribution` for bounded pointer, focus, selection, and component-scoped action routing metadata and `serverRegisterUiStateScope` for bounded UI state schema/lifecycle metadata. Clay validates those declarations on the server, preserves package provenance, stores accepted records in internal registries, and lets the native client compose accepted fixed panels, overlays, input routes, and inert lifecycle declarations through Clay-owned slot geometry.

The model is deliberately not a direct Masonry, CSS, HTML, or client-side JavaScript extension point. The package-facing surface is the Clay JS facade plus reference docs; the Rust modules are internal validators and runtime state for inert data.

## Responsibilities

- Provide public, runtime-backed Clay JS APIs under `clay:ui`:
  - `serverRegisterPanelContribution`
  - `serverRegisterComponentContribution`
  - `serverRegisterTransientOverlayContribution`
  - `serverRegisterInputContribution`
  - `serverRegisterUiStateScope`
  - `serverRegisterThemeToken`
- Validate package manifests, API prefixes, contribution IDs, fixed slots, overlay policies, input scopes, pointer/focus/selection policies, UI state scopes/lifecycles/schema metadata, manifest-declared input modes, component kinds, action targets, style variables, theme-token fallbacks, duplicate IDs, duplicate fixed-slot claims, prohibited authority fields, and payload budgets before storing contributions.
- Translate accepted server registry snapshots into crate-internal package UI runtime updates for fixed panels and transient overlays.
- Compose fixed panels with `PaneSlotLayout` while preserving the editor in the mandatory `main` slot.
- Render accepted package components and overlays through Clay-owned native SDUI/Masonry code without exposing Masonry widget IDs, widget constructors, native handles, raw CSS, raw ops, renderer callbacks, or client-side JavaScript hooks.
- Keep `PackageUiStateScope`, `PackageLayoutOverride`, direct working-area/split/slot APIs, user layout overrides, persisted panel state, and user theme-token remaps planned for later phases.

## How It Works

1. A package imports `clay:ui` inside Clay's constrained server-side JavaScript runtime. `runtime/js/ui.js` defines the TypeScript-facing facade shape, while `src/server/js_runtime.rs` supplies the embedded module source for the controlled runtime.
2. The facade encodes the package manifest and declarative contribution object as JSON and calls one Clay-owned op from `src/server/ops/ui.rs`. Raw `Deno.core.ops` names are implementation details; package code should use the documented facade exports.
3. Each op parses JSON, validates the package manifest with existing package-manifest rules, and delegates to `PackageUiRegistry` in `src/server/ui.rs`.
4. `PackageUiRegistry` validates the declaration against the package prefix/provenance and the already-registered package commands. It rejects unsupported slots/policies, unregistered command actions, duplicate IDs, duplicate fixed-slot claims, raw op/native/widget/CSS/client-JS authority fields, unsupported/deferred component kinds, invalid typed style variables, raw colors, type-incompatible theme token fallbacks, and payloads over SDUI/component budget expectations.
5. Accepted records are stored as `RegisteredPanelContribution`, `RegisteredComponentContribution`, `RegisteredTransientOverlayContribution`, `RegisteredPackageInputContribution`, `RegisteredPackageUiStateScope`, and `RegisteredPackageThemeTokenDeclaration` values with `UiContributionProvenance`.
6. `PackageUiRegistrySnapshot::runtime_update` maps registered panels, overlays, and input contributions into `PackageUiRuntimeUpdate`. `src/shell/package_ui.rs` applies that update only when the base version matches, enforces `MAX_FIXED_PANELS`, `MAX_TRANSIENT_OVERLAYS`, `MAX_INPUT_ROUTES`, duplicate contribution-ID rejection, and duplicate exclusive fixed-slot rejection, then increments the local runtime version.
7. `PackageUiRuntimeState::slot_layout` folds visible fixed panels into `PaneSlotLayout`. Side panels use bounded side sizes, top/bottom panels use bounded vertical sizes, and transient overlays are kept separate so they do not consume fixed slot geometry.
8. `src/masonry_sdui.rs` reads the installed inert package UI state during layout/paint. It paints fixed panels and transient overlays from validated state, while `src/masonry_editor.rs` uses the same slot-computed `main` rect to clip/offset `EditorSurface` painting and translate pointer hit-testing before caret/selection updates. `EditorSurface` remains responsible for text, caret, selection, viewport, edit queueing, and ordinary input.
9. Package UI action regions emit bounded `ClientMessage::SduiAction` command intents for registered command IDs. The pointer handler does not execute package JavaScript and does not wait synchronously for IPC acknowledgement. Phase 18.4 input routes are installed as inert `PackageInputRouting` values so native input code can read focus/action policy without package validation, configuration evaluation, or package JavaScript in pointer/key/text hot paths.

## Code Examples

Public usage details live in the Clay JS API reference pages. A representative package declaration flow is:

```ts
import {
  serverRegisterPanelContribution,
  serverRegisterThemeToken,
} from "clay:ui";

serverRegisterThemeToken(manifest, {
  token: "markdown.preview.background",
  type: "color-role",
  fallback: "surface.panel",
  description: "Markdown preview panel background",
});

serverRegisterPanelContribution(manifest, {
  id: "markdown.preview",
  kind: "fixed",
  slot: "right",
  defaultVisibility: "hidden",
  component: {
    id: "markdown.preview.root",
    kind: "panel",
    title: "Preview",
    style: { background: "markdown.preview.background" },
    children: [],
  },
  actionTargets: ["markdown.togglePreview"],
});
```

## Primitive Coverage

- `PanelContribution`
  - Owner/source: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/package_ui.rs`.
  - Public docs: `docs/reference/clay-js-api/ui/server-register-panel-contribution.md`.
  - Validation: package-prefixed ID, `kind = fixed`, slot in `left`/`right`/`top`/`bottom`, allowed default visibility, bounded component tree, registered action targets, duplicate ID/slot rejection, prohibited authority rejection.
  - Runtime: fixed panels compose into `PaneSlotLayout`; the editor remains clipped/offset in `main` for paint and pointer hit-testing.

- `ComponentContribution`
  - Owner/source: `runtime/js/ui.js`, `src/server/ui.rs`, `src/shell/components.rs`, `src/shell/package_ui.rs`.
  - Public docs: `docs/reference/clay-js-api/ui/server-register-component-contribution.md`.
  - Supported Phase 18.3 component kinds: `editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, and `statusItem`.
  - Deferred component kinds: `table`, `dropdown`, `collapse`, and `modal` fail with planned/deferred diagnostics instead of partially working semantics.
  - Style variables must reference typed Clay tokens or allowed enum values such as `variant` and `fontRole`; raw CSS, raw colors, concrete font families/sizes, and unsupported style keys are rejected. `fontRole` defaults to user-owned `ui`; only text-bearing panel, label, button, list, and statusItem components may select semantic `monospace` or `proportional`.

- `TransientOverlayContribution`
  - Owner/source: `runtime/js/ui.js`, `src/server/ui.rs`, `src/shell/package_ui.rs`, `src/masonry_sdui.rs`.
  - Public docs: `docs/reference/clay-js-api/ui/server-register-transient-overlay-contribution.md`.
  - Validation: package-prefixed overlay ID, supported anchor (`working-area`, `active-pane`, `main`, or `pointer`), focus policy (`none`, `restore`, or `trap`), dismissal policy (`manual`, `escape`, `outside`, or `escape-or-outside`), bounded component tree, registered actions, and prohibited authority rejection.
  - Runtime: overlays are versioned package UI state and compute overlay rectangles without adding fixed slots or mutating the Masonry child tree.

- `PackageInputContribution`
  - Owner/source: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/package_ui.rs`.
  - Public docs: `docs/reference/clay-js-api/ui/server-register-input-contribution.md`.
  - Validation: package-prefixed input ID and component ID, scope (`component`, `panel`, or `overlay`), pointer click/drag policy, focus policy, selection policy, manifest-declared mode context, registered action targets, payload budget, and prohibited authority rejection.
  - Runtime: accepted declarations become `PackageInputRouting` values read as inert state; keyboard routing remains behavior-manifest/keybinding responsibility.

- `PackageUiStateScope`
  - Owner/source: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs`.
  - Public docs: `docs/reference/clay-js-api/ui/server-register-ui-state-scope.md`.
  - Validation: package-prefixed state-scope ID, no hidden path segments, supported scope (`package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, or `transient-overlay`), owner, lifetime, persistence, implementation status, targeted package ID, schema kind, payload budget, and prohibited authority rejection.
  - Runtime: accepted declarations are inert schema/lifecycle metadata only; registration does not accept state values, hidden globals, raw JSON blobs, or persisted workspace/document mutation authority.

- `PackageThemeTokenDeclaration`
  - Owner/source: `runtime/js/ui.js`, `src/server/ui.rs`, `src/shell/theme.rs`, `src/masonry_sdui.rs`.
  - Public docs: `docs/reference/clay-js-api/ui/server-register-theme-token.md`.
  - Validation: package-prefixed token name, token type (`color-role`, `spacing`, `radius`, `typography`, or `opacity`), non-empty description, and same-type Clay core fallback.
  - Runtime: `ThemeTokenResolver` resolves package tokens to core fallback values before native paint/layout reads them.

## Invariants and Constraints

- Registration and validation are package load/config/update or explicit UI update work, not Masonry paint/layout, pointer, scroll, keypress, text-event, or ordinary editor hot-path work.
- Masonry hot paths read already-validated inert package UI state only; they do not parse manifests, run package JavaScript, perform schema validation, wait on IPC, serialize full documents, or mutate the child tree. Resolved profile stacks and UI metrics are client-local cached data; font discovery never occurs in paint, layout, pointer, or accessibility traversal.
- Package UI declarations grant no filesystem, network, shell, AI mutation, WASM, package-manager execution, package enable/disable, workspace mutation, raw Deno op, native widget, client-side JavaScript, renderer callback, raw CSS, or raw style authority.
- Package-owned IDs and tokens must use the package `apiPrefix`; packages cannot claim `clay.*` names.
- Action authority comes only from separately registered command IDs. UI declarations do not create command authority by themselves.
- Observability helpers are crate-internal and omit document text, filesystem paths, native handles, Masonry widget IDs, raw action payload authority, raw CSS, raw ops, callbacks, and executable package code.
- Historical Phase 18.3 boundary: User-visible layout overrides, default-slot overrides, persisted panel visibility, durable workspace/document state mutation, and user theme-token remapping remain planned APIs until they get facade/op/docs/registry/tests.
- Phase 18.4 update: user-visible layout overrides, package default-slot/default-visibility overrides, input/action defaults, package option records, and user theme-token remap records are now runtime-backed through documented `serverSetLayoutOverride` and `setPackageOption` validators. Durable workspace/document state-value mutation, pane selector APIs, multi-panel ordering, and overlay z-order remain planned until they get facade/op/docs/registry/tests.

## Tests

- `src/server/js_runtime.rs::runtime_imports_clay_ui_facade_and_registers_contributions`: imports `clay:ui`, registers panel/component/overlay/token declarations, and verifies registry snapshots preserve accepted records.
- `src/server/js_runtime.rs::runtime_clay_ui_rejects_invalid_prefix_unregistered_action_and_raw_css`: verifies invalid package prefixes, stale/unregistered actions, and raw CSS-style input fail.
- `src/server/ui.rs` unit tests: validate accepted contribution records, input contribution records, UI state-scope lifecycle records, duplicate IDs/slots, prohibited authority fields, hidden state key rejection, key-routing rejection, action target validation, payload budgets, component tree bounds, and typed theme-token fallback rules.
- `src/shell/components.rs` unit tests: validate supported/deferred component kinds and typed style variables.
- `src/shell/theme.rs` unit tests: validate core token resolution, package-token fallback resolution, and type mismatch rejection.
- `src/shell/package_ui.rs` unit tests: validate fixed panel slot composition, duplicate exclusive slot rejection, and transient overlay geometry.
- `src/masonry_sdui.rs` unit tests: validate package fixed-panel geometry, transient overlay geometry, action routing, observation privacy, resolved theme-token rendering, semantic package font-role selection, and shared row/hit/accessibility geometry.
- `src/server/ui.rs::package_component_font_role_is_semantic_and_text_only`: accepts allowed enum roles and rejects unsupported roles, concrete family/size fields, and structural-component use.
- `src/masonry_editor.rs::fixed_package_panel_shrinks_editor_hit_region`: validates editor hit-testing uses the fixed-panel-reduced `main` rect.
- `tests/package_loading.rs`: validates package manifest UI/input/state/layout/configuration metadata, contribution counts, deterministic conflicts, prefix/provenance rules, and invalid contribution rejection.
- `tests/package_primitive_gate.rs`: keeps package permission/prohibited-authority gates aligned with UI contribution rules.
- `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, and `tests/rust_visibility_api_mapping.rs`: verify public `clay:ui` docs, generated registry entries, facade exports, op mappings, and Rust visibility boundaries.
- `tests/primitives_docs.rs`: verifies primitive docs, package guide, security/hot-path rules, and wiki coverage.

Focused verification commands:

```text
cargo test --lib ui --quiet
cargo test --lib shell --quiet
cargo test --lib masonry_sdui --quiet
cargo test --test security package_loading:: --quiet
cargo test --test security package_primitive_gate:: --quiet
cargo test --test protocol clay_js_api_inventory:: --quiet
cargo test --test protocol clay_js_doc_registry:: --quiet
cargo test --test protocol primitives_docs:: --quiet
```

## Related

- [Masonry Shell Runtime](masonry-shell.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Package Loading](package-loading.md)
- [Configuration Runtime](configuration-runtime.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Clay JS Documentation Registry](clay-js-doc-registry.md)
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md) — Phase 19 `PackageUiRuntimeState::install_runtime_snapshot` atomically replaces package UI version during live reload.
- [Phase 18.3 Slot-Aware Package UI Primitive Review](phase18.3-slot-ui-primitive-review.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Package Authoring Guide](../../reference/packages/creating-packages.md) — includes Phase 20 multi-document / dirty-save / recovery chrome non-goals for package UI
- [Phase 20 Daily Editing Product Hardening Primitive Review](phase20-daily-editing-product-hardening-primitive-review.md)
- [File Open, Save, and Reload Workflow](../../development/file-open-save-reload-workflow.md)
