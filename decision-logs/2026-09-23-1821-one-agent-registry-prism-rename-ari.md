---
date: 2026-09-23 18:21
status: approved
decision_about: "Agent architecture: one agent registry / one UI wire / capability model with two runtime depths; Prism agent rename; pi as first external runtime"
proposed_by: agent
explicitly_approved_by_user: true
---

# Decision: One agent registry, one UI wire, capability-gated agent runtimes (A′)

## Decision

Clay converges on **one agent registry, one UI wire, and one capability
model**, while keeping **two runtime depths**: the native Prism daemon keeps
its rich Clay-owned RPC channel, and third-party agents integrate through the
core-owned external runtime contract (the Agent Runtime Interface, ARI) with
declared capabilities. The native coding agent is renamed the **Prism agent**
(registry id `prism`) — an identity rename in the agent registry and config
layout, not a daemon binary rename. The **pi coding agent is the first
external runtime adapter** and, like every configured runtime, may serve as a
tab's primary agent — not only as a delegation child.

## Context

The architecture question came from using herdr (terminal workspace manager
where agents live in real PTYs and agent-awareness is screen detection plus
optional per-agent integrations). The question: does Clay need two agent
mechanisms (one for the Clay agent, one for third parties), or one mechanism
with the Clay agent treated as just another third party (renamed Prism agent)?

Research findings (2026-09-23):

- **Current mechanism**: Rust core spawns the `clay-agent` Node daemon
  (one per server, `env_clear`, stdio). NDJSON-RPC with ~40 methods
  (`session.*`, `run.*`, `model.*`, `provider.*`, `credential.*`, `skill.*`,
  `command.*`, `knowledge.setOptions`, …) plus a reverse-RPC channel
  (approvals). Prism `AgentEvent`s are mapped in Rust (`map_event`,
  `src/server/agent.rs`) onto `AgentWireEvent` / `AgentServerMessage` —
  **the frontend never sees Prism; the AG-UI projection seam already
  exists**.
- **Roadmap state**: decision `2026-09-02-1440` already approved direct
  external adapters (Claude Code SDK, Antigravity headless) with a
  deliberately small contract: lifecycle, normalized bounded events,
  declared capability set, policy bundles. Per-agent config layout
  `~/.clay/agents/<id>/` already decided (2026-09-09-1420).
- **Vendor surfaces all exist**: pi RPC mode (`pi --mode rpc`: JSONL
  commands, streaming events, extension-UI dialog subprotocol for
  approvals, steering, session trees, model/thinking switching); Codex
  `app-server` (bidirectional JSON-RPC 2.0, powers its VS Code extension);
  Claude Code Agent SDK (`canUseTool` allow/deny/modify); Antigravity
  headless NDJSON (policy-based, no live approval callback).
- **herdr's lesson**: uniform treatment at the primitive layer (PTY +
  detected state), per-agent structured depth on top — not one fat
  protocol for everyone.

## Approval

- Proposed by: agent (option A′ out of options A / B / A′).
- Approved by user: Yes
- Approval evidence: "I agree with your recommendation. So we move ahead
  with option A`. … The implementation should include one third party
  agent integration as well which should be pi coding agent." (2026-09-23)

## Alternatives Considered

1. **Option A — two mechanisms exactly as roadmapped, no rename or
   convergence** — rejected as-is. Preserves native depth but leaves the
   "Clay agent is special" split visible in naming/registry, and defers the
   capability-driven UI discipline that makes heterogeneous agents coherent.
2. **Option B — one mechanism: rename to Prism agent and treat every agent
   (including Prism) as a third party speaking only the external contract** —
   rejected. The contract would have to grow to a superset (~40 methods +
   reverse-RPC + trees + compaction + context inspection) that every vendor
   partially implements — capability-flag explosion and
   lowest-common-denominator pressure, i.e. the same de-facto split with
   worse ergonomics. Deep native features (Memory tab, Context inspector,
   checkpoints, steering, credential vault) would regress or become
   native-only extensions anyway. Large mid-Phase-2.2 rewrite risk for zero
   user-visible gain.
3. **Route external agents through the Prism daemon** — already rejected by
   decision `2026-09-02-1440`; unchanged.

## Rationale and Evidence

- The thing that must be uniform is the layer users and the UI experience:
  **registry + AG-UI projection + capability model**. That layer already
  exists (frontend consumes `AgentServerMessage`; `map_event` is the seam);
  the work is to finish it (runtime identity + capability state on the
  snapshot, capability-gated UI).
- Below that layer, runtimes genuinely differ in depth and trust: the Prism
  daemon is in-repo, `env_clear`'d, same release train, secrets in the Clay
  vault; vendors are foreign binaries with their own auth stores. One
  protocol cannot unify that boundary — capability declarations make the
  difference truthful in the UI instead of hiding it.
- pi is the ideal first ARI adapter: richest documented frontend-less
  surface of the target vendors (RPC steering, session trees, extension-UI
  approval round-trip), so it exercises more of the contract than the
  Claude Code SDK path and validates it faster.
- The rename is identity-level and cheap now (before external agents land
  and `~/.clay/agents/coding-agent/` fossilizes); plan 142 and later plans
  reference `~/.clay/agents/<type>/` generically, so no later-plan rewrites
  are required.

## Consequences

- Plans `plans/149-Prism-Agent-Rename-and-One-Agent-Registry.md` (rename +
  registry descriptors), `plans/152-Agent-Runtime-Interface-and-Capability-Gated-UI.md`
  (ARI v1 contract + capability-driven UI), and
  `plans/153-Pi-Coding-Agent-External-Runtime-Adapter.md` (first external
  runtime) implement this; `roadmap.md` Phase 9 is amended accordingly.
- The native Prism daemon keeps its rich channel and becomes the reference
  implementation of the capability superset; the ARI contract grows toward
  that ceiling as adapters justify each addition. An ARI-conformant facade
  on the daemon for dogfooding is deferred (revisit when a second external
  adapter lands).
- Product Shape layer 1 wording is clarified: external-runtime lifecycle and
  policy stay **core-owned (Rust)** per decision 1440; the daemon hosts only
  the native Prism agent.
- The daemon binary/directory name `clay-agent` stays (internal plumbing;
  sidecar naming, release engineering). Registry id and config root become
  `prism`.
- Risks: drift between daemon protocol and ARI (mitigated by the shared
  projection and capability model); UX inconsistency across agent depths
  (mitigated by capability gating shipped with ARI v1, not after);
  pi RPC protocol drift across pi versions (fail-closed adapter, tolerate
  unknown record types, pin discovery).
- Revisit when: a second external runtime lands (consider the daemon ARI
  facade), or any vendor ships ACP/another structured session-control
  protocol worth adopting as an ARI transport.
