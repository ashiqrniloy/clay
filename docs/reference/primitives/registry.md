# Clay Primitive Registry Schema

Version: `phase16-primitives-v1`

This registry defines the first exhaustive-but-iterative taxonomy for primitives that Clay packages may contribute. It is an architecture document only: entries marked `New` or `Planned` describe the public Clay JS API shape and validation contract that later phases must implement.

## Registry Schema

Every primitive category entry uses the same vocabulary as `docs/reference/clay-js-api/api-inventory.toml` so future TOML/code-generation work can derive from this document without renaming fields.

| Field | Meaning |
| --- | --- |
| `primitive` | Stable primitive category name. |
| `description` | User/developer-visible capability provided by the primitive. |
| `owner` | Component that owns canonical state or validates declarations (`server`, `client`, or `server->client`). |
| `authority` | Boundary that may create, mutate, publish, or execute the primitive. |
| `hot_path_policy` | Manifest routing policy, async priority, or explicit `no-hot-path` rationale. |
| `js_module` | Proposed Clay JS facade module. |
| `js_export` | Proposed callable/export name, following Clay JS API naming rules. |
| `stable_id` | Proposed stable registry ID. Package-owned IDs must use the package prefix instead of `clay.*`. |
| `user_facing_name` | Search/help label for docs, command palette, and AI-agent lookup. |
| `permissions` | Required permission scope or `none` for inert declarations. |
| `budget_ref` | Budget constant from `src/perf/budgets.rs`, or `no-hot-path` with rationale. |
| `primitive_kind` | One of `manifest-data`, `server-first-command`, `SDUI-state`, `renderer/decorator-data`, or `configuration-data`. |
| `documentation_metadata` | Required docs/registry metadata for later implementation phases. |
| `test_expectations` | Minimum coverage gate expected when implemented. |
| `status` | `Exists`, `Extend`, `New`, or `Deferred`. |

All entries inherit the Phase 16 security baseline documented in `docs/reference/primitives/package-security.md`: primitive declarations are schema-validated server-side, package provenance is retained through package identity and prefix metadata, raw `Deno.core.ops` calls are not part of public package APIs, and packages never execute JavaScript in Rust client keypress, paint, layout, or text-event handlers.

## Category Matrix

