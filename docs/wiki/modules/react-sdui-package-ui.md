# React SDUI and Package UI Projection

## Source

- `src/protocol/{runtime,sdui}.rs`
- `src/server/ui.rs`, `src/server/mod.rs`, `src/server/connection/runtime.rs`
- `src-tauri/src/bridge/{dto,session,forwarder}.rs`
- `frontend/src/sdui/{types,state,actions,renderer,registry}.ts*`
- `frontend/src/packages/PackageWorkspace.tsx`
- `frontend/src/shell/{workspace-controller,WorkspacePanes,PaneTree}.ts*`
- `frontend/src/sdui/{state,renderer,registry}.test.*`
- Plan: `plans/097-Tauri-React-Architecture-Migration.md` Phase 8

## Overview

Plan 097 Phase 8 ports Clay's validated SDUI and package component trees from
the frozen Masonry client into the Tauri/React target. Server package loading,
validation, command execution, trust domains, runtime generations, and public
package APIs remain unchanged. Tauri is a narrow typed adapter. React owns only
presentation and client-local widget state.

The target supports current SDUI snapshots/updates, all 15 implemented package
component kinds, fixed slots, overlays, status components, generic empty-tab
pane content, and visible host-stamped package provenance. Reserved `table`
remains rejected.

## Responsibilities

- Server validates component kind/tree/style/action/budget/provenance and
  resolves package trust from enabled compiled-inventory records.
- Runtime snapshot publishes bounded package panels, overlays, components,
  input routes, and empty-tab content under one generation.
- Tauri resolves raw theme data and parses validated component JSON into inert
  `serde_json::Value` before the webview observes it.
- React reconciles stable IDs, projects catalog components, owns transient
  input/disclosure/dropdown/scroll state, and emits typed action intents.
- Package JavaScript remains server-side. React never imports package modules,
  JSX, callbacks, CSS, scripts, raw ops, or direct Tauri authority.

Non-responsibilities: Command Centre/path browser (Phase 9), AG-UI Chat stream
(Phase 10), arbitrary third-party custom web surfaces, and native-client
removal (Phase 12).

## How It Works

1. `PackageUiRegistrySnapshot::wire_snapshot` converts validated server records
   into `PackageUiSnapshot`. Every surface carries exact package name, version,
   prefix, and host-resolved `Trusted` or `ThirdParty` domain.
2. `RuntimeStateSnapshot` version 26 installs the SDUI tree and complete package
   UI atomically. Validation caps 4 fixed panels, 16 overlays, 64 input routes,
   16 KiB component JSON, allowed slots/anchors, unique IDs, and valid JSON.
3. Bridge pump intercepts the raw runtime snapshot. `RuntimeSnapshotDto::resolve`
   resolves theme/typography and parses each component string in Rust. Raw
   theme overrides and raw package JSON strings do not reach React.
4. `workspace-controller.ts` routes the snapshot to its owning client/tab,
   ignores stale generations, installs SDUI/package UI together, projects
   document behavior/render resets, then acknowledges
   `runtimeGenerationInstalled`.
5. `state.ts` stores SDUI nodes in a `Map<number, SduiNode>`. A matching-base
   update clones the map and changes only named node IDs. Stale updates return
   the current object unchanged.
6. `renderer.tsx` recursively renders SDUI panels, labels, buttons, lists,
   flex/stack containers, and the real editor slot. Existing server action
   intents are forwarded unchanged with current UI version.
7. `registry.tsx` maps package kinds onto Clay components. In Plan 103, components and slots expose static recipe attributes (`data-clay-component`, `data-clay-slot`, `data-variant`) consuming closed `--clay-ds-*` properties. Stable component IDs are React keys, so unrelated snapshots and runtime design-system variable updates preserve focus, input values, disclosure/dropdown state, and scroll state.
8. `PackageWorkspace.tsx` composes optional top/left/right/bottom panels around
   mandatory `main`, contains overlays, and appends package status items.
   Narrow layout stacks fixed panels while retaining a usable main region.
