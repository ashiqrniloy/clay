---
date: 2026-09-09 13:41
status: approved
decision_about: "MCP stdio config sources connect without approval gates; bare commands may PATH-resolve"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: MCP stdio servers from user and repo config connect with no approval gate; bare command names PATH-resolve

## Decision

MCP stdio servers are configured through two user-owned sources — `~/.config/clay/agents/coding-agent/mcp.json` (path per decision 2026-09-09-1420; the flat `~/.config/clay/agent/...` shown in the approval quote reflects the layout at decision time)
and repo-root `.mcp.json` — and connect **without any runtime approval gate**. Bare command
names in these files (e.g. `npx`, `uvx`) may PATH-resolve, matching Claude Code's behavior.
This amends the 1758 posture for these two sources only: package JS still never supplies
argv, and the daemon's remaining fail-closed validation stays (literal argv, explicit
environment-variable names, bounded server/tool counts).

## Context

Decision 1758 (`decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`)
designed the MCP allow-list as server-built and deferred its source ("later: config /
approved contribution data"). The interim implementation (`clay-agent/src/mcp.ts`) validates
entries fail-closed — canonical absolute executable path, literal argv, explicit env names,
no PATH search, never from package JS — but every production call site builds an empty
allow-list, so no MCP server can ever connect. The default posture for Clay's own
external-process authority (decision-log-driven, deny-by-default, approval bound to a fixed
contribution) was assumed to apply to MCP configuration too.

The user reviewed the cap/validation inventory and explicitly overrode the approval-gate
idea: MCP configuration is a user-owned surface in the same class as `init.js` and
`tool-caps.json`. Repo-root `.mcp.json` is also treated as user-owned content in this threat
model (the same reason `AGENTS.md` and workspace skills are trusted as user-placed files) —
an adversary who can write `.mcp.json` into a repo the user opens is outside the boundary
this decision defends; Claude Code accepts the same exposure.

## Approval

- Proposed by: user (no approval gate; Claude Code parity; user-responsibility posture),
  with agent analysis of Prism MCP caps and Clay validation layers.
- Approved by user: Yes
- Approval evidence: During the Phase 2.2 roadmap session (2026-09-09) the user directed:
  "MCP configuration surface supporting both ~/.config/clay/agent/mcp.json … AND repo-root
  .mcp.json, both connected WITHOUT any approval gate" and "security posture is the user's
  responsibility (they are responsible for servers they connect; Clay takes no
  responsibility, as Claude Code does not)". Locked in `roadmap.md` Phase 2.2 ("MCP over
  stdio — wire up the last mile") and the `.mcp.json` command-semantics lock.

## Alternatives Considered

1. **Approval gate on first connect per server (decision-1758-style contribution
   approval)** — rejected by user decision: adds a one-time dialog but no real boundary
   (the config file already represents the user's intent), and diverges from the Claude
   Code convention users know.
2. **Absolute-path-only commands (keep "nothing is searched")** — rejected: breaks the
   dominant `.mcp.json` ecosystem convention (`npx`, `uvx`, `bunx` launchers are bare
   names); users would hand-resolve every path, making the config brittle and non-portable.
3. **Repo `.mcp.json` rejected, user file only** — rejected: loses the shareable
   per-project server convention; the user accepted repo-file exposure explicitly.
4. **Status quo (empty allow-list, no MCP)** — rejected: MCP plumbing exists across three
   layers but can never connect anything.

## Rationale and Evidence

- Clay's MCP stack is complete except for a config source: `@arnilo/prism-mcp` bridge,
  daemon `connectAllowListedMcpServers`, Rust `AgentMcpAllowListEntry` in the `initialize`
  handshake (`src/server/agent.rs:97-135, 2060-2068`).
- Daemon validation that survives this amendment (`clay-agent/src/mcp.ts`): literal argv
  (≤64), explicit env **names** only, max 32 servers, tools surfaced prefixed
  `mcp:<serverId>:<name>`, connection failures hide tools rather than erroring.
- Prism MCP caps confirmed sane at defaults (tools/list rejects over-cap: >500 tools,
  >256KB schema, >4MB aggregate; call results truncate with a flag) — no Clay overrides.
- Per-server fault isolation is a separate locked requirement: one failing server hides
  only its own tools, never the healthy ones.
- Claude Code's `.mcp.json` convention (`mcpServers` → `command`/`args`/`env`) is the
  compatibility target; bare `command` values resolve via PATH there.

## References

- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md` — amended decision.
- `roadmap.md` Phase 2.2 — "MCP over stdio — wire up the last mile" (locked 2026-09-09).
- `clay-agent/src/mcp.ts` — surviving fail-closed validation rules.
- `src/server/agent.rs:97-135, 2060-2068` — allow-list transport (empty at all production
  call sites today).
- `.agents/skills/clay-execution/references/packages.md` — external-process authority
  pattern this decision carves an exception into.

## Consequences

- Positive: MCP servers become connectable from day one with Claude Code-compatible
  config; no approval friction; user and project server sets coexist (user file wins on
  server-id collision).
- Risks: a malicious repo can declare an `.mcp.json` whose servers exfiltrate data or run
  arbitrary commands on connect — accepted by explicit user decision, same exposure as
  Claude Code; PATH resolution means the executed binary depends on the user's
  environment.
- Scope guard: this applies **only** to the two named config files parsed by the Rust
  server. Package JS, webview input, and run-time model/tool output still never supply
  process authority; any new process-granting surface still needs its own decision log.
- Follow-up work (plan 117): `mcp.json`/`.mcp.json` parsing + validation, per-server fault
  isolation, `timeoutMs` knob, MCP UI surfaces.
- Revisit if: a connected-server compromise incident occurs, or multi-tenant/shared
  machine profiles make repo-level auto-connect unacceptable.