| Primitive | Description | Owner | Authority | Hot-Path Policy | JS Module | JS Export | Stable ID | User-Facing Name | Permissions | Budget Ref | Primitive Kind | Documentation Metadata | Test Expectations | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| DocumentClassification | Match open documents to modes by extension, filename, shebang, MIME hint, or package-declared predicate metadata. | server | Server validates package-declared patterns and selects one classification result per document. | no-hot-path: evaluated on open/reload or explicit reclassification, not per keypress. | `clay:modes` | `serverRegisterModePattern` | `clay.modes.serverRegisterModePattern` | Register Mode Pattern | `mode-registration` | `MODE_ACTIVATION_P95_BUDGET_MS` | configuration-data | API inventory entry, mode docs page, package provenance, supported pattern syntax. | Pattern validation, duplicate/conflict handling, extension/MIME matching, no filesystem scan beyond open document metadata. | New |
| MajorModeActivation | Activate exactly one major mode for a document and publish the selected behavior/rendering primitive set. | server->client | Server owns active mode selection; client installs only validated manifest/render declarations. | Server-first activation followed by atomic manifest install; no typing hot path work after activation. | `clay:modes` | `serverActivateMajorMode` | `clay.modes.serverActivateMajorMode` | Activate Major Mode | `mode-activation` | `MODE_ACTIVATION_P95_BUDGET_MS`, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | server-first-command | API inventory entry, mode lifecycle docs, behavior version metadata. | Single-active-major invariant, stale activation rejection, manifest version update, activation latency budget compile/reference test. | New |
| MinorModeActivation | Enable optional mode overlays compatible with the active major mode. | server->client | Server validates compatibility and conflict policy before composing manifests/declarations. | Activation/configuration path only; composed result must keep client-first behavior deterministic. | `clay:modes` | `serverActivateMinorMode` | `clay.modes.serverActivateMinorMode` | Activate Minor Mode | `mode-activation` | `MODE_ACTIVATION_P95_BUDGET_MS`, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | server-first-command | API inventory entry, compatibility metadata, conflict docs. | Compatibility matrix tests, deterministic conflict order, no silent override of major-mode bindings. | Deferred |
| KeyRoutingOverride | Package-declared key bindings and command routing overrides. | server->client | Server/configuration compiles inert keybinding rules; client routes installed rules only. | `ClientFirstPredictable` only for built-in deterministic edits; otherwise `ServerFirst`, `UiReactivePriority`, or `Background`. | `clay:keybindings` | `bindKey` | `clay.keybindings.bindKey` | Bind Key | none for declarations; target command permission applies at execution | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` | manifest-data | Existing keybinding docs plus package provenance/conflict metadata. | Existing bind/list/unbind tests plus package prefix conflict tests in Phase 17. | Exists/Extend |
| TextTransform | Declarative auto-indent, pair insertion, list continuation, comment continuation, and transform rules. | server->client | Server publishes inert rules in behavior manifests; client executes only known transform engines. | `ClientFirstPredictable`; no JavaScript and no IPC before local paint. | `clay:behavior` | `getActiveBehaviorManifest` | `clay.behavior.getActiveBehaviorManifest` | Get Active Behavior Manifest | none | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `CLIENT_EDIT_PAYLOAD_BUDGET_BYTES` | manifest-data | Behavior manifest docs, rule kind docs, examples for Enter/Tab/pair/list continuation. | Manifest validation, local edit latency invariant, Markdown list/code-block fixtures when added. | Exists/Extend |
| IncrementalParseUpdate | Background parser registration, bounded parse-window snapshots, and parse result publication for document versions. | server | Server schedules package parse work through server-side JavaScript runtime and validates results/window snapshots before publication. | `Background`; cancellable and viewport-prioritized, never blocks `ClientFirstPredictable` input. | `clay:parse` | `serverRegisterParseHandler` | `clay.parse.serverRegisterParseHandler` | Register Parse Handler | `parse-document` | `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `SYNTAX_CACHE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` | renderer/decorator-data | API inventory entry, parse lifecycle docs, parse-window schema, result schema, timeout/cancel metadata. | Cancellation, stale version discard, parse-window bounds, memory-budget validation, payload bound, viewport filtering, no client JavaScript. | New |
| DecorationRange | Syntax, semantic, diagnostic, and emphasis spans over byte ranges. | server->client | Server accepts package-produced spans only after schema, version, range, and payload validation; client renders locally. | `Background`/viewport update path; client render uses inert spans in paint without package code. | `clay:decorations` | `serverPublishDecorations` | `clay.decorations.serverPublishDecorations` | Publish Decorations | `render-decorations` | `DECORATION_PAYLOAD_BUDGET_BYTES`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` | renderer/decorator-data | API inventory entry, span kind/style token docs, priority/provenance metadata. | Payload ceiling, overlapping priority, stale document version rejection, viewport-only update tests. | New |
| FoldingRange | Collapsible document ranges such as heading sections or code blocks. | server->client | Server validates byte ranges and document version; client owns local collapsed/expanded UI state. | UI-reactive async update; local fold toggle is client-local after validated ranges arrive. | `clay:folding` | `serverPublishFoldingRanges` | `clay.folding.serverPublishFoldingRanges` | Publish Folding Ranges | `render-folding` | `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` | renderer/decorator-data | API inventory entry, range schema, nesting/priority docs. | Valid range ordering, nested fold handling, stale result rejection, no full-document repaint. | New |
| CompletionTriggerAndResult | Declarative completion trigger characters plus bounded result payloads. | server->client | Trigger metadata is manifest data; completion computation/result publication is server-side and permission-checked. | Trigger detection may be `ClientFirstPredictable`; result fetch is `UiReactivePriority` and cancellable. | `clay:completion` | `serverRegisterCompletionProvider` | `clay.completion.serverRegisterCompletionProvider` | Register Completion Provider | `completion-provider` | `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | manifest-data/server-first-command | API inventory entry, result item shape, trigger metadata, permissions for external sources if ever added. | Trigger manifest validation, cancellation, payload ceiling, no network/shell authority by default. | Deferred |
| CommandDeclaration | Register package commands with labels, routing policy, keybinding metadata, and handler authority. | server | Server owns command registry and validates package prefix, routing policy, and permission requirements. | Load-time registration; execution policy may be `ServerFirst`, `ServerFirstWithLock`, `UiReactivePriority`, or `Background`. | `clay:commands` | `serverRegisterCommand` | `clay.commands.serverRegisterCommand` | Register Command | `command-registration`; command-specific permissions apply at execution | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` or no-hot-path load-time | server-first-command | API inventory entry, command docs, key binding metadata, custom properties, help/search label. | Duplicate command rejection, package prefix validation, routing policy validation, docs coverage. | New |
| SduiPanelStatusContribution | Package-contributed panels, status items, buttons, lists, editor views, and layout regions. | server->client | Package code builds inert SDUI on the server; server validates tree before client publication. | SDUI update path only; not editor text hot path. | `clay:sdui` | `publishTree` | `clay.sdui.publishTree` | Publish UI Tree | none for inert UI; target command permissions apply to actions | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` | SDUI-state | Existing SDUI docs plus package region/provenance metadata. | Existing SDUI validation/codec/structural tests plus package-owned region conflict tests. | Exists/Extend |
| PackageOwnedConfiguration | Package-declared configuration options such as mode defaults, decoration style choices, parser timeouts, and enable/disable state. | server | Configuration entry point and package loader validate settings and permissions before effects occur. | no-hot-path: evaluated at config/package load or explicit setting change. | `clay:configuration` | `setPackageOption` | `clay.configuration.setPackageOption` | Set Package Option | `package-configuration` when setting affects package behavior; none for read-only metadata | `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS`, `MODE_ACTIVATION_P95_BUDGET_MS` | configuration-data | API inventory entry when a concrete setting becomes user-facing; custom properties with type/default/allowed values. | Init.js boundary tests, setting schema validation, no implicit filesystem/network/shell/AI authority. | New |
| PackagePermissionDeclaration | Declare and validate package permission scopes required by primitive contributions and command handlers. | server | Server validates declared permissions at package load; undeclared authority is rejected before execution. | no-hot-path: load/enable validation only. | `clay:packages` | `serverValidatePackagePermissions` | `clay.packages.serverValidatePackagePermissions` | Validate Package Permissions | none for validation itself; validates all requested scopes | no-hot-path: package load validation, outside typing/paint paths | configuration-data | Package manifest docs, permission docs, security notes, decision-log trace. | Missing/unknown permission rejection, prefix validation, no raw op/client JS checks. | New |

