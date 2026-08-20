# Package Input, State, and Configuration Integration

## Source

- `runtime/js/ui.js`
- `runtime/js/configuration.js`
- `src/server/ops/ui.rs`
- `src/server/ops/configuration.rs`
- `src/server/ui.rs`
- `src/server/configuration.rs`
- `src/server/js_runtime/mod.rs`
- `src/shell/package_ui.rs`
- `src/shell/components.rs`
- `src/shell/theme.rs`
- `src/packages/record/mod.rs`
- `src/packages/conflict.rs`
- `src/masonry_sdui.rs`
- `docs/reference/clay-js-api/ui/server-register-input-contribution.md`
- `docs/reference/clay-js-api/ui/server-register-ui-state-scope.md`
- `docs/reference/clay-js-api/ui/server-set-layout-override.md`
- `docs/reference/clay-js-api/configuration/set-package-option.md`
- `docs/reference/packages/creating-packages.md`
- `tests/package_loading.rs`
- `tests/package_primitive_gate.rs`
- `tests/primitives_docs.rs`
- `tests/clay_js_api_inventory.rs`
- `tests/clay_js_doc_registry.rs`
- `tests/clay_js_facade_layout.rs`
- `tests/rust_visibility_api_mapping.rs`
- `tests/performance_budgets.rs`
- `tests/performance_protocol.rs`
- `tests/editor_performance_invariants.rs`
- `tests/manual_smoke_docs.rs`

## Overview

Phase 18.4 completes the documentation-backed integration layer for package input declarations, component-scoped actions, inert UI state scope declarations, layout override records, package-owned configuration options, and typed theme-token remaps. In primitive-registry terms this covers `PackageInputContribution`, component-scoped action/focus metadata, `PackageUiStateScope`, `PackageLayoutOverride`, `PackageOwnedConfiguration`, package option schemas, and typed theme-token remaps. The public authoring contract is expressed through documented Clay JS APIs and checked registry metadata, while Rust keeps the implementation as server-side validators plus inert runtime state that the native shell can read without executing package code.

The implemented public surfaces are:

- `ui.serverRegisterInputContribution`
- `ui.serverRegisterUiStateScope`
- `ui.serverSetLayoutOverride`
- `configuration.setPackageOption`

These APIs are runtime-backed for declaration/override records. They do **not** implement durable state-value persistence, low-level working-area/split/direct slot mutation, pane selector APIs, multi-panel ordering, overlay z-order control, cross-window layout, or package enable/disable authority.

## Responsibilities

- Validate package input metadata for pointer/focus/selection behavior and component-scoped action routing.
- Validate package UI state scope metadata as schema/lifecycle declarations only, not state values.
- Validate package layout override records for fixed slot, visibility, split-ratio, theme-token, input-default, action-default, and fallback properties.
- Validate package option records from `~/.config/clay/init.js`/configuration code with package-owned names, supported option keys, explicit source metadata, and deterministic precedence.
- Keep package option and layout override work at package-load, startup, configuration-change, or explicit setting-change time; never run package JavaScript or configuration evaluation in Masonry hot paths.
- Preserve behavior-manifest compatibility: key/text routing remains behavior-manifest/keybinding work, while package input declarations are inert metadata for UI component focus/action policy.
- Preserve documentation-as-code coverage across Clay JS API Markdown, `api-inventory.toml`, generated registry freshness tests, package authoring docs, primitive docs, and wiki links.

## How It Works

1. Package or configuration code imports documented facades from `clay:ui` and `clay:configuration`. The TypeScript-facing modules in `runtime/js/ui.js` and `runtime/js/configuration.js` serialize only declared records and call Clay-owned ops.
2. `src/server/ops/ui.rs` and `src/server/ops/configuration.rs` parse JSON at the server boundary. Raw `op_clay_*` names remain internal implementation details; public authors use the documented facade exports.
3. `src/server/ui.rs` validates package-owned UI records against the package manifest/provenance, registered component IDs, registered command/action targets, manifest-declared input modes, package-prefixed IDs, allowed scopes/properties, payload budgets, and prohibited authority fields.
4. `src/server/configuration.rs` records package option values as typed configuration records with explicit source/precedence information. Supported option names include `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`.
5. Package manifests are also validated by `src/packages/record/mod.rs`. Their Phase 18.4 metadata (`input`, `uiStateScopes`, `layoutOverrides`, `packageOptions`, and `themeTokens`) must align with declared API dependencies and permissions before package enable/load can activate the record.
6. Cross-package checks in `src/packages/conflict.rs` reject duplicate input IDs, UI state scope IDs, layout override target/property pairs, package option schemas, theme tokens, fixed-slot claims, and ambiguous action/input conflicts with deterministic provenance diagnostics.
7. Accepted input declarations become inert `PackageInputRouting` values in `src/shell/package_ui.rs`. `src/masonry_sdui.rs` can read those values while composing native panels/overlays and routing already-validated component actions, but it does not parse manifests, evaluate configuration, execute JavaScript, or mutate the child tree during layout.
8. Accepted UI state scope declarations remain inert schema/lifecycle metadata. Phase 18.4 does not accept state values, hidden globals, raw JSON blobs, or persisted workspace/document mutation authority.
9. Theme-token remaps stay typed: package tokens and remap records resolve through `src/shell/theme.rs` and Clay core token fallbacks before native paint/layout reads resolved values.

