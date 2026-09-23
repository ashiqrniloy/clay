# Plan 151 — Apps-and-Splits Shell: Default View

Source: decision
`decision-logs/2026-09-23-1908-apps-and-splits-shell-default-view.md`
(user-decided, herdr-inspired). Clay's UI becomes an apps-and-splits
shell: left navigator with independent **Workspaces** and **Agents**
sections, main area hosting **apps** (editor, agent runtime, future
canvas/media) in user-controlled **splits**, workspace tabs as
**activities**, the composer/command palette as a **global bottom-lane
overlay**, and the agent info pane tucked behind a **gear icon**. The
layout is the *default view* over registry-driven seams so third-party
packages can later register apps, replace nav sections, and relocate
overlays (Phase 4 wiring; this plan ships the seams).

Depends on: plan 150 (running-agents inventory feeds the Agents
navigator; detached server keeps agents alive across GUI restarts — the
navigator's reason to exist). Plan 152's capability-gated agent-app UI
designs and implements inside this shell (execution order 146 → 147 →
148 → 149 → 150).

Binding prior decisions:

- `decision-logs/2026-09-23-1908-apps-and-splits-shell-default-view.md`:
  source.
- `decision-logs/2026-09-23-1908-detached-agent-runtime-server.md`:
  inventory + persistence this shell surfaces.
- `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`:
  agent runtime app is capability-gated (plan 152); one registry feeds the
  Agents navigator.
- `decision-logs/2026-09-14-1705-workspace-scoped-agent-sessions.md`:
  workspaces remain the scoping unit; the shell changes presentation, not
  session binding.
- Approved design language: `design-artifacts/approved/quiet-instrument-language/`,
  `agent-lane-palette/` — the shell composes these surfaces, it does not
  redesign them.

## Objectives

- Ship the shell primitives: `AppRegistry` (app id, title, icon,
  open-target semantics), `View` model (app instance + target), persisted
  **split tree** per workspace (activities tab strip + visible splits),
  and `openInSplit(app, target)` cross-app navigation.
- Ship the default view: left navigator with **Workspaces** (folder name
  by default, user-renamable, persisted) and **Agents** (cross-workspace
  inventory from plan 150 with working/blocked/idle status), editor app
  (file-path entry + file view), agent runtime app (activity view first;
  inspector tucked behind gear — inspect inline or open in split).
- Make the bottom lane (composer, command palette, mentions) a global
  overlay present in every app, including the editor.
- Expose composition seams (registry-driven nav sections, overlay slot,
  app registration) so Phase 4 packages can compose the shell without
  shell rewrites; document the seams as the extension contract draft.

## Expected Outcome

- The default view renders: navigator (Workspaces + Agents) | activity
  tabs + split grid | global bottom-lane overlay. A user can open a
  workspace, split the area, put the editor in one split and the agent
  runtime app in another, click a file in the agent app's Files tab and
  see the editor app open in a split with that file, and type a file path
  in the editor to load it — all with the composer overlay usable in
  both apps.
- The Agents section lists sessions across all workspaces with live
  status (working/blocked/idle), independent of which workspace is
  selected; clicking an agent navigates to its session (opening the
  agent runtime app in the current workspace context if the session
  belongs to another workspace).
- Layout persists per workspace across restarts; the shell composes the
  already-approved quiet-instrument surfaces without visual regressions
  (reviewed against the approved artifacts).

## Tasks

- [ ] Baseline: frontend surface inventory and primitive review
  - Acceptance Criteria:
    - Functional: recorded inventory of every surface the shell
      composes: transcript/agent-lane components, files/memory/context
      inspector, command center/composer + palette + mentions, editor
      surface, workspace/tab management, layout persistence
      (`layout_load`/`layout_save`), session reclaim/bootstrap glue.
    - Performance: none.
    - Code Quality: inventory is a primitive-first review per the
      mandatory duty — list what is reused as-is, re-parented, or newly
      built; no primitive duplicated (the shell must not fork a second
      transcript or composer).
    - Security: inventory notes which surfaces are package-reachable
      today (seams stay behind explicit extension points).
  - Approach:
    - Documentation Reviewed: `frontend/src/agent/`,
      `frontend/src/coding-agent/`, `frontend/src/editor/` (as
      applicable), `frontend/src/workspaces/` (as applicable),
      commands `layout_load`/`layout_save`.
    - Options Considered: none (mandatory inventory + primitive review —
      this plan creates reusable frontend architecture).
    - Chosen Approach: read + classify; every later task cites it.
    - Files to Create/Edit: none.
    - References: decision 1908; `references/clay.md` primitive review
      duty.
  - Test Cases to Write: none.

- [ ] Core model: AppRegistry, View, split tree, layout persistence
  - Acceptance Criteria:
    - Functional: typed registry (`AppId`, descriptor with title/icon/
      target semantics/capability flags); `View = { id, appId, target? }`;
      split tree (`SplitNode::Leaf(view) | Split::H/V[children]` with
      sizes); per-workspace activity list (open views) + split tree +
      active view; persisted via the existing layout machinery with
      schema versioning; `openInSplit(app, target)` resolves: existing
      view for target → focus, else new split (adjacent to focused leaf,
      default 50/50) or new activity when no split area exists.
    - Performance: registry lookups O(1); split resize re-renders only
      affected leaves; layout save debounced.
    - Code Quality: single source of truth in a small store (existing
      state-management pattern); pure reducer for layout ops
      (split/resize/close/move) fully unit-tested.
    - Security: persisted layouts sanitize targets (file paths stored as
      given; loading re-validates access at view mount, not at layout
      parse).
  - Approach:
    - Documentation Reviewed: existing layout persistence commands;
      existing frontend store pattern (agent state module).
    - Options Considered:
      - Off-the-shelf docking library — rejected: weight + styling
        lock-in contradict the quiet-instrument system; a minimal split
        tree is a few hundred lines with full control.
      - Minimal split tree + registry — chosen.
    - Chosen Approach: model-first, reducer-pure, persistence via
      existing commands (schema bumped).
    - API Notes and Examples:
      ```ts
      interface AppDescriptor {
        id: "editor" | "agent-runtime" | string; // package apps use dotted ids
        title: string; icon: IconRef;
        acceptsTarget?(target: ViewTarget): boolean;
        createView(target?: ViewTarget): View;
      }
      type SplitNode =
        | { kind: "leaf"; viewId: string }
        | { kind: "split"; dir: "h" | "v"; sizes: number[]; children: SplitNode[] };
      ```
    - Files to Create/Edit: `frontend/src/shell/` (registry, split tree,
      store, reducer), layout persistence glue.
    - References: decision 1908 (apps/views/splits concept).
  - Test Cases to Write:
    - Reducer: split/resize/close/focus/openInSplit (new vs existing
      target) property-tested against invariants (no orphan leaves, sizes
      sum to 1).
    - Layout save/load round-trip incl. schema-version bump.

- [ ] Shell chrome: navigator pane with Workspaces + Agents sections
  - Acceptance Criteria:
    - Functional: left pane renders registry-driven **sections**; defaults
      are Workspaces (list, folder-basename display names, user-rename
       persisted per workspace root) and Agents (plan 150 inventory:
      cross-workspace sessions with agent label, workspace badge, status
      chip working/blocked/idle, live updates); selecting a workspace
      switches the main area; selecting an agent focuses/opens its
      session (agent runtime app; cross-workspace sessions navigate to
      the owning workspace context).
    - Performance: navigator list virtualized beyond ~50 rows; inventory
      deltas update rows without re-rendering the pane.
    - Code Quality: section registry (`NavSectionDescriptor`) is the
      package seam — the default sections register through it like any
      package section would.
    - Security: workspace rename is display-only metadata (never mutates
      the folder); stored per-root in Clay-owned config.
  - Approach:
    - Documentation Reviewed: plan 150 inventory DTO; herdr sidebar
      (workspaces + agent panes with status at a glance);
      approved `agent-lane-palette` for chip/status styling.
    - Options Considered:
      - Hardcoded two sections — rejected: decision requires
        replaceable/extendable sections.
      - Section registry with two default registrations — chosen.
    - Chosen Approach: pane = ordered section slots; sections are
      self-contained components with data sources.
    - Files to Create/Edit: `frontend/src/shell/navigator/`, workspaces
      section (rename persistence), agents section (inventory
      subscription).
    - References: decision 1908; plan 150.
  - Test Cases to Write:
    - Rename persists and survives reload; folder basename default
      restores when rename cleared.
    - Inventory delta → correct row status transition (working → blocked
      → idle).

- [ ] Editor app (file-path entry + file view)
  - Acceptance Criteria:
    - Functional: editor app as a registered app: target = file path;
      view renders the existing editor surface for that file; a
      path-entry control (autocomplete from workspace files) opens a
      target; accepts `openInSplit` invocations from other apps.
    - Performance: file open within existing editor gates; split resize
      does not remount editor state unnecessarily (state preservation on
      layout change tested).
    - Code Quality: reuses the existing editor component tree; the app
      wrapper is thin (primitive review: no editor fork).
    - Security: path validation at mount (existing file-access rules);
      path entry rejects paths outside permitted roots with a visible
      typed error.
  - Approach:
    - Documentation Reviewed: existing editor surface + file-access
      rules (baseline inventory).
    - Options Considered: none beyond reuse.
    - Chosen Approach: thin wrapper app around existing editor.
    - Files to Create/Edit: `frontend/src/shell/apps/editor/`.
  - Test Cases to Write:
    - Open via path entry; open via openInSplit; outside-root path
      rejected.

- [ ] Agent runtime app (activity-first, gear-tucked inspector)
  - Acceptance Criteria:
    - Functional: registered app: target = agent session id; view renders
      the agent activity (transcript/lane) as the default full surface;
      a **gear control** opens the info pane (files/memory/context +
      future inspector tabs) as an inline drawer over the view or as a
      separate split ("Open in split"); inspector contents and composer
      controls render per plan 152 capability gating (until 152 lands,
      the native superset renders — the gating seam is the same accessor
      149 will consume).
    - Performance: transcript streaming unaffected by drawer open/close;
      drawer open does not remount the transcript (state preserved).
    - Code Quality: reuses existing inspector tab components unchanged;
      no second composer (the bottom-lane overlay is the composer).
    - Security: unchanged inspector data rules.
  - Approach:
    - Documentation Reviewed: existing agent lane + inspector components;
      decision 1908 (gear semantics); decision 1821 (capability gate).
    - Options Considered:
      - Keep side-by-side transcript+inspector as today — rejected by
        decision (transcript-first; inspector on demand).
    - Chosen Approach: activity-first layout + drawer/split inspector;
      wiring for capability gating lands with plan 152 against the same
      accessor.
    - Files to Create/Edit: `frontend/src/shell/apps/agent-runtime/`.
    - References: plan 152 (capability enum consumer); approved
      `quiet-instrument-language`.
  - Test Cases to Write:
    - Gear toggles drawer; "open in split" creates an inspector-split
      bound to the session; transcript state survives both.

- [ ] Global bottom-lane overlay
  - Acceptance Criteria:
    - Functional: composer + command palette + mentions render as an
      overlay docked to the shell bottom, present in every app view
      (editor included); overlay targets the **active split's context**
      (agent session in the focused agent-runtime view, or the active
      workspace's agent lane otherwise); keyboard summon unchanged.
    - Performance: overlay does not mount per-view (one instance,
      retargeted); summon latency within existing palette gates.
    - Code Quality: single composer instance (primitive review rule);
      target-resolution logic unit-tested.
    - Security: composer authority unchanged (it speaks to the bound
      session).
  - Approach:
    - Documentation Reviewed: existing command center/composer/palette
      components; `composer-palette-stages` approved artifact.
    - Options Considered:
      - Per-app composers — rejected: duplication + inconsistent
        behavior; decision says one global lane.
    - Chosen Approach: hoist the existing composer into the shell's
      overlay slot; active-view context binding.
    - Files to Create/Edit: `frontend/src/shell/` overlay slot; existing
      composer re-parented.
    - References: decision 1908.
  - Test Cases to Write:
    - Context retarget on split focus change; overlay visible in editor
      app; palette/mentions behavior unchanged (existing tests green).

- [ ] Prototype: default view + state matrix
  - Acceptance Criteria:
    - Functional: self-contained HTML prototype under
      `design-artifacts/prototypes/apps-and-splits-shell/` covering:
      default layout (navigator + tabs + splits + overlay), split states
      (1/2/3 splits, h+v nesting, resize affordances), navigator states
      (workspaces with rename affordance; agents idle/working/blocked,
      cross-workspace badge), agent app states (transcript-only; gear
      drawer; inspector-in-split), overlay states over editor and agent
      apps, empty states (no workspaces, no agents, no file open), and a
      package-composed variant (a replaced nav section + relocated
      overlay) proving the seams; narrow + wide; opens over `file://`,
      renders against the four shipped content themes.
    - Performance: none (prototype).
    - Code Quality: no build step; tokens per `references/tokens.md`,
      components per `references/components.md`; substantial new-surface
      design loads the four design skills per the plan gate.
    - Security: none.
  - Approach:
    - Documentation Reviewed: `DESIGN.md` (§14 especially),
      approved `quiet-instrument-language`, `agent-lane-palette`,
      `composer-palette-stages` artifacts; herdr sidebar for navigator
      inspiration.
    - Options Considered:
      - Implement directly from the decision text — rejected: mandatory
        prototype gate (new surfaces, new states).
    - Chosen Approach: variant prototype; user picks + requests changes.
    - Files to Create/Edit:
      `design-artifacts/prototypes/apps-and-splits-shell/**`.
    - References: prototype-gate duty in `references/clay.md`.
  - Test Cases to Write: none.

- [ ] Freeze approved shell artifact (explicit user approval)
  - Acceptance Criteria:
    - Functional: chosen variant copied to
      `design-artifacts/approved/apps-and-splits-shell/` with date,
      verbatim approval statement, chosen variant, requested changes;
      no chrome implementation task runs before this exists.
    - Performance: none.
    - Code Quality: append-only artifact.
    - Security: none.
  - Approach:
    - Documentation Reviewed: prototype gate freeze step.
    - Chosen Approach: present; record.
    - Files to Create/Edit:
      `design-artifacts/approved/apps-and-splits-shell/**`.
  - Test Cases to Write: none.

- [ ] Implement shell chrome citing the approved artifact
  - Acceptance Criteria:
    - Functional: default view implemented per the artifact: navigator
      (both sections), activity tabs, split grid with resize, overlay
      lane; behavior per the model tasks above; existing keyboard maps
      preserved or explicitly remapped (documented).
    - Performance: interaction latency within existing UI gates; split
      resize ≥ 60fps on the perf harness; no transcript streaming
      regression.
    - Code Quality: every chrome task cites the artifact path; reducer +
      registry seams exercised by unit tests.
    - Security: none beyond task-level rules.
  - Approach:
    - Documentation Reviewed: the approved artifact; `DESIGN.md` §14.
    - Chosen Approach: artifact-driven implementation of the model tasks
      already landed.
    - Files to Create/Edit: shell chrome components.
  - Test Cases to Write: component tests per surface; interaction tests
    (split, retarget, gear, openInSplit).

- [ ] Post-implementation visual and accessibility review
  - Acceptance Criteria:
    - Functional: running shell compared against the approved artifact;
      deviations recorded; keyboard traversal across navigator → tabs →
      splits → overlay works with a documented model; focus follows
      split changes sanely; screen-reader labels for nav sections,
      status chips, gear, splits.
    - Performance: part of review.
    - Code Quality: recorded in task evidence.
    - Security: none.
  - Approach:
    - Documentation Reviewed: planning-checklist review duty.
    - Chosen Approach: manual drill.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Extension-contract draft for shell composition (package seam docs)
  - Acceptance Criteria:
    - Functional: the seams (app registration, nav-section registration,
      overlay slot) are documented as a draft Phase 4 extension contract
      (no package loading yet — Phase 4 owns activation/approval);
      contract shipped with a built-in "package-style" registration
      proving a non-default section can replace a default one in a test.
    - Performance: none.
    - Code Quality: contract doc versioned under `docs/reference/`.
    - Security: the draft states that package shell composition will
      require explicit user approval at activation (Phase 4 pattern).
  - Approach:
    - Documentation Reviewed: Phase 4 extension-point decisions in
      `roadmap.md`; decision 1908 (replaceable sections).
    - Options Considered:
      - Ship seams undocumented — rejected: undocumented seams rot.
    - Chosen Approach: draft contract + test-registered replacement
      section.
    - Files to Create/Edit: `docs/reference/shell-extension-contract.md`
      (draft), test fixtures.
  - Test Cases to Write: replacement-section registration test.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: any new user-facing config this plan adds (e.g.
      workspace display-name store location, overlay defaults) is
      reflected exactly once in the example tree; init.js active part
      unchanged unless a loadable surface exists (none expected).
    - Code Quality: `node --check examples/config/init.js` passes.
    - Security: no trust implications.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Example Configuration
      Maintenance Task.
    - Chosen Approach: sweep + record.
    - Files to Create/Edit: as found.
  - Test Cases to Write: none.

- [ ] Example configuration live launch-test
  - Acceptance Criteria:
    - Functional: scratch-config GUI launch through the new default view:
      workspace opens, agents section populates from the inventory
      (plan 150), splits/overlay/gear drills run; evidence recorded.
    - Performance: within startup gates.
    - Code Quality: launch command + results recorded.
    - Security: scratch HOME only.
  - Approach:
    - Documentation Reviewed: Live Launch-Test duty.
    - Chosen Approach: manual drill.
    - Files to Create/Edit: none.
  - Test Cases to Write: none.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: app/nav-section registration stays internal Rust/TS
      API until Phase 4 defines package activation; any new JS-reachable
      op (none expected) follows the dotted-ID convention with docs.
    - Code Quality: doc-registry gates green.
    - Security: packages cannot register shell surfaces in this plan.
  - Approach:
    - Documentation Reviewed: `references/js-api.md`.
    - Chosen Approach: verification.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Code-wiki and documentation truth update
  - Acceptance Criteria:
    - Functional: wiki documents the shell architecture (registry, view
      model, split tree, overlay, seams), the default view, and the
      extension-contract draft; truth checks pass.
    - Code Quality: docs-as-code workflow.
    - Security: none.
  - Approach:
    - Documentation Reviewed: `references/docs-as-code.md`.
    - Chosen Approach: final wiki task.
    - Files to Create/Edit: `docs/wiki/`, `docs/reference/`.
  - Test Cases to Write: existing truth checks.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion. Known at planning time: Phase 4
  wiring for package-registered apps/sections/overlays; canvas/media app
  (first party or package) once the shell ships; navigator drag-reorder
  of workspaces; split presets/layouts beyond per-workspace persistence.