## Primitive Category Notes

### DocumentClassification

Document classification is intentionally server-owned. Packages declare static patterns; Clay chooses the active mode from validated metadata rather than allowing arbitrary file-system scans or client-side predicates. Classification may inspect open document metadata and bounded leading content if a future decision allows it, but the Phase 16 baseline only requires extension, filename, MIME hint, and declared package metadata.

### MajorModeActivation

A document has at most one active major mode. Activation publishes a new behavior version and the validated primitive set for that mode. The Rust client receives only inert behavior manifests, decoration updates, SDUI trees, and configuration state; it does not execute mode package JavaScript.

### MinorModeActivation

Minor modes are overlays that must declare compatible major modes. They cannot silently replace a major mode's behavior, key binding, command, or decoration priority. Phase 16 records the primitive but defers implementation until a concrete minor-mode POC justifies the conflict surface.

### KeyRoutingOverride

Key routing reuses the existing behavior manifest and keybinding API where possible. Package-owned keybindings must preserve provenance and deterministic conflict handling. Client-first routes may only target known deterministic client edit authorities.

### TextTransform

Text transforms cover auto-indent, pair insertion, continuation rules, and Markdown-style list/code-block continuation. They remain manifest data: packages may declare rule parameters, but the client executes only Rust-known transform kinds.

### IncrementalParseUpdate

Package parsers run server-side in the constrained JavaScript runtime as cancellable background work. Results carry document versions and are discarded if stale. Large-file handlers receive only validated `ParseWindowSnapshot` text slices selected from viewport/invalidated ranges, bounded by `ParsePolicy` and `SYNTAX_CACHE_BUDGET_BYTES`. The client keeps the last validated decoration/fold state when parsing lags behind edits.

