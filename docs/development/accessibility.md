# Accessibility (Phase 22.6 and 24.4)

Phase 22.6 (plan 077 tasks 3–4) establishes the accessibility contract for
the Clay window model: accessibility roles and names for the tab bar, tab
cards, and panes, plus polite screen-reader announcements for tab and split
changes. This page is the contract reference; the structural tests in
`src/masonry_shell/mod.rs` (via `WindowEvent::EnableAccessTree` + `RenderRoot`
tree updates) enforce the exact tree shape described here.

## Roles and names

| Surface | Role | Name | Notes |
| --- | --- | --- | --- |
| Shell root | `Group` | `Clay working area shell. Active pane {N}.` | Stable from prior phases. |
| Tab bar (2+ cards) | `TabList` | `Workspace tabs` | Virtual node built from `tab_bar_geometry`; present only when the window has at least two tab cards. A single-tab window keeps today's tree shape (no `TabList` noise). |
| Tab card | `Tab` | Sanitized workspace display name (basename) | One per card; `selected` set on the active card. Cards are painted chrome (`tab_card_chrome`, `src/shell/primitives.rs`), not widgets, so these are informational virtual nodes — the pane hosts remain the focusable units. |
| Pane host | `Pane` | `Empty pane {N} of {M}` / `Pane {N} of {M}: editor` / `Pane {N} of {M}: {display name}` (`: document` when the name is not yet known) | Numbered 1-based within the active tab's pane tree; `M` is the total pane count. Display names flow driver → shell → host via `ClientConnectionEvent::metadata_path()` and are sanitized at the accessibility boundary. |
| Announcement node | `Status` | Current announcement label (empty until the first announcement) | Always present as the last child; `live` = `polite`. |

Tree order is `TabList` (when present), then the active tab's pane hosts in
pane order. Hosts of inactive tabs are **stashed** (`LayoutCtx::set_stashed`
in `ClayShellWidget::layout`): Masonry skips stashed subtrees in paint and
in the accessibility walk, so an inactive tab's panes are never announced or
visited, while the widgets stay registered in `children_ids` (which
`register_children` requires) for reconnect continuity. Unstashing on tab
activation requests layout and accessibility automatically.

### Focus model

Pane hosts and document views are the focusable units (they keep
`accepts_focus`). Tab cards are informational: keyboard tab switching goes
through the Phase 22.4 tab commands (`TabNext`/`TabPrev`/`TabActivate`),
not per-card widget focus. Focus traversal therefore matches the keyboard
model: TabList nodes precede the active pane hosts, and no new focus
targets were introduced.

### Focus removal and event ingress (Plan 089)

Focus repair is tied to child removal, not a per-frame tree scan. The pinned
Masonry core patch invalidates `focused_widget`, `next_focused_widget`, and
fallback while `MutateCtx::remove_child` detaches a subtree; the normal focus
rewrite then rebuilds the path and clears ancestor focus flags. Clay's shell
moves focus to the surviving pane before same-tab removal; active-tab removal
clears focus while inactive hosts are still stashed, then the driver focuses
the newly active pane after its layout pass. A synthetic top-level `Window`
node is not a Masonry widget, so `RenderRoot::handle_access_event` ignores
actions addressed to that node instead of dispatching an invalid `WidgetId`.
Editor/document focus remains unchanged for valid Entry targets.

The local `masonry_core` 0.4.0 patch is based on the pinned crate source and
changes only this removal/event-ingress seam plus the matching `MutateCtx`
focus transfer methods; remove it when an upstream Masonry release carries
the guard.

## Editable editor text (P1 follow-up, Phase 28.7)

The active editor is a Clay-owned `Role::MultilineTextInput`, not a package
`textInput` component. `EditorWidget` and direct `PaneDocumentView` hosts use
one shared accessibility builder:

- The root keeps its sanitized status/document label and exposes the current
  bounded visible text window as `value`.
- A stable owner-derived `Role::TextRun` child carries UTF-8 character
  lengths. Its node ID uses the editor owner's `TEXT_RUN` virtual slot, so
  AT-SPI text ranges and selections survive redraws without `WidgetId::next()`
  churn.
- The root publishes `TextSelection`, `SetTextSelection`,
  `ReplaceSelectedText`, and (for editable leases) `SetValue`. Read-only
  observers retain text/selection semantics but set `readOnly` and do not
  advertise mutation actions.
