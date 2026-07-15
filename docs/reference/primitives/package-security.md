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

## Unified Package Trust and Authorization Policy

Clay uses one package authority model for Clay-shipped and user-installed packages. Package source (`@clay/*`, npm, GitHub, git URL, tarball, or local path) affects default trust prompts and provenance display, but not the capabilities a user can grant. Clay metadata in `package.json` declares the package contract and requested capabilities; user/admin authorization is the grant.

Package identity and provenance should record enough source information to explain what the user approved:

```toml
[package_authority."@vendor/example"]
source = "npm:@vendor/example@1.2.3"
resolved_version = "1.2.3"
package_root = "/clay/packages/node_modules/@vendor/example"
api_prefix = "example"
runtime_profile = "native-trust"
capabilities = ["mode-registration", "package-control", "network"]
approved_by = "user"
```

Required install/enable/load behavior:

1. Install may accept npm, GitHub, git URL, tarball, and local path specs through the shared npm-compatible package-manager boundary.
2. Install remains separate from enable/load: package-manager metadata and files do not automatically activate package behavior.
3. Enable/load validates Clay metadata, `apiPrefix`, entry/loadEntry confinement, package graph declarations, requested capabilities, and conflicts before contributions become active.
4. Source provenance is shown in diagnostics and approval prompts, but source does not create a permanent first-party/third-party capability ceiling.
5. Capability grants are explicit, visible, revocable, and tied to package identity/source enough for users to understand what they approved.
6. Package graph authority (`dependsOn`, `extends`, `disables`, `replaces`) requires matching package-control/import capabilities and deterministic conflict handling.

Authorization work happens at install, enable, load, reload, startup, explicit user command, or background verification time. It must not run from keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

Current implementation: `PackageRecord`, `PackageService`, and conflict checks already carry package name, version, `apiPrefix`, contribution provenance, and deterministic conflict diagnostics. `PackageAuthorizationRecord` stores package identity/source, approved capability list, runtime profile, and approver. `PackageService::authorize_package` records user/admin grants, `enable` fails closed on requested capabilities without matching grants, and `PackageInspection` shows requested capabilities, approved capabilities, runtime profile, and source provenance. `validate_manifest_value` parses package graph declarations (`dependsOn`, `extends`, `disables`, `replaces`) into `PackageGraphRelations`; `src/packages/graph.rs` builds the enable-time graph plan; `PackageService::enable` loads dependency/extension targets, reports missing targets/cycles deterministically, and requires an explicit `package-control` authorization grant before `disables` or `replaces` can withdraw another enabled package. Remaining gaps are durable on-disk authorization persistence, package-scoped revocation indexes, package-import boundary enforcement, and conflict override/extend/replace resolution.

## Unified Package Capability Model

Packages request capabilities in Clay metadata; users/admins grant them. The manifest parser accepts `clay.capabilities` while preserving the older `clay.permissions` compatibility path; both feed the same `PackagePermission` vocabulary. Source may influence default prompts or pre-approval, but not the maximum authority available after approval.

```json
{
  "clay": {
    "capabilities": ["mode-registration", "package-control", "network"],
    "dependsOn": ["@clay/markdown"],
    "extends": ["@clay/markdown"],
    "disables": ["@clay/markdown"],
    "replaces": []
  }
}
```

Initial target capability vocabulary:

| Capability | Grants | Enforcement point |
| --- | --- | --- |
| `mode-registration` | Declare mode/classification metadata | enable/load manifest validation and mode registration |
| `mode-activation` | Participate in server-owned mode activation | load/activation selection |
| `command-registration` | Register command metadata and handlers through Clay APIs | enable/load and command registration |
| `package-configuration` | Declare or apply package-scoped configuration/layout defaults | enable/load, configuration API calls, layout validation |
| `parse-document` | Run server-side parse work on Clay-provided document content/windows | parse handler registration and request boundary |
| `render-decorations` | Publish bounded decoration ranges | decoration publication validation |
| `render-folding` | Publish bounded folding ranges | folding/decorations publication validation |
| `completion-provider` | Provide server-side completion results | provider registration and completion request boundary |
| `language-server` | Approve one fixed external language-server contribution for known directory roots | configuration-only exact grant, package enable, and later session-operation boundaries |
| `package-control` | Disable, replace, extend, or configure other packages through package graph APIs | package graph evaluation, conflict resolution, enable/disable/reload |
| `package-import` | Import/use another enabled package API/load surface | module resolution and package dependency validation |
| `filesystem` | Access user-approved filesystem scopes through Clay APIs | filesystem API boundary and scope validator |
| `network` | Make user-approved network requests through Clay APIs | network API boundary and policy validator |
| `shell` | Run user-approved commands | shell API boundary, prompt, and audit log |
| `wasm` | Load/run WASM modules | runtime profile, fuel/time/memory limits |
| `ai-tools` | Invoke AI/tool orchestration or mutation APIs | AI API boundary and document/workspace locks |
| `workspace-mutation` | Mutate workspace files/projects through Clay APIs | workspace API boundary and transaction validation |
| `native-ui` | Use approved native UI/widget extension APIs | native UI API boundary |
| `client-runtime` | Run approved client-side package code when implemented | client runtime boundary and UI safety validation |
| `raw-ops` | Use explicitly exposed low-level ops for development/debugging | dev-mode/admin-only raw op boundary |

