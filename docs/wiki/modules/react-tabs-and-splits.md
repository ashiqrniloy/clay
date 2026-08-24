# React Tabs, Splits, and Layout Persistence

## Source

- `frontend/src/shell/{split-tree,tab-store,persist,workspace-controller}.ts`
- `frontend/src/shell/{PaneTree,WorkspacePanes}.tsx`
- `frontend/src/app/layout/{tab-bar,app-shell}.tsx`
- `src-tauri/src/bridge/{session,layout}.rs`
- `src/shell/layout_persist.rs`
- `frontend/src/shell/*.test.ts`

## Overview

Plan 097 Phase 6 ports the native window layout: each tab is an independent
Clay client connection; each tab owns a bounded pane split tree; persistence
reuses `layout.json` v2 through the existing Rust parser.

## Responsibilities

- Project the server `TabRegistry` into the shell tab strip.
- Keep split/focus/resize/move/close client-local (`MAX_PANES_PER_TAB = 4`).
- Host one CodeMirror view per visible pane of the active tab.
- Persist/restore validated v2 window state. Hostile files degrade.

Non-responsibility: decorations, completion, language intelligence (Phase 7),
package SDUI slots (Phase 7), native Masonry chrome (delete after parity).

## How It Works

1. `session_bootstrap` still opens the first connection. Extra tabs call
   `tab_open` → `connect_with_workspace_root` and live in `BridgeState`'s
   session map. `session_request` stamps the target tab's `client_id`.
2. Events leave the bridge as `Routed { clientId, tabId, event }` so a
   document open in tab A cannot land on tab B's pane sessions.
3. `split-tree.ts` mirrors `src/shell/layout.rs`: equal split, close-merges
   sibling, equal-area comb, reading-order move, 0.05–0.95 clamp.
4. `react-resizable-panels` draws nested groups; keyboard chords in
   `use-shell-chords.ts` call the same tree ops.
5. Dirty tab close is a Clay modal (Save all / Discard / Cancel). Last tab
   cannot close.
6. `layout_save` / `layout_load` run `parse_window_state` on the Rust side.

```ts
workspace.split("horizontal"); // Ctrl+\
workspace.openPath("notes.md"); // focuses the owner pane if already open
```

## Invariants and Constraints

- Tabs are separate clients. Leases, roots, and queues do not cross.
- Frontend cannot mint `client_id`; the bridge overwrites it.
- Duplicate path in a tab focuses the existing pane.
- Inactive tabs keep sessions; only the active tab mounts editor views.
- Persistence never panics; corrupt input is `None`.

## Tests

```bash
cd frontend && npx vitest run src/shell
cargo test -p clay-desktop --all-targets
```

## Related

- [Tabs and Independent Client Views](tabs-and-clients.md)
- [Pane Document Views](pane-document-views.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [React Shell](react-shell.md)
