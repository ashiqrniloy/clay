# Package Primitive Security and Provenance Requirements

This Phase 16 document is the canonical security surface for package-provided primitives. It applies to package declarations for modes, behavior manifests, commands, rendering data, parse handlers, SDUI contributions, configuration, and permissions.

Security rules are traceable to:

- `roadmap.md` Phase 16 package primitive security directive.
- `.agents/skills/project-patterns/references/package-distribution.md` and `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`.
- `.agents/skills/project-patterns/references/extensions-and-ai.md` and `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`.
- `.agents/skills/project-patterns/references/clay-js-api-boundary.md` and `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.

## Baseline Rules

- Package installation and execution are separate: a package manager may download packages, but Clay must validate Clay package metadata before enable/load.
- JavaScript runs server-side through `deno_core`, never in the Rust client; no package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.
- Clients receive only validated behavior manifests, SDUI trees, protocol updates, or renderer/decorator declarations.
- Primitive validation happens at package load time where possible, not on every edit. Validation failures produce actionable load-time errors and must not panic at runtime.
- Validation cost must not appear on the typing hot path; ordinary typing remains client-local or asynchronously server-confirmed according to the behavior manifest routing policy.

## Package Identity, Prefix, and API Provenance

Each package that contributes primitives must declare Clay metadata in its package manifest:

```json
{
  "name": "@clay/markdown",
  "version": "0.1.0",
  "clay": {
    "apiPrefix": "markdown",
    "permissions": [],
    "modes": ["markdown"],
    "entry": "./dist/index.js",
    "loadEntry": "./dist/load.js"
  }
}
```

Required provenance checks at load time:

1. `clay.apiPrefix` must match `^[a-z][a-z0-9-]{1,31}$`.
2. The prefix must be unique among enabled packages.
3. Package-owned stable IDs, command IDs, mode IDs, configuration keys, style tokens, and contribution IDs must be scoped to the package prefix.
4. Package-owned Clay JS API IDs must start with the package prefix; only first-party Clay APIs may use `clay.*` stable IDs.
5. All accepted primitive records must retain `package_name`, `package_version`, `api_prefix`, and source contribution metadata for diagnostics, conflict handling, generated docs, and AI-agent provenance.

## Permission Requirements by Primitive Category

| Primitive category | Default permission | Validation requirement |
| --- | --- | --- |
| `DocumentClassification` | `mode-registration` | Static extension, filename, MIME, or bounded metadata patterns only; no filesystem scans by default. |
| `MajorModeActivation` / `MinorModeActivation` | `mode-activation` | Mode must be declared by the package and selected by server-owned activation logic. |
| `KeyRoutingOverride` | none for inert keybinding declarations | Target command permissions apply at execution; key routes must declare routing policy. |
| `TextTransform` | none | Manifest rules must use known Rust client transform engines; no JavaScript callbacks. |
| `IncrementalParseUpdate` | `parse-document` | Parser runs server-side and may read only open document content/metadata provided by Clay. |
| `DecorationRange` | `render-decorations` | Spans are inert byte ranges with known kind/style/priority fields and package provenance. |
| `FoldingRange` | `render-folding` | Ranges must be valid for the target document version and bounded payload. |
| `CompletionTriggerAndResult` | `completion-provider` | Trigger metadata is inert; provider execution is server-side, cancellable, and permission checked. |
| `CommandDeclaration` | `command-registration` | Registration does not grant command handler authority; handler permissions are declared separately. |
| `SduiPanelStatusContribution` | none for inert UI | Actions embedded in SDUI must target declared commands with their own permissions. |
| `WorkingAreaLayout` / `PaneSplitTree` / `PaneSlotLayout` | none for inert layout declarations; `package-configuration` for behavior-changing defaults | Clay owns shell/pane/slot state; package/user defaults must be server-validated, bounded, package-provenance-aware layout state and must not expose Masonry widgets or native handles. |
| `PanelContribution` / `ComponentContribution` / `TransientOverlayContribution` | none for inert UI declarations | Phase 18.3 slot-aware package UI is inert component state; action targets inherit command permissions and Clay rejects invalid prefixes, unsupported slots/anchors/focus/dismissal policies, unknown/deferred component kinds, unsupported typed style variables, raw CSS, client-side JavaScript, renderer callbacks, native widget handles, direct Masonry mutation, duplicate IDs, duplicate fixed slot claims, and oversize component payloads. |
| `PackageThemeTokenDeclaration` | none for declarations; `package-configuration` for user overrides | Phase 18.3 theme/style tokens are typed semantic declarations only; package tokens require package prefixes and same-type Clay core fallbacks, and unknown tokens, type-incompatible fallbacks, raw CSS/style strings, raw colors without a token contract, Vello/Parley callbacks, and hidden override keys are rejected. |
| `PackageInputContribution` | none for inert input metadata; target command permissions apply to actions | Phase 18.4 input descriptors must be package-prefixed, scoped to component/panel/overlay targets, use supported pointer/focus/selection policies, reference manifest-declared modes and declared command actions, reject key-routing fields, and stay within update payload budgets. |
| `PackageUiStateScope` | none for inert scope metadata; future persisted/workspace/document mutation requires explicit permissions | State scopes must be declared as package-global, user-config, workspace, document, pane, component, or transient-overlay, use supported owner/lifetime/persistence/schema metadata, avoid hidden path segments, reject initial/default/raw state values, and cannot create hidden globals or grant filesystem/network/shell/AI/WASM authority. |
| `PackageLayoutOverride` | `package-configuration` when behavior-changing | Layout/panel/token/input/action defaults must flow through documented `~/.config/clay/init.js` Clay JS APIs or validated package-default/global-package manifest descriptors and cannot bypass precedence, slot, action, style-token, input, payload, or hidden-key validation. |
| `PackageOwnedConfiguration` | `package-configuration` when behavior-changing | Configuration cannot bypass `~/.config/clay/init.js` and documented Clay JS APIs; Phase 18.4 package option schemas are limited to package-prefixed supported options (`layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`). |
| `PackagePermissionDeclaration` | none for validation itself | Unknown, undeclared, or prohibited scopes are rejected before package enable/load succeeds. |

A primitive with `permissions = []` is still inert only; it does not imply filesystem, network, shell, AI, raw op, WASM, native widget, or client JavaScript authority.

Phase 18.4 manifest keys `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions` are validated at package load/enable time with package provenance. Runtime-backed public APIs `clay.ui.serverRegisterInputContribution`, `clay.ui.serverRegisterUiStateScope`, `clay.ui.serverSetLayoutOverride`, and `clay.configuration.setPackageOption` apply the same contract at configuration/package update time. Validators require registered actions for input/action defaults, reject unregistered actions, require `package-configuration` for behavior-changing defaults, hidden-key rejection, state-value rejection, and deterministic rejection of duplicate input, duplicate UI state scope, duplicate layout override, and duplicate package option metadata. These APIs do not grant package enable/disable authority.

## Server-Side Validation Checklist

Clay must reject a package primitive contribution before it becomes active when any check fails:

1. Package manifest schema is valid and contains required Clay metadata.
2. `apiPrefix` is valid and unique.
3. Contribution IDs are package-prefixed and do not claim reserved `clay.*` IDs unless first-party.
4. Primitive schema matches the registered primitive category.
5. Contribution payload size is within the category budget (`BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`, or the category-specific budget).
6. Required permissions are declared in package metadata and are known to Clay.
7. The primitive does not call raw `Deno.core.ops` or expose raw op names as the user-facing API.
8. The primitive does not contain client-side JavaScript references, draw callbacks, widget callbacks, CSS/HTML/script injection, raw style strings, direct Masonry widget constructors, native widget IDs, or native handles.
9. Runtime handlers are registered through Clay JS facade APIs, not direct Rust public functions.
10. Version, document, viewport, behavior, mode, and package provenance fields are present where the category requires them.

## Conflict Handling

Conflict handling must be deterministic and provenance-preserving. Conflicts produce load-time errors or documented resolution diagnostics; they must not silently override behavior.

| Conflict | Required behavior |
| --- | --- |
| Duplicate package prefix | Reject enabling the second package with an actionable error. |
| Duplicate mode name | Reject unless first-party metadata or a future decision log defines an override/alias policy. |
| Duplicate command ID | Reject if the command ID has the same package prefix; reject cross-package duplicate IDs unless the ID is explicitly namespaced. |
| Same key binding | Resolve only by declared priority and routing policy when both packages opt in; otherwise reject or disable the lower-priority binding with a visible diagnostic. |
| Same text transform trigger | Preserve deterministic package/load order only when priorities are explicit; ambiguous transforms are rejected. |
| Decoration range overlap | Allow overlap only because decorations carry priority, package prefix, and style token; equal priority falls back to deterministic package order. |
| SDUI region collision | Reject or require explicit region/slot priority; clients never merge arbitrary package widget code. |
| Duplicate shell slot claim | Reject or require explicit slot priority/precedence metadata; packages never win by load order alone. |
| Duplicate component or overlay ID | Reject unless the duplicate is the same package version replacing its own contribution through a documented update path. |
| Unknown style/theme token | Reject with package/token/source diagnostics; raw CSS/style strings are never treated as fallback tokens. |
| Unsupported UI state scope | Reject hidden globals, ad hoc keys, or undeclared scopes before package UI state affects the shell. |
| Configuration key collision | Reject unless keys are package-prefixed and scoped to the owning package. |

## Shell/Layout Precedence and Diagnostics

Shell/layout validators must follow the Phase 18.1 precedence order from `docs/reference/primitives/shell-layout-strategy.md`:

1. Clay shell safety invariants and hard prohibitions
2. User configuration through documented Clay JS APIs
3. Active major mode layout defaults
4. Compatible minor mode contributions
5. Global package contributions
6. Package fallback/defaults

Precedence never bypasses validation. Clay shell safety rejects invalid declarations before composition; user configuration may override package/default layout requests only through documented `~/.config/clay/init.js` Clay JS APIs; active major mode defaults establish the document-centered baseline; compatible minor modes and global packages may add non-conflicting UI; package fallback/defaults are lowest-precedence one-line-load defaults.

Shell/layout diagnostics must be structured, deterministic, and provenance-preserving. For slot, action, component, state, and style failures, diagnostics should include package name, package version, API prefix, primitive category, contribution ID, source file or manifest path when available, slot or pane selector, component ID or overlay ID, action/command ID, state scope/key, style/theme token name, payload size, failed precedence rule, and failed validation rule.

Required rejection categories for shell/layout declarations include duplicate slots, duplicate fixed slot claims, duplicate panel IDs, duplicate component IDs, duplicate overlay IDs, duplicate theme tokens, duplicate input contribution IDs, duplicate UI state scope IDs, duplicate layout override target/property pairs, duplicate package option schemas, duplicate commands/actions, invalid package prefixes, undeclared permissions, invalid `clay.ui.*` or `clay.configuration.*` API dependencies, unsupported slots, unsupported visibility values, unsupported overlay anchors, unsupported focus policies, unsupported dismissal policies, unsupported pointer/selection policies, unknown/deferred component kinds, unsupported typed style variables, unregistered action targets, unknown style tokens, unknown input defaults, type-incompatible token fallbacks, raw CSS, raw style strings, raw colors without typed token contracts, raw ops, native widget handles, direct Masonry widget constructors, client-side JavaScript, Vello/Parley/native renderer callbacks, oversize layout/component/state payloads, oversize layout/component/input/state/configuration payloads, hidden configuration keys, package/user override bypass attempts, state-value registration, and unsupported state scopes.

No package wins a shell/layout conflict by load order alone. A later implementation may define explicit slot priority, stacking, z-order, or pane-selector rules, but those rules must be documented, schema-validated, bounded, and tested before use.

## Prohibited Authorities by Default

No package primitive may claim these authorities by default:

- filesystem outside document content already open in Clay
- network
- shell
- AI mutation
- remote listeners
- WASM execution
- raw `Deno.core.ops`
- direct Masonry/widget mutation
- direct Masonry widget constructors
- arbitrary GPU draw calls
- native widget IDs or native widget handles
- raw CSS, raw style strings, or HTML/script injection
- client-side JavaScript
- extension/package installation or enable/disable mutation
- workspace mutation outside declared workspace APIs

Any future exception requires a new approved decision log before a plan can implement it, plus explicit permissions, documentation-as-code coverage, tests, and load-time validation.

## Hot-Path and Failure Policy

- Package load validation is allowed to inspect manifests, declarations, permissions, and bounded static payloads.
- Edit-time validation is limited to cheap version/range/payload checks on already-declared primitive categories.
- JavaScript parsing, completion, diagnostics, and commands run server-side and asynchronously according to their routing policy.
- Validation errors must produce structured package-load diagnostics that include package name, version, prefix, primitive category, contribution ID, source slot/component/action/token/input/state-scope/option/override target when applicable, payload size when applicable, and failed rule. Phase 18.3 package UI diagnostics must identify the panel/component/overlay/token ID, requested slot or overlay policy, action/command target, style variable or theme token, and the failed prefix/permission/payload/non-authority rule. Phase 18.4 input/state/configuration diagnostics must identify the input ID, state scope/key, layout override target/property/source, package option name, payload estimate, missing permission/API dependency, and package provenance before enable/load can activate the contribution.
- Runtime failures after load must degrade the affected primitive only: stale decorations may remain, commands may fail with an error, and SDUI contributions may be withdrawn, but the editor must not panic or block typing.
