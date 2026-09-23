# Phase 2.2 — Skill Discovery, Wiki/Graft Wiring, Project Prompt, MCP, Context Inspector

Source: `roadmap.md` "Phase 2.2" section (all open decisions locked 2026-09-09). This plan
turns the locked changes into executable tasks: configurable skill discovery with a third
home root, wiki/graft activation wiring, file-backed agent-delivered skills plus a user
`SYSTEM.md`, the workspace `AGENTS.md` prompt layer, the context-inspector fix, MCP stdio
wiring with two config sources and per-server fault isolation, MCP/@-mention/token-meter UI
surfaces, the agent settings page, and three user-reported fixes (git branch, effort
dropdown, `/resume`).

## Objectives

- Skill discovery roots (`<workspaceRoot>/.agents/skills`,
  `~/.config/clay/agents/coding-agent/skills/`, `~/.agents/skills`) are
  user-configurable through `~/.config/clay/agents/coding-agent/skills.json`,
  which also carries on/off toggles (`agentSkills`) for daemon-delivered skills
  (wiki-searcher, wiki-maintainer, graft). Per-agent config layout per decision
  2026-09-09-1420; the daemon data dir stays `~/.config/clay/agent`.
- Wiki activates only through `/wiki-init` (scan/align-or-create `.wiki/`, writes confined
  to `.wiki/`, wiki skills activate simultaneously); graft binds by default (fail-closed
  CLI resolution).
- Agent-delivered skill content is file-backed: seeded to and always loaded from
  `~/.config/clay/agents/coding-agent/skills/<name>/SKILL.md`; a user `SYSTEM.md` rides the system
  prompt; both editable from a new agent settings page in the UI.
- `<workspaceRoot>/AGENTS.md` is injected as the workspace project-prompt layer; the
  context inspector shows the real composed system prompt.
- MCP stdio servers connect from user `mcp.json` and repo `.mcp.json` with no approval
  gate (decision 1758 amendment), per-server fault isolation, and UI surfaces (session
  card + composer connections section).
- `@` mentions in the composer manually trigger skills and attach files/images; a token
  meter shows context occupancy vs the model ceiling with theme-token thresholds.
- Git branch always visible; effort dropdown usable from session start; `/resume` lists
  identifiable sessions and actually loads them.

## Expected Outcome

- A fresh clone + copied `examples/config/` starts the coding agent with: skills from all
  three configured roots, seeded/loaded agent skill files and `SYSTEM.md`, graft available,
  wiki available strictly after `/wiki-init`, MCP servers connected per config with healthy
  servers surviving failing ones, and a settings page that edits every agent-delivered file.
- All existing gates stay green: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test` (lib + integration targets),
  `clay-agent` tsc + vitest, frontend `tsc` + `vite build`.
- `roadmap.md` Phase 2.2 bullets each map to completed tasks here.

## Tasks

- [x] Baseline gates before any change
  - Acceptance Criteria:
    - Functional: current tree passes `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`, integration targets
      (protocol, runtime, security, presentation), `cd clay-agent && npx tsc -p tsconfig.json && npm test`,
      and frontend `vite build` with no new failures.
    - Performance: no regression expectation beyond recording current suite durations.
    - Code Quality: record baseline outputs for later comparison; note any pre-existing
      failures explicitly so they are not attributed to this plan.
    - Security: none specific.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/SKILL.md` (deterministic execution loop).
      - `.agents/skills/create-plan/references/clay.md` (platform-validation: Linux gates blocking).
    - Options Considered:
      - Skip baseline: risks attributing pre-existing failures to plan tasks. (Rejected.)
    - Chosen Approach: run every gate once, record results in the task evidence.
    - Files to Create/Edit: none.
    - References: `AGENTS.md` platform-validation block; prior session verification runs
      (cargo lib 1289 pass, clay-agent 111 pass/1 skip).
  - Test Cases to Write: none (gate run only).
  - Evidence (2026-09-09, clean tree at plan creation):
    - `cargo fmt --check`: pass.
    - `cargo check --all-targets`: pass (dev profile, 30.7s).
    - `cargo clippy --all-targets -- -D warnings`: pass (0 warnings).
    - `cargo test --lib`: 1289 passed, 0 failed, 1 ignored (14.2s).
    - Integration: protocol 209 / runtime 75 / security 152 / presentation 46 — all pass, 0 failed.
    - `clay-agent`: `npx tsc -p tsconfig.json` clean; `npm test` 0 fail, 1 skip (10.7s).
    - `frontend`: `npx vite build` success (3.0s).
    - Pre-existing failures: none observed.

- [x] Record the MCP external-process authority amendment (decision 1758)
  - Acceptance Criteria:
    - Functional: a new decision log under `decision-logs/` amends decision 1758: MCP stdio
      entries from user `~/.config/clay/agents/coding-agent/mcp.json` and repo-root `.mcp.json` connect
      **without an approval gate**; bare command names (e.g. `npx`, `uvx`) may PATH-resolve
      (Claude Code parity); server choice is the user's responsibility, Clay takes no
      responsibility for connected servers — same posture as Claude Code.
    - Performance: n/a (documentation).
    - Code Quality: decision log follows the create-decision-log skill format, references
      decision 1758 and the roadmap Phase 2.2 bullet, and states what stays fail-closed
      (literal argv, explicit env names, canonical absolute path **or** PATH-resolved bare
      name only for these two config sources — package JS still never supplies argv).
    - Security: the amendment's scope is explicit — it applies only to the two config
      files, never to package-delivered or webview-delivered process requests.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/SKILL.md` (decision-log integration: durable
        guidance extraction, never copying plan text).
      - `decision-logs/2026-07-…-1758…` (original two-domain posture; locate exact file).
    - Options Considered:
      - Fold into the roadmap only: leaves no auditable decision trail. (Rejected.)
      - Full authority framework with approval gates: explicitly overridden by the user.
    - Chosen Approach: one decision-log entry, then (per clay-execution integration rules)
      update `.agents/skills/clay-execution/references/packages.md` with the narrowed
      pattern only if it generalizes beyond MCP.
    - Files to Create/Edit:
      - `decision-logs/<date>-mcp-stdio-config-sources-no-approval-gate.md`: new entry.
    - References: roadmap Phase 2.2 MCP bullet; `clay-agent/src/mcp.ts` current validation.
  - Test Cases to Write: none (documentation).
  - Evidence (2026-09-09):
    - Created `decision-logs/2026-09-09-1341-mcp-stdio-config-sources-no-approval-gate.md`
      (full template: approval evidence quoted from the Phase 2.2 roadmap session;
      four alternatives recorded; scope guard — the exception applies only to the two
      named config files, package JS still never supplies argv).
    - Amended base decision located: `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
      (Phase 29 consequence line deferring the MCP allow-list source).
    - Updated `.agents/skills/clay-execution/references/packages.md` (host-owned binaries
      bullet): added the durable narrowed pattern — config-sourced MCP servers are the one
      exception to approval-gated process authority; PATH resolution limited to those two
      files; exception does not compose to new surfaces.

