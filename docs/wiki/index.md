# Clay Code Wiki

## Modules

- [Protocol Codec](modules/protocol-codec.md): IPC protocol messages, `rkyv` serialization, length-prefixed framing, validation, and tests.
- [Server IPC Skeleton](modules/server-ipc-skeleton.md): Tokio Unix Domain Socket server lifecycle, connection handshake, document state, edit acknowledgements, and tests.
- [Server Document State](modules/server-document-state.md): Server-owned canonical text, edit validation, version increments, acknowledgement generation, and deferred synchronization boundaries.
- [Client Snapshot Bootstrap](modules/client-snapshot-bootstrap.md): Native app client bootstrap, server snapshot loading, behavior manifest storage, editor state reset, and tests.

## Flows

- [Client Edit Emission](flows/client-edit-emission.md): Local editor mutations, behavior-manifest-gated edit events, bounded client edit queueing, and non-blocking Masonry forwarding.
- [Client/Server Edit Acknowledgement Flow](flows/client-server-edit-ack.md): Open client IPC session, background edit sending, asynchronous server acknowledgements, and non-blocking GUI wiring.
