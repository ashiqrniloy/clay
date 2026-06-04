# Prioritized Primitive Backlog and Phase 17 Prerequisite Checklist

This backlog turns the Phase 16 primitive analysis into a sortable implementation queue for Phase 17 package/mode loading and the Phase 18 Markdown mode POC. Every entry traces to `docs/reference/primitives/registry.md` and at least one Phase 16 analysis document. Phase 16.5's implemented validation bridge is documented in `docs/reference/primitives/implementation-gate.md`.

Priority tiers:

- **Phase-17-required**: must be implemented or stubbed by the Phase 17 package/mode foundation before Markdown work can safely begin.
- **Phase-18-required**: can be implemented in Phase 18, but Phase 17 must leave a compatible extension point or explicit handoff.
- **Deferred**: recorded for future modes/packages, not a Markdown POC readiness gate.

## Phase-17-required

| Primitive | Category | Priority Tier | Estimated Implementation Location | Clay JS API ID Target | Plan Owner | Permission / Security Note | Registry Trace | Phase 16 Analysis Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| DocumentClassification | mode/document classification | Phase-17-required | Package manifest schema + new server mode registry + load-time validator | `clay.modes.serverRegisterModePattern` | Phase 17 | Requires `mode-registration`; package pattern metadata must be declared and server-validated before classification becomes executable. | `DocumentClassification` | `registry.md`, `markdown-mode-requirements.md`, `package-security.md` |
| MajorModeActivation | major mode activation | Phase-17-required | New server mode registry + per-document active-mode state + behavior manifest selection path | `clay.modes.serverActivateMajorMode` | Phase 17 | Requires `mode-activation`; activation must be server-owned and publish only validated inert manifests/declarations. | `MajorModeActivation` | `registry.md`, `markdown-mode-requirements.md`, `package-security.md` |
| CommandDeclaration | command registry | Phase-17-required | New server command registry + Clay JS API stub/op + package load contribution validation | `clay.commands.serverRegisterCommand`; metadata query `clay.commands.serverListCommands` | Phase 17 | Requires `command-registration`; handler-side permissions remain separate and must be declared/server-validated before execution. Listing commands returns metadata only and grants no command execution authority. | `CommandDeclaration` | `registry.md`, `markdown-mode-requirements.md`, `package-security.md`, `audit.md` |
| PackagePermissionDeclaration | package permissions/provenance | Phase-17-required | Package manifest validator + permission table + load-time diagnostics | `clay.packages.serverValidatePackagePermissions` | Phase 17 | Validation primitive itself grants no authority; all requested permission-bearing primitives must be declared and server-validated before package enable/load. | `PackagePermissionDeclaration` | `registry.md`, `package-security.md` |
| KeyRoutingOverride | key routing and behavior manifest contribution | Phase-17-required | Behavior manifest extension + package-prefixed keybinding conflict metadata | `clay.keybindings.bindKey` | Phase 17 | Inert declarations require no permission; target command permissions apply at command execution and must be server-validated. | `KeyRoutingOverride` | `registry.md`, `audit.md`, `markdown-mode-requirements.md`, `package-security.md` |
| TextTransform | deterministic text transforms | Phase-17-required | Behavior manifest extension for package-owned transform rule declarations | `clay.behavior.getActiveBehaviorManifest` | Phase 17 | Inert manifest data requires no permission; client executes only Rust-known transform engines and never package JavaScript. | `TextTransform` | `registry.md`, `audit.md`, `markdown-mode-requirements.md` |
| SduiPanelStatusContribution | package SDUI panels/status | Phase-17-required | Existing SDUI server validation extended with package region/provenance metadata | `clay.sdui.publishTree` | Phase 17 | Inert UI requires no permission; SDUI actions targeting commands inherit command permissions and must be declared/server-validated. | `SduiPanelStatusContribution` | `registry.md`, `audit.md`, `rendering-strategy.md`, `markdown-mode-requirements.md` |
| PackageOwnedConfiguration | package configuration surface | Phase-17-required | Package manifest/config schema + `~/.config/clay/init.js`-driven configuration integration | `clay.configuration.setPackageOption`; preferences `clay.configuration.setModePreference`, `clay.configuration.setDecorationTheme`, `clay.configuration.setParsePolicy` | Phase 17 | Requires `package-configuration` when behavior-changing; configuration must not implicitly grant filesystem, network, shell, workspace, or AI authority. Package enable/disable remains intentionally unexposed until a future approved decision log defines that authority. | `PackageOwnedConfiguration` | `registry.md`, `package-security.md`, `markdown-mode-requirements.md` |

## Phase-18-required

