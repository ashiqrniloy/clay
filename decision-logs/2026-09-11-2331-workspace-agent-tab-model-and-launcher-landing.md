---
date: 2026-09-11 23:31
status: approved
decision_about: "Target information architecture: a tab is one workspace plus one agent with two views, and the launcher (not the Coding Agent) is the landing surface"
proposed_by: "user (information architecture), agent (consequences and boundaries)"
explicitly_approved_by_user: true
---

# Decision: Workspace + agent tabs, and the launcher as the landing surface

## Decision

The app's information architecture is:

1. **A tab is one workspace plus one agent, holding two views.** A tab carries a
   `workspaceRoot` (already in `~/.clay/layout.json` v2) and an *agent identity*
   (agent type + config root; nullable). It renders exactly one of its two views
   at a time — the **workspace view** (editor + tree) or the **agent view**
   (transcript + inspector) — switched from tab chrome in the titlebar (`⌘1` /
   `⌘2`), never from a control inside a view. The tab strip holds one entry per
   workspace, titled by the folder's basename (full path in the tooltip) with a
   hairline marker when an agent is attached (pulsing while it works). Layout
   state (active view, sidebar, rail, inspector) persists per tab.
2. **The launcher is the landing surface.** A fresh window and every empty tab
   (`⌘T`, the strip's `+`) show the launcher, not the Coding Agent: two panes —
   recent workspaces (name, real path, branch, MCP count) and configured agents
   (label, config root, skill count) — each with a filter field, over one action
   row whose button names exactly what it will open and stays inert until
   something is picked. Keyboard: `⇥` between panes, `↑↓` move, `⏎` pick, `⌘⏎`
   open both, `esc` clear, `⌘O` open folder.
3. **At most one workspace and one agent per launch, both changeable afterwards.**
   Each pane is single-select; launching picks one of each into one tab. The
   launcher sets a tab's *first* state, not its only state: the agent is changed
   from the agent view's picker (in place, without disturbing the workspace) and
   the folder from the workspace view (open folder / recents), without discarding
   the other half of the tab. Picking several workspaces is not a launcher
   feature — several workspaces means several tabs.
4. **The agent view's title is the agent-type picker.** The available types are
   whatever is configured under the Clay data dir's `agents/` root, enumerated by
   directory scan (no package load); picking one switches that tab's agent
   (identity, prompt, skill roots, MCP servers, model/effort defaults) and resets
   the model/effort controls to that agent's defaults with a visible note. The
   menu never lists an unconfigured agent.
5. **The agent view's Files tab is the session's file history** — the files this
   session read, wrote, created or deleted, newest first, with the role mark —
   and `⏎` switches the tab to its workspace view at that file. It is not a file
   browser; the workspace tree is the workspace view's job.
6. **The composer is one boundary:** the shell's accent border plus its 3px halo
   (deliberately kept), with the field inside drawing no outline of its own, the
   field spanning its zone, and the send/cancel controls inside the shell's
   trailing edge.

This supersedes point 5 of `2026-09-11-1700-quiet-instrument-migration-scope-no-third-design-system-chat-removal`
("The Coding Agent becomes the landing surface … expected to be revisited
later"), which is exactly the revisit that user statement anticipated.

## Context

`2026-09-11-1700` set the migration scope and made `@clay/coding-agent` the
`empty-tab` landing because the previous occupant (`@clay/chat`) was being
removed. That log recorded the landing as provisional. Reviewing the migration
prototypes (plan 118 tasks 3–6), the user specified the intended model instead:
the workspace and the agent are two views of the same tab, and the window opens
on a choice of recents rather than on an agent.

Repository state this lands on:

- Tabs already carry a workspace: `~/.clay/layout.json` v2 stores a
  `workspaceRoot` per tab, and the shell's tab strip is workspace-shaped.
- "Agent" is an implicit single-instance assumption: `@clay/coding-agent`
  contributes a `pane` surface plus `coding-agent.settings`; per-agent config
  already exists on disk (`~/.clay/agents/<agent>/`, decision
  `2026-09-09-1420`), so multiple agent *types* are a data question, not a new
  storage concept.
- Product landings are package contributions (decision `2026-08-21-2152`); the
  host has no compiled landing branch. `@clay/chat` held `empty-tab` via
  `chat.entry` and `@clay/coding-agent` holds `pane`; the frontend renders
  trusted first-party surfaces by provenance (`frontend/src/shell/PaneTree`).
- The migration is already rewriting the shell CSS, the surface catalog, the
  landing election and the agent page, so the model change lands cheaper here
  than after the rewrite (the alternative — migrate to the old model, then change
  it — pays for the same files twice).
- Prototype evidence for every statement above exists in the reviewed set:
  `start.html` (launcher), `shell.html` (tab strip + view switcher),
  `workspace.html` and `agent-landing.html` (the same tab in its two views),
  `agent-landing.html` (picker, session-history Files tab, one-boundary composer).

## Approval

- Proposed by: user (the tab model, the launcher, the Files-tab behaviour, the
  composer treatment); agent (the boundaries recorded in the decision, the
  supersession of `2026-09-11-1700` point 5, the plan tasks).
- Approved by user: Yes.
- Approval evidence: "Okay great. I have one little feedback and that's at
  component level. … The selected package is highlighted with a blue tint but
  with also a blue thick only at the left side. Remove this outline from there
  and at component level. Otherwise all good. Make this change" and, after the
  fix: "Yes. Design approved and you can log it. Keep the halo and yes one
  workspace + One agent per launch, changeable after launch."

## Alternatives Considered

1. **Keep the Coding Agent as the landing surface** (the position in
   `2026-09-11-1700`) — rejected: the window would open onto a transcript with no
   way to choose the folder or the agent, and a fresh user would have to find the
   workspace tree and the agent's configuration before doing anything. The
   launcher states the choice instead of hiding it.
2. **Reopen the last session automatically instead of a launcher** — rejected: it
   is the same single-shot landing with more hidden state; recovery from a wrong
   guess (deleted folder, wrong agent) is worse, and the first-run case has
   nothing to reopen, which is precisely the case that needs a designed surface.
3. **Launcher as a separate window or a route outside the tab system** — rejected:
   it would need its own chrome, its own recents lifetime and its own lifecycle,
   and it would not answer "what does an empty tab show?" — the empty tab would
   still need a landing.
4. **Two panes side by side per tab (workspace | agent split)** — rejected: the
   user asked for *switching* views; a permanent split halves both surfaces at the
   review width and duplicates the tab strip's meaning.
5. **A second tab kind ("agent tab") beside workspace tabs** — rejected: that is
   the current implicit model, and it is what makes the agent's Files tab double
   as a workspace browser. One tab holding both views matches `layout.json`'s
   per-tab `workspaceRoot` and gives the file a single home.
6. **Multi-select several workspaces (and several agents) in one launch** —
   rejected for now: it multiplies tabs from one action and makes the action row's
   label unbounded ("Open clay + 4 more"). Several workspaces are several tabs;
   the constraint is the user's explicit "one workspace + one agent per launch".
7. **Keep the agent hardcoded to `coding-agent` and add the picker later** —
   rejected: the launcher has to enumerate agents anyway, so the data exists
   either way; deferring leaves two places that assume one agent.
8. **Keep the composer's inner focus outline and drop the shell's ring** —
   rejected: the shell is the boundary that owns the field's geometry, so it owns
   the focus state; an outline inside a highlighted shell reads as two borders
   (`DESIGN.md` §14.4).

## Rationale and Evidence

- The tab is already the unit of workspace identity (`layout.json` v2's per-tab
  `workspaceRoot`), so widening a tab to hold an agent identity adds one nullable
  field to existing data rather than a parallel session concept.
- The agent's work is *about* the tab's folder: the transcript, the session file
  history and the workspace tree are three views of one activity. Splitting them
  across two unrelated tabs is what forces the Files tab to grow a file browser
  (prototype README §9 finding 14).
- The launcher answers the first-run case with real data instead of a fabricated
  greeting: recents come from disk, agents from the config root, and a
  deleted folder or unresolved agent is pruned with a diagnostic rather than
  listed (prototype README §7; `start-recents.json` records what was read).
- The composer correction is a `DESIGN.md` violation, not a preference: the
  approved screen drew the shell's accent ring *and* the textarea's own
  `:focus-visible` outline — two boundaries on one surface (§14.4), which the
  user reported as "two borders" and asked to be one.
- The retired leading bar on selected rows is the same class of finding: §2's
  "one signal per surface" against a recipe that added a fill *and* a 2px edge.
  Retiring it at component level (not only in the artifact where it was noticed)
  is what keeps the migration from re-introducing it on the tree in week two.

## References

- `DESIGN.md` §11 (`viewSwitch`, `agentPicker`, `recentRow`, `sessionRow`,
  `list.row.selected`), §12 (the tab, the launcher, the agent view, the composer),
  §13.2/§13.4, §14.3, §14.13 (the retired leading bar), §16 (recipe set).
- `design-artifacts/approved/quiet-instrument-migration/README.md` — the approval
  record, the frozen file hashes, the coverage table, and what the approval does
  not cover.
- `design-artifacts/prototypes/quiet-instrument-migration/` — `start.html`,
  `shell.html`, `workspace.html`, `agent-landing.html`, `README.md` §2 (surface
  inventory), §7 (coverage and verification: 112 runs, 0 failures, 79 baselines),
  §9 (16 findings).
- `plans/118-Quiet-Instrument-Migration-Component-and-Surface-Adoption.md` — tasks
  33–36 (Part D) implement this decision; tasks 8, 10, 12, 21, 25 and 28 were
  amended for it.
- `decision-logs/2026-09-11-1700-quiet-instrument-migration-scope-no-third-design-system-chat-removal.md`
  — point 5 superseded; points 1–4 (one design system, four themes, chat removed)
  stand unchanged.
- `decision-logs/2026-09-11-1655-html-prototype-approval-gate-and-design-artifacts.md`
  — the gate this approval round ran through.
- `decision-logs/2026-09-09-1420-per-agent-config-layout-agents-coding-agent.md`
  — per-agent config roots that make agent types data.
- `~/.clay/layout.json` v2 (`workspaceRoot` per tab) — the existing tab record the
  agent identity extends.

## Consequences

- Plan 118 Part D: task 33 (tab model + view switcher), 34 (launcher), 35
  (agent-type registry + picker), 36 (session-files history + open-in-workspace
  handoff). Task 25 is rewritten: the `empty-tab` landing becomes the launcher,
  and `@clay/coding-agent` keeps its `pane` surface.
- Which package owns the `empty-tab` launcher contribution is an implementation
  choice (a new `@clay/launcher`, or the package that ends up owning the recents
  store); the constraint from `2026-08-21-2152` is that it is a contribution —
  first-party, provenance-checked, SDUI content — and not a compiled host branch.
- The recipe set grows by the four new families (`viewSwitch`, `agentPicker`,
  `recentRow`, `sessionRow`), fixed in task 8 and asserted in task 12; the
  launcher's and the switcher's other surfaces (`seg`, `chip`, `empty`, shortcut
  vocabulary, status dot, swatch, agent stat rows) are declared there too, since
  they were already drawn and are currently unbacked.
- `layout.json` and its validation gain an agent identity and an active view per
  tab; a persisted tab with neither workspace nor agent must fail with a
  diagnostic rather than rendering blank, and a persisted preference naming a
  removed agent type must degrade to the picker.
- The roadmap's Phase 2 agent-UI binding spec (a 50/50 chat/agent split) is
  reconciled in the migration's documentation work (task 28); no document may
  still call the Coding Agent the window's landing surface.
- Work type: this is **feature work carried by the migration**, not re-skinning.
  It is why Part D is scheduled after the component/surface adoption and before
  the visual review, and why the launcher's and switcher's styles cannot be
  written in host CSS (they must be recipes).
- Conditions that would cause revisiting this decision: the launcher turning into
  a workspace manager (favourites, remote roots, per-workspace config) would want
  its own surface rather than an empty-tab contribution; and a third view on a tab
  (a diff, a terminal) would test whether two views is a rule or a coincidence.
