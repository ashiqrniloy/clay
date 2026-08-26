# Tauri React Client

- Target desktop client is Tauri v2 with React, TypeScript, Vite, React Router Data Mode using an in-memory router, and CodeMirror 6.
- Keep Clay server separate and authoritative. Tauri is a narrow local presentation/OS bridge; it does not absorb documents, workspaces, packages, language services, or `deno_core` authority.
- Preserve existing length-prefixed `rkyv` server transport behind the Tauri Rust core. React receives bounded JSON-compatible DTOs through typed commands/channels; IDs that may exceed JavaScript integer precision use strings.
- Ordinary typing applies locally in CodeMirror and queues bounded ordered deltas asynchronously. React rendering, Tauri IPC, server work, package JavaScript, file IO, and AI never block the keystroke-to-local-paint path.
- CodeMirror owns local text, viewport, incremental position indexing, and inert syntax projection. Package-selected parsers and executable syntax-management behavior stay in bounded server syntax sessions; the frontend consumes explicit atomic viewport patches.
- Do not move arbitrary package parser/highlighter JavaScript into the webview to reduce latency. Client-local parsing requires a separate measured decision covering authority, artifact trust, server/headless parity, duplicate trees, and merge precedence.
- Main webview gets narrow Clay commands only. Do not grant broad Tauri filesystem, shell, process, or network plugin capabilities. Authorization remains server-side and provenance-aware.
- Package UI remains inert and declarative by default, reconciled by stable node ID through a Clay-owned React component registry. First-party trusted components may be compiled with the frontend; arbitrary third-party UI must be isolated in a separate sandboxed surface with no direct Tauri IPC.
- Theme packages are validated data. One frontend theme runtime maps semantic tokens to CSS custom properties and CodeMirror styles; packages do not inject uncontrolled host CSS.
- AG-UI is the React-facing agent event/state protocol over a custom Tauri channel transport. The Prism daemon remains Clay-owned and speaks its existing internal server protocol; ACP remains out of the first-party path.
- TauRPC is optional replaceable bridge glue, accepted only after an exact-version compatibility spike. It does not replace Clay's server protocol.
- Migration ends with measured current-feature parity and native-client deletion. Do not maintain Masonry and React as permanent production clients.
- Decision sources: `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`, `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`.
