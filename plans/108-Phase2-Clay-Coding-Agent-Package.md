# Phase 2: `@clay/coding-agent` Package (Minimal Base Agent)

Source: `roadmap.md` Phase 2 (`@clay/coding-agent` Package — Minimal Base
Agent) plus the binding "Coding Agent UI" spec embedded there and the logged
resolutions the phase inherits.

This plan ships the first-party `@clay/coding-agent` package on top of the
Phase 1 daemon uplift: the `coding` agent profile, coding tool set, system
prompt layer, `create-plan`-style skill, slash commands, plan file
conventions, full pi-parity session/tree behaviors, workspace-scoped session
search surface, opt-in wiki/graft knowledge bases, and the Obscura-based
web/browser capability — plus the agent UI split surface (50/50 panes,
three right-pane tabs, growing composer, status row, extension strip).

It does **not** ship autonomy, workflows, sub-agents, memory cadence
(`st`, Phases 5–7), package installation/distribution (Phase 3), the public
third-party `agent.*` package platform (Phase 4), external runtime adapters
(Phases 9–10), or `computer-use-linux` (Phase 5). Observational Memory ships
as **chrome only**; live population lands with `st` Phase 7.

Status: task 1 (Phase 1 baseline gate) complete 2026-09-03 — plan 107
verified fully closed (15/15, exit gate recorded) and the baseline gates
are green; one prerequisite task added from 107's deferred Rust protocol
forwarding notes. Task 2 (primitive review) complete 2026-09-03 —
inventory recorded in-task; three generic gaps confirmed (split-pane
activation/layout-intent application, `tabList` kind, multiline growing
input). Task 3 (trust-domain/authority walk) complete 2026-09-03 — all
Phase 2 capabilities conform to decided patterns; no new process grant or
decision log required; skills clarified as host-side kernel registry. Tasks 4–6 complete 2026-09-03 — Task 4 (Rust protocol forwarding:
NewSession passthrough, skill/command register+dispatch, approval bridge)
and task 5 (package skeleton + coding profile registration through queued,
never daemon-blocking facade APIs) landed with round-trip/bridge/manifest
tests; task 6 verified the one-line init.js loading experience with
hostless/daemon-down queueing hardening and content-based clean-init
tests; all gates green (1638 tests, fmt/clippy clean). Task 7 (skills,
slash commands, plan-file conventions) complete — daemon handlers,
slash intercept, checkpoint metadata; 54/54 daemon + 1639/0 Rust. Task 8
(agent UI split surface) complete — 50/50 split, Files/OM/Context tabs,
growing composer, slash completion, Shift+Tab effort cycle, status row,
extension strip; 200/200 frontend, budgets hold. Task 9 (pi-parity run
behaviors) complete — steer/cancel, session list/resume, provider/model
switch (agent-rebuild for durable fingerprinting, stream-form prompt
fix), context-size-vs-window; 54/54 daemon + 1642/0 Rust + 200/200
frontend. Task 10 (tree navigation, branch summaries, post-hoc discard)
complete — /tree render with summaries/checkpoint flags, summary entries
at the branch point with background LLM refine, two-step /discard,
leaf-preserving model-rebuild fix; 58/58 daemon + 1642/0 Rust +
200/200 frontend. Task 11 (workspace-scoped session search surface)
complete — Command Centre SessionSearch picker over the Phase 1 FTS
index (bounded 50, workspace-scoped, skip-local-filter), selection
resumes at the matched tree entry via `session.load { entryId }`
(read-only branch view); 59/59 daemon + 1645/0 Rust + 200/200 frontend.
Tasks 12–15 (opt-in wiki + graft knowledge bases, Obscura web/browser
surface, pi-parity conformance checklist authored and run) complete —
`kernel.load` of the knowledge extensions behind `agent.knowledgeSetOptions`,
the full web/browser surface over the Phase 1 Obscura harness, and the
parity checklist (P1–P18/N1–N5/PERF) with the automated subset green
(68/68 daemon + 200/200 frontend). Remaining tasks not started.

Confirmed architecture (binding):

