---
date: 2026-09-18 19:45
status: approved
decision_about: "Working-area composition: the SDUI sidebar region is host-placed as the shell's own rail, so the agent lane spans the middle pane only"
proposed_by: "user (direction), agent (structure and implementation)"
explicitly_approved_by_user: true
---

# Decision: the shell places the workspace-sidebar region as its own rail

## Decision

The working area is **one grid of three tracks** — files rail · view pane ·
inspector rail — and the **workspace sidebar is a rail of that grid**, not a
column inside the pane: the SDUI tree still *carries* the region (a node whose
`size` names the host token `dimension.sidebar.default`), but the **shell**, not
the tree, places it — the host renders that region in its own rail element
(`hostRailRegionId()` + `SduiRegion` in `frontend/src/sdui/renderer.tsx`, the
`.side` rail in `frontend/src/shell/WorkspacePanes.tsx`) and renders the pane's
tree with the region **omitted**, so the sidebar cannot paint twice. Both rails
span `grid-row: 1 / -1`; the lane occupies the view pane's column and row
(`grid-column: 2`, `grid-row: 2`). A rail keeps the working area's full height
and the lane's hairline stops at its inner edge; below 1000px both rails become
fixed drawers and the lane keeps the pane's full width. The region stays
Clay-owned internal (not a package slot API) — a package's `slot: left` panel is
still pane content.

## Context

Plan 124 made the lane a full-width chrome strip; the user reversed that ("the
lane strip should not span the sidebars … the left and right side bar should
take the full height of the window and agent lane should be only spanning the
middle part leaving the space for left and right side bar"), and plan 125
implemented "the lane is the view pane's own strip" as `grid-column: 1`,
`grid-row: 2` with the rails at `grid-row: 1 / -1`. The fix did not hold: the
app's view pane *contains* the workspace sidebar as a nested column — the SDUI
tree draws it at `dimension.sidebar.default` inside `PackageWorkspace`'s own
`.middle` grid (`auto · minmax(0,1fr) · auto`: left panel, editor, right panel),
which is mounted in the pane's single grid cell. So the lane's column *was* the
sidebar's column: live AX extents showed the lane at `26,946 940x200` (starting
at the window's working-area edge, over the sidebar's 244px) with the sidebar
ending at the lane's top hairline. Plan 125 tracked this as its open deviation
D7 (`test-plan/13-window-splits.md` S47, review log
`code-reviews/screenshots/2026-09-18-plan125-palette/review-log.md`): the shell
promised rail invariance it did not own, because the box that needed to span two
rows was drawn by the package tree.

## Approval

- Proposed by: user (the composition and the rail-invariance requirement);
  agent (the structure — hoisting the region to the shell grid, and the
  host-placed-region mechanism).
- Approved by user: Yes.
- Approval evidence: "Addressing the first open item. The lane strip should not
  span the sidebars. Meaning the left and right side bar should take the full
  height of the window and agent lane should be only spanning the middle part
  leaving the space for left and right side bar" (2026-09-18 18:57), and after
  the implementation and its evidence were presented: "Just create the decision
  log" (2026-09-18 19:41).

## Alternatives Considered

1. **Leave the lane as the working area's full-width strip** (plan 124's
   decision) — directly reverses the user's direction; the rails stop at the
   lane's hairline and the middle pane is not the lane's measure.
2. **Keep the sidebar a pane column and offset the lane** — negative insets or a
   computed rail-width offset on the lane. The lane's box would be a sub-box of
   the pane, the rails would still end at the lane's top, and two grids (the
   pane's and the lane's) would have to agree on the rail's width and its
   toggle — the same class of drift that produced D7, now inside one column.
3. **Let the package's own grid own the lane's row** (the pane keeps a nested
   grid whose first column is the sidebar and whose second row is the lane) —
   same geometry, but it puts Clay shell chrome (the lane) inside the package's
   layout tree, and a package tree would then decide where shell chrome sits.
   Authority inversion for a geometry fix.
4. **Mirror the sidebar column in the shell grid by CSS alone** (e.g. the shell
   reserves a track and the pane translates) — two boxes for one rail: hit
   testing, focus order, and the rail's own toggle would disagree with the
   reserved track.
5. **Move the workspace sidebar out of SDUI into a Clay React component** —
   drops the tree/`publishTree` bridge and the file browser's server-driven rows
   (a server-authority surface) to fix geometry; far larger, and outside the
   issue.
6. **Make the sidebar a fixed overlay at every width** (the ≤1000px treatment,
   always on) — the sidebar would float over content instead of occupying a
   track, so the lane could not end at its edge and the pane would keep full
   width beneath it.

## Rationale and Evidence