- Selection offsets are converted between AT-SPI character indexes and Clay
  UTF-8 byte offsets only when the visible window is contiguous. When a
  collapsed fold makes that mapping non-contiguous, Clay omits selection
  metadata rather than applying an unsafe offset.

AccessKit 0.21.1's pinned Unix adapter exposes writable Entry state but did not
register AT-SPI's standard `org.a11y.atspi.EditableText` interface. Clay keeps
exact-version local patches in `vendor/accesskit_atspi_common` and
`vendor/accesskit_unix`: writable text-input roles advertise the interface,
and `SetTextContents`/`InsertText`/`DeleteText` translate to bounded AccessKit
`SetValue`/`SetTextSelection`/`ReplaceSelectedText` actions. Clipboard-specific
EditableText methods remain unsupported; Clay's existing explicit keyboard
copy/cut/paste path remains client-owned.

The editor handles those actions in `PaneDocumentView::handle_access_event`.
Text replacement uses the existing local paste/edit-history path, so newline
and Unicode input follow ordinary editor validation, optimistic queueing, undo,
and accessibility invalidation. No package JavaScript, filesystem, clipboard
read, or IPC wait is added to paint/layout/text-event paths.

Verification:

```bash
cargo test --lib masonry_editor::tests::editor_accessibility_exposes_editable_text_value_selection_and_stable_run -- --exact --test-threads=1
CLAY_LIVE_A11Y_SMOKE=1 cargo test --test security live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
```

The live probe checks `Entry` plus the actual `EditableText` interface, not
only the editable state flag. The ignored keyboard mutation portion remains
host-dependent when no development keyboard backend is available.

## Virtual node identity (plan 086 task 3)

All synthetic nodes (TabList/Tab, live announcement, status lines, menu
items/status) use one deterministic derivation,
`editor::accessibility::virtual_a11y_node_id(owner, slot)`:

