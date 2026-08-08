# Accessibility (Phase 22.6)

Phase 22.6 (plan 077 tasks 3–4) establishes the accessibility contract for
the Clay window model: accessibility roles and names for the tab bar, tab
cards, and panes, plus polite screen-reader announcements for tab and split
changes. This page is the contract reference; the structural tests in
`src/masonry_shell.rs` (via `WindowEvent::EnableAccessTree` + `RenderRoot`
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
pane order. Hosts of inactive tabs stay in the widget tree (zero-size) but
are unreachable from the accessibility root — an inactive tab's panes are
never announced or visited.

### Focus model

Pane hosts and document views are the focusable units (they keep
`accepts_focus`). Tab cards are informational: keyboard tab switching goes
through the Phase 22.4 tab commands (`TabNext`/`TabPrev`/`TabActivate`),
not per-card widget focus. Focus traversal therefore matches the keyboard
model: TabList nodes precede the active pane hosts, and no new focus
targets were introduced.

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
- Announcement strings are capped at
  `TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS` (256), defined in
  `src/perf/budgets.rs` — the same constant the transient menu uses.

## Verification

- Structural: the shell unit tests in `src/masonry_shell.rs` enable the
  access tree (`WindowEvent::EnableAccessTree`), redraw, and assert the
  `TreeUpdate`: TabList presence/absence, `selected` on the active card,
  per-tab host filtering, and pane labels including the numbered `of M`
  form. Announcement tests assert the exact label strings and that focus
  moves/repaints do not re-announce.
- Real assistive technology: out of scope for 22.6 (known ceiling). Orca
  and screen-reader verification of live-region behavior is deferred; the
  announcement node is a single persistent `Live::Polite` node so AT
  registration is stable across tree rebuilds.

## Known ceilings (`ponytail:`)

- An AT may skip an announcement whose label equals the previous one
  (identical consecutive actions); upgrade path is a two-phase
  clear-then-set label update if real-AT testing shows dropped
  announcements.
- Tab cards are informational nodes, not focusable widgets; if AT focus
  handling proves insufficient, promote cards to real Masonry widgets with
  `Role::Tab` focus semantics.