- **The shell owns every box; the tree owns content.** `DESIGN.md` §8/§12 makes
  the shell the owner of box geometry and package UI inert, declarative data
  (`docs/reference/packages/creating-packages.md`: contributions are
  "inert and additive-only"). The sidebar region is Clay-owned internal
  (`src/shell/file_browser.rs`), so the tree may name it and the token that
  sizes it (`dimension.sidebar.default`: 244px, 224px at the token's step), but
  the host places it.
- **One grid, one decision.** Rail height and lane containment are the same
  layout fact; expressing both in the working area's grid means a rail's toggle,
  its width, and the lane's edges cannot drift apart. The measured result:
  live 1500×950 — lane `270,946 916x200` (x 270 = the sidebar's inner edge,
  x 1186 = the inspector's left edge), inspector `1186,110 340x1036` running to
  `y 1146`, the lane's own bottom, and the sidebar's content continuing through
  the lane's row (its foot at `y 1113`).
- **The region is carried, not duplicated.** `SduiRenderer` gained an
  `omittedRegions` prop; the pane renders the tree with the sized region removed
  and the shell renders that one region in `.side` (`SduiRegion`), so the
  sidebar exists exactly once in the DOM — pinned by
  `frontend/src/shell/WorkspacePanes.test.tsx` (the rail carries the region; no
  region means a hidden rail; the agent view keeps no rail).
- **Geometry is pinned deterministically, not by eye.** The working area's
  tracks and every item's explicit placement (files rail col 1, pane col 2
  row 1, lane col 2 row 2, veil full area, inspector col 3 — the last two at
  `grid-row: 1 / -1`) are pinned in
  `frontend/src/test/workspace-composition.test.tsx`, which also asserts the
  ≤1000px drawer rules; `design-artifacts/tools/capture-sidebar.mjs` asserts the
  three legs in a browser fixture — 1500 (rail 244 wide, lane 244…1160),
  1024 (rail 224, lane 224…712, inspector 712…1024), 900 (both rails
  `position: fixed` drawers, lane 0…900, zero horizontal overflow) — 3/3 widths.
- **No collateral damage.** The plan-125 palette/lane matrix re-run over the
  same fixtures reports **290/290 checks, 0 product warnings/errors**; frontend
  tests 497/497, typecheck/lint/format clean, component conformance 18/18,
  bundle 176.9/180 kB; Rust protocol 222/222, presentation 62/62, fmt + clippy
  clean. Deviations and host ceilings are recorded in
  `code-reviews/screenshots/2026-09-18-plan126-rails/review-log.md`.

## References

- `decision-logs/2026-09-17-0345-agent-lane-and-slash-command-palette-shell-composition.md`
  — the lane decision this refines (the lane stays; its span is corrected).
- `plans/125-Composer-Palette-Stage-Flows-and-Centered-Sheet-Retirement.md`
  — the plan that confined the lane to the pane and left D7 open; its Further
  Actions record this fix.
- `DESIGN.md` §5/§6/§8/§12 — the sidebar rail, the working area's grid, box
  ownership, and the lane's span (amended with this decision).
- `frontend/src/sdui/renderer.tsx` (`WORKSPACE_SIDE_SIZE`, `hostRailRegionId`,
  `SduiRegion`, `omittedRegions`), `frontend/src/shell/WorkspacePanes.tsx`
  (`.side`), `frontend/src/routes/workspace.module.css`,
  `frontend/src/shell/workspace-panes.module.css` — the implementation.
- `src/shell/file_browser.rs` (`dimension.sidebar.default`),
  `docs/reference/primitives/shell-layout-strategy.md`,
  `docs/reference/ui-components.md`,
  `docs/reference/packages/creating-packages.md` — the region's authority and
  the authoring contract.
- `design-artifacts/tools/capture-sidebar.mjs` + `design-artifacts/README.md`,
  `test-plan/artifacts/126-rails/` (captures, `drive.txt`, AX extents),
  `test-plan/13-window-splits.md` S47 — the gates and evidence.

## Consequences

- **Positive:** the layout matches the user's model — rails hold the working
  area's full height, the lane spans the middle pane only, and a hidden rail
  returns its width to the lane; rail height, rail width and the lane's edges
  are one grid's decision; the sidebar's region stays server-driven (rows,
  filter, sizes) while its box becomes host geometry; package panels keep their
  meaning (`slot: left` is pane content).
- **Costs / risks:** the host must know the region's identity and size token
  (`WORKSPACE_SIDE_SIZE` in the SDUI renderer), so a *second* token-sized region
  would need the same explicit host treatment rather than a generic rule; the
  DEV `package-ui` fixture still renders the tree whole (region inside the pane)
  and therefore differs from the app — accepted and recorded, since its SDUI
  reference is the region's own paint; the narrow rails are drawers over
  content, so at ≤1000px the sidebar's width no longer comes out of the lane's
  measure.
- **Follow-up:** `docs/wiki/modules/react-shell.md` and
  `docs/reference/primitives/shell-layout-strategy.md` describe the new
  composition; the ≤1240px token step (244→224) and the drawer width
  (`min(var(--rail-files, 244px), 92vw)`) are host CSS values that must keep
  tracking the design-system tokens.
- **Revisit when:** a package needs its own always-visible rail (that needs a
  Clay-owned token and a host rail, not a slot API), or the shell grows a third
  side region — the grid, not the tree, is then the place to decide.
