# Agent Host

- First-party agents run in a Clay-owned Node >= 20 child (`clay-agent`) via Prism `createAgent` / `createAgentSession` / `AgentEvent`. Never inside `deno_core`. ACP remains out of the first-party path.
- `clay-agent` keeps its Clay-owned internal protocol to the Rust server. The server adapts bounded events into AG-UI for the React-facing stream over Tauri channels; AG-UI is not a daemon replacement and does not add a localhost listener.
- Clay server owns session identity, transcript, selected profile/provider/model, and mutation. React paints and forwards composer input. Typing/paint never wait on the daemon.
- Package JS cannot spawn or speak to the daemon. Core spawn, not a package-triggered process grant.
- Credentials: Clay vault + Prism credential resolvers. No `process.env`, no secrets in events/logs/menus/a11y.
- Agent profiles are `AgentDefinition`s registered by packages, not compiled core stubs. `@clay/chat` registers Chat (no tools). `@clay/coding-agent` (Phase 29) registers Coding Agent. Later Work/PA/Research are more packages on the same kernel, store, and `agent.*` APIs.
- Coding-agent CLI parity uses Prism tool factories plus Clay `ReadOperations`/`WriteOperations`/`EditOperations` over the document registry. Do not use `createAcpFilesystemOperations` or ACP `fs/*`.
- Provider/model/agent/setup pickers are Command Centre session kinds, not extra dropdown widgets. Entry-surface buttons invoke the same commands.
- Pin `@arnilo/prism*` and AG-UI packages to exact reviewed versions. Decisions: `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md` (host/no ACP, partially superseded for React-facing AG-UI), `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md` (package-owned profiles/landings), `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md` (AG-UI at the frontend transport boundary).
