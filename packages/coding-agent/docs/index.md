# @clay/coding-agent

First-party Coding Agent surface (Phase 2 skeleton).

Loading the package registers the **coding profile** on the clay-agent
daemon: the nine Prism coding tools (`shell`, `read`, `write`, `edit`,
`repo_list`, `repo_search`, `glob`, `delete`, `move`) plus
`ask_user_decision`, the coding system prompt layer, and the
`coding-agent.createPlan` skill. Registration is idempotent across loads (re-registration replaces) via the
trusted-only `clay:agent` facade (`agent.skillRegister` before
`agent.profileRegister`); omitted or unknown tool/skill ids fail closed at
session start, before any provider turn — exactly like Chat's fail-closed
profile handling. Load entries never spawn or block on the daemon: while the
daemon is down, declarations queue server-side and apply right after its
next initialize handshake; in runtimes with no agent host at all they
queue process-global and apply when a host installs.

Tool execution keeps the Phase 1 acceptance policy, unchanged:
in-workspace writes and reads are free; out-of-workspace writes,
delete/move, and shell metacharacter commands are approval-gated; the
full-autonomy toggle defaults to off (host-set only). Registering a profile
grants no execution authority.

## What the skeleton ships

| Piece | Where |
|-------|-------|
| Coding profile + system prompt layer | `dist/load.js` → `agent.profileRegister` |
| Slash command surface (nine commands) | `dist/load.js` → `agent.commandRegister` |
| Agent split surface (`coding-agent.surface`, activation `pane`) | `dist/load.js` → `ui.serverRegisterPaneContentContribution` |
| Chrome commands (`coding-agent.profile`, `coding-agent.close`) | `clay.contributions.commands` |
| UI pane contents / surface extension points | this package (launch via the Coding Agent command) |

Skills are not hardcoded here: the clay-agent daemon discovers them from
three roots (decision 2026-09-09-1420): the workspace (`npx skills` layout
`<root>/.agents/skills/<name>/SKILL.md`), the per-agent config dir
(`~/.clay/agents/coding-agent/skills/`, which also holds the seeded
agent-delivered skills), and the home dir (`~/.agents/skills/`). Roots and
agent-skill toggles are configured in
`~/.clay/agents/coding-agent/skills.json`.

Prompt layers compose per session in a locked order: the profile's base
instructions, then the user's global `SYSTEM.md`
(`~/.clay/agents/coding-agent/SYSTEM.md`, seeded empty), then the
workspace `AGENTS.md` app layer (repo-root project prompt; symlink-escape
excluded, 64 KiB cap, silently absent-safe).

The working-area empty tab is the launcher surface (plan 118): the landing is
its own pane-content contribution, and this package contributes only the named
`pane` surface. Loading is safe in any order; the Coding Agent adds
profile/skill registrations and its chrome command without touching the
landing.

## Activation

One line in `~/.clay/init.js` (or the canonical
`examples/packages/first-party.js`, loaded via `loadConfigurationModule`):

```js
import { loadPackage } from "clay:packages";
await loadPackage("@clay/coding-agent");
```

That line applies the manifest contributions (command, chrome extension
point) and runs `dist/load.js`, which registers the coding profile. No
copied manifests, no manual primitive registration, no raw
facade plumbing. Loading is explicit — without the line nothing
coding-agent-shaped registers and the landing stays untouched.
Load entries never spawn or block on the daemon: while the daemon is down,
declarations queue server-side and apply right after its next initialize
handshake; in runtimes with no agent host at all they queue process-global
and apply when a host installs.

Sessions then select the profile with `session.new { profile: "coding" }`.
Disabling or deleting the package withdraws the profile registration, the
slash surface, the pane surface, and the chrome commands; the daemon and the
landing package keep working unchanged.

## Agent split surface

Running **Coding Agent** (Command Centre, `coding-agent.profile`) opens the
binding-spec split surface in the active working-area pane; **Close Coding
Agent** (`coding-agent.close`) or the surface's Close button dismisses it.
Both commands are server-validated and answered with one client shell-command
request the client re-parses deny-by-default; the server holds no open/close
state.

The surface itself is 50/50 vertical split (user-resizable, ratio-clamped,
keyboard-operable separator):

- **Left** — chronological transcript (user prompts, agent messages,
  thinking, tool outputs incl. MCP tools, skills loaded, usage, errors) as
  uniform-height truncated boxes colored by content type from typed theme
  tokens; selecting a box shows its full content in the right pane. Composer
  grows with message length (Enter submits, Shift+Enter newline), `/`
  completion lists the nine slash commands, `Shift+Tab` cycles reasoning
  effort when the model exposes levels (no-op otherwise). Status row: left
  workspace path + git branch; right provider/model, effort, last usage.
  Extension strip: extensions + MCP allow-list server names.
- **Right** — three tabs: **Files** (the existing file-browser SDUI view,
  same server-owned snapshot and inert intent path), **Memory**
  (Observational-Memory chrome this phase), **Context** (category counts:
  system prompts, user prompts, agent messages, tool outputs, skills loaded,
  files loaded — counts only, never content).

The declared component tree carries static copy only; every dynamic value
rides the one core-owned AG-UI stream. Third-party `pane` surfaces render
through the unchanged generic SDUI renderer.

## Skills

`coding-agent.createPlan` — the plan-file convention: numbered plan documents
under `plans/`, tasks with acceptance criteria, dated evidence, appended
(not rewritten) history. Progressive disclosure: sessions see the name and
description; full instructions load through the daemon's `load_skill` tool.

## Slash commands