9. `PaneTree.tsx` renders the one winning `empty-tab` package surface when the
   pane has no path/text. Plan 118 Part D: the bundled `@clay/launcher`
   contribution renders as the host's compiled launcher panel (the
   `hostRenderedSurface` trusted-provenance lookup, alongside the Coding Agent
   panel; see [Launcher Landing Surface](launcher-landing-surface.md)); any other
   `empty-tab` contribution renders through the generic SDUI view, and no
   contribution renders the core Open File/Open Folder fallback (which keeps no
   product-named landing).
10. Plan 108 task 8 adds named pane surfaces (`activation: "pane"`): the wire
    snapshot carries them in `PackageUiSnapshot.surfaces` (same shape as the
    empty-tab landing); the empty-tab election ignores them. Launch/close is a
    package command (`coding-agent.profile` / `coding-agent.close`) the
    dispatcher answers with one `ShellClientCommandRequest` the client
    re-parses deny-by-default (same projection as `settings.open`);
    launching (`coding-agent.profile`) also sets the book's active
    profile to `coding` server-side (`select_picker(Agent)`), and the pane's
    mount STATE applies the same rule (`tab_state_snapshot` fills an *empty*
    profile with the coding surface profile) — a pane restored by the layout,
    reached through the view switcher, or rendered by the empty-tab landing
    never dispatched the launch command, and its session would otherwise run
    the daemon-level `Chat` default: no coding tools, no MCP servers. A
    deliberate profile selection is never overwritten; the
    per-tab runtime pins the surface to its hosting pane (`agentSurfaceOpen`
    + `agentSurfacePaneId`) and auto-closes when the contribution disappears.
    The bundled `@clay/coding-agent` surface is provenance-exact host
    rendering (`frontend/src/coding-agent/CodingAgentPanel.tsx`, the
    SettingsPanel precedent). Plan 124 places its composer, agent controls,
    approval strip, and session foot in the Clay-owned persistent
    `frontend/src/shell/AgentLane.tsx`; the panel retains transcript/state-strip
    and inspector presentation. The lane's `/` palette is `CommandPalette`, a
    host shell surface rather than a package UI contribution. Every dynamic
    agent value rides the one AG-UI stream, and third-party `pane` surfaces
    render through the unchanged generic SDUI renderer. Host-owned
    `agent.submit`/`agent.cancel`/`agent.steer` controls bypass static-tree
    source validation and are authorized by the bound tab session; they are
    not arbitrary package actions. Two catalog gaps closed generically:
    `tabList` kind (React Aria Tabs, widget-local selection) and `textInput`
    `multiline` (native textarea, auto-grow, Enter submits / Shift+Enter
    newline).

## Plan 124/125 ownership boundary

The persistent agent lane and the composer's `/` palette are Clay-owned shell
chrome, not `PackageWorkspace` slots. A package reaches the palette only by
registering a validated command; it cannot declare the lane, place its
composer, paint the veil, request the plan-125 halo, or open/drive a transient
menu session — Plan 125 moved the picker stages into the same sheet, and their
command ids are listed there but not package-executable (`agent.clientOpen*` are
built-in server-first ids).
The `secret` stage is Clay-owned end to end: its shielded field lives in the
sheet, the composer is disabled while it is open, and the value reaches the
server only through the credential op. The server catalogue may include package
command metadata, but query/filter/activation still use the server-owned menu
and command-executor paths. See [React Command Centre and Desktop
Workflows](react-command-centre-desktop-workflows.md) and
[Creating Clay Packages](../../reference/packages/creating-packages.md).

## Code Example

```tsx
<PackageWorkspace
  sdui={runtime.ui.sdui}
  packageUi={runtime.ui.packageUi}
  send={activePane.session.request}
  editorSlot={<PaneTree runtime={runtime} />}
/>
```

Packages still author only manifest data:

```json
{
  "kind": "button",
  "id": "example.refresh",
  "label": "Refresh",
  "action": { "commandId": "example.refresh" },
  "style": { "variant": "primary" }
}
```

