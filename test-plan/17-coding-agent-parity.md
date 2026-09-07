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
| C11 | OM worker-model selection: pick distinct observation/reflection models | Selection persists per workspace and per session; restored on resume; clear resets | om.test retention (I8) |
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

## Plan 112 cross-reference (2026-09-07)

Coding-agent surfaces gained icon-only composer actions (Send `message.send`
submit, streaming Stop `generation.stop`, Close `action.close`) with labels +
shortcuts in tooltips, Back navigation as icon + visible text, and recent
session rows compacted to distinct `session.resume` icon actions; agent tab
labels and approval Allow/Deny keep text. Steps:
[18 — Icon packs](18-icon-packs.md) (ICON-02, ICON-08, ICON-09, executed
2026-09-07).
