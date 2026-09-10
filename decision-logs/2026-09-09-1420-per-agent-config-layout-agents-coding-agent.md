---
date: 2026-09-09 14:20
status: approved
decision_about: "Per-agent config layout: ~/.config/clay/agents/<agentId>/, coding-agent first"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Per-agent config directories under ~/.config/clay/agents/; coding-agent config root is ~/.config/clay/agents/coding-agent

## Decision

Clay's user config root gains a per-agent layer: each agent owns
`~/.config/clay/agents/<agentId>/`. The coding agent — the first occupant — uses
`~/.config/clay/agents/coding-agent/` for everything agent-specific: `skills.json`,
`mcp.json`, `SYSTEM.md`, and `skills/<name>/SKILL.md` (seeded agent-delivered skills and
user-added skills share that one skills directory). The earlier flat
`~/.config/clay/agent/…` layout is abandoned before anything shipped. Skill discovery
roots become: `<workspaceRoot>/.agents/skills`, `~/.config/clay/agents/coding-agent/skills`,
and `~/.agents/skills`.

## Context

Clay will host multiple agents over time (coding agent now; Chat/work/research agents
later). The Phase 2.2 design had accumulated agent-specific files (skills config, MCP
config, seeded skill files, SYSTEM.md) under a single `~/.config/clay/agent/` directory.
That conflates "the config of the agent product line" with "the config of one agent", and
the first future agent would force either a rename (breaking user files) or collision.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "Notice I changed the directory structure a little bit for config
  root. I am using ~/.config/clay/agents/coding-agent as the config root for the coding
  agent. You need to adopt this structure instead of ~/.config/clay/agent … The idea is
  that Clay will have other agents as well so from the start coding-agent should be
  treated separately to maintain long term separation of concern."

## Alternatives Considered

1. **Flat `~/.config/clay/agent/`** — rejected: agent-specific and product-line files
   collide; renaming later breaks user files.
2. **`~/.config/clay/coding-agent/`** (agents layer implied by naming) — rejected: the
   explicit `agents/` group keeps the namespace unambiguous as agent count grows.
3. **One shared skills dir for all agents** (`~/.config/clay/.agents/skills`) — rejected:
   skills are agent-curated content; a shared pool forces every agent to see every skill.

## Rationale and Evidence

- The `agents/<agentId>/` layer maps 1:1 to agent profiles registered in the daemon
  (`Chat`, coding agent), so the filesystem layout and the runtime registry stay parallel.
- One skills directory per agent (`…/coding-agent/skills/`) unifies seeded
  agent-delivered skills and user-added skills in a single scannable location; gating
  (e.g. wiki skills behind `/wiki-init`) stays a daemon concern, not a directory concern.
- `tool-caps.json` is unaffected: the daemon reads it from the agent **data** dir (beside
  `book.json`), which is a runtime-state root, not this config root.

## References

- `roadmap.md` Phase 2.2 (updated path references).
- `plans/117-Phase2.2-….md` (all tasks adopt this layout).
- `clay-agent/src/host.ts` — `agentConfigRoot`/`homeSkillsRoot` options and
  `loadSkillsConfig`.
- `src/packages/service.rs` `default_config_root()` — the parent `~/.config/clay`
  convention this layer sits under.

## Consequences

- Positive: agents stay isolated from day one; adding an agent never renames existing
  user files; the skills directory is a single audit point per agent.
- Risks: path typo risk (`agents/` vs `agent/`) — the daemon's fail-closed loader handles
  a missing directory as defaults.
- Reserved names: `graft`, `wiki-searcher`, `wiki-maintainer` in the coding-agent skills
  directory are agent-delivered (seeded, gated) — disk discovery skips them so they can
  only activate through their own paths (`/wiki-init`, graft binding).
- Scope boundary: `~/.config/clay/agent` (singular, no `s`) remains the coding agent's
  **runtime data dir** (`sessions.sqlite`, `credentials.vault`, `book.json`,
  `tool-caps.json` — `src/server/agent.rs` `for_server`). Runtime state does not move;
  moving it would orphan session history and stored credentials. Only configuration and
  agent-delivered content live under `agents/coding-agent`.
- Revisit if: an agent needs shared skill pools, at which point a documented
  cross-agent include mechanism can be added.
