# Command Centre Surface (Centered Origin)

## Scope

The built-in Command Centre (and the Path Browser, which shares the same
session) is a server-owned transient menu rendered as a centered palette. This
page covers the centered origin, the current React projection and its
composition, and the authority boundary. Command execution, filesystem
browsing, fuzzy matching, keybindings and package APIs are documented on their
own pages ([Control Center](control-center.md),
[Transient Menu Round Trip](transient-menu-round-trip.md),
[Path Browser](path-browser.md), [Fuzzy Matching](fuzzy-matching.md)).

## Flow (current: React + Tauri)

1. `ControlCenter::session` and the Path Browser session set
   `TransientMenuOrigin::Centered` (`src/server/control_center.rs`).
2. The origin round-trips through the protocol snapshots
   (`src/shell/transient_menu.rs`, `src/protocol/menu.rs`), so the client never
   infers "centered" from a client-side rule.
3. The per-tab workspace controller (`frontend/src/shell/workspace-controller.ts`)
   keeps the live `menu` snapshot and exposes the typed intents — `menuQuery`
   (`menuQueryUpdate`), `menuBackspace`, `menuMove`, `menuActivate`
   (`secondary` for `Alt+Enter`), `menuCancel`. The server remains the owner of
   the query, the selection and the result set.
4. `frontend/src/command-centre/CommandCentre.tsx` renders that snapshot inside
   `ClayModal … flush` (the sheet paints its own surface and the modal supplies
   the scrim + focus containment). React Aria provides modal focus containment,
   Escape dismissal, focus restoration, and labelled
   textbox/listbox/option semantics; the result count is an
   `<output aria-live="polite">` with the bounded `0 results` / `1 result` /
   `{n} results` grammar.
5. Keys handled locally: `ArrowDown`/`ArrowUp` (selection), `Enter`
   (`Alt+Enter` = secondary activation), `Backspace` (semantic backspace
   intent, so the server owns filtering semantics). Every other key is left to
   the controlled input, which sends full query updates.

## Composition (plan 118)

The palette is the approved `command-centre.html` composition:

- **head** — a decorative search icon, the session prompt, and the controlled
  input;
- **results** — rows from the server catalogue (label, detail, selected state),
  with an empty state (`role="status"`) that names the session and shows the
  hint for a query with no match;
- **foot** — key hints and the polite result count, with the count pushed to the
  trailing edge.

Two deliberate omissions: the approved prototype's scope segmented control and
its group headers are **not** rendered, because `TransientMenuSnapshotDto`
carries neither a scope axis nor grouping — adding them would mean fabricating
data the server does not own. The palette therefore shows one flat, bounded
result list, and the prototype's `seg` recipe family is consumed elsewhere (the
tab view switcher, once that ships).

Appearance is entirely recipe-driven (`command-centre.module.css`): the sheet
uses `commandCentre.default.root.rest` (border, radius, fill, text, shadow,
transition) and the menu variant uses `popover.default.root.rest`; the scrim
colour comes from the theme's `surface.scrim` role through `ClayModal`. No
literal radius, border, shadow or duration exists in the component CSS — the
`no-literals` rule and the one-ring rule are asserted by
`frontend/src/test/surface-adoption.test.tsx` and the component-level
conformance audit (`design-artifacts/tools/verify-component-conformance.mjs`).

## Authority boundary

The snapshots are inert display data. `PackageOverlayAnchor::Centered` is
Clay-internal: `parse("centered")` falls back to the normal package anchor, so
the package-facing `OverlayAnchor` surface stays
`working-area | active-pane | main | pointer`. Packages cannot request the
centered layer, open or drive a server menu session, intercept its input, or
obtain browse authority. Path activation continues through the server-owned
Path Browser session and its existing grant conversion rules (one
`SingleFile`/`Directory` grant per explicit activation).

## Tests

- `frontend/src/command-centre/CommandCentre.test.tsx`: palette rendering,
  intent dispatch, count grammar, empty state, keyboard handling.
- `src/server/control_center.rs`: catalogue projection, fuzzy filtering,
  selected-command activation, shell-client allowlist, item-detail provenance,
  and the "no catalogue rebuild for a query update" invariant.
- `src/server/menu_sessions.rs`: session ids, replace/query/selection/cancel
  lifecycle, typed activation, stale-generation rejection.
- `src/shell/fuzzy.rs`: scorer behaviour shared by the palette, the file
  browser and the Path Browser.
- `frontend/src/shell/{shell-chords,workspace-controller}.test.ts*`: chord
  wiring and per-tab menu lifecycle/drop on close/reload.
- Visual evidence: `design-artifacts/tools/capture-overlays.mjs` →
  `design-artifacts/screenshots/quiet-instrument-overlays/` asserts the palette
  sheet's one ring, bounded results and key-hint foot across shipped themes.

## Historical note (removed native renderer)

Phase 24.4 originally implemented the centered surface in the native Masonry
client: a `PackageOverlayHost::new_centered()` mounted as a window-layer
`WidgetId`, geometry clamped from `dimension.overlay.centered.width`, retained
AccessKit `Role::Dialog` / `Role::Menu` / `Role::Status` nodes, and paint
through `paint_scrim` + `paint_tooltip_shell`. The Phase 12 Tauri/React cutover
deleted that renderer (`src/masonry_editor.rs`, `src/masonry_package_region.rs`,
`src/masonry_pane_document.rs` and its tests). The behaviour it guaranteed —
single window-scoped scrim, one dialog node, one polite count node, input
containment, idempotent removal on close/tab-change/disconnect — is now
provided by `ClayModal` + React Aria and the per-tab controller. Masonry-era
records remain in `docs/wiki/archive/` (see
[Masonry Shell Runtime](../archive/masonry-shell.md)).

## Related pages

- [Control Center](control-center.md) — the server-owned command catalogue
- [Transient Menu Session](transient-menu-session.md) — the shared bounded state model
- [Transient Menu Round Trip](transient-menu-round-trip.md) — wire lifecycle and intents
- [Path Browser](path-browser.md) — the second session kind sharing this surface
- [React Shell, Component Registry, and Theme Runtime](react-shell.md) — `ClayModal` and the recipe consumption rule
- [Design Artifact Gate](design-artifact-gate.md) — the approved `command-centre.html` reference