| Primitive | Category | Priority Tier | Estimated Implementation Location | Clay JS API ID Target | Plan Owner | Permission / Security Note | Registry Trace | Phase 16 Analysis Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| DecorationRange | inline decoration/render data | Phase-18-required | New bounded protocol message + server validation + client editor render hook outside paint-time package code | `clay.decorations.serverPublishDecorations` | Phase 18 | Requires `render-decorations`; span payloads must be declared, bounded, versioned, and server-validated before client delivery. | `DecorationRange` | `registry.md`, `rendering-strategy.md`, `parse-update-strategy.md`, `markdown-mode-requirements.md`, `package-security.md` |
| IncrementalParseUpdate | background parse task/results | Phase-18-required | New `src/server/parse_coordinator.rs` + `clay:parse` facade/op + viewport-prioritized result publication | `clay.parse.serverRegisterParseHandler` | Phase 18 | Requires `parse-document`; parser execution is server-side only and cannot access filesystem, network, shell, AI, WASM, raw ops, or client JavaScript. | `IncrementalParseUpdate` | `registry.md`, `parse-update-strategy.md`, `rendering-strategy.md`, `markdown-mode-requirements.md`, `package-security.md` |
| FoldingRange | optional Markdown headings/code fences | Phase-18-required | New folding range protocol/server validation + client-local fold state, if pulled into Markdown POC stretch scope | `clay.folding.serverPublishFoldingRanges` | Phase 18 (optional/stretch) | Requires `render-folding`; ranges must be bounded, versioned, and server-validated before executable UI state changes. | `FoldingRange` | `registry.md`, `markdown-mode-requirements.md`, `package-security.md` |

## Deferred

| Primitive | Category | Priority Tier | Estimated Implementation Location | Clay JS API ID Target | Plan Owner | Permission / Security Note | Registry Trace | Phase 16 Analysis Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| MinorModeActivation | optional mode overlays | Deferred | Server mode compatibility registry + deterministic manifest/declaration composition | `clay.modes.serverActivateMinorMode` | Later phase | Requires `mode-activation`; compatibility and conflict policy must be declared/server-validated before overlay activation. | `MinorModeActivation` | `registry.md`, `markdown-mode-requirements.md`, `package-security.md` |
| CompletionTriggerAndResult | completion providers | Deferred | Manifest trigger metadata + server completion provider registry + cancellable result protocol | `clay.completion.serverRegisterCompletionProvider` | Later phase | Requires `completion-provider`; no network, workspace, shell, or AI authority by default without a future decision log and explicit permissions. | `CompletionTriggerAndResult` | `registry.md`, `package-security.md` |

## Phase 17 Prerequisite Checklist

Phase 17 is ready to start when this backlog is accepted and used as its planning input. Phase 18 Markdown is ready to start only after Phase 17 satisfies these checks:

- [x] Package manifest validation supports package identity, `apiPrefix`, permissions, mode declarations, load/runtime entry separation, and documentation metadata; Phase 17 reuses the Phase 16.5 `implementation-gate.md` fixture contract instead of re-deriving it.
- [x] Package enable/load rejects invalid prefixes, unknown permissions, duplicate package prefixes, duplicate mode names, duplicate command IDs, and ambiguous keybinding conflicts with actionable diagnostics documented in `implementation-gate.md`, `package-security.md`, and `package-loading.md`.
- [x] `DocumentClassification` can register static extension/MIME patterns for open-document metadata without filesystem scans or package JavaScript in the client.
- [x] `MajorModeActivation` can atomically select one major mode per document and publish a validated behavior manifest/declaration set with behavior/version provenance.
- [x] `CommandDeclaration` can register package-prefixed commands with user-facing names, routing policies, key binding metadata, custom properties, permission metadata, and a metadata-only `clay.commands.serverListCommands` query.
- [x] Package-owned `KeyRoutingOverride`, `TextTransform`, and `SduiPanelStatusContribution` contributions preserve package provenance and stay within existing manifest/SDUI budgets.
- [x] Clay JS API inventory stubs exist for the Phase-17-required and Phase-18-required API ID targets listed above, with planned or runtime-backed status matching current implementation/docs readiness.
- [x] Configuration API stubs cover Phase 16 user-configurable surfaces through `~/.config/clay/init.js`: package options, mode preferences, decoration themes, and parse policies; package enable/disable stays out of scope until a future decision log approves that authority.
- [x] Phase 17 explicitly hands off `DecorationRange` and `IncrementalParseUpdate` implementation hooks to Phase 18 so Markdown syntax decoration and background parsing are not rediscovered late.

## Sorting and Handoff Notes

- Sort first by `Priority Tier`, then by `Plan Owner`, then by risk/permission-bearing status from `package-security.md`.
- Phase 17 should implement all `Phase-17-required` rows or explicitly document any exception before Phase 18 begins.
- Phase 18 should treat `DecorationRange` and `IncrementalParseUpdate` as required Markdown POC foundations; `FoldingRange` remains optional/stretch despite being listed in the Phase-18 tier for visibility.
- Deferred rows remain registry obligations but should not block Markdown mode unless the Phase 18 scope expands.

## References

- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/audit.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/markdown-mode-requirements.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/implementation-gate.md`
- `roadmap.md` Phase 17 and Phase 18
