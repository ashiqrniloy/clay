# clay-agent

Clay-owned Node >= 20 child that hosts Prism **0.5.1**. Not a Clay JS package.
Packages never spawn or speak to this process; the Clay server does.

## Spawn

```text
node dist/main.js --data-dir DIR [--mock]
```

- Requires Node >= 20. Exits non-zero with a clear message otherwise.
- `--data-dir` is required. Creates `sessions.sqlite` and `credentials.vault` (mode 0600).
- `--mock` registers Prism `createMockProvider` for offline smoke tests. Production Clay must omit it.
- Stdio is newline-delimited JSON-RPC 2.0. Frames over 1 MiB are rejected.
- First request must be `initialize` with `{ "passphrase": "..." }`. The passphrase is never logged. Prism does not read `process.env` for secrets.
- Unreadable/wrong-passphrase vault: initialize replies with an error and the process exits 1.
- `shutdown` or SIGTERM/SIGINT closes SQLite and exits 0.

## RPC methods

| Method | Role |
| --- | --- |
| `initialize` | Open vault + SQLite, load provider packages |
| `shutdown` | Close stores and exit |
| `session.new` / `list` / `load` / `resume` / `delete` | Session lifecycle |
| `session.prompt` / `cancel` / `steer` / `compact` | Run control + manual compact |
| `run.resume` | Resume a suspended durable run (decision/decisions + `expectedVersion` CAS) |
| `provider.list` / `provider.status` | Auth descriptors, no secrets |
| `model.list` / `model.search` | Featured catalogs only |
| `credential.put` / `oauthStart` / `oauthPoll` / `delete` | Vault (+ keychain when available) |
| `agentProfile.register` / `list` | Host-registered `AgentDefinition`s |
| `skill.register` / `skill.list` | Kernel skill registry; catalog is name+description only (progressive) |
| `command.register` / `command.dispatch` | Host commands; drivers injected daemon-side, never from RPC |

Live runs emit notifications `{ "method": "event", "params": { "sessionId", "event" } }` with redacted Prism `AgentEvent`s. Chat never emits tool/permission variants; the union still includes them for Phase 29.

`SubscribeOptions.maxQueuedEvents` is 256 with `overflow: "drop_oldest"`.

## Profiles

The daemon does not hard-code Chat. Clay (`@clay/chat`) registers profiles through `agentProfile.register`. Omitted `tools`/`skills` stay fail-closed (none). Named missing tools throw before any provider turn.

## Credentials

Encrypted file vault is the source of truth. OS keychain is used when the secret service answers; a locked/denied keychain fails closed at initialize (Prism 0.5 typed `CredentialStoreLockedError` — never treated as an empty vault), while an unavailable backend degrades to vault-only; there is no plaintext fallback. `credential.put` never echoes the secret. Logs and errors run through secret-shape redaction.

Native addon: the `@arnilo/prism-core/sessions/sqlite` subpath uses `better-sqlite3` (pinned directly in this package). If install scripts are blocked, run `npm rebuild better-sqlite3` in this directory.

## Chat honesty

Chat is prompt/response only: **no tools and no sandbox**. The daemon does not claim filesystem, shell, or network isolation for Chat. Coding profiles are a different shape: they list Prism coding tools (`shell`, `read`, `write`, `edit`, `repo_list`, `repo_search`, `glob`, `delete`, `move`, optional `ask_user_decision`), and tool execution performs real host operations under the Clay acceptance policy — **trusted subprocess authority, never a sandbox** (no OS confinement is claimed).

## Coding tools

Coding profiles build the nine Prism coding tools per session. `read`/`write`/`edit` route through daemon→server reverse RPC (`document.read`/`document.write`/`document.mkdir`/`document.stat`), so the Rust document registry stays authoritative: open dirty buffers win over disk, writes go through the server lease/version path, and user-held leases fail closed. `shell`/list/search/glob/delete/move run against the session workspace root.

Acceptance policy (decision 2026-08-30-2157): inside-workspace mutations run free; reads are free everywhere; outside-workspace mutations require host approval or per-session full autonomy (`session.new` `fullAutonomy`, default **off**). The server's Phase 1 `approval.request` handler fails closed; the approval UI lands with the Phase 2 agent panel.

`ask_user_decision` requires a host ask callback; in Phase 1 it routes to the server and fails closed there. D1 behavior (omitted `allowCustom` persists as `false`; resume accepts `selectedId`) is covered by daemon tests.

## Durable runs

Coding sessions prompt with `runState: { checkpoints, definitionRevision: "clay-agent.1", interruptBeforeTool: true }` against the SQLite checkpoint store — every tool call suspends before its side effect. Chat sessions stay non-durable. A suspended `session.prompt` reply carries `status: "suspended"`, `runId`, `version`, and redacted `pendingDecisions` (scope + `argumentsHash` only, never raw arguments). `run.resume { sessionId, runId, expectedVersion, decision | decisions[] }` maps to Prism `resumeAgentRun`; batch outcomes are `allow_once` / `allow_for_run` / `reject_once` / `reject_for_run`. Decision shape is validated before any checkpoint I/O; stale `expectedVersion` or revision/fingerprint mismatch fails closed with no side effect, and a dispatched tool is never auto-replayed.

