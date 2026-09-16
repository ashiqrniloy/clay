# 17 — Coding agent pi-parity (clay-agent, Phase 2)

Manual verification module for the @clay/coding-agent package's pi-parity
behavior set (plan 108 task 15). The canonical checklist lives at
`packages/coding-agent/docs/parity-checklist.md` (P1–P18, N1–N5, PERF-1/2);
this module mirrors it in manual-test-plan form with setup, expected
results, negative checks, and known ceilings.

Deep references: `packages/coding-agent/docs/index.md`,
`docs/wiki/modules/clay-agent.md`, `test-plan/16-agent-host.md` (Phase 1
host configuration steps A1–A19), plan 108.

## Setup

```bash
cargo build
cd clay-agent && npm install && npm test      # daemon suites (mock provider)
cd frontend && npm test                       # CodingAgentPanel suites
cargo run                                     # live build for manual steps
```

Open the agent surface via the `codingAgent.profile` command in a workspace
with a scratch file. Real-provider steps require a configured provider
credential (vault/keychain); the automated subset runs on the mock provider.

## Plan 109 steps (I2–I10, R1–R3)

Steps C1–C20 cover the plan 109 coding-agent behaviors (workspace binding,
model/effort controls, transcript, tabs, resume, status truth). Automated
legs cite the pinning suites; live-build steps run against
`cargo run` + `codingAgent.profile` (Command Centre entry or Command
Palette). P16/P17/P18 above are subsumed by C8–C11/C20 for plan 109.

| # | Action | Expected | Automated leg |
|---|--------|----------|---------------|
| C1 | Open two tabs with different workspace roots; open the agent surface in each | Each surface binds to its own tab's workspace root; prompts run there; switching the tab's workspace re-binds the session root | workspace-binding suites (I1) |
| C2 | Last-used model auto-load: pick `mini` in tab A, close, reopen | Surface reopens with `mini` pre-selected (book v2 per-workspace selection) | I2 suites |
| C3 | `/model` + dropdown: submit `/model`, then use the model dropdown | Both open the daemon model picker over all configured providers' models; selection persists to session metadata | model-picker suites (I3) |
| C4 | Effort control: Shift+Tab cycles declared levels; dropdown sets one | Status row shows the pending effort; run carries `thinkingLevel` (daemon applies model-aware level); no-op when the model declares none | thinking-level suites (I4) |
| C5 | Effort rebinding: `bindKey("Ctrl+M", "coding-agent.clientCycleEffort", { scope: "global" })` in init.js | PaneTree passes the bound chord; Ctrl+M cycles on the agent surface; unbound surfaces keep Shift+Tab | keybindings ops suites (I4) |
| C6 | Full transcript: prompt → tool call → tool output → assistant text → steer | Rows render in arrival order; tool rows show name + bounded redacted args/output; `load_skill` rows show the skill name; thinking rows present; steer lands as a user-kind entry | transcript-lifecycle.test (I5) |
| C7 | Files tab: run a session that reads/edits/creates files, then open the Files tab and `⏎` a row (plan 118 task 36 replaced the editor-in-inspector) | The tab lists the session's own file records, newest first, one row per path (role mark + word, basename, muted directory); `⏎` switches the tab to its workspace view at that file and the agent view stays mounted; no editor and no tree in the inspector (`Ctrl+B` toggles the left tree) | `session-files.test.ts`, `CodingAgentPanel.test.tsx` (files), `WorkspacePanes.test.tsx` (handoff) |
| C8 | Context tab: open the drawer, open an item | Seven server-authoritative categories with counts; drawer lists items; item detail shows full redacted content; Back returns | context.test + ContextTab test (I7) |
| C9 | Context across compaction: `/compact`, reopen Context tab | Compaction summary category reflects the compaction entry; counts refresh event-driven (no polling) | context.test compaction drill (I7) |
| C10 | OM tab: run observe→reflect→drop via real worker models | Activity log lists observations, reflections, drops (drops visible, not silently vanished), compaction folds; bounded to 200 rows | om.test drill (I8) |
| C11 | OM worker-model selection: pick distinct observation/reflection models | Selection persists per workspace and per session; restored on resume; clear resets; **the panel keeps its live session** — the selection broadcast carries the tab's session id, transcript and branch instead of a session-less snapshot (the Memory/Context/Settings tabs must not fall back to "No active agent session.") | om.test retention (I8); `book_selection_broadcast_keeps_the_tab_session` |
| C12 | `/resume` | Workspace-scoped picker lists this workspace's sessions (most-recent first, bounded); other workspaces absent | resume.test scoping (I9) |
| C13 | Resume restore drill: resume a session with a switched model | Both turns reload; session resumes on its persisted model; follow-up prompt continues the branch | resume.test restart drill (I9) |
| C14 | Session Info: click a transcript entry | Right pane auto-selects the Session Info tab with that entry's full detail (kind, content, tool/skill metadata); Back restores the prior tab; no selection shows guidance | CodingAgentPanel I10 tests |
| C15 | Branch readout: open the surface in a git repo; create a commit on another branch | Status row shows the real branch; cached within a run; refreshed on run completion; `—` outside a repo | read_git_branch/refresh_branch unit tests (R2) |
| C16 | Extension strip | Lists loaded opt-in extensions (wiki/graft); segment omitted when none report; MCP segment lists servers truthfully | parse_environment test (R3) |
| C17 | Slash completion: type `/` then `/de` | Daemon-registered commands complete from session state; `/deploy` (unregistered) never completes; `/model` + `/resume` always offered | CodingAgentPanel R1 test |
| C18 | New daemon command appears: register a command via a package | Completion lists it after the next session/attach snapshot without frontend changes | environment.list invalidation tests (R1) |
| C19 | Transcript budget feel: long session (200+ entries) | Scroll stays responsive; server snapshot bounded (200 entries / 256 KB); oversized single entries clamped | AGENT_MAX_* budgets (I5) |
| C20 | Status row truth: workspace path, provider/model, context usage, git branch, extensions | All segments match reality; no placeholder text remains | R1/R2/R3 frontend tests |

## Plan 118 steps (landing → agent view, 2026-09-13)

The Coding Agent is a **pane surface**, not the window landing: the window
landing is the launcher (module [01](01-launch-and-connection.md) L12a), whose
agent pane opens this view in the tab. The composition itself (measure, turns,
inspector, composer) is verified once in module
[15](15-ui-design-systems.md) UI-DS-35 — these steps only cover the launch
route and the agent-specific facts.

