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

## Known ceilings (not bugs)

- Real-provider latency (PERF-1) varies with the provider; the recorded
  budget applies to first-box latency on the mock path and the 2 s
  first-box guide on real providers.
- Shift+Tab keybind is fixed (not yet user-configurable) — plan 108 seam.
- Git branch in the status row is a seam showing `—` until wired.

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
