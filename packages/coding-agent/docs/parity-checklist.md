# pi-parity conformance checklist (@clay/coding-agent)

Numbered conformance checklist for the Phase 2 coding-agent package against
the pi-parity behavior set (plan 108 task 15). Each step is executable by a
human without reading source: it names the UI element or command, the
expected result, and — where the step is covered in CI — the automated
suite that gates it. Run the manual steps on a Linux build with at least
one real provider; the automated subset runs with the mock provider in CI.

Deep references: the package README (`packages/coding-agent/docs/index.md`),
`docs/wiki/modules/clay-agent.md` (daemon behaviors),
`test-plan/16-agent-host.md` (Phase 1 host configuration steps A1–A19),
plan 108 (binding spec and task evidence).

## Setup

```bash
cargo build                                   # server + shell
cd clay-agent && npm install && npm test      # daemon suites (mock provider)
cd frontend && npm test                       # panel suites
cargo run                                     # live build for manual steps
```

Launch with the coding-agent package loaded (first-party bundle); open the
agent surface via the `codingAgent.profile` command. Manual steps below
assume one open workspace with a scratch file.

## Parity steps

| # | Behavior | How to verify | Expected | Automated gate |
|---|----------|---------------|----------|----------------|
| P1 | Prompt → stream → tool output ordering | Submit a prompt that triggers a coding tool (e.g. "read notes.md") | Transcript boxes appear in stream order: user prompt, tool call box, tool output box, assistant text; usage row after finish. No reordering, no lost deltas on scroll | `prompt streams mock events, persists, and resumes` (host.test) |
| P2 | Steering (mid-run message) | While P1's run streams, type a follow-up instruction and submit | Composer stays enabled during the run; the steer lands as a queued user message; status shows the run continued; idle runs report a bounded `agent.idle` diagnostic instead | `command.register + dispatch a /steer-like command calls host drivers.steer` (skills-commands.test); `chat.steer` intent routing (Rust) |
| P3 | Cancel | Start a run, press cancel before completion | Stream stops, partial transcript is preserved, no zombie usage row; the session stays usable for the next prompt | `cancel aborts an in-flight mock generate` (host.test) |
| P4 | `/compact` (manual) | Type `/compact` in the composer | Slash completion offers the command; dispatch appends a compaction entry to the persisted session; an active run fails closed instead of queueing | `slash commands reach their daemon RPCs: compact, new, tree, fork, clone, checkout` + `manual compact on a mock session appends a compaction entry` |
| P5 | `/compact` auto threshold | Attach OM, set `compactAfterTokens` below the current usage, run until the threshold | Auto-compaction triggers near the threshold exactly once per window; the transcript shows the compaction entry | `compactAfterTokens override validates and reaches the OM settings provider` (compaction.test) |
| P6 | `/new` | Type `/new` | A fresh session starts in the same tab; the previous session stays resumable from the Files tab list | `slash commands reach their daemon RPCs…` (skills-commands.test) |
| P7 | Session list / resume | Open the Files tab empty state → session list; pick an earlier session | The picker lists workspace sessions with ids/labels; resuming reloads the bounded transcript history without injecting it into agent context | `session.load with entryId opens the branch through the matched entry (read-only view)` (session-search-tree.test) |
| P8 | Session delete | Delete a session from the session list | The session disappears from the list and its persisted record is removed; the current tab is unaffected if it was a different session | `session.delete` daemon RPC (manual step; scripted probe recorded 2026-09-03) |
| P9 | Provider/model switch | Use the provider/model row (status area) to switch models mid-session; submit another prompt | The switch is acknowledged, persists to session metadata, and the next run uses the new model; an unknown provider/model fails closed with a typed error | Run-scoped override + persistence (Rust `Prompt` params, `recreateSessionModel`); `prompt streams mock events, persists, and resumes` covers the resumed-config path |
| P10 | `/tree` navigation + summaries | Branch from an early entry (P7 resume → older message context), then run `/tree` | Tree renders ids and summaries; the abandoned branch keeps its bounded summary entry; the leaf re-roots at the summary | `tree drill: checkout from an early entry appends a summary and re-roots the leaf; /tree renders ids and summaries` (tree-branch-discard.test) |
| P11 | `/fork` | Type `/fork` with an entry id (or from the tree view) | Fork re-roots at the requested entry within the same session; branch summary appended | `LLM branch-summary refine lands after the preview and re-roots the leaf` (tree-branch-discard.test) |
| P12 | `/clone` | Type `/clone` | A new persisted session id appears with copied workspace metadata; the two sessions evolve independently | `fork and clone produce independent branches/sessions` (session-search-tree.test) |
| P13 | `--session` / `--fork` equivalents | Session picker = `--session` (resume last/selected at launch); "Open as fork" (`/openAsFork`) = `--fork` | Picker resume loads the selected session; open-as-fork copies to a new session id before the first prompt | `slash commands reach their daemon RPCs…` covers openSession/openSessionAsFork dispatch |
| P14 | Plan file round-trip | Invoke the `coding-agent.createPlan` skill; have it write a plan with todos; complete a todo | Plan file lands under `plans/` with Task ID/Status fields and `- [ ] [id] text` todos; a coding checkpoint artifact references the plan; re-reading parses todos | `create-plan skill run writes a plans/ doc with parseable todos and a checkpoint artifact` (skills-commands.test) |
| P15 | Composer growth | Type a long multi-line prompt in the composer | The input grows with content up to the CSS max-height cap, then scrolls; single-line prompts keep the compact height | `renders the 50/50 split with transcript, tabs, status row, and strip` (CodingAgentPanel.test) |
| P16 | Shift+Tab effort cycle | Press Shift+Tab in the composer | Cycles the exposed effort levels; with a model exposing none it is a no-op (no crash, no state change) | `keeps Shift+Tab a no-op when the model exposes no effort levels` (CodingAgentPanel.test) |
| P17 | Status row truth | Compare the status row against reality during use | Workspace path matches the open workspace; provider/model matches the active model; context usage matches the last run's structured usage; unknown git branch shows `—` rather than a wrong value | `renders the 50/50 split…` (status row content); context tokens via structured usage (Rust `Finished` event, task 9) |
| P18 | Extension strip | Load a package contributing an MCP server; open the agent surface | Active MCP servers are listed by name; with none configured the strip shows the empty-state label and no phantom entries | `mcp_servers` snapshot field (STATE_SNAPSHOT, task 8); `empty allow-list connects nothing and close is a no-op` (mcp-obscura.test) |

