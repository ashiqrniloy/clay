---
date: 2026-08-21 17:58
status: approved
decision_about: "First-party Prism agent host: Clay UI, no ACP, CLI-parity coding agent"
proposed_by: both
explicitly_approved_by_user: true
---

# Decision: Native Prism host in Clay UI; skip ACP; coding agent at CLI parity

## Decision

Clay talks to Prism through a Clay-owned Node daemon (`clay-agent`) and Clay IPC wrapping Prism `createAgent` / `createAgentSession` / `AgentEvent`. ACP and AG-UI are not the first-party agent bus. Chat ships first (Phase 25). The coding agent (Phase 29) must match a CLI coding agent’s capabilities inside Clay, using Prism tool factories plus Clay document operations for dirty buffers — not ACP `fs/*`.

## Context

Clay is becoming AI-native: launch and new tabs open an agent view. Prism 0.3.0 is the runtime. Prism ships ACP (`@arnilo/prism-ag-ui/acp`, `@arnilo/prism-acp-agent`) so *existing ACP editors* can drive Prism. Clay is writing its own UI, so ACP would be a translation of `AgentEvent` into another protocol Clay would then translate again.

The user asked whether to adopt ACP for the coding agent, then agreed to skip it and run Prism from Clay UI, provided the coding agent still has CLI-class capabilities (tools, shell, approvals, sessions, dirty-buffer awareness).

## Approval

- Proposed by: agent (skip ACP; Clay UI + Prism native; CLI-parity via operations seams), user (AI-native chat, Prism 0.3.0, coding agent first special-purpose profile)
- Approved by user: Yes
- Approval evidence: “Okay. I agree. To start with I want to skip ACP and just run the Prism agent from Clay UI. But we need to make sure that it has all the capabilities that a CLI agent would have inside clay.”

## Alternatives Considered

1. **ACP client in Rust, Prism ACP agent in Node** — rejected. Extra crate, dual session ids, capability matrix, still implement Clay mutation. ACP’s job is editor↔foreign-agent interop; Clay is both UI and host.
2. **`createAcpFilesystemOperations` for dirty buffers without a full ACP client** — rejected. Same operations seams exist without ACP. Clay document reverse-RPC is the authority-correct path (versions, leases, region locks).
3. **Chat-only forever / coding agent as prompt-with-no-tools** — rejected. User required CLI-parity inside Clay.
4. **Put Prism inside `deno_core`** — rejected. Node >= 20 is required; process boundary is the trust boundary.

## Rationale and Evidence

- ACP introduction (agentclientprotocol.com): standardize agent↔*any editor*. Clay is not trying to host third-party ACP agents or publish this agent to Zed in Phase 25/29.
- Prism `docs/acp.md`: use `createPrismAcpAgent` when the client already speaks ACP. “ACP is a protocol adapter, not an editor.”
- Prism `docs/agent-session-runtime.md` / `docs/agent-events.md`: `createAgentSession` + `AgentEvent` is the native host surface.
- Prism `docs/coding-agent-tools.md`: nine CLI tools plus Git, process sessions, `ask_user_decision`; `read`/`write`/`edit` accept host `ReadOperations` / `WriteOperations` / `EditOperations`. `createAcpFilesystemOperations` is one adapter, not the only one.
- Clay already owns documents, leases, confirmation sessions, Command Centre, and GUI IPC. Mapping ACP `fs/write_text_file` onto AI-safe mutation would implement Clay mutation *plus* ACP.

## References

- https://agentclientprotocol.com/get-started/introduction — why ACP exists (LSP for agents)
- `/home/arn/Projects/prism/docs/acp.md` — Prism ACP adapter “when to use”
- `/home/arn/Projects/prism/docs/coding-agent-tools.md` — tool factories and operations seams
- `/home/arn/Projects/prism/docs/agent-session-runtime.md` — `createAgent` / `createAgentSession`
- `roadmap.md` Phase 25 and Phase 29
- `.agents/skills/project-patterns/references/authority-boundaries.md` — server owns AI mutation
- `.agents/skills/project-patterns/references/extensions-and-ai.md` — AI edits carry version/range/scope

## Consequences

- Phase 25: `clay-agent` + Clay IPC + agent view + Command Centre provider/model/agent/setup + Chat (no tools). Event union includes unused tool/permission variants so Phase 29 does not rewrite IPC.
- Phase 29: same daemon loads coding-agent + coding-security; Clay document operations; approvals; diffs; process sessions without PTY; MCP allow-list. Interactive PTY waits on the terminal package.
- No `@agentclientprotocol/sdk` or AG-UI in Rust or in `clay-agent` for first-party agents.
- Revisit ACP only if the product is “run foreign ACP agents in Clay” or “this agent in other ACP editors.”
