# `examples/config/` — canonical starter configuration

Everything in this folder is what a new user copies into their
configuration root to get started:

```sh
cp -r examples/config/. ~/.clay/
```

Layout (mirrors `~/.clay/` on a user machine):

| Path                  | Destination                  | Purpose |
|-----------------------|------------------------------|---------|
| `init.js`             | `~/.clay/init.js`     | Base Clay config — every user-facing configuration surface, fully functional standalone. Ends with fault-isolated loads of the two package modules below. |
| `packages/first-party.js` | `~/.clay/packages/first-party.js` | Language-server grants + one-line `loadPackage` calls for bundled `@clay/*` packages (including the Coding Agent). |
| `packages/third-party.js` | `~/.clay/packages/third-party.js` | Commented template for third-party packages (none ship) — install/remove/update/adopt contract plus the capability-grant surface (`clay package authorize`, `authorize({...})`, inspect, revoke). |
| `agents/coding-agent/tool-caps.json` | `~/.clay/agents/coding-agent/tool-caps.json` | Coding-agent repository scan caps (`repo_list` / `repo_search` / `glob`). |
| `agents/coding-agent/skills.json` | `~/.clay/agents/coding-agent/skills.json` | Skill discovery roots (workspace, agent config dir, home) + on/off switches for agent-delivered skills (wiki-searcher, wiki-maintainer, graft). See `agents/coding-agent/` below. |

## `agents/coding-agent/` — per-agent configuration and data

Each Clay agent owns `~/.clay/agents/<agentId>/`; the coding agent
is the first occupant (decisions 2026-09-09-1420 and 2026-09-10-1526).
This directory holds agent configuration, agent-delivered content, and —
in `data/` — the agent's runtime state.

**More agent types.** Any directory under `~/.clay/agents/` is an agent type
Clay offers: copy `coding-agent/` to `~/.clay/agents/reviewer/`, edit its
`SYSTEM.md` / `skills.json` / `mcp.json` / `tool-caps.json`, and it appears in
the launcher's agent pane and in the agent view's title picker (both read the
same directory scan, no package load, no restart needed for a picker entry that
already exists). Picking one for a tab switches *that tab* to the agent's own
configuration — system prompt, skill roots, tool caps, MCP servers, model/effort
defaults — without touching the tab's folder or its conversation, and the
transcript keeps every earlier turn labelled with the agent that produced it.
The name must be a plain directory name (letters, digits, `-`, `_`, at most 64
characters): separators, `..`, and absolute names are rejected, so an agent type
can never address a path outside `agents/`. Runtime state (`data/`) stays with
the shipped agent's directory: one daemon holds one sessions database and one
credential vault.

- `skills.json` — skill discovery roots and agent-skill toggles. All
  keys optional; absent file = everything on with defaults. Relative
  paths are rejected (root disabled), unknown keys warned.
  - `roots.workspace.enabled` — scan `<workspaceRoot>/.agents/skills`
    (npx skills format).
  - `roots.configRoot.enabled` — scan
    `~/.clay/agents/coding-agent/skills/` (seeded agent-delivered
    skills plus any skill you add there).
  - `roots.home.enabled` / `roots.home.path` — scan `<path>/skills/`
    (default `~/.agents`); `~`-expanded or absolute paths only.
  - `agentSkills.{wikiSearcher,wikiMaintainer,graft}` — off ⇒ the skill
    never registers and its dependent slash commands never appear (wiki
    skills additionally require `/wiki-init`; graft additionally requires
    the graft CLI).
- `skills/<name>/SKILL.md` — seeded agent-delivered skills
  (`graft`, `wiki-searcher`, `wiki-maintainer`) land here and stay the
  source of truth: the daemon loads the frontmatter description and body
  from these files at daemon start (name and tool bindings stay
  daemon-owned), so edits change what the agent receives on the next
  daemon start. Deleted files
  are regenerated from the built-in seed at daemon start; a malformed or
  oversized file (256 KiB cap) warns and falls back to the built-in
  content instead of
  breaking the agent. Skills you add beside them are discovered like any
  other root (≤64 per root).
- `SYSTEM.md` — your global system-prompt layer for this agent. Seeded
  empty at first launch; text you add is appended to every new session's
  system prompt (after the profile's base instructions, before the
  workspace `AGENTS.md`), applies from the next session (each session
  build re-reads the file), and rides the cached prompt prefix once the
  session starts. Bounded at 64 KiB; an oversized or unreadable
  file is skipped with a warning.
- `mcp.json` — stdio MCP servers this agent connects. Shape:
  `{"servers": {"<serverId>": {"command", "args", "env", "cwd",
  "timeoutMs"}}}`. Commands are absolute paths or bare names
  (PATH-resolved); relative paths are rejected. `env` takes explicit
  name→value pairs only (nothing is inherited from Clay's environment);
  at most 32 servers and 64 args per server; `timeoutMs` is a per-server
  tool-call timeout in ms (daemon default 60 s, hard ceiling
  30 min). Server entries also come
  from the repo-root `.mcp.json` (`{"mcpServers": {...}}`; `cwd` and
  `timeoutMs` are ignored there — absent means defaults); on an id
  collision this user file wins. Servers connect without an
  approval gate — you own what you configure. Changes apply at the next
  Clay launch (the allow-list is built when the server starts).
- `tool-caps.json` — repository scan caps for this agent's coding tools
  (see the section below).
- `data/` — runtime state, owned by the daemon (created on first
  launch, mode 0700): `sessions.sqlite` (session history),
  `credentials.vault` + `vault.passphrase` (encrypted credentials), and
  `book.json` (persisted provider/model/profile selection). Leave this
  directory alone — deleting it orphans session history and stored
  credentials. A legacy `~/.clay/agent/` data dir from before the
  2026-09-10 layout migrates here automatically at server start, and a
  pre-2026-09-10 `~/.clay` tree migrates to `~/.clay` the same
  way.
- `.seed-manifest.json` — bookkeeping written by the daemon (seeded-file
  provenance for the agent settings page).

These agent files complement `~/.clay/init.js`: they are
declarative JSON/markdown with no JS knobs, so `init.js` stays the only
entry point for JS-level configuration.

## `agents/coding-agent/tool-caps.json`

The coding agent's repository tools walk the workspace under caps so one
search cannot stall a run on a huge tree. The shipped file matches the
defaults; edit it to raise a cap when the tree is genuinely large:

- `maxEntries` — entries scanned (hard ceiling 100000)
- `maxFiles` — files scanned (hard ceiling 100000)
- `maxDepth` — directory depth (hard ceiling 128)
- `maxResults` — results returned (hard ceiling 10000)
- `maxMatches` — search matches (hard ceiling 10000)
- `maxScanBytes` — bytes of file content scanned (hard ceiling 1073741824)
- `exclude` — directory names skipped by the walk (never scanned)

When a tool hits a cap, Clay returns an error naming the cap, this file,
and the hard ceiling — raise the key and restart Clay. Prefer scoping
searches with the tool's `path` argument over raising caps; excluding
build trees (`target/`, `dist/`, `node_modules/`) is almost always the
right fix.
