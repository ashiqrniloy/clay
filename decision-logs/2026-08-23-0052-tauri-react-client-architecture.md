---
date: 2026-08-23 00:52
status: approved
decision_about: "Replace the Masonry client with a Tauri v2 React client"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Tauri v2 and React become Clay's client architecture

## Decision

Clay will replace its Masonry/Vello/Parley desktop client with a Tauri v2 client whose application UI is built with React and TypeScript. Clay will retain the separate Rust server, server-authoritative document and workspace model, length-prefixed `rkyv` server transport, two persistent `deno_core` package-runtime trust domains, package graph, Prism-based `clay-agent` daemon, and package-owned product surfaces.

The target client uses Vite, React Router Data Mode with an in-memory router, CodeMirror 6 as the primary text/code editor, accessible headless React primitives, CSS custom properties generated from validated Clay theme tokens, and typed Tauri commands/channels. TauRPC may be used only as replaceable React↔Tauri bridge glue after a pinned compatibility spike. AG-UI becomes the React-facing agent event/state protocol through a custom Tauri channel transport; ACP remains out of the first-party path.

Migration is complete only when the Tauri/React client reaches verified parity with every currently implemented Clay user workflow, security boundary, accessibility contract, and performance invariant. The native client is removed after parity, not maintained as a second production frontend.

## Context

Clay's Rust server and package architecture have matured substantially, but the native application client has required Clay to implement every widget, editor surface, layout behavior, accessibility projection, visual state, and rich-content renderer over a foundational Masonry/Vello stack. Despite repeated design iterations, the UI remains costly to refine and does not meet the desired quality. The same client boundary also makes adoption of TypeScript-first UI protocols and ecosystems—especially AG-UI, rich document rendering, notebooks, and flexible package presentation—needlessly difficult.

The repository already has a clean server/client boundary. Approximately 77,000 Rust lines belong to the server, while the replaceable native editor, Masonry roots, and shell account for roughly 45,000–60,000 lines. A client-only pivot therefore preserves most canonical state, package, language, protocol, security, and agent-host work instead of rewriting Clay as a monolithic web application.

The user approved the recommended architecture and requested a replacement roadmap covering the complete port and repository-wide documentation consistency.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user said, "I agree with the recommended approach" and requested that `roadmap.md` be replaced with phases implementing the architecture shift through current feature parity and documentation consistency.

## Alternatives Considered

1. **Continue refining the Masonry/Vello client** — Rejected. It preserves current code but continues the high cost of bespoke controls, accessibility, editor chrome, rich content, and visual polish that motivated the change.
2. **Adopt Electron/Code-OSS** — Rejected for the current goal. It offers a uniform Chromium/Node environment and a stronger path toward broad VSIX compatibility, but adds distribution/runtime weight and weakens the direct fit with Clay's existing Rust core. Revisit only if full Node VSIX compatibility becomes a primary requirement.
3. **Use Monaco as the primary editor** — Rejected as the default. Monaco offers more conventional IDE behavior out of the box but does not run VS Code extensions and is less adaptable for notebook cells and mixed document surfaces. CodeMirror 6 better fits Clay's existing Rust-owned language services and multi-format goals.
4. **Use Vue instead of React** — Rejected. Vue is viable, but React has the stronger AG-UI, accessible-component, editor, notebook, and AI presentation ecosystem for Clay's stated direction.
5. **Fold the Rust server into the Tauri process** — Rejected. It would regress remote/container use, crash isolation, headless operation, multi-client behavior, and the existing server-authoritative design.
6. **Run arbitrary third-party React modules in the main webview** — Rejected as the default package model. Tauri capabilities bind to windows/webviews rather than individual JavaScript packages, so same-realm third-party code would inherit renderer IPC authority. Third-party UI remains declarative by default or runs in a separately sandboxed surface.
7. **Use HTTP/SSE solely to adopt AG-UI** — Rejected. AG-UI is transport-agnostic; a custom `AbstractAgent` transport over Tauri channels avoids an unnecessary localhost listener.
8. **Keep both native and web clients indefinitely** — Rejected. A bounded migration overlap is necessary, but long-lived dual production clients would double UI, accessibility, testing, and documentation cost.

## Rationale and Evidence