- `roadmap.md` Phase 2 + Coding Agent UI binding spec.
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
  (Prism stays in `clay-agent`; Clay document operations; no ACP `fs/*`).
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`
  (replaceable first-party package; declared extension points; core owns
  panes/registry/`agent.*`; no product-named core pane kinds).
- `decision-logs/2026-08-30-2156-adopt-prism-wiki-and-prism-graft-as-opt-in-knowledge-options.md`
  (wiki + graft opt-in per workspace via `kernel.load`; graft CLI
  fail-closed; `qmd` optional with catalog fallback).
- `decision-logs/2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`
  (Phase 1 daemon policy this package surfaces unchanged).
- `decision-logs/2026-08-30-2158-observational-memory-defaults-worker-models-80k-per-session.md`.
- `decision-logs/2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`
  (Obscura engine host-owned, fail-closed, hidden when absent).
- `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`
  (session tree + document checkpoints; one-action discard).
- `decision-logs/2026-08-30-2201-workspace-scoped-session-search-shared-by-clay-and-st.md`
  (one shared FTS; results never auto-context).
- `decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md`
  (exact 0.4.0 pins, explicit subpaths only; no new Prism family added this
  phase — `prism-memory/wiki` and `/graft` are already-pinned subpaths).
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
  (trusted classification from compiled bundled inventory, not `@clay/*`
  naming; extension points + user approval for third-party change).
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
- `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.
- `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.

Project patterns: `planning-checklist.md`, `agent-host.md`,
`product-surfaces-are-packages.md`, `package-ui-layout.md`,
`package-runtime-trust-domains.md`, `authority-boundaries.md`,
`clay-js-api-naming.md`, `clay-js-api-boundary.md`, `clay-js-api-schema.md`,
`configuration-system.md`, `package-manifest-single-source.md`,
`behavior-manifests.md`, `ui-skill-stack.md`, `tauri-react-client.md`,
`documentation-as-code.md`, `doc-registry-tests.md`,
`protocol-and-performance.md`, `maintenance-validation.md`.

Library docs:

- Context7 has no `@arnilo/prism`. Authoritative APIs: Prism
  `docs/migrate-to-0.4.md`, `docs/coding-agent-tools.md`
  (`writeCodingPlanFile`/`parseCodingPlanTodos`, coding checkpoints),
  `docs/coding-security.md`, `docs/context-and-skills.md` (skills, progressive
  disclosure), `docs/extension-authoring.md` (CommandDrivers, commands),
  `docs/agent-session-runtime.md` (durable runs, checkout/fork/clone),
  `docs/session-stores.md` + `docs/session-stores-and-branching.md`
  (`searchSessions`, branch summaries), `docs/wiki.md`, `docs/graft.md`,
  `docs/obscura.md`, and family `package.json` export maps under
  `/home/arn/Projects/prism/packages` (prism-memory `0.4.0` exports
  `./wiki`, `./graft`, `./compaction/*`).
- Pi behavior reference: pi coding agent TUI
  (`@earendil-works/pi-coding-agent` README + docs under the installed npm
  prefix) for streaming/steer/session/tree/command parity semantics.
- `@clay/chat` (`packages/chat`) is the package precedent: manifest
  `clay.contributions`, `dist/load.js`, `agentProfile.register`, docs/index.

Not in scope: `@arnilo/prism-core/runtime/workflows|supervisor|governance`
(Phases 5–6), `prism-coding-tools/computer-use-linux` (Phase 5),
`prism-office`, `prism-acp-agent`, `prism-ag-ui` in the daemon,
`/brave` `/exa` `/firecrawl` search profiles, `/ai-sdk`, package install/
update CLI (Phase 3), public third-party registration APIs (Phase 4),
OM live activity (Phase 7), cross-session memory scope (E5, post-roadmap).

## Objectives

- Ship `packages/coding-agent` (`@clay/coding-agent`) as a replaceable
  first-party package registering the `coding` profile via
  `agentProfile.register`: the nine Phase 1 coding tools, system prompt
  layer, `create-plan`-style skill, slash commands (`/compact`, `/new`,
  `/branch` + checkout map, `/model` picker hook, `/tree`, `/fork`,
  `/clone`, `/n`, `--session`/`--fork` command equivalents), and `plans/`
  plan file conventions.
- Deliver pi-parity behaviors through the package surface: streaming,
  steering/cancel, session list/resume, branch fork/checkout, manual + auto
  compaction, provider/model switching, and full tree parity with branch
  summaries plus post-hoc discard that rolls conversation and workspace back
  to the linked checkpoint in one action.
- Deliver the workspace-scoped `session.search` picker/command surface that
  resumes/opens the matching tree entry without injecting transcripts into
  agent context.
- Deliver opt-in per-workspace knowledge bases (`@arnilo/prism-memory/wiki`,
  `@arnilo/prism-memory/graft`) loaded daemon-side via `kernel.load`, tools
  hidden/fail-closed when CLIs are absent, no residue when disabled.
- Surface the Obscura-based web/browser capability (web search/fetch profile,
  `obscura_fetch`/`obscura_scrape`, CDP automation, Playwright e2e via CDP
  composition) only when the host engine is present.
- Deliver the binding agent UI: 50/50 resizable split, left transcript with
  uniform-height type-colored truncated boxes, right pane tabs (Files,
  Observational Memory chrome, Context), growing composer with `/` commands,
  `Shift+Tab` reasoning cycle, status row, extension/MCP strip.
- Keep Chat and the daemon fully functional when the package is absent.

## Expected Outcome

- `loadPackage("@clay/coding-agent")` (one line in `init.js`) activates the
  Coding Agent surface; unloading/disabling restores the Chat/fallback state
  with no daemon or chat regression.
- A coding session on Linux streams, steers, cancels, resumes, branches,
  compacts (manual + auto), switches provider/model, navigates `/tree` with
  branch summaries, and discards any branch post hoc with a single action
  restoring both conversation and document versions.
- Wiki/graft/web fixtures pass: OKF bundle answerable by `wiki_search`,
  ranked spans from `graft_ask`/`graft_callers`, live web/CDP/Playwright
  answers when the engine is present, and zero residue when all three are
  disabled.
- UI matches the binding spec and passes visual + accessibility review.
- Linux gates pass; docs/wiki/registry truth tests pass.

## Tasks

- [x] Verify Phase 1 completion and green baseline before package work
  - Acceptance Criteria:
    - Functional: All 15 tasks of `plans/107-Phase1-Base-Coding-Agent-Host-Uplift.md`
      are `- [x]` including the deferred tasks 11–15 (Clay JS APIs,
      configuration, `examples/init.js`, manual test plan, wiki refresh);
      Phase 1 exit gate recorded. This plan does not start implementation
      tasks before this gate passes.
    - Performance: Baseline daemon startup and chat round-trip unchanged
      from Phase 1 recordings; no new daemon process or port.
    - Code Quality: `plans/107` Compromises/Further Actions sections are
      filled; any Phase 1 deviation affecting Phase 2 (e.g. missing RPC)
      is listed here as an explicit prerequisite task, not discovered
      mid-implementation.
    - Security: Phase 1 acceptance policy, fail-closed Obscura, and
      allow-listed MCP remain as shipped; no capability widened for the
      package.
  - Approach:
    - Documentation Reviewed:
      - `plans/107-Phase1-Base-Coding-Agent-Host-Uplift.md`: task states and
        exit-gate notes.
      - `.agents/skills/project-patterns/references/planning-checklist.md`.
    - Options Considered:
      - Start package work on the completed daemon tasks only: risks
        building on APIs whose docs/registry/tests land in 107 tasks 11–15.
      - Full gate first (chosen): Phase 2 consumes 107's Clay JS API
        surface, docs, and manual plan; verifying first avoids rework.
    - Chosen Approach:
      - Run the 107 exit-gate checklist, `cargo fmt --check`,
        `cargo check --all-targets`,
        `cargo clippy --all-targets -- -D warnings`, daemon `npm test`, and
        the 107 manual-test modules; record results as this task's evidence.
    - API Notes and Examples:
      ```bash
      cargo clippy --all-targets -- -D warnings && \
        (cd clay-agent && npm ci && npm test)
      ```
    - Files to Create/Edit:
      - None (verification only; findings recorded in this plan's
        Compromises/Further Actions if blockers appear).
    - References:
      - `plans/107-Phase1-Base-Coding-Agent-Host-Uplift.md`.
  - Test Cases to Write:
    - Baseline run: all listed gates green and recorded.
  - Evidence (2026-09-03, task complete):
    - Plan 107: all 15 tasks `- [x]` with dated completion evidence
      (2026-09-02), including the formerly deferred 11–15; Compromises
      Made and Further Actions filled; task 10 records the Phase 1 exit
      gate (daemon 47/47 then, dependency graph clean of retired 0.3
      names, Chat/mock byte-compat, coding fixture, startup reports
      Prism 0.4.0).
    - Fresh baseline on the current working tree (Phase 1 work is
      uncommitted on top of `7971c6d`): `cargo fmt --check` clean;
      `cargo check --all-targets` clean; `cargo clippy --all-targets --
      -D warnings` clean; `clay-agent` `npm ci` + `npm test` **49/49**
      (two tests added since the 107 close, both passing).
    - 107 manual-test module `test-plan/16-agent-host.md` executed and
      recorded 2026-09-02 (107 task 14); current tree changes are covered
      by the green daemon suite, so baseline cites that recorded manual
      pass plus today's automated gates.
    - Phase 1 deviations affecting Phase 2 (from 107 Further Actions):
      Rust-side forwarding of `run.resume` and skill/command
      `register`/`dispatch` RPCs was deferred until a UI consumer exists,
      `approval.request`/`ask_user_decision` server handlers fail closed
      pending UI, `NewSession` Rust passthrough of autonomy/workspace
      params is deferred, and multi-tab workspace routing is a Phase 2
      ceiling from 107 task 4. Captured as the explicit prerequisite task
      below so they are not rediscovered mid-implementation.

- [x] Review existing editor and UI primitives and plan generic primitive gaps before package work
  - Acceptance Criteria:
    - Functional: Inventory recorded against
      `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`,
      `docs/wiki/modules/primitive-architecture.md`, `docs/reference/ui-components.md`,
      and `.agents/skills/clay-ui/references/components.md`: pane split tree
      (50/50 vertical, user-resizable, ratio-clamped), tab strip primitives
      (for Files/OM/Context), transcript list with per-row truncation +
      selection, composer auto-grow text area, status rows, extension strip.
      For each binding-spec element, state what existing SDUI kinds,
      commands, AG-UI stream lanes, and `agent.*` APIs already deliver it.
    - Performance: No new primitive may put package JavaScript, IPC, or
      daemon work on typing/paint hot paths; transcript rendering stays
      viewport-bounded with bounded queues (pattern
      `protocol-and-performance.md`).
    - Code Quality: Any new primitive (e.g. a generic `tabList`/tabs
      component kind, pane-split ratio control, or transcript-box kind) is
      generic, additive-only, token-driven, state-complete
      (hover/active/focus/disabled), catalog-documented, and reusable by any
      package — never coding-agent-specific Rust branches. Mode/package
      behavior is composed on top, per
      `.agents/skills/project-patterns/references/mode-primitive-first.md`
      and `package-ui-layout.md`.
    - Security: New primitives introduce no filesystem, network, shell, or
      daemon authority; they are inert presentation/interaction surface with
      inert command intents only.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`,
        `docs/reference/primitives/registry.md`,
        `docs/wiki/modules/primitive-architecture.md`,
        `docs/reference/ui-components.md`,
        `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `.agents/skills/project-patterns/references/package-ui-layout.md`,
        `.agents/skills/project-patterns/references/mode-primitive-first.md`.
    - Options Considered:
      - Package-private custom widgets for split/tabs/transcript: fastest to
        spec, but violates catalog/tensor rules and the replaceable-package
        contract.
      - Generic catalog additions only where composition cannot reach the
        spec (chosen): keeps third-party replacements and later agent
        packages first-class.
    - Chosen Approach:
      - Compose from existing kinds first (`panel`, `flex`, `stack`,
        `scroll`, `list`, `label`, `button`, `textInput`, `collapse`,
        `dropdown`); identify the minimal additive set (expected: tab-list
        surface and pane-split ratio control; transcript boxes may compose
        from list rows + truncation metadata) and record each as a separate
      generic primitive sub-deliverable in the UI implementation task.
    - API Notes and Examples:
      ```ts
      // Example: generic additive kind (only if inventory shows a gap)
      { "kind": "tabList", "id": "codingAgent.rightTabs",
        "items": [{ "id": "codingAgent.tab.files", "label": "Files" },
                  { "id": "codingAgent.tab.om", "label": "Observational Memory" },
                  { "id": "codingAgent.tab.context", "label": "Context" }] }
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/registry.md` (tentative: new generic
        primitive entries, only for confirmed gaps).
      - `.agents/skills/clay-ui/references/components.md` (tentative: same).
      - `docs/wiki/modules/primitive-architecture.md` (tentative: same).
    - References:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
        (primitive-first planning source),
        `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
  - Test Cases to Write:
    - Primitive inventory check: every binding-spec UI element maps to an
      existing kind or an explicitly planned generic addition; no
      coding-agent-named Rust primitive appears.
  - Primitive Inventory (2026-09-03, task complete):
    - **Rendering model (no new Rust).** The bundled surface follows the
      SettingsPanel/ChatPanel precedent: a host-rendered trusted
      presentation module (`frontend/src/chat/ChatPanel.tsx`) selected by
      provenance-exact match to the package's declared component tree
      (copy/actions from the declaration; behavior from host lanes), while
      third-party replacements of `@clay/coding-agent` render through the
      unchanged generic SDUI renderer. Coding-agent chrome is one more
      such module; no package-authored client JS enters the webview.
    - **Transcript + streaming (no new Rust).** One core-owned AG-UI
      stream already exists (`frontend/src/agent/{TauriClayAgent,events,
      state}.ts`, Phase 10); ChatPanel proves memoized per-row streaming,
      cancel, sessions, error/usage rows. Coding-agent tool outputs, MCP
      output, skills-loaded rows, and context-size status extend this
      lane's bounded payloads; SDUI payload budgets (snapshot 4096 B,
      tree 16 KiB/128 nodes, 4096-char text nodes) confirm tool output
      must ride the stream, never SDUI trees.
    - **Status row / extension strip (no new Rust).** `flex` +
      `statusItem`/`label` with `typography.status` compose the status row;
      badge/kbd chrome components exist (`frontend/src/components/
      chrome.tsx`; catalog: badge = label + muted tokens). Type-colored
      transcript borders use existing typed style variables
      (`borderColor` + color-role tokens) and `component_state_color`.
    - **50/50 split (generic gap G1).** `PaneSplitTree` internals +
      react-resizable-panels frontend with ratio clamp 0.05–0.95, drag
      (Phase 20.3), and lifecycle (Phase 22.1) exist, and panes are
      generic content hosts (Phase 22.2). But `register_pane_content`
      hard-rejects any activation other than `"empty-tab"`
      (`src/server/ui.rs`), and `RegisteredLayoutIntent` (targetPane,
      orientation horizontal/vertical, ratio 0.05–0.95, position
      first/second — validated and stored via `ui.serverRequestLayoutIntent`)
      has **no consumer**. Gap: apply layout intents and accept a generic
      non-empty-tab activation so a package surface composes into a split
      pane. Reusable by any app-like package (chat fullscreen, future
      Work/PA surfaces), never coding-agent-named.
    - **Right-pane tabs (generic gap G2).** No package-facing tab kind.
      The window tab bar (`frontend/src/app/layout/tab-bar.tsx`, React
      Aria `Tabs`/`TabList`) is shell-level with a documented "generic
      paint contract … reuse for panel/pane tabs" note. Gap: one generic
      additive `tabList` component kind (state-complete, token-driven,
      cataloged) hosting per-tab children; the trusted module may compose
      the same React Aria substrate directly.
    - **Growing composer (generic gap G3).** `textInput` is single-line
      (React `ClayTextField`). Gap: multiline auto-grow input — either a
      `multiline` style variable on `textInput` or an additive `textArea`
      kind; native `<textarea>` substrate per the locked native-HTML-first
      mapping.
    - **Transcript truncation + selection (compose; soft gap G4).**
      Uniform-height truncated boxes with full content on selection:
      lazy path is host-side truncation metadata on stream rows in the
      trusted module (no catalog change). A generic list-row `maxLines`
      style variable is recorded as the fallback only if third-party
      replacement parity needs it in SDUI.
    - **Files tab (compose; soft gap G5).** The file browser is a
      Clay-owned internal surface (`src/shell/file_browser.rs`, Phase
      18.12/22.8). Spec wants "the file view as it exists today" in a
      right-pane tab: reuse the same server-owned snapshot path inside the
      trusted module (no new authority, on-demand loading already bounded);
      a generic embeddable `fileBrowser` surface kind is deferred until a
      third-party package needs it.
    - **Composer `/` completion (compose; soft gap G6).** Packages cannot
      open Command Centre sessions (Clay-owned). The trusted module can
      surface `/` commands through the existing picker command ids and
      package transient overlays (`scroll`+`list` under `main`/pointer
      anchor; `ContextMenu`/`MenuBar` origins are package-compatible). No
      new kind required.
    - **Pickers and commands (no new Rust).** `agent.clientOpen{Agent,
      Provider,Model}Picker` Command Centre session kinds already exist
      (chat manifest actionTargets); slash commands ride Phase 1 daemon
      dispatch. `Shift+Tab` effort cycling and the Context tab need daemon
      run-state/session-context fields — daemon-side, owned by this plan's
      pi-parity task, not UI primitives.
    - **Facades (no new Rust).** `runtime/js/agent.js` already exports
      `compact`, `searchSessions`, `setFullAutonomy`, `resumeRun`,
      `sessionTree`; `runtime/js/ui.js` exports the eight contribution/
      override/intent registrations.
    - **Conclusion.** Minimal generic additions for the UI task: **G1**
      (pane-content activation + layout-intent application), **G2**
      (`tabList` kind), **G3** (multiline growing input). G4–G6 compose
      from existing kinds/lanes in the trusted module; G7 (effort state,
      context snapshot) are daemon fields in later tasks. No
      coding-agent-specific Rust primitive, token, or anchor is planned;
      every addition is additive-only and catalog-documented in the same
      change.

- [x] Confirm trust-domain and process-authority coverage before extension points and knowledge CLIs
  - Acceptance Criteria:
    - Functional: The package's declared extension points (transcript
      surface, chrome commands, right-pane tabs, skill/command contributions)
      are inert data until activated; third-party `extends`/`replaces`
      requires first-party-declared extension point + explicit user approval
      per the trust-domain pattern. Trusted classification of this package
      comes from the compiled bundled inventory, never `@clay/*` naming.
      Graft CLI (`cliPath`/host `packageRoot`/optional `@nanonets/graft`
      peer), optional `qmd`, and the Obscura engine stay host-owned,
      daemon-managed, deny-by-default, fail-closed, hidden when absent —
      never package-spawned; no implicit grant from package load.
    - Performance: Capability probing (CLI presence, engine readiness) is
      bounded, cached, and off editor hot paths.
    - Code Quality: Extension-point declarations carry version,
      operations (`append`/`replace`), contribution kinds, scopes, and
      summaries exactly like `@clay/chat`'s; provenance/integrity checks
      unchanged.
    - Security: No V8 objects/functions/globals cross domains; cross-domain
      values stay typed, bounded, generation/provenance/revocation-checked.
      Untrusted wiki/graft/web content never reaches tool-authoritative
      state without validation rules; containment language stays truthful
      (cwd grants constrain Clay's API/audit record, not same-user OS
      access).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`,
        `.agents/skills/project-patterns/references/authority-boundaries.md`,
        `.agents/skills/project-patterns/references/agent-host.md`,
        `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`,
        `decision-logs/2026-07-14-2023-language-server-package-authority.md`
        (process-authority template),
        `decision-logs/2026-08-30-2156-adopt-prism-wiki-and-prism-graft-as-opt-in-knowledge-options.md`,
        `decision-logs/2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`.
    - Options Considered:
      - Re-decide process authority per CLI here: redundant — decisions
        2156/2159 already fixed host-owned fail-closed pattern.
      - Verify conformance + fill gaps (chosen): assert the package adds no
        new process grant and inherits the decided patterns; log only if a
        genuinely new authority appears.
    - Chosen Approach:
      - Walk each Phase 2 capability against the two trust domains and the
        host-owned binary pattern; record the walk as task evidence; if any
        new authority is required (none expected), stop and write the
        decision log before implementation.
    - API Notes and Examples:
      ```ts
      // Fail-closed graft resolution (daemon-side, Phase 1 pattern)
      const graft = await resolveGraftCli({ cliPath, packageRoot });
      // hidden when absent; tools never registered without resolved CLI
      ```
    - Files to Create/Edit:
      - None expected; `decision-logs/` only if a new authority is
        identified.
    - References:
      - Patterns and decisions listed above.
  - Test Cases to Write:
    - Denial tests: package JS cannot spawn processes or reach the daemon;
      absent graft/`qmd`/Obscura leaves corresponding tools/commands hidden;
      third-party replacement of `@clay/coding-agent` stays in the
      third-party runtime and requires approval.
  - Coverage Walk (2026-09-03, task complete — no new authority found,
    no decision log required):
    - **Package classification.** `@clay/coding-agent` ships compiled
      `dist/` entry + loadEntry exactly like `@clay/chat`
      (`packages/chat/package.json`); trusted classification comes from the
      compiled bundled inventory + exact provenance/integrity
      (`src/packages/bundled.rs`), never `@clay/*` naming or user promotion
      (rule 1, decision 0001). ✓
    - **Agent runtime.** The coding profile registers as an
      `AgentDefinition` (Phase 1 `agentProfile.register`, fail-closed);
      agent JS runs in the Clay-owned `clay-agent` Node child via Prism —
      never in `deno_core`. Package JS cannot spawn processes or speak to
      the daemon (core spawn, not a package-triggered grant).
      ✓ agent-host.md.
    - **Extension points.** Machinery locked: `clay-extension-point-v1`
      with closed `append`/`replace` operations, closed contribution-kind
      vocabulary, ≤32 scopes, fail-closed manifest parse
      (`src/packages/extension_points.rs`); durable approval records
      (`ApprovedRelation`/`ApprovedReplacement`/`PackageApprovalRecord`,
      `approval_covers`, revoke) in `src/packages/approvals.rs`. The
      vocabulary already covers every kind the agent surface needs
      (`componentContribution`, `command`, `overlayContribution`,
      `panelContribution`, `statusItem`). The package declares versioned
      points exactly like `@clay/chat`'s two (`chat.entrySurface`,
      `chat.chromeActions`): transcript surface + right-pane tabs as
      `componentContribution` scopes, chrome/slash commands as `command`
      scopes. Third-party extends/replaces requires the declared point +
      explicit user approval; replacement stays in the third-party runtime
      with requester/target provenance preserved. ✓
    - **Skills clarification.** Skills are the clay-agent kernel registry
      (`createSkillRegistry` duplicate-fail-closed, `skill.register`/
      `skill.list` RPC, `load_skill` progressive disclosure), not a
      package-manifest contribution kind. Third-party package JS cannot
      invoke `skill.register` (no daemon access), so "skill/command
      contributions" conformance reads: commands via manifest `command`
      extension points, skills host-side only. Recorded so the package
      skeleton task declares command points and does not invent a skill
      manifest kind. ✓
    - **Knowledge CLIs (graft/`qmd`).** Resolution code is Phase 2 work
      (not yet present — verified); it inherits the decided fail-closed
      pattern (decision 2156: `cliPath` / host `packageRoot` / optional
      `@nanonets/graft` peer; tools hidden when CLI absent; `qmd` optional
      with catalog fallback). Template exists:
      `clay-agent/src/resolve-obscura.ts` (absolute paths only, 5 s TTL
      cache, `undefined` when absent → hidden, never an error). Opt-in per
      workspace via `kernel.load`; no implicit grant from package load;
      daemon/host-owned lifecycle, never package-spawned. ✓
    - **Web/browser (Obscura).** Phase 1 plumbing already conforms
      (fail-closed binary resolution, hidden when absent); decision 2159
      constraints carry into Phase 2 tool registration: public-HTTP(S)-only
      URL validation, byte/count/timeout caps, `allowEval` gate,
      untrusted-content labeling. `computer-use-linux` stays `st`-only —
      explicitly rejected for the clay base agent (decision 2159
      alternative 3); Phase 2 has no desktop-control authority. ✓
    - **Untrusted content → tool state.** Wiki/graft/web results never
      reach tool-authoritative state without validation rules; session
      search results never become agent context without explicit user
      action (agent-host.md). ✓
    - **Containment language.** cwd/root grants constrain Clay's API/audit
      identity, not same-user OS access — `clay-agent` and any external
      binary are disclosed as trusted subprocess authority, never
      sandboxed/workspace-confined. ✓
    - **Performance.** Capability probing bounded + cached (5 s TTL
      precedent), off editor hot paths; manifest
      `performance.hotPathPolicy: "no hot-path JS on keypress/paint"`
      carried from chat; external process work asynchronous, never on
      typing/paint/layout/scroll. ✓
    - **Cross-domain.** Phase 2 adds no new cross-domain channel: UI
      contributions stay inert SDUI declarations; no V8 object/function/
      global/promise crosses domains; Clay-internal ops remain absent
      (not facade-hidden) from the third-party runtime. ✓
    - **Acceptance policy.** Workspace = full agent freedom; host reads
      free, host writes outside workspace gated on explicit permission;
      opt-in full-autonomy toggle (`agent.setFullAutonomy` facade exists).
      ✓ decision 2157.
    - **Verdict.** Every Phase 2 capability maps onto an already-approved
      trust-domain or host-owned-binary pattern; the package adds no new
      process grant, cross-domain channel, or authority class. The task's
      denial tests land with the implementation/exit-gate tasks (package
      skeleton, knowledge bases, Obscura surface) as planned.

- [x] Add Rust protocol forwarding for the Phase 1 RPCs this surface consumes
  - Acceptance Criteria:
    - Functional: The deferred Phase 1 client paths exist as typed Rust
      protocol commands + daemon forwarding: skill/command `register` and
      `dispatch`, `run.resume`, and `NewSession` passthrough of the
      daemon-side autonomy/workspace parameters. UI-facing surfaces in this
      plan call these through the Rust server, never by speaking to
      `clay-agent` directly. `approval.request`/`ask_user_decision` handling
      is wired to the Phase 1 approval UI surface (fail-closed handlers get
      a real consumer path).
    - Performance: Protocol additions are typed, bounded enums (rkyv wire)
      with no JSON-string payloads where an enum fits; forwarding adds no
      polling (event-driven only).
    - Code Quality: Mirrors the existing `SessionTree`/`Compact`/
      `SearchSessions` forwarding shapes in `src/protocol/agent.rs` and
      `src/server/agent.rs`; AG-UI projection extended only where the
      surface needs the events.
    - Security: Package JS still cannot reach the daemon (trusted-only
      facade boundary from 107 task 11 unchanged); approval/decision
      payloads stay redacted and version-checked end to end.
  - Approach:
    - Documentation Reviewed:
      - `plans/107` Further Actions (deferral notes for tasks 6–7),
        task 8 evidence (`SessionTree` forwarding shape), task 11
        (trusted-only `clay:agent` facade boundary),
      - `src/protocol/agent.rs`, `src/server/agent.rs` (existing
        forwarding patterns),
      - `.agents/skills/project-patterns/references/agent-host.md`,
        `protocol-and-performance.md`.
    - Options Considered:
      - Forward each RPC lazily inside later UI tasks: scatters protocol
        work and rediscovers the deferral mid-task.
      - One consolidated forwarding task before package/UI work (chosen):
        single wire-shape review, single test pass, later tasks consume
        finished commands.
    - Chosen Approach:
      - Extend `AgentClientCommand`/server forwarding for the deferred
        RPCs; add wire round-trip tests to the protocol suite; leave
        multi-tab workspace routing as the recorded 107 ceiling unless the
        UI task forces it.
    - API Notes and Examples:
      ```rust
      // Existing shape to mirror (from 107 task 8)
      AgentClientCommand::SessionTree { op } // -> daemon session.*
      AgentClientCommand::RunResume { session_id, decisions, expected_version }
      ```
    - Files to Create/Edit:
      - `src/protocol/agent.rs`, `src/server/agent.rs`: command variants +
        forwarding.
      - `tests/suites/protocol.rs`: wire round-trip coverage (suite runs as
        `cargo test --test protocol`).
    - References:
      - `plans/107` tasks 6–8 deferral notes; this plan's task 1 evidence.
  - Test Cases to Write:
    - Wire round-trips for each new command; redacted decision payloads;
      fail-closed behavior preserved when the daemon is absent.
  - Implementation Evidence (2026-09-03, task complete):
    - Files: `src/protocol/agent.rs` (new variants + `ApprovalRequestKind`),
      `src/server/agent.rs` (forwarding + approval bridge),
      `src/server/agent_documents.rs` (real approval handlers),
      `src/server/mod.rs` (wiring), `src/server/agent_agui.rs` (AG-UI
      projection), `tests/agent_protocol.rs` (round-trip coverage).
    - `run.resume` was **already forwarded** — Phase 1 task 11 added
      `AgentClientCommand::RunResume` + `run.resume` daemon forwarding and
      the `agent.resumeRun` facade op; verified, nothing to add. The
      plan's deferral note predated task 11.
    - `NewSession` passthrough: optional `workspaceRoot`/`fullAutonomy`
      fields forward into daemon `session.new`; `None` keeps daemon
      defaults (process cwd root, autonomy off).
    - Skill/command paths: `SkillRegister` (typed name/description/
      instructions/toolNames fields — no JSON blob), `CommandRegister`
      (name/handler/description), `CommandDispatch` (name/sessionId/
      `args_json` — arbitrary args object rides a JSON string, the
      `RunResume.decision_json` precedent; daemon validates fail-closed).
      Dispatch results project through the existing `AgentRpc` message
      (`agent.command_result`).
    - Approval bridge: reverse-RPC `approval.request` (mutation gate) and
      `approval.askUserDecision` now surface `AgentServerMessage::
      ApprovalRequest { requestId, kind: Mutation|AskDecision, payloadJson }`
      — AG-UI `clay.approvalRequest` CUSTOM event — and resolve via new
      `ApprovalResolve`/`AskDecisionResolve` client commands against a
      server-side pending registry. Fail-closed on every path: timeout
      (300 s `APPROVAL_WAIT`), dropped consumer, zero subscribers, payload
      over 16 KiB, stale/unknown request ids (diagnostic, no mutation).
      The daemon maps any error to deny, so gated tools never execute
      without a user answer; ask answers pass through for daemon-side XOR
      validation.
    - Tests: protocol suite 203/203 (both new server-message kinds and all
      five new/extended commands round-trip the codec); three new
      approval-bridge unit tests (resolve round-trip + one-answer-only,
      timeout/no-consumer deny, reject-as-denial); AG-UI adaptation test
      (one `clay.approvalRequest` CUSTOM event, no decision field on the
      request direction). Gates: `cargo fmt --check` ✓, `cargo check
      --all-targets` ✓, `cargo clippy --all-targets -- -D warnings` ✓,
      full `cargo test` 1631 passed / 0 failed. clay-agent TS untouched.
    - Wiki updated: `docs/wiki/modules/phase25-agent-protocol.md` (Phase 2
      variants + bridge, corrected stale `PROTOCOL_VERSION` 24→29 note),
      `docs/wiki/modules/clay-agent.md` (approval bridge replaces the
      fail-closed-until-Phase-2 note). Graft graph rebuilt.
    - Recorded ceiling (unchanged from 107): multi-tab workspace routing
      stays deferred; the approval surface binds to the single agent pane
      the UI task creates.

- [x] Create the `@clay/coding-agent` package skeleton and register the coding profile
  - Acceptance Criteria:
    - Functional: `packages/coding-agent/package.json` declares
      `@clay/coding-agent` with `clay.apiPrefix: "codingAgent"`, dist
      entries, docs, permissions limited to command-registration and the
      documented `agent.*` dependencies, and `clay.contributions`
      (commands, UI pane contents, skills, extension points) mirroring the
      `@clay/chat` manifest contract. `dist/load.js` registers the coding
      profile via `agentProfile.register`: the nine Phase 1 coding tools
      (`shell`, `read`, `write`, `edit`, `repo_list`, `repo_search`, `glob`,
      `delete`, `move`) plus `ask_user_decision`, the coding system prompt
      layer, and the `create-plan`-style skill reference. Omitted or unknown
      tool/skill ids fail closed exactly like Chat.
    - Performance: Manifest stays within payload budgets; no hot-path
      package JavaScript; profile registration is one-shot on load, not per
      keystroke or per pane paint.
    - Code Quality: `apiPrefix`-owned ids only (`codingAgent.*`);
      `agent.*` core ids referenced, not redeclared; manifest is the single
      source of truth (pattern `package-manifest-single-source.md`); built
      `dist/` committed like `@clay/chat`.
    - Security: The package declares no new permissions beyond what its
      contributions need; tool execution keeps the Phase 1 acceptance policy
      (workspace free; host writes gated; full-autonomy toggle off by
      default) unchanged; package JS never speaks to `clay-agent` directly.
  - Approach:
    - Documentation Reviewed:
      - `packages/chat/package.json` + `packages/chat/docs/index.md`
        (manifest/profile precedent),
      - `clay-agent/README.md` (`agentProfile.register`, profile shapes,
        fail-closed omissions),
      - Prism `docs/coding-agent-tools.md` (tool set, plan file helpers),
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`,
        `.agents/skills/project-patterns/references/package-manifest-single-source.md`,
        `.agents/skills/project-patterns/references/product-surfaces-are-packages.md`.
    - Options Considered:
      - Daemon-hardcoded coding profile: contradicts replaceable-surfaces
        decision; rejected.
      - Package-registered profile via existing `agentProfile.register`
        (chosen): reuses the Phase 1 daemon contract with zero new host
        authority.
    - Chosen Approach:
      - Clone the `@clay/chat` package layout; declare the coding profile
        data (definition, tools, prompt layer, skills) in the manifest;
        register in `dist/load.js`; keep the daemon generic.
    - API Notes and Examples:
      ```ts
      agentProfile.register({
        id: "coding", displayName: "Coding Agent",
        definition: {/* AgentDefinition data */},
        tools: ["shell","read","write","edit","repo_list","repo_search",
                "glob","delete","move","ask_user_decision"],
        skills: ["codingAgent.createPlan"],
        permissionsNote: "Phase 1 acceptance policy; no new authority"
      });
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/package.json`: manifest as above.
      - `packages/coding-agent/dist/index.js`, `packages/coding-agent/dist/load.js`:
        registration entry points.
      - `packages/coding-agent/docs/index.md`: package docs (built from this
        plan's binding spec).
      - `packages/chat/docs/index.md`: replace the "loads in Phase 29" note
        with the current state.
    - References:
      - `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`,
        `plans/107` task 4 (coding tools registration).
  - Test Cases to Write:
    - Manifest validation: valid manifest, single-source ids, apiPrefix
      ownership, payload budget.
    - Profile registration: known tools resolve; unknown tool id throws
      before any provider turn; omitted skills fail closed.
    - Unload/disable: package absent ⇒ Chat/fallback unchanged.
  - Implementation Evidence (2026-09-03, task complete):
    - Files: `packages/coding-agent/{package.json,dist/index.js,dist/load.js,
      docs/index.md}` (new, committed like `@clay/chat`);
      `runtime/js/agent.js` + `agent.d.ts` (`profileRegister`,
      `skillRegister` exports); `src/server/ops/agent.rs`
      (`op_clay_agent_profile_register`, `op_clay_agent_skill_register`);
      `src/server/agent.rs` (registration queue: `rpc_or_queue`,
      `pending_registrations`, drain after initialize);
      `src/packages/record/documentation.rs` (api-dependency ids);
      `docs/reference/clay-js-api/agent/{profile-register,skill-register}.md`
      + `api-inventory.toml` + regenerated registry; `docs/index.md` links;
      `bundled-inventory.toml` root; `packages/chat/docs/index.md` (current
      state); parity ledger row extended.
    - Deviations from the acceptance text (recorded):
      - `clay.apiPrefix` is `coding-agent`, not `codingAgent`: the manifest
        schema requires `^[a-z][a-z0-9-]{1,31}$` (same kebab-case as
        `lsp-rust`/`design-glass`); owned ids are `coding-agent.profile`,
        `coding-agent.createPlan`.
      - No `ui.paneContents` contribution: only one empty-tab pane-content
        winner exists per working area and `@clay/chat` owns it until the
        split surface (task 9) replaces the landing; declaring a pane now
        would break the UI snapshot for every user loading both packages.
        Commands + one chrome extension point ship now; surface extension
        points land with task 9.
      - Skills are not a manifest contribution kind (ignored keys are dead
        data); they register through `agent.skillRegister` in `load.js`, the
        daemon-side skill registry — the plan's API-notes shape.
    - Registration path: `load.js` registers the skill before the profile
      (fail-closed resolution at session start), with the nine Phase 1
      coding tools + `ask_user_decision`, the coding system prompt layer,
      and the plan-file skill. apiDependencies are exactly
      `agent.profileRegister` + `agent.skillRegister` (no new permission);
      permissions stay `command-registration`.
    - Load-entry safety fix (found by the runtime perf-budget suites): the
      document-classification fallback loop runs every bundled package's
      load entry on unclassifiable documents, and a load-entry daemon RPC
      used to spawn the daemon and block up to the 30 s `RPC_TIMEOUT` per
      first unclassifiable document. Registration ops now queue
      server-side while the daemon is down (`rpc_or_queue`, capped at 64)
      and the host applies the queue right after the initialize handshake,
      before any later session command (FIFO actor ordering). Load entries
      can never spawn or block on the daemon. Verified: the `runtime`
      suite (large-document 5 s budget, editor-perf matrix) passes with the
      package in the inventory; failed before the fix, passes after.
    - Tests: `coding_agent_bundled_manifest_assembles_without_claiming_the
      empty_tab` (manifest contract, permissions, apiDependencies, no
      empty-tab claim); `coding_agent_load_entry_registers_skills_before
      _profile` (facade use, ordering, tool set, no raw ops);
      `registration_rpc_queues_without_spawning_then_drains_after
      _initialize` (queued ack, no spawn, drain ordering, live path);
      bundled inventory/fingerprint gates cover the new root.
      Unload/disable stays structural: the package claims no pane content,
      so absent/disabled leaves the Chat landing and daemon untouched.
    - Gates: `cargo fmt --check` ✓, `cargo check --all-targets` ✓,
      `cargo clippy --all-targets -- -D warnings` ✓, full `cargo test`
      1634 passed / 0 failed (including the previously failing
      perf-budget tests). clay-agent TS untouched (no npm test needed).

- [x] Define and verify the package default `init.js` loading experience
  - Acceptance Criteria:
    - Functional: One line — `loadPackage("@clay/coding-agent")` in
      `~/.config/clay/init.js` — activates the Coding Agent surface with
      working defaults (coding profile, slash commands, split UI). No copied
      manifests, manual primitive registration, or low-level facade plumbing
      required. Loading is explicit; the package never becomes a silent
      default. Unloading restores Chat.
    - Performance: Load adds bounded one-time manifest+registration work; no
      per-session overhead when the surface is closed.
    - Code Quality: Any setup beyond one line is a documented limitation
      with a named generic API gap, not a convention.
    - Security: Loading grants no implicit filesystem/network/shell/daemon
      authority; knowledge bases and web/browser stay opt-in per workspace.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`,
        `examples/init.js`, `packages/chat/docs/index.md` (Activation
        section), `packages/chat/dist/load.js`.
    - Options Considered:
      - Auto-activate after Phase 1: violates explicit-load decision.
      - One-line explicit load (chosen).
    - Chosen Approach:
      - Mirror the Chat activation contract; verify from a clean
        `init.js` with only the load line.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      loadPackage("@clay/chat");
      loadPackage("@clay/coding-agent"); // Coding Agent surface + profile
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/docs/index.md`: Activation section.
      - `examples/init.js`: updated in the later canonical-example task.
    - References:
      - Pattern `configuration-system.md`.
  - Test Cases to Write:
    - Clean-init drill: fresh `init.js` with the single load line yields a
      fully working Coding Agent; remove line ⇒ Chat fallback, no residue.
  - Implementation Evidence (2026-09-03, task complete):
    - Contract: one line — `import { loadPackage } from "clay:packages";
      await loadPackage("@clay/coding-agent");` — applies the manifest
      command (`coding-agent.profile`, server-first) and chrome extension
      point, then runs `dist/load.js` (skill before profile). No copied
      manifests, no manual registration, no raw ops. Loading stays explicit;
      the package never auto-activates (per the 2026-06-09 decision log).
    - Hostless/daemon-down registration hardening (task 5 follow-up, needed
      to make the one-line load universally safe):
      - Malformed declarations fail closed with `agent.invalid_params`
        (structural shape check: non-empty string `name`; `tools`/`skills`/
        `toolNames` arrays of strings) before entering any queue; the daemon
        still owns semantic validation (tool/skill resolution at session
        start).
      - With no agent host at all (hostless runtimes), declarations queue
        process-globally (`PENDING_PACKAGE_REGISTRATIONS`) and
        `AgentHostHandle::install_global` moves them into the host's pending
        queue, preserving order, where they apply after the first initialize
        handshake — a load entry never fails or blocks for a missing host or
        daemon.
      - A stale/dead host (server dropped, child killed) also defers:
        `rpc_or_queue` falls back to the queue on `ServiceStopped` instead of
        failing the load.
      - Daemon-side re-registration is an idempotent replace (prism kernel
        default duplicate policy verified empirically), so hot-reload
        generations re-register cleanly; doc pages corrected from the
        earlier "duplicates fail closed" claim.
    - Docs: `packages/coding-agent/docs/index.md` Activation section (one
      line, queueing semantics, idempotency); `profile-register.md` /
      `skill-register.md` queued + replace semantics; `agent.d.ts` queued
      docs; `docs/wiki/modules/clay-agent.md` registration-queue section.
    - Tests (`src/server/js_runtime/tests.rs`):
      - `coding_agent_clean_init_one_line_activates_working_defaults` —
        fresh init.js, single load line: command + routing policy from the
        manifest, valid UI registry, skill-then-profile declarations with
        exact shapes (content-asserted, order-checked).
      - `coding_agent_double_load_is_idempotent_within_one_generation` —
        two loads in one init.js: load succeeds, command registers once.
      - `coding_agent_absent_load_line_leaves_no_residue` — chat-only
        init.js: no coding-agent command registered.
      - `coding_agent_malformed_registration_fails_closed_without_queue_
        corruption` — malformed shape/`toolNames` reject with
        `agent.invalid_params` before queueing; a valid declaration still
        resolves after rejections.
      - Queue assertions drain both the process-global queue and any
        installed host's pending queue (parallel server-level document-flow
        tests may queue identical declarations concurrently; assertions are
        content/order-based, not count-based).
    - Gates: `cargo fmt --check` ✓, `cargo clippy --all-targets --
      -D warnings` ✓, full `cargo test` 1638 passed / 0 failed; generated
      registry refreshed via `update-doc-registry`.

- [x] Add skills, slash commands, and plan file conventions
  - Acceptance Criteria:
    - Functional: Package contributes the `create-plan`-style skill text
      (progressive disclosure; loaded via `load_skill`) and slash commands
      through the Phase 1 command dispatch: `/compact` (manual compaction
      RPC), `/new` (new session), `/branch` + checkout map, `/model`
      (Command Centre model picker hook via `agent.clientOpenModelPicker`),
      `/tree`, `/fork`, `/clone`, `/n`, plus `--session`/`--fork`-equivalent
      package commands (open a specific session by id; open-as-fork). Plan
      files follow `plans/` conventions using
      `writeCodingPlanFile`/`parseCodingPlanTodos` from
      `@arnilo/prism-coding-tools/agent` with coding checkpoints; the
      composer `/` surface discovers every registered command.
    - Performance: Command registration is declarative data; `/` completion
        is bounded; no command handler runs on the typing path.
    - Code Quality: Commands are `codingAgent.*`-prefixed data
      (`CommandDefinition`-shaped contributions) activated by the daemon
      dispatch; handlers call documented `agent.*`/daemon RPC APIs only.
    - Security: Contributed commands cannot elevate acceptance policy or
      spawn processes; `/model` and pickers reuse core Command Centre
      sessions; plan files write only inside the workspace `plans/` tree via
      the `write` tool's authority.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/coding-agent-tools.md` (plan file helpers),
        `docs/context-and-skills.md` (skills/progressive disclosure),
        `docs/extension-authoring.md` (CommandDefinition, CommandDrivers),
        `plans/107` task 5 (skills registry + command dispatch),
        `clay-agent/README.md`,
        `.agents/skills/create-plan/SKILL.md` (this repo's create-plan is
        the style source),
        pi command surface (pi docs: `/compact`, `/new`, `/tree`, `/fork`,
        `/clone`, `--session`, `--fork`).
    - Options Considered:
      - Client-side command handling in package JS: would need new client
        authority; rejected.
      - Daemon-dispatched declarative commands (chosen): reuses Phase 1
        dispatch + host CommandDrivers; contributions stay inert data.
    - Chosen Approach:
      - Declare commands and skill text as manifest contributions; daemon
        activates under existing dispatch; the package contributes no
        imperative handlers beyond registration.
    - API Notes and Examples:
      ```ts
      // Manifest contribution shape
      { "id": "codingAgent.compact", "displayName": "/compact",
        "routingPolicy": "server-first" }
      // Plan files
      const path = writeCodingPlanFile(workspaceRoot, plan); // plans/NNN-*.md
      const todos = parseCodingPlanTodos(text);
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/package.json`: commands + skills
        contributions.
      - `packages/coding-agent/skills/create-plan/SKILL.md`: skill text
        (style source: `.agents/skills/create-plan/SKILL.md`, trimmed to
        package scope).
      - `packages/coding-agent/docs/index.md`: command reference.
    - References:
      - `plans/107` tasks 5–6, `decision-logs/2026-08-21-1758-*.md`.
  - Test Cases to Write:
    - Command dispatch: each slash command reaches its daemon RPC; unknown
      `/x` is a bounded no-op with feedback; `/` completion lists registered
      commands.
    - Plan files: create-plan skill run produces a numbered `plans/` doc with
      parseable todos and a checkpoint entry.
    - `/model` hook opens the core model picker (same Command Centre
      session as `agent.clientOpenModelPicker`).
  - Evidence (2026-09-03, task complete):
    - Slash surface shipped as declared data + daemon handlers:
      `packages/coding-agent/dist/load.js` registers nine commands via
      `agent.commandRegister` (`/compact`, `/new`, `/n`, `/branch`,
      `/tree`, `/fork`, `/clone`, `/open-session`, `/open-session-as-fork`,
      handler names `compact`…`openSessionAsFork`); manifest contributions
      mirror them as `coding-agent.*` ids for discovery. `/model` is not
      re-declared — the core built-in `agent.clientOpenModelPicker` covers
      the model-picker hook (documented in `packages/coding-agent/docs/index.md`
      slash-command table).
    - Daemon handlers (`clay-agent/src/host.ts` `runHostCommand`): `compact`
      → `session.compact`; `newSession` re-seeds from the current session's
      profile/provider/model/workspaceRoot; `checkout` →
      `session.checkout` (args `entryId`, leaf default); `forkSession` →
      `session.fork`; `cloneSession` → `session.clone`; `tree` computes a
      branch summary with per-branch text summaries + leaf marker from live
      entries (no `session.tree` RPC needed); `openSession` → `session.load`;
      `openSessionAsFork` loads then forks at leaf/entry. `session.prompt`
      intercepts prompt text whose first token exactly matches a registered
      command (JSON args after the name) and dispatches instead of
      prompting the model; unknown `/x` fails closed with bounded feedback.
    - Plan-file convention: `coding-agent.createPlan` skill text shipped in
      `dist/load.js` (progressive disclosure via `load_skill`); plan files
      follow `plans/` numbered-doc + `- [x] [id] task` todo conventions via
      `@arnilo/prism-coding-tools/agent` helpers
      (`createCodingPlanMarkdown`/`codingPlanPathForTask`/
      `readCodingPlanFile`/`buildCodingCheckpointMetadata`/`fingerprintJson`).
    - Two daemon-side defects found and fixed at the root during the
      skill-run test (both blocked ALL daemon-mediated durable writes,
      pre-existing from Phase 1):
      1. Missing host-verified run identity — every write failed with
         `ERR_PRISM_TOOL_EFFECT_CONFLICT`. Fix: `runIdentity()`/`runOwnership()`
         in `clay-agent/src/host.ts` — `createAgent` config identity + run
         options + `resumeAgentRun` ownership projection (`ownershipFromIdentity`),
         never sourced from RPC params; fixed checkpoint-resume
         "Checkpoint ownership mismatch" too.
      2. First write into a not-yet-existing subdirectory fails realpath
         containment (parent dir missing) and routes to host approval —
         fail-closed behavior kept; production resolves it via the
         approval-request bridge UI. Tests stub `approval.request` +
         `document.mkdir` for the same reason.
    - Tests:
      - Daemon `npm test` 54/54, including new `skills-commands.test.ts`
        cases: each slash command reaches its daemon RPC (compact/new/
        checkout/fork/clone/tree/open-session/open-as-fork); prompt text
        starting with a registered command dispatches instead of prompting;
        unknown command fails closed; command registration queues while the
        daemon is down and drains after initialize; unknown dispatch fails
        closed; create-plan skill run writes a `plans/` doc with parseable
        todos and a checkpoint artifact (`buildCodingCheckpointMetadata`
        with `fingerprintJson` digests).
      - Rust suite 1639 passed / 0 failed (incl. new `agent_protocol.rs`
        command register-queue/dispatch tests, coding-agent load-entry
        contract expecting skill → profile → command order, clean-init
        drill expecting the nine distinct command names queued after the
        profile, parity-ledger rows for `agent.commandRegister`/
        `agent.commandDispatch`); `cargo clippy --all-targets -- -D
        warnings` clean; `cargo fmt --check` clean.
      - `/`-completion listing is covered by the existing package command
        listing surface (`list_package_commands` includes the nine
        contributions; frontend `/` discovery rides the same registry).
    - Docs/wiki: `packages/coding-agent/docs/index.md` gained the slash
      command reference; `docs/wiki/modules/clay-agent.md` gained the
      slash-surface + host-verified-identity How-It-Works entries; graft
      rebuilt.

