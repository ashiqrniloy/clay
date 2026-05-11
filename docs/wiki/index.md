# Clay Code Wiki

## Modules

- [Protocol Codec](modules/protocol-codec.md): IPC protocol messages, `rkyv` serialization, length-prefixed framing, validation, and tests.
- [Server IPC Skeleton](modules/server-ipc-skeleton.md): Tokio Unix Domain Socket server lifecycle, connection handshake, shared document state, versioned edit dispatch, resync responses, and tests.
- [Server Document State](modules/server-document-state.md): Server-owned canonical `crop::Rope` text, version enforcement, lease validation, region-lock enforcement, acknowledgement/resync generation, and tests.
- [Client Snapshot Bootstrap](modules/client-snapshot-bootstrap.md): Native app client bootstrap, server snapshot loading, editable/read-only access storage, behavior manifest storage, editor state reset, and tests.

## Flows

- [Client Edit Emission](flows/client-edit-emission.md): Local editor mutations, behavior-manifest-gated edit events, optimistic base-version assignment, bounded client edit queueing, and non-blocking Masonry forwarding.
- [Client/Server Edit Acknowledgement Flow](flows/client-server-edit-ack.md): Open client IPC session, background edit sending, asynchronous acknowledgements/rejections, resync handling, and non-blocking GUI wiring.
- [Versioned Text Synchronization](flows/versioned-text-synchronization.md): Client shadow state, pending transactions, confirmed/optimistic versions, stale rejection, and snapshot resync.
- [Document Leases and Region Locks](flows/document-leases-and-region-locks.md): Editable lease ownership, read-only observers, lease validation, and region-lock conflict enforcement.