- [x] Review existing primitives and plan generic gaps before config/package work
  - Acceptance Criteria:
    - Functional: inventory exists (task evidence) of the primitives this phase rides:
      config-file loading pattern (`tool-caps.json` read at `clay-agent/src/host.ts` ~319,
      stored ~486; `default_config_root()` in `src/packages/service.rs:328-333` and
      `src/server/configuration.rs:86-95`), daemon RPC surfaces (`skill.register`,
      `knowledge.setOptions` at `host.ts:758`/`2639-2660`), STATE snapshot transport
      (`src/server/agent.rs` snapshot build; `agent_agui.rs` state emission), environment
      cache invalidation list (`agent.rs` ~1936), and the frontend snapshot merge
      (`CodingAgentPanel.tsx` `useSyncExternalStore`).
    - Performance: no new hot-path work; config reads happen at host creation / session
      build only.
    - Code Quality: the plan's new surfaces (skills.json, mcp.json, seeded files, settings
      page) are built on these primitives; any new Rust surface is generic (config-root
      file loading, not skill-specific parsing); the `@clay/coding-agent` package keeps
      loading via one `loadPackage` line — no silent defaults.
    - Security: inventory notes which surfaces are server-authoritative vs webview input
      (config paths always resolve server-side; webview never supplies paths).
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`.
      - `.agents/skills/clay-execution/references/packages.md` (trust domains, authority).
      - `.agents/skills/clay-execution/references/config.md` (config ownership).
    - Options Considered:
      - Skip (daemon-only work): this phase changes package runtime capabilities
      (coding-agent profile, daemon config) — the primitive review is mandatory. (Rejected.)
    - Chosen Approach: read-only inventory task; output captured in task evidence, gaps
      called out as sub-items of the implementation tasks below.
    - Files to Create/Edit: none.
    - References: decision-logs for primitive-first mode (2026-06-04-1923), two trust
      domains (2026-07-21-0001), init.js loading (2026-06-09-0219).
  - Test Cases to Write: none (review task).
  - Evidence (2026-09-09 inventory — all line numbers verified against the tree):
    - **Config-file loading primitive (daemon TS).** `loadToolCaps(dataDir)`
      (`clay-agent/src/host.ts` ~316-325): read at host creation; missing file = defaults;
      malformed = warn + defaults; never blocks boot. Stored `private readonly toolCaps`
      (~486-493). Gap note: tool-caps.json lives in the agent **data** dir (beside
      book.json); the locked `skills.json`/`mcp.json` live under the config root —
      replicate the loading pattern, parameterize the root.
    - **Config-root resolution (Rust).** `default_config_root()`
      (`src/packages/service.rs:328-334`, HOME/USERPROFILE → `~/.config/clay`, fallback
      `.clay-config`); Option variant `src/server/configuration.rs:86-95`. Daemon side
      already defaults the agent config root to
      `join(homedir(), ".config", "clay", "agents", "coding-agent")`
      (host.ts:565).
    - **Skill discovery + registration (daemon TS).** `ensureSkillDiscovery`
      (host.ts:1185, global once + per-workspace cache), `discoverSkills` (:1194, Prism
      `discoverContributions`, ENOENT-tolerant), `resolveSkills` (:1146, fail-closed for
      profile skills, toolName-filtered append for discovered). Triggers: sessionNew
      (:806), ensureLive (:1660). Registry: `createSkillRegistry([], {duplicate:"error"})`;
      RPC surfaces `skill.register` (:748), `agentProfile.register` (:746);
      `visibleSkills()` (:2492) bounds + wiki/graft filter, consumed by environmentList
      (:2568).
    - **Wiki/graft activation (daemon TS).** `knowledge.setOptions` dispatch (:758) →
      handler (~2639-2678) → `enableWiki` (:2681, `kernel.load(createWikiExtension)`
      :2706) / `enableGraft` (:2741, `createGraftExtension` :2768); graft skill object
      :159, gated into resolveSkills :1156. Slash intercept
      `commandInvocation → registeredCommand → commandDispatch` (session prompt path
      ~1703-1712; unknown `/x` chat-safe).
    - **STATE snapshot transport (Rust + frontend).** `DaemonEnvironment`
      (agent.rs:264-267) + `parse_environment` (:273) + cache `Mutex<Option<…>>` (:449);
      invalidation on `command.register | skill.register | knowledge.setOptions |
      session.new` (agent.rs:1936-1939); skills ride the snapshot (:1233) and STATE
      emission (`agent_agui.rs:139`, commands :130, branch :134, extensions :137).
      Frontend: `useSyncExternalStore` merge (CodingAgentPanel.tsx:269); partial STATE
      merge preserves omitted keys; skills card reads `snapshot.state["skills"]`.
    - **MCP primitives.** `connectAllowListedMcpServers` + `MAX_MCP_SERVERS` (mcp.ts);
      host stores `ConnectedMcpServers` (host.ts:463), connects :613, merges tools :636.
      Rust: `AgentMcpAllowListEntry` (agent.rs:97), handshake :2060-2068, all four
      production constructors empty (:135, :644, :3717, :3922) — the wiring gap.
    - **Session/usage/model/branch primitives (for meter + fixes).** `contextWindow`
      from model registry (host.ts:2264); contextTokens/contextWindow state ride STATE
      (frontend :378-384). `sessionResumable` (:1932; label/summary/snippet already
      returned), `sessionResume` (:1640). Branch: `refresh_branch` (agent.rs:235-257,
      :1126) → snapshot → frontend :329-332.
    - **Generic gaps → assigned to tasks (no mode-specific Rust anywhere):**
      (1) generic config-root JSON loader (missing=defaults, malformed=warn) shared by
      skills.json + mcp.json — task 4/11; (2) generic bounded prompt-layer file reader
      (`readPromptLayer(path, maxBytes)`) for SYSTEM.md + AGENTS.md — tasks 6/7;
      (3) MCP allow-list = plain serde table parse, entry schema the only MCP shape —
      task 11; (4) manual skill trigger reuses the existing `loadedSkills` state via the
      `load_skill` path — no new channel — task 15; (5) settings-page listing = one
      bounded server op scoped to configRoot/agent, generic over file set — task 14;
      (6) package default loading unchanged: `@clay/coding-agent` still one
      `loadPackage` line, load.js carries no skill registration since the prior-session
      change.
    - **Security classification.** Server-authoritative: config paths, discovery roots,
      allow-list, branch read, catalogs, resume workspace scoping. Webview never
      supplies config/MCP paths; @ mentions resolve against server-known catalogs only;
      prompt text passes the existing redactor.
    - **Performance doctrine.** All new I/O at host creation or session build — matches
      the registry's no-hot-path rule (config evaluated at creation, not per keypress).

- [x] Implement `skills.json` — discovery roots + third home root + `agentSkills` toggles
  - Acceptance Criteria:
    - Functional: the daemon reads `~/.config/clay/agents/coding-agent/skills.json` at
      host creation following the `tool-caps.json` pattern. Schema (locked): `roots` with
      per-root enable flags for `workspace`, `configRoot`, `home`; only `home` has a path
      override (default `~/.agents`); `agentSkills` with `wikiSearcher`, `wikiMaintainer`,
      `graft` booleans (defaults: all true — wiki skills still require `/wiki-init`).
      Discovery roots: workspace `<ws>/.agents/skills` (npx skills format), agent config
      root `<agentConfigRoot>/skills/` (seeded agent-delivered + user-added skills share
      one directory), home `<homeOverride>/skills/`. Absent or unreadable file ⇒ all
      defaults on + stderr note, never a broken session.
    - Performance: file read once per host creation (same as tool-caps.json); discovery
      results stay cached per root as before.
    - Code Quality: paths `~`-expanded or absolute only; relative rejected fail-closed
      (stderr warning + root disabled, not a crash); unknown keys follow the tool-caps.json
      precedent; caps stay hardcoded (64 skills/root, 48/96-char truncation); reserved
      agent-delivered names (`graft`, `wiki-searcher`, `wiki-maintainer`) never come from
      disk discovery — they activate only through their own paths.
    - Security: symlink-escape and ENOENT tolerance kept (workspace via Prism's
      `discoverContributions`); config file never grants anything beyond skill catalog
      content; no eval.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts` tool-caps loading and the discovery/
        `resolveSkills` implementation.
      - `@arnilo/prism`: `discoverContributions` (workspace, symlink-safe) and
        `parseSkillFile(text, path)` (direct scans).
      - Decision `decision-logs/2026-09-09-1420-per-agent-config-layout-agents-coding-agent.md`
        (per-agent config layout; data dir stays `~/.config/clay/agent`).
    - Options Considered:
      - Per-root ad-hoc extra paths: rejected at lock time (symlink inside `~/.agents`
        covers it).
      - Extending `tool-caps.json` instead of a new file: mixes concerns. (Rejected.)
      - Reusing `discoverContributions` for config/home roots: it appends `.agents/skills`
        to the given root — wrong layout for `<agentConfigRoot>/skills` and
        `<home>/skills`. (Rejected: direct `readdir` + `parseSkillFile` scan for those
        two roots; workspace keeps Prism's symlink-safe discovery.)
    - Chosen Approach: `loadSkillsConfig(agentConfigRoot, defaultHomePath)` at host
      creation; `ensureSkillDiscovery` gates per root and scans config/home via
      `scanSkillsDir` (`<root>/skills/<name>/SKILL.md`, bounded 64, reserved names
      skipped); HostOptions seams renamed `agentConfigRoot` / `homeSkillsRoot`;
      `agentSkillEnabled(name)` accessor for the wiki/graft tasks.
    - API Notes and Examples:
      ```json
      {
        "roots": {
          "workspace": { "enabled": true },
          "configRoot": { "enabled": true },
          "home": { "enabled": true, "path": "~/.agents" }
        },
        "agentSkills": { "wikiSearcher": true, "wikiMaintainer": true, "graft": true }
      }
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: `loadSkillsConfig`, `scanSkillsDir`,
        `discoverWorkspaceSkills`, root gating in `ensureSkillDiscovery`, reserved-name
        set, `agentSkillEnabled`, `HostOptions.agentConfigRoot`/`.homeSkillsRoot` seams.
      - `examples/config/agents/coding-agent/skills.json`: starter file (pure JSON,
        copy-verbatim safe).
      - `examples/config/README.md`: `agents/coding-agent/` section documenting the
        layout, roots, and toggles.
      - `packages/coding-agent/docs/index.md`: skills-source description updated.
      - `roadmap.md` Phase 2.2: paths updated to the per-agent layout.
    - References: roadmap Phase 2.2 "Configurable skill discovery" bullet; decisions
      2026-09-09-1341 (MCP) and 2026-09-09-1420 (per-agent layout).
  - Test Cases to Write:
    - clay-agent vitest: absent file ⇒ three defaults on; relative home path ⇒ root
      disabled + warning; disabled root not scanned; `~` expansion works; agentSkills
      defaults true; reserved names skipped; malformed file ⇒ defaults.
    - Rust: none (daemon-side).
  - Evidence (2026-09-09):
    - Implemented as chosen: `loadSkillsConfig` (absent/malformed/unknown-key/relative-path
      handling with stderr notes), `scanSkillsDir` direct scan with `MAX_SKILLS_PER_ROOT=64`
      and `RESERVED_AGENT_SKILL_NAMES`, workspace via `discoverWorkspaceSkills`
      (`discoverContributions`), `agentSkillEnabled` accessor, renamed seams
      (`agentConfigRoot`, `homeSkillsRoot`).
    - Starter `examples/config/agents/coding-agent/skills.json` (pure JSON) + README
      section + `packages/coding-agent/docs/index.md` + wiki `clay-agent.md` data-dir
      clarification + roadmap path updates.
    - Tests: updated the discovery test to three roots (workspace + config-root + home
      fixtures); new test `skills.json gates discovery roots; malformed file falls back to
      defaults` (root gates, relative home path rejection, unknown key, reserved-name
      skip, malformed fallback); made `compaction.test.ts` bare-chat test hermetic (empty
      skill-root seams) after the real `~/.agents/skills/find-docs` leaked into exact
      tool-array assertions.
    - Gates: `npx tsc -p tsconfig.json` clean; `npm test` 113 tests, 112 pass, 0 fail,
      1 skip.

- [x] Wiki — `/wiki-init` as the sole initiator, honoring the agentSkills gate
  - Acceptance Criteria:
    - Functional: the slash intercept (`clay-agent/src/host.ts` ~1703) treats an unmatched
      `/wiki-init` as the initiator: if `agentSkills.wikiSearcher`/`wikiMaintainer` are
      config-off, `/wiki-init` stays a chat-safe prompt; otherwise it internally enables
      the wiki binding (idempotent, one per daemon — same path `knowledge.setOptions`
      uses) and dispatches the extension's `/wiki-init`. Init semantics: workspace has
      `.wiki/` ⇒ check/align pass (reconcile bundle against current sources); no `.wiki/`
      ⇒ create from existing workspace information. All writes stay inside `.wiki/`
      (`autoDeploySkills: false` stays). wiki-searcher + wiki-maintainer skills activate
      simultaneously with successful init (registry-at-enable + per-session `resolveSkills`
      as today). Without `/wiki-init`: no wiki skills, commands, or tools — the disabled
      default.
    - Performance: intercept adds one registry lookup per slash prompt (existing path);
      wiki enable is one-time per daemon.
    - Code Quality: `knowledge.setOptions` RPC remains as the internal mechanism and
      programmatic surface; no second enable path is introduced (intercept calls the same
      enableWiki).
    - Security: `.wiki/` confinement is enforced by the Prism wiki extension's own write
      surface; the daemon never writes outside it for wiki; intercept input is a bare
      command name, no path handling.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts` intercept (~1703-1712), `knowledge.setOptions` handler
        (2639-2660), wiki enable block (kernel.load, command/tool/skill registration
        ~2639-2720).
      - `@arnilo/prism-memory/wiki` extension surface (init/refresh/lint semantics).
    - Options Considered:
      - Keep RPC-only activation (status quo): nothing in the app calls it — dormant.
        (Rejected per lock.)
      - Intercept dispatches init without enabling first: the extension's command is not
        registered yet — order must be enable-then-dispatch. (Noted in chosen approach.)
    - Chosen Approach: extend the intercept's unmatched-command branch: name ===
      `/wiki-init` && gate on ⇒ `await enableWiki()` then dispatch the now-registered
      command; env-cache invalidation already fires on knowledge.setOptions.
    - API Notes and Examples:
      ```ts
      if (!command && slash.name === "wiki-init" && this.agentSkillEnabled("wikiSearcher")) {
        await this.enableWiki();            // idempotent; registers /wiki-init etc.
        return this.commandDispatch({ name: "wiki-init", args: slash.args, ... });
      }
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: intercept branch, `agentSkillEnabled(name)` helper
        reading the skills.json flags.
    - References: roadmap Phase 2.2 "Wiki — /wiki-init is the sole initiator" bullet;
      observation of dormant knowledge.setOptions wiring.
  - Test Cases to Write:
    - vitest: `/wiki-init` with gate on ⇒ wiki commands/skills registered and init
      dispatched (mock kernel.load); gate off ⇒ chat-safe (no registration), prompt
      handled as normal text; second `/wiki-init` is idempotent (no double enable);
      writes observed only under `.wiki/` (extension fixture).
    - Existing wiki tests stay green.
  - Evidence (2026-09-09):
    - Intercept branch added in `sessionPrompt` (`clay-agent/src/host.ts`): unmatched
      `/wiki-init` with `agentSkillEnabled("wikiSearcher")` resolves the live session
      (`ensureLive`), calls `enableWiki(live.workspaceRoot)` (the exact path
      `knowledge.setOptions` uses — idempotent per workspace), then dispatches the
      now-registered extension command with the standard command-feedback triple.
      Gate off ⇒ falls through to a normal chat prompt (zero residue).
    - `enableWiki` now honors per-skill flags at registration: a gated-off
      `wikiMaintainer` never registers; `wikiSearcher` is implied by the initiator
      gate (decision 2026-09-09-1420 gate semantics).
    - Rust env-cache: `src/server/agent.rs` `rpc()` now also invalidates the
      environment cache for `session.prompt` calls whose text starts with
      `/wiki-init` (the daemon enables the binding internally, so the
      `knowledge.setOptions` passthrough never fires; marker read before `params`
      moves into the command).
    - Semantics update (locked behavior): after an explicit `wiki: false` disable,
      `/wiki-init` re-initializes (it is the sole initiator, not a chat-safe prompt
      when the gate is on). Gate-off chat-safety is preserved.
    - Tests: new `wiki-init is the sole initiator...` (init without prior
      knowledge.setOptions, `.wiki/` confinement, skills registered, tools on next
      session, idempotent re-init with no re-bind, gate-off chat-safe via skills.json
      fixture); new `wiki-maintainer gate filters only its own skill at init`;
      updated the dormant-default test (no self-activation without `/wiki-init`) and
      the disable-residue test (re-init after disable). `codingHost` helpers in
      wiki/graft tests now pass hermetic `agentConfigRoot` seams so gates never
      depend on the developer's real skills.json.
    - Rust test helper: `temp_example_config_root` now copies the example tree
      recursively (`copy_tree`) for the nested `agents/coding-agent/` starter.
    - Gates: `npx tsc` clean; clay-agent 115 tests / 114 pass / 0 fail / 1 skip;
      cargo clippy -D warnings clean; cargo test --lib 1289 passed; runtime 75 and
      protocol 209 integration tests pass.

- [x] Graft — available by default, honoring the agentSkills gate
  - Acceptance Criteria:
    - Functional: the daemon attempts the graft binding by default for the workspace
      (pull mode) — bind point: first `session.new` for a workspace root (same per-root
      binding + `resolveSkills` match wiki uses). CLI resolution stays fail-closed:
      absent `graft` CLI ⇒ off, tools hidden, agent unperturbed. `agentSkills.graft`
      config-off ⇒ no binding attempt ever. `knowledge.setOptions` remains for
      disable/mode override. The graft skill body stays registered regardless of
      connection path (CLI tools or graft MCP) — it is workflow guidance, not a tool
      duplicate.
    - Performance: CLI resolution is cached per daemon (existing fail-closed resolver);
      bind attempt is once per workspace root per daemon.
    - Code Quality: one enableGraft path (default-on trigger and RPC both call it);
      no retry loops.
    - Security: graft CLI invocation keeps the existing canonical-path resolution and
      literal argv discipline; binding never reads webview input.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts` graft enable block (~2692), CLI resolution, skill at :159,
        `resolveSkills` workspace matching.
    - Options Considered:
      - Bind at host creation using process.cwd(): wrong root for multi-workspace
        sessions. (Rejected.)
      - Bind on every session.new: repeated work; per-root caching already exists.
    - Chosen Approach: in `sessionNew` (after skill discovery, before createSession),
      attempt `ensureGraftBound(workspaceRoot)` gated by config flag.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: `ensureGraftBound`, gate check, sessionNew hook.
    - References: roadmap Phase 2.2 "Graft — available by default" bullet.
  - Test Cases to Write:
    - vitest: session.new with resolvable CLI (fixture stub) ⇒ graft tools + skill active;
      unresolvable CLI ⇒ session starts clean, no error surfaced; config-off ⇒ no
      resolution attempted; knowledge.setOptions override still wins.
  - Evidence (2026-09-09):
    - `ensureGraftBound(workspaceRoot)` in `clay-agent/src/host.ts`: gated on
      `agentSkills.graft`, early-returns when already bound for the root, one attempt
      per workspace root per daemon (`graftBindAttempted` set — success and
      fail-closed outcomes both consume the attempt, no retry loops), then calls the
      single `enableGraft(workspaceRoot, { mode: "pull", cliPath? })` path. Load
      failures are swallowed in the default path (fail closed, agent unperturbed);
      the explicit RPC path still throws for caller feedback.
    - Hook: `sessionNew` runs `ensureGraftBound` inside the `wantsCoding` block
      (before `ensureSkillDiscovery`/`createSession`) — chat sessions never trigger a
      bind.
    - Gate authority: `enableGraft` checks `agentSkills.graft` at entry, so a
      config-off gate blocks BOTH the default attempt and explicit
      `knowledge.setOptions graft:true` (decision 2026-09-09-1420: gate off ⇒ no
      binding ever; the RPC remains the disable/mode-override surface).
    - New `HostOptions.graftCliPath`: default CLI path for the daemon-initiated bind
      and the `enableGraft` fallback when the RPC omits `graftCliPath`. Absent ⇒
      peer-package resolution only (unchanged fail-closed semantics; the resolver
      never PATH-searches). Also the test seam that makes default-bind tests
      deterministic on machines without the peer.
    - Tests (`graft-knowledge.test.ts`): new `graft is available by default...`
      (first session binds: tools registered, graft skill catalogued, extension in
      the environment; same-root re-session does not re-bind; explicit disable
      unbinds; explicit RPC re-binds); new `agentSkills.graft=false blocks every
      binding path...` (no default bind, RPC refused, no binding materializes);
      reworked the absent-CLI test to be deterministic everywhere
      (`graftCliPath: "/nonexistent/graft-cli"` — the old test depended on no peer
      being installed). `codingHost` grew the `graftCliPath`/`agentConfigRoot` opts.
    - Existing tests: explicit-RPC fixture tests pass unchanged (default attempt
      consumed at session.new does not block the explicit path — the attempt cache
      lives in `ensureGraftBound`, not `enableGraft`).
    - Gates: `npx tsc` clean; clay-agent 117 tests / 116 pass / 0 fail / 1 skip;
      `cargo fmt --check`, clippy -D warnings, cargo test --lib 1289 passed.

- [x] File-backed agent-delivered skills — seed and load `<name>/SKILL.md`
  - Acceptance Criteria:
    - Functional: on host creation (installation/first launch), the daemon seeds
      `~/.config/clay/agents/coding-agent/skills/graft/SKILL.md`, `.../wiki-searcher/SKILL.md`,
      `.../wiki-maintainer/SKILL.md` from the built-in definitions when absent (deleted ⇒
      regenerated). The daemon always loads skill content (frontmatter description +
      instructions body) from these files; the in-code objects are generators, not the
      delivered payload. Wiki skills seed at host creation too (content exists before
      `/wiki-init`; activation still waits for init). User edits take effect on next
      daemon start. Disk discovery skips these reserved names (task 4's
      `RESERVED_AGENT_SKILL_NAMES`), so seeding + gated activation is the only
      delivery path.
    - Performance: three small file reads/writes at host creation; skill registry entries
      built from parsed files.
    - Code Quality: `toolNames` stay daemon-owned (parsed file cannot change them — edits
      can't break activation or the fail-closed toolName check); parse via Prism's
      `parseSkillFile`; name fixed by directory name.
    - Security: files live under the user config root (user-owned, like init.js); oversized
      or malformed file ⇒ stderr warning + built-in seed content (never a broken skill);
      bounded bytes like discovery.
  - Approach:
    - Documentation Reviewed:
      - `@arnilo/prism` root export `parseSkillFile` (frontmatter name/description/tools
        + body parsing).
      - `clay-agent/src/host.ts` graftSkill (:159) and wiki skill imports (:90).
    - Options Considered:
      - Optional override files (built-in default unless overridden): rejected — the file
        is the delivery format, always present, always loaded (user lock).
      - Live reload on file change (fs watcher): YAGNI; next-start semantics documented.
    - Chosen Approach: `seedAgentSkillFiles(configRoot)` at host creation; registration
      sites (enableWiki, graft bind, and any future agent skill) register the parsed file
      content merged with daemon-owned toolNames.
    - API Notes and Examples:
      ```md
      ---
      name: graft
      description: Use the graft context graph to locate code by architecture...
      ---
      <instructions body — user editable>
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: seeding, load-from-file at register sites.
      - `examples/config/README.md`: document the seeded layout.
    - References: roadmap Phase 2.2 "Agent-delivered skills are file-backed" bullet.
  - Test Cases to Write:
    - vitest: first host creation seeds all three files; edited body loads into the
      registry; deleted file regenerates; malformed file falls back to seed with warning;
      toolNames never come from the file.
  - Evidence (2026-09-09):
    - `clay-agent/src/host.ts`: `AGENT_DELIVERED_SKILLS` table (wiki-searcher,
      wiki-maintainer, graft — in-code objects are generators), `skillToSeedMarkdown`
      (frontmatter `name`/`description` + body), `seedAgentSkillFiles` (absent ⇒ seed;
      present ⇒ untouched), and `loadAgentSkillFiles` (parse via Prism
      `parseSkillFile`, `MAX_AGENT_SKILL_FILE_BYTES = 256 KiB`; malformed/oversized ⇒
      stderr warning + built-in seed content in the registry, user file never
      clobbered; seeding failure ⇒ built-in content). Delivered skills get the
      directory name as `name` and daemon-owned `toolNames` merged over whatever the
      file declares — a file cannot rename a skill, grant tools, or break the
      fail-closed toolName check.
    - Host creation loads the map (`await loadAgentSkillFiles(agentConfigRoot)`
      constructor param); registration sites now deliver from it: `enableWiki`
      (wiki-searcher/wiki-maintainer, per-skill gate) and `enableGraft` (graft), with
      the in-code objects as fallback. Wiki content now exists before `/wiki-init`
      (activation still waits for init); user edits apply on next daemon start.
    - `examples/config/README.md` seeded-layout bullet updated (source of truth,
      restart semantics, regeneration, malformed fallback).
    - Tests (`src/__tests__/agent-skill-files.test.ts`, new file): seeds all three
      files at first creation; edited description/body delivered by the registry;
      deleted file regenerates from the built-in seed; unterminated-frontmatter file
      ⇒ built-in content + user file left alone; `toolNames: [shell, write]` in a
      file cannot escalate — daemon-owned wiki toolNames survive. (Note:
      `skill.list` truncates descriptions at the 96-char display cap — assertions
      read the registry, not the display list.)
    - Gates: `npx tsc` clean; clay-agent 122 tests / 121 pass / 0 fail / 1 skip
      (Rust unchanged).

- [x] User `SYSTEM.md` — global system-prompt layer
  - Acceptance Criteria:
    - Functional: host creation seeds `~/.config/clay/agents/coding-agent/SYSTEM.md` from a built-in
      default when absent; session build loads it as the user-owned system-prompt layer —
      the global complement to the workspace `AGENTS.md` layer. Layer order (locked):
      base instructions (profile) → `SYSTEM.md` (user) → `AGENTS.md` (workspace app
      layer). Absent/oversized ⇒ seed/default + warning; edits apply on next session
      build.
    - Performance: one bounded file read per session build; byte-stable per session so it
      rides the cached prefix (cache_aware layout — no per-turn churn).
    - Code Quality: rides the same trust/size discipline as skill discovery (symlink
      under config root is fine — user-owned; bounded bytes; silent skip when unreadable
      after warning).
    - Security: content is prompt text only; no tool grants, no eval; config-root owned.
  - Approach:
    - Documentation Reviewed:
      - Prism prompt assembly: `composeSystemPrompt` layering (base + contributions),
        cache_aware layout order (stable system prefix before history).
      - `packages/coding-agent/dist/load.js` `CODING_PROFILE` instructions (base layer).
    - Options Considered:
      - Read via Prism's `resolveAgentBundle` SYSTEM.md path: pulls the whole bundle
        resolution Clay deliberately bypasses. (Rejected — direct read.)
    - Chosen Approach: `loadUserSystemPrompt(configRoot)` at session build; inject through
      the same system-message assembly as AGENTS.md (next task shares the mechanism).
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: seeding + session-build load + layer merge.
    - References: roadmap Phase 2.2 "User SYSTEM.md" bullet; Prism app-config layer order.
  - Test Cases to Write:
    - vitest: seeds when absent; loaded text appears as a system layer in the built
      session prompt (inspect provider input fixture); layer order base → SYSTEM.md →
      AGENTS.md; oversized ⇒ skipped with warning; session-stable across turns.
  - Evidence (2026-09-09):
    - `clay-agent/src/host.ts`: `seedUserSystemPrompt` seeds
      `<agentConfigRoot>/SYSTEM.md` EMPTY at host creation (a default body would
      pollute every session's prompt — the empty file is the stable home + settings-page
      provenance); `loadUserSystemPrompt` reads it per session build (sync, bounded at
      `MAX_SYSTEM_PROMPT_BYTES = 64 KiB`; oversized/unreadable ⇒ stderr warning + empty
      layer, never a broken session; trimmed).
    - `createSession` injects the text as a `user`-source contribution
      (`id: "user-system-md"`) merged ahead of the profile's own `systemPrompt`
      contributions — Prism's `composeSystemPrompt` source rank (user=0 < app=2) places
      it after the base instructions and before the future AGENTS.md app layer;
      `def.systemPrompt === false` suppresses the user layer with the profile's own.
      `createSession` returns the MERGED config, so the context inspector shows the
      layer today (full inspector fix is task 8). Applies to session.new, resume, and
      model-recreate (all route through `createSession`). Byte-stable per session ⇒
      rides the cached prefix.
    - Tests (`agent-skill-files.test.ts`): seeded empty at creation; user text reaches
      the provider request's system message AFTER the profile base; identical system
      text across two turns; >64 KiB file skipped (no leak into prompt), session builds.
    - Gates: `npx tsc` clean; clay-agent 125 tests / 124 pass / 0 fail / 1 skip
      (Rust unchanged). AGENTS.md ordering assertion lands with the workspace
      AGENTS.md task (same fixture).

- [x] Workspace `AGENTS.md` — app project-prompt layer
  - Acceptance Criteria:
    - Functional: `<workspaceRoot>/AGENTS.md` is read at session build and injected into
      the system message block after `SYSTEM.md`. Symlink-escape excluded, bounded bytes,
      silent skip when absent (ENOENT-tolerant like discovery). Session-stable.
    - Performance: one bounded stat+read per session build; no per-turn I/O.
    - Code Quality: same reader helper as SYSTEM.md (parameterized root + filename);
      no new Prism dependency.
    - Security: repo content is untrusted input rendered as prompt text — bounded bytes
      (cap recorded in code), no special parsing, no tool grants; consistent with the
      threat model where repo text already flows through prompts.
  - Approach:
    - Documentation Reviewed:
      - Prism `resolveAgentBundle` repo-prompt semantics (`include.repoPrompt`, source
        "app") — the rank Clay mirrors with a direct read.
    - Options Considered:
      - Adopt `resolveAgentBundle` wholesale: changes AgentDefinition construction.
        (Rejected at analysis time — direct read keeps the current profile flow.)
    - Chosen Approach: shared `readPromptLayer(path, maxBytes)` used by both layers.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: workspace AGENTS.md read at session build.
    - References: roadmap Phase 2.2 "AGENTS.md as system layer" bullet.
  - Test Cases to Write:
    - vitest: present file injected; absent file skipped silently; symlink escaping the
      workspace excluded; oversized truncated/skipped per cap; layer order preserved.
  - Evidence (2026-09-09):
    - `clay-agent/src/host.ts`: shared `readPromptLayer(file, maxBytes)` reader (trimmed
      text; ENOENT ⇒ ""; unreadable/oversized ⇒ throw, so each caller applies its own
      trust policy). `loadUserSystemPrompt` now rides it (unusable ⇒ warn + skip,
      user-owned); `loadWorkspaceAgentsPrompt` adds the repo policy: realpath of the
      file must stay inside the realpath of the workspace root (symlink escape ⇒
      silent skip), ENOENT silent (most repos have none), oversized/unreadable silent
      skip — repo text is untrusted prompt content, consistent with the threat model.
    - `createSession` builds a `layers` array: `user-system-md` (source `user`) then
      `agents-md` (source `app`) ahead of the profile's own contributions;
      `composeSystemPrompt`'s rank sort (user=0 < app=2) yields the locked order base
      → SYSTEM.md → AGENTS.md. Shared `MAX_PROMPT_LAYER_BYTES = 64 KiB` cap for both
      layers. Session-stable ⇒ cached prefix. Covers session.new / resume /
      model-recreate via `createSession`.
    - Tests (`agent-skill-files.test.ts`): full 3-layer order assertion (base <
      SYSTEM.md < AGENTS.md positions in the provider request's system message) +
      byte-stability across turns; escape symlink (target outside workspace) not
      injected; >64 KiB file not injected; symlink contained within the workspace IS
      injected; absent file contributes nothing (all other sessions run without one).
    - Gates: `npx tsc` clean; clay-agent 126 tests / 125 pass / 0 fail / 1 skip
      (Rust unchanged).

- [x] Context inspector shows the real composed system prompt
  - Acceptance Criteria:
    - Functional: the inspector's "System prompt" group renders the composed layers the
      model actually receives: base instructions (`agent.config.instructions`), any
      `systemPromptContributions(live.systemPrompt)`, the user `SYSTEM.md` layer, and the
      workspace `AGENTS.md` layer — each labeled with its source. No longer empty for the
      coding profile.
    - Performance: inspection is on-demand (existing RPC); bounded text per item like
      other inspector groups.
    - Code Quality: builds on the existing `contextItems`/`contextItem` structures
      (`clay-agent/src/host.ts` ~1406); no new transport.
    - Security: read-only view; redaction posture same as other inspector items.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts` context inspector (~1252-1406); `systemPromptContributions`.
      - Prism `composeSystemPrompt` layer model.
    - Options Considered:
      - Move `CODING_SYSTEM_PROMPT` into profile `systemPrompt`: changes delivery
        semantics for all profiles. (Rejected — render-time fix is smaller.)
    - Chosen Approach: prepend an "instructions (base)" item and append the new layers in
      the inspector's system-prompt group.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: inspector group assembly.
    - References: roadmap Phase 2.2 inspector bullet.
  - Test Cases to Write:
    - vitest: inspector returns base + SYSTEM.md + AGENTS.md items with sources; empty
      contributions case still renders base.
  - Test Cases note: existing inspector tests updated for new items.
  - Evidence (2026-09-09):
    - `clay-agent/src/host.ts`: `contextCategories` prepends a "System prompt
      (base instructions)" item from `live.agent.config.instructions` (skipped when
      absent/empty — Chat profile still renders an empty group), then contributions
      via `rankedSystemPromptContributions` — a stable rank sort mirroring Prism's
      `composeSystemPrompt` source order (user=0, package=1, unknown=1.5, app=2,
      run=3) so the inspector lists layers in the order the model actually receives
      them. `systemPromptContributions` now also returns `source`;
      `PROMPT_LAYER_LABELS` maps host-owned ids to friendly source labels ("user
      SYSTEM.md", "workspace AGENTS.md"), unknown ids pass through (e.g. profile
      "core"). `contextItem` handles the new `system-prompt-base` id and uses the
      same ranked array for detail fetches (bounded like every group).
    - createSession already returns the merged config (user SYSTEM.md + AGENTS.md +
      profile contributions), so the layers appear without further wiring; the
      coding profile's group is no longer empty (base instructions always render).
    - Tests (`context.test.ts` new case): profile with base instructions + user
      SYSTEM.md + workspace AGENTS.md ⇒ systemPrompt items exactly [base, user
      SYSTEM.md, workspace AGENTS.md] with correct previews, and each item's full
      content fetchable via `session.context {itemId}` (same RPC, item-detail mode).
      The existing "Ctx" fixture (contribution only, no base) still renders one item.
    - Gates: `npx tsc` clean; clay-agent 127 tests / 126 pass / 0 fail / 1 skip
      (Rust unchanged; frontend takes titles/previews from the wire — no UI change).

- [x] MCP config surfaces — parse `mcp.json` and `.mcp.json`, build the allow-list (Rust)
  - Acceptance Criteria:
    - Functional: the server parses user `~/.config/clay/agents/coding-agent/mcp.json` (tool-caps.json
      pattern: `{ "servers": { "<serverId>": { "command", "args", "env", "cwd",
      "timeoutMs" } } }`) and repo-root `.mcp.json` (Claude Code convention). Entries from
      both sources are merged into `AgentMcpAllowListEntry` (server_id unique; user file
      wins on id collision) and sent in the `initialize` handshake, replacing the five
      `mcp_allow_list: Vec::new()` sites (`src/server/agent.rs:135, 644, 3717, 3922` — the
      production ones get real lists; test constructors keep explicit empties).
    - Performance: parse at agent-actor spawn, bounded entries (max 32, Prism cap);
      no hot-path work.
    - Code Quality: same validation split as the locked decision: absolute canonical
      command paths pass as-is; **bare command names may PATH-resolve** (decision-log
      amendment task); literal argv ≤64, explicit env names only, optional cwd; unknown
      keys warned per tool-caps precedent; malformed file ⇒ warning + empty list, never a
      failed startup.
    - Security: no approval gate (user decision — their servers, their responsibility);
      package JS still never supplies argv (two-trust-domain invariant intact); env values
      pass through but env *names* are explicit; no shell interpolation anywhere.
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs` `AgentMcpAllowListEntry` (:97-103), handshake (:2060-2068).
      - `clay-agent/src/mcp.ts` validation rules (absolute path, literal argv, env names,
        MAX_MCP_SERVERS=32).
      - Claude Code `.mcp.json` format (`mcpServers` key, `command`/`args`/`env`).
    - Options Considered:
      - Daemon reads the files itself: config parsing is server-side today
        (tool-caps precedent is daemon-side, but allow-list construction is already
        server-side in the handshake — keep parsing where the list is built). (Chosen:
        server parses, daemon validates.)
    - Chosen Approach: new `src/server/agent_mcp_config.rs` (or smallest extension of
      agent.rs): parse both files with serde, PATH-resolve bare names via canonical
      lookup, emit validated entries + warnings.
    - API Notes and Examples:
      ```json
      { "mcpServers": { "graft": { "command": "graft", "args": ["mcp"], "env": {} } } }
      ```
    - Files to Create/Edit:
      - `src/server/agent.rs` (or new module): parsing + allow-list build + the
        production call sites.
      - `examples/config/agents/coding-agent/mcp.json`: starter (commented examples only, JSON-C style
        guidance in README).
    - References: roadmap MCP bullet; decision-log task above; Prism cap inventory
      (tools/list rejects over-cap; call results truncate).
  - Test Cases to Write:
    - Rust unit: parse both formats; bare-name PATH resolution; absolute passthrough;
      relative command rejected; id collision user-wins; malformed file ⇒ warning +
      empty; env-name explicitness enforced; 32-server bound.
    - Integration: handshake carries the built list (existing protocol tests extended).
  - Evidence (2026-09-09):
    - New module `src/server/agent_mcp_config.rs`: `build_mcp_allow_list(config_root,
      workspace_root)` reads user `<configRoot>/agents/coding-agent/mcp.json`
      (`{"servers": {...}}`, optional `cwd`/`timeoutMs`) and repo `<root>/.mcp.json`
      (`{"mcpServers": {...}}`), merging into `AgentMcpAllowListEntry` entries — user
      file first, id collision ⇒ user wins (warned), cap 32 (over-cap dropped with a
      warning). Per entry: unknown keys warned by name (tool-caps precedent), empty
      id/command dropped, argv > 64 dropped, commands resolved per the locked decision
      (absolute pass as-is; bare names PATH-resolve + canonicalize; relative paths
      rejected). Missing file = silent; unreadable/malformed ⇒ warning + empty —
      startup never fails on config. No approval gate (decision 2026-09-09-1341).
    - `AgentMcpAllowListEntry` gained `timeout_ms: Option<u64>` (carried in the
      handshake JSON as `timeoutMs`; consumed by the daemon connect task).
    - Wiring: `AgentHostConfig::for_server(root, workspace_root)` builds the list;
      `AgentHost::for_server` passes through; `src/server/mod.rs` passes
      `workspace_roots.first()` with a `current_dir()` fallback mirroring
      `add_root_from_cwd`. Test constructors (`inert()`, protocol-fixture
      `host_for`, two in-file AgentHostConfig literals) keep explicit empties.
    - `examples/config/agents/coding-agent/mcp.json` starter (`{"servers": {}}`) +
      README section (shape, PATH/absolute rules, collision rule, no-gate posture,
      32 cap).
    - Tests: 7 unit tests in the module (absolute passthrough, bare-name PATH
      resolution, relative rejection + missing-bare-name drop, user-wins collision,
      malformed ⇒ empty, 32 cap, absent-silent); integration test
      `initialize_handshake_carries_the_built_mcp_allow_list` in
      `tests/agent_protocol.rs` (real config builder + scripted daemon echoing the
      initialize params back as the session id — asserts serverId + command ride the
      handshake). NOTE: env explicitness is enforced structurally (serde map of
      name→literal value, no inheritance) — covered by the passthrough test.
    - Gates: cargo fmt clean; clippy -D warnings clean; lib 1296 passed; protocol
      210 passed (209 + 1); runtime 75; security 152; clay-agent untouched
      (daemon-side consumption is the next task).

- [x] MCP daemon connection — per-server fault isolation + timeoutMs
  - Acceptance Criteria:
    - Functional: `connectAllowListedMcpServers` becomes per-server fault-isolated: each
      entry connects independently; one failing or over-cap server (oversized schema,
      >500 tools, crashed binary) hides only its own tools — healthy servers stay
      connected (matching the "connection failures hide the tools" contract). Per-server
      `timeoutMs` (default 60s) rides the connect options.
    - Performance: connects happen once at session/tool-build (existing merge point
      `host.ts:636`); no retries.
    - Code Quality: connection results (serverId → connected/tools/hidden-because)
      surfaced for the UI task; stderr logging per failed server.
    - Security: validation unchanged from the amendment (PATH-resolve only for config
      entries); merged tools keep the `mcp:<serverId>:<name>` prefix.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/mcp.ts` current all-or-nothing loop; Prism MCP bridge options
        (timeout, caps).
    - Options Considered:
      - Keep all-or-nothing: one bad server bricks every MCP tool — rejected (locked).
    - Chosen Approach: Promise.allSettled per server; collect per-server outcomes.
    - Files to Create/Edit:
      - `clay-agent/src/mcp.ts`: isolation + outcomes; `clay-agent/src/host.ts`: merge +
        outcome publication (environmentList or a new state field for the UI task).
    - References: roadmap MCP bullet (fault isolation), Prism cap semantics (reject on
      list, truncate on call).
  - Test Cases to Write:
    - vitest with stub servers: one failing + one healthy ⇒ healthy tools present,
      failing absent, no throw; timeoutMs plumbed to connect options.
  - Evidence (2026-09-09):
    - `clay-agent/src/mcp.ts`: `connectAllowListedMcpServers` validates the WHOLE
      list first (fail-closed rpcCode -32602 for malformed shape — including the new
      `timeoutMs` field: positive integer ≤ Prism `HARD_CALL_TIMEOUT_MS` — and a new
      duplicate-serverId check), then connects every entry via `Promise.allSettled`:
      a rejected connect (ENOENT binary, over-cap tools, timeout) hides only that
      server's tools and lands in `outcomes`, healthy servers stay bridged. New
      `McpServerOutcome {serverId, connected, tools, error?}` published on
      `ConnectedMcpServers.outcomes`; per-server stderr warning on failure. No
      retries; connects stay once at first session (existing merge point).
    - `timeoutMs` rides `connectMcpTools({callTimeoutMs})`; absent = Prism default
      60s. Prism semantics discovered by probe: a timed-out call RESOLVES with an
      error ToolResult (`error.message` = "MCP tool call timed out after Nms") —
      call errors surface as results, never thrown to the model.
    - `clay-agent/src/host.ts`: captures `mcpOutcomes` after connect (cleared on
      close); `environmentList()` now publishes `mcpServers: [{serverId, connected,
      tools, error?}]` (bounded to 32 entries, ids/errors trimmed to the completion
      caps) — additive key, tolerated by old Rust parsers.
    - Rust wire: new `AgentMcpServerInfo {server_id, connected, tools, error}` in
      `src/protocol/agent.rs`; `AgentSessionSnapshot.mcp_servers` upgraded from
      `Vec<String>` (allow-list names, decision 1758) to `Vec<AgentMcpServerInfo>`
      (real connect outcomes); `parse_environment` parses `mcpServers` (bounded
      32/48/96 like the other environment keys); `snapshot_for` /
      `unconfigured_snapshot` take environment outcomes; the pump's spawn-time
      capture is now empty (outcomes are post-connect truth; state-merge keeps the
      previous value — `mcpServers` moved into the omit-when-empty block in
      `snapshot_events`); `mcp_server_names()` deleted.
    - Frontend strip (`CodingAgentPanel.tsx`): reads the new object shape; status
      row shows connected server names only (`connectedMcpServers`).
    - Tests: `mcp-v2.test.ts` — rewrote the all-or-nothing test as "per-server
      fault isolation: one failing server hides only its own tools" (healthy tools
      present, failing absent, no throw, outcome carries the error); "timeoutMs
      rides the connect options" (200ms ceiling vs 1s fixture sleep ⇒ error
      ToolResult mentions the timeout); validation test (timeoutMs 0/1.5/over-ceiling,
      duplicate serverId all rejected). Fixture server gained a fixed-1s `sleep`
      tool. `agent_agui.rs` test asserts the rich `mcpServers` state shape.
    - Gates: clay-agent npm test 0 failures; frontend panel 22 passed; cargo fmt
      clean; clippy -D warnings clean; lib 1296 passed; protocol 210; runtime 75;
      security 152.

