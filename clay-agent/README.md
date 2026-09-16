# clay-agent

Clay-owned Node >= 22 child that hosts Prism **0.7.0**. Not a Clay JS package.
Packages never spawn or speak to this process; the Clay server does.

## Spawn

```text
node dist/main.js --data-dir DIR [--mock]
```

- Requires Node >= 22 (Prism 0.6 raised the floor; Node 20 is unsupported). Exits non-zero with a clear message otherwise.
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

`session.prompt` accepts an optional `toolNames: string[]` (Prism 0.7 `RunOptions.toolNames`): omitted = full registry, `[]` = no tools for that run, a list = that subset. Non-array / non-string entries fail closed with `-32602`; unknown names fail closed before any provider turn, and a resumed durable run intersects with its recorded grant so it cannot widen it. The Rust server omits the field (full registry) until a caller needs the narrowing.

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

## Supervisor children (spawn_agent / wait_agent / cancel_agent)

Coding sessions (plan 122) additionally carry Prism 0.7's host-owned supervisor tools. The catalog is exactly two child ids — `test` and `validation` — and the closed spawn schema (`childId`, `input`, optional `threadId`, `mode: "sync" | "async"`) cannot extend it, supply child tools, or widen identity. Children are isolated Prism sessions on the parent's provider/model, rebuilt with the parent's coding tools bound to the **parent** `sessionId` + workspace root, so document ops stay on the daemon→server registry; they never receive spawn tools (no recursion) and their identity narrows from the host run identity. Sync spawn blocks the parent tool call; async returns `{ delegationId, status: "running" }` for `wait_agent`/`cancel_agent`. Parent-run abort (including `session.cancel`) propagates to running children. Handles are **in-process**: a daemon restart forgets them, and a stale `delegationId` is a plain tool error (`wait_agent`/`cancel_agent` fail closed), never a cross-session resume. No git worktrees yet — children share the parent workspace, so parallel children can collide on files.

## Skills and commands
`skill.register { name, description?, instructions?, toolNames?, metadata? }` stores inert skill data on the kernel skill registry; duplicate names fail closed. `skill.list` returns the progressive catalog (name + description only — full `instructions` never leak over the wire). A profile that lists skills gets the Prism `load_skill` tool so the model can pull full instructions on demand; a skill whose `toolNames` includes an inactive tool throws before any provider turn.

`command.register { name, handler?, description?, parameters?, metadata? }` stores a `CommandDefinition` whose `execute` routes to a host-side handler: `startRun`, `startWorkflow`, or `steer`. `command.dispatch { name, args?, sessionId? }` runs it with `CommandDrivers` injected at dispatch time — never accepted over RPC. Drivers appear only when a live session is present (otherwise the context key is omitted). `startWorkflow` returns the Phase 5 "not in this phase" error. Unknown command names and unknown handlers fail closed.

## Compaction and observational memory

Named strategies on the kernel: `default` (local), `llm` (provider summary), `om` (folded observational memory). `session.compact` `{ sessionId, strategy?, compactAfterTokens? }` runs a manual compact; an in-flight run fails closed. `session.prompt` may pass `compaction` as a strategy name for that run. OM attaches only when the profile or `session.new` sets `observationalMemory: true` — Chat stays tool-free. Worker models come from host config, never the session model (`requireExplicitModel`). `compactAfterTokens` default **80000** (decision 2158; Prism package default 81000); a positive-integer `compactAfterTokens` on `session.compact` overrides the OM auto-compaction threshold for that session (runtime settings provider). Recall is exact-id only and is not auto-injected into the current run.

Auto-compaction (plan 121 follow-up) is armed on exactly the sessions that run the attention compiler: coding profiles whose resolved model declares a context window. Prism runs `autoCompact` once per prompt, before provider turns, only when `AgentConfig.compaction` carries a trigger, so the daemon arms one composed custom trigger that fires when the compiler reports `truncated` on two consecutive turns (stubs can no longer hold the request), when the assembled input reaches the compiler's `compactRatio` (Prism default 0.9 of the resolved input cap), or when it reaches the absolute `run.setOptions.compactAfterTokens` ceiling (default 800000 — the only gate that can fire first on very large windows). The automatic pass uses Prism's local deterministic strategy and the host secret list: no provider call, so a provider outage can never fail a run at assembly. `run.setOptions.compaction` keeps governing explicit `session.compact` / `/compact` and per-run `compaction`. Limit-less models (Ollama discovery, pass-through ids) and Chat keep no implicit branch rewrite; a prompt that is itself over the cap still fails closed with Prism's `AttentionBudgetError`.


## Knowledge bases (wiki, graft)

`knowledge.setOptions { workspaceRoot, wiki?, graft?, graftMode?, graftCliPath?, graftDeepModel?, qmdPath? }` opts a workspace in or out of the knowledge extensions (decision 2156). `wiki: true` loads `@arnilo/prism-memory/wiki`: the `/wiki-init`, `/wiki-refresh`, `/wiki-lint`, `/wiki-ingest` commands, the `wiki_search`/`wiki_read_page`/`wiki_record_insight`/`wiki_ingest` tools, and the two wiki skills. Ingested sources are untrusted: they stage into the raw layer (`raw/ingest/<id>`, workspace-relative) labeled `untrusted_external`, path escapes fail closed, and URL ingest runs the host-owned Obscura CLI only when it resolves (`--dump markdown`, staged as `source.md`) — a missing hook fails closed by name.

`graft: true` loads the `@arnilo/prism-memory/graft` extension: the six pull tools, the `/graft`-family commands, and the graft skill. CLI resolution fails closed before load (absent CLI ⇒ option off, tools hidden). `/graft-init` is non-interactive by design (`initYes: true`; Prism passes `--yes` plus `--no-global --no-mcp --no-hooks --no-statusline`, never user-level state), and `/graft-build-deep` refuses before spawning unless a deep model is configured. `graftDeepModel { provider, model, apiKey?, baseUrl? }` supplies it: `provider` is graft's own id (`openai`/`anthropic`/`litellm`/`orcarouter`), an omitted `apiKey` reads the stored credential for that provider (the key never has to appear in `init.js`), an inline key joins the redactor set, and the key reaches the child only as `GRAFT_API_KEY` in its environment — never argv. A changed deep model rebinds the extension in place; a malformed shape, a deep model without `graft: true`, or a key that resolves nowhere fails closed with `-32602`.

## Pins

Exact `0.7.0` for `@arnilo/prism`, `@arnilo/prism-core`,
`@arnilo/prism-providers`, `@arnilo/prism-coding-tools`,
`@arnilo/prism-web-tools`, `@arnilo/prism-memory`, and `@arnilo/prism-mcp`,
plus exact `better-sqlite3@13.0.3` and `playwright-core@1.63.0` (CDP
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

1. Read `docs/migrate-to-0.6.md` (Node >= 22 floor, third-party floors, MCP
   SDK v2 module move) and `docs/migrate-to-0.7.md` (lockstep bump, ACP/router
   refusals, opt-ins) plus the changelog for the target line.
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