### DecorationRange

Decoration ranges are renderer/decorator data: byte spans, kind, style token, priority, provenance, document version, and viewport range. They cannot contain draw callbacks, arbitrary styles that mutate native widgets, or executable code.

### FoldingRange

Folding ranges are validated ranges plus labels/kinds. Collapsed state is client-local UI state, but range authority comes from server-validated package output.

### CompletionTriggerAndResult

Completion triggers can be manifest data; result providers are server-side and cancellable. Any future provider that needs workspace, network, AI, or shell authority must introduce explicit permissions and a decision log before implementation.

### CommandDeclaration

Commands are public programmatic surfaces and must have documentation metadata. Registration does not grant execution authority; command handlers must separately declare the permissions needed for document, workspace, AI, package, or other side effects. Phase 16 also plans a metadata-only command discovery API, `clay.commands.serverListCommands`, which traces to this same `CommandDeclaration` category and grants no execution authority.

### SduiPanelStatusContribution

SDUI is the approved package UI contribution path for panels and status areas. It remains inert server-validated UI state. Public live SDUI state querying remains undecided and should not be inferred from internal Phase 15 observability types.

### PackageOwnedConfiguration

Configuration remains Clay JS API based. Concrete user-facing settings must be represented as documented APIs with `custom_properties`, not ad hoc settings hidden in package metadata. Phase 16 planned configuration stubs trace here: `clay.configuration.setPackageOption` for package-owned options, `clay.configuration.setModePreference` for mode activation defaults, `clay.configuration.setDecorationTheme` for decoration style preferences, and `clay.configuration.setParsePolicy` for parse timeout/unit/viewport-priority preferences. Package enable/disable is intentionally not exposed as a configuration API in Phase 16 because `docs/reference/primitives/package-security.md` requires a future decision log and explicit permission model before that authority can exist.

### PackagePermissionDeclaration

Permission declarations are load-time package metadata. They cannot request filesystem outside open documents, network, shell, AI mutation, remote listeners, WASM execution, raw Deno ops, native widget mutation, or client-side JavaScript by default.

## Security Requirements Shared by All Categories

- Package-owned stable IDs and exports must carry the package prefix; only first-party Clay APIs may use `clay.*` stable IDs.
- Server validation must reject malformed schemas, unknown primitive kinds, oversize payloads, undeclared permissions, raw operation references, and client-side JavaScript hooks.
- Inert declarations with `permissions = none` still do not grant filesystem, network, shell, AI, WASM, native widget mutation, or raw-op authority.
- Permission-bearing primitives must be declared in package metadata and validated before load/enable succeeds.
- Runtime parse, completion, and command work executes server-side and asynchronously; the client receives validated data or server-routed intents only.

## Budget Constants Proposed by This Registry

The following advisory constants are added to `src/perf/budgets.rs` so later implementation phases can compile against stable names before hard CI thresholds are enabled:

```rust
pub const DECORATION_PAYLOAD_BUDGET_BYTES: usize = 8192;
pub const INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES: usize = 4096;
pub const SYNTAX_CACHE_BUDGET_BYTES: usize = 30 * 1024 * 1024;
pub const MODE_ACTIVATION_P95_BUDGET_MS: u64 = 100;
pub const COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const FOLDING_RANGE_PAYLOAD_BUDGET_BYTES: usize = 2048;
pub const PRIMITIVES_REGISTRY_VERSION: &str = "phase16-primitives-v1";
```

These constants are advisory in Phase 16. Later phases may promote representative payload and latency checks to hard failures after concrete protocol messages exist.

## Required Implementation Follow-Up

- Add planned Clay JS API inventory stubs for new Phase 17/18 APIs after the backlog task chooses priority.
- Add `docs/reference/clay-js-api/` Markdown pages only when the APIs move from planned stubs to implementation-ready public surfaces.
- Add protocol messages for decoration/folding/parse updates only in implementation phases; this registry intentionally does not change runtime behavior.
- Keep this document linked from `docs/index.md` and the future `docs/reference/primitives/index.md` navigation page.
