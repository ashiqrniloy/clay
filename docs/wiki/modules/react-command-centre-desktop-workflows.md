# React Command Centre and Desktop Workflows

## Source

- `frontend/src/command-centre/CommandCentre.tsx`
- `frontend/src/settings/SettingsPanel.tsx`
- `frontend/src/shell/workspace-controller.ts`
- `src-tauri/src/commands.rs`
- `src-tauri/src/bridge/session.rs`
- `src/server/connection/runtime.rs`
- `src/server/command_execution.rs`

## Overview

Plan 097 Phase 9 ports Clay's server-owned Command Centre, Path Browser,
settings, diagnostics, native file/folder selection, and client workflow
commands to the Tauri/React client. Existing server sessions and configuration
runtime remain authoritative. React renders bounded inert snapshots and sends
opaque session intents; it does not filter command catalogues, resolve paths,
or execute package code.

## Responsibilities

- `CommandCentre` renders command, path, and picker snapshots through one React
  Aria modal/list projection.
- `workspace-controller` keeps one menu, diagnostic, and settings visibility
  state per tab connection and deny-by-default dispatches approved client UI
  commands.
- `SettingsPanel` is a compiled first-party presentation module for the exact
  `@clay/settings` contribution. It emits only versioned `settings.*` SDUI
  intents.
- Tauri commands run existing Clay native dialog backends off the render thread
  and hand selected paths directly to `ClientEditQueue`; paths never enter the
  DOM.
- Server Rust retains command catalogue, path listing, grant conversion,
  configuration reload, preference validation, package provenance, and runtime
  generation authority.

## How It Works

1. A manifest chord or command intent opens `ServerMenuSessions`; the server
   pushes `TransientMenuSnapshotData` with an opaque high-bit session ID.
2. The bridge forwards the validated event with its client/tab identity.
   `workspace-controller` installs it only on that tab.
3. React renders prompt, query, bounded rows, server-selected row, empty status,
   and polite result count. Query, semantic backspace, relative selection,
   primary/secondary activation, and cancel go back as existing menu messages.
4. Path Browser reuses the same component. `Alt+Enter` sends secondary
   activation; the server resolves its installed canonical entry and converts
   the user browse action into a directory grant.
5. Client UI activations return `ShellClientCommandRequest`. The controller
   accepts a closed command set for panes, tabs, dialogs, editor commands, and
   settings visibility; unknown sibling IDs do nothing.
6. Native dialog commands call Clay's existing portal/Windows/macOS backend in
   `spawn_blocking`. A selected path is submitted through the queue's single-use
   selected-path capability and server canonicalization. Cancel is a no-op.
7. `settings.open`/`settings.close` receive a server-approved client projection.
   Theme and appearance choices persist through the existing preference/reload
   path. Typography sends one complete JSON transaction, validated before the
   atomic preference write and validated again during reload.
8. Live `RuntimeDiagnostic` events and runtime-snapshot diagnostics update the
   shell footer. Failed reloads preserve the previous generation.

## Code Examples

```ts
workspace.menuActivate(true); // Path Browser: open selected directory as workspace
workspace.menuCancel();       // opaque session id remains server-owned
```

```text
native picker -> ClientEditQueue selected-path capability -> server canonicalize
-> SingleFile or Directory grant -> document/tab snapshot
```

## Invariants and Constraints

- One menu per connection; tab switch, reload, replacement, and disconnect
  remove it.
- Snapshot query/selection is server truth. React keeps no parallel command
  catalogue or fuzzy matcher.
- Menu and file-browser collections remain capped at 256 rows. Native scrolling
  is retained until profiling demonstrates a virtualization need.
- Dialog paths are never returned to package code or rendered as DOM data.
- Clipboard reads/writes happen only for explicit user-routed editor commands;
  package and configuration runtimes receive no clipboard API.
- Typography requires all three profiles and all seven hierarchy ratios. Any
  invalid field rejects the whole transaction.
- No package gets Tauri commands, native dialogs, centered overlay authority,
  raw CSS, or direct configuration mutation.

## Tests

- `frontend/src/command-centre/CommandCentre.test.tsx`: modal semantics and all
  menu intents.
- `frontend/src/settings/SettingsPanel.test.tsx`: complete typography payload,
  invalid-bound denial, and secret-free DOM.
- `frontend/src/shell/workspace-controller.test.ts`: per-tab menu lifecycle,
  client-command allowlist, and dialog routing.
- `src/server/connection/runtime.rs`: exact manifest client-UI projection and
  settings persistence/reload.
- `src/server/command_execution.rs`: settings theme/appearance/typography
  validation.
- `src-tauri/tests/config_security.rs`: no broad filesystem/shell/network plugin
  capabilities.

```bash
cargo test --lib menu_sessions -- --test-threads=1
cargo test --lib settings_ -- --test-threads=1
cargo test -p clay-desktop --all-targets
npm --prefix frontend test
```

## Related

- [Transient Menu Round Trip](transient-menu-round-trip.md)
- [Path Browser](path-browser.md)
- [Configuration Runtime](configuration-runtime.md)
- [Client File Dialog](client-file-dialog.md)
- [React SDUI and Package UI](react-sdui-package-ui.md)