| # | Action | Expected |
|---|--------|----------|
| C38 | With `@clay/coding-agent` loaded, open the agent view from the landing (agent row → primary button) or via the `coding-agent.profile` command | The tab's pane switches to the agent view: header (title, model dropdown, effort control, context meter, inspector toggle), transcript (turns or the designed empty state), state strip with the status dot, composer, and the agent foot (workspace · branch · extensions · MCP). The package claims **no** empty-tab landing — with no launcher installed the empty tab stays the core `Start with a file or folder` card (module 01 L12). UNRESOLVED on hosts without input synthesis; automated: `frontend/src/shell/WorkspacePanes.test.tsx`, `packages/coding-agent` manifest test |
| C39 | Open the inspector's `Settings` tab in the agent view | The tab lists the session's delivered agent files (name, mono size, provenance badge) with its `.agents/skills/*/SKILL.md` + `SYSTEM.md` caption and a designed empty state — the Agent Settings surface lives here, not on a separate page (module [15](15-ui-design-systems.md) UI-DS-36) |
| C40 | Open the inspector's `Files` tab after reading and editing files in the session | The tab lists **every file the session has touched** (basename + directory, status marker: `M` modified, `R` read, `A` added, `D` deleted) and opens the selected file's editor view — not a second workspace tree and not a single hard-coded document |

## Plan 121 steps (Prism 0.7 cheap wins, 2026-09-16)

