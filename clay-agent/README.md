# clay-agent

Clay-owned Node >= 20 child that hosts Prism **0.3.0**. Not a Clay JS package.
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
| `session.prompt` / `cancel` / `steer` | Run control |
| `provider.list` / `provider.status` | Auth descriptors, no secrets |
| `model.list` / `model.search` | Featured catalogs only |
| `credential.put` / `oauthStart` / `oauthPoll` / `delete` | Vault (+ keychain when available) |
| `agentProfile.register` / `list` | Host-registered `AgentDefinition`s |

Live runs emit notifications `{ "method": "event", "params": { "sessionId", "event" } }` with redacted Prism `AgentEvent`s. Chat never emits tool/permission variants; the union still includes them for Phase 29.

`SubscribeOptions.maxQueuedEvents` is 256 with `overflow: "drop_oldest"`.

## Profiles

The daemon does not hard-code Chat. Clay (`@clay/chat`) registers profiles through `agentProfile.register`. Omitted `tools`/`skills` stay fail-closed (none). Named missing tools throw before any provider turn.

## Credentials

Encrypted file vault is the source of truth. OS keychain is used when the secret service answers; there is no plaintext fallback. `credential.put` never echoes the secret. Logs and errors run through secret-shape redaction.

Native addon: `@arnilo/prism-session-store-sqlite` uses `better-sqlite3`. If install scripts are blocked, run `npm rebuild better-sqlite3` in this directory.

## Chat honesty

Chat is prompt/response only: **no tools and no sandbox**. The daemon does not claim filesystem, shell, or network isolation for Chat. Phase 29 coding-agent is a different profile.

## Pins

Exact `0.3.0` for `@arnilo/prism` and first-party provider/session/credential packages. No ACP, AG-UI, coding-agent, MCP, browser, or web-tools dependencies.

## Upgrade Prism

1. Read the Prism changelog for the target `0.3.x` (or later) line.
2. Bump every `@arnilo/prism*` pin in `package.json` together. Do not mix versions.
3. `npm install` in this directory. Rebuild `better-sqlite3` if install scripts were skipped.
4. `npm test` here, then `cargo test --test protocol agent_protocol`.
5. Confirm `package.json` still has no ACP, AG-UI, coding-agent, MCP, browser, or web-tools deps.
6. Update the version strings in this README if the pin changed.
