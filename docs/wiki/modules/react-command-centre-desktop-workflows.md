# React Command Centre and Desktop Workflows

## Source

- `frontend/src/shell/{WorkspacePanes,AgentLane,workspace-controller}.tsx`
- `frontend/src/command-centre/{CommandPalette.tsx,CommandPalette.test.tsx}` (`CommandCentre.tsx` was deleted in plan 125)
- `frontend/src/coding-agent/{Composer,AgentView}.tsx`
- `frontend/src/components/text-field.tsx`
- `frontend/src/settings/SettingsPanel.tsx`
- `frontend/src/shell/{layout-state,use-shell-chords}.ts`
- `frontend/src/shell/{workspace-panes,command-centre}.module.css`
- `src/server/{control_center,menu_sessions,command_execution}.rs`
- `src/protocol/menu.rs` / `src/protocol/mod.rs`
- `src-tauri/src/{commands.rs,bridge/session.rs}`

## Overview

Plan 097's React command workflows retain server authority for command
catalogues, menu sessions, path resolution, grants, settings validation, and
runtime generations. Plan 124 changes only the host projection: the command
and path sessions are now a `/` palette owned by the agent lane's composer.
Plan 125 finished the consolidation — the centered `CommandCentre` projection is
gone, and the pickers that used to stay centered are palette stages behind the
server's `mode` (`catalogue`, `path`, `picker`, `secret`, `url`, `oauth`).

React renders bounded inert snapshots and sends opaque menu intents. It does
not filter commands, resolve paths, execute package code, or decide grants.

## Responsibilities

- `WorkspacePanes` owns the per-tab `ComposerPalette` callbacks and selects the
  `commandPalette` session from the tab runtime.
- `AgentLane` mounts the composer for every tab and supplies the query field;
  `CommandPalette` is rendered inside that field's `menu` slot.
- `CommandPalette` renders every server session — the catalogue, path mode, and
  each picker stage — as the one field-child sheet, choosing its stage
  presentation, foot verb, and shielded field from the session's `mode`.
  `CommandCentre.tsx` no longer exists.
- `workspace-controller` installs menu snapshots on the owning tab and keeps
  client UI dispatch deny-by-default.
- `SettingsPanel` remains the compiled first-party presentation for the exact
  `@clay/settings` contribution and emits only versioned `settings.*` intents.
- Tauri dialog commands retain their narrow platform backends and pass selected
  paths to `ClientEditQueue`; path authority stays server-side.

## How It Works

1. A titlebar Control Center trigger, `Ctrl+X Ctrl+O`, or a composer `/` opens
   `controlCenter.open` through the tab's server connection. The server creates
   a generation-stamped `TransientMenuSession` and pushes its bounded snapshot.
2. The bridge and `workspace-controller` install that snapshot only on the
   bound tab. `WorkspacePanes` maps the snapshot and menu intents into the
   `ComposerPalette` interface.
3. `Composer` owns the draft and the focus boundary. On palette-session
   appearance it resets the local scope to `All`, sends the current `/`-led
   filter, and sends later query/scope changes to the server. `Tab` is not a
   row-completion mechanism; Enter submits the selected palette row when one
   exists and otherwise reaches the composer's built-in handling.
4. `CommandPalette` is a display-only results sheet: it echoes the field query,
   renders server rows, scope chips (`All`, `Session`, `Shell`, `Files`),
   per-row binding chips, the bounded empty state, and an `aria-live` result
   count. It owns no input or menu session.
5. `CommandPalette` is a child of `ClayTextField`'s `menu` slot. CSS anchors it
   6px above the field border box at exactly the field width, caps its height
   at `min(52vh, 420px)`, and scrolls its rows internally. It has no separate
   focus ring; the composer field remains the focus boundary.
6. `WorkspacePanes` renders one modal scrim in the working-area grid over the
   panes and inspector rail. The lane stays above it (`z-index` 41 over veil
   40), so the query field and palette remain interactive. Hiding the lane
   removes both its palette and the veil; showing it restores the live session.