Registered via `agent.commandRegister` (queued like profile/skill
declarations while the daemon is down). Prompt text starting with a
registered command name (exact first-token match) dispatches the daemon
handler instead of prompting the model; unknown `/x` fails closed with
bounded feedback. Args are JSON `{ ... }` after the command name.

| Command | Handler | Effect |
|---------|---------|--------|
| `/compact` | `compact` | Manual compaction via the session's compaction strategy. |
| `/new`, `/n` | `newSession` | Fresh session with the same profile, provider, model, and workspace root. |
| `/branch` | `checkout` | Check out a prior entry: args `{ entryId }` (omit for leaf). |
| `/tree` | `tree` | Branch tree with per-branch summaries and leaf marker. |
| `/fork` | `forkSession` | Fork at leaf (or `{ entryId }`) and continue in the fork. |
| `/clone` | `cloneSession` | Deep copy into a new session id. |
| `/open-session` | `openSession` | Load a session by id: args `{ sessionId }`. |
| `/open-session-as-fork` | `openSessionAsFork` | Load and immediately fork (session id + optional `{ entryId }`). |

`/model` is intentionally not re-declared: the core built-in
`agent.clientOpenModelPicker` already provides the model-picker hook.

## Run options (policy caps and default compact)

From `init.js` or a trusted config module, after `loadPackage("@clay/coding-agent")`:

```js
import { setRunOptions } from "clay:agent";

await setRunOptions({
  maxInputTokens: null,          // default; cumulative billed, or a positive int
  maxOutputTokens: null,         // default
  maxTurns: null,                // default; null = run until done
  maxToolRounds: null,
  maxToolCalls: null,
  maxWallTimeMs: null,           // default; set e.g. 1_800_000 for a 30-min fence
  compactAfterTokens: 800_000,   // stored; unused until auto-compact
  compaction: "llm",             // default for /compact when strategy omitted
});
```

Omit the call to keep those defaults: every policy axis (tokens, turns,
tool rounds, tool calls, wall) is unbounded (`null`); the only hard caps
are Prism's per-frame request/response bytes (64 MiB). `compactAfterTokens` here is **not** the
OM `agent.compact` threshold (decision 2158, 80_000).

## Knowledge options (opt-in wiki)

The wiki knowledge base (`@arnilo/prism-memory/wiki`, decision 2156) is
**off by default** — nothing loads and no commands, tools, or skills exist
until you opt in from workspace configuration:

```js
// init.js
await agent.knowledgeSetOptions({
  workspaceRoot: "/path/to/your/project",
  wiki: true, // false disables again — no residue
});
```

When enabled, the daemon loads the wiki kernel via `kernel.load` and the
workspace gains:

| Capability | Ids |
|------------|-----|
| Commands | `/wiki-init`, `/wiki-refresh` (SHA-256 Merkle-diff incremental), `/wiki-lint` |
| Tools | `wiki_search`, `wiki_read_page`, `wiki_record_insight` |
| Skills | `wiki-searcher`, `wiki-maintainer` (progressive disclosure via `load_skill`) |

The compiled wiki lives in the workspace `.wiki/` tree as OKF v0.2 bundles
(profiles `codebase`/`pkm`/`hybrid`/`auto`; `auto` picks per workspace).
Writes stay inside `.wiki/`. Optional `qmd` hybrid search is host-owned and
deny-by-default: pass `qmdPath` in the options to enable it; without it the
catalog fallback serves `wiki_search`. Enabling for a second workspace
rebinds the daemon's single wiki binding; sessions created after enabling
carry the tools.

### Opt-in graft context graph

```js
await agent.knowledgeSetOptions({
  workspaceRoot: "/path/to/your/project",
  graft: true, // false disables again — no residue
  graftCliPath: "/usr/local/bin/graft", // host-owned CLI
});
```

When the graft CLI resolves, the daemon loads the `@arnilo/prism-memory/graft`
kernel and the workspace gains the pull tools `graft_ask`, `graft_grep`,
`graft_callers`, `graft_skeleton`, `graft_map`, and `graft_blast`, the
`/graft`-family commands (`status`/`build`/`check`/`viz`), and the `graft`
skill. `graftMode: "push"` adds the gated retrieval pack, first-turn
orientation, and post-edit blast radius (`graft:dirty` on mutating-tool
boundaries — never keystrokes). CLI resolution fails closed: an absent CLI
leaves tools hidden and the agent unperturbed. Graph output is agent-aid,
not authority; the CLI runs as a host-owned child process (bounded time and
stdout), never spawned by package JavaScript.

### pi-parity conformance checklist

The numbered parity checklist (P1–P18) with negative checks and performance
observations lives at [parity-checklist.md](parity-checklist.md); execution
records are in `test-plan/17-coding-agent-parity.md`.

## Web and browser capability (Obscura)

When the Phase 1 Obscura engine is present on the host, coding sessions
additionally carry the web/browser tool surface — all of it hidden when the
engine is absent (capability reduction, never an error):

| Tools | Surface |
|-------|---------|
| `web_search`, `web_fetch` | Public-web search and bounded markdown fetch through the host's replaceable HTML search profile. |
| `obscura_fetch`, `obscura_scrape` | Native one-shot fetch (bounded dump modes, optional CSS selector) and batch scrape. Custom scrape expressions are deny-by-default (`allowEval` is never enabled by the daemon). |
| `browser_open`, `browser_snapshot`, `browser_act`, `browser_evaluate`, `browser_close` | Run-owned browser automation over CDP: observe, act, policy-gated evaluate, plus block/throttle/emulate on Chromium. |

All web results are labeled untrusted external content: public-HTTP(S)-only
URL validation, byte/count/timeout caps, and credentials/cookies never cross
event boundaries. The engine process and its children are daemon-owned
(group close on shutdown); package JavaScript never spawns anything.
