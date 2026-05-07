# Authority Boundaries Pattern

## Core Rule

Clay uses server-authoritative documents with optimistic client shadows.

## Server Owns

- Canonical document ropes/state.
- Document versions and transaction ordering.
- Edit validation and correction.
- File/workspace authority and environment-specific operations.
- Open document registry.
- Editable leases and read-only observer state.
- Region/document/behavior/workspace locks.
- JavaScript extension execution and behavior definitions.
- AI/tool orchestration and mutation authority.

## Client Owns

- Native rendering and input handling.
- Masonry/Vello/Parley UI surface.
- Viewport, caret, selection, pointer, focus, local UI transient state.
- Local shadow rope/cache for immediate editing.
- Pending edit queue and client transaction IDs.
- Execution of server-issued hot-path behavior manifests.

## Document Access Pattern

- One editable lease per document.
- Other clients opening the same document are read-only observers.
- Lease transfer/release is explicit.
- Phase 3 may not enforce leases fully, but protocol and plan language should preserve the final model.

## Planning Guidance

- Do not describe the server as a stateless behavior service.
- Do not make the client the canonical owner for convenience.
- If a phase uses a simplified in-memory server document, call it a minimal server-canonical placeholder.
- Prefer per-document owner/actor boundaries over global document locks or global serialization.
