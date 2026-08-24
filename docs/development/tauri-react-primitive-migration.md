# Tauri/React Primitive Migration Matrix

Plan 097 Phase 1 — review of existing editor, package, protocol, and runtime
primitives before port work. Sources:
`docs/reference/primitives/registry.md`,
`docs/wiki/modules/{primitive-architecture,server-driven-ui,decoration-transport,parse-coordinator,embedded-js-runtime,phase25-agent-host-primitive-review}.md`,
`.agents/skills/project-patterns/references/{mode-primitive-first,authority-boundaries}.md`,
and the parity ledger `docs/development/tauri-react-parity-ledger.json`.

Approved architecture: `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.

## How to read this matrix

Phase 12 is complete. Paths in the former-native-source column are historical
migration inputs and may no longer exist; every row marked **delete** has been
removed. Target paths are current.

Dispositions (closed vocabulary):

- **reuse** — server/Rust primitive is unchanged; the Tauri bridge only
  transports it. No new authority, schema, or budget.
- **adapter** — thin Rust translation between the existing server protocol and
  typed frontend DTOs/channels (`src-tauri/src/bridge/*`). Validation stays in
  Rust before any DTO is built; archived bytes never reach the webview.
- **projection** — React/CodeMirror frontend code renders or executes inert
  data that already crossed validation (components, decorations, behavior-manifest
  rules). Frontend projection never validates authority and never becomes
  canonical state.
- **delete** — native-client-only implementation removed after verified parity.

Hot-path contract (unchanged from the native client):

- CodeMirror applies user transactions locally before any IPC; bounded edit
  queues ack/correct/resync asynchronously.
- Rendering reads cached validated state only: no package JavaScript,
  theme re-resolution, parse work, or blocking IPC in paint/layout/input paths.
- Viewport-bounded decoration/diagnostic requests stay deduplicated and
  background-scheduled by the existing `ParseCoordinator`.

DTO deny list (enforced by
`tests/documentation_coverage.rs::frontend_bridge_sources_stay_free_of_forbidden_authority_markers`
once bridge sources exist): frontend DTOs contain no filesystem handles, no
process handles, no V8 values/functions, no raw op names, and no archived-byte
(`rkyv`) access. IDs that may exceed `Number.MAX_SAFE_INTEGER` are strings;
editor positions convert UTF-8↔UTF-16 at exactly one reviewed boundary.

## Summary matrix

| Area | Current owners | Registry primitives | Disposition | Target owner |
| --- | --- | --- | --- | --- |
| Documents | `src/server/document.rs`, `src/editor/document_session.rs`, `src/masonry_pane_document.rs` | — (protocol-owned) | reuse + adapter + projection + delete (native shadow) | `frontend/src/editor/sync/*`, `document-store.ts` |
| Behavior manifests | `ActiveBehaviorManifest`, `src/client/behavior.rs` | TextTransform, KeyRoutingOverride, MajorModeActivation, DocumentClassification | reuse + projection + delete (native executor) | `frontend/src/editor/extensions/{behavior,keymaps}.ts` |
| Edits | `src/masonry_pane_document.rs`, `src/editor/surface/mod.rs` | CommandExecution (side effects), TextTransform (local rules) | adapter + projection + delete (native queue/shadow) | `frontend/src/editor/sync/*` |
| Decorations | `src/server/decorations.rs`, `EditorDecorationState`, `src/protocol/decorations.rs` | DecorationRange, DiagnosticSpan, FoldingRange, IncrementalParseUpdate, SyntaxGrammarContribution | reuse + adapter + projection + delete (native chunk cache/paint) | `frontend/src/editor/extensions/{decorations,diagnostics,folding}.ts` |
| Completion | `src/server/completion.rs`, `ParseCoordinator` lane, `TransientMenuSession` display | CompletionTriggerAndResult, TransientMenuSession | reuse + adapter + projection + delete (native menu display path) | `frontend/src/editor/extensions/completion.ts` |
| Intelligence | `src/server/language_intelligence.rs`, LSP bridge packages | LanguageIntelligenceRequestAndResult, LanguageServerSession | reuse + adapter + projection | `frontend/src/editor/extensions/intelligence.ts` |
| SDUI | `src/protocol/sdui.rs`, `src/server/sdui.rs`, `SduiNativeState`/region widgets | SduiPanelStatusContribution | reuse + adapter + projection + delete (Masonry widgets) | `frontend/src/sdui/*` |
| Package UI | `src/server/ui.rs`, `src/shell/package_ui.rs`, catalog in `src/shell/components.rs` | PanelContribution, ComponentContribution, TransientOverlayContribution, PackageInputContribution, PackageUiStateScope, PackageThemeTokenDeclaration | reuse + projection + delete (client-side install widgets) | `frontend/src/packages/*` |
| Configuration | `src/server/configuration.rs`, config watch/generation swap | PackageOwnedConfiguration, PackageLayoutOverride, PackagePermissionDeclaration | reuse + adapter + projection | `frontend/src/configuration/*` (status/diagnostics projection) |
| Themes | `src/shell/theme.rs` resolver, `StyleRegistry`, `TypographyRegistry` | PackageThemeTokenDeclaration, SemanticTypographyRole | reuse + adapter + projection + delete (native resolvers) | `frontend/src/theme/*` |
| Menus / Command Centre / path browser | `src/server/control_center.rs`, `menu_sessions.rs`, `workspace.rs`; `src/shell/transient_menu.rs`, `path_browser.rs` | TransientMenuSession, BuiltInUserBrowseListing, CommandDeclaration, CommandExecution | reuse + adapter + projection + delete (native geometry/paint) | `frontend/src/command-centre/*`, `path-browser/*` |
| Tabs / splits / panes | `src/server/connection.rs` tab registry, `src/shell/layout.rs`, `src/masonry_shell/*`, `src/driver/*` | TabRegistryBinding, TabBarChrome, WorkingAreaLayout, PaneSplitTree, PaneSlotLayout | reuse + adapter + projection + delete (window chrome) | `frontend/src/shell/{tabs,splits,panes}/*` |
| Agents / Chat | `clay-agent/` daemon, `src/protocol/agent.rs`, Prism session store | CommandExecution (`agent.*` command IDs) | reuse + adapter (new generic AG-UI mapping) + projection | `src/server/agent_agui.rs`, `src-tauri/src/bridge/agent.rs`, `frontend/src/{agent,chat}/*` |
| Persistence | `src/server/connection.rs` registry, `src/driver/restore.rs`, `src/shell/layout_persist.rs` | TabRegistryBinding | reuse + projection | `frontend/src/persistence/layout.ts` |

## Parity ledger coverage map

Every `docs/development/tauri-react-parity-ledger.json` capability row maps to
one matrix area (or cross-cutting section) below:

| Capability ID | Matrix area |
| --- | --- |
| `launch.connection.lifecycle` | Cross-cutting: server lifecycle is reused unchanged; Tauri owns window/process integration only (Phase 2 bridge) |
| `configuration.initjs.runtime` | Configuration |
| `files.workspace.documents` | Documents + Menus / Command Centre / path browser (browse/grant UX) |
| `editing.core.pipeline` | Documents + Edits + Behavior manifests |
| `movement.selection` | Edits (client-local cursor/selection projection) |
| `multicursor` | Edits (client-local multi-cursor projection) |
| `caret.typography.rendering` | Themes + Decorations |
| `syntax.textobjects.intelligence` | Decorations + Completion + Intelligence |
| `packages.modes.settings.themes` | Package UI + Configuration + Themes |
| `package.sdui.ui.registry` | SDUI + Package UI |
| `keybindings.commands.menus` | Menus / Command Centre / path browser + Behavior manifests |
| `performance.budgets.feel` | Cross-cutting: budgets |
| `platform.windows` | Cross-cutting: long-term target, Linux remains blocking |
| `shell.splits.panes` | Tabs / splits / panes |
| `tabs.workspaces.persistence` | Tabs / splits / panes + Persistence |
| `agent.chat.prism` | Agents / Chat |
| `accessibility.semantics` | Cross-cutting: accessibility |
| `security.trust-domains` | Cross-cutting: trust domains |

## Area detail

### Documents

- Reuse: server ropes, versions, leases/read-only observers, open-document
  registry, save/reload validation (`clay:documents` ops unchanged).
- Adapter: `DocumentId`/versions/deltas become typed DTOs; string IDs.
- Projection: CodeMirror document text is the local projection; dirty/read-only
  status chrome from metadata snapshots.
- Delete after parity: native shadow rope/cache and pending-edit bookkeeping in
  `src/editor/document_session.rs` / `src/masonry_pane_document.rs`.
- Gap: none new. The one-time UTF-8↔UTF-16 position map is a pure conversion
  module (`frontend/src/editor/position-map.ts`), property-tested against the
  Rust rope conversion; it introduces no authority.

### Behavior manifests

- Reuse: manifest compilation, generation stamping, broadcast, stale rejection
  all server-side; wire shape unchanged.
- Projection: generic frontend executor runs `ClientFirstPredictable`
  transform/key-routing rules as CodeMirror keymap/transform extensions — same
  inert rule vocabulary (Enter/Tab/pairs/comments/electric characters).
- Delete: `src/client/behavior.rs` native rule execution engine.
- Invariant preserved: manifests are Rust-known data; no JS callbacks arrive at
  the frontend either.

### Edits

- Reuse: edit validation, correction, ordering, per-document versioning on the
  server; optimistic model unchanged (local apply → queue → ack/reject/resync).
- Adapter: bounded edit frames over the existing codec via bridge commands.
- Projection: CodeMirror transaction annotations distinguish user, correction,
  resync, remote, undo, programmatic operations; sync module owns queue state.
- Hot path: local CM dispatch first; no await before paint; no full-document
  frames for ordinary edits.

### Decorations

- Reuse: `DecorationSet`/`DecorationBatch`, `DiagnosticSet`, `FoldingRangeSet`
  transport, viewport requests, chunk budgets, provenance/version validation,
  128-byte replacement grid, `SYNTAX_CACHE_BUDGET_BYTES`.
- Adapter: chunk DTOs mirror validated sets; nothing decoded reaches React
  unvalidated.
- Projection: CodeMirror decoration/diagnostic/folding extensions render
  chunks; provisional optimistic interpolation (currently
  `EditorDecorationState`) is reimplemented as a frontend extension with the
  same exact-range replacement semantics — port work, not new authority.
- Delete: native chunk caches and Vello span painting.

### Completion

- Reuse: provider registry, coordinator cancellation/stale rejection, buffer-words
  built-in, payload caps, trigger manifest data.
- Adapter: result sets as bounded DTOs.
- Projection: CodeMirror autocompletion source renders items; accept routes the
  item's declared action through the shared `CommandExecution` intent path —
  identical to today's `TransientMenuSession` accept routing.
- Delete: native completion display adapter into menu sessions.

### Intelligence

- Reuse: request/result types, feature tags, timeouts, definition/action/signature
  caps; LSP framing stays package-side JavaScript.
- Projection: hover tooltips, signature help, code-action menus, go-to-definition
  navigation through existing OpenDocument/command intents.

### SDUI

- Reuse: `SduiTree`/snapshot/update protocol, stable `SduiNodeId`s, action-intent
  validation, publication budgets (bytes/nodes/depth/text), editor-binding checks.
- Adapter: snapshot/update DTOs keyed by node ID (string-safe).
- Projection: stable-ID React reconciler — unchanged node IDs keep component
  state across updates (the native retained-subtree guarantee carried forward);
  component kinds map onto Clay components (React Aria for collection/menu/
  combobox/modal behavior); typed tokens resolve to CSS custom properties.
- Delete: `src/masonry_sdui*.rs`, `SduiNativeState`, region widgets.

### Package UI

- Reuse: contribution registration/validation, slot ownership, overlay focus/
  dismissal policy, input-routing declarations, UI-state scopes, theme-token
  declarations, trust-domain classification — all server-side and unchanged.
- Projection: fixed panels/overlays/status render from validated declarations;
  package actions remain inert command intents.
- Delete: client-side install widgets in `src/shell/package_ui.rs` consumers of
  Masonry (`PackageRegionWidget` etc.).
- Known generic gaps carried from the Phase 25 review (child-plan required
  before a package consumes them):
  - Generic pane-content package contribution (empty-tab `main` hosting) so
    `@clay/chat` landing works without product-named kinds.
  - Multiline `textArea` component kind for the Chat composer (single-line
    `textInput` cannot host newline chords).

### Configuration

- Reuse: `init.js` evaluation, candidate/reload atomicity, generation swap,
  option validation, package options/layout overrides — fully server-side.
- Adapter/projection: reload status, diagnostics, and effective settings are
  read-only projections. No frontend preference store; browser storage may hold
  non-authoritative view state only (later phase decision).

### Themes

- Reuse: token catalog, package token declarations with same-typed fallbacks,
  design-token overrides, typography profile ownership (`theme.setTypography`),
  `ActiveTheme`/`ActiveTypography` broadcasts, WCAG contrast enforcement.
- Adapter: resolved theme/typography snapshots as typed DTOs.
- Projection: one adapter maps resolved tokens to `--clay-*` CSS custom
  properties and CodeMirror theme/highlight inputs (including the two-axis
  `TokenType`+`Modifiers` style table replacing `StyleRegistry` lookups);
  resolution happens at snapshot install, never per frame.
- Delete: native `ResolvedUiTheme`/`StyleRegistry`/`TypographyRegistry` client
  caches (server-side validation counterparts remain).

### Menus / Command Centre / path browser

- Reuse: `TransientMenuSession` lifecycle, Control Center command snapshots,
  fuzzy filtering, generation stamping, browse listing bounds, grant conversion
  on activation — all server-authoritative.
- Projection: React dialog/listbox renders bounded snapshots and emits typed
  query/selection/activate/cancel/backspace intents.
- Delete: `src/shell/transient_menu.rs` and `src/shell/path_browser.rs` client
  geometry/paint/scoring surfaces.

### Tabs / splits / panes

- Reuse: server tab registry (identity/order/workspace binding), per-tab
  independent grants, split/slot topology validation, persisted layout schema
  where compatible.
- Projection: React tab strip, accessible split separators, pane hosts; stable
  IDs preserve editor/scroll/focus state across reconciliation.
- Delete: window-tab chrome and Masonry shell widget tree.

### Agents / Chat

- Reuse: `clay-agent` Node daemon, stdio JSON-RPC, session/transcript/provider
  ownership on the server, credential vault separation, `agent.*` command IDs
  routed through `CommandExecution`.
- Adapter (new generic module, child plan required before `@clay/chat` consumes
  it): `src/server/agent_agui.rs` maps internal agent event families to AG-UI
  lifecycle/text/state/error events over a Tauri channel; React uses a custom
  `AbstractAgent` transport and keeps no parallel reducer. Credentials never
  enter events, snapshots, logs, DOM attributes, or accessibility names.
- Packages cannot spawn or speak to the daemon — unchanged.

### Persistence

- Reuse: server tab/workspace restore flow and validation; corrupt/hostile
  layout fallback stays fail-closed.
- Projection: `frontend/src/persistence/layout.ts` persists only the approved
  non-authoritative subset of the existing schema; canonical state restores
  through the server.

## Cross-cutting

- Accessibility: AccessKit tree semantics translate to DOM ARIA through semantic
  HTML first and React Aria primitives for collections/menus/dialogs; modal
  containment, focus restoration, and announcements keep their current contracts
  (ledger row `accessibility.semantics`).
- Budgets: `src/perf/budgets.rs` constants remain the named budgets; Phase 4+
  adds equivalent frontend checks rather than raising or dropping limits
  (ledger row `performance.budgets.feel`).
- Trust domains: exactly two persistent runtimes, compiled bundled inventory,
  cross-domain envelopes, adoption/replacement records — untouched by this
  migration (ledger row `security.trust-domains`). Tauri capabilities add a new
  deny-by-default boundary around the webview; broad filesystem/shell/process
  plugins stay denied.

## Verification

- `tests/documentation_coverage.rs::primitive_migration_matrix_covers_ledger_and_registry_primitives`
  — every ledger capability row appears here with a disposition, and every
  registry primitive cited exists in `docs/reference/primitives/registry.md`.
- `tests/documentation_coverage.rs::frontend_bridge_sources_stay_free_of_forbidden_authority_markers`
  — tripwire over future `src-tauri/src` / `frontend/src` bridge sources for
  the DTO deny list; active as soon as those directories exist.