- [x] MCP UI surfaces — session-start card + composer connections section
  - Acceptance Criteria:
    - Functional: (1) a pinned card above the transcript (same slot as the skills card)
      lists connected MCP servers and their tool counts, persists once messages arrive,
      hidden when no server connects; (2) below the message input box, a section shows
      MCP connections (server + tool state, hidden/failed servers marked). Both read the
      daemon-published connection outcomes.
    - Performance: renders from snapshot state only; no polling beyond existing STATE
      events.
    - Code Quality: reuses the skills-card component pattern (`CodingAgentPanel.tsx` +
      `coding-agent.module.css`); cataloged primitives/tokens only.
    - Security: display-only; no server control actions (restart/connect) — future work
      explicitly out of scope.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md`, `references/components.md`,
        `references/tokens.md` (UI gate — mandatory for this task).
      - Skills-card implementation from the prior session (`CodingAgentPanel.tsx`).
    - Options Considered:
      - Fold MCP servers into the skills card: different lifecycle/data; separate card
        locked. (Rejected.)
    - Chosen Approach: mirror the skills card structure; composer section under the
      status row.
    - Files to Create/Edit:
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`, `coding-agent.module.css`.
    - References: roadmap "MCP UI surfaces" bullet.
  - Test Cases to Write:
    - Panel tests: card renders with servers and hides when empty; composer section
      shows hidden/failed state; both persist across transcript growth.
  - Evidence (2026-09-09):
    - UI gate honored: read `references/ui.md` + `references/components.md` +
      `references/tokens.md` before editing. Ordinary component work — the four
      design skills not loaded (per the routing rule); primitives/cataloged patterns
      only; token-only styling (no literals, no new tokens/kinds — catalog unchanged,
      drift tests unaffected).
    - `frontend/src/coding-agent/CodingAgentPanel.tsx`:
      - `McpCard` (memo, mirrors `SkillsCard`): pinned above the transcript directly
        under the skills card, `aria-label="MCP servers connected"`, `boxLabel` "MCP
        servers"; per row: serverId + tool count for connected servers or "hidden:
        <error>" for failed ones; returns `null` when the list is empty (hidden when
        nothing connects, matching the skills-card contract).
      - Composer section: `mcpSection` list inside `composerArea` after the form,
        `aria-label="MCP connections"`, same per-row state, hidden when empty —
        display-only (no restart/connect actions; explicitly out of scope).
      - State parse unified into one bounded `useMemo` (serverId string filter,
        connected boolean, integer tools ≥ 0, error string) — the status strip's
        `connectedMcpServers` derives from the same list.
    - `coding-agent.module.css`: `.mcpCard` (surface-panel card, neutral
      `border-subtle` left edge — accent edge stays on skills), `.mcpList`/`.mcpRow`
      (same hairline rhythm as skills), `.mcpError`
      (`var(--clay-diagnostic-error)` theme token), `.mcpSection`/
      `.mcpSectionRow` (hairline top divider, spacing tokens only).
    - Tests (`CodingAgentPanel.test.tsx`, 3 new — 25 total in file): card lists
      connected servers with tool counts and marks hidden ones; card hidden when no
      server configured; composer section shows tool state + hidden reason and both
      surfaces persist across transcript growth. Test lessons: the panel region
      pre-exists before the STATE render, so lookups must `findByLabelText` (await
      the coalesced state paint) rather than `findByRole(region)` + sync asserts.
    - Gates: frontend 284/284 (37 files), panel file 25 passed, tsc clean, eslint
      adds zero new problems vs the stashed baseline (pre-existing non-null-assertion
      errors untouched); cargo lib 1296 passed; clay-agent 0 failures.