Powerful capabilities are allowed by user choice, not categorically banned for non-`@clay/*` packages. They must have documented APIs, diagnostics, revocation behavior, and tests before implementation.

Permission checks are install/enable/load/reload/registration/request/publication work only. They must not run from keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

## Permission Requirements by Primitive Category

| Primitive category | Default permission | Validation requirement |
| --- | --- | --- |
| `DocumentClassification` | `mode-registration` | Static extension, filename, MIME, or bounded metadata patterns only; no filesystem scans by default. |
| `MajorModeActivation` / `MinorModeActivation` | `mode-activation` | Mode must be declared by the package and selected by server-owned activation logic. |
| `KeyRoutingOverride` | none for inert keybinding declarations | Target command permissions apply at execution; key routes must declare routing policy. |
| `TextTransform` | none | Manifest rules must use known Rust client transform engines; no JavaScript callbacks. |
| `IncrementalParseUpdate` | `parse-document` | Parser runs server-side and may read only open document content/metadata provided by Clay. |
| `SyntaxGrammarContribution` | `parse-document` + `render-decorations` | Phase 18.16 keeps the Phase 18.10 first-party-only package metadata contract and adds a tiered engine: Tier 1 compiled first-party native descriptors, Tier 2 package-root-confined `tree-sitter-wasm`/`.scm` assets, and Tier 3 server-side package-JS handlers. All captures map through one `TokenType` + `Modifiers` vocabulary path. Parse/highlight runs as `Background` no-hot-path work bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`; open is non-blocking and failures become sanitized `RuntimeDiagnostic` values. Clay rejects non-`@clay/*` grammar packages, absolute/external/traversing paths, runtime downloads, native libraries, package-manager/download/shell fields, raw CSS/colors, client JavaScript, raw ops, and duplicate language/pattern registry conflicts. |
| `DecorationRange` | `render-decorations` | Spans are inert byte ranges with known kind/style/priority fields and package provenance. |
| `SemanticTypographyRole` | none beyond the enclosing mode/decoration/component primitive | Packages may select only validated `monospace`, `proportional`, or component-only `ui` roles. Users own concrete fallback stacks and sizes; concrete font fields and executable/render authority are rejected. |
| `FoldingRange` | `render-folding` | Ranges must be valid for the target document version and bounded payload. |
| `CompletionTriggerAndResult` | `completion-provider` | Phase 18.11 completion provider metadata is load/registration-time inert package data under `clay.contributions.completionProviders`: package-prefixed provider ID, priority, inert trigger characters, inert word-boundary characters, and bounded `timeoutMs`/`maxItems`. Trigger metadata is inert manifest data; typing a trigger edits locally first and then enqueues a typed `CompletionRequest`. Provider execution is server-side, cancellable `UiReactivePriority` work bounded by `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `COMPLETION_RESULT_MAX_ITEMS`, and per-field caps; display/acceptance reuses `TransientMenuSession` and commits a validated text replacement, never a command. Phase 18.11 is metadata-only: Clay rejects `handler`/`callback`/`complete`/`function`/`module` executable values, raw ops, native handles, client JavaScript, snippets/commands, URLs, shell/network/AI/WASM/native/package-manager fields, duplicate IDs, reserved `clay.*` IDs, and oversize metadata. Providers may read only Clay-provided open-document content/windows; no filesystem/network/shell/AI/raw-op/native-UI/client-runtime authority is granted. |
| `LanguageIntelligenceRequestAndResult` | `parse-document` | Phase 18.20 feature-tagged providers register through `clay.language.serverRegisterLanguageIntelligenceProvider` and return inert hover/definition/code-action/signature-help results. Semantic/diagnostic/completion outputs still require `render-decorations` / `completion-provider`. Canonical positions are UTF-8 byte offsets; LSP conversion stays package-side. See [Language Intelligence and LSP 3.17 Bridge Contract](language-intelligence.md). |
| `LanguageServerContribution` | `language-server` | `clay.contributions.languageServers` contains only a package-prefixed ID, fixed executable, bounded literal argv, and explicit inherited-environment names. `authorizeLanguageServer` binds one validated contribution to current directory-root IDs before package execution; `loadPackage` seals authority mutation and bundled trust never auto-grants it. |
| `CommandDeclaration` | `command-registration` | Registration does not grant command handler authority; handler permissions are declared separately. |
| `CommandExecution` | command-specific permissions validated at execution | Server validates command ID, routing policy, package provenance, declared permissions, target context, argument budget, and session/action freshness before side effects; activation through SDUI, package UI, keybinding, or transient-menu intent must normalize to this one server-owned boundary. |
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

## Semantic Typography Authority Boundary

[Semantic typography roles](typography.md) grant selection, not concrete font authority. Mode defaults accept `monospace` or `proportional`; only syntax/semantic decoration layers may carry range roles; only text-bearing panel/label/button/list/status-item components may carry `style.fontRole`, with `ui` as component default. User `clay.theme.setTypography` configuration remains the sole owner of concrete family fallback stacks and logical-pixel sizes.

Validation rejects unknown roles and concrete or executable fields including `fontFamily`, `fontFamilies`, `fontSize`, `fontStack`, font paths/bytes/URLs/downloads, raw CSS, raw Parley properties, renderer callbacks, native handles, client JavaScript, and raw ops. Packages cannot enumerate installed fonts or learn which fallback resolved. Role installation/normalization is bounded load/activation/background-publication work; native hot paths read cached client state and perform no package JavaScript, IPC, filesystem/network access, or font discovery.

## Range Diagnostics Authority Boundary

[Range diagnostics](diagnostics.md) publish inert `DiagnosticSet` data under existing `render-decorations`. Explicit analyzer packages call `clay.diagnostics.serverPublishDiagnostics`; Tree-sitter highlighting does not publish recovery-node diagnostics. Publication grants no filesystem, network, shell, AI, WASM, workspace, language-server process, raw-op, client-JavaScript, CSS, draw-callback, or native-render authority. Metadata must be bounded/sanitized; empty source chunks clear only that source. Diagnostics cannot choose font roles or replace syntax/semantic decoration state. The Phase 18.20 contribution/grant boundary is implemented and backed by a host-owned, bounded language-server session primitive.

## Language-Server Authority Boundary

Packages declare `language-server` through `clay.capabilities` and fixed entries under `clay.contributions.languageServers`. Descriptor validation accepts only `id`, `executable`, bounded literal `args`, and bounded `inheritEnvironment` names; IDs are package-prefixed and duplicate/dynamic/shell/cwd/environment-value fields fail closed. Requesting the capability without a descriptor, or declaring a descriptor without the capability, is invalid.

Users authorize before loading package code:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/lsp-rust");
```

The configuration-only op resolves/canonicalizes the installed contribution executable, validates every root as a current directory root, and records package source/version/API-prefix, descriptor fingerprint, canonical executable, roots, approver, and approval time. The first `loadPackage` call seals authority mutation for the runtime generation before `loadEntry` import, so loaded package code cannot self-authorize. Missing, unknown-root, mismatched-contribution, stale source/version/descriptor, or revoked grants fail closed. Runtime replacement reruns `init.js` with a fresh grant registry; bundled `NativeTrust` defaults exclude `language-server` unless that fresh generation already recorded an exact grant.

The grant layer records authority only and starts no process. An authorized package opens a host-owned, bounded child session through `clay:language-server.startLanguageServerSession({ package, contribution, workspaceRootId })`, which returns an opaque session wrapper exposing only bounded UTF-8 `send`/`read`/`stop`. The host spawns the child directly via `tokio::process::Command` (never a shell string) using only the fixed validated descriptor executable/argv, `env_clear()` plus the declared inherited environment names, and the approved root's canonical path as cwd, with piped stdio and `kill_on_drop`. Session/read/write/stderr are hard-capped by `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`, `LANGUAGE_SERVER_MAX_SESSIONS`, `LANGUAGE_SERVER_STDERR_BUDGET_BYTES`, and a read timeout; diagnostics are sanitized. Every session operation rechecks the current grant identity (package/contribution/fingerprint); missing/stale grants, unknown roots, wrong cwd, timeout, and child exit fail closed with typed errors and cleanup. Package withdrawal, runtime-generation replacement (reload), and server shutdown reap owned sessions. LSP `Content-Length` framing and server initialization are deferred to Phase 18.21 package adapters layered over this opaque transport.

The approved containment warning still applies to every live session: cwd/root identity and `kill_on_drop` are launch metadata, not an OS filesystem/network/process sandbox, and a same-user child can read other paths, use network, or spawn processes. See `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Server-Side Validation Checklist

Clay must reject a package primitive contribution before it becomes active when any check fails:

1. Package manifest schema is valid and contains required Clay metadata.
2. `apiPrefix` is valid and unique.
3. Contribution IDs are package-prefixed and do not claim reserved `clay.*` IDs unless first-party.
4. Primitive schema matches the registered primitive category.
5. Contribution payload size is within the category budget (`BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`, or the category-specific budget).
6. Required permissions are declared in package metadata and are known to Clay.
7. The primitive does not call raw `Deno.core.ops` or expose raw op names as the user-facing API.
8. The primitive does not contain client-side JavaScript references, draw callbacks, widget callbacks, CSS/HTML/script injection, raw style strings, direct Masonry widget constructors, native widget IDs, or native handles. `SyntaxGrammarContribution` also rejects non-`@clay/*` grammar packages, grammar/query URLs, absolute paths, parent traversal, native-library fields, runtime downloads, package-manager/download/shell fields, and style-map values outside Clay's known style-token allowlist. Tier 2 overrides require an explicit user engine preference; package load order cannot silently replace Tier 1 native highlighting. Third-party grammar/native trust remains out of scope until Phase 23 and a separate security decision.
9. Runtime handlers are registered through Clay JS facade APIs, not direct Rust public functions.
10. Version, document, viewport, behavior, mode, and package provenance fields are present where the category requires them.
11. Command execution requests normalize SDUI actions, package UI action intents, keybinding intents, and transient-menu selections through one server-owned boundary that re-checks command ID, routing policy, provenance, permissions, target context, argument budget, and session freshness before any side effect.

## Tiered Syntax Engine Authority Boundary

Tier 1 native grammar data is compiled into Clay and registered from static first-party descriptors; packages cannot load arbitrary native libraries. Tier 2 web-tree-sitter uses only resolver-validated, package-root-confined `./grammars/*.wasm` and `./queries/*.scm` assets. There are no runtime downloads and no shell/package-manager execution; Clay does not fetch artifacts or build WASM during open/parse. Tier 3 package-JS fallback uses existing server-issued parse-handler tokens and receives only bounded open-document windows.

`setSyntaxEnginePreference(target, tier)` is a documented user configuration API evaluated at init/package-load/open/reclassification time. It accepts `native`, `wasm`, and `javascript`/`js`; it does not grant filesystem, network, shell, package-manager, extension loading, native-library, WASM artifact, client-side JavaScript, AI mutation, or workspace authority. Open returns before parsing completes, and handler failures publish sanitized `clay.parse.open_failed` diagnostics. Parse/query work, package loading, configuration evaluation, and artifact validation stay outside keypress, paint, layout, scroll, pointer, and text-event handlers.

## Conflict Handling

Conflict handling is deterministic, provenance-preserving, and policy-driven. `check_enabled_packages` still builds the canonical conflict index and reports duplicate/overlapping contributions with both package provenances. `PackageConflictResolutionPolicy` can then resolve a conflict only through explicit inputs:

1. **User override:** documented user configuration selects the winning package for a contribution ID.
2. **Package graph replacement/disable:** a package with a user-approved `package-control` grant may declare `replaces` or `disables`; `PackageService::enable` withdraws the target and records a `PackageConflictResolutionDiagnostic`.
3. **Explicit priority/routing metadata:** key bindings with distinct priority/routing entries are non-conflicting; identical priority/routing falls back to the deterministic diagnostic.
4. **Diagnostic fallback:** unresolved conflicts remain load-time errors. Clay never resolves by load order alone.

Security rule: a package cannot override, replace, or disable another package without either explicit user conflict configuration or a user-approved `package-control` grant. Resolution runs only at enable/load/reload/package-control time; editor hot paths read already-resolved state.

| Conflict | Required behavior |
| --- | --- |
| Duplicate package prefix | Reject unless explicit user configuration selects a winner or a package-control `replaces`/`disables` relation withdraws the target. |
| Duplicate mode name | Reject unless a documented user override or package-control replacement relation resolves the active owner. |
| Duplicate command ID | Reject unless explicitly resolved; command IDs should normally remain package-prefixed. |
| Same key binding | Distinct declared priority/routing entries are allowed; identical priority/routing is rejected unless user configuration selects a winner. |
| Same text transform trigger | Reject unless future explicit priority metadata is schema-validated and tested. |
| Decoration range overlap | Allow overlap only because decorations carry priority, package prefix, and style token; equal priority falls back to deterministic package order. |
| SDUI region collision | Reject unless explicit user configuration or future region/slot priority selects a winner. |
| Duplicate shell slot claim | Reject unless explicit slot priority/precedence metadata is documented, schema-validated, bounded, and tested. |
| Duplicate component or overlay ID | Reject unless package-control replacement/update withdraws the previous owner. |
| Unknown style/theme token | Reject with package/token/source diagnostics; raw CSS/style strings are never treated as fallback tokens. |
| Unsupported UI state scope | Reject hidden globals, ad hoc keys, or undeclared scopes before package UI state affects the shell. |
| Configuration key collision | Reject unless explicit user configuration or package-control replacement selects the active owner. |
| Unregistered or unauthorized command execution | Reject command intents whose command ID, routing policy, provenance, permissions, target context, argument budget, or session freshness fail validation; client-first and client-ui routing policies must not be executed from server-owned menus or action intents. |

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

## Powerful Capabilities Require Explicit Grants

Powerful package capabilities require explicit user/admin grants and documented Clay APIs before use:

- filesystem scopes
- network access
- shell commands
- AI/tool orchestration or mutation
- remote listeners
- WASM execution
- raw `Deno.core.ops` or low-level debug ops
- direct Masonry/widget/native UI extension APIs
- native widget IDs or native widget handles
- raw CSS, raw style strings, or HTML/script injection
- client-side package runtime
- extension/package installation or enable/disable mutation
- workspace mutation outside declared workspace APIs

These capabilities are not categorically unavailable to user-installed packages. They must be visible, revocable, provenance-preserving, documented, and tested before implementation.

## Native UI and Client Runtime Are Explicit Capability/API Work

Native UI and client-side package runtime are explicit capability and API work, never implicit through package source. A package does not receive native widget handles, direct Masonry mutation, raw CSS, client-side JavaScript, renderer callbacks, or native widget IDs merely because it was installed from npm, GitHub, git URL, tarball, or a local path.

- The UI/layout authoring contract is identical for `@clay/*` packages and user-installed packages. `@clay/*` means shipped by Clay, not more capable; both package kinds contribute UI/layout through the same `clay:ui` facades, `PackageService` validation, shell/slot/precedence rules, and conflict-resolution policy. See [Creating Clay Packages — Unified UI/layout authoring contract](../packages/creating-packages.md#unified-uilayout-authoring-contract-across-package-sources).
- `native-ui` (approved native UI/widget extension APIs) and `client-runtime` (approved client-side package code when implemented) are grantable capabilities in the unified vocabulary, but they are granted only through an explicit user/admin authorization record tied to package identity/source/provenance — never inferred from the package source kind.
- A granted `native-ui` or `client-runtime` capability still requires a matching, documented, validated, revocable Clay API before the surface exists. A capability grant authorizes a package to use a surface; it does not materialize the surface or bypass Masonry/client safety validation. Native widget handles, client JavaScript, raw CSS/style strings, and renderer callbacks remain rejected until the corresponding API, validator, and tests ship.
- No UI/layout/security primitive branches on package source. The capability, validation, conflict, revocation, and diagnostic paths are the same code regardless of whether a package was bundled by Clay or installed by a user.

## Hot-Path and Failure Policy

- Package load validation is allowed to inspect manifests, declarations, permissions, and bounded static payloads.
- Edit-time validation is limited to cheap version/range/payload checks on already-declared primitive categories.
- JavaScript parsing, completion, diagnostics, and commands run server-side and asynchronously according to their routing policy.
- Validation errors must produce structured package-load diagnostics that include package name, version, prefix, primitive category, contribution ID, source slot/component/action/token/input/state-scope/option/override target when applicable, payload size when applicable, and failed rule. Phase 18.3 package UI diagnostics must identify the panel/component/overlay/token ID, requested slot or overlay policy, action/command target, style variable or theme token, and the failed prefix/permission/payload/non-authority rule. Phase 18.4 input/state/configuration diagnostics must identify the input ID, state scope/key, layout override target/property/source, package option name, payload estimate, missing permission/API dependency, and package provenance before enable/load can activate the contribution.
- Runtime failures after load must degrade the affected primitive only: stale decorations may remain, commands may fail with an error, and SDUI contributions may be withdrawn, but the editor must not panic or block typing.