## Skills and commands

`skill.register { name, description?, instructions?, toolNames?, metadata? }` stores inert skill data on the kernel skill registry; duplicate names fail closed. `skill.list` returns the progressive catalog (name + description only — full `instructions` never leak over the wire). A profile that lists skills gets the Prism `load_skill` tool so the model can pull full instructions on demand; a skill whose `toolNames` includes an inactive tool throws before any provider turn.

`command.register { name, handler?, description?, parameters?, metadata? }` stores a `CommandDefinition` whose `execute` routes to a host-side handler: `startRun`, `startWorkflow`, or `steer`. `command.dispatch { name, args?, sessionId? }` runs it with `CommandDrivers` injected at dispatch time — never accepted over RPC. Drivers appear only when a live session is present (otherwise the context key is omitted). `startWorkflow` returns the Phase 5 "not in this phase" error. Unknown command names and unknown handlers fail closed.

## Compaction and observational memory

Named strategies on the kernel: `default` (local), `llm` (provider summary), `om` (folded observational memory). `session.compact` `{ sessionId, strategy?, compactAfterTokens? }` runs a manual compact; an in-flight run fails closed. `session.prompt` may pass `compaction` as a strategy name for that run. OM attaches only when the profile or `session.new` sets `observationalMemory: true` — Chat stays tool-free. Worker models come from host config, never the session model (`requireExplicitModel`). `compactAfterTokens` default **80000** (decision 2158; Prism package default 81000); a positive-integer `compactAfterTokens` on `session.compact` overrides the OM auto-compaction threshold for that session (runtime settings provider). Recall is exact-id only and is not auto-injected into the current run.


## Pins

Exact `0.5.1` for `@arnilo/prism`, `@arnilo/prism-core`,
`@arnilo/prism-providers`, `@arnilo/prism-coding-tools`,
`@arnilo/prism-web-tools`, `@arnilo/prism-memory`, and `@arnilo/prism-mcp`,
plus exact `better-sqlite3@13.0.3` and `playwright-core@1.61.0` (CDP
composition for Obscura only; never browser launch). Imports use family
subpaths only
(`@arnilo/prism-core/credentials/node`, `@arnilo/prism-core/sessions/sqlite`,
`@arnilo/prism-core/validation/json-schema`,
`@arnilo/prism-providers/<adapter>`, `@arnilo/prism-web-tools/obscura`,
`@arnilo/prism-web-tools/browser`, and
`@arnilo/prism-coding-tools/agent`); retired 0.3 package names and the 27
exports removed in 0.5 (`docs/migrate-to-0.5.md` §3) must not reappear. No
ACP, AG-UI, office, coding-agent, or Antigravity dependencies. MCP is a
package-declared bridge (`@arnilo/prism-mcp`, transitive
`@modelcontextprotocol/client` + `/server` 2.0.0), never a first-party
agent bus — clay-agent never imports SDK modules. hyper/commandcode adapters
are loaded explicitly with the other providers. The `/brave`, `/exa`, and
`/firecrawl` subpaths are not imported — direct Brave/Exa/Firecrawl
providers are rejected for Phase 1. Antigravity lands in Phase 6.

## MCP and Obscura (fail-closed)

- **MCP**: the daemon connects only servers the Clay server supplies in the
  `initialize` allow-list (`mcpAllowList`), validated fail-closed — canonical
  (absolute) executable, literal argv, explicit env names, bounded counts.
  No PATH search, no package-JS argv. Empty allow-list connects nothing.
  Validation errors fail closed; connection failures hide the tools.
- **Obscura**: host-owned (decision 2159). The binary resolves from
  `CLAY_OBSCURA_BIN`, `PATH`, or `/usr/local/bin/obscura`; absent binary =
  hidden capability, never an error. Present binary: the daemon owns
  `obscura serve` (CDP on 127.0.0.1 only) + `obscura mcp` children, exposes
  the complete advertised MCP surface with the `obscura_` prefix plus
  Prism browser tools, and kills both children on shutdown. Insecure flags
  are rejected; nothing is called sandboxed. Capabilities activate lazily on
  the first coding session — never on daemon import or Chat initialize.

## Upgrade Prism

1. Read `docs/migrate-to-0.5.md` (0.5.x: lockstep cut, MCP SDK v2 module
   move, model-aware thinking effort, 27 removed exports) and the changelog
   for the target line.
2. Bump the family pins in `package.json` together. Do not mix versions or
   reintroduce retired `@arnilo/prism-provider-*` / `-credentials-node` /
   `-session-store-sqlite` / `-tool-validator-json-schema` names or the 27
   exports removed in 0.5.
3. `npm install` in this directory. Rebuild `better-sqlite3` if install scripts
   were skipped.
4. `npm test` here, then `cargo test --test protocol agent_protocol`.
5. Confirm `package.json` still has no ACP, AG-UI, office, coding-agent, or
   Antigravity deps, and that MCP appears only as the `@arnilo/prism-mcp`
   bridge (never `@modelcontextprotocol/*` direct, `prism-acp`,
   `prism-ag-ui`, or retired names).
6. Update the version strings in this README if the pin changed.
