# Clay Code Wiki

## Modules

- [Behavior Manifests](modules/behavior-manifests.md): Server-issued inert behavior schema, routing policies, validation rules, editor declarations, and tests.
- [Protocol Codec](modules/protocol-codec.md): IPC protocol messages, `rkyv` serialization, length-prefixed framing, validation, and tests.
- [Server IPC Skeleton](modules/server-ipc-skeleton.md): Tokio local IPC server lifecycle across Unix sockets and Windows named pipes, connection handshake, shared document state, versioned edit dispatch, resync responses, and tests.
- [Server Document State](modules/server-document-state.md): Server-owned canonical `crop::Rope` text, version enforcement, lease validation, region-lock enforcement, dirty-state hooks, acknowledgement/resync generation, and tests.
- [Server File Workspace Model](modules/server-file-workspace.md): Workspace roots, canonical path registry, duplicate-open identity, file-backed dirty state, and server/client authority boundaries.
- [Client Snapshot Bootstrap](modules/client-snapshot-bootstrap.md): Native app client bootstrap over platform IPC, server snapshot loading, editable/read-only access storage, behavior manifest storage, editor state reset, and tests.
- [Clay JS Facade Skeleton](modules/clay-js-facade-skeleton.md): Planned Clay JavaScript/TypeScript facade source tree, domain modules, typed planned stubs, authority boundaries, and validation tests.
- [Clay JS Documentation Registry](modules/clay-js-doc-registry.md): Markdown-derived generated Clay JS API registry artifacts, stale checks, update command, validation rules, and tests.

## Flows

- [Client Behavior Routing](flows/client-behavior-routing.md): Atomic client manifest installation, hot-path key classification, local edit routing, and server-intent routing without synchronous IPC.
- [Client Edit Emission](flows/client-edit-emission.md): Local editor mutations, behavior-manifest-gated edit events, optimistic base-version assignment, bounded client edit queueing, and non-blocking Masonry forwarding.
- [Client/Server Edit Acknowledgement Flow](flows/client-server-edit-ack.md): Open client IPC session, background edit sending, asynchronous acknowledgements/rejections, EventLoopProxy GUI event routing, visible connection/access/version status, resync handling, and non-blocking GUI wiring.
- [Versioned Text Synchronization](flows/versioned-text-synchronization.md): Client shadow state, pending transactions, confirmed/optimistic versions, stale rejection, and snapshot resync.
- [Document Leases and Region Locks](flows/document-leases-and-region-locks.md): Editable lease ownership, read-only observers, lease validation, and region-lock conflict enforcement.
