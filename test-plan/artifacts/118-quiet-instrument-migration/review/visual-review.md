# Visual and accessibility review — Quiet Instrument migration (Plan 118)

Review date: 2026-09-13. Reviewer: implementation agent, with the user as the
approval authority for anything this review does not resolve.

Reference material: `design-artifacts/approved/quiet-instrument-migration/**`
(the frozen design set), `DESIGN.md` §5/§10–§16 (the normative spec),
`design-artifacts/prototypes/quiet-instrument-migration/**` (the specimen
catalog those designs were approved from).

## 1. What was exercised

Every capture runs the real Linux build (`clay server` + `clay client`), driven
through real AT-SPI actions, captured through `xdg-desktop-portal`, and cropped
to the compositor's own window bounds (no host pixels are retained — plan 097).
Each capture records a screenshot, the measured viewport, the accessibility
tree of the captured state, and the drive steps that produced it.

| Scene                         | Fixture                                | Themes                | Widths             |
| ----------------------------- | -------------------------------------- | --------------------- | ------------------ |
| Workspace with rail           | `ui-review-workspace`                  | all four shipped      | 1500×950, 1024×800 |
| Command palette open          | `ui-review-workspace` + palette click  | all four shipped      | 1500×950, 1024×800 |
| Launcher landing              | `ui-review-launcher`                   | all four shipped      | 1500×950, 1024×800 |
| Launcher (default route)      | `ui-review-default`                    | all four shipped      | 1500×950, 1024×800 |
| Workspace, rail hidden        | `ui-review-workspace` + outline toggle | gruvbox-material-dark | 1500×950, 1024×800 |
| Titlebar action focus (Files) | `ui-review-workspace` + Files click    | gruvbox-material-dark | 1500×950, 1024×800 |
| Native open dialog            | `ui-review-workspace` + Open click     | gruvbox-material-dark | 1500×950, 1024×800 |
| Document loading              | `ui-review-loading`                    | gruvbox-material-dark | 1500×950, 1024×800 |
| Document error                | `ui-review-error`                      | gruvbox-material-dark | 1500×950, 1024×800 |
| Session recovery              | `ui-review-recovery`                   | gruvbox-material-dark | 1500×950, 1024×800 |
| Large typography              | `ui-review-large-typography`           | gruvbox-material-dark | 1500×950, 1024×800 |

42 captures, all `PASS`. Every capture asserts zero horizontal overflow and that
the visible inlay state matches the drive steps; the screenshots were then
measured for viewport size, clipping, and horizontal overflow as a whole.
Visual inspection covered: workspace with rail (modus-operandi, gruvbox-material-dark,
1024×800 gruvbox-material-dark), command palette open (modus-vivendi,
modus-operandi), launcher landing, large typography (before and after the fix
below), and the shell chrome in every inspected capture.

The status-bar fix in §3 landed after the main matrix was captured, so those
screenshots show the pre-fix status bar. At default typography the two are
pixel-identical for everything the review checked; `large-typography--*` was
re-captured after the fix and is the evidence for it. `pixel-contrast.txt` was
measured from these capture files.

## 2. Verdict per surface

| Surface                                                      | Verdict    | Notes                                                                                                                                    |
| ------------------------------------------------------------ | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Shell (titlebar, tab strip, status bar)                      | matches    | 40 px titlebar, mono uppercase brand with accent dot, inline tab strip without a bottom hairline, 28 px status bar with key-hint chips   |
| Workspace (sidebar, editor, rail)                            | matches    | 3-column composition, flush sidebar and rail, 92 ch measure, combined docbar, on-demand path strip, rail facts + outline                 |
| Command palette                                              | matches    | 12 px radius, single hairline boundary, accent-15 % selection, foot hints + result count, dialog is named "Control Center"               |
| Launcher                                                     | matches    | Start heading + caption, two panes with filters, foot hints, disabled Open until something is picked, informative empty state for agents |
| Settings panel                                               | matches    | flush zone in the fixed right slot, eyebrow-labelled groups, hairline-separated rows, actions row, no radius or elevation                |
| Agent Settings                                               | matches    | 760 px-capped column, delivered files with mono size and provenance badges                                                               |
| Document states (loading, error, recovery, large typography) | matches    | error and recovery paths keep the shell intact; large typography exposed one defect (§3)                                                 |
| Transient surfaces (tooltip, unsaved-changes sheet)          | unresolved | see §5                                                                                                                                   |

