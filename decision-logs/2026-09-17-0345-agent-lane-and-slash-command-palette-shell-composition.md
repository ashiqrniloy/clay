---
date: 2026-09-17 03:45
status: approved
decision_about: "Shell composition: one persistent agent lane at the bottom of both views, with the Control Centre consolidated into the composer's / palette"
proposed_by: "user (direction and three review rounds), agent (prototype and wording)"
explicitly_approved_by_user: true
---

# Decision: one persistent agent lane, and the Control Centre becomes the composer's `/` palette

## Decision

The Clay shell gains a **persistent agent lane** at the bottom of the view pane —
mounted once, shared by the workspace and agent views — carrying the approval
strip, the composer box (text field, the tab's agent-type/model/effort pickers
and context meter, the hint row) and a session-environment foot (workspace root,
git branch, extensions, MCP summary). The Control Centre stops being a centered
modal: its catalogue becomes the composer's **`/` palette**, anchored to the
composer box, spanning it edge to edge, over a blurred veil shared with the
`@` mentions menu. The agent view keeps the transcript, state strip and
inspector but loses its header; the run's signal is the window mark's dot (plus
the state strip's three working bars); `Ctrl+X Ctrl+P` toggles the lane and
`Ctrl+X Ctrl+O` opens the palette. Approved artifact:
`design-artifacts/approved/agent-lane-palette/`.

## Context

The agent view owned the composer, the agent controls and the session foot, so
the workspace view had no way to prompt the tab's agent and switching views
moved the bottom of the window. The Control Centre was a centered modal holding
the server's command catalogue — a second command surface competing with the
composer's own `/` list, and the one place `/` did **not** discover commands
from. The user asked for the bottom section to be constant (`Control Center`
functionality folded into it, hideable by chord), for the Control Centre's
options to live behind `/` in the input box, and for the `/` menu to keep the
Control Centre's blurred-backdrop aesthetic. Three review rounds then settled
the details: the palette spans the composer box like the `@` menu; the agent
controls move into that box and the agent view's header is retired; the send
button is dropped (`↵` sends, the slot is Stop's while running); no provider
never blocks typing (the reason moves to the lane's foot); the `@` menu shares
the veil; the run's signal moves to the window mark's dot; and the title bar is
adopted from the app as implemented rather than the earlier prototype's invented
chrome.

## Approval

- Proposed by: user (the shell direction, the composition feedback across the
  rounds); agent (the prototype, its README and the wording of the decisions).
- Approved by user: Yes.
- Approval evidence: "Okay. Design artifact now approved" (2026-09-17), after
  the third round: "You misunderstood one feedback. In the streaming state, the
  bar above the input box that says working, should show the animation with
  three bars instead of the dot. Remove the dot from there and show the
  animation." The earlier rounds: "The / palette should take the full width of
  the control center input box like the mentions menu do" / "Add the agent type
  dropdown, the model picker and the thinking/reasoning effort dropdown to the
  agent lane" / "remove those from the Agent main view header"; and "Remove the
  Send button, allow me to type in the lane input when no provider is
  configured, add the streaming animation to the agent lane, add the blurred
  background to the @ mentions menu".

## Alternatives Considered

1. **Keep the Control Centre modal and add a separate `/` menu** — two command
   surfaces over one catalogue; rejected by the user's instruction to fold the
   Control Centre's options behind `/`.
2. **Lane animated per view, or a second composer in the workspace view** —
   two owners of one draft, remount or state loss on view switch; rejected in
   the prototype's variant table.
3. **Palette as its own sheet with its own input** (the shipped Command Centre
   shape, merely moved down) — duplicates the composer directly beneath it, two
   inputs for one query.
4. **Palette as a 640px sheet anchored above the lane** — drawn first, then
   superseded: the user asked for the composer box's own width.
5. **Agent controls as a row above the composer box** — the palette hugs the
   box, so the row would be hidden while the palette is open; rejected.
6. **Send button in the lane / inert composer with no provider** — both were
   drawn and removed: `↵` already sends, and blocking the field punishes
   drafting a prompt before a provider is configured.
7. **Run cue in the lane's foot** (`● Working` + bars) — drawn, then removed:
   the foot states the session's environment; the run's signal belongs to the
   window mark, with the agent view's state strip saying it in words.
8. **Title bar from the earlier prototype kit** (labelled `Commands` chip,
   `Hide files` / `Hide inspector` buttons, drawn window buttons) — rejected:
   the app implements the real shell (mark, tab strip, spacer, window actions)
   and has no window buttons.

## Rationale and Evidence

- **One tab, two views, one bottom section.** The tab already owns one
  workspace and one agent (plan 118 T33 / plan 119 SC-6, `decision-logs/2026-09-11-2331-...`),
  so the composer belongs to the tab, not the view: one mount below both view
  slots keeps the draft, the run state and the pickers across a view switch,
  and matches the user's "the bottom section should always match the coding
  agent view".
- **One command surface.** The palette reuses the server catalogue
  (`src/server/control_center.rs`: snapshot built once per menu session, fuzzy
  re-scoring, routing-policy filtering) and the shipped `commandCentre.*`
  recipes; the composer's own `/` list stops being a second implementation.
  The palette owning the composer's draft as its query removes an input.
- **One veil.** The `@` menu and the palette are the same gesture on the same
  composer; the veil is the shipped `modal.default.scrim.rest`
  (`backdrop-filter: blur(3px)`, `surface.scrim` @0.5) with the lane above it,
  because the lane is the palette's input.
- **Signal versus decoration.** "Accent appears for running work" (`DESIGN.md`
  §12) is carried by one pulsing mark at the window edge plus the state strip's
  bars — in both views, without adding a chip to the environment foot.
- **Verified, not asserted.** `design-artifacts/tools/capture-agent-lane.mjs`
  asserts 88 runs (11 scenes × 4 themes × 2 widths) plus a keyboard walk:
  geometry of the palette against the composer box, real `blur(3px)` with the
  lane above the veil, no Send button anywhere, Stop exactly while streaming,
  the mark's dot at 1.1s exactly while streaming, the strip's dot swapped for
  three 1.1s bars exactly while a turn is in flight, the title bar in the app's
  composition, `backdropBlur == 0` on content surfaces, no overflow, no console
  error — 68 PNGs + `report.json` in
  `design-artifacts/screenshots/agent-lane-palette/`.
- **Chrome follows the implementation.** The prototype's own window buttons and
  titlebar toggles were removed once the review checked the app: the shell's
  composed chrome is `frontend/src/app/layout/app-shell.tsx` +
  `shell.module.css`, and porting a prototype to code is cheaper than porting
  code back to a drawing.

## References

- `design-artifacts/approved/agent-lane-palette/` — the approved set (frozen
  2026-09-17, per-file hashes, approval record); `README-prototype.md` there
  holds the 15 decisions, the three review rounds and the rejected variants.
- `design-artifacts/prototypes/agent-lane-palette/` + `design-artifacts/tools/capture-agent-lane.mjs`
  + `design-artifacts/screenshots/agent-lane-palette/report.json` — the working
  copy and its assertion gate.
- `plans/124-Persistent-Agent-Lane-and-Slash-Command-Palette.md` — the plan
  that implements this decision (task 3 froze the artifact).
- `DESIGN.md` §7/§11/§12/§14 — to be amended by plan 124 (shell composition,
  palette placement, the mark's pulse and the working bars, retirement of the
  centered palette sheet).
- `frontend/src/app/layout/app-shell.tsx`, `frontend/src/shell/WorkspacePanes.tsx`,
  `frontend/src/coding-agent/CodingAgentPanel.tsx`,
  `frontend/src/command-centre/CommandCentre.tsx` — the surfaces this changes.
- `src/server/control_center.rs`, `src/shell/transient_menu.rs` — the catalogue
  and anchor machinery the palette reuses.

## Consequences

- **Positive:** one composer, one command surface, one bottom section across
  views; the workspace view can prompt the tab's agent; `/` and the Control
  Centre can no longer disagree; less chrome in the agent view (no header, no
  Send button); the palette reuses shipped recipes and the server catalogue.
- **Costs / risks:** `controlCenter.open` changes meaning (it opens the palette
  in the lane, not a modal) and `Ctrl+X Ctrl+P` is rebound, so hints,
  `examples/config/init.js`, the settings/`?` map and docs must move together;
  the lane is a new Clay-native internal shell surface needing a catalog row and
  a `DESIGN.md` §12 update; the state strip loses its `statusDot.busy` caller.
- **Recorded, not decided:** whether the app's tab marker keeps its own
  `agentPulse` beside the window mark's; whether `↵`-only sending needs a
  palette row as the pointer path (plan 124's accessibility review decides).
- **Revisit when:** the palette needs its own input (multi-line queries),
  the lane's once-per-tab mount shows a state-sharing problem across tabs, or
  `DESIGN.md` §12's wording and the artifact disagree — the specification wins
  and the artifact is corrected through a new approval.
