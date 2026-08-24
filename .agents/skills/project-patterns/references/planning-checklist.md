# Clay Planning Checklist

Use this before writing or updating any Clay plan.

## Required Checks

- **Mandatory UI skill stack:** If work touches UI, theme, typography, tokens, components, layout, SDUI, or accessibility, load `clay-ui` plus `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend` before reviewing or editing implementation. List every skill under each UI task's `Documentation Reviewed`; plan-level evidence does not substitute.
- **Decision alignment:** Identify relevant decision logs and roadmap sections.
- **Authority boundary:** State which component owns state, behavior, execution, persistence, and validation. Apply `tauri-react-client.md`: separate Rust server remains authoritative; Tauri is a narrow OS/transport bridge; React owns presentation; CodeMirror owns local editor hot-path state. Agent work uses `agent-host.md`: Prism remains in `clay-agent`, ACP stays out, and AG-UI is limited to the React-facing Tauri transport. Product landings and agent profiles are packages (`product-surfaces-are-packages.md`); core owns host primitives, not greeting copy.
- **Client hot path:** Confirm ordinary CodeMirror typing/rendering applies locally and does not block on React rerenders, Tauri/server IPC, server work, package JavaScript, AI, file IO, or full-document serialization.
- **Server authority:** Preserve server ownership of canonical documents, versions, transactions, file/workspace authority, extension execution, leases, and locks.
- **Behavior manifest:** If a feature changes hot-path editor behavior, decide whether it belongs in a server-issued behavior manifest, server-first command, or later phase.
- **Documentation as code:** Public programmatic behavior must be exposed and documented through Clay JS APIs. Server-side Rust public functions must have Clay JS APIs; functions that should remain internal should be private or `pub(crate)`. Clay JS APIs must include user-facing names, key binding metadata, custom properties for behavior-changing settings, Markdown docs, generated registry coverage, and lookup access. Internal implementation details belong in the project wiki, not the public registry.
- **Clay JS API naming:** Apply `clay-js-api-naming.md` when designing or documenting APIs. Keep callable exports concise and behavior-oriented, distinguish them from stable registry IDs and `user_facing_name`, preserve server/client authority markers for editor-core APIs, and require package API provenance prefixes.
- **Configuration:** User configuration starts at `~/.config/clay/init.js`; each configuration option is a documented Clay JS API, not an undocumented config key.
- **Security:** Say what authority is not introduced: file IO, network, script execution, WASM, AI mutation, remote listener, shell, etc.
- **Performance:** Prefer deltas, bounded queues, per-document ordering, cancellable background work, viewport-bounded rendering, code-split heavy web renderers, and explicit React render-count/bundle budgets.
- **Phase boundary:** If enforcement is deferred, describe it as a scoped limitation of the approved architecture, not a competing model.
- **Decision-log feedback:** After logging a decision, update this skill's reference patterns if the decision creates reusable planning guidance.