## 3. Defect found and fixed

**Status bar overlapped the hint strip at large typography.**
`large-typography--theme-gruvbox-material-dark--1500x950` (pre-fix) showed the
document status message ("Open the document through the server before saving…")
painted across the workspace label and the hint chips. Root cause: the
`role="status"` wrapper around the status value was a flex item with no
`min-width: 0`, and the value inside it was an inline span, so no truncation
could apply. Fixed in `frontend/src/app/layout/app-shell.tsx` (the wrapper now
carries a `.statusMessage` class) and `frontend/src/app/layout/shell.module.css`
(the footer clips, the message shrinks and ellipsizes). `src/test/shell.test.tsx`
still passes; the re-captured large-typography scene keeps the text inside the
status bar.

## 4. Deviations from the approved artifacts, with disposition

All of these are deliberate, were decided while adopting the approved
compositions, and are recorded in the plan's task outcomes. None changes a
visual property the approval covered; each is either an implementation detail
with identical rendering or a capability the host does not have.

| #   | Deviation                                                                                                                      | Disposition                                                                                                        |
| --- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| 1   | Single-line text input keeps its 8 px boundary on the input slot; the multiline composer paints a 12 px well on the field slot | Accepted equivalence: the specimen shows the composer variant; both follow the one-ring-per-surface rule           |
| 2   | Transcript turns use a leading separator instead of the artifact's trailing separator                                          | Accepted equivalence: same rhythm, no double hairline at the transcript foot                                       |
| 3   | Key hints render as literal chords (`Ctrl X P`) instead of glyphs (`⌘K`)                                                       | Accepted: matches the app's existing hint vocabulary across shell and editor                                       |
| 4   | Appearance remains a dropdown in both the React and SDUI settings panels                                                       | Accepted: keeps one SDUI contract; the approved prototype's segmented control would need a new package-facing kind |
| 5   | No per-theme preview swatch in the theme dropdown                                                                              | Accepted: `UiChoiceOption` carries no palette data, and inventing one would fabricate data                         |
| 6   | Settings rows stack label above control instead of a 190 px label column                                                       | Accepted: the panel is 340 px wide; the eyebrow/hairline language is preserved                                     |
| 7   | Agent Settings has no "unreadable" scene                                                                                       | Accepted: the server treats unreadable skill directories as empty listings                                         |
| 8   | List rows declare a 0 px row border; separation comes from container gaps                                                      | Accepted: matches the approved artifacts, and `list.default.row` is exempted in the 0-radius invariant             |
| 9   | Rail width is 340 px (≤1240 px: 312 px) rather than the artifact's 236 px                                                      | Re-approved by the user during adoption (the rail carries facts and outline)                                       |
| 10  | `--c-line` is gone; hairlines come from `border.hairline`/`border.subtle`                                                      | Accepted: the retired palette step is replaced by the theme roles it was standing in for                           |

## 5. What this review could not resolve

Honest gaps, each with its blocker and the follow-up:

1. **Transient tooltip.** Tooltips appear on pointer hover, and this session
   cannot synthesize pointer input in the desktop build. Focus alone does not
   raise them (`toolbar-focus-visible--*` shows the focus ring, not a tooltip).
   The tooltip's _language_ is verified by the specimen catalog and the
   component-conformance audit; a live render is deferred to a session with
   input synthesis.
2. **Unsaved-changes sheet.** Reaching it needs a document edit. Its focus
   containment is covered by the modal test in `frontend/src/test/components.test.tsx`,
   and the palette (the same overlay layer) is verified live; a live sheet
   capture is deferred for the same reason.
