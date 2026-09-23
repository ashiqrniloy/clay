---
date: 2026-09-02 14:40
status: approved
decision_about: "Direct adapters and Clay policy bundles for external coding agents"
proposed_by: both
explicitly_approved_by_user: true
---

# Decision: Direct external coding-agent adapters, not Prism delegation

## Decision

Clay will run Claude Code and Antigravity as direct external-agent runtimes,
not through Prism. Prism remains the native Clay runtime. Each delegated task
receives a Clay policy bundle translated to the external agent's documented
configuration, and Clay exposes only controls the chosen runtime declares as
supported.

Claude Code uses its Agent SDK for managed delegation. Antigravity uses its
documented headless CLI stream for configured delegation; this supersedes
only the `prism-antigravity-agent` consequence of
`decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md`. Its per-tool,
interactive approval remains unavailable to Clay until Antigravity publishes a
structured control protocol. An embedded PTY is the fallback for vendor-native
controls that cannot be represented safely.

## Context

The user wants Clay to delegate bounded coding tasks to external agents while
the selected agent owns its loop until completion. Clay should apply native
agent instructions, skills, tools, and acceptance policy as far as the vendor
allows, without falsely claiming equivalent controls.

Clay's existing first-party `clay-agent` daemon already uses a Clay-owned
protocol around Prism. That remains appropriate for the native agent but would
make Prism an unnecessary intermediary for vendor-owned sessions.

## Approval

- Proposed by: Both
- Approved by user: Yes
- Approval evidence: “Create two new phases in the @roadmap.md document after
  Phase 8 to bring support of Claude Code and Antigravity agents in the way
  that we just discussed with adapters and controlling external agent
  behaviour.”

## Alternatives Considered

1. **Route external agents through Prism** — rejected. It adds an adapter hop
   without increasing Claude Code or Antigravity control, and risks hiding
   vendor-specific limitations.
2. **Use MCP as the agent-control protocol** — rejected. MCP provides tools
   and resources, not agent-session lifecycle or approval control.
3. **Adopt ACP as the required transport** — rejected for now. Neither
   reviewed vendor documents a supported ACP integration suitable for this
   path; Clay may add ACP later for agents that actually implement it.
4. **Drive terminal TUIs by screen scraping** — rejected as the structured
   path. A PTY may expose native controls as a fallback, but is not a safe
   semantic control API.

## Rationale and Evidence

- Claude Code's Agent SDK exposes a custom `systemPrompt`, explicit filesystem
  setting sources, built-in-tool selection, MCP configuration, programmatic
  skills/agents, and a `canUseTool` callback that can allow, deny, or modify a
  requested action. `strictMcpConfig` prevents ambient MCP configuration from
  joining a Clay-managed task.
- Antigravity documents headless NDJSON events, persistent conversations,
  model/agent selection, and policy-based permissions. It documents custom
  agents, skills, plugins, hooks, and MCP configuration, but not an arbitrary
  per-run base-system-prompt replacement or a structured live approval
  callback. Its documented stream rejects `control_request` and
  `control_response` messages.
- Agent Client Protocol standardizes editor-to-agent communication but is not
  a required general-purpose implementation surface for either vendor today.

## References

- [Claude Code Agent SDK](https://code.claude.com/docs/en/agent-sdk/) — SDK
  host integration surface.
- [Claude Code SDK options](https://code.claude.com/docs/en/agent-sdk/typescript)
  — `systemPrompt`, `settingSources`, tool and MCP configuration.
- [Claude Code approvals](https://code.claude.com/docs/en/agent-sdk/user-input)
  — `canUseTool` pause/allow/deny/modify behavior.
- [Antigravity headless mode](https://antigravity.google/docs/cli/headless) —
  NDJSON, resumption, cancellation, and unsupported control messages.
- [Antigravity permissions](https://antigravity.google/docs/cli/permissions)
  — policy-based tool authority.
- [Agent Client Protocol](https://agentclientprotocol.com/get-started/introduction)
  — editor/agent interoperability purpose.
- `clay-agent/src/host.ts` — current first-party Prism session host.
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md` —
  native Prism-host boundary.

## Consequences

- Positive: external agents retain their official integration path while Clay
  has a truthful, consistent delegation surface.
- Positive: Clay tools can be offered to both agents through MCP, separately
  from lifecycle control.
- Risk: prompt and tool control varies by runtime; capability declarations and
  negative tests prevent a misleading UI.
- Risk: an external process runs with same-user authority. Core owns launch,
  policy translation, redaction, process lifecycle, and cancellation; this is
  not package-spawned or a sandbox claim.
- Revisit when either vendor ships ACP or another documented structured
  session-control protocol, or when Antigravity exposes live approval events.
