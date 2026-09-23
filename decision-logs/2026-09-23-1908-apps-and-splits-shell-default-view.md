---
date: 2026-09-23 19:08
status: approved
decision_about: "UI shell rearchitecture: workspace/agent navigator left pane, activities as tabs, apps in user-controlled splits, global bottom lane overlay, replaceable sections"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Apps-and-Splits Shell as the Default View

## Decision

Clay's UI becomes an **apps-and-splits shell**, inspired by herdr's
workspace/agent navigator. The new default view:

- **Left navigation pane** with two independent sections: **Workspaces**
  (folder name by default, user-renamable) and **Agents** (every engaged
  agent across all workspaces — not only active, and not confined to the
  selected workspace — each with status: idle, working, blocked, …).
  Sections are registry-driven: a third-party package can replace,
  remove, or add sections (e.g. a different navigator, or move the
  command center / agent lane to a right-side overlay).
- **Main area** hosts **views conceptualized as apps**. Built-in apps at
  launch: the **editor** (file-path entry + file view) and the **agent
  runtime app** (agent activity view plus the current right-side
  information — files, memory, context). Future apps are first-class:
  canvas (diagrams, images, slides, documents, video), anything a package
  registers.
- **Tabs within a workspace are activities** (open app views); the user
  can **split** the main area freely, each split hosting one view; an app
  engaging another app opens it in a split automatically (e.g. selecting a
  file under the agent app's Files tab opens the editor app in a new
  split).
- **Agent runtime app defaults to the activity view only**; the
  information pane is tucked behind a **gear icon** (inspect inline or
  open in a separate split).
- **The bottom lane stays as-is** — message composer, command palette,
  mentions — and becomes a **global overlay present in every app** (also
  over the editor), so the agent is reachable no matter what the user is
  doing.

The architecture must make future apps and shell re-composition possible;
this layout is the *default view*, not a hard-coded shell.

## Context

Current model: one workspace per tab; everything for that workspace runs
inside that tab (transcript + right info pane + composer). The user
redesigned this after using herdr (independent workspace/agent navigation,
agent-status-at-a-glance) and wants multi-app surfaces (editor beside
agent runtime, future canvas/media) with agent access everywhere.

## Approval

- Proposed by: user.
- Approved by user: Yes
- Approval evidence: full UI concept message (2026-09-23): "I want to
  modify the UI concept based on herdr as well. … The left pane will have
  two sections, just like herdr … Users then can split the UI area however
  they want and in each split they can have one view … The main point is
  that we need to have the architecture that it is possible in the
  future." Plus: "This goes along with the 149 prototype freeze and should
  be mentioned as guidelines while creating the prototype."

## Alternatives Considered

1. **Keep per-workspace monolithic tab; add secondary panes ad hoc** —
   rejected: caps at one app per workspace, no composition, contradicts
   the requested editor-beside-agent and future canvas apps.
2. **Full docking framework (Eclipse/VS Code style)** — rejected for v1:
   weight and complexity beyond the ask; the registry + split-tree
   approach delivers composability with a fraction of the surface.
3. **Apps registry + split tree + registry-driven nav sections** — chosen.

## Rationale and Evidence

- herdr validates independent workspace/agent navigation with live status
  (its sidebar shows every agent pane with detected state).
- Clay already has the pieces to compose: transcript surface, files/
  memory/context inspector tabs, command center/composer, command palette,
  layout persistence (`layout_load`/`layout_save`), and package extension
  points roadmapped for Phase 4. The shell generalizes existing surfaces
  into registered apps rather than inventing new ones.
- Capability gating (plan 152 / ARI decision, same date) composes cleanly:
  the agent app's inspector contents and composer controls render from
  the active runtime's declared capabilities; the gear-tucked inspector is
  the gated surface.

## Consequences

- Frontend gains an `AppRegistry` (app id, title, icon, open-target
  semantics, capabilities), a `View` model (app instance + target: editor
  → file path, agent runtime → agent session), and a persisted split-tree
  layout (per workspace: activity tabs + visible splits).
- The Agents navigator consumes the detached-server running-agents
  inventory (detached-runtime decision, same date); workspaces remain the
  session-scoping unit inside the server.
- The command center/composer becomes a movable overlay component
  (default: bottom lane overlay across apps); package-provided shell
  composition (section replacement, overlay relocation) lands as Phase 4
  extension points — v1 ships the seams and the default view.
- The agent runtime app's info pane moves behind the gear (inspect or
  open-in-split); transcript-first matches the quiet-instrument language.
- Plans: `plans/151-Apps-and-Splits-Shell-Default-View.md` implements
  behind the mandatory prototype → user-approval → implementation gate;
  plan 152's prototype guidelines and UI tasks are updated to design
  within this shell (execution order 149 → 150 → 151 → 152 → 153).
- Revisit when: a second shell layout ships (then promote composition to
  a package-level surface), or split layout persistence proves costly
  across schema evolution.
