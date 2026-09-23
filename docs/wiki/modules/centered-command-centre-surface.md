# Retired Centered Command Centre Surface

## Status

Plan 124 re-anchored the command/path sessions to a composer-width bottom sheet;
**Plan 125 finished the retirement**: every transient Clay session — the command
catalogue, the Path Browser, and the Agent Picker's list/provider/auth/credential/
URL/OAuth stages — is now one session rendered by one sheet, so the window-centered
projection, its `CommandCentre.tsx` renderer, its CSS, and
`PackageOverlayAnchor::Centered` are gone. This page is the removal record: it
explains what was there, what replaced it, and what still exists on the wire so
old code and review records stay searchable. Current behavior lives in
[Control Center](control-center.md),
[React Command Centre and Desktop Workflows](react-command-centre-desktop-workflows.md),
and [Transient Menu Round Trip](transient-menu-round-trip.md).

## Historical implementation

Phase 24.4 rendered server-owned command and path sessions through
`TransientMenuOrigin::Centered`, a Masonry `PackageOverlayHost` on the window
layer. The host supplied a modal dialog, menu rows, a bounded polite result
count, scrim containment, focus restoration, and inert session intents. The
server still owned query, selection, activation, path canonicalization, and
permissions; the centered renderer never granted package authority. The native
renderer was removed during the Tauri/React cutover, and its React successor
(`frontend/src/command-centre/CommandCentre.tsx`, a plain-text modal) served the
non-command origins until Plan 125.

## What survives, and why

- **Wire compatibility.** `TransientMenuOriginData::Centered` is still a decoded
  variant so a snapshot from an older daemon (or an old test fixture) does not
  fail to parse. No constructor emits it, and
  `src/shell/package_ui.rs` maps it to `PackageOverlayAnchor::Bottom`, so an old
  origin renders as the bottom sheet rather than resurrecting a modal.
- **Package anchor parsing.** `PackageOverlayAnchor::parse("centered")` keeps
  returning `WorkingArea`: package manifests that spelled it still load, and the
  package contract stays closed to window-level anchors.
- **`TransientMenuOrigin::Centered`** remains a Rust enum variant with the same
  fail-closed mapping (see [Slot-Aware Package UI](slot-aware-package-ui.md)); the
  client no longer has a render branch for it.

## Current replacement

Everything the centered sheet did is now a stage of the composer's `/` palette:

- `WorkspacePanes` selects the `CommandPalette`-origin snapshot from the tab
  runtime and renders the single modal veil as a working-area grid item
  (`grid-area: 1 / 1 / -1 / -1`, z-index 40) over the panes and the inspector
  rail; the lane stays interactive at z-index 41 and its strip is opaque, so the
  veil neither dims it nor shows through it.
- `CommandPalette` is a child of the lane composer's `ClayTextField` `menu` slot:
  exactly the field's width, 6px above it, capped at `min(52vh, 420px)`, no focus
  ring of its own (the composer field is the focus boundary), and the plan-125
  **halo** instead of a drop shadow.
- The server owns the stages. Protocol v32 adds the bounded `mode`
  (`catalogue`, `path`, `picker`, `secret`, `url`, `oauth`) to
  `TransientMenuSnapshotData`; `Esc`/`Alt+←`/backspace ascend a picker flow
  (closing at its flow entry via `MenuEdit { close }`), `Alt+↵` runs a row's
  declared secondary action, and the `secret` stage draws its own shielded field
  while the composer is disabled, so a credential never enters the persisted
  composer draft.
- `Ctrl+X Ctrl+O` (and the titlebar Control Center trigger, and typing `/`)
  opens the catalogue; `Ctrl+X Ctrl+P` toggles the per-tab agent lane;
  `Ctrl+X Ctrl+F` opens path mode. Hiding the lane removes the palette and the
  veil together, which is the fix for the former orphaned-scrim defect.

## Boundaries and tests

The palette remains a display projection. Packages cannot request the palette
sheet or its veil/halo, open or drive menu sessions, intercept their input,
receive raw paths, or obtain Tauri APIs. Commands still pass through the server's
validated registry/executor, and path activation resolves only from installed
canonical entries.

The retirement is pinned by
`src/server/menu_sessions.rs::no_session_constructor_produces_the_retired_centered_origin`,
`src/server/agent_picker.rs::every_picker_stage_is_a_palette_session_with_its_mode`,
and the frontend suites that assert no window sheet renders for a centered
origin (`frontend/src/shell/{shell-chords,WorkspacePanes}.test.tsx`,
`frontend/src/test/overlay-composition.test.ts`). Current coverage is otherwise
`frontend/src/command-centre/CommandPalette.test.tsx`,
`frontend/src/coding-agent/Composer.test.tsx`, and the server menu/control-center
suites. Live evidence: `test-plan/artifacts/124-agent-lane/` (Plan 124) and
`test-plan/artifacts/125-palette/` (Plan 125, including the retired-anchor and
veil/halo measurements).

## Related

- [React Command Centre and Desktop Workflows](react-command-centre-desktop-workflows.md)
- [Control Center](control-center.md)
- [Transient Menu Session](transient-menu-session.md)
- [Transient Menu Round Trip](transient-menu-round-trip.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [React Shell](react-shell.md)
- [Design Artifact Gate](design-artifact-gate.md)
- `plans/124-Persistent-Agent-Lane-and-Slash-Command-Palette.md`,
  `plans/125-Composer-Palette-Stage-Flows-and-Centered-Sheet-Retirement.md`