## Primitive Coverage

- **SDUI tree/update**: `src/protocol/sdui.rs`; stable numeric node IDs,
  explicit base/new versions, inert action intents, existing snapshot/update
  budgets. React projection performs no server validation or package work.
- **Package UI snapshot**: `src/protocol/runtime.rs`; generation-scoped fixed
  panels, overlays, components, input routes, empty-tab content, and (plan
  108) named `pane` surfaces. Protocol version 26 reflects the changed
  archived shape.
- **Panel/component/overlay/input/pane-content contributions**:
  `src/server/ui.rs`; existing manifest and `clay:ui` registration validators.
  No new facade, permission, manifest key, or package setup step.
- **Trust domains**: `src/packages/bundled.rs`, `src/server/js_runtime`, and
  `src/server/cross_domain.rs`; exact inventory/provenance only, typed bounded
  cross-domain values, user-approved mutation, third-party replacement remains
  third-party.
- **Action execution**: frontend emits only `ClientMessage::SduiAction`; Tauri
  stamps client identity and the server rechecks command, provenance,
  permission, context, argument budget, and freshness.
- **Hot path**: package evaluation, JSON parsing, schema validation, and theme
  resolution occur before snapshot install. React render/input reads cached
  objects and CSS variables. CodeMirror local typing still precedes IPC.

Future packages reuse these primitives by declaring existing component trees
and actions in `package.json`. Do not add package-specific React branches,
Rust UI kinds, imperative load registration, or client modules.

## Invariants and Constraints

- `package.json` `clay.contributions` is registration source; load entries stay
  execute-only and one-line `loadPackage` remains the user default.
- Trusted classification never follows `@clay/*` naming or user promotion.
- Same-realm arbitrary third-party React is denied. Isolated custom surfaces
  require a separate decision and implementation.
- Unknown kinds, raw styles/colors, scripts, callbacks, executable fields,
  unsafe anchors, oversized payloads, stale generations, and stale UI updates
  fail closed before interaction.
- React keys are stable IDs. A kind change may remount that node; a property
  update must not remount surviving siblings.
- Package provenance is visible text, not authority. Third-party packages are
  disclosed as one shared runtime trust cohort.
- Package renderer is code-split. Production baseline is 164.3 kB startup
  shell, 27.8 kB package renderer, and 299.3 kB total gzip.

## Tests

- `frontend/src/sdui/state.test.ts`: targeted replacement, surviving identity,
  stale update denial.
- `frontend/src/sdui/renderer.test.tsx`: SDUI layout/editor slot and typed action.
- `frontend/src/sdui/registry.test.tsx`: stable local state, input action values,
  provenance, and inert hostile text.
- `frontend/src/shell/workspace-controller.test.ts`: routed atomic generation
  install and post-install acknowledgement.
- `src/server/ui.rs`: wire snapshot completeness and trusted label stamping.
- `src-tauri/src/bridge/dto.rs`: component parsing and raw-theme exclusion.
- Existing package graph/loading/cross-domain/conformance suites: adoption,
  revoke, replacement, rollback, internal-op denial, raw-style denial.

```bash
cargo test --lib server::ui::
cargo test -p clay-desktop --all-targets
cargo test --test security package_graph::
cargo test --test security package_loading::
cd frontend && npx vitest run && npm run build && npm run check:budget
```

## Related

- [UI Design System Runtime](ui-design-system-runtime.md) — Plan 101 inert UI design-system recipe schema and fallback engine.
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Icon Pack Runtime](icon-pack-runtime.md) (optional `icon` semantic references on labels/buttons/list items; core keys at runtime, package-owned keys at record time)
- [Third-Party Runtime Authority](third-party-runtime-authority.md)
- [React Shell](react-shell.md)
- [Desktop Typed Bridge](desktop-typed-bridge.md)
- `docs/reference/packages/creating-packages.md`
- `docs/development/ui-design-system-recipe-matrix.md`
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
- `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
- `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`