| # | Action | Expected |
|---|--------|----------|
| C45 | In a scratch coding workspace with wiki enabled, submit `/wiki-init`, then inspect the registered commands and skill card | Wiki binding activates once; existing wiki skills appear and `/wiki-ingest` is available; repeating `/wiki-init` is idempotent |
| C46 | After C45, submit `/wiki-ingest {"text":"External manual source.","title":"Manual text"}` and `/wiki-ingest {"path":"notes.md","title":"Manual path"}` | Each source stages under workspace `raw/ingest/<timestamp>-<slug>/` with `extract.md` and `metadata.trust: "untrusted_external"`; with `.wiki/` present, `.wiki/log.md` records `Ingested`; an active session starts the maintainer filing run |
| C47 | With Obscura unavailable, submit `/wiki-ingest {"url":"https://example.com/nohook"}` | URL ingest returns the missing-`fetchUrl` error before any fetch/CLI spawn; inline text ingest remains available |
| C48 | With the graft CLI resolvable, open the graft skill in the agent Settings/Skills view (or its command help) | The delivered skill/help names `/graft-init` and `/graft-build-deep`; graft appears as an active extension |
| C49 | Submit `/graft-init`, then `/graft-build-deep` without configuring a deep model | Init completes non-interactively with no user-level/MCP/hook/statusline wiring; deep build returns a fail-closed configuration error before spawning and does not expose a key |
| C50 | In a coding session on a windowed model (e.g. 200k window), grow a branch past the compiler's compact ratio (≈0.9 of the input cap) and submit the next prompt; then check the transcript and the agent foot | Auto-compaction fires once at that prompt boundary before the provider turn (`compaction_started` → `compaction_finished`, a local summary entry appears, no extra provider call); the session stays usable and the context meter drops. A prompt that is itself over the cap still fails closed with the attention-budget error instead of silently evicting |
| C51 | On the same windowed coding session, send two consecutive turns that end `truncated` (the compiler mutated what it could and the request still sat over the trigger ratio), then submit the next prompt; repeat with the chat profile | Two truncated turns arm compaction: the next prompt compacts even when small. Chat and limit-less models (Ollama discovery, pass-through ids) never auto-compact — no compaction entry appears |
| C52 | Call `agent.setRunOptions({ compactAfterTokens: 2000 })` and start a fresh coding session on a windowed model; send a prompt whose estimate is over that ceiling but under the compact ratio; then configure the graft deep model and run `/graft-build-deep` | The ceiling fires (live knob, not stored-and-unused). `/graft-build-deep` refuses before the deep model exists; after `knowledgeSetOptions({ graft: true, graftDeepModel: { provider, model } })` it spawns with `GRAFT_API_KEY` in the child environment only (visible in the child's env, never in argv or logs), and a key that resolves nowhere fails closed with `-32602` |

### Negative checks (plan 118)

| # | Check | Expected |
|---|-------|----------|
| C-N12 | Chat surface absence | No chat pane, chat command (`chat.*`) or chat package is offered anywhere: the removed `@clay/chat` is not in the bundled inventory, not loadable by name from the example config, and no launcher/status entry opens it |
| C-N13 | Landing ownership | The agent package never wins the empty-tab election (its contribution is `activation: pane`); a second empty-tab contribution from another package still conflicts in one-winner order |

## Steps

| # | Action | Expected |
|---|--------|----------|
| P1 | Submit a tool-triggering prompt | Ordered transcript: prompt → tool call → tool output → assistant text; usage row after finish |
| P2 | Steer during a streaming run | Composer stays enabled; steer queues as a user message; idle runs get a bounded `agent.idle` diagnostic |
| P3 | Cancel an in-flight run | Stream stops, partial transcript preserved, session usable |
| P4 | `/compact` | Compaction entry appended; active run fails closed |
| P5 | `/compact` auto threshold | Auto-compaction fires once near the configured threshold |
| P6 | `/new` | Fresh session in the same tab; previous session resumable |
| P7 | Session list / resume | Workspace sessions listed; resume reloads bounded history without context injection |
| P8 | Session delete | Deleted session leaves the list and persistence; other sessions unaffected |
| P9 | Provider/model switch | Switch persists to session metadata; next run uses the new model; unknown ids fail closed |
| P10 | `/tree` | Ids and summaries render; abandoned branch keeps its summary; leaf re-roots |
| P11 | `/fork` | Re-roots at the requested entry; summary appended |
| P12 | `/clone` | New session id with copied workspace metadata; independent evolution |
| P13 | Session picker / open-as-fork | `--session` equivalent resumes; `--fork` equivalent copies to a new session |
| P14 | Plan file round-trip | `plans/` doc with Task ID/Status and parseable todos; checkpoint artifact references it |
| P15 | Composer growth | Grows with content to the max-height cap, then scrolls |
| P16 | Shift+Tab | Cycles exposed effort levels; no-op with none |
| P17 | Status row truth | Workspace path, provider/model, context usage match reality; unknown git branch shows `—` |
| P18 | Extension strip | Active MCP servers listed; empty state otherwise |

## Negative checks

| # | Check | Expected |
|---|-------|----------|
| N1 | Cross-workspace search | No hits from other workspaces; hits metadata-only, never context |
| N2 | Disabled knowledge bases | No wiki/graft tools/skills when disabled; disable restores prior state |
| N3 | Secrets | No credential-shaped strings in status row, context tab, transcript metadata |
| N4 | Unknown slash command | Bounded typed no-op; no provider turn |
| N5 | Search-hit context | Hit opens a read-only branch view; hit text never enters prompt context |

## Negative checks (plan 109)

| # | Check | Expected |
|---|-------|----------|
| C-N1 | Cross-workspace session leakage | `/resume` in workspace A never lists workspace B sessions; empty workspace lists nothing |
| C-N2 | Unconfigured-provider filtering | Model/OM-worker dropdowns offer only configured providers; unknown ids fail closed |
| C-N3 | Redaction spot checks | Credential-shaped strings never appear in transcript digests, context item detail, OM activity, or the status row |
| C-N4 | Fail-closed effort | Submitting with an effort the model does not declare is rejected at the daemon boundary (parse fail-closed), not silently dropped |

## Known ceilings (not bugs)

- Real-provider latency (PERF-1) varies with the provider; the recorded
  budget applies to first-box latency on the mock path and the 2 s
  first-box guide on real providers.
- Shift+Tab keybind is fixed (not yet user-configurable) — plan 108 seam.
  (Plan 109 I4 added the bindKey surface; the ceiling sentence applies to
  plan 108-era builds only.)
- Git branch in the status row is a seam showing `—` until wired.
  (plan 109 R2 wired it; the ceiling sentence applies to plan 108-era
  builds only.)

## Recorded results (Linux, 2026-09-03, plan 108 task 15)

Automated subset (mock provider, CI): daemon 67/67, frontend 200/200 —
covers P1–P7, P9–P18 automated gates and N1–N5; scripted session-delete
probe recorded for P8 (list → delete → gone → others intact). Live-build
manual pass executed on the fixture surface (split layout, composer
growth, slash completion, Shift+Tab no-op, status row, strip, truncated
box full-detail view). Real-provider interactive pass (P1–P4, P9 with
live streaming) is a standing manual step pending a configured provider
credential on this host; the mock-provider automated gates pin the same
code paths.

| # | Result | Evidence |
|---|--------|----------|
| P1–P3 | PASS (automated) | host.test: prompt streams mock events/persists/resumes; cancel aborts in-flight generate; steer via host drivers |
| P4–P6 | PASS (automated) | compaction.test manual/override; skills-commands.test slash dispatch (compact/new) |
| P7, P13 | PASS (automated) | session-search-tree.test load-with-entryId; slash openSession/openSessionAsFork dispatch |
| P8 | PASS (scripted probe) | `session.delete` removes the record; siblings intact (2026-09-03) |
| P9 | PASS (automated) | Run-scoped override + persisted metadata (task 9 suites); resume path covered by prompt/persist suite |
| P10–P12 | PASS (automated) | tree-branch-discard.test: tree drill, fork/clone independence, summary re-root |
| P14 | PASS (automated) | create-plan skill run writes plans/ doc with parseable todos + checkpoint artifact |
| P15–P18 | PASS (automated + live fixture) | CodingAgentPanel.test split/composer/slash/Shift+Tab/status/strip; live fixture screenshot |
| N1–N5 | PASS (automated) | workspace-scoped search; hidden-when-disabled wiki/graft suites; secret-redaction suite; unknown-command typed no-op; read-only branch view |
| PERF-1/2 | PASS (mock) | Per-hop stream latency < 100 ms on mock path; panel interaction immediate while streaming; shell budgets unchanged (155.1 kB gzip / 400 kB cap) |

## Plan 109 execution record (Linux, 2026-09-06)

Build: `cargo build --bin clay` fresh against the plan 109 working tree
(Prism 0.5.0 pins, I1–I10 + R1–R3 complete). Automated gates: cargo fmt/
clippy clean; cargo test green (lib 1227, protocol 208); clay-agent 97
pass / 1 skip; frontend 250 pass (34 files). Live-build launch gate:
`ui-review-default` capture PASS with the Coding Agent entry point exposed
in the AT-SPI tree (`test-plan/artifacts/109-coding-agent/launch-gate/`);
interactive keyboard steps remain the documented host ceiling (no TTY
input path). Real-provider interactive runs (C3/C4/C6/C10 streaming legs)
stay a standing manual step pending a configured provider credential on
this host; the mock-provider automated suites pin the same code paths.

| Steps | Result | Evidence |
|---|--------|----------|
| C1, C2 | PASS (automated) | workspace-binding suites (I1); book v2 per-workspace model restore suites (I2) |
| C3 | PASS (automated) | model-picker suites (I3); `/model` intercept test |
| C4, C5 | PASS (automated) | thinking-level suites (I4: applyThinkingLevelForModel, fail-closed parse); keybindings ops suites (allow-list + routing); PaneTree chord passthrough (effortChordOf) |
| C6, C19 | PASS (automated) | transcript-lifecycle.test + tool row evolution/digest bounds tests (I5); AGENT_MAX_* budget tests |
| C7 | PASS (automated) | FilesTab editor-view tests (I6); Ctrl+B default in default_keymaps + bindKey docs |
| C8, C9 | PASS (automated) | context.test (7 categories, redaction, bounds, compaction) + ContextTab drawer/detail test (I7) |
| C10, C11 | PASS (automated) | om.test drill + retention (real observe→reflect→drop pipeline) (I8) |
| C12, C13 | PASS (automated) | resume.test scoping/fail-closed/restart-drill (I9) |
| C14 | PASS (automated) | Session Info auto-select/Back/guidance tests (I10) |
| C15, C16, C17, C18, C20 | PASS (automated) | read_git_branch/refresh_branch/parse_environment unit tests; snapshot environment keys test; daemon environment.list test; frontend R1/R2/R3 tests (completion source, branch readout, strip truth) |
| C-N1 | PASS (automated) | resume.test workspace scoping + empty-workspace case |
| C-N2 | PASS (automated) | model/OM dropdown filtering tests; unknown-id fail-closed suites |
| C-N3 | PASS (automated) | redaction suites (transcript digests, context items, OM activity) |
| C-N4 | PASS (automated) | parseThinkingLevel fail-closed daemon test (I4) |
| Launch gate (all live steps) | PASS | artifacts/109-coding-agent/launch-gate (AT-SPI tree + window-cropped screenshot) |
| Real-provider streaming legs | UNRESOLVED | standing manual step — no provider credential on this host; mock-provider automated gates cover the same paths (plan 108 precedent) |
| Live surface-state walk (attempt 2, 2026-09-06) | PARTIAL / BLOCKED | Launch + AT-SPI welcome-state inspection PASS; Ctrl+B toggle verified live via portal keyboard. Surface-internal states (dropdowns, transcript boxes, four tabs, /resume picker, theme variants) UNRESOLVED: portal clicks into WebKitGTK land focus-only (no activation via click/Enter/Space), window bounds drift across calls (x=8..-1205 for one window), welcome-state Coding Agent button unreachable once an editor tab opens, Ctrl+X Ctrl+P chord not delivered via portal keyboard. Blocker is synthetic-input reliability, not app a11y (full webview subtree confirmed in AT-SPI). Follow-up: AT-SPI perform_action activation + pinned window; manual click completes the matrix for a human runner. |
| Live surface-state walk (attempt 3, 2026-09-06, human-in-the-loop + AT-SPI do_action) | PASS with 3 live findings | Method: human runner drove pointer/keyboard (WebKitGTK button activation is not reachable via synthetic input), agent drove tab switching via AT-SPI `do_action(0)` (page-tab selection works; button `press` remains inert), and captured a 2 s frame loop over the interaction window. Artifacts: `artifacts/109-coding-agent/live-walk/` (18 screenshots). PASS legs: welcome dark theme (welcome-dark.png); panel default unconfigured guidance (agent-default-unconfigured.png); four-tab strip per-tab empty states with correct SELECTED tracking (files-empty/memory-empty/context-empty/session-info-empty.png); R1 composer slash completion from daemon registry — `/ /model /resume /compact /new /n` with full descriptions (slash-completion-popup.png), inline hint bar while typing `/mode` (slash-completion-model-hint-configured.png), Control Centre command list with `server-first — @clay/coding-agent@0.1.0` routing labels (control-centre-daemon-commands.png); /model provider picker with configured/not-configured labels (provider-picker*.png) and live model switching `opencode-go/grok-4.5` → `opencode-go/mimo-v2.5` reflected in the status row chip; full chronological transcript with user/thinking/assistant/usage box kinds (transcript-files-pane.png, memory-tab-workers.png); streaming state with Cancel + 'Steer the agent, or wait' composer (streaming-state-cancel.png) and usage segment `3844 in / 56 out` in the status row; Memory tab Observation/Reflection worker dropdowns 'Not set (workers off)' + helper text (memory-tab-workers.png); Session Info auto-select on card click with per-kind detail (user kind session-info-user-detail.png, assistant kind session-info-assistant-detail.png) and Back affordance; narrow 760 px layout holds the split with graceful tab-strip truncation (narrow-760-with-sdui-error.png). LIVE FINDINGS (defect candidates, evidence in live-walk/): (1) Context tab stays on 'Loading context…' indefinitely after a completed run (context-stuck-loading.png, ~6 s+ across frames 72/75) — session.context response never renders; (2) Files-tab 'Resume session' button dispatch surfaces `invalid SDUI message: UnknownActionCommand("agent.clientOpenSessionPicker")` in the status bar and no picker opens (narrow-760-with-sdui-error.png bottom bar) — the button intent path rejects where the composer `/resume` intercept is expected to work; (3) status row shows `git —` although the tab workspace is a real git repo on `feature/coding-agent` — R2 branch readout did not populate live even after a completed run. Also noted: in-header Model dropdown never renders even when configured (selection works only via the /model Command-Centre flow), consistent with the AG-UI snapshot carrying no models list; effort dropdown correctly absent while the selected models declare no thinking levels. Theme variants (core/neobrutal/glass × dark/light) remain UNRESOLVED live (would require app restart per theme; design-system fixtures cover them via automated gates). |

## Plan 113 steps (Prism 0.5.1 kernel construction, 2026-09-07)

Steps C21–C23 cover the Prism 0.5.1 adoption (plan 113): the host
`createSessionCachePolicy` stopgap from decision 1325 is deleted and
request construction is kernel-owned (decision 2026-09-07-2149).
Automated legs cite the pinning suites; the live OpenCode Go legs keep
the standing credential blocker.

| # | Action | Expected | Automated leg |
|---|--------|----------|---------------|
| C21 | OpenCode Go first prompt with **no** host request policy | `options.sessionId`/`options.cacheKey` filled by the kernel from the session id; gateway never sees a missing session key → no `MissingSessionID` | request-construction.test (sessionId/cacheKey fills without host policy); agent_protocol deny: `createSessionCachePolicy` absent from `host.ts` |
| C22 | OM-attached OpenCode Go session: worker turns (observe → reflect) | Worker generate options carry kernel-derived `om:{session.id}` correlation; no 400 from the gateway | om.test drill (`om:`-prefixed worker `options.sessionId` assert) |
| C23 | Effort still snaps/wires after `RunOptions.thinkingLevel` | Prompt-level level resolves kernel-side: family wire field stamped (`output_config.effort` / `reasoning_effort` / `thinkingLevel`), out-of-set snaps to the declared ladder, invalid input rejected fail-closed, non-reasoning models untouched | thinking-level suites (I4, now through the kernel path); request-construction.test |

## Plan 113 execution record (Linux, 2026-09-07)

Build: plan 113 working tree on Prism 0.5.1 pins (seven families exact
0.5.1). Automated gates: cargo fmt/check/clippy clean; `cargo test
--test protocol` 208 pass (incl. the new `createSessionCachePolicy`
deny and `initialize reports prism 0.5.1`); clay-agent 101 pass / 1
skip (new request-construction.test × 3, om `om:` correlation assert,
thinking-level suites green through the kernel path). Doc-registry
gates green (protocol `doc` 148, lib `doc` 107).

| Legs | Status | Evidence |
|------|--------|----------|
| C21/C22/C23 automated legs | PASS (automated) | citations in the table above |
| C21/C22 live OpenCode Go legs | UNRESOLVED | standing manual step — no provider credential on this host (same blocker as the plan 109 real-provider legs); retry after credential setup |
| C23 live effort leg (Shift+Tab / dropdown) | UNRESOLVED | same standing credential/GUI blocker; wire-path covered by the automated kernel suites |

## Plan 117 steps (skill discovery, MCP, prompt layers, settings page, @ mentions, token meter, coding-agent fixes, 2026-09-10)

Steps C24–C35 cover the plan 117 user-visible behaviors. Automated legs cite
the pinning suites; live-build legs run against `cargo run` (scratch config
root from `examples/config/`, per the launch-test setup in plan 117).

| # | Action | Expected | Automated leg |
|---|--------|----------|---------------|
| C24 | Open the agent surface on a workspace with skills in any enabled root (workspace `.agents/skills/`, agent-config `skills/`, home `~/.agents/skills/`); no message sent yet | Skills card renders at the top of the transcript listing each catalog skill (name + description); card stays pinned once messages arrive | clay-agent skills discovery suites; CodingAgentPanel SkillsCard tests |
| C25 | Toggle a root off in `agents/coding-agent/skills.json` (`roots.home.enabled: false`), restart the daemon, reopen the surface | Skills from the disabled root disappear from the card; workspace/config-root skills remain; no error surface | skills.json loader suites (root gating) |
| C26 | Put `mcp.json` in the agent config root with a stdio server (e.g. the `mcp-fixture-server.mjs` fixture), relaunch | MCP card renders next to the skills card: one row per server with connection state + tool count; composer gains an MCP section below the input listing connected servers; tools surface as `mcp:<server>:<tool>` | clay-agent mcp suites; CodingAgentPanel McpCard + composer-section tests |
| C27 | Repo-root `.mcp.json` with the same server id as the user file | User file wins on collision; the merged allow list connects the user's entry; bare command names PATH-resolve (repo file ignores `cwd`/`timeoutMs`) | `agent_mcp_config` merge suites |
| C28 | Stop one configured MCP server's binary; relaunch | Per-server isolation: the failed server shows its error in the MCP card and hides only its tools; healthy servers stay connected and usable | connectAllowListedMcpServers allSettled isolation tests |
| C29 | Open the coding agent's right-hand **Settings** tab | Lists exactly `SYSTEM.md` + `skills/<name>/SKILL.md` files with size and built-in-vs-edited provenance (untouched seeds read built-in); selecting one opens it in the pane editor (the agent surface releases, the tab does not open a side panel); edits save and provenance flips to edited; deleted seed files regenerate at next daemon start | `tests/agent_settings_listing.rs` (real `clay::client` connection); agent_settings listing/provenance/resolve suites (7); AgentSettingsPanel tests (3); CodingAgentPanel Settings-tab test |
| C30 | Type `@` in the composer | Sectioned dropdown (Skills + Files) appears; type to filter; ArrowUp/Down navigate, Tab/Enter select, Escape dismisses; selecting a skill embeds `@skill:<name>` (body loaded for the run); selecting a file embeds `@file:<path>` and attaches its content (images as image blocks) | CodingAgentPanel mention tests; mentions.test (skill load, unknown-skill passthrough, unavailable-tool skip, file attach, symlink-escape rejection) |
| C31 | Send a prompt and watch the status row token meter after the first finished turn | Meter shows `occupancy/ceiling` (e.g. `12k/270k`) from the last provider turn's prompt tokens vs the active model's context window; tone turns warning above 60% and error above 80%; ceiling follows model switches; when usage is unreported the meter estimates from transcript chars (calibration, not measurement) | CodingAgentPanel meter tests (thresholds, model switch, heuristic); clay.contextTokens state tests |
| C32 | In a wiki-configured workspace submit `/wiki-init` | Daemon enables the wiki binding: wiki slash commands + skills appear; re-running is idempotent; with wiki disabled in skills.json the command stays a prompt-only no-op with no residue | skills-commands wiki intercept tests; daemon dispatch arm tests |
| C33 | Open a coding session with the graft CLI available / absent | Graft available: graft skill registered + graft tools surface (extension strip shows the graft extension); graft absent: binding fails closed silently — no graft tools, no error surface; graft skill body still loads for guidance | graftBindAttempted/ensureGraftBound fail-closed tests; extension naming tests |
| C34 | Open `/resume` (composer intercept) and the Files-tab recent list | Both list workspace sessions with a human label (first user-message words) **and the last-active local time**, never a bare profile + raw UTC stamp; clicking a row restores the full transcript (user/thinking/tool/assistant rows), model, and leaf into the live view without an entry-less snapshot wiping it; the next prompt continues the **resumed** session rather than starting a new one | resumable label/local-stamp tests (clay-agent resume suite); `resume_after_daemon_load_restores_bounded_history` (persisted entry shape); `resumed_tab_keeps_its_session_on_the_next_prompt`; `session_picker_rows_show_the_label_and_the_local_stamp`; FilesTab labeled-row tests |
| C35 | Open a fresh session in a git workspace; check effort before any prompt | Status row shows the real branch from the first snapshot (no `—` placeholder); effort dropdown is visible and changeable pre-first-prompt (levels resolved from the models inventory), and a changed level applies from the next prompt | ensure_tab_session refresh_branch tests; effort-from-inventory tests; unit-variant wire contract test (bare-string client commands) |

### Plan 117 prompt layers (context inspector)

| # | Action | Expected |
|---|--------|----------|
| C36 | Run a prompt, open the context inspector, expand the System prompt group | The group renders the composed layers: user `SYSTEM.md` (user-owned layer), workspace `AGENTS.md` (app layer), and the profile's base instructions — never an empty group for instruction-only profiles | contextCategories system-prompt-base test; PROMPT_LAYER_LABELS tests |
| C37 | Edit `agents/coding-agent/SYSTEM.md`, start a new session | The edited text appears as the top-ranked system-prompt layer; absent file seeds as EMPTY (no prompt pollution); 64 KiB cap enforced | loadUserSystemPrompt suites; seed manifest tests |

## Negative checks (plan 117)

| # | Check | Expected |
|---|-------|----------|
| C-N5 | `agentSkills` toggled off in skills.json (e.g. `graft: false`) | The skill never registers; its dependent slash commands never appear; disk-discovered skills unaffected | skills.json agentSkills gate tests |
| C-N6 | Malformed or unreadable skills.json / mcp.json | Bounded stderr warning; defaults stay on (skills.json) or the file contributes nothing (mcp.json); startup never fails | loader warn-and-default tests |
| C-N7 | Relative home path in skills.json (`home.path: "rel"`) | Home root disabled fail-closed for the session; workspace/config roots unaffected | home-path rejection tests |
| C-N8 | Oversized or unreadable SYSTEM.md / SKILL.md seed | File skipped (SYSTEM.md) or falls back to the built-in seed (SKILL.md) with a stderr warning; the user file is never clobbered; session opens normally | MAX_PROMPT_LAYER_BYTES / MAX_AGENT_SKILL_FILE_BYTES tests |
| C-N9 | Symlink escape | Workspace AGENTS.md pointing outside the workspace root and `@file:` mentions resolving outside the root are silently excluded | realpath containment tests |
| C-N10 | Discovered skill with unavailable tools | Skill is skipped from activation (never bricks the session); built-in skills keep fail-closed semantics | toolName-subset filtering tests |
| C-N11 | MCP entry with relative command or 33rd server | Entry/server rejected fail-closed at the daemon boundary; diagnostic names the offense; other servers still connect | parseEntry / MAX_MCP_SERVERS tests |

## Known ceilings (not bugs)

- Real-provider latency (PERF-1) varies with the provider; the recorded
  budget applies to first-box latency on the mock path and the 2 s
  first-box guide on real providers.
- Shift+Tab keybind is fixed (not yet user-configurable) — plan 108 seam.
  (Plan 109 I4 added the bindKey surface; the ceiling sentence applies to
  plan 108-era builds only.)
- Git branch in the status row is a seam showing `—` until wired.
  (plan 109 R2 wired it; plan 117 extended it to fire at session
  creation — the ceiling sentence applies to plan 108-era builds only.)
- `@skill:` mentions cannot select skill names containing spaces
  (whitespace-delimited token grammar) — documented plan 117 ceiling.
- The token meter's chars-per-token estimate is a calibration fallback,
  not a measurement; provider-reported occupancy always wins when present.
- Agent settings listing covers only `SYSTEM.md` and
  `skills/<name>/SKILL.md` (the delivered-agent layout); arbitrary config
  files are not exposed.
- The Settings tab fetches on open; the listing is not cached across tab
  switches or session switches (re-requested each time the tab is entered).
- Resume rows show the opening prompt's first five words, stamped when the
  entry is written. Sessions persisted before the stamp existed read
  "Untitled session" — no backfill is attempted, and a session created by the
  pane mount but never prompted stays unlabelled until its first prompt.

## Plan 117 execution record (Linux, 2026-09-10)

Build: plan 117 working tree. Automated gates: cargo fmt/clippy clean;
cargo test green (lib 1307, protocol 210, security 152); clay-agent 138
(137 pass / 0 fail / 1 skip); frontend 294/294. Live launch-test executed
with an isolated scratch config root (canonical `examples/config/` copied
verbatim + sample MCP fixture server + scratch workspace skill), real
server + GUI on a dedicated socket — never the developer profile.

| Legs | Status | Evidence |
|------|--------|----------|
| C24/C25 automated legs (skills discovery + gating) | PASS (automated) | clay-agent skills discovery suites (3 roots, gating, toolName filtering); SkillsCard tests |
| C26/C27/C28 automated legs (MCP wiring, merge, isolation) | PASS (automated) | clay-agent mcp suites; agent_mcp_config merge/PATH-resolve suites; allSettled isolation tests |
| C29 automated legs (settings page) | PASS (automated) | 6 agent_settings suites (layout, provenance, resolve, symlink escape, per-agent root); document pipeline tests |
| C30 automated legs (@ mentions) | PASS (automated) | mentions.test × 7 + frontend mention dropdown tests |
| C31 automated legs (token meter) | PASS (automated) | meter threshold/model-switch/heuristic tests; clay.contextTokens state tests; per-turn prompt-token source pinned by tests |
| C32/C33 automated legs (wiki-init, graft default) | PASS (automated) | wiki intercept tests; graft fail-closed bind + extension-naming tests |
| C34/C35 automated legs (resume, branch, effort) | PASS (automated) | resumable label/local-stamp + parse tests; resume entry-shape + resumed-tab-retention tests; resume clobber-fix + broadcast tests; branch-at-creation tests; effort-from-inventory tests |
| C36/C37 automated legs (prompt layers) | PASS (automated) | contextCategories base-layer test; loadUserSystemPrompt + seed manifest tests |
| C-N5–C-N11 | PASS (automated) | per-check citations above (skills.json gates, malformed-config warnings, home-path rejection, size caps, realpath containment, MCP fail-closed) |
| Live launch gate (scratch config, isolated roots) | PASS | Server ~2 s to listen, GUI connected ~6 s (debug build); status bar `Workspace · Connected`; zero wire errors after the dist+binary rebuild (the stale-bundle `listSessions:{}` unit-variant error surfaced and was fixed end-to-end); scratch isolation verified on disk — daemon seeded + read `/tmp/clay-launch-home/.clay/agents/coding-agent/` (skills seeds, SYSTEM.md, `.seed-manifest.json`), never the real home; scratch skills.json parsed (no absent/defaults warning) |
| MCP fixture connect through the real daemon (C26 daemon half) | PASS | `environment.list` on a scratch-config daemon session → `mcpServers: [{serverId: "fixture", connected: true, tools: 2}]` — the exact outcome shape the MCP card renders; daemon fail-closed rejection of the bare `node` command confirmed the two-layer design (server canonicalizes PATH; daemon demands absolute) |
| Skills discovery through the real daemon (C24 daemon half) | PASS | Scratch-config daemon session catalog: workspace-root skill (`demo-skill`) + home-root skill (`find-docs`) discovered from the enabled roots |
| Interactive GUI legs (click-through of cards/settings/mentions/meter) | UNRESOLVED | Standing host ceiling (no input-synthesis path: no sudo for `/dev/uinput`, ydotoold cannot open uinput, portal consent requires a human); server-side halves verified above; card rendering pinned by the 294-test frontend suite; `test-plan/artifacts/117-coding-agent/launch-gate/` holds the launch capture |


## Plan 112 cross-reference (2026-09-07)

Coding-agent surfaces gained icon-only composer actions (Send `message.send`
submit, streaming Stop `generation.stop`, Close `action.close`) with labels +
shortcuts in tooltips, Back navigation as icon + visible text, and recent
session rows compacted to distinct `session.resume` icon actions; agent tab
labels and approval Allow/Deny keep text. Steps:
[18 — Icon packs](18-icon-packs.md) (ICON-02, ICON-08, ICON-09, executed
2026-09-07).

## Plan 118 coding-agent view record (2026-09-13)

The shipped agent view composition was adopted earlier in this plan (module
[15](15-ui-design-systems.md) UI-DS-35, with `design-artifacts/screenshots/quiet-instrument-agent/report.json`
and 34 `CodingAgentPanel` tests). This record covers the plan 118 *landing*
change and re-executes the agent module's legs that touch it. Artifacts:
`test-plan/artifacts/118-quiet-instrument-migration/`.

| Steps | Result | Evidence |
|---|---|---|
| C38 (launch route) | UNRESOLVED live / PASS structural | The live capture (`coding-agent/`) reached the empty tab but not the agent view: the pane is activated by a *package-contributed* command (`coding-agent.profile`), which is not dispatchable from the same `init.js` generation and needs keyboard/pointer input this host cannot synthesize. Fixture updated to load the package explicitly (finding 2 in module 15's record). Structural legs: `WorkspacePanes.test.tsx`, the manifest `activation: pane` test, and the DEV-harness captures recorded under UI-DS-35 |
| C39 (Settings tab) | PASS automated | `frontend/src/agent-settings/AgentSettingsPanel.test.tsx` (6) + UI-DS-36 evidence; the tab renders in the shipped inspector |
| C40 (Files tab session history) | PASS automated | `CodingAgentPanel` Files-tab tests (session files with status markers, editor view for the selected file) |
| C-N12 (chat absence) | PASS automated | `tests/package_ui_conformance.rs` + `frontend/src/test/chat-surface-absence.test.ts` pin that the removed chat surface has no package, command, recipe or entry point |
| C-N13 (landing ownership) | PASS automated | Empty-tab election tests: the agent keeps its `pane` surface and never claims the landing; the launcher's landing + the agent's pane coexist, and two landings still conflict in one-winner order |
| C1–C20, C24–C37 regression class | PASS automated | Daemon and frontend suites green on this tree (frontend 357; the agent daemon suites are unchanged by plan 118) — the landing change touches only how the view is reached |
| Real-provider streaming legs | UNRESOLVED | Standing provider-credential ceiling, unchanged |

No existing step was deleted or weakened. New steps were added rather than
duplicating module 15's visual composition checks.

## Plan 119 steps (workspace-scoped sessions, MCP connect floor, panel review)

| # | Action | Expected | Automated leg |
|---|--------|----------|---------------|
| C41 | Configure a stdio MCP fixture that waits about 1 s before `initialize`, with `timeoutMs: 200` in the agent `mcp.json`; open a coding session, then invoke its intentionally slow tool | The server becomes visible/connected instead of being hidden during its bootstrap. Its tool call still returns a bounded 200 ms timeout result; run cancellation also wins over the local deadline. The fixed 5 s handshake floor is host policy, not a user setting. | `clay-agent/src/__tests__/mcp-v2.test.ts` slow-boot and cancellation cases |
| C42 | With a configured provider (or the `CLAY_AGENT_MOCK=1` review daemon), open two tabs rooted at scratch folders A and B, each with a distinct marker file; prompt each to write in its own workspace | Tabs bind different session IDs. A can read/write only A and B only B; transcript events and files do not bleed between tabs. | `tests/agent_session_isolation.rs::two_workspaces_keep_their_agent_writes_in_their_own_root`; `frontend/src/shell/WorkspacePanes.test.tsx` |
| C43 | Restart the daemon after C42, resume both tabs, then list workspace files or submit a follow-up prompt | Each resumed session recovers its recorded root, not the daemon launch directory; A lists A's marker and B lists B's marker. A dead daemon handle is cleared so the next RPC respawns it rather than hanging. | `tests/agent_session_isolation.rs::real_daemon_serves_one_session_per_workspace` |
| C44 | Review the agent panel's empty, conversation, streaming, approval, error, and five-inspector-tab states at wide/narrow widths; run the real desktop fixture's Agent tab through AT-SPI | The panel keeps the approved Quiet Instrument composition: named Coding Agent landmark, polite Transcript log, Message entry, Send action, ARIA tab list, and keyboard-reachable approval actions. No state is silently dropped by the SC-4 component split or SC-6 tab-local store. | `CodingAgentPanel.test.tsx`, `TranscriptList.test.tsx`, `Composer.test.tsx`, `WorkspacePanes.test.tsx`, browser/AT-SPI evidence below |

### Negative check (Plan 119)

| # | Check | Expected |
|---|-------|----------|
| C-N14 | Close the only live tab for A while its session is still attempting a document/tool operation | The operation fails closed with `agent.workspace_unresolved`; it does not fall back to B, the first workspace, bootstrap root, launch directory, or active tab. Reopening A restores live-root authority through the session binding. |

## Plan 119 execution record (Linux, 2026-09-15)

Fresh build: `cargo build --bins`, `cargo build -p clay-desktop --bins`,
`clay-agent npm run build`, and `frontend npm run build` all passed. Review
artifacts: `test-plan/artifacts/119-editor-agent-remediation/live-agent/` and
`code-reviews/screenshots/2026-09-15-plan119-sc4-agent-review/`.

| Steps | Result | Evidence |
|---|---|---|
| C41 | PASS automated | Fresh `clay-agent npm test`: 149 pass / 1 skip. The slow-boot MCP fixture connected with a 200 ms call timeout, then its slow call timed out at 200 ms; cancellation remained propagated. |
| C42/C43/C-N14 | PASS real server + daemon integration | Fresh `cargo test --test security agent_session_isolation`: 2 pass. Layer A uses a real server plus scripted daemon for cross-root writes and fail-closed tab closure; Layer B uses the shipped daemon in `CLAY_AGENT_MOCK=1` mode to prove per-root listings and post-restart root recovery. |
| C44, live rest state | PASS | Fresh `scripts/capture-ui-review.sh` capture passed at a 1280×1104 measured viewport after AT-SPI selected Agent. The tree exposes `Coding Agent`, `Transcript`, `Message`, `Send`, `Agent detail`, and all five inspector tabs. The inspected portal PNG was deleted because its status bar exposed the isolated temporary review-root path; retained AT-SPI/drive/diagnostic evidence is root-redacted. The isolated harness logged unavailable optional `npm` package discovery, but registered 13 agent lines and had zero configuration-failure lines; this capture does not claim package-install coverage. |
| C44, state/keyboard matrix | PASS (D2 and F3 resolved 2026-09-15) | Browser fixture review captured wide/narrow landing, conversation, streaming, approval, error, agent settings, and every inspector tab; keyboard flow verified effort cycling, mentions, slash dispatch, approval activation, and ARIA tab roving. Follow-ups from `code-reviews/screenshots/2026-09-15-plan119-sc4-agent-review/review-log.md` are closed with evidence in `code-reviews/screenshots/2026-09-15-plan119-further-actions/review-log.md`: D2 keeps the approved artifact's scrolling strip and adds the missing thin-scrollbar affordance (measured 303/337 px at 1440/1280, `scrollbar-width: thin`), and F3 moves focus into the alertdialog and returns it on resolution (new panel test + live fixture check). F2/F4/F5/F6 stay recorded, unchanged. |
| Live two-tab panel interaction | UNRESOLVED — host input limit, recipe retained | Re-run on 2026-09-15 with a configured mock provider (seeded vault + `book.json`), two tabs rooted at distinct scratch folders, real server + real daemon + real desktop client. Live: both tabs restore with their own root (AT-SPI shows `ws-a` selected, `ws-b`, and ws-a's `MARKER-A.txt`); prompting and per-tab agent binding need keyboard input, which this host cannot synthesize (`/dev/uinput` is root-only for ydotool; the portal grant is interactive), and one live `session.new` hit the server's 30 s ceiling while the same daemon creates it in 8 ms standalone. Full record + rerun recipe: `code-reviews/screenshots/2026-09-15-plan119-further-actions/review-log.md` §4. Isolation itself stays proven by C42/C43 above. |
| Frontend regression | PASS | `frontend npm test`: 49 files / 426 tests passed with **exit 0** after the 2026-09-15 further-actions fix (fire-and-forget bridge calls now route through `frontend/src/lib/detached.ts`; the two `src/test/shell.test.tsx` unhandled `invoke` rejections are gone). `npm run lint` is clean (0 errors / 0 warnings) and `npm run check:budget` passes against the decision-logged 404 kB total ceiling. |

No existing step was weakened. C41–C44 and C-N14 add the Plan 119 user-visible and negative paths; unresolved GUI/provider limits and the two existing review findings remain visible for a future input-capable/provider-configured pass.

## Plan 121 execution record (Linux, 2026-09-16)

Build: `cargo build --bin clay` and `cargo build -p clay-desktop --bins` passed; `cd clay-agent && npm test` passed with 163/163 tests, 1 pre-existing skip. An isolated `CLAY_AGENT_MOCK=1` server plus real Clay desktop was launched from `/tmp/clay-manual-121` with the canonical example config, scratch workspace, and graft available; Obscura was intentionally absent for C47.

| Steps | Result | Evidence |
|---|---|---|
| C45–C46 | UNRESOLVED live / PASS automated | The real server and coding profile reached the launch state, but `/wiki-init` and `/wiki-ingest` could not be entered. `clay-agent/src/__tests__/wiki-knowledge.test.ts` covers binding, text/path staging, raw-layer paths, trust metadata, `.wiki/log.md`, and live-driver dispatch. |
| C47 | UNRESOLVED live / PASS automated | Live URL input was blocked. The wiki suite verifies missing Obscura/fetchUrl fails closed while text ingest remains available. |
| C48–C49 | UNRESOLVED live / PASS automated | Graft CLI availability and the seeded skill text were present in the isolated config; interactive command/help inspection was blocked. `graft-knowledge.test.ts` verifies both skill instructions, non-interactive `init`, no secret on argv, and pre-spawn deep-build refusal. |
| Real-build launch gate | PASS | Server listened on `/run/user/1000/clay.sock`; the desktop connected and the daemon registered the `coding` profile in `artifacts/clay-x11.log`; scratch config/workspace remained isolated. |
| Interactive GUI legs | BLOCKED | `computer-use-linux doctor`: AT-SPI/window discovery passed, but `can_send_development_input=false`; `/dev/uinput` is root-only (`Permission denied`), no connectable `ydotoold` socket exists, `wtype` is incompatible with this compositor, and XDG RemoteDesktop input consent requires a human. No live pass is claimed for C45–C49. |

The new steps and this exact blocker are recorded without weakening modules 16/17 or converting automated coverage into a manual pass.

## Plan 121 follow-up record (auto-compaction + graft deep model, Linux, 2026-09-16)

Build: `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo test --test protocol` 216/216; `cd clay-agent && npm test` 169 passed / 1 skip (170 total). The isolated `CLAY_AGENT_MOCK=1` launch from the earlier record was reused; interactive input remained blocked by the same host ceiling.

| Steps | Result | Evidence |
|---|---|---|
| C50–C51 | UNRESOLVED live / PASS automated | No live transcript could be driven. `clay-agent/src/__tests__/auto-compaction.test.ts` covers the arming predicate (coding + windowed only; chat and limit-less models unarmed), the compact-ratio gate compacting locally at the prompt boundary with zero provider calls, `attention_compiled` events feeding the truncation streak through `session.prompt`, and the two-truncated-turns fire; `attention-compiler.test.ts` keeps the over-budget fail-closed case. |
| C52 | UNRESOLVED live / PASS automated | `auto-compaction.test.ts` proves the `compactAfterTokens` ceiling fires under the ratio gate (the knob is live). `graft-knowledge.test.ts` proves `/graft-build-deep` refuses before a deep model exists, then spawns with the vault-resolved key only in `GRAFT_API_KEY` (argv carries provider/model/base-url, never the key), rebinds on a changed model, and fails closed (`-32602`) for a missing key, a deep model without `graft: true`, and malformed shapes; `src/server/ops/agent.rs` unit tests pin the pre-queue shape validation. |
| Interactive GUI legs | BLOCKED | Same host ceiling as the Plan 121 record above (root-only `/dev/uinput`, no connectable `ydotoold` socket, `wtype` incompatible, portal consent interactive). No live pass is claimed for C50–C52. |

## Plan 122 steps (host-owned spawn agents, 2026-09-17)

Steps C53–C55 + C-N15 cover the Prism 0.7 supervisor primitive on coding
profiles. Git-worktree isolation for children is explicitly **out of
coverage** — children share the parent workspace by design this cut.

| # | Action | Expected | Automated leg |
|---|--------|----------|---------------|
| C53 | Open a coding session, then a Chat session; inspect each session's tool set (environment/context or a provider-captured registry) | The coding session lists `spawn_agent`, `wait_agent`, `cancel_agent` alongside the coding tools; Chat lists none of the three | `clay-agent/src/__tests__/spawn-agent.test.ts` (coding vs Chat tool lists) |
| C54 | In a mock-provider coding session, prompt a sync spawn of the `test` child (e.g. via `spawn_agent {"childId":"test", ...}`) | The transcript renders the `spawn_agent` tool row plus a child lifecycle row pair (`test` started → stopped with status) from the `subagent_*` → Tool mapping; the child result text returns through the parent tool call | `spawn-agent.test.ts` (sync spawn: lifecycle rows, child result, child registry without spawn tools); `src/server/agent.rs` mapper test (`map_event_maps_subagent_lifecycle_onto_tool_rows`) |
| C55 | Spawn async, then `cancel_agent`/`session.cancel` while the child runs | The parent run abort/`session.cancel` stops the running child (a stopped lifecycle row arrives); handles are in-process only — after a daemon restart a stale `delegationId` is a plain tool error from `wait_agent`, never a cross-session resume | `spawn-agent.test.ts` (parent abort stops async child; foreign `delegationId` wait error); `clay-agent/README.md` non-durability note |

| # | Check | Expected |
|---|-------|----------|
| C-N15 | Model-supplied `childId` outside the catalog (e.g. `"evil"`), and a `wait_agent` on a fabricated `delegationId` | Both fail closed before delegation: the closed spawn schema (enum = host catalog `test`/`validation`) blocks the call at validation with no child provider turn; the foreign handle is a tool error. Model arguments can never name a child id, tool set, or identity the host did not install |

## Plan 122 execution record (Linux, 2026-09-17)

Build: `cd clay-agent && npm test` 175 passed / 1 pre-existing skip (176
total, includes the six new `spawn-agent.test.ts` cases); `cargo fmt
--check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D
warnings` clean; `cargo test --lib server::agent` 100/100 (includes the
new subagent-lifecycle mapper test and the updated unknown-drop contract).

| Steps | Result | Evidence |
|-------|--------|----------|
| C53–C55, C-N15 | PASS (automated) | `clay-agent/src/__tests__/spawn-agent.test.ts` × 6; `map_event_maps_subagent_lifecycle_onto_tool_rows`; updated `map_event_drops_unknown_event_types` (identified-id rule) |
| Live GUI legs | UNRESOLVED | Standing host input ceiling (root-only `/dev/uinput`, no `ydotoold`, interactive portal consent — same blocker as the plan 121 record). No live spawn against a real model was run; the mock-provider automated suites pin the same code paths |
| Worktree isolation | Out of coverage | Deferred by plan 122: children reuse the parent workspace root and the parent document `sessionId` |

## Plan 123 note (per-prompt OM work-scopes, 2026-09-16)

Automated-only cut; no C-step changed or weakened. Plan 123 attaches an
internal work-scope to each OM-attached coding prompt and nests a closed child
scope per delegated child run, so the C53–C55 spawn expectations above still
hold (the tool catalog, lifecycle rows, cancel path, and fail-closed handles
are untouched). Scopes live only in the daemon OM ledger — nothing new renders
in the transcript, Memory tab, or Session Info. Recall stays exact-id only.

## Manual-test remediation: the pane mount selects the coding profile (2026-09-16)

Manual test finding (the C38 route, Linux): a tab restored by `layout.json`
into the agent view — or entered through the `Ctrl+2` switcher or the
empty-tab landing — never dispatched `coding-agent.profile`, so the book's
profile stayed empty and the pane's session was created with the daemon-level
`Chat` default. The agent then ran **without the coding tools and without
MCP**: a repo `.mcp.json` declaring `graft` produced `mcpServers: []` and the
foot read `MCP none`.

Root cause proved end to end against a real server + the shipped daemon + the
repo's own `.mcp.json`: the persisted session record carried
`agent_definition_id = "Chat"`. With the profile set, the same run created a
`coding` session and its mount snapshot carried the `graft` outcome
(`connected: true`).

Fix: `AgentHost::tab_state_snapshot` (the pane's mount STATE) now fills an
*empty* book profile with `CODING_SURFACE_PROFILE_ID` (`agent:coding`) before
ensuring the tab's session; a non-empty (deliberate) profile is never
overwritten, and the launch command path is unchanged.

Known ceiling: a tab whose workspace *already* adopted a `Chat` session before
this fix keeps that live session (the book profile is per workspace, the live
session is per `(agent, root)`); the next session for that workspace — after a
daemon/pane restart or a workspace change — runs the coding profile.

| # | Action | Expected | Automated leg |
|---|--------|----------|---------------|
| C56 | Open a workspace whose stored book profile is empty, then enter the agent view (restore, switcher, or launch); inspect the session's profile and the MCP card/foot | The pane's session runs `coding`; a repo `.mcp.json` server connects and the foot lists it (`<id> · n tools`); a profile the user picked deliberately survives a later mount | `tab_state_snapshot_starts_the_session_and_carries_branch_and_environment` (profile `coding`; `custom` survives a second mount) |

| Steps | Result | Evidence |
|-------|--------|----------|
| C56 | PASS (automated) | `cargo test --lib -- server::agent::` 41/41 (mount test extended with the profile assertions); `cargo test --lib -- server::connection::` 89/89; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean. The live leg is blocked by the standing host input ceiling; the end-to-end evidence is the real server + shipped daemon reproduction above |