- [x] Agent settings page — view/edit agent-delivered files
  - Acceptance Criteria:
    - Functional: a config/settings button in the coding-agent view opens a settings
      page listing every agent-delivered file: `SYSTEM.md` plus each seeded
      `skills/<name>/SKILL.md`, with provenance (built-in seed vs user-edited, e.g.
      mtime/hash comparison). Selecting a file loads it into the Clay editor view for
      view/edit/save (save writes through the document surface). The page is the future
      home for agent configuration — for now only these files.
    - Performance: file list loads on page open; edits save through the normal document
      pipeline (no agent hot-path involvement).
    - Code Quality: built as a shell surface (Clay-owned), not a package contribution;
      reuses the pane/document primitives per the UI gate; edits apply on next daemon
      start (communicated in the page copy).
    - Security: only files under `~/.config/clay/agents/coding-agent/` are listable/editable through
      this surface (path built server-side from the config root; no webview-supplied
      paths); save is a normal user document write.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md`, `references/components.md`,
        `references/tokens.md` (UI gate).
      - The four design skills (`impeccable`, `full-output-enforcement`,
        `high-end-visual-design`, `design-taste-frontend`) — substantial new surface;
        load at execution time.
      - `docs/reference/ui-components.md` (navigation/contract entry point).
    - Options Considered:
      - Modal overlay instead of a page: settings will grow (user statement); a page
        with left rail fits the shell. (Chosen.)
      - Raw textarea in the panel: bypasses the editor view, no syntax help. (Rejected —
        user asked for the Clay editor view.)
    - Chosen Approach: new settings route/panel in the shell; server op lists
      agent-delivered files (bounded, config-root scoped); documents open by the normal
      open-document path.
    - Files to Create/Edit:
      - `frontend/src/` settings page + entry button; Rust listing op; wiki/docs per
      documentation tasks.
    - References: roadmap "Agent settings page" bullet.
  - Test Cases to Write:
    - Rust: listing op returns exactly the config-root files, rejects outside paths.
    - Panel tests: page lists files with provenance; open routes to the document surface;
      save round-trips.
  - Evidence (2026-09-09):
    - UI gate honored (ui.md/components.md/tokens.md re-read; token-only styling, catalog
      patterns only — no new tokens/kinds, catalog drift tests unaffected). The four
      design skills not loaded: ordinary settings surface, no bespoke visual design.
    - Wire (Rust): `ClientMessage::ListAgentSettingsFiles` /
      `OpenAgentSettingsFile { name }`; `ServerMessage::AgentSettingsFiles`;
      `AgentSettingsFileInfo { name, displayPath, sizeBytes, modifiedMs, edited }` in
      `src/protocol/mod.rs` (rkyv+serde, codec round-trips covered).
    - Server (new `src/server/agent_settings.rs`): `agent_config_root`
      (configuration root or `~/.config/clay`, joined with `agents/coding-agent`),
      `list_agent_settings_files` (fixed layout `SYSTEM.md` + `skills/*/SKILL.md`,
      sorted, 64-cap, strays ignored), `resolve_agent_settings_file` (allowlist shape
      `SYSTEM.md` | `skills/<dir>/SKILL.md`, bounded dir chars, canonicalize +
      containment — symlink escape rejected). Provenance: compares each file against
      the daemon-written `.seed-manifest.json` (sizeBytes+mtimeMs): match ⇒ built-in,
      mismatch/absent ⇒ edited (honest fallback, no new deps — size/mtime, not hash).
    - Connection arms in `src/server/connection/mod.rs`: list answers the bounded
      snapshot (no root ⇒ empty page, never an error); open validates + resolves
      server-side and opens through `open_selected_file_response` +
      `write_document_open_response` — the OS-dialog machinery minus the capability
      (the name carries no path authority). Save rides the ordinary `SaveDocument`
      pipeline; identity/tab-state routing extended for the two messages.
    - Daemon (`clay-agent/src/host.ts`): `recordSeedStamp` — after seeding
      `SYSTEM.md` or a skill SKILL.md, the size+mtime stamp lands in
      `.seed-manifest.json` (read-modify-write; failures degrade to "edited").
    - Intent lane: `is_agent_surface_command` extended with
      `coding-agent.agentSettings.open|close` (projection-only toggle, same as the
      coding-agent surface). Package `package.json` contributions register the two
      commands (apiPrefix namespace rule) + `agentSettings.open` action target;
      `load.js` mirrors it in `AGENT_SURFACE.actionTargets`.
    - Shell: `CodingAgentPanel` composer actions gained an "Agent settings" icon
      button (`sendIntent`, core-icon fallback label — no new icon pack entry);
      `workspace-commands.ts` maps `coding-agent.agentSettings.open|close` (open
      subscribes the active pane session's feature lane + requests the listing);
      `workspace-controller.ts` owns `agentSettingsOpen`/`agentSettingsFiles` state,
      `subscribeAgentSettings` teardown, `openAgentSettingsFile` (opens into the
      active pane via `session.openAgentSettings`, drops the agent surface so the
      editor is visible, closes the page), `closeAgentSettings`;
      `PackageWorkspace` renders the new `AgentSettingsPanel` beside `SettingsPanel`;
      `WorkspacePanes` passes state + callbacks.
    - Page (`frontend/src/agent-settings/`): heading + close, copy states "Edits
      apply on the next daemon start", bounded list with size + provenance badge,
      loading/empty states; token-only CSS (`surface-panel`, `border-subtle`,
      `accent-primary`, spacing/dimension/typography tokens).
    - Tests: `agent_settings.rs` 6 unit tests (exact layout, provenance badges,
      edited-detection, allowlist rejections incl. traversal/absolute/nesting,
      symlink escape, per-agent root join); codec round-trips for the two client
      messages + the server reply; connection test extends the projection list;
      daemon seed-manifest test; `AgentSettingsPanel.test.tsx` 4 tests (provenance
      list, open routes, close, loading/empty).
    - Gates: cargo lib 1302 / agent_protocol 152 / security 152, clippy -D warnings
      clean, fmt clean; frontend 288/288 (38 files), tsc clean, eslint zero new
      problems; clay-agent 129 pass / 0 fail / 1 skip.
  - Follow-up (2026-09-10, user-reported "clicking the agent settings button does
    nothing"):
    - Root cause 1 (dead button): the composer button sent
      `sendIntent("agentSettings.open")` while the registered command — and the
      surface's declared action target — is `coding-agent.agentSettings.open`
      (`packages/coding-agent/package.json`, `load.js AGENT_SURFACE.actionTargets`,
      `is_agent_surface_command`). The unprefixed id matched no server branch
      (`is_settings_command` is `settings.`-prefixed, `is_chat_command` is `chat.`)
      and fell through to the empty-registry executor, so the intent was rejected
      before it ever reached the client-command map that owns the page toggle. Every
      other composer intent (`coding-agent.close`, `agent.clientOpenModelPicker`, …)
      was already prefixed; this one was not.
    - Fix 1: prefix the id. Pinned by a new frontend test that parses every
      `sendIntent` the composer emits and asserts it is a declared
      `surface.actionTargets` entry — the general invariant that would have caught
      this without knowing the id (the test fixture's `actionTargets` now mirrors the
      shipped list instead of a two-entry stub).
    - Root cause 2 (found while verifying the page, would have shipped as a wrong
      badge): every untouched seed file rendered as **edited**. The daemon stamped
      `Math.round(stats.mtimeMs)` in `.seed-manifest.json`, but the Rust listing
      derives the file's mtime with `duration.as_millis()` — i.e. truncation. Any
      seed whose sub-ms fraction was >= 0.5 landed +1 ms high, so the equality check
      failed and provenance fell back to "edited". Reproduced on the user's real
      manifest: all four delivered files stored exactly +1 ms over their file mtime.
    - Fix 2: the daemon truncates (`Math.trunc`) so writer and reader agree by
      construction, and `SeedManifest::matches` accepts a 1 ms delta so manifests
      already written by the rounding daemon still read as built-in (the size check
      remains the load-bearing half; a 1 ms window cannot hide a real edit).
    - Verified live (scratch server + a copy of the user's real config root and its
      rounding-era manifest): before the fix all four files reported
      `edited=true`; after, `SYSTEM.md` 0 B, `graft` 850 B, `wiki-maintainer`
      1039 B, `wiki-searcher` 714 B all report `edited=false`, and appending one byte
      to `graft/SKILL.md` flips only that file to `edited=true` while its siblings
      stay built-in.
    - Also fixed an unrelated pre-existing flake the suite exposed while gating:
      `ChatPanel > keeps composer disabled while streaming` asserted
      `toBeEnabled()` immediately after `findByLabelText`, reading a stale disabled
      input before the rAF-coalesced notify() repainted (same class as the MCP-card
      flake). Wrapped both assertions in `waitFor`; the file failed 5/5 when run
      alone and now passes 5/5, full suite 301/301 three consecutive runs.
    - Gates after the follow-ups: lib 1318, protocol 212, security 152, runtime 75,
      presentation 46, clay-agent 139 (138 pass / 1 skip), frontend 301/301, tsc
      clean, clippy -D warnings + fmt clean.
  - Follow-up 2 (2026-09-10, user: "clicking Agent settings opens another split at
    the right side; nothing loads, it says Loading and never finishes"):
    - Root cause (never loaded): the server answered `ListAgentSettingsFiles`
      correctly, but the client read loop had **no arm** for
      `ServerMessage::AgentSettingsFiles`, so the reply fell through the catch-all
      `Ok(_) => {}` in `src/client/mod.rs` and never became a
      `ClientConnectionEvent`. The webview feature lane was never reached; the page
      sat on "Loading…" forever. Every existing test passed because they all
      injected the `agentSettingsFiles` envelope synthetically — nothing exercised
      the server→client-leg. Fixed by adding
      `ClientConnectionEvent::AgentSettingsFiles { client_id, files }` plus the
      read-loop arm; the bridge already forwards unknown variants generically via
      `push_routed`.
    - Root cause (wrong surface): the page was a shell-owned aside rendered beside
      the workspace (`PackageWorkspace` + `runtime.agentSettingsOpen` /
      `agentSettingsFiles` / `subscribeAgentSettings`, toggled by a client-local
      command). That is the "another split at the right side" the user saw.
    - Design change (per user): the delivered-file page is now the coding agent's
      own **Settings tab**, fifth in the right-hand strip (Files / Memory / Context
      / Session Info / Settings), and the composer's Agent-settings gear button is
      removed. The tab rides this pane's own `DocumentSession`
      (`session.listAgentSettings()` + `session.subscribeFeatures` for the reply),
      so no shell state, no separate side panel, and no sidebar command are
      involved. Selecting a file still opens it through the normal document
      pipeline and then releases the agent surface (`coding-agent.close`) so the
      editor is what the pane shows.
    - Deleted as dead with the move: the shell-side
      `agentSettingsOpen`/`agentSettingsFiles` state, `subscribeAgentSettings`,
      `openAgentSettingsFile`/`closeAgentSettings`, the `PackageWorkspace` aside,
      and the `coding-agent.agentSettings.open|close` commands (package manifest +
      `index.js` actionTargets + `is_agent_surface_command` + the retired two
      commands' count assertion). Leaving them declared with no handler would have
      re-created exactly the silent-no-op class this follow-up exists to kill.
    - Pins: `tests/agent_settings_listing.rs` drives a **real `clay::client`
      connection** (the layer that had the hole) — boot a real `IpcServer` against a
      seeded config root, connect, send `ListAgentSettingsFiles`, assert the
      `AgentSettingsFiles` event arrives with the sorted layout and the two
      provenance states. It fails (10s timeout, no event) when the read-loop arm is
      removed. Frontend: a Settings-tab test asserts the listing is requested on the
      pane session, renders from a synthetic `agentSettingsFiles` reply, and that a
      file click opens it and releases the surface.
    - Gates: lib 1318, protocol 213, security 152, runtime 75, presentation 46,
      clay-agent 139 (138 pass / 1 skip), frontend 301/301, tsc clean, clippy
      -D warnings + fmt clean.
  - Follow-up 3 (2026-09-10, user: "In the Memory tab I can see the model list, but
    selecting one as a worker says 'No active agent session'"):
    - Root cause: `AgentHost::select_worker` (and `select_picker`, same helper) wrote
      the book and then called `publish_book_snapshot()`, which broadcast
      `unconfigured_snapshot()` — a snapshot whose whole point is `session_id: ""`,
      `entries: []`, `branch: ""`. The panel merges STATE with
      `{ ...current, ...incoming }` (`frontend/src/agent/state.ts`), so that broadcast
      overwrote the live `sessionId` and transcript with empty values. The Memory tab
      then hit its `if (!sessionId)` branch and rendered "No active agent session." —
      and so did the Context and Settings tabs, and the transcript itself was wiped.
      The session was never actually gone: the daemon half (`session.om.set` with the
      real session id) had already succeeded.
    - Root cause (second half): the connection-layer intercepts resolved the tab
      inconsistently. `TabState`/`ResumableSessions` use
      `tab_for_client(client_id).unwrap_or(client_id)`; `Select`/`SelectWorker` passed
      the bare `Option`, and the `coding-agent.profile` launch passed `None`. A bare
      `None` resolves no session, so even a tab-scoped broadcast would have found
      nothing to publish.
    - Fix: `publish_book_snapshot(tab)` now publishes the tab's own state —
      `snapshot_for(session)` when `book.session_for_root(tab, root)` already has one
      (the triple it reports already reflects the book write) — and keeps the
      session-less snapshot only for a tab that genuinely has no session yet (picking
      a provider before the first prompt), where there is nothing to wipe. All three
      call sites now pass `tab_for_client(client_id).unwrap_or(client_id)`, the tab the
      panel's own mount uses.
    - Approval gate: the requested change was "find out the issue and fix it"; no
      design decision was needed (the fix restores the already-agreed behaviour that a
      state broadcast must not destroy live session state), so no new decision-log
      entry.
    - Pins: `book_selection_broadcast_keeps_the_tab_session` (in
      `tab_workspace_tests`) mounts a session against a mock daemon, adds a transcript
      row, selects an Observation worker, and asserts the broadcast still carries
      `session_id: "s1"`, one entry, and the repo branch. Reverting the broadcaster to
      `unconfigured_snapshot` fails it with `left: ""` / `right: "s1"` — the exact
      user-visible bug.
    - Live verification: real server + real daemon against a scratch config root, tab
      bound to this repo, `TabState` → `SelectWorker{model:hyper/glm-5.3-flash}` →
      `AFTER SELECT: session=2b6a735d-… entries=0 branch=fix/coding-agent` (previously
      `session=''`).
    - Gates: lib 1319, protocol 213, security 152, runtime 75, presentation 46,
      clippy -D warnings + fmt clean.
  - Follow-up 4 (2026-09-10, user: "Resuming a past session does not seem to work …
    /resume only shows the data and timestamp of the session … it should show the
    last date and time the session was active and also the first 5 words of the user
    prompt"):
    - Root cause 1 (resume loaded nothing): `snapshot_from_load` parsed a flat
      `{role, content}` entry shape that **no daemon ever sends**. `session.load`
      answers with persisted `SessionEntry` records — `{kind, message: {role,
      content: [blocks]}, summary}` — so every entry found neither a role nor text,
      `json_text` returned `""`, and all rows were filtered out. Every resume
      restored an empty transcript, which is exactly "the session chosen does not
      load and nothing changes in the UI". The suite stayed green because the mock
      daemon in `tests/agent_protocol.rs` spoke the same fictional flat shape.
    - Root cause 2 (a resume did not survive its own first prompt): `resume_tab`
      recorded `tab_session` but not `tab_session_root`, and `session_for_root`
      *prunes* a tab whose recorded root differs from `current_root` — so the next
      `ensure_tab_session` dropped the binding and created a brand-new session,
      silently abandoning the resumed one.
    - Root cause 3 (unidentifiable rows): the picker rendered
      `item(id, &session.profile, &session.updated_at)` — the profile (empty for the
      resumable list) as the primary line and a raw UTC ISO stamp as the detail.
      `parse_resumable_sessions` was already filling `AgentSessionInfo.label`, and
      `picker_items` already preferred it; the render path never did. And nothing
      ever wrote the label at all: Prism reads `SessionEntry.label` (newest non-null
      per session), and the daemon never set it.
    - Fix 1: `snapshot_from_load` walks the real record shape and maps blocks to the
      rows the live path builds (`apply_tool_event`): text → user/assistant,
      `thinking` → thinking, `tool_call` → a tool row carrying arguments + call id,
      `tool_result` → the `"… -> output"` suffix on that same row (one row per call).
      `loaded_tool_result_text` mirrors the live `tool_output_digest` projection
      (`result.content` text blocks, else `result.value`). Image/video/file blocks
      carry no transcript text, matching the live path.
    - Fix 2: `resume_tab` also records `book.tab_session_root` from the tab registry.
    - Fix 3: the daemon stamps the session's **first** entry's `label` with the
      opening prompt's first five words via a store seam
      (`labelFirstPromptStore`, applied only to the `store:` handoff, leaving all
      read paths on the raw persistence object). Eligibility is stateless — no
      `parentId` and a user message — so a restart cannot double-stamp, and later
      prompts never re-label (the opening prompt *is* the session's identity).
      `session.resumable` additionally returns `updatedAtLabel`, a local
      `YYYY-MM-DD HH:MM`; the picker renders label over that stamp, and the Files-tab
      rows show both. New `AgentSessionInfo.updated_at_label` carries it (raw ISO
      `updated_at` unchanged). Local time is rendered daemon-side on purpose: the
      picker is server-rendered and the server has no timezone database (adding
      `chrono` for one line was not worth a dependency), while the daemon runs on the
      user's machine — the raw UTC stamp read as 20:17 for a 22:17 session.
    - Approval gate: the user asked for the fix plus a specific row format; the format
      is implemented as asked (label + last-active time). No new design decision —
      the label/timestamp contract (`SessionEntry.label`, `updatedAt`) already
      existed and was simply never populated or rendered — so no decision-log entry.
    - Pins: `resume_after_daemon_load_restores_bounded_history` (mock daemon now
      speaks the real record shape; asserts user/thinking/tool/assistant rows, the
      call row's arguments and the result suffix on the same row) — reverting
      `snapshot_from_load` fails it; `resumed_tab_keeps_its_session_on_the_next_prompt`
      (reverting the root binding fails with `left: None`);
      `session_picker_rows_show_the_label_and_the_local_stamp`; clay-agent
      `resumable rows carry the opening prompt label and a local last-active stamp`
      (reverting the stamp + `updatedAtLabel` fails it).
    - Live verification: real server + real daemon against a copy of the user's store,
      `TabState` → `ResumableSessions` (rows now carry `updatedAtLabel: "2026-09-11
      02:33"`) → `ResumeSession` → `entries=6` with correct kinds — User, Tool
      (`repo_search {"query":"repo_search"} -> clay-agent/README.md-…`), Assistant,
      User, Thinking, Assistant (previously `entries=0` for the same session).
    - Gates: lib 1321, protocol 213, security 152, runtime 75, presentation 46,
      clay-agent 140 (139 pass / 1 skip), frontend 301/301, tsc clean, clippy
      -D warnings + fmt clean.

- [x] `@` mentions — manual skill trigger + file/image attachment
  - Acceptance Criteria:
    - Functional: typing `@` in the composer opens a searchable dropdown (type-to-filter,
      keyboard selection, selection embeds into the input). Two entry kinds: (1) the
      session's active skill catalog — selecting embeds the skill into the message as an
      explicit user instruction to use that skill, and the daemon treats it as a manual
      trigger: the skill is added to the session's loaded set (the same state
      `load_skill` mutates) so the body renders from the next round without a tool
      round-trip; (2) workspace filesystem entries — selecting attaches the file to the
      user input (images as image content), the same attach path pi uses.
    - Performance: dropdown filtering is client-side over the bounded skill catalog
      (≤64) and a workspace file listing (bounded, debounced).
    - Code Quality: one dropdown component for both kinds (sectioned); reuse the
      slash-completion machinery already in `CodingAgentPanel.tsx` (completion index,
      keyboard chords).
    - Security: skill names matched against the registered catalog only; file mentions
      resolve within the workspace root server-side (no arbitrary paths from the
      webview); attached images go through existing attachment validation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md`, `references/components.md`,
        `references/tokens.md` (UI gate); design skills for the dropdown surface.
      - Existing slash completion in `CodingAgentPanel.tsx` (~600).
      - Prism `load_skill` state (`session.loadedSkills`) and prompt-parsing point in
        the daemon submit path.
    - Options Considered:
      - Instruction-text-only trigger (no loaded-set mutation): model may still skip the
        skill; loaded-set is deterministic. (Rejected.)
      - Separate `#` for files: pi uses one `@` for both — parity locked. (Rejected.)
    - Chosen Approach: extend the composer input handler: `@` opens the merged dropdown;
      on submit, the daemon parses embedded `@skill:` mentions against the catalog,
      adds them to loadedSkills, and passes an instruction line; `@file:` mentions
      attach through the existing attachment flow.
    - API Notes and Examples:
      ```ts
      // submit path (daemon): after prompt parse
      for (const name of mentionedSkills) await session.loadSkill(name); // same state as load_skill
      ```
    - Files to Create/Edit:
      - `frontend/src/coding-agent/CodingAgentPanel.tsx` (dropdown + embed), 
        `coding-agent.module.css`; `clay-agent/src/host.ts` (mention parse + trigger);
        workspace file-list RPC if none exists (bounded, server-side).
    - References: roadmap "@ mentions" bullet; pi parity behavior.
  - Test Cases to Write:
    - vitest: `@` opens dropdown with catalog + files; filter narrows; selection embeds;
      submit with a valid skill mention ⇒ loadedSkills gains it and the instruction line
      is present; unknown mention ⇒ plain text; file mention attaches.
  - Evidence (2026-09-09):
    - UI gate honored (ui.md/components.md/tokens.md re-read): the dropdown reuses the
      existing `.completions` listbox styling plus one token-only section-label class
      (`.completionSection`); no new tokens or component kinds.
    - Composer (frontend): a trailing `@token` (regex, mutually exclusive with slash
      completion) opens the merged sectioned listbox — Skills (from the catalog state)
      + Files (from `workspaceFiles` state). `@skill:x` / `@file:x` narrow to one
      section; a bare token filters both; ≤8 matches per section. Shares the
      completionIndex/dismiss state; ArrowUp/Down navigate, Escape dismisses, Tab
      embeds, click embeds. Embedding writes `@skill:<name> ` / `@file:<path> ` into
      the draft (the daemon parses the tokens); placeholder now hints "type / or @".
    - File list RPC (plan's "if none exists"): daemon `workspace.files` RPC — bounded
      walk of the session's workspace root (≤200 entries, depth ≤8, dotfiles +
      `.git`/`node_modules`/`target`/`dist`/`build` skipped), workspace-relative
      paths, absent root ⇒ empty (never a throw). New `AgentClientCommand::WorkspaceFiles`
      rides the generic agent-RPC custom event; the reply caches into agent state
      (`workspaceFiles`), fetched once per session on the first `@`.
    - Daemon trigger: `sessionPrompt` parses `@skill:<name>` / `@file:<path>` before
      streaming. Valid skill mentions: `restoreLoadedSkills` (the public LoadedSkillSet
      path — same state the load_skill tool mutates; bodies re-resolve from the
      registry, persistence rides the snapshot names-only) + the skill appends to the
      session's `mentionSkills`, which rides every subsequent run's `skills` option
      (profile config union) so the loaded body keeps rendering across turns; runs
      without mentions keep the config-provided catalog byte-identical (no override).
      Prompt rewrite: one `[skills loaded by mention: a, b]` line + tokens → plain
      backticked names. Unknown mentions and unresolvable files stay plain text
      (chat-safe). Mention names validate against the registry with the same
      toolNames-subset discipline activation applies. Ceiling: mention tokens are
      whitespace-delimited, so skill names with spaces cannot be mentioned
      (`ponytail:` comment; escape syntax if ever needed).
    - File attachment: server-side resolution — join with the workspace root, realpath
      containment (symlink escapes stay plain text), ≤256 KiB, images (png/jpg/gif/
      webp/bmp) attach as base64 `image` content blocks, other files as a fenced
      `[attached file: …]` text block; the prompt streams as a Message only when
      attachments exist (string otherwise — zero churn on the cached path).
    - Parity ledger: the two new client messages + `AgentSettingsFiles` server message
      registered in `docs/development/tauri-react-parity-ledger.json` (documentation
      coverage gate).
    - Tests: daemon `mentions.test.ts` (7: skill mention loads body + instruction line,
      unknown stays plain, tool-gated skill never loads, text file attaches, image
      block shape, symlink escape + missing stay plain, bounded listing); panel tests
      (2: merged dropdown + filter narrowing + click-embeds, Tab-select embeds).
    - Gates: cargo full run green (lib 1302, protocol 210 incl. documentation
      coverage, agent_protocol 152, security 152, js_runtime 75, presentation 46),
      clippy -D warnings clean, fmt clean; frontend 290/290 (38 files), tsc clean,
      eslint only pre-existing problems; clay-agent 137 tests / 136 pass / 0 fail /
      1 skip.

- [x] Session token meter — context occupancy with theme thresholds
  - Acceptance Criteria:
    - Functional: the status strip shows current context tokens vs the loaded model's
      ceiling, compact format (`220k/270k`), from session start (not only after the first
      usage event). Ceiling follows the active model (`model.limits.contextWindow`,
      `host.ts:2264`) and updates on model switch. Above 60% of ceiling the text uses the
      theme's warning token; above 80% the theme's error token; otherwise normal — theme
      tokens only, nothing hardcoded. Providers/models that don't report usage get a
      model-family-keyed estimation heuristic (baseline: chars-per-token by tokenizer
      class; exact table picked at implementation and recorded in code).
    - Performance: derived from existing usage state events; no extra RPCs; heuristic
      runs only when usage is unreported.
    - Code Quality: builds on the existing `contextTokens`/`contextWindow` state plumbing
      (`CodingAgentPanel.tsx:378-384`, render at :645-648/:829) — fix population timing
      rather than adding a parallel channel.
    - Security: none (display only).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md`, `references/tokens.md` (theme
        warning/error tokens — no hardcoded colors).
      - Prism usage reporting in run state; `host.ts:2264` contextWindow source.
    - Options Considered:
      - Cumulative-billing counter: different metric; context occupancy is what fits the
        ceiling format. (Rejected — recorded as possible future addition.)
    - Chosen Approach: daemon publishes contextTokens/contextWindow in the initial
      session state (before first message); frontend adds threshold classes via semantic
      tokens; heuristic module keyed by model family when usage absent.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts` (initial state population, heuristic), 
        `frontend/src/coding-agent/CodingAgentPanel.tsx`, `coding-agent.module.css`.
    - References: roadmap token-meter bullet.
  - Test Cases to Write:
    - vitest: initial state carries tokens/window; model switch updates ceiling;
      threshold classes at 59/61/79/81%; heuristic chosen when usage unreported;
      format `220k/270k`.
  - Evidence (2026-09-09):
    - Occupancy source fixed at the root: the old mapping booked
      `agent_finished.usage.inputTokens` — the RUN ACCUMULATOR (Prism sums every
      provider turn's input), which double-counts context across turns (turn 3 of a
      session books turn1+turn2+turn3 prompts). The meter's numerator is now the last
      provider round's prompt size: new `provider_turn_finished` mapping →
      `AgentWireEvent::ContextTokens` (input + cacheRead + cacheWrite; unreported usage
      falls to the catch-all, no booking), `agent_finished` no longer books occupancy,
      the book keeps the latest turn (latest wins). Client delivery rides a
      `clay.contextTokens` custom event (bounded counter, never content) — live per
      turn, no extra RPC; snapshots keep carrying `contextTokens` for fresh binds.
    - Ceiling: resolved client-side from the models inventory the picker already holds
      (`snapshot.state["models"]` + active `provider`/`model`) — present from session
      start, updates on model switch. No new protocol field; the dead
      `contextWindow`-from-state branch (never populated) removed.
    - Heuristic (recorded in code, `CodingAgentPanel.tsx`): chars-per-token by
      model-family tokenizer class — claude 3.6, gpt/o-series 4, gemini 4,
      llama/mistral/qwen/deepseek 3.8, default 4 — over total transcript characters;
      runs only while no provider usage has been reported (`# ponytail:` calibration
      table, not measurement; swap for a real tokenizer count if Clay ships one).
    - Format + tones: `compactTokens` (220000 → `220k`, 1250000 → `1.2m`, digits
      below 1k); ratio >0.8 → `.meterError` (`--clay-diagnostic-error`), >0.6 →
      `.meterWarning` (`--clay-diagnostic-warning`), else normal caption; title tooltip
      carries "Context usage: N% of ceiling". Renders in the status strip next to the
      provider/model label; falls back to the raw usage-box text when no ceiling
      resolves (inventory absent).
    - Test-infra bug fixed en route: `resetChatAgentForTests` deleted
      `globalScope.__clayChatAgent`, but the `export const chatAgent` binding kept the
      ORIGINAL instance alive — every test in a file shared one agent (the estimator
      was the first reader to observe cross-test message bleed). The singleton now
      resets in place (`resetForTests` clears messages/state/pending closures); the
      panel-test harness mock's `unsubscribe` no-op also fixed (stale subscribers
      accumulated). Removed a committed `console.log("SNAPHOOK")` debug debris in
      `state.ts` while there.
    - Tests: Rust — provider-turn mapping books input+cache occupancy (12_300 =
      10k+2k+300, output excluded), unreported usage maps to the catch-all, book keeps
      the latest turn; the ContextTokens custom-event mapping. Frontend — meter from
      initial state (`0/100k`), thresholds 59/61/79/81%, compact `220k/270k`, ceiling
      follows model switch (`220k/1m`), heuristic estimate from transcript chars
      (7200 ÷ 3.6 = `2k/270k`).
    - Gates: cargo full run green (lib 1304 incl. 2 new, protocol 210, security 152,
      js_runtime 75, presentation 46), clippy -D warnings clean, fmt clean; frontend
      292/292 (38 files), tsc clean, eslint back to the 3 pre-existing warnings /
      0 errors; clay-agent 137 tests / 0 fail.

- [x] Coding-agent fixes — git branch, effort dropdown, `/resume`
  - Acceptance Criteria:
    - Functional: (1) the real current git branch always renders in the status row
      (diagnose where `snapshot.state["branch"]` drops today — Rust `refresh_branch`
      → STATE snapshot → frontend merge; ensure read at session bind + refresh on branch
      change); (2) the reasoning-effort dropdown is visible and changeable from session
      start (effort levels from the model manifest, not gated on a first message), and a
      changed level reaches the agent from the next prompt; (3) `/resume` list entries
      show the first few words of the user's first message (label/snippet already
      returned by `sessionResumable`, `host.ts:1932-1960`) instead of id/timestamp only,
      and selecting a session actually resumes: full transcript, content, and model load
      into the live view (fix the `sessionResume` → UI rebind path; today selection is
      not visibly applied).
    - Performance: branch refresh rides existing STATE snapshots; resume loads the
      persisted session once.
    - Code Quality: no parallel state channels; each fix lands in the existing pipe.
    - Security: session listing stays server-scoped by workspace root (fail-closed,
      unchanged); no new webview-driven inputs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md`, `references/components.md`,
        `references/tokens.md` (UI gate for status-row/panel changes).
      - `src/server/agent.rs` branch plumbing (:180, :235-257, :1126, :1229-1234);
        `CodingAgentPanel.tsx` (:329-332 branch, :817-835 status row, effort dropdown
        ~:832, :517 `/resume`, :1050-1070 session list); `host.ts:1640` sessionResume.
    - Options Considered:
      - Frontend-only branch read (git in webview): violates server-authority split.
        (Rejected.)
    - Chosen Approach: per-fix root-cause repair inside the existing state pipes; resume
      verified end-to-end with a persisted session fixture.
    - Files to Create/Edit:
      - `src/server/agent.rs`, `clay-agent/src/host.ts`, `frontend/src/coding-agent/CodingAgentPanel.tsx`.
    - References: roadmap "Coding-agent fixes (user-reported)" bullet.
  - Test Cases to Write:
    - Rust/vitest/panel: branch present at session bind and after change; effort dropdown
      active pre-first-message and level applied to next prompt (inspect prompt params);
      resume list shows first-message words; resume loads transcript + model (snapshot
      equality with persisted session).
  - Evidence (2026-09-10):
    - Git branch (root cause): the session-CREATION path in `ensure_tab_session` never
      recorded `book.session_root` nor called `refresh_branch` — a fresh session had no
      branch until a later rebind, and the run-settle refresh (which reads
      `session_root`) silently no-oped. The creation path now records the root and
      reads the branch (cached per run generation, re-read across runs — the settle
      refresh picks up tool-driven checkouts). Bind reads ride the prompt/picker
      snapshots that already serialize `book.branches`; no new channel.
    - Effort dropdown (root cause): `effortLevels` came from `snapshot.state["effortLevels"]`
      (`book.model_levels`), which is only populated inside `inventory_rich` during
      session CREATION — the dropdown was structurally hidden until the first prompt.
      The models inventory the panel already holds at mount (`snapshot.state["models"]`)
      carries per-model `thinkingLevels` (plan 109 I4) — levels now resolve client-side
      from the active `provider`/`model` (same pattern as the token-meter ceiling):
      visible from session start, updates on model switch. The changed level still rides
      the next prompt through the existing `pendingEffort` → `sendPrompt` →
      `Prompt{thinking_level}` pipe (unchanged).
    - `/resume` (two root causes): (a) the panel/Chat resume sent `resumeSession`, whose
      daemon reply carries NO transcript entries and whose dispatch path had no tab —
      no rebind, no visible effect. (b) Even the picker's rich path (`resume_tab`) was
      sabotaged by its own trailing `dispatch(ResumeSession)`: the spawned broadcast of
      an entry-less snapshot raced and clobbered the rich load snapshot's restored
      transcript. Fixes: `resume_tab` no longer fires the clobbering dispatch (the
      daemon creates the live session lazily at the next prompt); the connection layer
      now tab-resolves `ResumeSession` and broadcasts the rich load snapshot (full
      transcript + trio + tab rebind) through the relay. The FilesTab recent-sessions
      list switched from the unlabeled `session.list` inventory to the labeled,
      workspace-scoped `session.resumable` data (new `AgentClientCommand::ResumableSessions`,
      root from the tab registry — never webview input; empty list fail-closed) riding
      the generic `session.resumable` agent-RPC custom event; rows render the first
      words of the first user message (empty label → id prefix), and selection resumes
      through the same rich path. The `/resume` slash command + picker already used
      labels and stay as-is.
    - Latent wire-contract bug fixed en route: unit `AgentClientCommand` variants only
      deserialize from the BARE STRING (`"listSessions"`); the `{ listSessions: {} }`
      map form the webviews sent fails serde with "invalid type: map, expected unit" —
      every existing `listSessions` request died silently. `agentCommandPayload` now
      accepts the bare-string form and both panels' `listSessions` sends are fixed;
      the contract is pinned by a protocol test (map form stays rejected).
    - Tests: Rust — `broadcast_delivers_to_subscribed_views` (the panel reply lane),
      `agent_unit_commands_deserialize_from_the_bare_string_only` (wire contract).
      Frontend — effort dropdown renders pre-first-message from the inventory;
      recent-sessions rows show labels (empty label → id prefix), the mount fetch fires,
      and selection sends `resumeSession`; existing chord/dropdown tests moved to the
      inventory source.
    - Gates: cargo full run green (lib 1306, protocol 211, security 152, js_runtime 75,
      presentation 46), clippy -D warnings clean, fmt clean; frontend 294/294 (38
      files), tsc clean.
  - Follow-up (2026-09-10, user-reported "git branch still not shown"):
    - Root cause: NO agent session was ever created, so there was no branch to show
      (and no working turn either). `AgentMcpAllowListEntry::to_json` (agent.rs) wrote
      `"cwd": null` / `"timeoutMs": null` for every entry that left the optional unset
      — which is every repo-root `.mcp.json` entry — and the daemon's `parseEntry`
      (clay-agent/src/mcp.ts) rejected a literal `null` as an invalid value. The whole
      allow-list failed validation, `session.new` returned
      `agent.error: mcpAllowList[0].cwd must be an absolute path`, and
      `ensure_tab_session` returned `None` (unconfigured snapshot: empty session, no
      branch). Reproduced live against a real server + daemon binding a tab to
      `/home/arn/Projects/clay` (whose `.mcp.json` declares `graft`).
    - Fix: `to_json` OMITS absent optionals instead of emitting `null` (the daemon
      contract is "absent = default"); the daemon now also reads `null` as absent, so a
      single malformed field can never fail every session again.
    - Regression pins: `fresh_tab_session_records_the_workspace_branch` (mock daemon,
      tab-registry root, asserts `book.branches[session]` + `snapshot.branch`),
      `initialize_handshake_carries_the_built_mcp_allow_list` extended to assert the
      wire JSON omits `cwd`/`timeoutMs`, and the daemon test
      "validation: null cwd/timeoutMs read as absent, not as invalid values".
    - Verified live: the same `chat.submit` SduiAction the panel sends now answers
      `SNAPSHOT session=<uuid> branch="fix/coding-agent" provider=hyper model=glm-5.3-flash`.
    - Gates after the follow-up: lib 1315, protocol 212, security 152, runtime 75,
      presentation 46, clay-agent 138 pass/1 skip, clippy + fmt clean.
  - Follow-up 2 (2026-09-10, user-reported "still cannot see the git branch, and no MCP
    servers connected either" — after rebuilding with `scripts/build.sh run`):
    - Root cause: **nothing emitted a STATE snapshot when the coding-agent pane opens.**
      The panel's mount traffic was `listSessions` only (→ `AgentServerMessage::Inventory`),
      and the first `Snapshot` arrived only after a prompt. So at pane open the status row
      rendered `git —` and the skills/MCP cards were empty — no session, no branch, and the
      daemon had never activated capabilities (MCP connects on the first *coding session*,
      `ensureCapabilities` in host.ts:973, so `environment.list` reported no MCP outcomes).
      Reproduced on the real wire: `listSessions` → `saw_snapshot=false`.
    - Fix: new tab-resolved `AgentClientCommand::TabState` (bare-string wire form
      `"tabState"`, served in the connection layer like `ResumableSessions`) that starts
      the pane's session (`ensure_tab_session` → workspace skill discovery, MCP connect,
      graft default-bind, branch refresh) and broadcasts the resulting
      `AgentServerMessage::Snapshot`; with no provider/model selected yet it falls back to
      an unconfigured snapshot carrying the tab's workspace branch (read from the tab
      registry, never webview input). `CodingAgentPanel` sends `tabState` at mount next to
      `listSessions`.
    - Verified live against a real server + real daemon (scratch config root, book
      selection, repo bound as the tab root) — the mount now answers in one shot:
      `SNAPSHOT {sessionId: <uuid>, branch: "fix/coding-agent", mcpServers: [{serverId:
      "graft", connected: true, tools: 6}], skills: [find-docs, clay-execution, create-plan,
      …13 workspace skills], commands: 10, effortLevels: [low, high, max]}`. The graft MCP
      server itself was separately confirmed healthy through the daemon bridge
      (`connectMcpServers([graft mcp])` → `[{serverId: "graft", connected: true, tools: 6}]`).
    - Regression pins: `tab_state_snapshot_starts_the_session_and_carries_branch_and_environment`
      and `tab_state_snapshot_reports_the_branch_without_a_configured_provider` (agent.rs),
      the `"tabState"` bare-string contract in `agent_unit_commands_deserialize_from_the_bare_string_only`
      (codec.rs), and the frontend mount test "requests the tab's state at mount so branch +
      MCP are known before the first prompt".
    - Gates after follow-up 2: lib 1317, protocol 212, security 152, runtime 75,
      presentation 46, clay-agent 139 (138 pass / 1 skip), frontend 300, tsc clean,
      clippy -D warnings + fmt clean.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: review the phase's new surfaces; expose as Clay JS APIs only what is
      genuinely programmatic (candidate: settings-page file listing stays internal;
      `knowledge.setOptions` is an RPC, not init.js API — verify no new init.js API is
      implied; document if any facade is promoted). Rust public functions introduced or
      changed get explicit `deno_core` op wrappers + facades or are made `pub(crate)`.
    - Performance: no hot-path ops added.
    - Code Quality: dotted-ID naming (`<domain>.<name>`, no `clay.*` prefix); docs for
      any new API (stable ID, name, bindings, usage, errors, permissions, backing path);
      master index + generated registry updated; `cargo test` fails on missing/stale docs.
    - Security: no configuration API implicitly grants filesystem/network/shell/agent
      authority beyond the documented surfaces.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md` (naming, boundary, schema).
      - `docs/reference/clay-js-api/` existing agent docs (skill-register,
        profile-register — already updated in the prior session).
    - Options Considered:
      - Expose skills.json knobs as init.js APIs: config-file surface is the locked
        design; init.js duplication rejected.
    - Chosen Approach: verification task; add docs only where a public surface actually
      changed.
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/index.md`, generated registry (as needed).
    - References: decision-logs 2026-05-08-1509, 2026-05-08-1840.
  - Test Cases to Write:
    - Existing doc-registry gates (cargo test) stay green; add cases for any new API doc.
  - Evidence (2026-09-10):
    - Verification outcome: NO new Clay JS API is implied by this phase — every new
      surface is configuration (skills.json, mcp.json, SYSTEM.md, seeded SKILL.md
      files — covered by the configuration-API task), the daemon RPC plane
      (`knowledge.setOptions` unchanged; new `workspace.files` / `session.resumable`
      agent-RPCs ride the existing validated dispatch, not init.js), or SDUI/
      shell-client surfaces (skills/MCP cards, agent settings page, @ mentions, token
      meter). The settings-page file listing stays internal per the plan's candidate:
      it is protocol plumbing for one UI, not a programmatic surface. No facade was
      promoted, so no API inventory/docs/registry changes are needed.
    - Boundary audit of Rust functions introduced or changed this phase (rule:
      explicit op wrapper + facade + docs, or private/pub(crate)):
      - Tightened to `pub(crate)`: `AgentHostConfig::for_server` + `AgentHost::for_server`
        (config constructors; only crate-internal caller src/server/mod.rs),
        `AgentHost::broadcast` + `AgentHost::resumable_for_tab` (connection-layer relay
        plumbing for the panel resume/list path), `build_mcp_allow_list` (config
        parser; only callers are AgentHostConfig::for_server and src/server/mod.rs).
      - Already internal: `context_tokens` (private fn), `agent_settings.rs` fns
        (pub(crate) at introduction), protocol additions (message variants + the
        `AgentSettingsFileInfo` / `AgentMcpServerInfo` structs — wire types, not
        callables). No hot-path ops added.
    - Test adjustment: the `initialize_handshake_carries_the_built_mcp_allow_list`
      integration test imported `build_mcp_allow_list` directly; with the tighter
      visibility it now hand-builds one `AgentMcpAllowListEntry` — its actual purpose
      (the allow list rides the initialize handshake) is unchanged, and parser
      behavior stays covered by the agent_mcp_config unit tests (10 tests).
    - Doc-registry gates: `clay_js_api_inventory`, `clay_js_doc_registry`,
      `clay_js_facade_layout`, and the `update-doc-registry` freshness checks all
      green — consistent with a no-new-API verification. Note: there is no automated
      scan that fails on undocumented `pub` fns today (the inventory tests validate
      documented entries); the manual audit above is the enforcement path for this
      task, and the tightened visibilities shrink the surface future scans would flag.
    - Gates: cargo full run green (lib 1306, protocol 210, security 152, js_runtime
      75, presentation 46, main 9), clippy -D warnings clean, fmt clean.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: every behavior-changing setting introduced by this phase
      (skills.json roots/agentSkills, mcp.json servers, SYSTEM.md/skill file contents) is
      documented as configuration with file path, schema, defaults, and restart/session
      semantics; `~/.config/clay/init.js` remains the entry point for JS-level config and
      is not bypassed by hidden options.
    - Performance: n/a.
    - Code Quality: docs linked from `docs/index.md`; option names/types/defaults match
      the validated parsers (cross-check against the daemon loaders, not prose).
    - Security: config grants catalog/prompt text and MCP connections only — never
      implicit filesystem, network, shell, or agent-mutation authority beyond the
      documented, user-owned files.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md`.
      - `examples/config/README.md`, `examples/config/agent/tool-caps.json` (pattern).
    - Options Considered:
      - Document schemas only in roadmap: not user-discoverable. (Rejected.)
    - Chosen Approach: extend `examples/config/README.md` + reference docs for the two
      new config files and the seeded files.
    - Files to Create/Edit:
      - `examples/config/README.md`, `docs/reference/**` (configuration section).
    - References: decision-log 2026-05-08-1841.
  - Test Cases to Write:
    - Doc cross-check test (if a gate exists for config docs) or manual verification
      recorded in evidence.
  - Evidence (2026-09-10):
    - Cross-check performed against the loaders, not prose: skills.json against
      `loadSkillsConfig` (host.ts — nested `roots.{workspace,configRoot,home.{enabled,path}}`
      booleans, home path tilde/absolute only with relative rejected fail-closed (root
      disabled), unknown keys warned, absent/malformed file = all roots on with a stderr
      note, ≤64 skills per root) and mcp.json against `build_mcp_allow_list`
      (agent_mcp_config.rs — user `{"servers": ...}` wins collisions, repo `.mcp.json`
      `{"mcpServers": ...}` ignores `cwd`/`timeoutMs`, bare names PATH-resolved,
      relative paths rejected, explicit `env` pairs only, ≤32 servers / ≤64 args,
      malformed file warns and contributes nothing) and the daemon validator
      (mcp.ts — `timeoutMs` positive integer ≤ 30 min, default 60 s).
    - README precision fixes from the cross-check (examples/config/README.md): `env`
      documented as explicit name→value pairs (nothing inherited); repo `.mcp.json`
      documented as ignoring `cwd`/`timeoutMs` (was "same shape"); timeoutMs bounds
      (60 s default / 30 min hard ceiling) and the 64-args cap added; restart/session
      semantics stated per surface (skills.json + skill files = next daemon start;
      SYSTEM.md = re-read at each session build, next session; mcp.json allow-list =
      next Clay launch). `init.js` documented as the only JS-level entry point — the
      agent files are declarative JSON/markdown with no JS knobs, so nothing bypasses
      it. 256 KiB skill-file cap and per-root bound noted.
    - Discoverability: `docs/index.md` now links the canonical example README from the
      Documentation Contract section (next to the configuration-system reference), and
      `docs/reference/clay-js-api/configuration.md`'s entry-point section points at
      `agents/coding-agent/` as the file-backed agent-config reference. No init.js API
      docs changed: the locked design is config files, not JS APIs (init.js duplication
      rejected in the Options Considered above), so the API inventory/registry is
      correctly untouched and its gates stay green.
    - Cross-check tests added: shipped `examples/config/agents/coding-agent/mcp.json`
      parses under the real `build_mcp_allow_list` (Rust unit test, zero servers), and
      the shipped skills.json boots the daemon cleanly via a host test pointing
      `agentConfigRoot` at the example dir (clay-agent suite, 138 tests / 137 pass /
      0 fail / 1 skip). The example files a user copies are now pinned to the schemas
      the loaders accept.
    - Gates: cargo full run green (lib 1307, protocol 210, security 152, js_runtime
      75, presentation 46, main 9), clippy -D warnings clean, fmt clean.

- [x] Update the canonical example configuration (examples/config/)
  - Acceptance Criteria:
    - Functional: `examples/config/agents/coding-agent/skills.json` and `examples/config/agents/coding-agent/mcp.json`
      starters ship, comprehensive (every documented option, annotated, defaults active);
      `examples/config/README.md` documents all three agent files (tool-caps, skills,
      mcp) and the seeded skill files + SYSTEM.md; `examples/config/init.js` gains any
      newly bindable surface (expected: none — verify).
    - Performance: n/a.
    - Code Quality: `node --check examples/config/init.js` passes; active lines safe to
      copy verbatim; heavy setups commented.
    - Security: starters contain no real credentials; mcp.json examples use clearly
      sample server ids.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/` current contents; tool-caps.json starter style.
    - Options Considered:
      - Ship starters only in docs: drift risk. (Rejected.)
    - Chosen Approach: mirror the tool-caps.json starter style for both new files.
    - Files to Create/Edit:
      - `examples/config/agents/coding-agent/skills.json`, `examples/config/agents/coding-agent/mcp.json`,
        `examples/config/README.md`, `examples/config/init.js` (verify only).
    - References: user instruction 2026-08-03 (canonical example config duty).
  - Test Cases to Write:
    - Starter files parse (serde/node check) and match the validated schemas.
  - Evidence (2026-09-10):
    - Starters verified against the criteria: `agents/coding-agent/skills.json` is the
      comprehensive starter — every documented option present and defaults active
      (`roots.workspace/configRoot/home.{enabled,path}` + the `agentSkills` trio), safe
      to copy verbatim. `agents/coding-agent/mcp.json` ships inert
      (`{"servers": {}}`) — a live sample entry would connect (and error) at launch,
      so the full option shape (command/args/env/cwd/timeoutMs, sample server-id
      conventions) is demonstrated in the README, mirroring the bare-file + README
      annotation style of `agent/tool-caps.json`. No credentials anywhere; both files
      parse (JSON check) and are pinned to the real schemas by the cross-check tests
      added in the configuration-API task (Rust `build_mcp_allow_list` parse of the
      shipped mcp.json; daemon boot test pointing `agentConfigRoot` at the shipped
      skills.json — both green).
    - README covers all three agent files (tool-caps.json section + the
      `agents/coding-agent/` section for skills.json, skills/<name>/SKILL.md seeds,
      SYSTEM.md, mcp.json) — updated in the configuration-API task with schema,
      defaults, and restart/session semantics cross-checked against the loaders.
    - `examples/config/init.js`: verified no new bindable surface exists (the phase's
      configuration is file-backed, not JS APIs — see the JS-API verification task),
      and fixed the one stale comment that still described MCP servers as
      "server-allow-listed (data_dir/mcp.json)" — it now points at
      `agents/coding-agent/mcp.json` plus the repo-root `.mcp.json`. `node --check`
      passes; no other stale references in examples/ or docs/ (grep clean).
    - Gates: the two cross-check tests green (cargo lib, clay-agent 138 tests / 137
      pass / 0 fail / 1 skip); `node --check` clean.

- [x] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: copy `examples/config/` (including `agents/coding-agent/skills.json`,
      `agents/coding-agent/mcp.json`, and packages) to an isolated scratch config root (temp
      HOME/`.config/clay`); launch a real Linux GUI build; verify healthy startup
      (Connected, config generation commits without diagnostics, shell interaction) and
      exercise this phase's surfaces as loaded from the example: skills discovered from
      all enabled roots, seeded agent skill files + SYSTEM.md present, settings page
      opens/edits/saves, MCP server from the example config connects (sample stdio
      server or recorded blocker), token meter and branch render, `/resume` works.
    - Performance: startup time recorded; no regression vs baseline.
    - Code Quality: launch command, scratch path, and observations recorded in task
      evidence.
    - Security: never launched against the developer's real profile; scratch root torn
      down.
  - Approach:
    - Documentation Reviewed:
      - clay.md live launch-test requirements (2026-09-07 instruction).
    - Options Considered:
      - Server-only check: acceptable fallback only if GUI launch is blocked, with the
        blocker recorded and interactive acceptance left unresolved.
    - Chosen Approach: full GUI launch-test with a scratch profile.
    - Files to Create/Edit: none (evidence in task).
    - References: plan 109 review precedent.
  - Test Cases to Write: manual checklist in evidence.
  - Evidence (2026-09-10):
    - Launch path: isolated scratch root `/tmp/clay-launch-home/.config/clay` (canonical
      example copied verbatim, plus a sample MCP server entry + a scratch workspace with
      one repo skill), real Linux GUI (`clay server /run/user/1000/clay-launch.sock
      --configuration-root <scratch>` + `clay-desktop <endpoint>`), never the developer's
      real profile. Startup healthy: status bar "Workspace · Connected", config generation
      clean (the `npm list` package-store warning is pre-existing and appears with the
      real config too), no InvalidRequest wire errors after rebuilding the frontend dist
      + desktop binary (the first launch surfaced the stale-bundle `listSessions:{}`
      error — fixed earlier in the plan — confirming the fix end-to-end).
    - Two real fixes the launch test surfaced:
      1. `src-tauri/src/bridge/session.rs` `stamp_client_id` match was non-exhaustive —
         the new `ListAgentSettingsFiles` / `OpenAgentSettingsFile` variants failed the
         desktop build (the server suite alone doesn't compile the Tauri crate). Stamped
         like the other client-id-carrying messages.
      2. The daemon's per-agent config root ignored the server's configuration root —
         with `env_clear()` and no inherit list, `homedir()` resolved the REAL home
         (config-root leak) and MCP bare commands couldn't PATH-resolve. Fixes:
         `for_server` now (a) passes `--agent-config-root <root>/agents/coding-agent` to
         the daemon (parsed in `main.ts`, overriding the homedir default only when the
         server has a root), and (b) inherits exactly `HOME`/`USERPROFILE`/`PATH` for
         the daemon spawn. Verified live: the daemon seeded + reads the SCRATCH config
         root (`agents/coding-agent/skills/{graft,wiki-searcher,wiki-maintainer}/SKILL.md`,
         `SYSTEM.md`, `.seed-manifest.json`, `agent/sessions.sqlite` + vault in scratch),
         and no `skills.json absent` warning (the scratch skills.json parsed).
    - Skills + MCP verified through the real daemon code against the exact scratch
      config values (the daemon-side half of the GUI surfaces): skills discovered from
      all enabled roots (scratch workspace `demo-skill` via the workspace root;
      `find-docs` via the home root), and the sample MCP server connects —
      `environment.list` → `mcpServers: [{serverId: "fixture", connected: true,
      tools: 2}]` — the exact outcome shape the MCP card and composer section render.
      The daemon's fail-closed validation rejected the bare `node` command until the
      server canonicalizes it — confirming the locked two-layer design (server resolves
      PATH canonically; daemon only accepts absolute paths).
    - RECORDED BLOCKER — interactive GUI pass: the desktop runs on Wayland and this
      environment cannot inject input (no sudo for `/dev/uinput`; `ydotoold` cannot
      open uinput; `wtype` is keyboard-only and the computer-use backend insists on a
      ydotool socket). The XDG Remote Desktop portal consent toggle ("Allow Remote
      Interaction") requires a human click by design. Observability works (screenshots +
      AT-SPI tree: window healthy, Connected); what remains unresolved is clicking
      through the coding-agent surface (skills/MCP cards on screen), the settings page
      open/edit/save, `/resume` selection, and visually reading the token meter +
      branch. Server-side halves of those surfaces are verified above and by the
      frontend suite (294 tests cover card rendering, meter thresholds, effort dropdown,
      resume rows). Interactive acceptance to be completed once portal input is granted
      (or re-run on a session with input injection available).
    - Scratch root kept at `/tmp/clay-launch-home` + `/tmp/clay-launch-ws` (and the
      server/GUI left running on `/run/user/1000/clay-launch.sock`) pending the
      interactive pass; teardown is `pkill -f 'clay server|clay-desktop'` + removing
      both paths. Startup time: server ~2 s to listen, GUI connected in ~6 s from
      launch (debug build).

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: affected `test-plan/` modules identified via `test-plan/index.md`
      coverage matrix; relevant steps executed on a real Linux build with pass/fail
      recorded; new numbered steps added for every new user-visible behavior (skills
      card already covered? add MCP card/connections, settings page, @ mentions, token
      meter thresholds, wiki-init flow, graft default, /resume, branch, effort) with
      expected results, negative checks, and known ceilings.
    - Performance: n/a.
    - Code Quality: `test-plan/index.md` updated when modules/coverage change; no
      existing steps weakened.
    - Security: negative checks include gate-off behaviors (agentSkills off, malformed
      config).
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` module map.
    - Chosen Approach: extend the coding-agent module file(s) with the new steps.
    - Files to Create/Edit:
      - `test-plan/**` (coding-agent module + index).
    - References: user instruction 2026-08-04.
  - Test Cases to Write: the added steps themselves.
  - Evidence (2026-09-10):
    - `test-plan/17-coding-agent-parity.md`: added steps C24–C37 (skills card +
      root gating, MCP card/composer + merge + per-server isolation, agent
      settings page with provenance, @ mentions, token meter bands + heuristic
      fallback, /wiki-init flow, graft default-on fail-closed, labeled /resume
      with rich restore, branch at creation, effort from session start, prompt
      layers C36/C37 in the context inspector) and negative checks C-N5–C-N11
      (agentSkills off, malformed skills.json/mcp.json, relative home path,
      oversized/unreadable seed files, symlink escapes, unavailable-tool
      discovered skills, relative/over-cap MCP entries). Known ceilings updated
      (@-mention space-name grammar, meter calibration status, settings
      file-layout scope) and superseded seam notes re-dated.
    - `test-plan/index.md`: added the plan 117 execution record (automated legs
      PASS with suite counts, live launch gate PASS with the scratch-config
      isolation evidence, MCP fixture + skills discovery through the real
      daemon, interactive legs UNRESOLVED per the standing host ceiling, two
      launch-test defects found+fixed) and extended the module-map row 17.
    - Artifact: `test-plan/artifacts/117-coding-agent/launch-gate/` — README
      describing the isolated scratch-config launch + `window.png` (clean,
      window-cropped, host windows/paths excluded) showing the healthy
      `Workspace · Connected` state.
    - No existing step deleted or weakened (the plan 112 cross-reference and
      all prior steps/records intact); covers-matrix rows updated via the
      module-map row.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: real Linux GUI build exercised across changed states: skills card +
      MCP card (empty/populated, persisted after messages), settings page (list, edit,
      save), @ dropdown (open, filtered, keyboard), token meter (normal/warning/error
      bands), branch/effort/resume surfaces, narrow and wide layouts. Screenshots
      inspected and stored under a clearly named artifact path.
    - Performance: n/a.
    - Code Quality: when `computer-use-linux` is available, `get_app_state` before
      interaction; keyboard-only flow, focus visibility/order, roles/names/states,
      announcements verified; re-check after interactions.
    - Security: no secrets in screenshots (scratch profile).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md` (a11y duties).
    - Options Considered:
      - Source inspection only: never a substitute. (Rejected.)
    - Chosen Approach: screenshot + accessibility-tree pass; failures become defects or
      explicitly prioritized follow-ups.
    - Files to Create/Edit: evidence artifacts only.
    - References: decision-log 2026-08-14-0200.
  - Test Cases to Write: evidence checklist.
  - Evidence (2026-09-10, `test-plan/artifacts/117-coding-agent/a11y/`):
    - Captured + inspected (window-cropped portal screenshots, scratch profile,
      no secrets): healthy connected shell at 1280×1151 (two launches — tab
      bar, file tree, syntax-highlighted document, `Workspace · Connected`
      status bar; no clipping/overlap/contrast failures) and the 760×1151
      narrow layout (tab strip truncates gracefully, editor wraps, status bar
      intact — plan 109 narrow behavior preserved). Wide layout restored after
      the capture.
    - Automated a11y legs PASS: the frontend suite pins accessible structure
      (CodingAgentPanel tests: `getByRole("tab")` ×10, `button` ×9, `region`
      ×2, `log` ×1, aria-labels on icon-only actions; 294 tests green).
    - Live AT-SPI finding (defect candidate, recorded with reproduction):
      the WebKitGTK web content subtree is NOT bridged to AT-SPI in this
      build/session — the dump exposes only the frame + native window chrome
      (14 nodes), reproduced across two fresh launches (different PIDs, same
      shape), while plan 109 (2026-09-06) recorded a full webview subtree —
      a regression candidate, most likely environmental (WebKitGTK a11y
      enablement for this launch context). With no web node exposing actions
      and no input-synthesis backend (uinput denied, ydotoold cannot start,
      RemoteDesktop portal without pointer capability), the changed-surface
      states (skills/MCP cards, settings page, @ dropdown, token meter bands,
      branch/effort/resume) could not be driven live: their rendering + roles
      are pinned by the frontend suite and their server-side halves verified
      through the real daemon (module 17 plan 117 record). Follow-up: restore
      the webview a11y bridge, then re-run the surface walk with AT-SPI
      activation + a human click-through (plan 109 attempt-3 precedent).

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow, quality bar, and archive policy.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code. (Chosen.)
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation links for changed implementation areas.
      - `docs/wiki/**`: implementation wiki pages for changed code (coding-agent daemon
        surfaces: skill discovery/config, wiki/graft wiring, prompt layers, MCP, panel UI).
  - Test Cases to Write:
    - Manual wiki review: the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Evidence (2026-09-10):
    - `docs/wiki/modules/clay-agent.md`: source list extended (`agent_mcp_config.rs`,
      `agent_settings.rs`, `CodingAgentPanel.tsx`); Responsibilities gained four
      entries (three-root skill discovery + skills.json gating, prompt layers
      SYSTEM.md/AGENTS.md + seeded agent skill files with `.seed-manifest.json`,
      MCP configuration surface with per-server fault isolation, graft
      default-on + `/wiki-init` intercept); a new "Coding Agent panel surfaces
      (plan 117)" section documents the UI half (skills/MCP cards, @ mentions,
      token meter with the last-provider-turn source rule, effort/branch from
      session start, labeled /resume, bare-string unit-variant wire
      invariant); How It Works item 14 rewritten for the config-built allow
      list + `HOME`/`USERPROFILE`/`PATH` inherit; Spawn section documents
      `--agent-config-root` and the data-dir/config-root split with the
      launch-test leak rationale; Security documents the no-approval-gate MCP
      posture with its enforced invariants and the prompt-layer caps/containment;
      Tests updated to the current suite counts and new suites.
    - `docs/wiki/modules/agent-protocol.md`: plan 117 wire additions documented
      (AgentMcpServerInfo outcomes, context_tokens/thinking_levels,
      WorkspaceFiles, ResumableSessions, agent-settings messages, bare-string
      unit-variant invariant).
    - `docs/wiki/modules/agent-process-manager.md`: env_clear spawn now names
      the plan 117 inherit list and its rationale.
    - `docs/wiki/index.md`: clay-agent + agent-protocol blurbs extended to name
      the plan 117 surfaces (index link coverage test stays green).
    - Defect the task caught: extending `test-plan/17` with steps C24–C37 broke
      `documentation_coverage::parity_ledger_covers_every_manual_step…` — the
      parity ledger `agent.codingAgent.parity` row did not cover the new steps.
      Fixed by extending the ledger row's manual_steps with C24–C37 (hyphenated
      `C-N*` IDs are not parseable step IDs — the plan-109 negative checks were
      never ledger-covered either, so only C-steps belong in the ledger) and
      refreshing the row's `verified_automated`/`verified_manual` evidence
      strings with the plan 117 results.
    - Gates: `cargo test` green (lib 1307, protocol 210 incl. documentation_coverage
      11/11 + wiki index coverage, security 152, runtime 75, presentation 46);
      fmt + clippy -D warnings clean.

## Compromises Made

- Skill-file / skills.json edits apply on next daemon start (no live reload);
  SYSTEM.md applies on next session build. Both documented in the config README
  and wiki; live reload waits for a daemon-reload story.
- Token-meter fallback is a chars-per-token estimate keyed by model family, not
  a tokenizer; provider-reported per-turn tokens always win when present.
- MCP UI is display-only (card + composer section, no restart/connect actions);
  lifecycle stays config-file-driven for now.
- @-mention skill names cannot contain spaces (whitespace-delimited token
  grammar); `@file:` is workspace-root contained and text/image only.
- MCP `.mcp.json` repo entries ignore `cwd`/`timeoutMs` (user file supports
  both); bare commands resolve only against the server's PATH.
- Agent settings listing covers exactly `SYSTEM.md` + `skills/<name>/SKILL.md`
  (the delivered-agent layout); provenance is size+mtime, not a content hash
  (no sha2 dependency in Cargo.toml at the time — recorded in decision log).
- Discovered skills with unavailable tools are silently skipped at activation
  (never brick a session); built-in skills keep fail-closed semantics.
- Interactive GUI legs of the manual test plan remain UNRESOLVED on this host
  (input synthesis unavailable; WebKitGTK AT-SPI bridge not exposing the web
  subtree this session) — recorded in `test-plan/artifacts/117-coding-agent/`.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
  Candidates already visible: settings page as future home for more agent config; MCP
  server lifecycle controls; cumulative token/cost counter alongside context occupancy;
  per-workspace overrides for agent skill files.
- From the 2026-09-10 visual/a11y review: restore the WebKitGTK web-content AT-SPI
  bridge (webview subtree absent from dumps on this host while plan 109 saw a full
  tree) — it blocks live a11y verification and the AT-SPI-activation fallback for
  input-blocked walks; then complete the human click-through of the plan 117 surfaces
  (skills/MCP cards, settings page, @ dropdown, token meter bands).