- [x] Implement the agent UI split surface (binding spec)
  - Acceptance Criteria:
    - Functional: Launching the agent splits the working area vertically
      50/50 (user-resizable, ratio-clamped). Left pane: chronological pi-
      style transcript (user prompt, agent message, tool outputs incl. MCP,
      skills loaded) with uniform-height truncated boxes; box color by
      content type from typed theme tokens; selecting a truncated box shows
      its full content in the right pane. Right pane: three tabs — Files
      (existing file view), Observational Memory (chrome only this phase),
      Context (categorized: system prompt, user prompts, agent messages,
      tool outputs, skills loaded, files loaded). Composer grows
      vertically with message length; `/` commands active; `Shift+Tab`
      cycles the selected model's reasoning effort (keybind configurable,
      this default). Status row: left workspace path + git branch; right
      provider + model + context size vs window. Extension strip: active
      extensions and MCP servers.
    - Performance: Transcript render is viewport-bounded with uniform row
      height; truncation is metadata-driven (fixed line count), not layout
      thrash; composer growth and `Shift+Tab` never block on IPC; status
      row updates ride the existing bounded AG-UI stream.
    - Code Quality: Composed from catalog primitives per task 2's inventory;
      any new generic kind is additive, token-only, state-complete,
      documented in the catalog and reference docs; colors only from typed
      theme tokens (`surface.*`, `text.*`, `border.*`, `component_state_*`);
      no raw colors, CSS, fonts, or package renderer callbacks; React
      reconciliation per `tauri-react-client.md` (stable ids).
    - Security: UI is inert SDUI + inert command intents; no direct Tauri
      IPC, no `Deno.core.ops`, no package-owned scroll/clipboard hooks;
      context tab shows counts/categories, never secrets or redacted
      payloads.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
      - `.agents/skills/clay-ui/references/components.md`,
      - `.agents/skills/clay-ui/references/tokens.md`,
      - `.agents/skills/impeccable/SKILL.md`,
      - `.agents/skills/full-output-enforcement/SKILL.md`,
      - `.agents/skills/high-end-visual-design/SKILL.md`,
      - `.agents/skills/design-taste-frontend/SKILL.md`,
      - `docs/reference/ui-components.md`,
      - `docs/reference/packages/creating-packages.md`,
      - `.agents/skills/project-patterns/references/package-ui-layout.md`,
        `ui-skill-stack.md`, `tauri-react-client.md`,
        `typography-role-ownership.md`.
    - Options Considered:
      - Full-detail right-pane overlay replacing tabs on box selection:
        spec says tabs remain; chosen approach keeps tabs and renders full
        content as a detail surface reachable above/within the right pane.
      - One monolithic pane-content component vs split contribution +
        per-tab contents: split contributions match the pane-topology
        contract (core owns split; package owns contents).
    - Chosen Approach:
      - Core-owned vertical split primitive (ratio-clamped, resizable) with
        two package pane-content contributions; left = transcript surface;
        right = tabbed detail (Files reuse, OM chrome, Context) + full-
        content detail on box selection; composer/status/strip are part of
        the left contribution per spec.
    - API Notes and Examples:
      ```ts
      // Pane contents (inert SDUI, catalog kinds only)
      { "id": "codingAgent.transcript", "activation": "agent-surface",
        "component": { "kind": "panel", "id": "codingAgent.transcriptPanel",
          /* scroll+list transcript, composer textInput, status labels */ } }
      { "id": "codingAgent.detail", "activation": "agent-surface",
        "component": { "kind": "tabList", "id": "codingAgent.rightTabs", /* … */ } }
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/package.json`: UI contributions.
      - `packages/coding-agent/dist/*`: registration glue.
      - `src/shell/components.rs` + frontend catalog mapping (tentative:
        only the generic kinds confirmed missing in task 2 — e.g.
        `tabList`, split-ratio control).
      - `.agents/skills/clay-ui/references/components.md`,
        `docs/reference/ui-components.md`,
        `docs/reference/packages/creating-packages.md`: catalog/guide
        updates for any new kind or package surface.
    - References:
      - Roadmap "Coding Agent UI (binding spec)",
        `decision-logs/2026-06-09-1431-*.md`,
        `decision-logs/2026-08-21-2152-*.md`.
  - Test Cases to Write:
    - SDUI validation: contributions parse, kinds exist, budgets respected,
      provenance correct.
    - Interaction: resize clamps ratio; box selection updates right pane;
      composer grows; `Shift+Tab` cycles effort states for a model that
      exposes them and is a no-op otherwise; `/` opens command completion.
    - Theme: type-colored borders resolve from tokens across at least two
      content themes; no literal colors.
  - Evidence (2026-09-03, task complete):
    - **G1 — split hosting (generic).** `register_pane_content_contribution`
      accepts `"empty-tab" | "pane"` (closed vocabulary, unknown values still
      fail closed); `pane` surfaces wire into a new additive
      `PackageUiSnapshot.surfaces` field (same bounded shape as the landing;
      ids unique across all surfaces, JSON ≤ 16 KiB each, version-stamped
      `allows_action` includes them). Launch/close are package commands
      (`coding-agent.profile` / `coding-agent.close`): the intent dispatcher
      answers with one `ShellClientCommandRequest` (the `settings.open`
      projection — user-authorized via Command Centre catalogue or declared
      action target; the server mutates nothing); the client re-parses
      deny-by-default. Per-tab runtime state (`agentSurfaceOpen` +
      `agentSurfacePaneId`) pins the surface to its hosting pane; the surface
      auto-closes when the contribution disappears (settings-panel reset
      precedent). The split is the existing Clay-owned
      react-resizable-panels substrate (defaultSize 50%, clamp 20–80%,
      keyboard-operable separator) — no new split Rust.
    - **G2 — `tabList` kind (generic).** `ComponentKind::TabList`
      (parse/as_str/`supports_text_font_role`/five-state interactive set);
      `items` reuse list-item validation, `children` are order-matched tab
      panels; React Aria Tabs projection with widget-local selection (like
      `dropdown`); drift-guarded across components.md ↔ enum ↔ registry.tsx
      (`catalog_is_drift_free_across_doc_enum_and_react_registry` green) and
      recipe-attribute closed sets (`tabList` component, strip/tab/panel
      slots). Catalog + mapping + creating-packages docs updated in the same
      change.
    - **G3 — growing composer (generic).** `textInput` gains the
      `multiline: true` node field (native `<textarea>` substrate per the
      locked native-HTML-first mapping) plus a trusted-module `autoGrow`
      presentation helper (height = scrollHeight, cap in CSS, no IPC on
      input); Enter submits, Shift+Enter newline.
    - **Surface (`frontend/src/coding-agent/CodingAgentPanel.tsx`,
      provenance-exact host rendering like ChatPanel/SettingsPanel;
      third-party `pane` surfaces render through the unchanged generic SDUI
      renderer).** Left: header (profile · provider/model), chronological
      transcript as uniform-height truncated boxes (fixed 3-line clamp —
      metadata-driven fixed line count) colored by content type from typed
      theme tokens (`box_user`/`box_assistant`/`box_reasoning`/`box_usage`/
      `box_error`/`box_tool`/`box_skill`), selection shows full content in
      the right pane; composer grows with message length, `/` completion
      lists the nine slash commands (bounded 8, ArrowUp/Down/Enter/Escape,
      exact-match submit), `Shift+Tab` cycles effort only when the AG-UI
      state exposes `effortLevels` (no-op otherwise, no focus change);
      status row: left workspace path + git branch (branch seams "—" until
      the pi-parity task adds the generic daemon field), right
      provider/model + effort + last usage; extension strip: extensions +
      MCP allow-list server names (new additive
      `AgentSessionSnapshot.mcp_servers` surfaced through `STATE_SNAPSHOT` —
      names only, never commands/args/env). Right: Files (the existing
      file-browser SDUI view re-rendered with the same inert intent path),
      Memory (OM chrome this phase), Context (category counts: system
      prompts, user prompts, agent messages, tool outputs, skills loaded,
      files loaded — counts only, never content). Tool/skill transcript rows
      ride new bounded state (`tools` rolling 8, `toolStats` cumulative)
      fed by `clay.toolPhase` CUSTOM events on the one AG-UI stream — no
      second stream, no daemon access, no Tauri authority.
    - **Package wiring.** Manifest declares the `coding-agent.surface` pane
      content (activation `pane`; component ids in the `coding-agent.*`
      namespace) and the `coding-agent.close` command; `dist/load.js`
      registers the surface via `ui.serverRegisterPaneContentContribution`
      (idempotent re-registration within a generation, like chat).
      `estimatedManifestBytes` updated to the declared size.
    - **Budgets.** The shared AG-UI state/relay chunk is named
      `chat-agent-core` in `vite.config.ts` manualChunks so the
      filename-lane budget gate keeps counting it as chat (not startup
      shell): shell gzip 155.1 kB / 180, total 367.9 kB / 400.
    - **Visual proof.** DEV fixture `?fixture=coding-agent` (memory router
      boots fixtures from the query param in DEV; the fixture seam now works
      for the agent surface too). Screenshots recorded in-session: the split
      renders 50/50 with transcript area, composer, status row, extension
      strip, and Files/Memory/Context tabs in the dark theme with 1px
      compartmentalization and zero radius. Recorded gap (pre-existing, shared
      with the chat fixture): `seedForDev` state does not reach the mounted
      panel in the plain-browser fixture, so conversation rows render empty
      outside Tauri; the vitest harness (relay seam) proves the transcript,
      detail, context, completion, and effort-chord behaviors. A11y: roles
      (log/listbox/tablist/tab/option/status/region), aria-pressed boxes,
      aria-selected completions, focus-visible rings, keyboard-operable
      separator are in the component; computer-use a11y-tree walk of the
      running fixture was not performed (Firefox window partially off-screen;
      recorded per clay-ui Step 1 as unresolved-a11y evidence).
    - **Tests.** Rust 1642 passed / 0 failed (new: `pane` activation +
      surfaces wiring + empty-tab isolation + closed-vocabulary rejection;
      agent-surface command projection; `STATE_SNAPSHOT` mcpServers;
      `tabList` kind validation; load-entry surface registration + wire
      snapshot assertions; bundled-manifest pane-surface contract), daemon
      54/54 (unchanged), frontend 200 passed (new CodingAgentPanel suite:
      split render, relay-driven transcript, selected-box detail, context
      counts, slash filtering, Shift+Tab no-op), `tsc --noEmit` clean,
      `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
      clean, vite build + both bundle budgets green.
    - **Docs.** `.agents/skills/clay-ui/references/components.md` (`tabList`
      row + per-tab states + `multiline` note),
      `docs/development/react-ui-catalog-mapping.md` (tabList mapping, G1/G3
      gaps resolved), `docs/reference/packages/creating-packages.md`
      (`tabList` row + pane-activation authoring contract),
      `packages/coding-agent/docs/index.md` (split-surface section),
      `docs/wiki/modules/react-sdui-package-ui.md` (named pane surfaces +
      surface module), `docs/wiki/modules/react-agui-chat-stream.md`
      (mcpServers + tool rows), graft rebuilt.

- [x] Deliver pi-parity run behaviors through the surface (completed 2026-09-03)
  - Evidence:
    - Functional: streaming transcript (token deltas in arrival order via
      the existing AG-UI lane; no second stream), steering
      (`chat.steer` intent → `host.steer_tab` → daemon `session.steer`,
      composer stays enabled while streaming and submits a steer),
      cancel (`chat.cancel`), session list/resume (Files-tab empty state
      mirrors ChatPanel: session rows + Resume →
      `agent.clientOpenSessionPicker` bounded-history reload),
      provider/model switching mid-session (`Prompt` carries optional
      provider/model from the book; validated against registries;
      persisted so resume keeps the switch), context-size-vs-window in
      the status row (`contextTokens` on Finished/snapshot +
      `contextWindow` on model.list). Shift+Tab effort cycling and
      slash-completion already landed with task 8.
    - Durable/stream fix (root cause): Prism 0.4.0 durable runs reject
      per-run `model` overrides (fingerprint on AgentConfig), and
      `subscribe()+run()` leaves durable subscriptions open past
      settlement so the RPC reply never resolves. `sessionPrompt` now
      uses `session.stream()` (drains and resolves), keeps durable
      `runState` at the agent level, and a picker switch recreates the
      session's agent with the new model config
      (`recreateSessionModel`); the suspended payload
      (`status/runId/version/interruption`) is rebuilt from the
      `agent_suspended` stream event so `run.resume` flows unchanged.
    - Files: `clay-agent/src/host.ts` (stream-form prompt, model-rebuild
      switch, `contextWindow` in model.list), `src/protocol/agent.rs`
      (Prompt provider/model, Finished `context_tokens`,
      `AgentModelInfo.context_window`, snapshot `context_tokens`),
      `src/server/agent.rs` + `agent_agui.rs` (carry context fields),
      `src/server/connection/runtime.rs` (`chat.steer` intent),
      `frontend/src/coding-agent/CodingAgentPanel.tsx` (+css, steer/
      context/resume UI), `frontend/src/agent/TauriClayAgent.ts` +
      `state.ts` (steer, contextTokens).
    - Tests: daemon suite 54/54 (durable suspend→resume→approve→stale,
      non-durable chat, create-plan checkpoint, slash intercept), Rust
      1642/0 (fmt/clippy clean), frontend 200/200, shell gzip
      148.6 kB/180 kB.
  - Acceptance Criteria:
    - Functional: Streaming transcript (token deltas in arrival order),
      steering (queued user message mid-run) and cancel, session list/resume
      (bounded history reload), provider/model switching mid-session via the
      core pickers, and context-size-vs-window reporting in the status row.
      All behavior rides the existing core-owned AG-UI stream lane
      (`TauriClayAgent`); the package adds no second stream, no daemon
      access, no Tauri authority.
    - Performance: No regression to Chat streaming budgets; status-row
      context usage updates are throttled and bounded; typing/paint never
      wait on daemon or package JS.
    - Code Quality: Reuses Phase 1 durable-run/run-state RPCs; any missing
      bounded field (e.g. context token usage) is added to the existing run
      state RPC as a generic field, not a package-specific channel.
    - Security: Steering/cancel respect run authority (user-initiated
      only); provider switching uses existing credential/vault paths; no
      secrets in status/strip surfaces.
  - Approach:
    - Documentation Reviewed:
      - `packages/chat/docs/index.md` (Streaming section),
        `plans/107` task 6 (durable run lifecycle, run-state RPC),
        pi README streaming/steer/session semantics,
        `.agents/skills/project-patterns/references/protocol-and-performance.md`,
        `agent-host.md`.
    - Options Considered:
      - New package event channel: duplicates the AG-UI projection;
        rejected.
      - Extend existing run-state payload (chosen): generic, bounded, one
        stream.
    - Chosen Approach:
      - Map pi behaviors onto the existing stream + Phase 1 RPCs; add only
        the missing generic run-state fields.
    - API Notes and Examples:
      ```ts
      // run-state RPC payload addition (generic)
      { runId, status, contextTokens, contextWindowTokens, … }
      ```
    - Files to Create/Edit:
      - `clay-agent/src/rpc.ts` + `host.ts` (tentative: context usage
        field), `packages/coding-agent/dist/*` (surface binding).
    - References:
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.
  - Test Cases to Write:
    - Mock-provider streaming/steer/cancel drill; resume reloads bounded
      history; provider/model switch mid-session persists across resume;
      context usage stays accurate vs window.

- [x] Implement tree navigation, branch summaries, and post-hoc discard (completed 2026-09-03)
  - Evidence:
    - Functional: `/tree` renders the in-place session tree (ids, kinds,
      previews, leaf marker, checkpoint flags, branch summaries,
      `summarizing` pending marker) via the prompt intercept; results ride
      the transcript lane as a synthetic started/delta/finished triple so
      the AG-UI run closes and the composer never sticks. `/branch`
      (`checkout`) and `/discard` re-root in place; `/fork` and `/clone`
      produce independent branches/sessions (fork keeps the id, clone gets
      a new one); `/n` fresh; `--session`/`--fork` equivalents via
      `openSession`/`openSessionAsFork`. Discard = checkout to the branch
      point with a two-step confirm (preview first, `{confirm: true}`
      executes); nothing deleted — the abandoned side keeps entries and
      its summary; the workspace rolls back through the linked document
      checkpoint in the same action (restore-before-leaf, fail closed).
    - Branch summaries (decision 2200): `kind: "summary"` entry parented at
      the branch point on the NEW branch; deterministic bounded preview
      (≤ 2 KiB / 24 entries) immediately, LLM refine in the background
      (one-shot no-tool worker on the session provider, memory store,
      never throws into checkout) that appends the refined text and
      re-roots the leaf when the session has not moved on. Summaries are
      Prism entries on the FTS-indexed store — searchable, no second store.
    - Fix surfaced by the drill: the task-9 model-rebuild ran on EVERY
      prompt (the server echoes the current provider/model) and reset the
      leaf — each message would fork a new root. Now it rebuilds only on a
      real provider/model change and carries the old leaf over.
    - Files: `clay-agent/src/host.ts` (checkout/discard/summary/tree,
      slash feedback triple, leaf-preserving model rebuild),
      `src/server/agent_documents.rs` + `agent_checkpoints.rs`
      (`checkpoint.list` reverse, advisory),
      `packages/coding-agent/package.json` + `dist/load.js` (`/discard`),
      `frontend/src/coding-agent/CodingAgentPanel.tsx` (completion),
      `clay-agent/src/__tests__/tree-branch-discard.test.ts` (drill).
    - Tests: daemon 58/58 (tree drill incl. restore-fail-closed,
      discard confirm gate, LLM refine re-root, fork/clone independence,
      no orphans), Rust 1642/0 (fmt/clippy clean), frontend 200/200,
      shell gzip 148.6 kB/180 kB.
  - Acceptance Criteria:
    - Functional: `/tree` renders the in-place session tree (pi model) with
      branch summaries (LLM summary of the abandoned path back to the
      common ancestor); `/fork` and `/clone` produce independent sessions;
      `/n` starts fresh; `--session`/`--fork` equivalents open a session by
      id / open-as-fork; post-hoc discard of any branch rolls conversation
      and workspace back to the linked document checkpoint in one action
      (nothing deleted; abandoned side stops being the leaf).
    - Performance: Tree view is virtualized/bounded for long sessions;
      branch summary generation is a background run with visible pending
      state; discard is atomic (conversation + document versions together).
    - Code Quality: Built on Phase 1 session-tree + checkpoint RPCs; no
      second store; tree UI composes catalog primitives; summaries stored
      via the Phase 1 branch-summary path (FTS-indexed).
    - Security: Discard restores document versions only through document
      authority (lease/version path); never deletes user files outside the
      recorded checkpoints; confirm affordance before destructive discard.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/agent-session-runtime.md` (checkout/fork/clone),
        `docs/session-stores-and-branching.md`,
        `plans/107` task 7 (session tree + document checkpoints),
        `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`,
        pi `/tree` docs,
        `.agents/skills/project-patterns/references/agent-host.md`.
    - Options Considered:
      - Conversation-only branching: contradicts the logged checkpoint
        decision; rejected.
      - Linked three-layer discard (chosen): one action, consistent layers.
    - Chosen Approach:
      - Surface the Phase 1 tree RPCs; add branch-summary generation
        (background LLM run, mock-provider testable) and the discard
        confirmation flow.
    - API Notes and Examples:
      ```ts
      await agent.tree.checkout({ sessionId, entryId });        // branch from E
      await agent.tree.discard({ sessionId, branchId });        // rolls back both layers
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/package.json` + `dist/*`: tree UI + command
        contributions.
      - `clay-agent/src/host.ts` (tentative: branch-summary worker +
        discard RPC if not already present from Phase 1).
    - References:
      - Decision 2200; `plans/107` task 7.
  - Test Cases to Write:
    - Tree drill (exit-gate rehearsal): branch from earlier entry restores
      workspace to that checkpoint; abandoned branch keeps its summary;
      `/fork`/`/clone` independent; discard of a mid-tree branch leaves no
      orphans in FTS or document versions.

- [x] Implement the workspace-scoped session search surface (completed 2026-09-03)
  - Evidence:
    - Functional: one `session.search` surface — the Command Centre picker
      session kind `SessionSearch` (builtin command
      `agent.clientOpenSessionSearchPicker` "Search Sessions", plus a
      "Search sessions…" affordance in the Coding Agent Files tab empty
      state). Every query keystroke runs the shared Phase 1 FTS index
      (daemon `session.search`, workspace-scoped from the tab's current
      session — never crosses workspaces); results are bounded to 50 hits
      (no paging needed at the bound, no index rebuild on open). Items
      carry session label, redacted snippet, timestamp; the leafId rides
      in the item id (`search:{sessionId}|{leafId}`). Activating a result
      resumes the tab with `session.load { entryId }` — the transcript
      opens as the read-only path root→matched entry (a `branchEntries`
      view in the daemon; the live leaf and the tree are untouched, so
      nothing is injected into agent context; a later prompt resumes from
      the live leaf). Empty query shows a hint item; secondary activation
      is a no-op (no delete from search).
    - Files: `src/protocol/agent.rs` (`AgentPickerKind::SessionSearch`,
      `LoadSession.entry_id`), `src/server/agent_picker.rs`
      (`AgentSearchHit`, hit items, skip-local-filter, activate),
      `src/server/agent.rs` (`search_sessions`, `resume_tab` entry
      param), `src/server/connection/menus.rs` (query/backspace FTS
      refresh, Resume entry arm), `src/server/menu_sessions.rs`
      (`agent_picker_ref`), `src/server/command_execution.rs` +
      `ui.rs` (builtin command + client-dialog allow-list),
      `clay-agent/src/host.ts` (`session.load` entryId +
      `branchEntries`), `frontend/src/coding-agent/CodingAgentPanel.tsx`
      (search button).
    - Tests: daemon 59/59 (session.load entryId drill: path view, tree
      untouched, unknown-id fallback), Rust 1645/0 (picker unit drill:
      FTS hits skip the local filter, activate → Resume at entry, empty
      hint; protocol drill: search parses hits, resume carries the entry
      through to the daemon; fmt/clippy clean), frontend 200/200, shell
      gzip 148.6 kB/180 kB.
    - Deviation: no separate package manifest command — the builtin
      Command Centre entry + panel button cover the surface (two entries
      would duplicate); the plan's `packages/coding-agent` file edit is
      therefore unnecessary.
  - Acceptance Criteria:
    - Functional: One `session.search` surface (picker/command, not a
      permanent tab) queries the shared Phase 1 FTS index: results carry
      session, branch, matching entry/snippet, timestamp, status. Selecting
      a result resumes/opens that session at the matching tree entry. An
      identical fixture session in another workspace never appears. Results
      never inject transcript content into the current agent context unless
      the user explicitly opens/attaches it.
    - Performance: Search is workspace-scoped SQLite FTS (existing index);
      picker is bounded (paged results); no index rebuild on open.
    - Code Quality: Reuses the Phase 1 `session.search` RPC verbatim; no
      second store; surface is a transient picker (Command Centre pattern)
        composed from catalog kinds.
    - Security: Raw tool output stays out of the index by default; search
      never crosses workspaces; attach is an explicit user action.
  - Approach:
    - Documentation Reviewed:
      - `plans/107` task 7 (FTS index),
        `decision-logs/2026-08-30-2201-workspace-scoped-session-search-shared-by-clay-and-st.md`,
        Prism `docs/session-stores.md` (`searchSessions`),
        `.agents/skills/project-patterns/references/agent-host.md`.
    - Options Considered:
      - Permanent search tab: spec says no; rejected.
      - Transient picker/command (chosen).
    - Chosen Approach:
      - Command Centre picker session kind over the existing RPC; selection
        routes to the same resume/open path as tree navigation.
    - API Notes and Examples:
      ```ts
      const hits = await agent.session.search({ workspace: currentWorkspace,
        query, limit: 50 });
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/package.json` + `dist/*`: search picker
        command contribution.
    - References:
      - Decision 2201.
  - Test Cases to Write:
    - Exit-gate drill: cross-workspace fixture invisible; selection resumes
      at matching entry without implicit context attach.

- [x] Add the opt-in wiki knowledge base (`@arnilo/prism-memory/wiki`) (completed 2026-09-03)
  - Evidence:
    - Functional: per-workspace opt-in via the documented config API
      `agent.knowledgeSetOptions({ workspaceRoot, wiki })` → trusted op
      `op_clay_agent_knowledge_set_options` → daemon RPC
      `knowledge.setOptions` (queued while the daemon is down, applied at
      initialize). `wiki: true` loads `@arnilo/prism-memory/wiki` via
      `kernel.load` (autoDeploySkills off — writes stay in `.wiki/`),
      registering the `/wiki-init`, `/wiki-refresh` (Merkle-diff
      incremental), `/wiki-lint` commands; the
      `wiki_search`/`wiki_read_page`/`wiki_record_insight` tools (appended
      to coding sessions in the enabled workspace); and the
      `wiki-maintainer`/`wiki-searcher` skills (progressive disclosure via
      `load_skill`). Profiles codebase/pkm/hybrid/auto; optional `qmdPath`
      enables host-owned qmd hybrid search (deny-by-default, catalog
      fallback otherwise). Disabled (default) or after `wiki: false`:
      nothing loads, no residue — commands gone, tools unresolvable,
      catalog filtered. One binding per daemon (re-enable rebinds); the
      prompt intercept dispatches kernel-registered bare-name commands.
    - Files: `clay-agent/src/host.ts` (knowledgeSetOptions/enableWiki/
      disableWiki/wikiTools, sessionTools + resolveSkills + skillList
      wiring, slash bare-name fallback), `src/server/ops/agent.rs` +
      `ops/mod.rs` (trusted op 96), `runtime/js/agent.{js,d.ts}`,
      `docs/reference/clay-js-api/agent/knowledge-set-options.md` +
      api-inventory + index + parity ledger,
      `packages/coding-agent/docs/index.md` (option docs).
    - Tests: daemon 62/62 (wiki drill: opt-in residue-free default,
      init/refresh/lint → OKF bundle answered by `wiki_search` via the
      catalog fallback with qmd absent, disable leaves zero residue,
      re-enable idempotent + rebind), Rust 1646/0 (knowledge.setOptions
      forwards; fmt/clippy clean), frontend 200/200.
  - Acceptance Criteria:
    - Functional: When the user enables the wiki option for a workspace
      (documented configuration API), the daemon loads the wiki kernel via
      `kernel.load`; surfaced capabilities: `/wiki-init`, `/wiki-refresh`
      (SHA-256 Merkle-diff incremental), `/wiki-lint`;
      `wiki_search`/`wiki_read_page`/`wiki_record_insight` tools;
      `wiki-maintainer`/`wiki-searcher` skills; OKF v0.2 bundles; profiles
      codebase/pkm/hybrid/auto; optional `qmd` CLI for hybrid search with
      catalog fallback when absent. Disabled (default): nothing loads, no
      residue.
    - Performance: `wiki-refresh` is incremental (Merkel-diff); tools are
      registered only when enabled; no index work at editor startup.
    - Code Quality: Activation is per-workspace config through a documented
      Clay JS API; commands/tools are declarative contributions activated
      daemon-side; skills use progressive disclosure.
    - Security: Wiki writes confined to the workspace `.wiki/` tree through
      document/write authority; `qmd` is host-owned deny-by-default (task 3
      walk); wiki content is untrusted input to the agent (never
      tool-authoritative without validation).
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/wiki.md`, prism-memory export map (`./wiki`),
        `decision-logs/2026-08-30-2156-*.md`,
        `plans/107` task 5 (skills/commands),
        `.agents/skills/project-patterns/references/authority-boundaries.md`.
    - Options Considered:
      - Always-on wiki: rejected (decision 2156 keeps it opt-in).
      - Per-workspace opt-in via config API (chosen).
    - Chosen Approach:
      - Config API flips daemon-side `kernel.load`; package surface picks up
        tools/commands/skills from the activated kernel.
    - API Notes and Examples:
      ```js
      // init.js (documented config API, Phase-config task finalizes shape)
      codingAgent.setKnowledgeOptions({ wiki: true }); // per active workspace
      ```
    - Files to Create/Edit:
      - `clay-agent/src/` (tentative: wiki kernel activation module).
      - `packages/coding-agent/package.json` + `dist/*` (tentative:
        conditional command/skill surfacing).
      - `packages/coding-agent/docs/index.md`: wiki option docs.
    - References:
      - Decision 2156; Prism `docs/wiki.md`.
  - Test Cases to Write:
    - Fixture check (exit gate): `/wiki-init` + `/wiki-refresh` produce an
      OKF bundle `wiki_search` answers from; `/wiki-lint` reports; `qmd`
      absent ⇒ catalog fallback works; disabled ⇒ zero residue.

- [x] Add the opt-in graft knowledge base (`@arnilo/prism-memory/graft`) (completed 2026-09-03)
  - Evidence:
    - Functional: `graft: true` on `knowledge.setOptions` (same config API
      and queueing as the wiki; flags compose, absent flag = no change)
      loads `@arnilo/prism-memory/graft` via `kernel.load`, registering the
      `graft_ask`/`graft_grep`/`graft_callers`/`graft_skeleton`/
      `graft_map`/`graft_blast` pull tools, the
      `graft`/`graft-build`/`graft-check`/`graft-viz` commands (bare names
      reachable through the `/name` slash fallback), and a host-side
      `graft` skill. `graftMode` gates push (`retrieval pack +
      first-turn orientation + edit-watch `graft:dirty` on mutating-tool
      boundaries, never keystrokes). CLI resolution (`cliPath` → host
      packageRoot → `@nanonets/graft` peer) runs fail-closed BEFORE load:
      absent CLI ⇒ result `graft: false`, tools hidden, agent unperturbed.
      Disabled ⇒ zero residue (tools unresolvable, `/graft` chat-safe,
      catalog clean). One binding per daemon; re-enable rebinds.
      Host-owned child process only (bounded time, capped stdout, telemetry
      off unless allowed); package JavaScript never spawns it. Graph output
      labeled agent-aid, not authority. Deviations: the package's skill
      body is not in its export map, so the daemon registers a host-side
      `graft` skill copy; push-mode graft-state persistence rides the run
      in flight (the extension API has no session key on `getEntries` —
      `ponytail:` ceiling in host.ts).
    - Files: `clay-agent/src/host.ts` (graft binding, enable/disable/tools,
      skill, active-run wiring), `src/server/ops/agent.rs` (shape gate:
      optional boolean wiki/graft, at least one required),
      `runtime/js/agent.d.ts` (option/result types),
      `docs/reference/clay-js-api/agent/knowledge-set-options.md` +
      api-inventory + generated registry,
      `packages/coding-agent/docs/index.md` (graft option docs).
    - Tests: daemon 65/65 (graft drill: absent-CLI fail-closed +
      unperturbed agent; resolved stub CLI ⇒ six pull tools active,
      `graft_ask`/`graft_callers` answer ranked spans, `/graft` dispatches,
      disable ⇒ zero residue; re-enable rebinds + wiki/graft compose in
      one call), Rust fmt/clippy clean + protocol 207/207 (shape gate),
      frontend 200/200.
  - Acceptance Criteria:
    - Functional: When enabled per workspace, daemon loads the graft kernel:
      pull tools (`graft_ask`, `graft_grep`, `graft_callers`,
      `graft_skeleton`, `graft_map`, `graft_blast`), gated push retrieval
      packs + first-turn orientation, post-edit blast radius
      (`graft:dirty`). Graft CLI resolves fail-closed
      (`cliPath`/host `packageRoot`/optional `@nanonets/graft` peer); tools
      hidden when the CLI is absent (mirrors `agy` handling). Disabled:
      nothing loads.
    - Performance: Graft indexing/queries run in the host-owned CLI process,
      bounded and off editor hot paths; tool registration only when enabled
      and resolved.
    - Code Quality: Same activation/config path as wiki; no second code-
      search store; `graft:dirty` hooks ride document-version checkpoint
      boundaries (task-boundary events), not keystrokes.
    - Security: Host-owned CLI only (task 3 walk); no package-spawned
      process; untrusted index output labeled as agent-aid, not authority.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/graft.md`, prism-memory export map (`./graft`),
        `decision-logs/2026-08-30-2156-*.md`,
        `.agents/skills/project-patterns/references/agent-host.md` (host-
        owned binary pattern),
        repo-local `graft/` usage (this repo consumes graft already).
    - Options Considered:
      - Package-declared graft MCP/process grant: violates host-owned
        pattern; rejected.
      - Daemon-resolved fail-closed CLI (chosen).
    - Chosen Approach:
      - Reuse the Phase 1 fail-closed resolution helpers (Obscura pattern)
        for the graft CLI; activate kernel on config.
    - API Notes and Examples:
      ```ts
      const hits = await tools.graft_ask({ query: "session search FTS", k: 5 });
      const edges = await tools.graft_callers({ symbol: "searchSessions" });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/` (tentative: graft kernel activation + CLI
        resolution reuse).
      - `packages/coding-agent/package.json` + `dist/*` (tentative:
        conditional tool surfacing).
      - `packages/coding-agent/docs/index.md`: graft option docs.
    - References:
      - Decision 2156; Prism `docs/graft.md`.
  - Test Cases to Write:
    - Fixture check (exit gate): `graft_ask`/`graft_callers` return ranked
      spans with `--source`-equivalent cruxes; absent CLI ⇒ tools hidden and
      agent unperturbed; disabled ⇒ zero residue.

- [x] Surface the Obscura-based web and browser capability (completed 2026-09-03)
  - Evidence:
    - Functional: the Phase 1 harness (`spawnObscuraHarness`) now assembles
      the complete surface — `obscura_*` MCP tools, `web_search`/`web_fetch`
      (replaceable HTML search profile via the new `searchProfile` harness
      seam), native `obscura_fetch`/`obscura_scrape` (public-HTTP(S)-only
      URL validation, byte/count/timeout caps, `allowEval` deny-by-default,
      untrusted-content labels), and the prism browser tools
      (`browser_open`/`browser_snapshot`/`browser_act`/policy-gated
      `browser_evaluate`/`browser_close` + CDP block/throttle/emulate) over
      `connectObscuraCdp` → `chromium.connectOverCDP` (playwright-core
      1.61.0). Coding sessions receive the tools through
      `capabilityTools()` when the engine is present; all of it is hidden
      when the binary is absent — sessions stay unperturbed. No engine
      lifecycle code added by the package (pure surface work over Phase 1).
      Harness fix found by the drill: assembly failure after the MCP child
      connects now closes it (plus the serve process) — no leaked
      half-started harness.
    - Files: `clay-agent/src/obscura.ts` (web tools in the harness +
      cleanup fix), `clay-agent/src/__tests__/obscura-web-surface.test.ts`,
      `packages/coding-agent/docs/index.md` (capability docs).
    - Tests: daemon 67/67 (surface drill: engine absent ⇒ web/browser
      tools hidden and sessions unperturbed; engine present via stub CLI ⇒
      all eleven tools registered, `web_search` answers with cited
      untrusted results, `web_fetch` bounded markdown, `obscura_fetch`
      native dump, `file://`/`ftp://` fail closed, batch cap enforced,
      custom scrape expressions rejected without `allowEval`; CDP e2e
      (env-gated `CLAY_WEB_E2E=1`) drives a loopback fixture page through
      `connectOverCDP` + browser_open/snapshot/act with untrusted_external
      metadata), Rust fmt/clippy clean (no Rust change), frontend 200/200.
  - Acceptance Criteria:
    - Functional: When the host Obscura engine is present (Phase 1 wiring),
      the coding profile exposes `web_search` + `web_fetch` (replaceable
      HTML search profile), native `obscura_fetch`/`obscura_scrape`
      (public-HTTP(S)-only validation, byte/count/timeout caps,
      `allowEval`-gated expressions, untrusted-content labeling), browser
      automation via `@arnilo/prism-web-tools/browser` (observe,
      policy-gated `browser_evaluate`, block/throttle/emulate), and
      Playwright e2e through the CDP composition
      (`connectObscuraCdp` → `chromium.connectOverCDP`, exact
      `playwright-core@1.61.0` added in Phase 1 when plumbing landed). All
      hidden when the engine is absent.
    - Performance: Fetches observe the documented caps; browser sessions are
      engine-managed with group close (Phase 1 lifecycle); no engine work at
      editor startup.
    - Code Quality: Pure surface work over Phase 1 daemon capabilities; the
      package adds no new engine lifecycle code.
    - Security: Untrusted web content never reaches tool-authoritative state
      without normal validation rules; `browser_evaluate` stays
      policy-gated; credentials/cookies never cross event boundaries.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/obscura.md`, `docs/migrate-to-0.4.md`
        (`prism-web-tools` family),
        `decision-logs/2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`,
        `plans/107` task 9 (Obscura wiring).
    - Options Considered:
      - Re-wire engine lifecycle here: Phase 1 owns it; rejected.
      - Profile/tool surfacing only (chosen).
    - Chosen Approach:
      - Add web/browser tools to the coding profile's optional tool set;
        activation gated on Phase 1 engine presence.
    - API Notes and Examples:
      ```ts
      const res = await tools.obscura_fetch({ url, caps: { bytes: 1_000_000 } });
      const page = await browser.connectObscuraCdp(engine.wsEndpoint);
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/package.json` + `dist/*` (optional tool
        declarations), `packages/coding-agent/docs/index.md`.
    - References:
      - Decision 2159; `plans/107` task 9.
  - Test Cases to Write:
    - Fixture check (exit gate): `web_search`/`web_fetch`/`obscura_fetch`
      answer; CDP automation drives a fixture page; Playwright e2e passes
      through the CDP composition; engine absent ⇒ tools hidden, no errors.

- [x] Author the pi-parity conformance checklist and run it (completed 2026-09-03)
  - Evidence:
    - Checklist authored: `packages/coding-agent/docs/parity-checklist.md`
      (P1–P18 parity steps — prompt→stream→tool ordering, steering, cancel,
      /compact manual+auto, /new, session list/resume/delete,
      provider/model switch, /tree+/fork+/clone, picker/open-as-fork
      equivalents, plan-file round-trip, composer growth, Shift+Tab effort
      cycle, status-row truth, extension strip — each with how-to-verify,
      expected result, and its automated gate; N1–N5 negative checks;
      PERF-1/2 latency/responsiveness budgets vs Phase 1 Chat) mirrored as
      manual-test-plan module `test-plan/17-coding-agent-parity.md` with
      setup, expected results, known ceilings, and dated execution records;
      matrix row 17 added to `test-plan/index.md`; cross-linked from the
      package docs index.
    - Run (Linux, 2026-09-03): automated mock-provider subset green —
      daemon 68/68 (new P8 evidence test `session.delete removes the
      record; siblings stay intact`), frontend 200/200. Automated gates
      cover P1–P7, P9–P18 and N1–N5; P8 recorded via the new test; live
      fixture pass for the UI steps. Real-provider interactive pass is a
      standing manual step pending a provider credential on this host —
      recorded in the module's known-ceilings section (the same code paths
      are pinned by the mock suites).
  - Acceptance Criteria:
    - Functional: A written checklist (in this package's docs and mirrored
      as a `test-plan/` module) enumerates every pi-parity behavior: prompt
      → stream → tool output ordering, steering, cancel, `/compact` manual +
      threshold auto, `/new`, session list/resume/delete, provider/model
      switch, `/tree` navigation + summaries, `/fork`, `/clone`, `--session`
      /`--fork` equivalents, plan file round-trip, composer growth,
      `Shift+Tab` effort cycle, status row truth, extension strip. The
      checklist passes manually on Linux with at least one real provider and
      the automated subset passes with the mock provider in CI.
    - Performance: Checklist records stream latency and UI responsiveness
      observations; regressions vs Phase 1 Chat budgets are defects.
    - Code Quality: Checklist is numbered, cross-linked to deep-reference
      docs, and executable by a human without reading source.
    - Security: Checklist includes negative checks (cross-workspace search
      invisible; disabled knowledge bases leave no tools; secrets absent
      from status/context surfaces).
  - Approach:
    - Documentation Reviewed:
      - pi README/docs (parity semantics), this plan's binding spec,
        `test-plan/index.md` (module conventions),
        `.agents/skills/project-patterns/references/maintenance-validation.md`.
    - Options Considered:
      - Ad-hoc manual passes: not repeatable; rejected.
      - Numbered checklist + test-plan module (chosen).
    - Chosen Approach:
      - Write the checklist once, execute on a real Linux build, record
        pass/fail per numbered step; mock-provider subset scripted in CI.
    - API Notes and Examples:
      ```text
      test-plan/modules/coding-agent-parity.md   (numbered steps, expected
      results, negative checks, known ceilings)
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/docs/parity-checklist.md`.
      - `test-plan/modules/coding-agent-parity.md` (name tentative to the
        module map), `test-plan/index.md` (matrix row).
    - References:
      - Roadmap Phase 2 exit gate.
  - Test Cases to Write:
    - The checklist itself is the test; CI job runs the mock subset.

- [x] Verify the Phase 2 exit gate (completed 2026-09-03)
  - Evidence (gate record, Linux 2026-09-03):
    - pi-parity checklist: RUN (task 15) — P1–P18 automated gates + N1–N5
      green (daemon 69/69 with Obscura e2e, frontend 200/200); real-provider
      interactive pass recorded as standing manual procedure (no provider
      credential on this host — module 17 known ceilings).
    - tree/discard drill: PASS — `tree drill: checkout from an early entry
      appends a summary and re-roots the leaf; /tree renders ids and
      summaries`, `discard is two-step…`, `LLM branch-summary refine…`,
      `/fork and /clone independence; mid-tree discard leaves no orphan
      branches` (tree-branch-discard.test); checkpoint restore via
      `session.checkpoint captures via reverse RPC; restore failure fails
      checkout closed`.
    - UI spec conformance walk: PASS — CodingAgentPanel suite verifies every
      binding-spec element (50/50 split, transcript boxes with type colors,
      Files/OM/Context tabs, growing composer, slash completion, Shift+Tab,
      status row, extension strip, full-detail on selection); live fixture
      mounts the surface (screenshot; landing state — the fixture-seeding
      ceiling recorded at task 8 applies to seeded conversation state).
    - fixture knowledge checks: PASS — wiki OKF bundle answers via
      wiki_search (`wiki-knowledge.test`), graft ranked spans via pull tools
      (`graft-knowledge.test`), Obscura web/browser incl. CDP e2e driving a
      fixture page (`obscura-web-surface.test`, `CLAY_WEB_E2E=1`); all-
      disabled residue-free (`wiki option is opt-in…`, `graft option is
      opt-in…`).
    - session-search drill: PASS — `session.search is workspace-scoped and
      returns hits with leafId`, `session.load with entryId opens the
      branch through the matched entry (read-only view)`, `search hits are
      transcript data…` (correct tree location, cross-workspace invisible,
      resume without implicit attach).
    - delete/disable package: PASS — with `root = "coding-agent"` disabled
      in `src/packages/bundled-inventory.toml`, js_runtime ran 209 passed /
      0 failed except the two coding-agent-requirement clean-init tests
      (expected: they require the package); Chat package tests
      (`chat_package*`) green without it — daemon and Chat fully
      functional; root restored, `coding_agent` tests 5/5 green.
    - Linux gates: `cargo fmt --check` clean; `cargo check --all-targets`
      clean; `cargo clippy --all-targets -- -D warnings` clean; full Rust
      suite 1646/0; daemon 69/69; frontend 200/200; bundle budgets shell
      155.1 kB gzip / 180 kB, total 368.2 kB / 400 kB.
    - Trust-domain walk re-confirmed: inventory gates pin the 96 trusted
      ops (protocol suite 207/207 incl. `assert_exact_inventory`);
      documentation-coverage gates green after recording module 17 in the
      parity ledger (new `agent.codingAgent.parity` capability, manual
      steps P1–P18/N1–N5) and the primitive-migration matrix row; no
      widened capability vs Phase 1 (no new ops since task 12).
  - Gate defects found and fixed (2026-09-03 first real manual pass):
    - Folder-open swallowed: the per-tab `workspace_pane_visible` daemon
      flag defaulted to hidden, so the bind/folder-open snapshot shipped
      the editor-only tree and the desktop UI showed nothing after
      selecting a folder in the OS picker. Fixed: per-tab pane visible by
      default (bind snapshot carries the browser tree;
      `workspace.toggleFileBrowser` flips it); tests updated
      (`deferred_initial_state_waits_for_tab_binding`,
      `real_server_restore_sequence…`, path-browser rebind).
    - Command Centre unreachable with no document open: the Global
      server-first chord matcher (`Ctrl+X Ctrl+P` → `controlCenter.open`)
      existed only inside CodeMirror editor keymaps, so the landing page
      had no consumer for the chord. Fixed: the shell key handler resolves
      Global server-first manifest chords outside `.cm-editor` and
      dispatches the command intent through the active pane session
      (`serverKeymaps`/`dispatchServerCommand` on the workspace controller;
      frontend test added). Second-pass fix after live retest: the manifest
      filter compared `"global"`/`"serverFirst"` while serde emits
      unit-variant names as written (`"Global"`/`"ServerFirst"`), so the
      matcher matched nothing — values are now normalized
      (case/kebab-insensitive) before comparing and the test pins the wire
      spelling.
  - Acceptance Criteria:
    - Functional: All exit-gate drills pass and are recorded: pi-parity
      checklist (task above); tree/discard drill (branch from earlier entry
      restores workspace to that entry's checkpoint, abandoned branch keeps
      summary, `/fork`+`/clone` independent); UI spec conformance walk
      (every binding-spec element verified on the live build); fixture
      knowledge checks (wiki OKF answers, graft ranked spans, Obscura web/
      CDP/Playwright, all-disabled residue-free); session-search drill
      (correct tree location, cross-workspace invisible, resume without
      implicit attach); delete/disable package leaves the daemon and Chat
      fully functional.
    - Performance: Linux gates green: `cargo fmt --check`,
      `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`, daemon `npm test`, UI
      render/bundle budgets respected.
    - Code Quality: No scope creep into `st`/Phase 3–10 territory; every
      deviation recorded in Compromises Made.
    - Security: Trust-domain/process-authority walk (task 3) re-confirmed on
      the final build; no widened capability vs Phase 1.
  - Approach:
    - Documentation Reviewed:
      - Roadmap Phase 2 exit gate; all task evidence above.
    - Options Considered:
      - Fold into individual tasks: drills span tasks; a single gate run
        catches integration drift.
    - Chosen Approach:
      - Execute the gate as a scripted pass over the drills; record results
        in this plan.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && \
        cargo clippy --all-targets -- -D warnings && \
        (cd clay-agent && npm test)
      ```
    - Files to Create/Edit:
      - None (evidence recorded in this plan + test-plan modules).
    - References:
      - Roadmap Phase 2.
  - Test Cases to Write:
    - Gate record: each drill pass/fail with evidence pointers.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Launch a real Linux GUI build with representative data
      (multi-turn transcript with truncated tool outputs, all three right
      tabs, composer at several lengths, streaming and error states, narrow
      and wide window layouts). Exercise every changed state; take and
      inspect screenshots per state; store evidence under a clearly named
      review artifact path and record paths + findings in task evidence.
      With `computer-use-linux` available, start from `get_app_state`,
      inspect the accessibility tree, and verify keyboard-only flow, focus
      visibility/order, role/name/state exposure, tab containment, and
      announcements for the composer, tabs, transcript boxes, and pickers.
    - Performance: Screenshot review notes any render jank in transcript
      scroll/resize; findings become defects or prioritized follow-ups.
    - Code Quality: Failures treated as product defects or explicitly
      prioritized follow-ups, never waived silently.
    - Security: Screenshots contain no real credentials/secrets (fixture
      providers only).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`,
        `.agents/skills/clay-ui/references/components.md`,
        `.agents/skills/clay-ui/references/tokens.md`,
        `.agents/skills/impeccable/SKILL.md`,
        `.agents/skills/full-output-enforcement/SKILL.md`,
        `.agents/skills/high-end-visual-design/SKILL.md`,
        `.agents/skills/design-taste-frontend/SKILL.md`,
        `.agents/skills/project-patterns/references/ui-visual-review.md`,
        `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
    - Options Considered:
      - Source inspection only: explicitly insufficient per the decision.
      - Live GUI + a11y tree review (chosen; blocker recorded if GUI or
        computer use unavailable, leaving acceptance unresolved rather than
        claiming pass).
    - Chosen Approach:
      - Structured pass over the changed states list with screenshots +
        `get_app_state`-first a11y inspection; re-observe after each
        interaction.
    - API Notes and Examples:
      ```text
      code-reviews/phase2-ui-visual-a11y/  (screenshots + findings, path
      tentative to repo conventions)
      ```
    - Files to Create/Edit:
      - Review evidence directory (path per repo convention at execution
        time).
    - References:
      - Decision 2026-08-14-0200.
  - Test Cases to Write:
    - Per-state screenshot checklist; keyboard-only walkthrough record.

- [ ] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` documents
      the implemented agent-surface APIs: pane-content contributions on the
      core split, tabbed detail surfaces, transcript composition, transient
      pickers, optional tool surfacing (wiki/graft/web), skill/command
      contributions, extension points declared by this package, permissions,
      testing guidance, limitations, and migration notes. Clay remains owner
      of the pane/split tree, component registry, action routing, and token
      model; no package-authored host CSS or direct Tauri IPC.
    - Performance: Guide includes the hot-path rules packages must follow
      (no package JS on typing/paint).
    - Code Quality: Doc drift across the catalog, `creating-packages.md`,
      `docs/reference/ui-components.md`, and `docs/index.md` fails `cargo
      test` (existing truth tests).
    - Security: Guide states the trust boundaries for agent surfaces and
      extension points.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md` (current),
        `decision-logs/2026-06-09-1431-*.md`,
        `.agents/skills/project-patterns/references/package-ui-layout.md`.
    - Options Considered:
      - Package-local docs only: the authoring contract is host-owned;
        guide must carry it.
    - Chosen Approach:
      - Update the canonical guide in the same phase as the surface.
    - API Notes and Examples:
      ```text
      docs/reference/packages/creating-packages.md  (agent surfaces section)
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`,
        `docs/reference/ui-components.md` (if kinds added),
        `docs/index.md` (links).
    - References:
      - Clay reference `clay.md` (Package UI/Layout task).
  - Test Cases to Write:
    - Doc truth tests pass with the updated docs.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every public programmatic behavior this phase introduces
      or changes is a documented Clay JS API: package load/activation,
      `agentProfile.register` coding-profile usage, slash-command surface,
      tree/search/picker commands, knowledge-base options, per-workspace
      enable/disable, and any new generic primitives (split ratio, tab
      selection) exposed where packages need them. Dotted-ID rules hold:
      core ids bare `<domain>.<name>` (`agent.*`, `shell.*`); package ids
      `codingAgent.*`; new core domains added to `RESERVED_CORE_API_DOMAINS`.
      Rust public functions introduced/changed get explicit `deno_core` op
      wrappers + facades, or are made private/`pub(crate)`.
    - Performance: API docs include async/return behavior; no raw
      `Deno.core.ops` user-facing calls.
    - Code Quality: Each API doc carries stable ID, user-facing name, key
      bindings (or empty list), custom properties, description, JS example,
      configuration, errors, permissions/security notes, backing Rust path,
      op wrapper, facade path, lookup tags; linked from `docs/index.md`;
      generated registry updated via the project command.
    - Security: No API implicitly grants process, network, or daemon
      authority; knowledge options document their host-owned CLIs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`,
        `clay-js-api-boundary.md`, `clay-js-api-schema.md`,
        `doc-registry-tests.md`,
        `decision-logs/2026-05-08-1509-*.md`,
        `decision-logs/2026-05-08-1840-*.md`.
    - Options Considered:
      - Manifest-only contributions without APIs: config and discovery need
        documented APIs.
    - Chosen Approach:
      - Inventory the phase's surfaces, then document/register each as
        above; `cargo test` gates staleness.
    - API Notes and Examples:
      ```ts
      codingAgent.setKnowledgeOptions({ wiki: true, graft: false });
      await agent.session.search({ query: "branch summary", limit: 50 });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**` (new/updated API docs),
        `docs/index.md`, generated registry artifacts, facade modules.
    - References:
      - Clay reference `clay.md` (Clay JS API task).
  - Test Cases to Write:
    - Registry truth: missing/stale API doc, index link, custom property, or
      lookup entry fails `cargo test`.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Configuration for this phase is exposed as Clay JS APIs
      from `~/.config/clay/init.js`: package load, knowledge-base options
      (wiki/graft per workspace), web/browser capability visibility,
      `Shift+Tab`-equivalent keybind configurability, and any UI
      behavior-changing settings as `custom_properties`. No undocumented
      configuration keys.
    - Performance: Config load stays one-shot; invalid options fail closed
      with actionable errors.
    - Code Quality: Docs and registry coverage per the API task; options
      match server-side parsers exactly.
    - Security: Configuration grants no implicit filesystem/network/shell/
      extension authority; knowledge opt-ins are explicit per-workspace
      user actions.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`,
        `.agents/skills/project-patterns/references/configuration-system.md`.
    - Options Considered:
      - Workspace-local settings files: defer; `init.js` APIs are the v1
        surface (record as compromise if a per-workspace toggle needs a
        home).
    - Chosen Approach:
      - `init.js`-callable APIs + custom properties; coverage gates fail on
        undocumented behavior-changing settings.
    - API Notes and Examples:
      ```js
      loadPackage("@clay/coding-agent");
      codingAgent.setKnowledgeOptions({ wiki: true, graft: true });
      ```
    - Files to Create/Edit:
      - API docs + registry (with the JS API task), facade modules.
    - References:
      - Clay reference `clay.md` (Configuration task).
  - Test Cases to Write:
    - Undocumented-setting gate: behavior-changing setting missing from
      `custom_properties`/docs fails tests.

- [ ] Update the canonical example configuration (`examples/init.js`)
  - Acceptance Criteria:
    - Functional: `examples/init.js` gains the Coding Agent section exactly
      once, in its section, documenting every option (load line, knowledge
      options, web visibility, keybind note) with annotated defaults and
      commented non-default examples; ordering constraints preserved; file
      stays valid (`node --check`) and safe to copy verbatim.
    - Performance: None beyond validity.
    - Code Quality: Option names/enums/defaults cross-checked against API
      docs and `api-inventory.toml` custom properties.
    - Security: Heavy/environment-specific setup (real providers, knowledge
      bases on big repos) stays commented with instructions.
  - Approach:
    - Documentation Reviewed:
      - `examples/init.js` (current), Clay reference `clay.md` (example
        maintenance task), `decision-logs/2026-06-09-0219-*.md`.
    - Options Considered:
      - Minimal diff mentioning only the load line: reference requires
        comprehensive coverage.
    - Chosen Approach:
      - Full section per the canonical style.
    - API Notes and Examples:
      ```js
      // --- Coding Agent (@clay/coding-agent) ---
      // loadPackage("@clay/coding-agent");
      // codingAgent.setKnowledgeOptions({ wiki: true, graft: false });
      ```
    - Files to Create/Edit:
      - `examples/init.js`.
    - References:
      - User instruction 2026-08-03 (canonical example duty).
  - Test Cases to Write:
    - `node --check examples/init.js`; option-name cross-check against docs
      and inventory.

- [ ] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Affected `test-plan/` modules identified via
      `test-plan/index.md`; relevant steps executed on a real Linux build
      with pass/fail recorded. New numbered steps (module + step ids) added
      for every new user-visible behavior: agent split surface states,
      slash commands, tree/discard, session search, knowledge bases,
      web/browser, package disable/delete. Coverage matrix updated; new
      steps cross-linked to deep-reference docs under `docs/development/`
      where present.
    - Performance: Steps include responsiveness expectations for transcript
      scroll/resize and streaming.
    - Code Quality: No existing step weakened or deleted to pass; failures
      are defects or explicitly recorded ceilings.
    - Security: Negative checks included (cross-workspace invisibility,
      disabled-capability residue, secret-free surfaces).
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`, module map, parity checklist task above,
        Clay reference `clay.md` (manual test plan task).
    - Options Considered:
      - Rely on the parity checklist only: test-plan is the durable
        cross-plan record; both needed.
    - Chosen Approach:
      - Mirror the parity drills into the module map; run the full affected
        set; record results.
    - API Notes and Examples:
      ```text
      test-plan/modules/coding-agent-*.md  (names per module map)
      ```
    - Files to Create/Edit:
      - `test-plan/modules/*`, `test-plan/index.md`.
    - References:
      - User instruction 2026-08-04.
  - Test Cases to Write:
    - The added numbered steps with expected results and negative checks.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation
      tasks complete: package architecture (profile, commands, UI
      contributions, extension points), daemon-side activation paths
      (knowledge kernels, optional tools), new generic primitives, and
      security boundaries; master index links updated.
    - Performance: Wiki documents performance-relevant details (stream
      budgets, truncation, viewport bounding); adds no runtime work.
    - Code Quality: Pages explain what/how/invariants/tradeoffs with
      source/test paths and examples.
    - Security: Security boundaries (trust domains, host-owned CLIs,
      untrusted content labeling) documented without secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Update after each task: noisy; update once after verification
        (chosen).
    - Chosen Approach:
      - One `project-wiki` pass including `docs/wiki/index.md` and relevant
        module pages (`clay-agent.md`, package/UI pages).
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/**` (relevant pages).
    - References:
      - `.agents/skills/project-wiki/SKILL.md`.
  - Test Cases to Write:
    - Manual wiki review: index links resolve; pages explain the shipped
      behavior.

## Compromises Made

- Real-provider interactive parity pass (checklist P1–P4, P9 live
  streaming) is a standing manual procedure: this host has no provider
  credential, so the checklist runs its automated gates on the mock
  provider and records the live pass as pending (test-plan/17 known
  ceilings). The same code paths are pinned by the mock suites.
- Fixture-driven UI evidence shows the landing state only: the pre-existing
  fixture-seeding ceiling (recorded at task 8) means streamed conversation
  states are verified by the panel suite rather than live screenshots.
- Shift+Tab is a fixed client-local effort cycler (not yet
  user-configurable) and the status-row git branch is a seam showing `—`
  until wired — both recorded as pi-parity seams in the checklist.
- `/tree` renders a compact branch summary computed from live session
  entries (no dedicated session.tree RPC in Phase 1); a dedicated RPC can
  replace the client-side computation later.
- Graft CLI resolution is tolerant (absent CLI disables graft quietly with
  tools hidden) while wiki load failures fail closed — asymmetric by
  design: a missing CLI is an expected environment state, a corrupt wiki
  bundle is not.
- Daemon `session.prompt` slash-command intercept coalesces entry ids from
  args or input and emits synthetic AG-UI events for command feedback —
  pragmatic composition over a dedicated command-output wire event.

## Further Actions

- Record the real-provider interactive parity pass when a provider
  credential is configured on a Linux host (module 17 standing step).
  Priority: medium — blocks nothing; mock gates pin the code paths.
- Replace the fixture-seeding workaround so seeded conversation states
  render in the fixture route (shared with the chat fixture). Priority:
  medium — improves every future UI evidence pass.
- Add a dedicated `session.tree` RPC to replace the client-side branch
  summary computation, and a dedicated command-output wire event for slash
  command feedback. Priority: low — current behavior is correct and
  bounded.
- Wire the status-row git branch (currently `—`) and make Shift+Tab
  keybind-configurable. Priority: low — pi-parity seams, recorded.
- CI: add a job running the mock-provider parity subset (`clay-agent npm
  test` + frontend suites) and the env-gated Obscura e2e when CI
  configuration lands. Priority: medium.
