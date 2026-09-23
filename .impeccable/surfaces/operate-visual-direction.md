# Surface Direction: Operate Mode UI Design System

<!-- impeccable:surface-brief 1 -->

## Scope & Operating Context

- **Surface Scope:** Global desktop application shell, multi-pane editor, toolbars, overlays, agent transcript, and SDUI component catalog.
- **Visitor / Operator Mode:** Operate. Software engineers and systems programmers engaged in long, focused editing, debugging, and agent-assisted sessions.
- **Audience Job:** Fast keyboard navigation, distraction-free code reading/editing, unambiguous pane focus, immediate state feedback on interactive controls, and data that aligns on a readable grid.
- **Primary Constraints:**
  - Strict separation of content-theme color authority (`var(--clay-*)`) from design-system geometry/material/motion (`var(--clay-ds-*)`).
  - Zero literal hex/RGB color values in design-system recipes.
  - Sub-millisecond keystroke responsiveness; zero layout shift, measurement, or JavaScript recalculation on state changes.
  - Full accessibility conformance: forced-colors, prefers-reduced-motion, prefers-reduced-transparency, visible focus everywhere.
  - Normative specification: [`DESIGN.md`](../../DESIGN.md) (Quiet Instrument). This brief records the direction; `DESIGN.md` owns the values.

---

## Direction Contract

### THESIS
A quiet instrument: one continuous surface zoned by 1px hairlines and whitespace, exactly two elevations (canvas and overlay), accent reserved for state rather than decoration, and typography — monospace for data, never for prose — carrying the hierarchy that borders used to. It refuses boxed-in-boxes chrome, hard offset shadows, 90-degree control corners, gradients, textures, film grain, and any motion that does not report a state change.

### OWN-WORLD
- **Palette & Material Authority:** Content themes are the sole color authority. The default surface is the canvas (`surface.main`); grouped content sits on a veil (`surface.panel` @0.55); insets (fields, composers, meters) use `surface.control`; transient layers use `surface.overlay`. Chrome strips inherit the canvas and are separated by a single `border.hairline` (~34% ink alpha; themes supply the alpha).
- **Geometry & Structure:** One radius ladder (5 chips/kbd, 8 controls/rows/tabs, 12 panels/popovers, 16 window/sheets/modals, pill for tabs/chips/toasts) and one border weight (1px). The only two-pixel marks are state: the focus ring and the composer shell's accent boundary, both inset shadow layers, never boxes or one-sided borders; the leading selection bar and any underline are **retired** (`DESIGN.md` §14.13). Full-bleed region edges stay square and flush.
- **Spatial Rhythm:** 32px rows (26 compact), 30px controls (28 compact), 40px title bar, 28px status bar, 244px sidebar (224 at ≤1240px), 340px rail/inspector (312 at ≤1240px); 4/8/12/16/24px spacing steps with 10–14px panel interiors and 18–20px sheet padding. Density scales the spacing rhythm only.
- **Motion Grammar:** 150ms `ease-out` for state changes, 240ms `spring-snappy` for surfaces entering (fade + small translate/scale), one-shot 620ms accent pulse when a keyboard action moves focus, `press-shift-down` on button press. No hover lift, no bounce, no loops except the running-work indicator; reduced motion collapses everything to instant.
- **Type & Measure:** 20/15/13/12/11/10px ladder; monospace with tabular figures for every datum; 10px/0.14em uppercase micro-labels for section eyebrows; editor column 92ch, transcript 72ch, empty-state prose ≤48ch.

### STORY
The developer opens Clay and lands on the launcher — recent workspaces and configured agents, one of each per launch — and then sees the document, not the editor. Regions are separated by a hairline and air; nothing is framed. The eye lands on text and data. Moving the pointer tints a row; pressing a button nudges it 1px; selecting a file tints its fill — nothing is drawn on its leading edge (no bar, no underline). When a keyboard action moves focus, the destination pulses once so the eye finds it. Overlays lift on a soft, wide shadow — the only shadows in the application — and then disappear without ceremony.

### FIRST VIEWPORT
Launcher (`⌘T`, and what a fresh window opens): two panes — recent workspaces and configured agents — each with a filter well, closed by one action row whose primary button names what it will open. Choosing a workspace opens the tab's **workspace view**: sidebar with the workspace tree (mono names, right-aligned counts, count footer) · centered editor column with a line-number gutter and a 92ch measure · optional right rail with document facts and the entry outline (`⌘I`) · status bar with workspace, connection, counts, and the keyboard hint row. `⌘2` switches the same tab to its **agent view**: header with the agent-type picker as the view title, model, usage meter and effort · 72ch transcript of hairline-separated turns · one-boundary composer · inspector (340px, drawer below 1000px) with Files (session file history), Memory, Context, Session Info and Settings. `⌘K` raises the centered command palette: radius-16 sheet, hairline border, overlay shadow, mono group eyebrows, rows that tint on hover and on keyboard selection.

### FORM
Quiet Instrument — approved direction from the 2026-09-11 four-screen proposal round (`design-artifacts/DS/`), documented in `DESIGN.md`. Ancestry: technical instrument panels and editorial typography — the density discipline of a mixing desk, the restraint of a type specimen sheet.

### FINISH
Unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and `DESIGN.md` conformance proven across every recipe-styled surface in the matrix, with the retired patterns of `DESIGN.md` §14 provably absent.

---

## Contrast Case: Removed Reference Systems

The architecture keeps multiple design systems replaceable without host changes;
what shipped is one language.

- **Restrained Neobrutal** and **Luminous Glass** were the two comparison systems
  while the direction was chosen; both packages were **removed** by the Quiet
  Instrument migration (plan 118 task 9), so neither is a selectable system any
  more. Their languages survive only in `decision-logs/` and `plans/104-*`.

Neither is selectable any more: the architecture keeps systems replaceable (typed `uiDesignSystem` data, one language at a time, no host branching), and the shipped set is `@clay/design-instrument` plus the `@clay/core` baseline. Appearance changes are design-system package data plus `DESIGN.md` updates — never host CSS or component rewrites.
