# @clay/design-instrument — Quiet Instrument

The shipped UI design system for Clay. It is **declarative data**: geometry,
material, motion and role selection for every cataloged component. It contains no
colour values, no typography values, no CSS, no JavaScript and no permissions.

- Normative specification: `DESIGN.md` (this package implements §4–§11 and obeys §14).
- Approved visual reference: `design-artifacts/approved/quiet-instrument-migration/`.
- Decision records: `decision-logs/2026-09-11-1615-quiet-instrument-design-language.md`,
  `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`.

## 1. The language in one paragraph

One plane: the canvas is the only surface, and everything else is a hairline zone
on it or a veil over it. Two elevations: **veil** (a panel at `surface.panel` @0.55
with no shadow) and **overlay** (a popover/sheet at `surface.overlay` with the pop
or overlay shadow). Accent is reserved for state — selection, focus, the running
indicator — never decoration. Typography carries structure, so containers stay
unpainted. Motion is meaningful: 150ms for a state change, 240ms for a surface
entering, 620ms for a keyboard focus announcement.

## 2. Content-theme colour authority

Recipes reference **semantic theme roles only** (`surface.control`, `text.muted`,
`accent.primary`, `focus.ring`, `border.hairline`, `diagnostic.error`, …) or
`transparent`. There are zero hex/RGB/HSL literals and zero palette aliases: the
active content theme (`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`,
`@clay/theme-gruvbox-material-dark`, `@clay/theme-gruvbox-material-light`, or a
user theme) is the only colour authority. Typography is user-owned and is never
set here.

## 3. Design-system values (the language's numbers)

Declared in `values`; recipes carry the same numbers within their typed domains.

