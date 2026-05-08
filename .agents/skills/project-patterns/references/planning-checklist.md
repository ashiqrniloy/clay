# Clay Planning Checklist

Use this before writing or updating any Clay plan.

## Required Checks

- **Decision alignment:** Identify relevant decision logs and roadmap sections.
- **Authority boundary:** State which component owns state, behavior, execution, persistence, and validation.
- **Client hot path:** Confirm ordinary typing/rendering does not block on IPC, server work, JavaScript, AI, file IO, or full-document serialization.
- **Server authority:** Preserve server ownership of canonical documents, versions, transactions, file/workspace authority, extension execution, leases, and locks.
- **Behavior manifest:** If a feature changes hot-path editor behavior, decide whether it belongs in a server-issued behavior manifest, server-first command, or later phase.
- **Documentation as code:** Public programmatic behavior must be exposed and documented through Clay JS APIs. Server-side Rust public functions must have Clay JS APIs; functions that should remain internal should be private or `pub(crate)`. Clay JS APIs must include user-facing names, key binding metadata, custom properties for behavior-changing settings, Markdown docs, generated registry coverage, and lookup access. Internal implementation details belong in the project wiki, not the public registry.
- **Clay JS API naming:** Apply `clay-js-api-naming.md` when designing or documenting APIs. Keep callable exports concise and behavior-oriented, distinguish them from stable registry IDs and `user_facing_name`, preserve server/client authority markers for editor-core APIs, and require package API provenance prefixes.
- **Configuration:** User configuration starts at `~/.config/clay/init.js`; each configuration option is a documented Clay JS API, not an undocumented config key.
- **Security:** Say what authority is not introduced: file IO, network, script execution, WASM, AI mutation, remote listener, shell, etc.
- **Performance:** Prefer deltas, bounded queues, per-document ordering, cancellable background work, and viewport-bounded rendering.
- **Phase boundary:** If enforcement is deferred, describe it as a scoped limitation of the approved architecture, not a competing model.
- **Decision-log feedback:** After logging a decision, update this skill's reference patterns if the decision creates reusable planning guidance.