## Runtime-Backed vs Deferred

Runtime-backed in Phase 18.4:

- Package input contribution registration: inert pointer/focus/selection/action metadata.
- Component-scoped action metadata: action targets must already be registered commands.
- UI state scope registration: schema, owner, lifetime, persistence, implementation status, and targeted package/component metadata.
- Layout override records: validated default slot/visibility/split ratio/theme/input/action/fallback override metadata.
- Package option records: package-owned configuration records with supported option names and source precedence.
- Typed theme-token fallback/remap metadata.

Deferred or unavailable:

- Durable workspace/document/component state-value persistence.
- Direct `WorkingAreaLayout`, split-tree, or pane-slot mutation from packages.
- Pane selector APIs and cross-window layout APIs.
- Multi-panel ordering and overlay z-order APIs.
- Package enable/disable mutation through configuration or package JavaScript.
- Raw Masonry/native widget construction, raw CSS, raw Deno ops, renderer callbacks, and client-side JavaScript.

## Invariants and Constraints

- Registration and validation happen outside ordinary typing, paint, layout, pointer, scroll, keypress, text-event, and edit acknowledgement hot paths.
- Masonry hot paths read already-validated inert package UI/input/configuration state only; they do not run package JavaScript/config evaluation, parse manifests, perform schema validation, wait synchronously on IPC, serialize full documents, and do not mutate Masonry children during layout.
- Component action routing sends bounded command intents only after action IDs have been registered separately; input declarations do not grant command execution authority by themselves.
- Hidden configuration keys are rejected. Examples such as `preview.position`, `preview.defaultVisibility`, `layout.preview.defaultSlot`, ad hoc style keys, and raw token override keys are invalid unless represented through documented Clay JS APIs with registry metadata and validators.
- Package-owned IDs, state scopes, option schemas, and tokens must use the package API prefix. Packages cannot claim `clay.*` names.
- Package records grant no filesystem, network, shell, AI mutation, WASM, package-manager execution, package enable/disable, workspace mutation, raw op, native widget, direct Masonry, raw CSS, renderer callback, client-side JavaScript, or external authority. In shorthand, raw Masonry/native widget construction, raw CSS, raw Deno ops, renderer callbacks, and client-side JavaScript are denied.
- Observability remains crate-internal and privacy-preserving: it omits document text, filesystem paths, native handles, Masonry widget IDs, raw action payload authority, raw CSS, raw ops, callbacks, secrets, and executable package code.

## Extension Guidance

When adding a future package input/state/configuration primitive:

1. Add or update the authoritative Clay JS API Markdown under `docs/reference/clay-js-api/`.
2. Add an `api-inventory.toml` entry with explicit `status`, facade/op/Rust mapping, permissions, custom properties, lookup tags, and security notes.
3. Implement a typed server validator before exposing the API as runtime-backed.
4. Keep package-facing data inert at the client boundary; native code should read validated structs, not execute package code.
5. Add package manifest/conflict tests, facade/API inventory tests, generated registry tests, primitive docs tests, performance/hot-path tests, and wiki/index coverage.
6. Regenerate `docs/generated/clay-js-api-registry.json` only when the freshness test asks for it, using `cargo run --bin update-doc-registry --quiet`.

## Tests

Focused verification:

```text
cargo fmt --check
cargo test --test security package_loading:: --quiet
cargo test --test protocol package_loading_docs:: --quiet
cargo test --test security package_primitive_gate:: --quiet
cargo test --test protocol primitives_docs:: --quiet
cargo test --test protocol clay_js_api_inventory:: --quiet
cargo test --test protocol clay_js_doc_registry:: --quiet
cargo test --test protocol clay_js_facade_layout:: --quiet
cargo test --test security rust_visibility_api_mapping:: --quiet
cargo test --test protocol performance_budgets:: --quiet
cargo test --test protocol performance_protocol:: --quiet
cargo test --test editor editor_performance_invariants:: --quiet
cargo test --test protocol manual_smoke_docs:: --quiet
```

Representative coverage:

- `tests/package_loading.rs` validates Phase 18.4 manifest metadata, duplicate/conflict diagnostics, provenance, package option schemas, layout overrides, and rejected invalid metadata.
- `tests/package_primitive_gate.rs` keeps prohibited authority, permission, and behavior-manifest gates aligned with package input/action metadata.
- `tests/primitives_docs.rs` verifies package guide, primitive docs, security/hot-path language, deferred API status, and wiki/index links.
- `tests/clay_js_api_inventory.rs` and `tests/clay_js_doc_registry.rs` verify public API inventory status, generated registry entries, custom properties, lookup tags, and no-authority metadata.
- `tests/performance_*` and `tests/editor_performance_invariants.rs` preserve bounded payload and client-first editor behavior.
- `tests/manual_smoke_docs.rs` preserves the developer handoff commands for GUI smoke validation.

## Related

- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Configuration Runtime](configuration-runtime.md)
- [Package Loading](package-loading.md)
- [Command Registry](command-registry.md)
- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.4 Input, State, and Configuration Primitive Review](phase18.4-input-state-config-primitive-review.md)
- [Clay JS Documentation Registry](clay-js-doc-registry.md)
- [Package Authoring Guide](../../reference/packages/creating-packages.md)
- [Package Security Reference](../../reference/primitives/package-security.md)