| Value | Type | Number | Used for |
| --- | --- | --- | --- |
| `radius.xs` | radius | 5 | Badges, kbd chips, small selectable cells |
| `radius.control` | radius | 8 | Buttons, inputs, rows, tabs, menu items |
| `radius.panel` | radius | 12 | Panels, veils, popovers, dropdowns, composers |
| `radius.surface` | radius | 16 | Window, sheets, command palette, modals |
| `radius.pill` | radius | 9999 | Tab-bar tabs, segmented controls, toasts, split handles, dots |
| `border.hairline` | border-width | 1 | Every structural boundary (zoning) |
| `border.emphasis` | border-width | 2 | State indicators only, as inset shadows (never a box border) |
| `motion.fast` | motion-duration | 150 | Hover, active, focus, fill/border transitions |
| `motion.enter` | motion-duration | 240 | Popover/sheet/overlay entrance |
| `motion.flash` | motion-duration | 620 | Keyboard-initiated focus move (one-shot pulse) |
| `opacity.disabled` | opacity | 0.5 | Disabled controls |
| `opacity.veil` | opacity | 0.55 | Panel/plane fill over the canvas |
| `opacity.soft` | opacity | 0.15 | Accent-tinted fill for selected/active surfaces |
| `blur.scrim` | backdrop-blur | 3 | Overlay scrim (the only blur besides the toast's 8) |

Values are declarative documentation in the current schema — recipes carry
literals — so the language reads in one place and the migration has one source
for its numbers.

## 4. Materials and state (what the recipes encode)

| Surface | Fill | Opacity | Border | Radius | Shadow |
| --- | --- | --- | --- | --- | --- |
| Canvas / window / chrome strips | `surface.main` | 1.0 | 1px `border.hairline` on the inner edge | 16 (window only) | none |
| Panel / plane (veil) | `surface.panel` | 0.55 | 1px `border.hairline` | 12 | none |
| Inset well (field, composer) | `surface.control` | 1.0 | 1px `border.hairline` | 8 single-line, 12 composer | none |
| Row / tab (rest) | transparent | — | none | 8 (rows), pill (tabs) | none |
| Popover / dropdown / menu / tooltip | `surface.overlay` | 1.0 | 1px `border.hairline` | 12 (tooltip 8) | pop |
| Sheet / palette / modal | `surface.overlay` | 1.0 | 1px `border.hairline` | 16 | overlay |
| Scrim | `surface.scrim` | 0.5 | none | — | none, `backdropBlur: 3` |
| Toast | `surface.panel` | 0.88 | 1px `border.hairline` | pill | pop, `backdropBlur: 8` |

Shadows are structured layers in the ink role — no literals:

- `overlay` = `[{0,24,60,-20, text.primary @0.42}, {0,2,10,-4, text.primary @0.22}]`
- `pop` = `[{0,14,34,-14, text.primary @0.34}, {0,1,3,-1, text.primary @0.16}]`
- accent halo (fields/composers on focus) = `[{0,0,0,+3, accent.primary @0.15}]`

States follow `DESIGN.md` §9: hover is a `surface.hover` fill (and
`border.subtle` where a boundary exists), active is `surface.active` plus
`press-shift-down` **on buttons only**, focus is a 2px `focus.ring` outline at
offset 2 on every interactive element, selected is `accent.primary` @0.15 with
`text.primary` (and `accent.primary` text on tabs, segmented items, the agent
picker and launcher rows), disabled is `text.disabled` at `opacity.disabled`
with no shadow and no transform. A selected row is a fill and nothing else —
there is no leading accent bar (`DESIGN.md` §14.13).

Retired by this language (do not reintroduce): hard offset shadows, `borderRadius: 0`,
borders wider than 1px, hover lift, blur on canvas/panels/rows, film grain,
gradients, inner-highlight rims.

## 5. Recipe inventory

63 families / 165 keys. The key set is the reference packages' 142 keys minus the
12 `chat.default.*` keys (the chat surface is deleted by plan 118 tasks 23–24),
plus the families the target information architecture and the drawn-but-unbacked
surfaces need.

### Reference families, re-valued (130 keys)

| Family | Slots.states |
| --- | --- |
| `badge.{accent,default,error,muted,success,warning}` | root.rest |
| `button.{default,primary,muted,danger}` | root.{rest,hover,active,focus,disabled} |
| `card.default` | root.{rest,hover} |
| `collapse.default` | root.rest, header.{rest,hover,focus}, body.rest |
| `commandCentre.default` | root.rest, empty.rest, status.rest |
| `divider.default` | root.rest |
| `dropdown.default` | root.rest, trigger.{rest,hover,active,focus,disabled}, popover.rest, list.rest, item.{rest,hover,active,selected,focus} |
| `editor.default` | root.rest, container.rest, chrome.rest, gutter.rest, activeLine.rest, selection.rest, findMatch.rest, matchingBracket.rest, path.rest, tooltip.{rest,selected} |
| `fileBrowser.default` | root.rest |
| `flex.{default,row,column}`, `stack.default`, `portal.default`, `scroll.default` | root.rest (+ `scrollbarTrack.rest`, `scrollbarThumb.{rest,hover,active}`) |
| `kbd.default` | root.rest |
| `label.{default,title,display,section,body,detail,caption,status}` | root.rest |
| `list.default` | row.{rest,hover,active,selected,focus} |
| `menu.default` | root.rest, item.{rest,hover,active,selected,focus} |
| `modal.default` | dialog.rest, scrim.rest |
| `overlay.default`, `popover.default`, `tooltip.default` | root.rest |
| `paneSplitTree.default` | group.rest, pane.{rest,active}, handle.{rest,focus} |
| `panel.default` | root.rest, header.rest; `panel.fixed.root.rest`, `panel.transient.root.rest` |
| `settingsPanel.default` | panel.rest, heading.rest, actions.rest |
| `shell.default` | root.rest, header.rest, workingArea.rest, brand.rest, footer.rest |
| `statusBar.default`, `statusItem.default` | root.rest |
| `tab.default` | item.{rest,hover,active,selected,focus,disabled} |
| `tabBar.default` | root.rest |
| `textInput.default` | field.rest, input.{rest,hover,focus,invalid,disabled}, label.rest, description.rest, error.rest |

### Families added by this package (35 keys)

| Family | Slots.states | Why it exists |
| --- | --- | --- |
| `seg.default` | root.rest, item.{rest,hover,selected,focus,disabled} | The segmented control, including the tab's Workspace \| Agent view switcher (plan 118 task 33). One family, not two: the switcher *is* a segmented control. |
| `agentPicker.default` | trigger.{rest,hover,expanded,focus,disabled} | The agent view's title is the agent-type picker (task 35); its menu body is `dropdown.default.list`. |
| `recentRow.default` | root.{rest,hover,selected,focus} | The launcher's workspace/agent rows (task 34) — a list row plus a muted meta column. |
| `sessionRow.default` | root.{rest,hover,focus} | The Files tab's session-history row (task 36): role mark, basename, muted directory, role word. Never a selected state — a session file is opened, not selected. |
| `toast.default` | root.rest | Transient confirmation: veil fill at 0.88, pill, pop shadow, `backdropBlur: 8`. |
| `empty.default` | root.rest | Empty states (first run, no results): transparent, centred, ≤48ch prose. Titles and bodies use the `label.*` roles. |
| `statusDot.{success,warning,error,muted,busy}` | root.rest | The status dot, including the accent-filled running indicator whose 1.2s pulse is host motion, not a recipe. |
| `keyHint.default` | root.rest, row.rest, keys.rest | The shortcut vocabulary: a hint line, a shortcut-map row, and the row of `kbd.default` chips it contains. |
| `swatch.default` | root.{rest,hover,focus,selected} | The theme swatch in settings (the three stripes are host composition; the recipe owns the rim, radius and selected state). |
| `statRow.default` | root.{rest,hover}, bar.rest | The agent's stat rows (label + value + meter bar) in the inspector's Context tab. |

Unifications made while declaring the above, so no surface has two names:
**chip → `badge.*`** (the chip and the badge were the same idea drawn twice, and
`badge.{accent,success,warning,error,muted}` already covers the chip variants),
and **view switcher → `seg.default`** (the switcher is a segmented control; a
second `viewSwitch` family would have duplicated every one of its keys).

## 6. Gaps this package does not close (owned by later plan 118 tasks)

- **Role bindings in themes.** `border.hairline`/`border.subtle`/`border.strong`,
  `accent.primary`/`accent.muted`, `surface.scrim` and the state fills are only
  meaningful once the four shipped themes contribute the typed `designTokens`
  overrides (task 14). Until then the resolved values come from the core token
  fallbacks, which do not yet distinguish hairline from strong.
- **Core fallbacks and host CSS.** `core_design_system_fallbacks()` and
  `frontend/src/styles/tokens.css` must mirror these recipes before any surface
  paints them (task 16); the migration's host-CSS adoption is tasks 17–25.
- **A `status.*` role vocabulary.** The status dot and session-file marks use
  `diagnostic.{success,warning,error}` and `text.muted` instead of the
  `status.ok/warn/error` names sketched in an early draft of the plan: those roles
  do not exist in the token catalog, and inventing them here would be colour
  authority in the wrong layer. `DESIGN.md` §11 names the real roles.
- **Edge-only borders.** The schema has no per-edge border width, so a chrome
  strip's bottom hairline or a panel header's inner hairline is declared as a
  1px `border.hairline`; the host applies it to the correct edge (task 16).
- **Border alpha.** `borderColor` has no opacity companion, so a "role at 34%"
  border (semantic badges) is declared at full role strength; the muted step is
  the host's `color-mix` (tasks 14–16) or a role that already carries alpha.
- **The pulse and the halo annunciation.** Motion that is a *loop* (running
  status dot) or a *one-shot announcement* (620ms keyboard focus pulse) is host
  motion per `DESIGN.md` §7/§9; the recipes carry the colours and the 150/240ms
  state transitions only.