- Layout: prefix `0xD000_0000_0000_0000` | `(owner.to_raw() & 0x0000_7FFF_FFFF_FFFF) << 9` | slot.
- The owner is the retained widget that attaches the node, so identity is
  stable across accessibility passes and dies with its owner; the prefix is
  unreachable by real widget IDs (masonry's counter starts at 1), so
  synthetic and widget nodes never collide.
- Slot namespaces per owner are defined in `virtual_a11y_slots` (shell:
  TabList = 1, announcement = 2, Tab(i) = 3 + client_id; editor/pane
  document: status = 1; region: status = 1, item(i) = 2 + i). Slots fit the
  9-bit space under the existing caps (tabs ≤ 64 connections, menu ≤ 256
  items); the helper asserts the bound.
- Tab slots derive from the connection id, so a card keeps its node id
  across registry reorders and selection changes.

## Consumer-validation contract (plan 086 task 3)

The tree is validated exactly as a desktop AT sees it: unit tests feed the
real `TreeUpdate` through `accesskit_consumer::Tree` (dev-dependency,
same version as the live adapter) on every mutation. The invariant:
**every node the walk emits must be attached — a child of an updated node
or already in the consumer tree.** Three classes of mismatch caused the
startup crash and are now structural:

1. A parent's `accessibility()` children must cover every child the walk
   emits. The editor always lists its region child (even without a
   sidebar), the region always attaches its reconciled pod alongside the
   semantic menu nodes, and the shell lists the active tab's hosts plus
   its synthetic nodes.
2. Subtrees that must not be reachable are stashed, not listed (inactive
   tab hosts, pending orphans).
3. Every emitted node keeps a stable id; churning ids would re-register
   live regions and drop Tab selection on every pass.

Consumer tests cover: initial single- and multi-tab trees, unchanged
redraws, announcement, tab add/reorder/remove, selected-tab, pane
name/status updates, and menu query/selection/close — all without panic,
with stale and inactive nodes absent from the reachable tree.

## Centered Command Centre dialog (Phase 24.4)

Command and path sessions with `TransientMenuOrigin::Centered` are exposed
through the retained window-level overlay layer as one modal `Dialog`, named
from the bounded/sanitized menu prompt. Its child menu reports `MenuItem`
children with selected state, plus one stable `Status` child with
`Live::Polite`. That status uses exact count grammar: `0 results`, `1 result`,
or `{n} results`.

The dialog does not move Masonry focus away from the originating pane. The
server-owned pane route remains the keyboard entry point, and every key,
clipboard paste, or IME event is consumed while the modal is active; supported
keys enqueue the existing bounded menu intents. Scrim pointer-down events are
swallowed and restore/retain originating-pane focus, so they cannot mutate the
editor. Closing removes the root layer and leaves focus on the originating
pane.

Menu and item/status virtual node IDs are derived from the retained region ID
via the shared `virtual_a11y_node_id` policy, so query snapshots and
selection-only snapshots reuse identity. Selection-only updates with
unchanged result count keep the same status label; a changed count updates
that same node. The reconciled pod stays attached while the menu is open
(consumer-validation contract above). Construction is bounded by the
existing 256-item menu cap and runs only during accessibility passes.

## Announcements

One shell-owned `Status` node with `Live::Polite` announces tab and split
changes. The label is replaced by a shared O(1) builder
(`compose_announcement`) exactly once per user action:

| Action | Announcement |
| --- | --- |
| Tab activate | `Switched to tab {position}: {name}` |
| Tab create | `Opened tab {position}: {name}` |
| Tab close | `Closed tab {position}: {name}; {n} tabs open` |
| Split vertical | `Split pane vertically` |
| Split horizontal | `Split pane horizontally` |
| Add equal pane | `Added pane` |
| Close pane | `Closed pane; {n} panes remain` |
| Move pane forward/back | `Moved pane forward` / `Moved pane backward` |

Announcements fire only from user-initiated paths (tab activation after a
real switch, the new-tab folder dialog, close, and pane commands that
actually change the tree). No-op operations, pure focus moves, repaints,
and startup/restore flows stay silent. The tree is invalidated only when
the announcement changes (`request_accessibility_update` inside
`announce`).

Announcements are **unconditional behavior, not configuration**: there is
no `init.js` key or Clay JS API to enable/disable or tune them (plan 077
task 10). If a real user need for a verbosity/on-off setting appears, it
must ship as a documented Clay JS API with `custom_properties` coverage,
not an undocumented key.

### Budgets

- Names are sanitized through `sanitize_document_display_name`
  (`src/editor/accessibility.rs`): basename only, control characters and
  path separators filtered, capped at
  `ACCESSIBILITY_DISPLAY_NAME_MAX_CHARS` (64). Absolute host paths never
  reach labels.
- Announcement strings and transient-menu item labels are capped at
  `TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS` (256), defined in
  `src/perf/budgets.rs`. Item labels pass through
  `compose_menu_item_accessibility_label` at `MenuA11y` construction: controls
  and path separators are removed, empty/invalid labels fall back to the
  display label or `Menu item`, and the selected suffix stays inside the same
  256-character ceiling. Display/action data is unchanged.

## Verification

- Structural: the shell unit tests in `src/masonry_shell/mod.rs` enable the
  access tree (`WindowEvent::EnableAccessTree`), redraw, and assert the
  `TreeUpdate`: TabList presence/absence, `selected` on the active card,
  per-tab host filtering, and pane labels including the numbered `of M`
  form. Announcement tests assert the exact label strings and that focus
  moves/repaints do not re-announce.
- Consumer: the `consumer_accepts_*` tests run every mutation through
  `accesskit_consumer::Tree` (see contract above) — the desktop adapter's
  exact validation, catching unattached-node failures structural tests
  cannot. Focus coverage includes dirty-close rejection/discard, clean
  focused-pane removal, active-tab removal, and clear/survivor outcomes.
- Event ingress: `top_level_frame_focus_action_does_not_dispatch_to_a_widget`
  verifies a synthetic frame Focus action is ignored while valid editor focus
  remains intact.
- Real assistive technology: plan 086 added a live check on the Linux
  desktop with the AT-SPI bus active — the app must stay alive through
  startup and tab/pane/menu/announcement updates and expose a queryable
  tree (`clay` application, shell `Group`, pane `Pane`, editor `Entry`,
  status `StatusBar`).

## Known ceilings (`ponytail:`)

- An AT may skip an announcement whose label equals the previous one
  (identical consecutive actions); upgrade path is a two-phase
  clear-then-set label update if real-AT testing shows dropped
  announcements.
- Tab cards are informational nodes, not focusable widgets; if AT focus
  handling proves insufficient, promote cards to real Masonry widgets with
  `Role::Tab` focus semantics.
