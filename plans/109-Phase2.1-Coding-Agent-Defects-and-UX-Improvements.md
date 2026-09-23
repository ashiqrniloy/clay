# Phase 2.1: `@clay/coding-agent` Defect Fixes, Prism 0.5.0 Adoption, and UX Improvements

Source: user-reported defects and UX improvements against the completed
Phase 2 implementation (`plans/108-Phase2-Clay-Coding-Agent-Package.md`,
tasks 1–15 complete), plus an implementation review of the shipped coding
agent against the tool's intent (pi-parity coding agent inside Clay,
decision `2026-08-21-1758`). Execution stopped mid-plan: I1–I3 landed;
**I4 was blocked by Prism 0.4.0 thinking-level defects** (Clay report
`/home/arn/Projects/prism/2026-09-05-prism-thinking-level-gaps-report.md`:
Google silent no-op, Anthropic `thinking_type` 400, no declared per-model
level set). Prism **0.5.0** (released 2026-09-06, `docs/migrate-to-0.5.md`)
fixes that contract (`applyThinkingLevelForModel`, declared
`capabilities.thinkingLevels`, snap, Google/`output_config_effort` families)
and is a lockstep cut — Clay must adopt the whole 0.5.0 family, not only
the thinking helpers. Meanwhile `plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md`
tasks 1–15 landed: unified `ClayTabStrip`, recipe-consumption gates, and
legible neobrutal/glass chrome. Remaining 109 UI work binds to that catalog,
not the pre-110 hand-rolled tabs. Image support stays out (roadmap Phase 2.1
bullet, still deferred). Roadmap Phase 2.2 “Prism update” is absorbed here.

## Reported Issues (binding scope)

- **I1 — Workspace defect.** The agent always runs against the directory
  Clay was launched from. `ensure_tab_session` creates sessions with
  `workspace_root: None`, and the daemon defaults `workspaceRoot` to
  `process.cwd()` (`clay-agent/src/host.ts:628`). Changing the workspace in
  Clay never rebinds the agent session: all coding-agent operations keep
  targeting the launch directory.
- **I2 — No model auto-load.** Every coding-agent launch starts with no
  provider/model. A single global `book.json` trio
  (`profile`/`provider`/`model`) is persisted, but it is not per-workspace
  and does not reliably restore the last used model for the workspace the
  agent is opened in. Requirement: track the last used model per workspace
  and always load it automatically.
- **I3 — Model selection UX.** No `/model` command in the coding agent and
  no model dropdown in the panel. Requirement: a `/model` slash command
  that loads all models from all configured providers (grouped by
  provider) and lets the user select; the same list and selection logic
  from a dropdown in the coding-agent UI.
- **I4 — Reasoning effort invisible/unchangeable.** `Shift+Tab` cycles a
  frontend-local `effortIndex` that reaches no daemon field (the panel
  comment records this as a no-op for every current model). Requirement:
  visible + adjustable effort from the UI dropdown and a default
  `Shift+Tab` cycle, with the key binding user-configurable. Prism
  supports portable `ThinkingLevel` per run via Prism 0.5.0
  `applyThinkingLevelForModel` / `capabilities.thinkingLevels`
  (`docs/thinking-and-reasoning.md`). 0.4.0 blocked this (Google silent
  no-op, Anthropic `thinking_type` 400, no declared level set).
- **I5 — Transcript incomplete.** The chat window shows only the user's
  initial message and the agent's last message. Requirement: the
  transcript shows all agent activity chronologically — user messages,
  steering messages, skills read, tool calls with their outputs, model
  thinking/reasoning, errors, usage.
- **I6 — Workspace tree duplicated.** The workspace file tree renders both
  as the left tab and inside the Files tab of the coding-agent right pane
  (the tab embeds the full SDUI file browser). Requirement: the tree lives
  only in the left workspace tab; the Files tab is the editor view of Clay
  showing the file selected in the workspace; the left workspace tab gets
  a toggle key binding (default + user-configurable).
- **I7 — Context tab shows counts, not content.** Clicking a context
  category shows nothing. Requirement: clicking a category (e.g. System
  Prompt) opens a drawer with the actual content. The tab must show the
  *current* live context of the session — every active context item
  (user/steering messages, system prompt, loaded skills, tool calls and
  outputs, model thinking, compacted summaries) — and reflect compaction
  (compacted-away items disappear from active context; the summary
  appears).
- **I8 — Observational Memory tab inert.** Requirement: the tab records
  all Observer activity including dropped observations (reflection drops),
  live; and provides model selection for the Observational Memory workers
  through the UI, following the coding agent's model-selection logic, with
  a model that can differ from the session model, retained per workspace
  and per session.
- **I9 — No `/resume`.** Requirement: a `/resume` command listing all
  sessions of the current workspace; selecting one resumes it and restores
  the full content, current context, and model from that session.
- **I10 — Card detail placement.** Clicking a chat card renders details
  above the right-pane tabs. Requirement: a fourth right-pane tab
  **Session Info** (alongside Files, Observational Memory, Context) that
  shows the selected card's details; selecting a card auto-selects the
  Session Info tab.

## Implementation Review Findings (from the pre-plan review; fold into tasks)

- **R1 — Frontend slash-command list is hardcoded** (`SLASH_COMMANDS` in
  `frontend/src/coding-agent/CodingAgentPanel.tsx`) and will drift from the
  daemon-registered command set. Drive completion from the daemon command
  registry (Phase 1 dispatch surface) instead.
- **R2 — Status row git branch is a hardcoded placeholder** (`"git —"`).
  Show the real branch of the session's workspace root (bounded, cached).
- **R3 — Extension strip hardcodes `Extensions: core`**; it should reflect
  actual active extensions (persona packages when loaded) from daemon
  state, or omit the segment when unknown.
- **R4 — Tool/skill rows carry no content.** `clay.toolPhase` custom events
  carry only `name`/`phase`/`toolCallId`; transcript rows render
  `tool ${phase}` / `loaded skill (phase)`. Needed by I5: bounded argument
  and output digests plus the loaded skill name.
- **R5 — Side-channel tool list is capped at 8 and separate from the
  transcript** (`snapshot.tools`). Once tool calls are transcript entries
  (I5), the side list and the client-derived `toolStats` counters feeding
  the Context tab counts disappear in favor of server-authoritative data.
- **R6 — Recorded ceilings that stay:** transcript entry caps
  (`AGENT_MAX_SNAPSHOT_ENTRIES` 200 entries / 256 KiB text budget) and the
  trusted-module rendering model (host-rendered panel for the bundled
  package; third-party replacements render through generic SDUI). Not
  defects; documented.

Status: resumed after mid-execution stop (I1–I3 complete 2026-09-05).
Remaining work starts at the Prism 0.5.0 pin, then I4–I10 / R1–R3, then
closing tasks. Do not start I4 until the 0.5.0 consumer smoke is green.

Confirmed architecture (binding, inherited):

- All Phase 108 governing decisions apply unchanged:
  `2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`,
  `2026-08-21-2152-product-surfaces-are-replaceable-packages.md`,
  `2026-08-30-2156-adopt-prism-wiki-and-prism-graft-as-opt-in-knowledge-options.md`,
  `2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`,
  `2026-08-30-2158-observational-memory-defaults-worker-models-80k-per-session.md`,
  `2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`,
  `2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`,
  `2026-08-30-2201-workspace-scoped-session-search-shared-by-clay-and-st.md`,
  `2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md` (live pins
  superseded by this plan's 0.5.0 pin decision; 0.4 family/subpath rules
  stay),
  `2026-07-21-0001-two-package-runtime-trust-domains.md`,
  `2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`,
  `2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`,
  `2026-08-23-0052-tauri-react-client-architecture.md`,
  `2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`,
  `2026-08-28-2234-package-defined-ui-design-systems.md` (plan 110).
- New decision log required: exact `@arnilo/prism*@0.5.0` family pin set
  (same class as `2026-09-02-0121`). UX items I4–I10 still conform to
  logged decisions (workspace-scoped sessions = 2201; per-workspace model
  persistence = `configuration-system.md`; OM worker models = 2158).
  If implementation surfaces a genuinely new authority choice, stop and
  log it per `create-decision-log` before proceeding.

Project patterns: `planning-checklist.md`, `agent-host.md`,
`product-surfaces-are-packages.md`, `package-ui-layout.md`,
`package-runtime-trust-domains.md`, `authority-boundaries.md`,
`clay-js-api-naming.md`, `clay-js-api-boundary.md`,
`clay-js-api-schema.md`, `configuration-system.md`,
`package-manifest-single-source.md`, `behavior-manifests.md`,
`ui-skill-stack.md`, `ui-design-system-packages.md`,
`ui-modernization.md`, `ui-visual-review.md`, `tauri-react-client.md`,
`protocol-and-performance.md`, `documentation-as-code.md`,
`doc-registry-tests.md`, `maintenance-validation.md`.

Library docs:

- Prism 0.5.0 (local `/home/arn/Projects/prism`, authoritative — Context7
  has no `@arnilo/prism`):
  `docs/migrate-to-0.5.md` (lockstep cut: dead exports, MCP SDK v2,
  thinking wire moves, sqlite 13 / keyring 2, child-env allow-list),
  `docs/thinking-and-reasoning.md` (`applyThinkingLevelForModel`,
  `parseThinkingLevel`, `thinkingLevelsForModel`, `snapThinkingLevel`,
  `capabilities.thinkingLevels`, `compat.thinkingFamily`, families
  `google` / `output_config_effort`; legacy `applyThinkingLevel` still
  exported — do not use for new host code),
  `docs/mcp-tools.md` + `docs/migration.md` (MCP 2026-07-28, modular
  `@modelcontextprotocol/client` + `/server` 2.0.0),
  `docs/agent-session-runtime.md`, `docs/compaction-observational-memory.md`,
  `docs/session-stores-and-branching.md`, `docs/context-and-skills.md`.
- Plan 110 (implemented tasks 1–15): `ClayTabStrip` is the only `tabList`;
  recipe-consumption vitest; neobrutal/glass chrome recipes; Settings
  design-system selector. Remaining 109 UI consumes that catalog.
- Pi behavior reference: `@earendil-works/pi-coding-agent` README + docs
  under the installed npm prefix (`/model`, `/resume`, effort cycling,
  transcript composition semantics).
- Existing implementation inventory:
  `frontend/src/coding-agent/CodingAgentPanel.tsx` (already on
  `ClayTabStrip` + grouped `ClayDropdown` from I3/110),
  `frontend/src/components/tab-strip.tsx`,
  `frontend/src/agent/{state,events,TauriClayAgent}.ts`,
  `src/server/{agent,agent_agui,agent_picker}.rs`,
  `src/protocol/agent.rs`, `clay-agent/src/{host,rpc,providers,mcp}.ts`,
  `packages/coding-agent/dist/load.js`.

Not in scope: image support, `st` autonomy (Phases 5–7), cross-workspace
session search (stays workspace-scoped), OM live population for `st`
cadences beyond what the coding profile already attaches, external
runtimes (Phases 9–10), office/ACP/AG-UI/antigravity Prism packages.
Roadmap Phase 2.2 Prism update is this plan's 0.5.0 tasks, not a later
phase. Plan 110 leftover tasks 16–18 (wiki, tab-frame oversize, DS
reload deadlock) stay owned by 110; record them if they block visual
review, do not absorb them here.

## Objectives

- Adopt Prism 0.5.0 lockstep in `clay-agent` (exact family pins, MCP SDK
  v2, sqlite 13 / keyring 2, thinking adapter, hyper/commandcode
  adapters) before any remaining I4–I10 work. Log the pin set.
- Keep every original Phase 2.1 defect/UX task (I1–I10, R1–R5, JS API,
  configuration, manual test plan, visual/a11y review, wiki). I1–I3 stay
  complete; I4–I10 / R1–R3 / closing tasks stay required.
- Bind remaining coding-agent UI to plan 110: `ClayTabStrip`, cataloged
  `ClayDropdown`/`list`/`scroll`, recipe `--clay-ds-*` two-layer fallbacks,
  no new component kind, design-system consumption tests stay green.
- Make the coding agent workspace-correct: sessions bind to the tab's
  current workspace root, follow workspace changes, and all tools,
  knowledge bases, session search, and FTS metadata honor that root (I1).
- Make model selection persistent and fast: last-used model per workspace
  auto-loads on launch; `/model` slash command and panel dropdown list all
  models from all configured providers; selection persists per workspace
  and applies without a restart (I2, I3).
- Make reasoning effort a real control: daemon-side `ThinkingLevel` wiring
  per run, model-aware level list, status-row display, dropdown selection,
  `Shift+Tab` cycle default with a user-configurable binding (I4).
- Make the transcript a complete chronological record: every user/steer
  message, skill load (with name), tool call (with bounded argument and
  output content), thinking/reasoning, errors, and usage, rendered as
  uniform-height type-colored boxes with full content on selection (I5).
- Fix the right pane's information architecture: Files tab becomes the
  editor view of the workspace-selected file (tree only in the left
  workspace tab, toggle-bindable); Session Info becomes the fourth tab and
  the destination for selected chat cards (I6, I10).
- Make Context a real inspector: categorized live context — user/steering
  messages, system prompt, loaded skills, tool calls/outputs, thinking,
  compacted summaries — with per-item content in a drawer, reflecting
  compaction and context removal as it happens (I7).
- Make Observational Memory observable: the tab shows all Observer
  activity including dropped observations, and offers OM worker-model
  selection following the coding agent's model-selection logic, retained
  per workspace and per session, distinct from the session model (I8).
- Add `/resume`: workspace-scoped session list; resume restores content,
  current context, and the session's model (I9).
- Close review findings R1–R5 (daemon-sourced slash completion, real git
  branch, truthful extension strip, tool payloads in transcript,
  server-authoritative context counts).

## Expected Outcome

- `clay-agent` reports Prism `0.5.0`; `package.json` pins the seven
  adopted families at exact `0.5.0` plus `better-sqlite3@13.0.3`; lockfile
  has `@modelcontextprotocol/client` + `/server` `2.0.0` and no
  `@modelcontextprotocol/sdk` 1.30.0; existing SQLite session fixtures
  open with no schema migration; chat/coding mock flows stay compatible.
- Remaining I4–I10 / R1–R3 / closing tasks still ship the original
  defect/UX outcomes below, on the post-110 catalog (`ClayTabStrip`,
  recipe tokens) and the 0.5.0 thinking adapter.
- Open Clay in directory A, switch the workspace to B, launch the coding
  agent: every operation (shell, file tools, knowledge bases, session
  search, FTS metadata) runs against B; switching the workspace again
  rebinds the session; two tabs with different workspaces get independent
  sessions scoped to their roots.
- With provider(s) configured, launching the coding agent in workspace W
  immediately shows and uses the last model selected in W (or the global
  fallback on first use); `/model` and the dropdown show every model of
  every configured provider grouped by provider; picking one applies to
  the next run and survives restart.