7. Path Browser uses the same `CommandPalette` origin and sheet. Its server
   session still owns canonical listing, filtering, navigation, and file/root
   grant conversion; `Alt+Enter` remains secondary workspace activation.
8. Every session renders through `CommandPalette`, whose layout and keys follow
   the session `mode`: picker stages keep the composer as the query source, swap
   their prompt/rows in place with no re-anchor and no new veil pass, ascend on
   `Esc`/`Alt+←`/backspace, and expose `Alt+↵` as the row's declared secondary
   action; the `secret` stage draws its own shielded field and disables the
   composer. Client UI activations continue through the closed
   `ShellClientCommand` parser; unknown IDs do nothing.
9. Native file/folder dialogs, settings persistence, and runtime diagnostics
   keep their existing validated paths. Failed reloads preserve the previous
   runtime generation.

## Code Examples

```ts
// Internal shell wiring: the menu remains server-owned.
workspace.dispatchServerCommand("controlCenter.open");
workspace.menuQuery("/resume", "session");
workspace.menuCancel();
```

```text
composer `/` -> server menu snapshot -> CommandPalette field child
           -> opaque query/selection/activation intent -> server authority
```

## Invariants and Constraints

- There is one server-owned menu session per tab connection; replacement, tab
  switch, reload, cancel, and disconnect close it.
- The server owns query results, selection, scope filtering, path resolution,
  grants, command validation, and generation checks. React owns only draft/focus
  presentation state.
- Rows, query text, scope, binding chips, and result counts are bounded; no
  package JavaScript or registry rebuild runs on query, paint, or layout paths.
- The palette's scrim covers the working-area row, including the inspector
  rail, but never blocks the lane query input; the lane's chrome is opaque, so
  the veil neither shows through it nor dims it (`agent-lane.module.css`).
  Reduced-transparency fallback removes blur through the global design-system
  fallback.
- One sheet per connection: a session never stacks a second surface. A picker
  stage transition reuses the same sheet, and the retired centered origin has no
  renderer left (`TransientMenuOriginData::Centered` maps to the bottom anchor).
- Dialog paths never enter package code. Path activation resolves only from the
  server's installed canonical entries and converts browse authority into the
  existing single-file or directory grant.
- Packages cannot open or drive menu sessions, request the palette veil, obtain
  Tauri dialogs, or receive raw paths. Their commands appear only through the
  validated command-registration path.

## Tests

- `frontend/src/command-centre/CommandPalette.test.tsx`: sheet rows, scopes,
  binding chips, empty state, result-count accessibility, stage modes and their
  foot verbs, the shielded credential stage, and activation.
- `frontend/src/coding-agent/{Composer,Composer.test}.tsx`: `/` query sync,
  `@` mentions, keyboard submission, and Stop behavior.
- `frontend/src/shell/{WorkspacePanes,AgentLane,shell-chords}.test.tsx`:
  palette ownership, veil/lane lifecycle, per-tab controls, and chord routes.
- `frontend/src/shell/workspace-controller.test.ts`: menu lifecycle, client
  command allowlist, tab routing, and persistence callbacks.
- `src/server/{control_center,menu_sessions,connection/tests}.rs`: catalogue,
  scope/filtering, typed activation, lifecycle, stale generation, and shell
  client request tests.
- `src-tauri/tests/config_security.rs`: narrow dialog capability posture.

```bash
cargo test --test protocol
npm --prefix frontend test
```

## Related

- [Control Center](control-center.md) — server catalogue and activation authority
- [Transient Menu Session](transient-menu-session.md) — generic bounded session
- [Transient Menu Round Trip](transient-menu-round-trip.md) — protocol and lifecycle
- [Path Browser](path-browser.md) — composer-owned path mode
- [React Shell](react-shell.md) — grid, lane, veil, and theme ownership
- [Tabs and Independent Client Views](tabs-and-clients.md) — per-tab state/lifetime
- [Configuration Runtime](configuration-runtime.md)
- [Client File Dialog](client-file-dialog.md)
- [React SDUI and Package UI](react-sdui-package-ui.md)
