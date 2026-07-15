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

## Built-in `core.*` Modes and Bounded Probing (Phase 18.9)

- Clay owns always-on built-in major modes `core.text` (universal fallback) and `core.code` (code-like extensions and any shebang), registered at server startup via `register_builtin_mode` with no `init.js` line and no `loadPackage` step. They grant no package authority.
- The `core.` and `clay.` mode-ID prefixes are reserved for Clay-owned built-ins; `register_mode`/`register_minor_mode` must reject them. Built-in manifests ship without an owning package (`select_behavior_manifest_for_document` bypasses package-record lookup on the `core.` prefix).
- Classification probing reads only a bounded constant prefix (`MAX_LEADING_CONTENT_BYTES = 512`) of an already-open document supplied by the open path — never a filesystem scan, directory walk, or package-supplied predicate. Oversize slices are treated as absent and fall to the remaining precedence ladder. The open path is the sole authority supplying shebang/leading-content slices.
- Mode-discovery commands (`clay.modes.listActiveModes`/`explainActiveMode`) are read-only `ServerFirst` built-ins with empty permissions resolved via `CommandExecutor::execute_discovery`; they carry no execution/document/workspace authority.
- Decision log source: `decision-logs/2026-07-01-0350-phase18-9-generic-text-code-fallback-modes-and-key-behavior.md`.

## External Process Authority

- Package-triggered external processes require a dedicated deny-by-default capability and an approved decision log; never silently compose them from package load, first-party trust, `shell`, or `filesystem`.
- Bind approval to package provenance, a fixed inert contribution, canonical executable, literal argv, explicit inherited-environment names, and known workspace roots. Runtime input selects only an already-approved contribution/root.
- Launch directly without a shell, clear environment by default, bound all I/O/time/concurrency, and terminate on revocation, reload, root removal, runtime replacement, or shutdown.
- Working directory and root-bound grants constrain Clay's API/audit identity, not the operating system. A same-user child may access other files, network, and processes; call it trusted subprocess authority, never sandboxed or workspace/filesystem confined.
- Keep process work asynchronous and outside typing, paint, layout, scroll, and local text-application paths.
- Decision log source: `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Planning Guidance

- Do not describe the server as a stateless behavior service.
- Do not make the client the canonical owner for convenience.
- If a phase uses a simplified in-memory server document, call it a minimal server-canonical placeholder.
- Prefer per-document owner/actor boundaries over global document locks or global serialization.
- Do not add a runtime configuration knob for built-in fallback mode or transform toggles (YAGNI); declaring a package mode is the override path, and `setPackageOption`'s closed suffix allowlist rejects `core.preferredFallbackMode`/`electricCharacters`/`pairInsertion`/`commentContinuation`.