- For a reasoning-capable model, the status row shows the active effort;
  the dropdown and `Shift+Tab` (or the user's rebound chord) change it;
  the changed effort reaches the provider request (verified via mock
  provider capture); non-reasoning models show no control and the cycle is
  a no-op.
- A multi-turn session with tool calls and a skill load renders the full
  chronological activity — user, thinking, tool call + bounded output,
  skill loaded (named), assistant, usage — every turn, with selection
  showing full content in the right pane's Session Info tab.
- The workspace tree appears exactly once (left tab, toggleable by a
  documented default binding and rebindable via `bindKey`); the Files tab
  shows the file selected in the workspace (editor view).
- The Context tab lists live categories with counts; clicking a category
  opens a drawer listing its items; clicking an item shows its content;
  after compaction the compacted-away items leave the active list and the
  summary item appears.
- The Observational Memory tab logs observe/reflect/drop activity for the
  session, and its model selector follows the `/model` logic with its own
  persisted per-workspace/per-session selection.
- `/resume` lists this workspace's sessions; selection restores transcript,
  context, and model.
- Linux gates pass: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, `clay-agent`
  `npm test`, frontend suites; docs/wiki/registry truth tests pass.

## Tasks

- [x] Verify Phase 2 completion state and green baseline before Phase 2.1 work (completed 2026-09-05)
  - Acceptance Criteria:
    - Functional: Plan 108 remains fully checked (15/15) with dated
      evidence; no Phase 2.1 task starts implementation before this gate
      records a green baseline on the current tree.
    - Performance: Baseline daemon startup and chat/coding round-trip
      unchanged from Phase 108 close recordings; no new daemon process,
      port, or startup work.
    - Code Quality: Baseline results recorded here; any failing gate is a
      blocking defect logged in Compromises/Further Actions, not worked
      around.
    - Security: Phase 1 acceptance policy, fail-closed knowledge CLIs, and
      allow-listed MCP unchanged; this plan adds no capability widenings.
  - Approach:
    - Documentation Reviewed:
      - `plans/108-Phase2-Clay-Coding-Agent-Package.md`: task states and
        completion evidence.
      - `.agents/skills/project-patterns/references/planning-checklist.md`.
    - Options Considered:
      - Trust plan-108 evidence and skip a fresh run: risks building on a
        tree that regressed after the close.
      - Fresh baseline first (chosen): single recorded gate before defect
        work, matching plan 108 task 1's pattern.
    - Chosen Approach:
      - Run `cargo fmt --check`, `cargo check --all-targets`,
        `cargo clippy --all-targets -- -D warnings`, full `cargo test`,
        `clay-agent` `npm ci && npm test`, and the frontend test suite;
        record results as this task's evidence.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && \
        cargo clippy --all-targets -- -D warnings && cargo test && \
        (cd clay-agent && npm ci && npm test) && \
        (cd frontend && npm test)
      ```
    - Files to Create/Edit:
      - None (verification only; blockers recorded in Compromises/Further
        Actions).
    - References:
      - `plans/108-Phase2-Clay-Coding-Agent-Package.md`.
  - Test Cases to Write:
    - Baseline run: all listed gates green and recorded with dates.
  - Evidence (2026-09-05):
    - Plan 108 state: all 15 core implementation tasks checked with dated
      evidence (lines 173–1749; completions dated 2026-09-03). The 7
      unchecked trailing tasks (visual review, UI/layout contract, Clay JS
      APIs, configuration APIs, `examples/init.js`, manual test plan,
      wiki) are covered 1:1 by this plan's closing tasks — consistent
      with the Phase 2.1 scope split.
    - Gates: `cargo fmt --check` OK; `cargo check --all-targets` OK;
      `cargo clippy --all-targets -- -D warnings` OK (0 warnings);
      `cargo test` 1241 passed / 0 failed (1195 + 6 + 40, 1 ignored);
      `clay-agent` `npm test` 71 passed / 0 failed / 1 skipped (72);
      frontend 30 files / 206 tests passed. Counts grew from the plan-108
      close recordings (68 daemon, 200 frontend) via post-108 additions;
      all green.
    - Performance: verification-only task — no code changed, so daemon
      startup, ports, and round-trip paths are unchanged by construction;
      no new daemon process, port, or startup work introduced.
    - Security: verification-only task — Phase 1 acceptance policy,
      fail-closed knowledge CLIs, and allow-listed MCP unchanged (their
      guard tests pass in the suites above); no capability widenings
      introduced.
    - Blockers: none; baseline is green, Phase 2.1 implementation may
      proceed.

- [x] Review UI catalog/primitives and confirm the generic surface plan before UI work (completed 2026-09-05)
  - Acceptance Criteria:
    - Functional: Inventory recorded against
      `.agents/skills/clay-ui/references/components.md`,
      `references/tokens.md`, `docs/reference/ui-components.md`, and the
      Phase 108 primitive inventory (G1–G7): for each Phase 2.1 UI element
      (model dropdown, effort dropdown, transcript tool/skill boxes,
      Session Info tab, Context drawer, OM activity list + model dropdown,
  `/resume` picker, Files-tab editor view), state which existing kind
      composes it (`dropdown`, `tabList`, `list`, `overlay`/drawer
      composition, `scroll`, `panel`) and confirm no new package-facing
      kind or Rust primitive is required, or record the minimal generic
      addition with catalog updates in the same change. The Files-tab
      editor view reuses the existing document/editor surface machinery
      (`PaneDocumentView`/`EditorSurface` precedent or the document
      store's content view) — no coding-agent-specific editor branch.
    - Performance: Drawer and dropdown rendering stays viewport-bounded;
      no per-keystroke or per-paint IPC beyond existing bounded queues;
      context/OM payloads ride typed bounded events, never SDUI trees.
    - Code Quality: No coding-agent-named Rust primitive, token, or
      anchor; trusted-module changes stay in the provenance-exact
      `CodingAgentPanel` precedent; third-party replacements keep
      rendering through generic SDUI.
    - Security: No new package authority; UI contributions stay inert
      declarations; keybinding additions go through the existing
      `bindKey` validation path only.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `docs/reference/ui-components.md`,
        `docs/wiki/modules/primitive-architecture.md`,
        `.agents/skills/project-patterns/references/package-ui-layout.md`,
        `.agents/skills/project-patterns/references/mode-primitive-first.md`.
    - Options Considered:
      - New drawer/inspector component kind: rejected unless inventory
        shows `overlay`/`panel` composition cannot reach the spec; a
        right-pane in-tab drawer is panel-local content, not a floating
        overlay.
      - Compose from catalog (chosen): `tabList` (fourth tab),
        `dropdown` (model/effort), `list` rows (context items, OM
        activity, session list), `scroll` + bounded text for drawers.
    - Chosen Approach:
      - Record the per-element mapping; implement UI tasks strictly on
      cataloged kinds and the trusted-module precedent.
    - API Notes and Examples:
      ```ts
      // Session Info joins the existing tabList; drawer is in-tab detail
      <Tab id="session-info">Session Info</Tab> // alongside files/memory/context
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-ui/references/components.md` (tentative: only
        if a generic addition is confirmed necessary).
      - `docs/reference/ui-components.md` (same condition).
    - References:
      - Plan 108 task 2 primitive inventory (G1–G7); Phase 20.7
        conformance rules in `components.md`.
  - Test Cases to Write:
    - Inventory check: every Phase 2.1 UI element maps to an existing
      kind or an explicitly planned generic addition; no
      coding-agent-named primitive appears.
  - Evidence — Primitive Inventory (2026-09-05, task complete):
    - Sources reviewed: `.agents/skills/clay-ui/references/components.md`
      (catalog current through Plan 108 G2/G3), `references/tokens.md`,
      `docs/reference/ui-components.md`, `docs/development/
      react-ui-catalog-mapping.md`, plan 108 task 2 inventory (G1–G7),
      `frontend/src/editor/ClayEditor.tsx` + `frontend/src/shell/
      PaneTree.tsx` (editor substrate verification).
    - Plan 108 gaps already closed: **G2** `tabList` kind implemented;
      **G3** `textInput` `multiline` implemented; **G1** pane-content
      activation + layout-intent application landed in 108. No reopened
      gaps.
    - Per-element mapping (all compose from implemented kinds; no new
      `ComponentKind`, style variable, token, overlay anchor, or Rust
      primitive):
      | Element | Composition |
      |---|---|
      | Model dropdown (panel) | `dropdown` kind; grouped-by-provider items rendered as group-header rows (trusted module, Command Centre ListBox precedent); selection emits the existing inert picker-selection intent |
      | Effort dropdown + status display | `dropdown` + `label`/`statusItem` with `typography.status` |
      | Transcript tool/skill boxes | Trusted-module transcript rows (ChatPanel memoized-row precedent); uniform-height truncation host-side (108 G4); type coloring via typed style variables (`borderColor` + color-role tokens); payloads ride the bounded AG-UI stream, never SDUI trees (4096 B snapshot / 16 KiB tree budgets) |
      | Session Info tab | Existing `tabList` kind (108 G2), fourth item; detail = bounded text + `list`/`scroll` in the trusted module |
      | Context drawer | In-tab drawer region: widget-local selection state composing `list` + `scroll` + `label` (not `modal`, no floating overlay); category → items → item detail |
      | OM activity list + model dropdowns | `list` + `scroll` + `dropdown` |
      | `/resume` picker | Server-owned centered Command Centre picker session (precedent: `agent.clientOpen{Agent,Provider,Model}Picker`); no new UI kind |
      | Files-tab editor view | Reuse the `ClayEditor`/`DocumentSession` substrate (one EditorView per mount; pane = generic content host — `PaneTree.tsx` precedent); bound to the tab's selected document; no coding-agent editor branch (updates 108 G5 for the React target: direct `ClayEditor` reuse instead of the native snapshot path) |
    - Performance: dropdown/tab selection is widget-local (no server
      round-trip); transcript/drawer rendering viewport-bounded; context
      and OM payloads ride typed bounded events; slash completion consumes
      the state-carried command list (no per-keystroke IPC).
    - Code quality: no coding-agent-named Rust primitive, token, or
      anchor; trusted-module changes stay in the provenance-exact
      `CodingAgentPanel` precedent; third-party replacements render the
      unchanged generic SDUI tree.
    - Security: no new package authority; UI contributions stay inert
      command intents; the effort-cycle chord goes through the existing
      `bindKey` validation path (client command id, Task 14).
    - Conclusion: catalog untouched — zero generic additions required.
      If implementation surfaces a gap, catalog/docs updates land in the
      same change per the catalog's update rules.

- [x] I1: Bind agent sessions to the tab's current workspace and follow workspace changes (completed 2026-09-05)
  - Acceptance Criteria:
    - Functional: `ensure_tab_session` resolves the tab's current
      workspace root (tab workspace state, `layout_persist` /
      workspace-controller source of truth) and passes it as
      `workspace_root` on `NewSession`; the daemon `session.new` uses it
      (already supported — `host.ts:628`) so tools, acceptance-policy
      workspace, knowledge bases, and FTS metadata
      (`SESSION_SEARCH_WORKSPACE_METADATA_KEY`) all target that root.
      When the user changes the workspace in a tab, the tab's agent
      session rebinds to the new root (new session for the new workspace
      or explicit rebind; old session stays resumable). Two tabs with
      different workspaces hold independent, correctly-scoped sessions.
      Launch-directory `process.cwd()` remains only the fallback when no
      workspace is open.
    - Performance: Workspace-root resolution is a cached lookup, not a
      filesystem probe per prompt; no polling; rebind is event-driven off
      the existing workspace-change path.
    - Code Quality: Workspace identity reuses the existing tab workspace
      state — no parallel workspace registry; multi-tab routing ceiling
      recorded in plan 108 task 4 is resolved here or explicitly rescoped
      with a `ponytail:`-style ceiling note.
    - Security: The acceptance policy's workspace boundary (decision
      2157) now actually tracks the UI workspace; no broadened authority —
      the agent gains nothing beyond the already-granted root, it stops
      silently running in the wrong one.
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs` (`ensure_tab_session`, `begin_prompt`,
        `resume_tab`), `src/shell/layout_persist.rs` (tab workspace
        state), `frontend/src/shell/workspace-controller.ts`,
        `clay-agent/src/host.ts:628-690` (session.new workspaceRoot),
        `.agents/skills/project-patterns/references/agent-host.md`,
        `decision-logs/2026-08-30-2201-workspace-scoped-session-search-shared-by-clay-and-st.md`.
    - Options Considered:
      - Daemon-side cwd juggling or a workspace param on every prompt:
      scattered; every tool call would need re-validation.
      - Session creation carries the root + rebind on workspace change
      (chosen): the root is immutable per Prism session metadata — a
      workspace change maps to a session switch, which the book already
      models per tab.
    - Chosen Approach:
      - Resolve tab workspace root server-side at `ensure_tab_session`;
      on workspace change, mark the tab's session binding stale so the
      next interaction creates/rebinds a session for the new root,
      persisting the transcript of the old one; surface the active root in
      the status row (it already receives `workspaceRoot`).
    - API Notes and Examples:
      ```rust
      // ensure_tab_session: resolve root from the tab's workspace state
      let workspace_root = self.tab_workspace_root(tab); // cached
      ... NewSession { profile, provider, model,
          workspace_root: workspace_root.map(str::to_string), .. }
      ```
    - Files to Create/Edit:
      - `src/server/agent.rs`: tab→workspace resolution, rebind-on-change,
        `NewSession` passthrough.
      - `src/shell/layout_persist.rs` or workspace-controller seam (only
        if the root lookup needs a new accessor).
      - `clay-agent/src/host.ts`: no change expected (`workspaceRoot`
        already honored); verify FTS metadata key uses it.
    - References:
      - `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`;
        plan 108 task 4 (NewSession passthrough), task 11 (FTS workspace
        metadata).
  - Test Cases to Write:
    - Workspace switch drill: launch in A, switch to B, prompt → daemon
      session metadata/workspace tools target B; switch back → A session
      resumes with its transcript.
    - Two-tab isolation: tabs on different roots keep independent
      correctly-scoped sessions.
    - FTS scoping: sessions created under B never appear in A's
      workspace-scoped search.
  - Evidence (2026-09-05, implemented):
    - Mechanism: the server tab registry (Phase 22.3, `src/server/
      tab_registry.rs`) is the single source of truth — each tab already
      carries `workspace_root` (tab = workspace client, 1:1;
      `TabRuntime.workspaceRoot` client-side, `open_workspace` rebinds
      server-side). `AgentHost` gained `set_tab_registry` (installed by
      `ServerState` construction, `src/server/mod.rs`) and an async
      `tab_workspace_root(tab)` cached in-memory lookup. Rebind is
      state-comparison at session acquisition (`SessionBook::
      session_for_root`): a tab whose registry root no longer matches its
      session's recorded root drops the stale binding on the next
      interaction and creates a session for the new root — no listener
      wiring, no polling, old session stays resumable from `transcripts`.
      Pre-I1 bindings (no recorded root) stay stable when no registry
      resolves, so legacy sessions and registry-less test hosts never
      surprise-rebind.
    - End-to-end chain: `begin_prompt(tab)` (tab id = registry-bound
      `bound_tab_id`, `connection/runtime.rs:662`) → `ensure_tab_session`
      resolves the root and passes `workspace_root: Some(root)` on
      `NewSession` → daemon `host.ts:628` uses it for the Prism session
      and stamps FTS metadata (`host.ts:659`, plan 108 task 11) so
      tools/KBs/session search all target that root. Only one session
      creator exists (`ensure_tab_session`); launch-directory cwd remains
      the daemon fallback when no root is passed.
    - Tests added (`src/server/agent.rs` `tab_workspace_tests`):
      session kept while root matches; legacy unbound session survives
      missing registry; root change clears the stale binding for rebind
      (old session resumable); bound session rebinds when the registry
      disappears; host resolves tab root from registry and follows
      `open_workspace` rebinds. Gates: `cargo fmt` OK, `cargo check
      --all-targets` OK, `cargo clippy --all-targets -- -D warnings` OK,
      `cargo test` 1246 passed / 0 failed (+5), clay-agent 71 pass /
      0 fail (JS unchanged). Graft graph refreshed (`graft build`).
    - Remaining drills for the manual test plan task: live two-tab
      isolation + FTS scoping against a real daemon (unit tests pin the
      binding and resolution logic; the daemon-side root handling was
      covered by plan 108 task 4 tests).

- [x] I2: Persist and auto-load the last used model per workspace (completed 2026-09-05)
  - Acceptance Criteria:
    - Functional: The persisted book selection becomes per-workspace: a
      workspace-root-keyed selection map (`profile`/`provider`/`model`,
      plus the OM worker-model fields from the I8 task) with the existing
      global trio as fallback for first use in a workspace. On coding
      agent launch (or workspace rebind), the tab's session uses the
      per-workspace selection automatically — the panel never shows
      "Configure a provider" when a valid configured selection exists for
      the workspace. Picking a model in any surface (picker, `/model`,
      dropdown) writes the workspace-keyed selection. Migration: an
      existing global `book.json` loads as the fallback entry.
    - Performance: Selection persistence is the existing best-effort
      write path (no fsync barrier); selection resolution is an in-memory
      map lookup per session creation.
    - Code Quality: Single persistence format, single source of truth
      (pattern `package-manifest-single-source` mindset applied to the
      book); invalid/unconfigured selections fall back to the global trio,
      never to a broken session.
    - Security: The selection file is host-owned config data in the
      existing data dir; no credential material moves (provider ids only);
      unconfigured providers are filtered exactly as the picker filters
      today.
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs` (`SessionBook`, `load_persisted_book`,
        `persist_book`, `select_picker`, `persist_book_selection`),
        `clay-agent/src/host.ts:203-247` (session-record model
        persistence), `.agents/skills/project-patterns/references/configuration-system.md`,
        `.agents/skills/project-patterns/references/agent-host.md`.
    - Options Considered:
      - Daemon-side per-workspace settings store: second store for data
      the Rust book already owns; rejected.
      - Extend the Rust book to a workspace-keyed map with global fallback
      (chosen): one file, one owner, tab workspace root already flows from
      the I1 task.
    - Chosen Approach:
      - `book.json` v2: `{ fallback: {profile, provider, model},
        workspaces: { "<root>": {profile, provider, model, ...} } }`;
      v1 payloads load as `fallback`. Resolution order: workspace entry →
      fallback → unconfigured.
    - API Notes and Examples:
      ```rust
      fn selection_for(book: &SessionBook, root: Option<&str>) -> (String, String, String) {
          root.and_then(|r| book.workspaces.get(r))
              .map(|s| (s.profile.clone(), s.provider.clone(), s.model.clone()))
              .filter(|(_, p, _)| provider_configured(p))
              .unwrap_or_else(|| (book.fallback.profile.clone(), ...))
      }
      ```
    - Files to Create/Edit:
      - `src/server/agent.rs`: book shape v2, load/persist/resolve,
        `select_picker` and every model-selection entry point write the
        workspace key.
      - `tests/` (agent suite): v1 migration, per-workspace resolution,
        fallback-on-unconfigured.
    - References:
      - Plan 108 task 9 (provider/model switch, agent-rebuild); decision
        2158 (separate worker models precedent).
  - Test Cases to Write:
    - Restart drill: select model M in workspace W, restart Clay, open the
      agent in W → M loaded and used; a different workspace keeps its own
      selection.
    - Migration: pre-v2 `book.json` loads; behavior unchanged for
      single-workspace users.
    - Unconfigured fallback: workspace selection's provider loses its
      credential → falls back to global; no broken session.
  - Evidence (2026-09-05, implemented):
    - Book v2 (`src/server/agent.rs`): new `BookSelection`
      {profile, provider, model}; `SessionBook` gains `workspaces:
      HashMap<String, BookSelection>` keyed by workspace root. `persist_book`
      writes `{fallback: {…}, workspaces: {…}}`; `load_persisted_book`
      sniffs the `fallback` key for v2 and loads a flat v1 payload as the
      fallback entry — migration is read-side, no writer upgrade step.
    - Resolution: `selection_for(book, root, is_configured)` — the
      workspace's entry when its provider is non-empty and configured,
      else the global trio. The configured check runs against the daemon
      provider inventory (`picker_inventory()`), fetched only on actual
      session creation (never on the cached-session path); a fully-empty
      book short-circuits before the inventory RPC. The run-scoped
      override in `begin_prompt` (plan 108 task 9) re-resolves through the
      same `selection_for` with the tab's root, so picker switches made
      between runs apply at the next prompt per workspace.
    - Writes: `select_picker(kind, id, tab)` gained the bound tab; a
      selection made while a tab is bound to a workspace upserts that
      workspace's entry from the current trio (after `ensure_default_model`,
      so provider picks capture the defaulted model). Callers pass
      `bound_tab_id` from the picker-menu handler (`connection/menus.rs`) —
      both Provider and Model/Agent selects; the launch-surface profile
      auto-select (`connection/runtime.rs`) passes `None` and writes only
      the global fallback. I3's `/model` command and dropdown will route
      through this same path.
    - Launch auto-load: `ensure_tab_session` resolves the workspace
      selection and passes it to `NewSession` — the panel gets a session
      snapshot carrying the applied provider/model, so no "Configure a
      provider" when a valid selection exists for the workspace.
    - Tests added (`src/server/agent.rs`): v1 book.json loads as fallback
      (no workspace entries); v2 round-trip (persist→reload preserves
      values); workspace selection wins over global; unknown root/no-root
      fall back to global; unconfigured workspace provider falls back to
      global; picker selection with a bound tab writes the workspace key
      while a no-tab selection writes only the fallback. Gates: `cargo
      fmt` OK, `cargo check --all-targets` OK, `cargo clippy --all-targets
      -- -D warnings` OK, `cargo test` 1251 passed / 0 failed (+5),
      clay-agent 71 pass / 0 fail (JS unchanged). Graft refreshed.
    - Remaining drill for the manual test plan task: live restart drill
      against a real daemon (select M in W, restart Clay, reopen W → M
      used; second workspace keeps its own selection).

- [x] I3: Add `/model` command and panel model dropdown over all configured providers' models (completed 2026-09-05)
  - Acceptance Criteria:
    - Functional: A daemon-dispatched `/model` slash command in the coding
      agent opens a model picker listing every model of every *configured*
      provider, grouped by provider, reusing the existing inventory
      (`AgentPickerKind::Model` already filters configured providers);
      selecting applies through the same book path as the I2 task
      (per-workspace persistence, next-run application, session-rebuild
      semantics from plan 108 task 9). The panel header/status area gets a
      `dropdown` with the same grouped items and the same selection
      logic. Both surfaces are the same selection path — no second model
      registry.
    - Performance: Dropdown/picker data is the already-bounded inventory;
      no per-open daemon round-trip beyond the existing inventory fetch;
      grouping is client-side sort of bounded arrays.
    - Code Quality: Grouped list derived in one place (picker items
      extension), consumed by Command Centre picker and the panel
      dropdown; slash completion entries come from the daemon command
      registry (fixes R1 for this command).
    - Security: Only configured providers' models are listed (existing
      filter); selection grants no new authority; picker/dropdown emit the
      existing inert command intents.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `src/server/agent.rs` (`picker_items`, command dispatch),
        `frontend/src/command-centre/CommandCentre.tsx` (picker session
        precedent), pi `/model` reference docs.
    - Options Considered:
      - Flat `provider/model` list without grouping: harder to scan with
      16 adapters; pi groups by provider.
      - Grouped-by-provider list (chosen): matches pi; groups are headers,
      items are the existing `AgentPickerItem` ids.
    - Chosen Approach:
      - Daemon: register `/model` command that opens the existing model
      picker session (Command Centre) with grouped items; frontend: panel
      dropdown (`ClayDropdown`-equivalent cataloged kind) bound to the
      same inventory state; both selections dispatch the existing
      `model:` picker-selection intent.
    - API Notes and Examples:
      ```ts
      // Same items, two surfaces
      const grouped = groupBy(inventory.models.filter(m =>
        configured.has(m.provider)), m => m.provider);
      // selection → existing select_picker("model:provider/name") path
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/dist/load.js` + `package.json`:
        `/model` command registration (manifest commands list).
      - `src/server/agent.rs`: grouped picker items (add provider group to
        `AgentPickerItem` or render-group field).
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: model dropdown.
      - `frontend/src/coding-agent/coding-agent.module.css`: token-driven
        dropdown styles only.
    - References:
      - Plan 108 task 9 (picker switch semantics); pi `/model` behavior.
  - Test Cases to Write:
    - `/model` lists all configured providers' models grouped; unconfigured
      providers absent; selection applies to the next run and persists per
      workspace (with I2).
    - Dropdown selection matches picker selection (same id, same state
      change).
  - Evidence (2026-09-05, implemented):
    - `/model` (panel): added to the composer's `SLASH_COMMANDS` (completion
      + description) with a client-side intercept in `submit` that dispatches
      the core built-in intent `agent.clientOpenModelPicker` — the daemon's
      existing picker flow (Command Centre transient menu, grouped configured
      models, `picker_items` filtered to configured providers). No new model
      registry; the package deliberately does not re-register the command
      (`load.js` comment + `packages/coding-agent/docs/index.md` already
      document `/model` as the core built-in hook — plan's anticipated
      manifest edit was unnecessary; R1's registry-sourced completion list
      remains the later R1 task).
    - Panel dropdown (`frontend/src/coding-agent/CodingAgentPanel.tsx`):
      header now carries a `ClayDropdown` bound to the same inventory state
      the picker consumes — `snapshot.state["models"]`/`["providers"]` from
      the existing `listSessions` → `Inventory` → STATE_SNAPSHOT path
      (`agent_agui.rs inventory_state`), zero new fetches. Grouping derives
      in ONE place (`groupModelsByProvider`, exported + unit-tested):
      configured providers only, groups in inventory order, ids exactly
      `model:provider/name` — the same id the picker emits. Active item from
      `state.provider`/`state.model`, refreshed by book snapshots after
      every selection or session creation.
    - Selection path: dropdown emits the existing AG-UI
      `Select { kind: "model", id }` command; `connection/mod.rs` routes
      Model/Provider/Agent selects through `select_picker(kind, id, tab)`
      with the connection's bound tab resolved from the tab registry
      (`tab_for_client`) — so I2's per-workspace persistence, next-run
      application, and plan 108 task 9 session-rebuild semantics all apply.
      Other picker kinds keep the previous dispatch behavior. (The prior
      `Select` arm was a no-op diagnostic; nothing depended on it.)
    - Catalog: `ClayDropdown` gained an optional `groups` prop (React Aria
      `Section`/`Header`, token-driven header styles in
      `controls.module.css`); backward compatible, catalog
      (`components.md`) updated in the same change.
    - Tests added: `groupModelsByProvider` grouping unit (order, unconfigured
      filter, display-name fallback); panel `/model` submit dispatches the
      picker intent (works unconfigured); dropdown renders the active model
      label from inventory state. Gates: `cargo fmt`/`check`/`clippy -D
      warnings` OK, `cargo test` 1251 passed / 0 failed, frontend `tsc`
      clean, vitest 209 passed / 0 failed (+3), clay-agent unchanged.
      Graft refreshed.
    - Remaining drill for the manual test plan task: live `/model` +
      dropdown selection against a real daemon — list matches Command
      Centre picker, selection applies next run and persists per workspace.

- [x] Record the Prism 0.5.0 clay-agent family pin decision (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: A new `decision-logs/YYYY-MM-DD-HHMM-prism-0.5.0-clay-agent-family-pins.md` records exact pins for `@arnilo/prism`, `@arnilo/prism-core`, `@arnilo/prism-providers`, `@arnilo/prism-coding-tools`, `@arnilo/prism-web-tools`, `@arnilo/prism-memory`, `@arnilo/prism-mcp` at `0.5.0`, direct `better-sqlite3@13.0.3`, subpath-only imports, hyper/commandcode explicit load, MCP via `prism-mcp` public API only (no direct SDK import), no office/ACP/AG-UI/antigravity packages. User already directed this adoption; log cites this plan and `docs/migrate-to-0.5.md`.
    - Performance: Decision adds no runtime work.
    - Code Quality: Follows `create-decision-log`; supersedes live pins in `2026-09-02-0121` without rewriting that log; updates `.agents/skills/project-patterns/references/agent-host.md` pin sentence after the log exists.
    - Security: No new package/daemon authority; MCP remains allow-listed; credentials stay vault+keychain.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md`, `CHANGELOG.md` `[0.5.0]`, `decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md`, `.agents/skills/create-decision-log/SKILL.md`, `.agents/skills/project-patterns/references/agent-host.md`.
    - Options Considered:
      - Stay on 0.4.0 and patch thinking in Clay: rejected — 0.4 thinking is the I4 blocker; 0.5 is the upstream fix and a lockstep cut.
      - Range pins `^0.5.0`: rejected — same exact-pin rule as 0121.
      - Exact 0.5.0 family pins (chosen), same shape as 0121 plus Phase 1 families already in the tree.
    - Chosen Approach:
      - Write the log, then update `agent-host.md` pin text only.
    - API Notes and Examples:
      ```text
      decision-logs/YYYY-MM-DD-HHMM-prism-0.5.0-clay-agent-family-pins.md
      ```
    - Files to Create/Edit:
      - `decision-logs/*-prism-0.5.0-clay-agent-family-pins.md`: new log.
      - `.agents/skills/project-patterns/references/agent-host.md`: 0.5.0 pin sentence.
    - References:
      - Prism `docs/migrate-to-0.5.md`; decision `2026-09-02-0121`.
  - Evidence (2026-09-06, implemented):
    - Log written: `decision-logs/2026-09-06-0432-prism-0.5.0-clay-agent-family-pins.md`
      — exact seven-family `0.5.0` pins + `better-sqlite3@13.0.3`; subpath-only
      imports; hyper/commandcode explicit loads; MCP via `@arnilo/prism-mcp`
      public API only (no SDK v2 direct imports); no office/ACP/AG-UI;
      antigravity removed upstream (Phase 10 direct `agy`). Cites this plan,
      `migrate-to-0.5.md` (sections 1–7), `CHANGELOG.md` `[0.5.0]`,
      `thinking-and-reasoning.md`, `mcp-tools.md`, the 0.4 pin log 0121, and
      the thinking-gaps report. Approval from user's adoption directive
      (`proposed_by: both`, approved: yes).
    - Reference updated: `.agents/skills/project-patterns/references/agent-host.md`
      pin bullet now states live pins = seven families at exact `0.5.0` +
      `better-sqlite3@13.0.3`, MCP via `prism-mcp` public API only, no retired
      0.3 names / 0.5 dead exports; cites 0432 log alongside 0121.
    - Verification: `docs/migrate-to-0.5.md` headings confirm 27 dead exports,
      MCP SDK v2 (client+server 2.0.0), tenant-aware factories, child-env
      allow-list, keyring 2 typed errors, `applyThinkingLevelForModel` surface;
      CHANGELOG confirms antigravity workspace removal; 10 publishable
      manifests at the lockstep cut.
  - Test Cases to Write:
    - None (decision record). Pin tests live in the adopt task.

- [x] Re-verify green baseline after plan 110 and before the 0.5.0 pin (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Record a fresh Linux gate run on the current tree (plan 110 tasks 1–15 landed; I1–I3 already in). No 0.5.0 or I4 edit starts until this gate is green or blockers are listed in Compromises.
    - Performance: Verification-only; no new daemon/port/startup work.
    - Code Quality: Record counts; do not weaken failing gates.
    - Security: No capability change.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/planning-checklist.md`, `plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md` (tasks 16–18 leftovers).
    - Options Considered:
      - Trust 2026-09-05 I1–I3 evidence: stale after 110.
      - Fresh run (chosen).
    - Chosen Approach:
      - Same command set as the original baseline task.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && \
        cargo clippy --all-targets -- -D warnings && cargo test && \
        (cd clay-agent && npm ci && npm test) && \
        (cd frontend && npm test)
      ```
    - Files to Create/Edit:
      - None (verification only).
    - References:
      - This plan's first baseline evidence (2026-09-05).
  - Evidence (2026-09-06, verified green):
    - Tree state: working tree carries plan 110 tasks 1–15 + I1–I3 (231 dirty
      files, none from this plan's edits: decision log 0432 + agent-host.md
      committed-equivalent in-tree). Baseline tested against the dirty tree
      as-is — that is the tree the 0.5.0 pin will land on.
    - Rust: `cargo fmt --check` clean; `cargo check --all-targets` clean on a
      no-incremental rebuild (initial incremental run briefly showed one
      unused-import warning in `clay` lib; a fresh rebuild is warning-free —
      stale incremental artifact, not current source); `cargo clippy
      --all-targets -- -D warnings` clean; `cargo test` ok — 1671 passed,
      0 failed, 3 ignored (1210 lib + 6 + 41 + 207 + 73 + 135 suites).
    - clay-agent: `npm ci` clean install; `npm test` ok — 71 pass, 1 skipped,
      0 fail (72 tests, vitest). `npm audit` reports 1 high-severity
      transitive vuln; unchanged from pre-110 (re-evaluate after the
      `better-sqlite3@13.0.3` bump in the adopt task — expected to replace
      the flagged 12.x resolution).
    - frontend: `npm test` ok — 33 files, 238 passed, 0 failed (includes
      editor perf invariants + plan 110 recipe-consumption tests).
    - Conclusion: baseline green exactly once, on the current tree; 0.5.0
      pin and I4 may proceed. No blockers recorded in Compromises.
  - Test Cases to Write:
    - Baseline run: all listed gates green and dated, or blockers recorded.

- [x] Adopt Prism 0.5.0 family pins, peers, imports, and consumer smoke (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: `clay-agent/package.json` + lockfile pin exact `0.5.0` for the seven adopted families; `better-sqlite3` exact `13.0.3` (`allowScripts` key updated); startup `prism` field is `0.5.0`; `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp` asserts the new pins and still denies ACP/AG-UI/retired 0.3 names; no 0.4/0.5 mix; no retired 0.3 names; no `@arnilo/prism-office`, `prism-acp-agent`, `prism-ag-ui`, `prism-antigravity-agent`. Compile against 0.5: zero imports of the 27 removed exports (`docs/migrate-to-0.5.md` §3). Existing SQLite session fixtures open and round-trip with no schema migration. Keychain: locked/unavailable stores surface typed Prism errors (`CredentialStoreLockedError` / `CredentialStoreUnavailableError`) and are not treated as an empty vault; missing credential still `undefined`. Durable sqlite factory: if 0.5 requires explicit tenant on construct, pass existing `TENANT = "clay"`; current session rows already tenanted. Child processes (MCP stdio, Obscura): do not rely on ambient `process.env`; omitted env uses Prism `buildChildEnv` allow-list; Clay allow-list `env` map still passed through. Providers: keep the current 16 explicit loads; add `createHyperProviderPackage` and `createCommandCodeProviderPackage` the same way; Azure/Bedrock/Vertex stay host-config stubs. Do not import `/document-reader` or add `pdf-parse` (optional peer only). `clay-agent/README.md` version strings match.
    - Performance: `npm ci` + `npm test` in `clay-agent` on Linux; native `better-sqlite3` 13 prebuilds (rebuild only if install scripts blocked). No extra daemon process.
    - Code Quality: Subpath imports only; family install activates nothing extra. Isolation: three-family-plus-coding graph does not open Postgres/NATS/browsers/office.
    - Security: Vault+keychain unchanged except locked-store honesty; no `process.env` credentials; MCP still deny-by-default allow-list; no new package spawn.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.5.md` (all seven sections + upgrade steps), `CHANGELOG.md` `[0.5.0]`, `docs/thinking-and-reasoning.md`, `clay-agent/package.json`, `clay-agent/src/{host,providers,main,mcp,obscura}.ts`, `tests/agent_protocol.rs`.
    - Options Considered:
      - Thinking-only bump of `@arnilo/prism`: rejected — 0.5 is lockstep; peers are `^0.5.0`.
      - Full seven-family exact pin + consumer smoke (chosen).
    - Chosen Approach:
      - One atomic pin + import compile + fixture smoke. MCP protocol details in the next task; thinking UI in I4.
    - API Notes and Examples:
      ```json
      "@arnilo/prism": "0.5.0",
      "@arnilo/prism-core": "0.5.0",
      "@arnilo/prism-providers": "0.5.0",
      "@arnilo/prism-coding-tools": "0.5.0",
      "@arnilo/prism-web-tools": "0.5.0",
      "@arnilo/prism-memory": "0.5.0",
      "@arnilo/prism-mcp": "0.5.0",
      "better-sqlite3": "13.0.3"
      ```
      ```ts
      import { createHyperProviderPackage } from "@arnilo/prism-providers/hyper";
      import { createCommandCodeProviderPackage } from "@arnilo/prism-providers/commandcode";
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`, `clay-agent/package-lock.json`, `clay-agent/README.md`.
      - `clay-agent/src/{main,providers,host}.ts`: version string, adapters, tenant/keychain if required.
      - `tests/agent_protocol.rs`: pin assertions `0.5.0` + `better-sqlite3` `13.0.3`.
    - References:
      - Prism `docs/migrate-to-0.5.md`; this plan's pin decision; `agent-host.md`.
  - Evidence (2026-09-06, implemented):
    - Pins: `clay-agent/package.json` + `package-lock.json` resolve all seven
      families at exact `0.5.0`, `better-sqlite3@13.0.3`; `allowScripts` key
      moved to `better-sqlite3@13.0.3`; description/README/main.ts version
      strings now `0.5.0`. Started from clean `npm install` → lockfile shows
      only `@modelcontextprotocol/client` + `/server` (+ `/core`) `2.0.0`
      transitively under `@arnilo/prism-mcp`; no monolithic `sdk`, no
      office/ACP/AG-UI/antigravity, no retired 0.3 names; `pdf-parse` only
      as an uninstalled optional peer of prism-coding-tools (metadata, no
      `node_modules/pdf-parse`).
    - Imports: tsc against 0.5 is clean — zero imports of the 27 removed
      exports (per-symbol grep, no hits); subpath imports unchanged
      (credentials/node, sessions/sqlite, validation/json-schema,
      coding-tools/agent, memory subpaths, web-tools/obscura+browser,
      prism-mcp). providers.ts loads `createHyperProviderPackage` +
      `createCommandCodeProviderPackage` with the other 13; Azure/Bedrock/
      Vertex stay host-config stubs.
    - Keychain (0.5 keyring 2): extracted `resolveKeychain` in host.ts —
      availability probe via `get` of a sentinel key (0.5 `list()` is
      unsupported on keychain stores); `CredentialStoreLockedError` fails
      closed at boot, unavailable degrades to vault-only, missing
      credential stays `undefined`. New `src/__tests__/keychain.test.ts`
      (4 tests) with `createKeychain` HostOptions seam.
    - Tenant/child env: `createSqlitePersistence` keeps its 0.5 signature
      (no tenant field — tenant requirement applies to other store
      factories; clay passes `TENANT = "clay"` at row level already). MCP
      stdio env stays explicit-only (`mcp.ts` builds env from allow-list
      entries; no ambient `process.env`); Obscura children rely on Prism's
      child-env allow-list (`buildChildEnv`), no custom env required.
    - Deny test: `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp`
      now asserts the `0.5.0` pins + `better-sqlite3@13.0.3`, denies
      `@arnilo/prism-office` / `prism-antigravity-agent` as exact needles,
      forbids direct `@modelcontextprotocol/*` deps in package.json,
      asserts `/hyper` + `/commandcode` subpath imports, README `0.5.0`.
      `cargo fmt` applied once (array formatting).
    - Consumer smoke (Linux): `npm test` 75 pass / 1 skip (was 71 — 4 new
      keychain tests); SQLite session fixtures reopen + round-trip across
      durable-run / tree / session-search suites with no schema migration;
      `cargo test --test protocol agent_protocol` 20 passed incl. deny pin
      gate; full `cargo fmt --check` + `cargo check --all-targets` +
      `cargo clippy --all-targets -- -D warnings` clean; `cargo test` all
      suites ok, 0 failures; frontend `npm test` 33 files / 238 passed.
    - Note: `npm audit` still reports 1 high-severity (fast-uri, transitive
      via ajv path) — same as pre-0.5 baseline; not introduced by the bump.
      Unconfigured providers (incl. hyper/commandcode) remain filtered from
      pickers via `providerConfigured`; no non-mock harness test added for
      registration (tsc + deny-test subpath asserts cover the load).
  - Test Cases to Write:
    - Pin gate: `package.json` exact 0.5.0 families + sqlite 13.0.3; README `0.5.0`; no `@modelcontextprotocol/sdk` in clay-agent direct deps.
    - Dead-export compile: `tsc` green; grep of removed symbols empty in `clay-agent/src`.
    - SQLite fixture reopen + mock chat/coding round-trip.
    - Keychain locked vs missing (unit with stubbed store errors).
    - Hyper/commandcode register when loaded; unconfigured providers stay filtered from pickers.

- [x] Verify MCP 2026-07-28 / SDK v2 host path (allow-listed `connectMcpTools`) (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Clay still talks only to `@arnilo/prism-mcp` (`connectMcpTools`); daemon source does not import `@modelcontextprotocol/client` or `/server` or the retired `/sdk`. Lockfile may carry client+server `2.0.0` as prism-mcp dependencies. Empty allow-list → zero tools. One stdio fixture server still connects fail-closed (one failure closes all). Explicit per-entry `env` still forwarded; omitted `env` must not inherit ambient `process.env` (Prism child-env allow-list). Draft-era MCP task vocabulary stays unadvertised. OAuth/elicitation unused in clay-agent v1 (record as unused, do not wire). `phase25_dependencies_deny_acp_agui_mcp` still forbids MCP in `Cargo.toml`; comment/needles updated so `@modelcontextprotocol/sdk` is not required in clay-agent `package.json`.
    - Performance: Connect stays on session start, not keystroke; existing server/tool caps unchanged.
    - Code Quality: Public Prism MCP surface unchanged (`docs/mcp-tools.md`); no Clay copy of SDK types.
    - Security: Allow-list remains host-supplied canonical executable + literal argv + explicit env names; no PATH search; no package-spawned MCP.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/mcp-tools.md`, `docs/migrate-to-0.5.md` §5, `docs/migration.md` MCP table, `clay-agent/src/mcp.ts`, `tests/agent_protocol.rs`.
    - Options Considered:
      - Import SDK v2 types in clay-agent: rejected — stay behind `prism-mcp`.
      - Keep `connectMcpTools` + smoke the v2 transport (chosen).
    - Chosen Approach:
      - No API rewrite unless 0.5 `connectMcpTools` options changed; verify env/stdio; update deny-test comments.
    - API Notes and Examples:
      ```ts
      await connectMcpTools({
        serverId: entry.serverId,
        transport: { type: "stdio", command, args, env, cwd, stderr: "pipe" },
      });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/mcp.ts` (only if 0.5 options require it).
      - `clay-agent/src/__tests__/` MCP connect/env tests.
      - `tests/agent_protocol.rs`: pin/deny comments for client+server vs sdk.
    - References:
      - Prism `docs/mcp-tools.md`; decision 1758 allow-listed MCP.
  - Evidence (2026-09-06, verified):
    - Host path: `src/mcp.ts` still talks only to `@arnilo/prism-mcp`
      (`connectMcpTools`); stdio-only transport; explicit per-entry `env`
      forwarded, omitted env not spread from `process.env`. Prism 0.5's
      `createMcpTransport` passes `env` straight to SDK v2
      `StdioClientTransport`, which merges explicit env over
      `getDefaultEnvironment()` — an allow-list (`LOGNAME`, `PATH`,
      `SHELL`, `TERM`, `USER`) — so ambient secrets never reach children.
      No SDK import anywhere in `clay-agent/src` (new source-grep test).
    - New tests (`src/__tests__/mcp-v2.test.ts` + fixture
      `src/__tests__/mcp-fixture-server.mjs`, a real 2026-07-28-era
      `@modelcontextprotocol/server` stdio server via `serveStdio(factory)`
      registering a `get_env_keys` tool):
      1. Fixture stdio server connects; bridged tool is prefixed
         `mcp:<serverId>:` and callable through `ToolDefinition.execute`.
      2. Env allow-list: child spawned with omitted env does NOT see a
         planted ambient `CLAY_MCP_PLANTED_SECRET`; explicit
         `CLAY_MCP_EXPLICIT` entry does reach it.
      3. Explicit env forwarding asserted standalone.
      4. Bad command fails closed: entire connect rejects, good server
         closed before propagation, nothing partially bridged.
      5. Source-level grep: `src/**/*.ts` (excluding `__tests__`) contains
         no `@modelcontextprotocol/client` / `/server` / `/sdk` import.
    - Existing coverage kept green: empty allow-list zero-tools/no-spawn,
      validation fail-closed, non-canonical command rejection, no partial
      spawn (mcp-obscura.test.ts).
    - Deny test already asserts MCP forbidden in `Cargo.toml` and no direct
      `@modelcontextprotocol/*` in clay-agent `package.json` (retired
      monolithic `/sdk` never required there).
    - Recorded as unused in clay-agent v1 (not wired): MCP OAuth
      (streamable-http auth), MRTR elicitation, subscriptions, MCP Apps.
      Draft-era MCP task vocabulary stays unadvertised (Prism 0.5
      property, §5 of migrate doc).
    - Gates: clay-agent `npm test` 80 pass / 1 skip (5 new); `npm ci`
      clean; Rust/deny gates unchanged-and-green from the 0.5 adopt task.
  - Test Cases to Write:
    - Empty allow-list: zero tools, no spawn.
    - Fixture stdio connect + fail-closed on bad command.
    - Omitted env: child does not see a planted ambient secret; explicit env does.

- [x] Re-review UI catalog after plan 110 before remaining coding-agent UI work (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Binding inventory for remaining I4–I10 UI recorded against the post-110 catalog. Right-pane tabs (Files / Observational Memory / Context / Session Info) use `ClayTabStrip` (`tabList`, `frontend/src/components/tab-strip.tsx`, controlled `activeId`/`onActivate`). Model/effort/OM dropdowns use cataloged `ClayDropdown` (grouped `groups` from I3). Context drawer stays in-tab (`list`+`scroll`+bounded text), not `modal`/`overlay`. Files tab hosts existing `editorView`/`ClayEditor` substrate — no coding-agent editor branch. New CSS uses `var(--clay-ds-…, var(--clay-…))` two-layer fallbacks and keeps `frontend/src/test/design-system-consumption.test.ts` green; no coding-agent-named kind, token, or recipe. Catalog/docs updated in the same change only if a generic addition is required (expected: none).
    - Performance: Tab/dropdown selection widget-local; no per-keystroke IPC; recipe vars are cached CSS custom properties.
    - Code Quality: Trusted-module changes stay in provenance-exact `CodingAgentPanel`; third-party replacements still generic SDUI.
    - Security: No new package authority; inert intents only.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `docs/reference/ui-components.md`,
        `.agents/skills/project-patterns/references/ui-design-system-packages.md`,
        `plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md` tasks 5–8.
    - Options Considered:
      - Keep pre-110 hand-rolled Tabs CSS: rejected — 110 deleted that path; consumption tests fail.
      - Compose remaining surfaces from `ClayTabStrip` + catalog controls (chosen).
    - Chosen Approach:
      - Record the post-110 mapping; I4–I10 implement against it. Original 2026-09-05 catalog task stays historical evidence for I1–I3.
    - API Notes and Examples:
      ```tsx
      <ClayTabStrip
        ariaLabel="Agent detail"
        activeId={rightTab}
        onActivate={setRightTab}
        tabs={[{ id: "files", label: "Files", content: <FilesTab /> }, /* memory, context, session-info */]}
      />
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-ui/references/components.md` / `tokens.md` only if a generic gap appears.
    - References:
      - Plan 110 task 5 (`ClayTabStrip`); this plan's 2026-09-05 inventory.
  - Test Cases to Write:
    - Inventory check: every remaining UI element maps to an existing kind; `design-system-consumption` still green after later UI tasks.
  - Evidence (2026-09-06, verified):
    - Post-110 catalog re-read: components.md `tabList` entry (unified
      `ClayTabStrip`, closed recipes `tabList.root/strip/tab/panel`, five
      tab interaction states), `dropdown` entry (React `ClayDropdown`
      accepts `groups` for labeled sections, landed by I3), plus
      recipe-attributes closed allowlists (`KNOWN_COMPONENT_KINDS`,
      `KNOWN_SLOT_NAMES` — no coding-agent names).
    - Consumption gate: `src/test/design-system-consumption.test.ts`
      12/12 pass on the current tree; `coding-agent.module.css` already
      uses `var(--clay-ds-…, var(--clay-…))` two-layer fallbacks (88
      `var(--clay` references).
    - Current panel state confirmed: `CodingAgentPanel.tsx` imports
      `ClayTabStrip` (right-pane strip, `ariaLabel="Agent detail"`,
      Files tab present) and `ClayDropdown` with `groups` (model picker,
      I3). Trusted-module provenance intact — all changes stay in
      `CodingAgentPanel.tsx` / `coding-agent.module.css`.
    - Binding inventory for remaining UI (all existing kinds, zero new
      kinds / tokens / recipes):
      - I4 effort control → `dropdown` (ClayDropdown, optional groups for
        declared levels); status-row effort readout → `label`/
        `statusItem` text; Shift+Tab via client keybind, widget-local.
      - I5 transcript boxes → `label`/text + CSS two-layer fallbacks in
        `coding-agent.module.css` only.
      - I6 Files tab → existing ClayTabStrip tab content hosting the
        `editorView`/ClayEditor substrate (FilesTab); no second strip,
        no coding-agent editor branch.
      - I7 Context tab → `list` + `scroll` + `label` composed in-tab;
        no `modal`/`overlay`.
      - I8 OM tab → `dropdown` (worker models, grouped) + `list`/`label`
        for observations.
      - I9 /resume → existing inert intents (`agent.clientOpenSessionPicker`,
        `agent.clientOpenSessionSearchPicker` already wired in the panel).
      - I10 Session Info → fourth ClayTabStrip tab; switch to controlled
        `activeId`/`onActivate` when it lands.
    - Catalog/docs edits: none required — expected generic gap did not
      materialize; `components.md`/`tokens.md` untouched.

- [x] I4: Wire real reasoning-effort control (daemon ThinkingLevel, dropdown, configurable Shift+Tab) (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Blocked on Prism 0.4.0; this task runs only after the
      0.5.0 pin + MCP tasks. The daemon accepts a thinking level per run:
      `session.prompt` gains an optional `thinkingLevel` mapped through
      Prism 0.5.0 `applyThinkingLevelForModel(base, level, model)` (family
      + snap + merge in one call — do not call `applyThinkingLevel` +
      `thinkingFamilyForModel` for new host code). Level pickers read
      `model.capabilities.thinkingLevels` via `thinkingLevelsForModel`;
      empty/undefined → no control. `parseThinkingLevel` fail-closes empty
      / non-string at the daemon boundary; opaque non-empty strings may
      passthrough. STATE events carry `effortLevels` + active `effort`;
      the status row displays the active effort; the panel exposes a
      cataloged `ClayDropdown`; `Shift+Tab` cycles declared levels when
      present and is a no-op otherwise. Binding: `coding-agent.clientCycleEffort`
      via `bindKey`, default `Shift+Tab`. Post-110: dropdown uses existing
      `dropdown` kind + recipe vars; no new kind.
    - Performance: Level resolution is registry metadata (no provider
      call); the per-run compat patch rides existing run options; no new
      IPC on keystroke paths.
    - Code Quality: Portable `ThinkingLevel` is the only new vocabulary —
      no provider-specific effort fields cross the daemon boundary
      (Prism owns the mapping); `requireExplicitModel`-style fail-closed
      on unknown levels.
    - Security: Effort is a run option, not authority; unvalidated level
      strings are rejected at the daemon boundary.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/thinking-and-reasoning.md` (0.5.0 contract:
        `applyThinkingLevelForModel`, declared `thinkingLevels`, snap,
        `google` / `output_config_effort` families), `docs/migrate-to-0.5.md`
        §7, `docs/agent-session-runtime.md` (RunOptions),
        `clay-agent/src/host.ts` (session.prompt params),
        `src/protocol/agent.rs` (wire), `frontend/src/coding-agent/
        CodingAgentPanel.tsx` (current no-op chord),
        `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `docs/reference/clay-js-api/keybindings/bind-key.md` (client
        command binding rules).
    - Options Considered:
      - Provider-specific effort fields in the UI: leaks provider vocab;
      rejected — Prism's portable level exists exactly for this.
      - Frontend-only state + prompt prefix hack: not real; rejected.
      - Daemon-owned portable level + Prism compat mapping + configurable
      client command (chosen).
    - Chosen Approach:
      - Daemon: `thinkingLevel` on prompt/run options via
        `applyThinkingLevelForModel`; expose `thinkingLevelsForModel` as
        `effortLevels` (empty for non-reasoning / undeclared models); a
        `session.setThinkingLevel`-style state update (or prompt param —
        pick the smaller surface during implementation and record it).
      - Rust: wire `effortLevels`/`effort` into STATE snapshot fields.
      - Frontend: cataloged `ClayDropdown` + chord reading the effective
        binding; default `Shift+Tab` documented in the Clay JS API doc.
    - API Notes and Examples:
      ```ts
      import {
        applyThinkingLevelForModel,
        parseThinkingLevel,
        thinkingLevelsForModel,
      } from "@arnilo/prism";
      await session.run(input, {
        providerOptions: applyThinkingLevelForModel(base, level, model),
      });
      const levels = thinkingLevelsForModel(model); // picker source
      ```
    - Files to Create/Edit:
      - `clay-agent/src/{host,rpc,providers}.ts`: level param, registry
        levels, state fields.
      - `src/protocol/agent.rs`, `src/server/agent.rs`,
        `src/server/agent_agui.rs`: wire fields + prompt passthrough.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: dropdown +
        binding-aware chord.
      - `packages/coding-agent/{package.json,dist/load.js}`: client
        command registration.
    - References:
      - Prism `docs/thinking-and-reasoning.md`, `docs/migrate-to-0.5.md` §7;
        Clay report `2026-09-05-prism-thinking-level-gaps-report.md` (fixed
        in 0.5.0); roadmap Phase 2.1 effort question (yes).
  - Test Cases to Write:
    - Mock-provider capture: selected level reaches the 0.5 wire field for
      the model's family (Anthropic `output_config.effort`, Google
      `thinkingLevel`, xAI `reasoning_effort` — no 0.4 silent drop).
      Changing levels between runs changes the next request.
    - Declared-level picker: `effortLevels` equals `thinkingLevelsForModel`;
      out-of-set portable levels snap (or host rejects — record which).
    - Non-reasoning model: no dropdown options, chord is a no-op, no
      state fields.
    - Empty/non-string level rejected fail-closed at the daemon boundary
      (`parseThinkingLevel`).
    - Rebound chord: `bindKey` override changes the composer's cycle key.
  - Evidence (2026-09-06, implemented):
    - Surface decision (recorded per plan): **prompt param only** —
      `session.prompt.thinkingLevel` carries the level per run; no
      `session.setThinkingLevel` RPC. The pending level lives in the
      panel; STATE echoes the session's active level set at prompt time.
    - Daemon (`clay-agent/src/host.ts`): `sessionPrompt` parses the level
      fail-closed via Prism 0.5.0 `parseThinkingLevel` (empty/non-string
      -> rpc -32602; opaque non-empty passthrough), applies the
      model-aware `applyThinkingLevelForModel(undefined, level, model)`
      (family + snap + merge in one call; never the split 0.4 path), and
      rides the run's `providerOptions`. `modelList` exposes
      `thinkingLevels` from `thinkingLevelsForModel` (registry metadata,
      no provider call). New `registerModels` test seam on `HostOptions`.
    - Rust wire: `AgentSessionSnapshot.effort_levels`/`effort`,
      `AgentModelInfo.thinking_levels`,
      `AgentClientCommand::Prompt.thinking_level`; `agent_agui` STATE
      carries `effortLevels` + `effort`; `begin_prompt_with_effort`
      records the session's active level in the book; the chat.submit
      SDUI intent carries an optional second `thinkingLevel` argument
      (`intent_thinking_level` in runtime.rs).
    - Frontend: `TauriClayAgent.sendPrompt(text, thinkingLevel?)` rides
      the intent arg; `CodingAgentPanel` reads STATE levels/active
      effort, renders a cataloged `ClayDropdown` (Effort) in the status
      row for declared levels (plain caption readout otherwise), cycles
      via the manifest-bound chord (default Shift+Tab), and sends the
      pending level with each prompt. `PaneTree` resolves the effective
      chord from the pane session's behavior manifest so `bindKey`
      overrides apply (rebound-chord test covers a Ctrl+E override;
      Shift+Tab becomes a no-op after rebind).
    - Package: `@clay/coding-agent` manifest declares
      `coding-agent.clientCycleEffort` (Cycle Reasoning Effort) with
      default `keyRouting` Shift+Tab; keybindings.rs allowlists it as
      runtime-bindable ClientUiCommand; documented in
      `docs/reference/clay-js-api/keybindings/bind-key.md`.
    - Tests: daemon `thinking-level.test.ts` (5): wire capture per family
      (output_config.effort / reasoning_effort / thinkingLevel — no 0.4
      silent drop), out-of-set snap (max -> high), empty/non-string/null/
      object fail-closed with zero provider requests, non-reasoning model
      invents no compat field, `model.list` thinkingLevels exposure.
      Rust: STATE effort-fields test, `parse_models` thinkingLevels test,
      intent `thinkingLevel` arg test, keybinding bindability test.
      Frontend: chord-cycle + rebound-chord panel tests. Manifest
      idempotency test updated 12 -> 13 commands.
    - Gates: clay-agent 85 pass/1 skip; frontend 240 pass (33 files);
      cargo fmt --check clean; cargo check --all-targets clean; cargo
      clippy --all-targets -- -D warnings clean; full cargo test 1213 lib
      + 6 + 41 + 207 + 73 + 135, 0 failed.

- [x] I5 + R4 + R5: Full chronological transcript with tool payloads and skill names (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Root-cause and fix the "only initial user message + last
      agent message" defect so every turn renders from the
      server-authoritative transcript: user prompts, steering messages,
      thinking/reasoning, tool calls with bounded argument and output
      content, `load_skill` rows with the skill name, assistant messages,
      errors, usage — in arrival order, uniform-height truncated boxes,
      type-colored, full content on selection. Tool events carry bounded
      payload digests (args summary and output excerpt within the existing
      `AGENT_MAX_ENTRY_TEXT_BYTES` per-entry budget) and skill events carry
      the skill name; the client-side `snapshot.tools` side list and
      derived `toolStats` counters are removed in favor of transcript
      entries (R5). Steering messages appear as user-kind entries (pi
      parity: mid-run user input is visible).
    - Performance: Transcript payloads stay within the existing snapshot
      budgets (200 entries / 256 KiB); tool output excerpts are
      server-truncated before the wire; no per-row IPC; React rendering
      stays memoized per row (ChatPanel precedent).
    - Code Quality: The investigation result (which layer dropped history:
      run-pipeline message capture vs `MESSAGES_SNAPSHOT` replacement
      ordering) is recorded in task evidence with the fix at the
      pipeline/ordering root, not a client-side patch-over; transcript
      kinds stay the standard AG-UI roles + `metadata.clayKind`.
    - Security: Tool outputs pass the daemon redactor before the wire
      (existing `redactor.redact` path on entries); secrets containment
      unchanged; payloads bounded server-side, never trusted from the
      client.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/agent.rs` (`apply_transcript_event`,
        `append_delta`, caps), `src/server/agent.rs` (`begin_prompt`,
        `apply_book_event`, `snapshot_for`),
        `src/server/agent_agui.rs` (event mapping, snapshot events),
        `frontend/src/agent/state.ts` (`applyOutOfRun`,
        MESSAGES_SNAPSHOT replace), `frontend/src/agent/TauriClayAgent.ts`
        (run pipeline, `input.messages` server-authoritative comment),
        `clay-agent/src/host.ts` (event emission, redaction),
        `@ag-ui/client` AbstractAgent message pipeline docs,
        `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `.agents/skills/project-patterns/references/protocol-and-performance.md`.
    - Options Considered:
      - Client-side merge of run-pipeline messages with snapshots: treats
      the symptom; two sources of truth keep racing.
      - Server-authoritative transcript as single source; run pipeline
      renders live deltas only within the active run and every snapshot
      boundary reconciles from the server list (chosen): matches the
      existing design comment ("daemon owns conversation context") and
      fixes the ordering/lifecycle bug at its root.
    - Chosen Approach:
      - 1) Reproduce and record the drop (expected candidates:
      `begin_prompt`'s Snapshot arriving while the run pipeline holds a
      pre-run message list, then RunFinished publishing the pipeline's
      shorter list; or chunk-id stability reopening messages). 2) Fix the
      lifecycle so `MESSAGES_SNAPSHOT` always wins outside an active run
      and run-scoped deltas only append to the current run's messages.
      3) Extend `AgentWireEvent::Tool` with bounded arg/output digests +
      skill name; new transcript kinds render them; delete the side list.
      Post-110: type-colored boxes stay token/`--clay-ds-*` two-layer
      fallbacks; no new kind; consumption tests stay green.
    - API Notes and Examples:
      ```rust
      AgentWireEvent::Tool { phase, name, tool_call_id,
          args_digest: Option<String>, // bounded, redacted
          output_digest: Option<String>, // bounded, redacted
          skill_name: Option<String>, } // load_skill rows
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: event payload enrichment (redacted,
        bounded).
      - `src/protocol/agent.rs`: wire fields + transcript entry kinds.
      - `src/server/agent_agui.rs`: event → AG-UI mapping.
      - `frontend/src/agent/state.ts`, `frontend/src/coding-agent/
        CodingAgentPanel.tsx`: lifecycle fix, tool/skill box rendering,
        side-list removal.
      - `frontend/src/coding-agent/CodingAgentPanel.test.tsx`: multi-turn
        transcript fixture.
    - References:
      - Plan 108 task 8 (transcript boxes), task 9 (stream-form prompt
        fix — prior ordering bug in the same area); pi transcript
        semantics.
  - Test Cases to Write:
    - Multi-turn regression: 3+ turns with tools and a skill load render
      complete history after each turn and after remount (snapshot
      restore).
    - Steer mid-run: the steer text appears as a user-kind entry.
    - Bounded payloads: oversized tool output truncates server-side;
      redaction applies (secret fixture never reaches the wire).
    - Run-pipeline race: prompt → immediate snapshot ordering fixture no
      longer drops history.
  - Evidence (2026-09-06, implemented):
    - Investigation result (recorded per Code Quality): the layer that
      dropped history was the **client run pipeline**, not the daemon
      capture. `@ag-ui/client`'s `apply` re-inserts `input.messages` — a
      stale pre-prompt copy captured at `runAgent()` time, before the
      prompt-time server snapshot (carrying the new user row) arrived — at
      `RUN_STARTED`, clobbering the snapshot list; and the server never
      republished a snapshot after a run settled, so the shorter pipeline
      list persisted until the next prompt. Reproduced deterministically in
      `frontend/src/agent/transcript-lifecycle.test.ts` (stale-seed
      resurrection + missing post-finish reconciliation).
    - Server fix (single source of truth):
      - `AgentWireEvent::Tool` gained bounded `argsDigest` / `outputDigest`
        / `skillName` (serde default, skip-if-none). `map_event` builds
        them server-side from the daemon's `call.arguments` / result text
        content blocks (falling back to `value`), error message, and block
        reason — secret-redacted (`redact_text`) and truncated to
        `AGENT_MAX_ENTRY_TEXT_BYTES` before the wire. `load_skill` rows
        carry `skill_name` from `arguments.name`. The daemon itself needed
        no change (deviation from the planned `clay-agent/src/host.ts`
        edit: redaction lives where the secret list lives — the Rust
        server).
      - Transcript: new `AgentTranscriptKind::Tool` entry with
        `tool_call_id` + `skill_name` (serde default, additive). Tool rows
        evolve ONE entry per call: Started records the args summary,
        Finished/Error/Blocked append the output excerpt in place,
        Progress is transient, empty call ids never produce rows,
        per-entry budget enforced.
      - The daemon pump republishes the book transcript snapshot right
        after forwarding a run's `Finished`/`Error` event (usage row
        included) so every snapshot boundary reconciles from the server
        list.
      - Steers are now recorded as user-kind entries in the book
        transcript and republished mid-run (pi parity: mid-run user input
        visible).
    - Client fix (`frontend/src/agent/state.ts`):
      - `onRunStartedEvent` keeps the live server-authoritative list
        (`stopPropagation`) — the stale re-seed can no longer clobber the
        prompt-time snapshot.
      - `onMessagesSnapshotEvent` reconciles from the server list at every
        snapshot boundary while preserving in-flight `clay-text-` /
        `clay-reasoning-` / `clay-tool-` run messages the server list does
        not contain yet (content-aware: a settled run's coalesced rows are
        not duplicated).
      - `onCustomEvent` evolves one tool box per `clay.toolPhase` row
        (args digest + output excerpt), mirroring the server evolution.
      - Snapshots arriving while a run pipeline is active are deferred and
        the last queued snapshot flushes right after `runAgent()`
        resolves (`chatAgent.runTurn()`), because the async pipeline's
        delta publishes would otherwise race `setMessages` (a late chunk
        publish could resurrect stale rows after the boundary). Mid-run
        rendering is owned by the in-pipeline reconcile hook; the server
        list always wins at the boundary.
      - `MESSAGES_SNAPSHOT` always wins outside an active run (unchanged
        `setMessages` path).
    - R5 removal: the client-side `snapshot.tools` side list and derived
      `toolStats` counters are gone (`ToolActivityRow` / `ToolStats`
      deleted); Context-tab category counts (tool outputs / skills loaded
      / files loaded) now derive from transcript entries via box metadata
      (`toolName`, `clayKind`).
    - Rendering: tool rows ride the standard AG-UI `tool` role with
      `metadata.clayKind` (`tool` / `skill` for `load_skill`) +
      `skillName`; live CUSTOM rows and snapshot rows reconcile to the
      same `clay-tool-{toolCallId}` message id. Panel boxes: label = skill
      name (skill rows) or tool name, content = bounded args + output
      digest; kind coloring and uniform-height truncation unchanged
      (token / `--clay-ds-*` two-layer fallbacks, no new kind).
    - Tests: Rust — `tool_rows_evolve_one_entry_per_call_with_bounded_digests`
      (evolution, progress transient, skill name, per-entry cap, empty-id
      guard), `map_event_builds_bounded_redacted_tool_digests` (args
      redaction incl. secret fixture never reaching the wire, output
      excerpt, skill name), `tool_transcript_entries_map_to_standard_tool_role_messages`.
      Frontend — `transcript-lifecycle.test.ts`: race regression (3+
      turns), multi-turn with tools + skill in order + remount restore,
      mid-run steer visible without losing in-flight text.
    - Gates: cargo fmt --check clean; cargo check --all-targets clean;
      cargo clippy --all-targets -- -D warnings clean; full cargo test
      1216 lib + 6 + 41 + 207 + 73 + 135, 0 failed (the
      `large_document` 500 ms latency-budget test flaked once under load
      average 25 from background compiler processes and passed at load 9 —
      environmental, not a regression); frontend 243 pass (34 files);
      clay-agent 85 pass / 1 skip (unchanged surface).

- [x] I6: Files tab shows the selected file's editor view; workspace tree only in the left tab with a toggle binding (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: The Files tab no longer embeds the SDUI workspace
      browser; it shows the file currently selected in the workspace (the
      Clay editor view of the active/selected document — content follows
      workspace selection; no file selected → the existing empty state
      with Open file actions). The workspace tree renders only in the
      left workspace tab. The left workspace tab toggle is reachable by a
      documented default key binding and is user-configurable via
      `bindKey` on the existing `workspace.toggleFileBrowser` command id
      (verify default binding exists or add one that does not collide with
      the documented default set; record it in the Clay JS API doc).
    - Performance: The embedded view reuses the existing document view
      machinery (no second document pipeline); no change to editor hot
      paths; lazy mount as today.
    - Code Quality: No coding-agent-specific editor branch — the tab hosts
      the same document surface the shell uses, driven by the tab's
      selection state; tree rendering remains the shell file browser's
      sole instance.
    - Security: Read/write authority unchanged — the embedded view talks
      to the same document authority; no new file access beyond the
      workspace.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `frontend/src/shell/PaneTree.tsx` (sdui/send props today),
        `frontend/src/shell/file-browser` surface + `PaneDocumentView`/
        `EditorSurface` precedent, `src/shell/file_browser.rs`,
        `src/server/command_execution.rs` (`workspace.toggleFileBrowser`),
        `docs/reference/clay-js-api/keybindings/bind-key.md`.
    - Options Considered:
      - Keep the tree in Files as a collapsible: keeps the duplication the
      user rejected.
      - Files tab = editor view of the selected document; tree stays
      shell-left only (chosen); reuse the pane document view or document
      store content view — whichever the inventory confirms composes
      inside a tab panel without new authority.
    - Chosen Approach:
      - Replace `FilesTab`'s embedded `SduiRenderer` with the selected
      document's view bound to the tab's selection state; add/verify the
      `workspace.toggleFileBrowser` default binding and document it.
      Post-110: Files tab is a `ClayTabStrip` panel hosting `ClayEditor` /
      `editorView`, not a second tab strip or SDUI tree.
    - API Notes and Examples:
      ```ts
      // Files tab content follows workspace selection
      <FilesTab document={tab.selectedDocument} /> // editor view or empty state
      ```
    - Files to Create/Edit:
      - `frontend/src/coding-agent/CodingAgentPanel.tsx` (+ module CSS):
        FilesTab rewrite; props change from `sdui` to selection-driven.
      - `frontend/src/shell/PaneTree.tsx`: prop plumbing if needed.
      - `src/server/mod.rs` / keybinding defaults (only if no default
        binding exists).
      - `docs/reference/clay-js-api/workspace/` toggle doc (binding field).
    - References:
      - Plan 108 task 8 (Files tab original composition); Phase 22.8
        file-browser notes in `components.md`.
  - Test Cases to Write:
    - Selection drill: select file X in the workspace tree → Files tab
      shows X's content; select Y → follows; deselect/empty → empty state.
    - Tree appears exactly once (left tab); toggling the binding hides and
      restores it; rebind via `bindKey` works.
  - Evidence (2026-09-06, implemented):
    - Files tab rewrite (`frontend/src/coding-agent/CodingAgentPanel.tsx`):
      `FilesTab` no longer imports or mounts the `SduiRenderer` workspace
      browser. It hosts the shell's own `ClayEditor` bound to the pane's
      `DocumentSession` (new `session` prop plumbed from
      `PaneTree.tsx`'s `pane.session`), driven by the session's store —
      content follows workspace selection through the existing session
      open path, no second document pipeline, no coding-agent editor
      branch. Because the pane's editor is unmounted while the agent
      surface is open, the tab's `ClayEditor` is the session's only
      EditorView (one view per mount invariant holds). No file selected
      (no path, empty doc) → the existing empty state with Open file /
      Resume session / Search sessions actions and the recent-sessions
      list. The `sdui` prop is gone from `CodingAgentPanelProps`.
    - Tree instance: the SDUI workspace tree renders only through
      `PackageWorkspace` (left slot / shell main) — the Files-tab embed
      was the second instance and is removed.
    - Toggle binding: verified NO shipped default chord existed for
      `workspace.toggleFileBrowser` (only the canonical `examples/init.js`
      example bound `Ctrl+B` editor-scope). Added the shipped default
      `Ctrl+B` — Global scope, `ServerFirst` routing
      (`KeyBindingRule::global_server_first` in `src/protocol/mod.rs`
      `default_keymaps`) so it fires wherever focus sits (agent surface,
      tree, editor). `Ctrl+B` collides with no shipped default and no
      editor-internal binding (the CodeMirror emacs keymap is not
      installed). Added the matching
      `CommandDeclaration::server_intent("workspace.toggleFileBrowser", …)`
      to `default_commands` (manifest keymap validation requires the
      declared command). Rebindable via `bindKey`/`unbindKey` like every
      default.
    - Docs: `docs/reference/clay-js-api/keybindings/bind-key.md` records
      the shipped `Ctrl+B` default (Global), the tree-only-in-left-tab /
      Files-tab-editor split, and rebinding; the usage example and
      `examples/config/init.js` updated (the explicit bind is an
      idempotent no-op re-declaration); `configuration.md`
      file-browser-toggle row updated.
    - Tests: Rust — `default_keymaps_contain_workspace_file_browser_toggle_binding`
      (chord, Global, ServerFirst),
      `default_commands_declare_workspace_file_browser_toggle_as_server_intent`,
      existing `default_keymaps_are_prefix_collision_free` green, and the
      existing `workspace_file_browser_toggle_is_bindable_and_server_routed`
      (rebind gate) unchanged. Frontend — two new panel tests: empty state
      with Open file actions and no workspace tree until selection;
      selection drill (store path update → shared `clay-editor` mounts;
      second selection rebinds the same session; no empty-state residue).
      Behavior-manifest payload estimates bumped for the two new entries
      (`src/server/js_runtime/tests.rs` 6400→6540 / 6600→6740 — the
      manifest grew by the command + keymap record).
    - Gates: cargo fmt --check clean; cargo clippy --all-targets --
      -D warnings clean; full cargo test 1218 lib + 6 + 41 + 207 + 73 +
      135, 0 failed; frontend 245 pass (34 files); clay-agent 85 pass /
      1 skip (surface untouched).

- [x] I7: Context tab as a live context inspector with content drawer (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: A daemon `session.context` RPC (bounded, redacted)
      returns the session's current active context as categorized items:
      system prompt, user messages (incl. steering), loaded skills, tool
      calls + outputs, model thinking/reasoning, agent messages, and
      compaction summaries — derived from the live session
      (`session.entries()` + system prompt + active skills + compaction
      state). The Context tab lists categories with live counts; clicking
      a category opens a drawer (in-tab detail panel) listing its items
      with bounded previews; clicking an item shows its full content.
      After compaction, compacted-away items leave the active list and the
      summary item appears (and vice versa on branch checkout); removed
      context never lingers. The client-derived `contextCounts` are
      replaced by this server-authoritative data.
    - Performance: Context fetch is on-demand (tab visible / category
      opened), bounded item counts and text per item (existing entry
      budgets), cached per session version; never on keystroke/paint
      paths; no full-transcript re-serialization per poll — event-driven
      invalidation (run finished / compaction / entry append) or a
      debounced refresh, chosen during implementation and recorded.
    - Code Quality: One daemon-side category model reused by counts and
      items; redaction through the existing redactor; drawer composes
      cataloged kinds (`list`, `scroll`, bounded text).
    - Security: Context items are read-only inspection of what the agent
      already holds; redacted like `session.load` entries; no new
      injection path (nothing here feeds back into agent context).
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/agent-session-runtime.md` (`entries()`, compaction
        entries, skills disclosure), `docs/compaction-and-retry.md`
        (compaction entry shape), `clay-agent/src/host.ts`
        (`sessionLoad`, `redactor`, tree render precedent for categorized
        entries), `src/server/agent.rs` (RPC forwarding shapes),
        `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`.
    - Options Considered:
      - Client-side reconstruction from the transcript: cannot see system
      prompt, skills, or compaction state; wrong by construction.
      - Daemon `session.context` RPC + typed wire + drawer UI (chosen):
      the daemon already holds every input to this view.
    - Chosen Approach:
      - Daemon: build the categorized context from the live session at
      request time (bounded slices, redacted), versioned by session
      entry count for cheap invalidation. Rust: forward as a typed
      command. Frontend: category list → in-tab drawer → item detail.
      Post-110: compose `list` + `scroll` + `label` inside the Context
      `ClayTabStrip` panel; not `modal`/`overlay`; recipe vars only.
    - API Notes and Examples:
      ```jsonc
      // session.context → { version, categories: [
      //   { kind: "systemPrompt", count: 1, items: [{ id, title, previewBytes }] },
      //   { kind: "toolOutput", count: 12, items: [...] }, ... ] }
      // item fetch: session.context { sessionId, itemId } → full redacted text
      ```
    - Files to Create/Edit:
      - `clay-agent/src/{host,rpc}.ts`: `session.context` + item detail.
      - `src/protocol/agent.rs`, `src/server/agent.rs`: typed forwarding.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx` (+ CSS): Context
        tab rewrite with drawer.
    - References:
      - Prism `docs/agent-session-runtime.md`, `docs/context-and-skills.md`;
        plan 108 task 10 (compaction/tree entries precedent).
  - Test Cases to Write:
    - Category drill: system prompt / skill / tool output / thinking items
      show their real content in the drawer.
    - Compaction drill: trigger compaction → compacted categories shrink,
      summary category appears with content; branch checkout to a
      pre-compaction entry restores the earlier active context.
    - Bounds: oversized items truncate; secrets redacted (fixture).
  - Evidence (2026-09-06, implemented):
    - Daemon (`clay-agent/src/host.ts`): new `session.context` RPC.
      - `{ sessionId }` → `{ sessionId, version, categories: [{ kind,
        label, count, items: [{ id, title, preview }] }] }`; `{ sessionId,
        itemId }` → one item's full redacted content. Unknown ids fail
        closed (`-32000 Unknown context item`).
      - Active context = the branch chain from the latest `kind:
        "compaction"` entry onward (compacted-away items leave the list;
        the summary item stays). `session.entries()` is branch-scoped, so
        a branch checkout rebinds and the same derivation restores the
        earlier context.
      - One category model feeds counts and items: `systemPrompt`
        (definition-owned prompt contributions, captured on LiveSession at
        creation via a `systemPrompt` field + `registerAgents` test seam),
        `userMessage`, `skill` (real `load_skill` tool-call blocks —
        skill tool-execution events are not persisted as entries),
        `toolOutput` (assistant `tool_call` blocks + role-tool
        `tool_result` blocks, per the Prism transcript contract),
        `thinking`, `agentMessage`, `compactionSummary`.
      - Bounds: previews ≤ 160 chars; item content ≤ 32 KiB (transcript
        entry budget); items per category capped at 200 with the real
        `count` kept so the client renders "…and N more".
      - Redaction: every entry passes `this.redactor.redact` before any
        text is projected (previews and full content).
      - Performance choice (recorded): fetch is on-demand — the panel
        fetches when the Context tab opens and re-fetches when the
        transcript length changes (event-driven invalidation: run
        finished / compaction / steer / entry append all land transcript
        entries and bump the server `version`); no polling, no
        keystroke/paint path involvement. The daemon builds on demand
        (bounded walk, no cache); the client caches the view per version
        in agent state.
    - Rust (`src/protocol/agent.rs`, `src/server/agent.rs`): new
      `AgentClientCommand::Context { session_id, item_id }` forwarded as
      the `session.context` RPC; the response rides the existing generic
      `agent_rpc("session.context", …)` → `clay.agentRpc` AG-UI custom
      event — no new snapshot state.
    - Frontend (`frontend/src/agent/state.ts`,
      `CodingAgentPanel.tsx`, module CSS): the `clay.agentRpc` handler
      stores list responses in `contextView` and item responses in
      `contextItemDetail` (agent state, version-cached). The Context tab
      renders the server-authoritative category list (the client-derived
      `contextCounts` are deleted), an in-tab drawer (item list with
      bounded previews + "…and N more"), and item detail (full redacted
      content + Back) — all inside the `ClayTabStrip` panel (no
      modal/overlay), catalog list/label/button composition, host-token
      vars only (no invented `--clay-ds-*` names; provenance test
      enforced). The strip is now controlled (`activeId`/`onActivate`) so
      the fetch tracks tab visibility.
    - Tests: daemon `context.test.ts` (real runs through a
      turn-sequenced fake provider + real `load_skill` dispatch +
      `skill.register` + planted secret): categories expose real content
      (system prompt / user / thinking / skill / tool call+output, secret
      redacted in previews), item detail returns full redacted content
      and fails closed on unknown ids, bounds (40 KiB item clipped to the
      32 KiB budget), compaction drill (active context shrinks, summary
      appears; checkout restore covered by the branch-scoping contract —
      the checkout RPC itself is server-coupled via checkpoint.restore
      and stays in the manual plan). Rust: the codec round-trip suite
      covers both `Context` variants. Frontend: the obsolete
      client-counts test is replaced by the server-authoritative
      drill (fetch on tab open → categories → drawer → detail → back).
    - Gates: cargo fmt --check clean; cargo clippy --all-targets --
      -D warnings clean; full cargo test 1218 lib + 6 + 41 + 207 + 73 +
      135, 0 failed; frontend 245 pass (34 files); clay-agent 89 pass /
      1 skip.

- [x] I8: Observational Memory tab — live activity log including drops, and OM worker-model selection (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: The OM tab shows all Observer activity for the session:
      observations recorded (with fact summaries), reflections recorded,
      observations dropped through reflection (with the drop visible, not
      silently vanished — Prism `om.observations.dropped` entries), and
      compaction folds — derived from session entries + the OM ledger
      (Prism `createMemoryStatusCommand`/`createMemoryViewCommand`
      surfaces), bounded and live (updated on run completion / OM worker
      completion, not polled on hot paths). The tab provides OM
      worker-model selection through the same model-listing/selection
      logic as the coding agent (`/model`-equivalent dropdown over all
      configured providers' models), selecting distinct observation and
      reflection worker models; the selection is retained per workspace
      (book v2 map, I2) and per session (session metadata on the record,
      restored on resume); it may differ from the session model; changes
      apply to the next session attach.
    - Performance: OM activity surfaces ride entry/ledger reads at
      completion boundaries; worker-model selection is registry metadata;
      no extra provider calls; no per-keystroke work.
    - Code Quality: OM worker config flows through the existing
      `HostOptions.observationalMemory` seam (host setter or per-session
      params — smallest surface that satisfies per-session retention,
      recorded in evidence); `requireExplicitModel: true` semantics
      preserved (no accidental session-model reuse for workers).
    - Security: Worker models are provider/model ids only; no credential
      movement; OM content redacted as all session content; selection UI
      grants no authority.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/compaction-observational-memory.md` (complete:
        activation, worker models, `om.*` entries, status/view commands,
        settings provider), `clay-agent/src/host.ts:169-193, 780-830`
      (`attachOm`, `omConfig`), decision
        `2026-08-30-2158-observational-memory-defaults-worker-models-80k-per-session.md`,
        `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`.
    - Options Considered:
      - Host-global OM config only: cannot honor per-workspace/per-session
      retention.
      - Per-session OM params defaulting from the per-workspace book
      selection (chosen): session creation already takes options; the
      book supplies defaults; resume restores from session metadata.
    - Chosen Approach:
      - Daemon: `session.new`/`resume` accept OM worker models; host
      applies them at `attachOm`; expose `om.activity` read (entries +
      ledger via the status/view command surfaces) and OM model
      get/set. Rust wire + panel `ClayDropdown` reusing the I3 grouped
      list. Post-110: Memory tab stays a `ClayTabStrip` panel; worker
      models use cataloged dropdown, not a new picker widget.
    - API Notes and Examples:
      ```ts
      // per-session OM workers, workspace-book defaults
      session.new({ ..., observationalMemory: {
        observation: { provider, model }, reflection: { provider, model } } })
      ```
    - Files to Create/Edit:
      - `clay-agent/src/{host,rpc}.ts`: OM params, activity read, model
        get/set.
      - `src/protocol/agent.rs`, `src/server/agent.rs`,
        `src/server/agent_agui.rs`: forwarding + state fields.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx` (+ CSS): OM tab
        activity list + model dropdowns.
      - `src/server/agent.rs`: book v2 OM fields (extend I2 shape).
    - References:
      - Decision 2158; plan 108 OM chrome note (live population was
        deferred to Phase 7 for `st` cadences — this task surfaces the
        coding profile's own OM activity, which the daemon already
        attaches).
  - Test Cases to Write:
    - Activity drill: a session with OM attached records
      observe→reflect→drop; all three appear (drops included) with
      summaries.
    - Model retention: select OM models different from the session model;
      restart → workspace default restored; resume session → session's OM
      models restored.
    - Fail-closed: OM worker model without credentials never silently
      falls back to the session model (`requireExplicitModel` honored).
  - Evidence (2026-09-06, implemented):
    - Daemon (`clay-agent/src/host.ts`):
      - Per-session worker store `omWorkerModels` (session →
        `{ observation?, reflection? }`). `session.om.set { sessionId,
        workers: { observation: {provider, model}|null, reflection: …
        |null } }` validates shapes (non-empty strings, object-or-fail
        `-32602`) and the provider registry (unknown provider →
        `-32000 Unknown OM worker provider` — never a silent session-model
        fallback), persists into the session record metadata
        (`metadata.omWorkers`, same advisory appendSession pattern as the
        provider/model persistence), and re-attaches OM for the live
        session so the change applies immediately (leaf-preserving
        recreate — the same mechanism as a mid-session model switch).
      - `session.om.activity { sessionId }` returns the read model:
        `{ sessionId, attached, observation, reflection, activity }` where
        `activity` rows (`{ id, kind, summary }`, kinds
        `observation`/`reflection`/`drop`/`fold`) are projected from the
        branch's `kind: "custom"` entries
        (`om.observations.recorded` → one row per fact with the content
        summary, `om.reflections.recorded` → one row per reflection,
        `om.observations.dropped` → a visible drop row
        "Dropped N observations", `om.folded` → compaction-fold row).
        Bounded: ≤ 200 rows, summaries redacted + ≤ 160 chars; the read
        is on-demand (tab open + transcript-length invalidation — run/OM
        completion boundaries land transcript entries; never polled).
      - `session.new` accepts `observationalMemoryWorkers` (per the
        decided shape); `ensureLive` restores `metadata.omWorkers` so
        resume re-applies the session's selection. `attachOm` resolves
        each worker's config per session: a selection binds the
        registered provider instance + the selected model (thresholds
        still from host config); unset workers stay skipped via
        `requireExplicitModel: true` (decision 2158 preserved). The host
        OM config seam also forwards `dropper` (policy/targetTokens —
        `lowest-relevance` drops deterministically) and
        `reflection.observationTokens`.
      - **Root-cause fix found by the activity drill**: `recreateSessionModel`
        discarded the OM-attached proxy session for a raw
        `agent.createSession`, silently detaching the Observer (post-run
        flush, context provider, compaction strategy) after ANY
        mid-session model switch (plan 108 task 9 path). The recreate now
        keeps the attached session (`createSession` returns the proxy and
        binds the current branch leaf).
    - Rust (`src/protocol/agent.rs`, `src/server/agent.rs`,
      `src/server/connection/mod.rs`, `src/server/agent_picker.rs`):
      `AgentPickerKind::OmObservation`/`OmReflection` + `AgentOmWorkerKind`
      + `AgentOmWorkerModel`; `AgentClientCommand::SelectWorker
      { worker, id, session_id }` and `SetOmWorkers` + `OmActivity`;
      `BookSelection` gains `omObservation`/`omReflection` (serde-default,
      v1/v2 back-compat) — the workspace book records the binding through
      `AgentHost::select_worker` (same per-workspace path as
      `select_picker`) and `ensure_tab_session` passes the workspace's OM
      defaults into `NewSession` → `session.new`.
      `om.observationalMemoryWorkers` mapping helper
      (`json_om_workers`: both-absent omits the key; one-sided nulls
      clear). The connection intercept routes `SelectWorker` through
      `select_worker` (book + daemon) exactly like `Select`.
    - Frontend (`frontend/src/agent/state.ts`,
      `CodingAgentPanel.tsx`, module CSS): the `clay.agentRpc` handler
      caches `session.om.activity` / `session.om.set` responses in
      `omView`. The Memory tab is now `MemoryTab`: two `ClayDropdown`
      worker slots (Observation worker / Reflection worker) over the same
      grouped, configured-only model list as the session model (I3), with
      a "Not set (workers off)" clear entry injected into the rendered
      set (`options` is ignored when `groups` is present — fixed in the
      component wiring); selection sends `SelectWorker` (per-workspace
      book + per-session daemon metadata; may differ from the session
      model). Below, the server-authoritative activity log renders
      observation/reflection rows with fact summaries, visible drop rows,
      and fold rows — catalog list/label composition inside the
      `ClayTabStrip` panel, host-token vars only.
    - Tests: daemon `om.test.ts` drives the REAL worker pipeline through
      one prompt-routed fake provider (session turns answer with text;
      observer/reflector turns, detected by their worker tools, emit
      `record_observation`/`record_reflection` calls with ids parsed from
      the worker prompts): observe → reflect → drop all surface with
      summaries (drops visible), per-session selection persists to record
      metadata and restores via `ensureLive`, unattached sessions report
      `attached: false`, fail-closed (unknown provider, empty model,
      malformed shape). Rust: `om_worker_selection_writes_workspace_fields_and_round_trips`
      (book fields, clear, persist/reload) +
      `json_om_workers_maps_bindings_to_daemon_payload`. Frontend: the
      Memory-tab drill (fetch on open → activity rows incl. the drop row
      → server selection shown in the dropdowns → hidden-select change →
      `SelectWorker` with `worker`/`sessionId`).
    - Gates: cargo fmt --check clean; cargo clippy --all-targets --
      -D warnings clean; full cargo test 1220 lib + 6 + 41 + 207 + 73 +
      135, 0 failed; frontend 246 pass (34 files); clay-agent 93 pass /
      1 skip.

- [x] I9: `/resume` — workspace-scoped session list with full restore (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: A `/resume` slash command (and a panel affordance in the
      Session Info or Files empty state, consolidated — the buried
      Files-tab session list moves here) lists the current workspace's
      sessions (workspace-scoped inventory, decision 2201 metadata;
      bounded, most-recent first, with label/snippet). Selecting one
      resumes it on the tab: transcript restored, live context reloaded
      (agent state from the session), and the session's persisted
      provider/model applied (session metadata already carries
      `profile`/`provider`/`model`/`workspaceRoot` — apply them to the
      book so the status row and next run match). `/new` keeps starting a
      fresh session in the same workspace.
    - Performance: Reuses the existing bounded session inventory/FTS
      surfaces; no new index; resume path is the existing
      `session.load`/`resume` flow.
    - Code Quality: One workspace-scoped listing surface shared by the
      command and the panel (no duplicate filters); resume restores model
      through the same selection path as I2 (per-workspace book updated
      from session metadata).
    - Security: Workspace scoping is enforced server-side (never trust a
      client filter); resumed sessions never auto-inject into another
      session's context (decision 2201 rule).
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts` (`sessionLoad`, `sessionResume`,
        `ensureLive`, session metadata persistence), `src/server/agent.rs`
      (`resume_tab`, inventory, search), plan 108 task 11 (workspace-
        scoped search), pi `/resume` reference,
        `.agents/skills/project-patterns/references/agent-host.md`.
    - Options Considered:
      - Reuse the Command Centre SessionSearch picker only: search-first,
      not list-first; pi's /resume is a plain recent-sessions list.
      - `/resume` list surface (session picker with workspace filter) +
      search picker unchanged (chosen).
    - Chosen Approach:
      - Daemon: workspace-scoped session list (metadata filter, bounded);
      resume already restores live state — extend to apply the session's
      model to the book. Frontend: `/resume` opens the picker; selection
      routes through the existing resume intent. Post-110: picker stays
      Command Centre; Files-tab buried session list still moves here (I9
      original); no extra tab strip.
    - API Notes and Examples:
      ```ts
      // resume applies the session's model to the book (workspace key)
      await resume(sessionId); // → state.provider/model reflect the session
      ```
    - Files to Create/Edit:
      - `clay-agent/src/{host,rpc}.ts`: workspace-scoped list (if not
        already exposed), model application on resume.
      - `src/server/agent.rs`: workspace filter on inventory listing for
        the picker.
      - `packages/coding-agent/{package.json,dist/load.js}`: `/resume`
        command.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: picker affordance;
        remove the buried Files-tab session list.
    - References:
      - Decision 2201; plan 108 tasks 9/11.
  - Test Cases to Write:
    - Resume drill: sessions from another workspace never appear; resume
      restores transcript + context + model; a follow-up prompt continues
      the resumed session with that model.
    - Bounded list; empty-workspace state renders guidance.
  - Evidence (2026-09-06, implemented):
    - Daemon (`clay-agent/src/host.ts`): new `session.resumable
      { workspaceRoot, limit? }` RPC — workspace-scoped bounded session
      list (safe display fields only: sessionId/updatedAt/label/summary/
      redacted snippet/metadata), most-recent first via the store's
      `updated_at DESC` ordering. Fail-closed: requires a non-empty
      workspaceRoot (the surface never lists across workspaces) and
      reuses the existing `searchSessions` store seam — no new index.
      `session.search` (session-derived scope) is unchanged; the two
      surfaces share the redaction posture and limit bounds.
    - Rust: `AgentSessionInfo` gains a `label` display field (serde
      default; pickers fall back to the profile name). New
      `AgentHost::resumable_sessions(root, limit)` parses the daemon
      response; the Command Centre session-picker open path
      (`connection/menus.rs`) replaces the unscoped inventory sessions
      with the tab-registry-scoped list — the root comes from the tab
      registry (authoritative server state), never webview input; a tab
      without a workspace yields an empty list, not a cross-workspace
      dump. `picker_kind_for_command` maps the new package command
      `coding-agent.resume` to the Session picker, so the palette entry
      and the typed slash both ride the existing Command Centre resume
      flow (`agent.clientOpenSessionPicker` precedence).
    - Resume path (unchanged, verified): `resume_tab` binds the tab,
      restores transcript + context via `LoadSession`, applies the
      session's persisted provider/model to the per-workspace book from
      the snapshot (I2 selection path), and dispatches `ResumeSession`.
    - Package (`packages/coding-agent/package.json`): `coding-agent.resume`
      (`/resume`, server-first) declared for discovery. Panel:
      `/resume` added to the slash surface; submit intercepts it and
      sends the session-picker intent (same shape as the `/model`
      built-in). The Files empty state keeps its consolidated Resume
      session affordance — no buried session list remains in the tab.
    - Tests: daemon `resume.test.ts` — cross-workspace exclusion,
      most-recent-first order, bounded page, empty-workspace empty list,
      fail-closed without a root, and a restart drill (fresh host
      restores transcript entries + persisted model via `ensureLive`,
      and a follow-up prompt continues the resumed session); Rust —
      `parse_resumable_sessions_prefers_label_over_summary`,
      `session_picker_items_prefer_display_label`,
      `resume_command_ids_map_to_the_session_picker`; frontend —
      `/resume opens the workspace-scoped session picker` drill.
    - Gates: cargo fmt --check clean; cargo clippy --all-targets --
      -D warnings clean; full cargo test green (1223 lib + all suites,
      0 failed; package command count budget bumped 13 → 14); clay-agent
      96 pass / 1 skip; frontend 247 pass.

- [x] I10: Session Info tab as the card-detail destination (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: The right pane gains a fourth tab **Session Info**
      (Files, Observational Memory, Context, Session Info). Selecting a
      chat card in the transcript auto-selects Session Info and renders
      that entry's full detail: kind, complete content, and type-specific
      metadata (tool: name, bounded args/output; skill: name + body
      reference; thinking: full text; usage/error: text) — replacing the
      current above-tabs detail region, which is removed. Deselecting
      returns to the previously selected tab (or Files); Session Info with
      no selection shows guidance.
    - Performance: Detail renders from the already-loaded transcript
      entry; no refetch; tab switch is widget-local (`tabList`
      widget-local selection).
    - Code Quality: One detail renderer shared by the drawer needs of I7
      where kinds overlap (bounded-text viewer); no new component kind.
    - Security: Read-only rendering of redacted transcript content; no new
      data path.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `frontend/src/coding-agent/CodingAgentPanel.tsx` (current detail
        region + `ClayTabStrip`), `frontend/src/components/tab-strip.tsx`
        (`activeId` / `onActivate`).
    - Options Considered:
      - Keep detail above tabs + add tab: duplicates the surface.
      - Tab-only detail with auto-select (chosen): single destination,
      matches the requested UX.
    - Chosen Approach:
      - Controlled `ClayTabStrip` (`activeId`/`onActivate`); card select
      sets selection + switches to `session-info`; render per-kind detail
      from the transcript entry (I5 payloads included). Remove the
      above-tabs detail region.
    - API Notes and Examples:
      ```tsx
      <ClayTabStrip
        activeId={rightTab}
        onActivate={setRightTab}
        tabs={[/* files, memory, context, */
          { id: "session-info", label: "Session Info",
            content: selected ? <SessionInfoDetail entry={selected} /> : <EmptyHint /> }]}
      />
      ```
    - Files to Create/Edit:
      - `frontend/src/coding-agent/CodingAgentPanel.tsx` (+ CSS): fourth
        tab, controlled selection, detail renderer; remove the old detail
        region.
      - `frontend/src/coding-agent/CodingAgentPanel.test.tsx`: selection
        auto-switch + per-kind detail tests.
    - References:
      - Plan 108 task 8 (tab strip + detail region being replaced).
  - Test Cases to Write:
    - Card click selects Session Info with the right content per kind;
      Back/deselect restores the prior tab; keyboard flow reaches the tab
      and detail (a11y check in the review task).
  - Evidence (2026-09-06, implemented):
    - `frontend/src/coding-agent/CodingAgentPanel.tsx` (+ module CSS):
      the right pane is a four-tab controlled `ClayTabStrip` — Files,
      Memory (Observational Memory), Context, **Session Info**. The
      above-tabs detail region is removed (`.detail` CSS block deleted;
      the shared `.detailHeader`/`.detailContent` classes remain).
    - Card selection: `selectCard` sets the selection and auto-switches
      to `session-info`, remembering the tab the selection came from in
      `previousTabRef`; deselect (Back button or clicking the selected
      card again) restores that tab (default Files). Tab switching
      itself never clobbers the selection. All widget-local — React Aria
      Tabs selection, no refetch, no server round-trip.
    - `SessionInfoTab` renders the selected `TranscriptBox` from the
      already-loaded transcript: kind row, tool/skill name row (tool:
      name + bounded args/output — the I5 bounded digests ARE the
      content; skill: name as the body reference, body stays
      server-side), then the full redacted content. Thinking, usage, and
      error cards render their full text the same way. No selection →
      guidance copy.
    - Shared `BoundedText` viewer (read-only, redacted, wrapping) used
      by both the Session Info detail and the Context drawer item detail
      — one renderer where kinds overlap; no new component kind (plain
      function component over existing CSS classes).
    - Security: read-only rendering of the same server-redacted
      transcript content; no new data path.
    - Tests (`CodingAgentPanel.test.tsx`, 17 pass): card click selects
      Session Info with entry content and tool name/output metadata; Back
      restores the prior tab (Context fixture); Session Info with no
      selection shows guidance. The old above-tabs-region test was
      rewritten for the tab flow.
    - Gates: tsc --noEmit clean; frontend 249 pass (34 files). No
      daemon/Rust changes — pure panel-surface work.

- [x] R1 + R2 + R3: Daemon-sourced slash completion, real git branch, truthful extension strip (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Slash completion in the composer lists the
      daemon-registered commands for the session (single source: the
      Phase 1 command registry — `/compact`, `/new`, `/branch`, `/tree`,
      `/discard`, `/fork`, `/clone`, `/open-session*`, plus this plan's
      `/model`, `/resume`), not a hardcoded frontend constant (R1). The
      status row shows the workspace root's real git branch (bounded,
      cached, refreshed on session/workspace change and run completion;
      `—` only when not a git repo) instead of the hardcoded `git —`
      placeholder (R2). The extension strip reflects actual active
      extensions from daemon state (or omits the segment when none
      report), removing the hardcoded `Extensions: core` (R3).
    - Performance: Branch lookup is a cached background read (no per-prompt
      subprocess); command list arrives with session state, not per
      keystroke; extension list rides existing state events.
    - Code Quality: Branch detection is best-effort and failure-silent
      (`—` fallback); no new long-lived process; command-source-of-truth
      removes the drift class entirely.
    - Security: Git branch is a read of `.git/HEAD`-equivalent bounded
      data (implementation may read HEAD file directly rather than spawn
      git — smaller authority); no repo mutation; command list is
      registry data, already permissioned.
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs` (command registry inventory exposure —
        extend if the list isn't surfaced in state today),
        `frontend/src/coding-agent/CodingAgentPanel.tsx` (SLASH_COMMANDS,
        status row), `clay-agent/src/host.ts` (registered commands),
        `.agents/skills/project-patterns/references/protocol-and-performance.md`.
    - Options Considered:
      - Keep a frontend list synced by hand: drift already happened (this
      plan adds commands); rejected.
      - Daemon registry as the list source (chosen).
      - Git branch via spawned `git`: process authority for a string;
      prefer direct `.git` reads where correct (worktrees complicate —
      fall back to `git rev-parse` only if file reads prove
      insufficient, and then via the existing shell policy path).
    - Chosen Approach:
      - Expose registered commands (name+description, bounded) in session
      state; composer filters that list. Branch: cached server-side read
      keyed by workspace root. Extensions: from daemon state when
      available; omit when unknown.
    - API Notes and Examples:
      ```ts
      // completion source becomes state
      const commands = snapshot.state["commands"] as SlashCommand[] ?? [];
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts` / `src/server/agent.rs`: command list in
        state; branch lookup (server-side, cached); extension state.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: consume; delete
        `SLASH_COMMANDS`.
    - References:
      - Plan 108 task 7 (daemon slash registration — the registry exists).
  - Test Cases to Write:
    - Registered commands appear in completion; unregistered names don't;
      adding a daemon command appears without frontend changes.
    - Branch: repo → branch shown and refreshed on workspace change;
      non-repo → `—`; no per-prompt subprocess (assert cache hit in
      tests).
    - Extension strip truthful against a fixture with/without extension
      packages.
  - Evidence (2026-09-06, implemented):
    - R1 (daemon-sourced completion): new daemon RPC `environment.list`
      returns the kernel command registry — `{ commands, extensions }`
      where commands carry `{ name, description }` normalized to
      `/name` (bare kernel commands included; slash invocation reaches
      them through the same registry), bounded at 64 commands /
      48-char names / 96-char descriptions (`clay-agent/src/host.ts`
      `environmentList`). The Rust server caches one environment fetch
      per daemon generation (`Inner.environment`), invalidates on
      `command.register` / `knowledge.setOptions` success and on
      daemon death/timeouts (`Inner::rpc` wrapper), and parses
      bounded via `parse_environment`. Commands ride `STATE_SNAPSHOT`
      (`snapshot_events` emits `commands` only when non-empty — the
      client's merge keeps prior values on snapshots that omit it).
      The panel's completion source is `snapshot.state["commands"]`
      merged with `CLIENT_SLASH_COMMANDS` (`/model`, `/resume` — the
      two client-intercept built-ins, declared beside their intercepts);
      `SLASH_COMMANDS` (10 hand-synced entries) is deleted. New daemon
      commands appear without frontend changes.
    - R2 (real git branch): `read_git_branch` (src/server/agent.rs)
      reads the workspace's `.git` directly — `HEAD` ref, worktree/
      submodule `gitdir:` pointer files, detached-HEAD short sha; no
      subprocess, bounded to 80 chars, failure-silent. Refresh cadence:
      cached per workspace root keyed by a run generation
      (`SessionBook.branch_cache` + `run_generation`, bumped on
      Finished/Error) so prompts within one run are cache hits; run
      completion re-reads (the settled-run pump republish refreshes via
      the session's recorded root `session_root`); session/workspace
      changes re-read (`ensure_tab_session`, resume paths). Surfaced as
      `branch` in STATE; the status row renders `git {branch}` with
      `—` when empty.
    - R3 (truthful extension strip): the daemon reports loaded opt-in
      extensions (wiki/graft `LoadedExtension.name`s) in the same
      `environment.list` response; they ride STATE as `extensions`.
      The panel renders `Extensions: {list}` only when non-empty — the
      hardcoded `Extensions: core` is removed; the MCP segment keeps
      its existing truthful rendering.
    - Tests: Rust — `read_git_branch_parses_head_ref_worktree_and_detached`,
      `refresh_branch_caches_within_generation_and_rereads_after`
      (counting-reader cache-hit proof), `parse_environment_bounds_commands_and_extensions`,
      `snapshot_state_carries_environment_only_when_known`; daemon —
      `environment.list lists registered commands normalized to /name`;
      frontend — daemon-command completion (registered names complete,
      unregistered `/deploy` never does, branch + extension strip
      truthful with a fixture, `git —` + omitted segment without data).
    - Gates: cargo fmt --check clean; cargo clippy --all-targets --
      -D warnings clean; cargo test green (lib 1227; all suites pass);
      clay-agent 97 pass / 1 skip; frontend 250 pass; tsc clean.

- [x] Create or verify Clay JS APIs for public programmatic surfaces (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Every new public programmatic behavior from this plan
      has a documented Clay JS API with stable ID, user-facing name, key
      binding (or empty list), custom properties for behavior-changing
      settings, description, usage examples, configuration, return/async
      behavior, errors, permissions/security notes, backing Rust path, op
      wrapper, JS facade path, and lookup tags: `coding-agent.clientCycleEffort`
      (bindable client command, default Shift+Tab), any facade surface for
      model/OM-model selection or context inspection exposed to packages
      (expected: none beyond the client command — daemon RPCs stay
      internal; verify and record), and the `workspace.toggleFileBrowser`
      default-binding documentation update. IDs follow
      `clay-js-api-naming.md` (package prefix `coding-agent.` for package-
      owned; no `clay.*` spellings).
    - Performance: No new hot-path APIs; facade additions are one-shot
      registration/lookup surfaces.
    - Code Quality: Registry/inventory truth tests updated; `cargo test`
      fails on missing/stale docs, master-index links, registry entries,
      key-binding fields, or lookup entries.
    - Security: APIs grant no new filesystem/network/shell/daemon
      authority; permissions notes state what each does not grant.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`,
        `clay-js-api-boundary.md`, `clay-js-api-schema.md`,
        `documentation-as-code.md`, `doc-registry-tests.md`,
        `docs/reference/clay-js-api/keybindings/bind-key.md` (client
        command list — add the new id).
    - Options Considered:
      - Expose daemon RPCs as facades: packages don't need them (UI is
      trusted-module + existing facades); keep internal, verify boundary.
      - Document only the user-facing surfaces (chosen).
    - Chosen Approach:
      - Add the client command id to the bindKey allow-list + docs;
      verify no internal RPC leaked public; regenerate the registry.
    - API Notes and Examples:
      ```js
      // examples/init.js
      bindKey("Ctrl+M", clientCycleCodingEffort(), { scope: "editor" }); // example rebind
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**` (new/updated docs + index links),
        `src/server/ops/keybindings.rs` (allow-list), generated registry
        artifacts, `runtime/js/**` facade (only if a surface is promoted).
    - References:
      - Decisions `2026-05-08-1509`, `2026-05-08-1840`.
  - Test Cases to Write:
    - Registry truth: new docs linked, indexed, registry-generated;
      bindKey accepts the new command id; unknown ids still rejected.
  - Evidence (2026-09-06, verified + completed):
    - `coding-agent.clientCycleEffort` (I4): the id was already on the
      bindKey allow-list (`src/server/ops/keybindings.rs` lines 433/523)
      with the unit truth test `coding_agent_effort_cycle_is_bindable_and_client_ui_routed`
      (runtime-bindable + `RoutingPolicy::ClientUiCommand`); unknown ids
      still fail closed through the existing `keybindings.unknown_command`
      validation path. This task completed the documentation: bind-key.md
      now lists the id in the `command` option list (package-contributed
      coding-agent command), records the shipped package-manifest default
      `Shift+Tab` in Key bindings, and states the security boundary in
      Permissions and security (user-mediated effort cycle only; no
      model-provider management, OM worker-model selection, or
      context-inspection daemon RPC). ID follows `clay-js-api-naming.md`
      (package prefix `coding-agent.`, no `clay.*` spelling).
    - `workspace.toggleFileBrowser` (I6): default-binding documentation
      was shipped with I6 (bind-key.md Key bindings + Options list +
      configuration.md row + examples/init.js); re-verified present.
    - Facade surface verification (recorded): NO facade exists for
      model/OM-model selection or context inspection — `runtime/js/` has
      no coding-agent module and no export references any of the plan's
      internal daemon RPCs (`session.context`, `session.om.*`,
      `session.resumable`, `environment.list`) or the client command.
      Daemon RPCs stay internal; packages interact only through the
      existing trusted-module UI surface and `bindKey`. No
      api-inventory.toml entry is added because no `clay:*` facade
      surface was promoted (the generated registry is unchanged and
      current).
    - Registry truth test added: `plan109_coding_agent_client_command_docs_and_internal_rpc_boundary`
      (tests/clay_js_doc_registry.rs) — fails when bind-key.md drops the
      `coding-agent.clientCycleEffort` id, its `Shift+Tab` default, the
      reasoning-effort purpose, or the I6 `workspace.toggleFileBrowser`
      `Ctrl+B` default; and fails when any `runtime/js/*.ts` facade
      references an internal daemon RPC marker or a `coding-agent.ts`
      facade module appears.
    - Gates: cargo fmt --check clean; cargo clippy --all-targets --
      -D warnings clean; clay_js doc-registry + inventory suites 69/69;
      protocol suite green.

- [x] Create or verify Clay configuration APIs and update the canonical example configuration (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: Behavior-changing settings introduced by this plan are
      configuration APIs, not hidden keys: effort-cycle default binding
      (bindable), workspace-browser toggle binding (bindable), and the
      per-workspace model/OM-model selections (state persisted by the app
      itself — verify no undocumented config key is needed; if a config
      surface is added, it is a documented API with custom properties).
      `examples/init.js` is updated in the same sectioned, annotated style
      (new bindable ids present exactly once; active copy-paste-safe part
      stays safe; `node --check examples/init.js` passes).
    - Performance: Configuration resolution stays off hot paths.
    - Code Quality: Option names/enums/defaults in the example match the
      server-side parsers and `api-inventory.toml` custom properties.
    - Security: Configuration grants no implicit authority (bindings are
      inert routes to already-permissioned commands).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`,
        `docs/reference/clay-js-api/keybindings/bind-key.md`,
        `examples/init.js`.
    - Options Considered:
      - Separate per-workspace config file users edit by hand: the app
      owns this state; a config key would duplicate the book.
      - App-owned persisted state + bindable keys documented (chosen).
    - Chosen Approach:
      - Verify + document; update `examples/init.js` with the new bindable
      ids (commented examples for non-default bindings).
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js (example section)
      // Rebind the coding agent effort cycle (default Shift+Tab):
      // bindKey("Ctrl+M", clientCycleCodingEffort());
      ```
    - Files to Create/Edit:
      - `examples/init.js`; related API docs if fields changed.
    - References:
      - Decision `2026-05-08-1841`.
  - Test Cases to Write:
    - `node --check examples/init.js`; doc/registry cross-check gates
      green.
  - Evidence (2026-09-06, verified + completed):
    - Effort-cycle default binding (I4): bindable — `coding-agent.clientCycleEffort`
      is on the bindKey allow-list with a shipped package-manifest default
      (`Shift+Tab`) and documented bindKey semantics (previous task).
    - Workspace-browser toggle binding (I6): bindable — `workspace.toggleFileBrowser`
      ships a Global `Ctrl+B` default; `examples/config/init.js` keeps
      exactly one active idempotent re-declaration (asserted by the
      existing truth test) plus a commented rebinding example.
    - Per-workspace model/OM-model selections: app-owned persisted state
      (book v2 workspace map + per-session metadata, I2/I8) — verified no
      undocumented config key is needed and none was added;
      `docs/reference/clay-js-api/configuration.md` gained no plan-109
      keys (grep clean for clientCycleEffort/omWorkers/effortLevel), so
      no new configuration API surface is required. The effort dropdown,
      OM worker dropdown, and `/model`/`/resume` selections are UI state
      resolved server-side, not configuration.
    - Canonical example updated (`examples/config/init.js`, same sectioned
      annotated style): new "Coding agent:" entry in the bindable-ids
      header list (`coding-agent.clientCycleEffort  Shift+Tab`, shipping
      from the `@clay/coding-agent` package manifest) and a single
      commented rebinding example (`// bindKey("Ctrl+M",
      "coding-agent.clientCycleEffort", { scope: "global" });`) — the
      active, copy-paste-safe part is unchanged (copy-safe truth test
      scans active lines only and stays green).
    - Truth test extended: `canonical_example_active_configuration_is_copy_safe`'s
      sibling phase-22.8 canonical test now asserts the example references
      the effort-cycle id and keeps exactly one commented rebind form, so
      stale or duplicated examples fail `cargo test`.
    - Gates: `node --check examples/config/init.js` passes; clay_js
      doc-registry suite 69/69 (canonical tests included); cargo fmt
      clean; cargo clippy --all-targets -- -D warnings clean.

- [x] Execute and update the manual test plan (test-plan/) (completed 2026-09-06)
  - Acceptance Criteria:
    - Functional: The affected modules are identified via
      `test-plan/index.md` (agent host/protocol, coding agent UI,
      keybindings, workspace flows) and executed on a real Linux build;
      new numbered steps cover every user-visible behavior shipped here:
      workspace rebind, per-workspace model auto-load, `/model` +
      dropdown, effort control + rebinding, full transcript (tools/skills/
      thinking/steer), Files-tab editor view + tree toggle binding,
      context drawer + compaction reflection, OM activity + worker-model
      retention, `/resume` restore, Session Info auto-select, branch/
      extension strip. Pass/fail recorded per step; failures become
      defects or explicitly decided ceilings — never weakened steps.
    - Performance: Steps include the bounded/latency expectations where
      user-visible (transcript scroll, drawer open, resume time).
    - Code Quality: `test-plan/index.md` coverage matrix updated; steps
      cross-link deep-reference docs under `docs/development/` where they
      exist.
    - Security: Negative checks included (cross-workspace session
      leakage, unconfigured-provider filtering, redaction spot checks).
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` + affected module files; plan 108's manual
        modules for the surfaces being changed.
    - Options Considered:
      - Automated-only coverage: user-visible UX changes require manual
      drills; automated suites stay as regression nets.
      - Manual modules updated + executed (chosen).
    - Chosen Approach:
      - Add steps per shipped behavior; run on Linux; record results.
    - API Notes and Examples:
      ```text
      test-plan/<coding-agent module>.md — step IDs <module>.<n>
      ```
    - Files to Create/Edit:
      - `test-plan/*.md`, `test-plan/index.md`.
    - References:
      - Plan 108 manual modules; `docs/development/` deep references.
  - Evidence (2026-09-06, executed):
    - Module 17 (`test-plan/17-coding-agent-parity.md`) gained steps
      C1-C20 (workspace binding, per-workspace model auto-load, /model +
      dropdown, effort control + rebinding, full transcript with
      tool/skill/thinking/steer rows, Files-tab editor view + Ctrl+B
      toggle, context drawer + compaction reflection, OM activity +
      worker-model retention, /resume restore, Session Info
      auto-select, branch/extension strip, daemon-sourced slash
      completion, transcript budget feel, status truth) and negative
      checks C-N1-N4 (cross-workspace session leakage,
      unconfigured-provider filtering, redaction spot checks,
      fail-closed effort). Each step cites its pinning automated suite;
      the plan-108 P16/P17 ceilings are annotated as superseded by
      C4/C5/C15.
    - `test-plan/index.md`: module 17 map row extended with plan 109
      coverage; coverage matrix gained the "Plan 109 coding-agent
      defects/UX + Prism 0.5.0" row (17 + 16 + 10 + 14 + 01); dated
      execution record added.
    - Executed on a freshly rebuilt Linux build (`cargo build --bin clay`
      against the plan 109 working tree): all automated legs PASS
      (cargo test lib 1227 / protocol 208; clay-agent 97 pass / 1 skip;
      frontend 250 pass); live-build launch gate PASS via
      `scripts/capture-ui-review.sh --fixture ui-review-default` with
      the Coding Agent entry point visible in the AT-SPI tree
      (`test-plan/artifacts/109-coding-agent/launch-gate/`).
    - Recorded UNRESOLVED (host ceilings, not defects): interactive
      keyboard steps (no TTY/uinput input path - standing ceiling since
      plan 097) and real-provider streaming legs (no configured provider
      credential on this host; mock-provider automated suites pin the
      same code paths, plan 108 precedent). No step weakened.
    - Performance expectations recorded per step where user-visible
      (transcript scroll responsiveness at 200+ entries, bounded server
      snapshot 200 entries / 256 KB, drawer open, resume reload).
    - No new deep-reference docs needed; steps cross-link existing
      module citations (parity checklist, clay-agent wiki, plans
      108/109).
  - Test Cases to Write:
    - Each new step with expected result and negative check.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Evidence (2026-09-06, attempt 2):
    - PASS — launch gate: real Linux GUI build (debug, CLAY_AGENT_MOCK=1,
      isolated config/socket) opens with the welcome state; 'Coding Agent'
      button present in the WebKitGTK AT-SPI tree (probe:
      button|Coding Agent|829|622|111|42); window-cropped portal
      screenshots captured. Artifacts:
      `test-plan/artifacts/109-coding-agent/launch-gate/`,
      `test-plan/artifacts/109-coding-agent/agent-surface/`.
    - PASS — I6 workspace tree toggle exercised end-to-end via portal
      RemoteDesktop Ctrl+B (sidebar closes/reopens, 'Editor review.rs'
      landmark resizes 266..1280 x).
    - PASS — portal keyboard delivery proven (Shift+Tab, Ctrl+B change UI
      state).
    - UNRESOLVED — coding-agent surface internal states (model/effort
      dropdowns, transcript boxes, Session Info/Context/OM tabs, /resume
      picker, theme variants). Root cause chain recorded: (1) the Coding
      Agent button only exists in the empty-tab welcome state; once an
      editor tab opens there is no portal-input path back (dead end);
      (2) synthetic portal clicks land as focus-only on the webview
      button (focus ring, no activation; Enter/Space also inert);
      (3) reported window bounds drift across calls (x = 8/-318/963/632/
      -909/-1205 for the same window), so coordinate input targets the
      wrong screen region; (4) the Ctrl+X Ctrl+P command-centre chord does
      not reach the webview via the portal keyboard session.
    - Blocker class: synthetic-input reliability into WebKitGTK (wry),
      not an app defect — AT-SPI tree exposure of the full webview
      subtree is confirmed working.
    - Follow-up: drive activation via AT-SPI `perform_action` on the
      button node (no coordinates), pin the window with move_window
      before each event, and re-run the state matrix. Manual click on the
      welcome-state button also completes the review for a human runner.
  - Evidence (2026-09-06, attempt 3 — review COMPLETE, PASS with 3 live
    findings):
    - Method: human runner drove pointer/keyboard input (WebKitGTK button
      activation remains unreachable via synthetic input — portal clicks
      and AT-SPI `press` land focus-only; AT-SPI `do_action(0)` DOES
      switch page tabs, so the agent drove tab navigation directly), plus
      a background 2 s capture loop over the interaction window so no
      state needed timed coordination.
    - PASS — four-tab ClayTabStrip: Files/Memory/Context/Session Info
      all render with correct per-tab empty-state guidance and AT-SPI
      page-tab roles with SELECTED state tracking
      (files-empty/memory-empty/context-empty/session-info-empty.png).
    - PASS — R1 slash completion live: typing `/` in the composer opens
      the completion popup sourced from the daemon registry with full
      descriptions (`/model`, `/resume`, `/compact`, `/new`, `/n`;
      slash-completion-popup.png); inline hint bar while typing a prefix
      (slash-completion-model-hint-configured.png); Control Centre lists
      daemon commands with `server-first — @clay/coding-agent@0.1.0`
      routing labels (control-centre-daemon-commands.png).
    - PASS — /model picker flow: provider palette with configured / not
      configured labels (provider-picker*.png), live model switching
      (`opencode-go/grok-4.5` → `opencode-go/mimo-v2.5`) reflected in
      the status-row chip and panel header `coding ·`.
    - PASS — transcript box kinds user / thinking / assistant / usage in
      one chronological card (transcript-files-pane.png,
      memory-tab-workers.png); usage segment `3844 in / 56 out` also in
      the status row; streaming state shows `Streaming` status, Cancel
      button, and 'Steer the agent, or wait' composer placeholder
      (streaming-state-cancel.png). Tool/skill/error kinds not exercised
      live (no tool-calling run on this host); their rendering is pinned
      by frontend tests and the digests are redacted server-side.
    - PASS — Memory tab: Observation worker / Reflection worker
      dropdowns render 'Not set (workers off)' with helper text
      (memory-tab-workers.png).
    - PASS — Session Info: clicking a transcript card auto-selects the
      Session Info tab and renders per-kind detail (user kind:
      session-info-user-detail.png; assistant kind:
      session-info-assistant-detail.png) with a Back affordance and the
      selected card highlighted in the transcript.
    - PASS — narrow 760 px window: split layout holds, tab strip
      truncates gracefully (narrow-760-with-sdui-error.png).
    - PASS — a11y: full webview subtree exposed in AT-SPI with correct
      roles (page tab list 'Agent detail', log 'Transcript', form entry
      'Message', buttons Send/Close); tab switching via AT-SPI worked;
      keyboard-only flows exercised by the human runner.
    - FINDING 1 (defect, follow-up): Context tab stays on 'Loading
      context…' indefinitely after a completed run
      (context-stuck-loading.png) — the session.context response never
      renders in the live build, though the same path passes frontend
      and daemon tests.
    - FINDING 2 (defect, follow-up): the Files-tab 'Resume session'
      button surfaces `invalid SDUI message:
      UnknownActionCommand("agent.clientOpenSessionPicker")` in the
      status bar and no picker opens (narrow-760-with-sdui-error.png
      status bar) — the button intent path is rejected where the
      composer `/resume` intercept path is expected to work.
    - FINDING 3 (defect, follow-up): status row shows `git —` although
      the tab workspace is a real git repo on branch
      `feature/coding-agent` — the R2 branch readout did not populate
      live even after a completed run, despite passing unit tests.
    - Note: the in-header Model dropdown never rendered even when
      configured (selection worked via the /model Command-Centre flow
      only), consistent with the AG-UI snapshot carrying no models
      list; the effort dropdown was correctly absent because the models
      used declare no thinking levels.
    - UNRESOLVED — coding-agent chrome under core / neobrutal / glass ×
      dark/light content themes: would require an app restart per theme
      combination; the same recipe surfaces are covered by the
      design-system capture fixtures (ui-review-design-*) and frontend
      tests. Recorded as the one open leg of visual acceptance.
    - Artifacts: `test-plan/artifacts/109-coding-agent/live-walk/`
      (18 screenshots). Execution record: test-plan/17-coding-agent-parity.md
      'Live surface-state walk (attempt 3, 2026-09-06, human-in-the-loop
      + AT-SPI do_action)' row.
  - Acceptance Criteria:
    - Functional: A real Linux GUI build is launched with representative
      data and every changed state exercised and screenshotted: default
      panel, model dropdown open (grouped), effort dropdown + status-row
      effort, full transcript with all box kinds (user/thinking/tool/
      skill/assistant/usage/error), Session Info auto-select per kind,
      Context categories + drawer + post-compaction state, OM activity
      list + model selection, `/resume` picker, Files tab following
      selection + empty state, workspace tree toggle states, and narrow
      window layouts for the four-tab right pane. After plan 110, also
      capture coding-agent chrome under core / neobrutal / glass × one
      dark and one light content theme so recipe restyle is visible
      (`ClayTabStrip`, dropdowns, transcript boxes). Screenshot paths and
      findings recorded in task evidence. Plan 110 leftover tasks 16–18
      (wiki, tab-frame oversize, DS reload deadlock) are not this plan's
      work; if they block GUI launch, record the blocker and leave visual
      acceptance unresolved.
    - Performance: Screenshot review confirms no visible jank on
      transcript streaming and tab switches.
    - Code Quality: With `computer-use-linux` available: `get_app_state`
      first, accessibility-tree inspection, keyboard-only flows (composer
      → completion → tabs → drawer → dropdowns), focus visibility/order,
      roles/names/states, live-region announcements for streaming and
      approval; re-observe after interactions. Blockers recorded if
      unavailable; automated structural/a11y checks preserved; visual
      acceptance never claimed from source inspection alone.
    - Security: No secret/redaction regressions visible in screenshots
      (fixture-based check).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md` (Step 1),
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
    - Options Considered:
      - Structural tests only: explicitly insufficient per the decision
      log.
      - Full visual + a11y pass with screenshots (chosen).
    - Chosen Approach:
      - Batched review pass: launch, exercise states, screenshot, a11y
      walk, fix findings, one confirm round.
    - API Notes and Examples:
      ```text
      review artifacts under test-plan/ or docs/development/ review path
      ```
    - Files to Create/Edit:
      - Review artifact path + findings recorded in this plan's task
        evidence; defects filed as follow-ups or fixed in-plan.
    - References:
      - Decision `2026-08-14-0200`.
  - Test Cases to Write:
    - Screenshot set covering every changed state; a11y checklist results
      recorded.

- [ ] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` and
      `docs/reference/ui-components.md` describe the coding-agent trusted
      module on post-110 primitives (`ClayTabStrip`, grouped `ClayDropdown`,
      recipe two-layer fallbacks). No product-named pane kind. Third-party
      replacements stay generic SDUI. Catalog files updated only if a
      remaining UI task added a generic kind (expected: none).
    - Performance: Docs only; no runtime work.
    - Code Quality: Docs match shipped APIs; Phase 20.8 drift tests pass.
    - Security: Authoring notes keep inert contributions, no Tauri IPC,
      no raw ops.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `docs/reference/packages/creating-packages.md`,
        `.agents/skills/project-patterns/references/package-ui-layout.md`.
    - Options Considered:
      - Skip because 110 already updated catalogs: rejected — 109 still
        changes coding-agent composition (fourth tab, Files editor view).
      - Update authoring contract after UI tasks (chosen).
    - Chosen Approach:
      - Patch creating-packages + ui-components for the four-tab agent
        surface and Files-as-editor-view; no new kind.
    - API Notes and Examples:
      ```text
      docs/reference/packages/creating-packages.md
      docs/reference/ui-components.md
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`
      - `docs/reference/ui-components.md`
      - `.agents/skills/clay-ui/references/components.md` only if needed
    - References:
      - Decisions `2026-06-09-1431`, `2026-08-21-2152`.
  - Test Cases to Write:
    - Doc/registry drift tests green; coding-agent composition described.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all
      implementation tasks complete: coding-agent module pages reflect the
      workspace binding, book v2, transcript pipeline fix, context/OM
      inspection RPCs, the four-tab right pane, and Prism 0.5.0 family
      pins / MCP SDK v2 / thinking adapter; master index links updated.
    - Performance: Wiki notes performance-relevant details (snapshot
      budgets, cached branch reads, on-demand context fetch, 0.5.0 pin
      with no persisted-schema migration).
    - Code Quality: Pages explain what changed code does, how it works,
      invariants/tradeoffs, source/test paths, and links from the index.
    - Security: Touched boundaries (workspace scoping, redaction on new
      payloads, keybinding allow-list, MCP allow-list + child-env, locked
      keychain vs empty vault) documented without secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Update per task: noisy; once after tests pass (chosen).
    - Chosen Approach:
      - One wiki pass at the end using `project-wiki`, then `graft build`.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<coding-agent pages>
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/**` (touched pages).
    - References:
      - `.agents/skills/project-wiki/SKILL.md`.
  - Test Cases to Write:
    - Manual wiki review: index links relevant pages; pages explain the
      changed implementation.

## Compromises Made

- Known before remaining execution: I4 was blocked on Prism 0.4.0 thinking
  defects; remaining 109 work waits on the 0.5.0 pin tasks added above.
  Original I4–I10 / R1–R3 / closing tasks are retained, not dropped.
- Plan 110 landed mid-109. Remaining UI binds to `ClayTabStrip` + recipe
  consumption instead of pre-110 hand-rolled tabs. Original catalog task
  (2026-09-05) stays as I1–I3 evidence; a post-110 recatalog task covers
  I4–I10.
- Image support stays deferred (roadmap Phase 2.1 bullet).
- Plan 110 tasks 16–18 remain 110-owned.

## Further Actions

- Image support (roadmap Phase 2.1) after this plan.
- To be filled after remaining tasks complete with extra improvements,
  rationale, and priority.
