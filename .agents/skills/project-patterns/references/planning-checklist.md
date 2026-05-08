# Clay Planning Checklist

Use this before writing or updating any Clay plan.

## Required Checks

- **Decision alignment:** Identify relevant decision logs and roadmap sections.
- **Authority boundary:** State which component owns state, behavior, execution, persistence, and validation.
- **Client hot path:** Confirm ordinary typing/rendering does not block on IPC, server work, JavaScript, AI, file IO, or full-document serialization.
- **Server authority:** Preserve server ownership of canonical documents, versions, transactions, file/workspace authority, extension execution, leases, and locks.
- **Behavior manifest:** If a feature changes hot-path editor behavior, decide whether it belongs in a server-issued behavior manifest, server-first command, or later phase.
- **Documentation as code:** Any new public protocol/API/command/manifest/permission/tool surface must have a registry/documentation path and tests or acceptance criteria.
- **Security:** Say what authority is not introduced: file IO, network, script execution, WASM, AI mutation, remote listener, shell, etc.
- **Performance:** Prefer deltas, bounded queues, per-document ordering, cancellable background work, and viewport-bounded rendering.
- **Phase boundary:** If enforcement is deferred, describe it as a scoped limitation of the approved architecture, not a competing model.
- **Decision-log feedback:** After logging a decision, update this skill's reference patterns if the decision creates reusable planning guidance.
