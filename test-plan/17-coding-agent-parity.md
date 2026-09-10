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
| C7 | Files tab: select a file in the workspace tree | Right pane Files tab hosts the document's editor view (no second tree); `Ctrl+B` toggles the left tree (Global default) | CodingAgentPanel FilesTab tests (I6) |
| C8 | Context tab: open the drawer, open an item | Seven server-authoritative categories with counts; drawer lists items; item detail shows full redacted content; Back returns | context.test + ContextTab test (I7) |
| C9 | Context across compaction: `/compact`, reopen Context tab | Compaction summary category reflects the compaction entry; counts refresh event-driven (no polling) | context.test compaction drill (I7) |
| C10 | OM tab: run observe→reflect→drop via real worker models | Activity log lists observations, reflections, drops (drops visible, not silently vanished), compaction folds; bounded to 200 rows | om.test drill (I8) |
| C11 | OM worker-model selection: pick distinct observation/reflection models | Selection persists per workspace and per session; restored on resume; clear resets; **the panel keeps its live session** — the selection broadcast carries the tab's session id, transcript and branch instead of a session-less snapshot (the Memory/Context/Settings tabs must not fall back to "No active agent session.") | om.test retention (I8); `book_selection_broadcast_keeps_the_tab_session` |
| C12 | `/resume` | Workspace-scoped picker lists this workspace's sessions (most-recent first, bounded); other workspaces absent | resume.test scoping (I9) |
| C13 | Resume restore drill: resume a session with a switched model | Both turns reload; session resumes on its persisted model; follow-up prompt continues the branch | resume.test restart drill (I9) |
| C14 | Session Info: click a chat card | Right pane auto-selects the Session Info tab with that entry's full detail (kind, content, tool/skill metadata); Back restores the prior tab; no selection shows guidance | CodingAgentPanel I10 tests |
| C15 | Branch readout: open the surface in a git repo; create a commit on another branch | Status row shows the real branch; cached within a run; refreshed on run completion; `—` outside a repo | read_git_branch/refresh_branch unit tests (R2) |
| C16 | Extension strip | Lists loaded opt-in extensions (wiki/graft); segment omitted when none report; MCP segment lists servers truthfully | parse_environment test (R3) |
| C17 | Slash completion: type `/` then `/de` | Daemon-registered commands complete from session state; `/deploy` (unregistered) never completes; `/model` + `/resume` always offered | CodingAgentPanel R1 test |
| C18 | New daemon command appears: register a command via a package | Completion lists it after the next session/attach snapshot without frontend changes | environment.list invalidation tests (R1) |
| C19 | Transcript budget feel: long session (200+ entries) | Scroll stays responsive; server snapshot bounded (200 entries / 256 KB); oversized single entries clamped | AGENT_MAX_* budgets (I5) |
| C20 | Status row truth: workspace path, provider/model, context usage, git branch, extensions | All segments match reality; no placeholder text remains | R1/R2/R3 frontend tests |

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
| C32 | In a wiki-configured workspace submit `/wiki-init` | Daemon enables the wiki binding: wiki slash commands + skills appear; re-running is idempotent; with wiki disabled in skills.json the command stays a chat-safe prompt with no residue | skills-commands wiki intercept tests; daemon dispatch arm tests |
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