## Negative checks

| # | Check | Expected |
|---|-------|----------|
| N1 | Cross-workspace search invisibility | Session search from this workspace never returns hits from a session opened under a different workspace root; hits are metadata-only (never appended to the conversation or agent context) | 
| N2 | Disabled knowledge bases leave no tools | With wiki/graft disabled (default), no wiki/graft tools or skills appear in the session tool set; disabling after enable restores the exact prior state |
| N3 | Secrets absent from status/context surfaces | No API keys, tokens, or credential-shaped strings in the status row, context tab, transcript metadata, or slash completion list |
| N4 | Unknown slash command fails closed | `/notacommand` yields a bounded typed no-op — no provider turn, no partial transcript entry |
| N5 | Search hits never enter agent context | Opening a search hit loads a read-only branch view; the hit text is not injected into the next prompt's context |

## Performance observations

| # | Check | Expected |
|---|-------|----------|
| PERF-1 | Stream latency | First transcript box appears promptly after submit; no multi-second silent gaps between tool call and output boxes on the mock provider (observed < 100 ms per hop; real-provider budget: first box < 2 s) |
| PERF-2 | UI responsiveness while streaming | Composer input, slash completion, and tab switching stay immediate while the transcript streams; no dropped frames from transcript box re-render (uniform-height boxes) |

Regressions against the Phase 1 Chat budgets (shell bundle 180 kB gzip,
total 400 kB; render budgets in `test-plan/11-performance.md`) are defects.

## Recorded runs

See `test-plan/17-coding-agent-parity.md` for the dated execution records
(automated subset with the mock provider; manual steps on the Linux build).