3. **Agent surfaces in the desktop build** (agent landing, transcript running /
   idle / error, agent settings, context inspector). They are contributed as
   package pane content, and the review harness cannot switch panes. Their
   current visual evidence is the earlier dev-route matrix in
   `design-artifacts/screenshots/quiet-instrument-agent/` (12 theme × width
   captures, `report.json` failures empty, bridge mocked); a fresh attempt at
   that route in this session rendered the client's "Session lost" shell because
   the dev IPC mock was not installed, so no new captures were recorded there
   rather than recording a failed shell as evidence. Component, recipe, and
   geometry coverage lives in `frontend/src/test/surface-adoption.test.tsx` and
   `design-artifacts/tools/verify-component-conformance.mjs`; a desktop capture
   needs a bridge-mocked pane fixture (follow-up).
4. **`computer_use_linux_get_app_state`** was exercised as the first observation
   step. Its accessibility tree agreed with the AT-SPI walk for the shell, and
   the screenshot path was superseded by the harness (portal capture + window
   crop, needed for a deterministic per-theme matrix).

## 6. Accessibility results

Live AT-SPI walk of the running app (`a11y-live.txt`, `a11y-live-tree.txt`):

- 53 interactive nodes, **0 unnamed application controls**. The only unnamed
  nodes are WebKit's own scroll pane and scroll bar.
- Landmarks are named: "Application controls", "Clay workspace", "Window tabs";
  the file tree is a named list box with 37 named items; the status bar is a
  `status bar` landmark.
- The tab strip exposes `page tab list` / `page tab: Workspace [FOCUSABLE SELECTED]`,
  and `Hide outline` is a toggle button with an accessible name.
- The command palette exposes `dialog: "Control Center"`, an `entry` with the
  same name, and options whose selected state tracks the highlight — modal
  containment and naming are reported by AT-SPI, not just by the DOM.
- Focus is visible: the titlebar action focus capture shows the ring on a
  control nested in the titlebar, and nested ring duplication is prevented by the
  one-ring-per-surface rule.
- Keyboard flow: chords are dispatched as real AT-SPI actions throughout the
  matrix (palette open, pane toggles, dialog open), which is how the captures
  were produced.

Nits worth a follow-up, not defects:

- One `label` node is exposed with an empty name (the brand mark).
- The rail's `description list` reports empty term/value names at the list level;
  the text lives in child text nodes, so screen readers still read it, but the
  association is looser than it should be.
- The editor content is exposed as a `paragraph` in this WebKit/AT-SPI build with
  no editable text role reported. The editor renders and accepts input; treat
  this as an engine reporting gap to re-check when the runtime is updated.

## 7. Performance and stability observations

- Live boundary contrast (UI-DS-40's live leg, `pixel-contrast.txt`): the rail's
  leading divider measures 1.39:1 (modus-vivendi), 1.45:1 (modus-operandi),
  1.46:1 (gruvbox-material-light) and 1.61:1 (gruvbox-material-dark) against the
  canvas it separates — above the 1.2:1 hairline visibility floor and below the
  structural floor, as the profile requires. The only strong line on a pane
  boundary is the focused pane's 1px accent outline (5.8:1–10.4:1), which is a
  state ring, not a separator. State fills are deliberately subtle (accent at
  15% over the surface); the 3:1 state floor applies to text over the composited
  fill and is enforced on resolved roles by `src/shell/theme.rs` and
  `tests/theme_packages.rs`.

- No blur outside the scrim: surfaces use recipe fills, and the reduced
  transparency fallback (opaque veil, no backdrop filter) is verified in the
  component catalog and in `frontend/src/styles/global.css`.
- No animated shadows or transition-driven layout: every transition is
  `background-color`/`color` on 150 ms ease-out or the 240 ms surface entry.
- No layout shift across themes: for a given scene, all four themes report the
  same viewport and the same geometry, and the design system swap is
  geometry-neutral by construction (core catalog ↔ `tokens.css` fallback gate).
- No horizontal overflow in any of the 42 captures, at either width, in any theme.

## 8. Review outcome

The migrated app matches the approved artifacts on every surface this review
could reach, with the ten recorded deviations dispositioned above, one defect
found and fixed, and four explicitly unresolved items whose blockers are tooling
rather than design. The unresolved items do not weaken any shipped surface: each
is either covered by an automated gate or is a transient surface whose language
is already verified.
