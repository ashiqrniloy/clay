# clay-agent Daemon

## Source

- `clay-agent/src/main.ts`
- `clay-agent/src/host.ts`
- `clay-agent/src/providers.ts`
- `clay-agent/src/rpc.ts`
- `clay-agent/src/redact.ts`
- `clay-agent/README.md`
- `clay-agent/src/__tests__/host.test.ts`
- `clay-agent/src/__tests__/rpc.test.ts`

## Overview

`clay-agent` is Clay’s Node >= 20 child process that hosts Prism 0.3.0. It is
**not** a Clay JS package and is not loaded by Deno. `AgentHost` in
`src/server/agent.rs` lazy-spawns one daemon per server. Package JS cannot
spawn or speak to it.

## Responsibilities

- Stdio JSON-RPC wrapping `createAgent` / `createAgentSession` / `AgentEvent`.
- SQLite session store under `--data-dir/sessions.sqlite`.
- Encrypted credential vault under `--data-dir/credentials.vault`; OS keychain
  when the secret service answers. No plaintext fallback.
- Load first-party Prism 0.3.0 provider packages through the extension kernel
  with a stored credential resolver (never `process.env`).
- Host-registered `AgentDefinition`s. Chat is not compiled in.

## How It Works

1. `main.ts` refuses Node < 20, requires `--data-dir`, reads NDJSON JSON-RPC.
2. `initialize { passphrase }` opens the vault (exit 1 if unreadable) and SQLite.
3. `--mock` registers `createMockProvider`; production loads `providers.ts`.
4. Azure/Bedrock/Vertex packages are installed but only register auth stubs
   until a later task supplies endpoint/region/project.
5. `session.prompt` uses `session.stream` with `maxQueuedEvents: 256` and
   `overflow: "drop_oldest"`. Events go out as `{ method: "event", params }`.
6. Profiles must be re-registered after a daemon restart; session rows survive.

## Spawn

```text
node clay-agent/dist/main.js --data-dir DIR [--mock]
```

First request:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"passphrase":"…"}}
```

## Invariants

- Frames > 1 MiB fail closed.
- Secrets never appear in RPC results, events, or logs.
- Omitted tools/skills activate none; unknown names throw before a provider turn.
- No ACP, AG-UI, coding-agent, MCP, browser, or web-tools dependencies.

## Tests

```text
cd clay-agent && npm test
```

Covers mock prompt/persist/resume, cancel, oversize frames, secret redaction,
missing tools, and unreadable-vault process exit.

## Related

- [Phase 25 Agent Host and Pane Content Primitive Review](phase25-agent-host-primitive-review.md)
- [Agent Host project pattern](../../../.agents/skills/project-patterns/references/agent-host.md)
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
