---
date: 2026-09-10 15:26
status: approved
decision_about: "Clay root moves to ~/.clay; per-agent data dir at agents/coding-agent/data; tool-caps.json becomes agent config"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: `~/.clay` replaces `~/.config/clay`; agent runtime data lives in `agents/coding-agent/data`; tool-caps.json is agent configuration

## Decision

Three linked layout changes to Clay's user-level storage:

1. **Root move.** The Clay root leaves `~/.config/clay` and becomes
   `~/.clay`. Clay stores runtime data (SQLite sessions, credential
   vault, book) alongside configuration, so the XDG config location no
   longer describes what the directory contains. All resolvers
   (`ConfigurationRuntime::default_config_root`,
   `packages::service::default_config_root`) resolve `$HOME/.clay`;
   `layout.json` moves to `~/.clay/layout.json` (the XDG location is
   no longer consulted for the root, only as a legacy read path).
2. **Per-agent data.** The coding agent's runtime data dir —
   `sessions.sqlite`, `credentials.vault`, `vault.passphrase`,
   `book.json` — moves under the per-agent root from decision
   2026-09-09-1420: `~/.clay/agents/coding-agent/data/` (mode 0700).
   The data/config separation is kept as a subdirectory, not a
   sibling: one agent owns one directory. The data/config-root split
   inside it mirrors the old top-level split.
3. **tool-caps.json re-homed as configuration.** Repository scan caps
   are user configuration for the coding agent's tools, not runtime
   state: the file lives at `~/.clay/agents/coding-agent/tool-caps.json`
   beside `skills.json` and `mcp.json`. The daemon reads it from the
   agent config root, falling back to the data-dir location (which is
   where migrations deposit it) for compatibility.

## Migration (one-time, automatic, best-effort)

- `~/.config/clay` exists and `~/.clay` does not → the whole legacy
  tree is renamed to `~/.clay` (atomic, same filesystem).
- Both exist (user pre-created `~/.clay`) → only the legacy runtime
  data dir is salvaged (`~/.config/clay/agent` → `~/.clay/agent`),
  because stranding `sessions.sqlite`/`credentials.vault` would lose
  session history and lock the vault. Fresh config in the new root is
  left alone.
- At daemon-spawn time, a legacy `<config-root>/agent/` data dir is
  renamed to `<config-root>/agents/coding-agent/data/`. On rename
  failure the legacy dir keeps serving — data is never deleted or
  orphaned by a failed move.
- `tool-caps.json` needs no copy: after the data-dir rename it sits in
  the data dir and the daemon's legacy fallback reads it there.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "Move all data into ~/.config/clay/agents/coding-agent/data and adopt
  this change to clay, update examples config and update documentation and decision log";
  then "Given that inside this folder we are now storing data and config, it doesn't make
  sense to keep it inside .config. I would now create a .clay folder in user home and move
  everything from ~/.config/clay inside this new .clay directory. Again make the structural
  change and adopt in code, update documentation and create a decision log with all the
  changes discussed."

## Alternatives Considered

1. **Keep `~/.config/clay` for config, `~/.config/clay/agent` for data**
   — rejected: the root's contents are no longer "configuration" only;
   `.config` would keep holding a database and credential vault.
2. **Flat `~/.clay/agent` for data** (sibling of `agents/`) — rejected:
   the coding agent's data would sit outside its per-agent directory,
   breaking the one-directory-per-agent model the 1420 layout created.
3. **Copy instead of rename for migration** — rejected: copies leave
   two live trees (split-brain credentials); rename is atomic and
   self-cleaning. Failures keep the legacy tree rather than half-move.

## Rationale and Evidence

- The daemon already treats the per-agent root as the unit of agent
  state (skills.json, mcp.json, SYSTEM.md, seeded skills); runtime data
  is the remaining piece that scattered to a top-level `agent/` dir.
  Moving it under `data/` makes `~/.clay/agents/coding-agent` fully
  self-contained — backup, sync, and wipe become single-directory
  operations, and future agents (Chat, research) get the same shape.
- tool-caps.json is validated, user-editable configuration documented
  in the canonical example config; its placement beside book.json was
  an accident of the earlier flat layout, not a design statement.
- Migration lives at the resolver level (server boot), so every entry
  point (desktop launch, CLI, supervised server) migrates exactly once,
  idempotently, before anything reads the old paths.

## Consequences

- `~/.config/clay` is abandoned after migration; users who skip nothing
  see their sessions, credentials, preferences, and init.js untouched
  under `~/.clay`.
- The example config tree ships no `agent/` directory:
  `examples/config/agents/coding-agent/tool-caps.json` is the shipped
  location, and the README documents the `data/` subdir as daemon-owned.
- Legacy read paths remain: tool-caps.json (data dir) and layout.json
  (XDG) load when the new location is absent.

## Superseded

- Amends decision 2026-09-09-1420: the per-agent root gains the
  `data/` subdir and moves from `~/.config/clay/agents/...` to
  `~/.clay/agents/...`.
- Amends decision 2026-05-08-1841 and every doc referencing
  `~/.config/clay/init.js` as the entry point (now `~/.clay/init.js`).

## References

- `src/server/configuration.rs` (`default_config_root`,
  `migrate_legacy_config_root`, migration tests)
- `src/server/agent.rs` (`AgentHostConfig::for_server` data-dir
  derivation + legacy rename, data-dir migration tests)
- `src/packages/service.rs` (`default_config_root`)
- `src/shell/layout_persist.rs` (`~/.clay/layout.json`, legacy read)
- `clay-agent/src/host.ts` (`loadToolCaps` config-root read +
  data-dir fallback)
- `examples/config/README.md` (layout table, `data/` documentation)
- `docs/wiki/modules/clay-agent.md` (Spawn section)