- Tauri v2 separates the privileged Rust core from webview processes and exposes asynchronous message-passing primitives through commands, events, and channels. Capability files restrict IPC access by window or webview; a webview without a matching capability has no IPC access.
- Tauri supports any frontend that builds to HTML/CSS/JavaScript, including React/Vite. Linux development requires WebKitGTK 4.1 and must remain Clay's blocking development/CI host.
- AG-UI is transport-agnostic and defines `run(input) -> Observable<BaseEvent>`, typed lifecycle/message/tool/state events, snapshots, and RFC 6902 state deltas. This permits a Tauri-channel transport without changing the Prism daemon into an HTTP service.
- CodeMirror 6 provides modular immutable editor state, transactions, extensions, decorations, completion, linting, view plugins, compartments, and merge support. These map well to Clay's optimistic client shadow and Rust-owned language intelligence.
- Monaco's official FAQ states that VS Code extensions do not run in Monaco. Full VSIX compatibility requires an extension host plus substantial VS Code API behavior, not merely a web editor.
- Tauri capabilities protect webview boundaries, not package identity. Clay must retain server-side package principal, capability, provenance, generation, and revocation validation and avoid broad filesystem/shell permissions in the main renderer.
- Existing project evidence shows a strong separable backend: `src/server/`, `src/protocol/`, `src/packages/`, `src/server/js_runtime/`, and `clay-agent/` remain reusable, while `src/masonry_*`, `src/editor/`, `src/shell/`, `src/driver/`, and native client launch code are migration targets.

## References

- [Tauri process model](https://v2.tauri.app/concept/process-model/) — privileged core and webview process separation.
- [Tauri IPC](https://v2.tauri.app/concept/inter-process-communication/) — asynchronous commands/events and serializable message boundaries.
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/) — window/webview capability ACLs.
- [Tauri calling Rust](https://v2.tauri.app/develop/calling-rust/) — command registration and frontend invocation.
- [Tauri sidecars](https://v2.tauri.app/develop/sidecar/) — bundled external binary lifecycle considerations.
- [CodeMirror system guide](https://codemirror.net/docs/guide/) — state, transactions, views, and extensions.
- [CodeMirror reference](https://codemirror.net/docs/ref/) — editor APIs and extension points.
- [AG-UI architecture](https://docs.ag-ui.com/concepts/architecture) — transport-agnostic event stream and client abstraction.
- [AG-UI state](https://docs.ag-ui.com/concepts/state) — snapshots and RFC 6902 deltas.
- [Monaco FAQ](https://github.com/microsoft/monaco-editor#faq) — VS Code extensions do not run in Monaco.
- [VS Code extension hosts](https://code.visualstudio.com/api/advanced-topics/extension-host) — Node, browser, and remote extension-host requirements.
- [VS Code web extensions](https://code.visualstudio.com/api/extension-guides/web-extensions) — browser worker restrictions and manifest entry points.
- [TauRPC repository](https://github.com/MatsDK/TauRPC) — generated trait-based Tauri bindings and channels.
- Context7 `/tauri-apps/tauri-docs` — Tauri v2 capability layout, command registration, Vite host configuration, and Linux prerequisites reviewed on 2026-08-23.
- `Cargo.toml` — current native client dependencies and local Masonry/AccessKit patches.
- `docs/development/architecture-ownership.md` — current server/client ownership and hot-path baseline.
- `docs/wiki/modules/embedded-js-runtime.md` — two persistent package-runtime trust domains.
- `docs/wiki/modules/server-driven-ui.md` — current inert SDUI and stable-identity contracts.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` — superseded only where it selects Masonry as renderer; declarative package UI and Clay-owned shell remain.
- `decision-logs/2026-07-29-1451-stable-identity-sdui-reconciliation.md` — superseded only where identity maps to Masonry widgets; stable SDUI node identity remains.
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md` — partially superseded: no ACP and one Prism daemon remain; AG-UI is now adopted at the React-facing transport boundary.
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md` — retained; product surfaces remain packages.

## Consequences

- `roadmap.md` is replaced with migration phases ending in verified feature parity, native-client deletion, and repository-wide documentation consistency.
- A new Tauri crate and React/Vite frontend are introduced while the root Rust server crate remains authoritative and separately runnable.
- React owns rendering, accessibility semantics, shell presentation, and local transient state. CodeMirror owns editor hot-path state. Rust continues to own canonical documents, workspace/file/process authority, packages, language services, persistence, and validation.
- Existing SDUI IDs, snapshots, updates, budgets, provenance, and inert command intents are retained but rendered through a React component registry.
- First-party trusted UI may use compiled React components. Third-party UI is declarative by default; arbitrary custom UI requires an isolated webview/iframe with a typed message boundary and no direct Tauri IPC.
- Themes become validated data consumed by one frontend theme runtime and projected to CSS custom properties and CodeMirror themes.
- The main webview receives only narrow Clay commands. Broad Tauri filesystem/shell/plugin capabilities remain denied.
- Existing native client code, Masonry/Vello/Parley/winit dependencies, AccessKit patches, native UI benchmarks, and native-only docs are deleted only after the parity ledger is fully satisfied.
- Full VSIX execution compatibility remains out of scope. Declarative themes, grammars, snippets, icons, and language configuration may be imported later.
- Revisit this decision if Linux WebKitGTK cannot meet measured editor/rendering/accessibility requirements, if full VSIX compatibility becomes primary, or if the compatibility spike shows Tauri cannot preserve Clay's required remote/server topology.
