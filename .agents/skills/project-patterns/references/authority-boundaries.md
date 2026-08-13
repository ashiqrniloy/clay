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

## Daily Editing Ownership (Phase 20)

- Undo/redo is a per-document client history of inverse insert/delete/replace operations applied as ordinary optimistic `Edit`s under the editable lease. The server stays undo-unaware.
- Clipboard cut/copy/paste is client OS-mediated through `ClipboardSink`; paste reads and cut/copy write only on explicit user commands. No server clipboard proxy.
- IME preedit is client paint-only until `Commit`, which becomes one ordinary edit. Cancel unfinished composition on focus loss, document switch, and before undo/redo.
- Multi-document: server remains open-registry/lease/dirty authority; the client retains a bounded session map keyed by server `DocumentId` (shadow/caret/viewport/pending/history/status chrome).
- Package/configuration/AI authority over clipboard, filesystem, shell, network, and raw ops is **not finalized** by Phase 20 daily-editing semantics and requires a later explicit decision. Until then, Phase 20 ships Clay-owned user commands on existing selected-file/workspace-root paths and does not invent those surfaces.
- Decision log source: `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.

## Built-in `core.*` Modes and Bounded Probing (Phase 18.9)

- Clay owns always-on built-in major modes `core.text` (universal fallback) and `core.code` (code-like extensions and any shebang), registered at server startup via `register_builtin_mode` with no `init.js` line and no `loadPackage` step. They grant no package authority.
- The `core.` and `clay.` mode-ID prefixes are reserved for Clay-owned built-ins; `register_mode`/`register_minor_mode` must reject them. Built-in manifests ship without an owning package (`select_behavior_manifest_for_document` bypasses package-record lookup on the `core.` prefix).
- Classification probing reads only a bounded constant prefix (`MAX_LEADING_CONTENT_BYTES = 512`) of an already-open document supplied by the open path — never a filesystem scan, directory walk, or package-supplied predicate. Oversize slices are treated as absent and fall to the remaining precedence ladder. The open path is the sole authority supplying shebang/leading-content slices.
- Mode-discovery commands (`modes.listActiveModes`/`explainActiveMode`) are read-only `ServerFirst` built-ins with empty permissions resolved via `CommandExecutor::execute_discovery`; they carry no execution/document/workspace authority.
- Decision log source: `decision-logs/2026-07-01-0350-phase18-9-generic-text-code-fallback-modes-and-key-behavior.md`.

## External Process Authority

- Package-triggered external processes require a dedicated deny-by-default capability and an approved decision log; never silently compose them from package load, first-party trust, `shell`, or `filesystem`.
- Bind approval to package provenance, a fixed inert contribution, canonical executable, literal argv, explicit inherited-environment names, and known workspace roots. Runtime input selects only an already-approved contribution/root.
- Launch directly without a shell, clear environment by default, bound all I/O/time/concurrency, and terminate on revocation, reload, root removal, runtime replacement, or shutdown.
- Working directory and root-bound grants constrain Clay's API/audit identity, not the operating system. A same-user child may access other files, network, and processes; call it trusted subprocess authority, never sandboxed or workspace/filesystem confined.
- Keep process work asynchronous and outside typing, paint, layout, scroll, and local text-application paths.
- Decision log source: `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Package Runtime Trust Domains

- Clay and exact integrity-verified bundled packages run in one trusted JavaScript runtime; adopted third-party packages run together in a second runtime and cannot be promoted through normal approval.
- Third-party runtime installs only documented public package ops and narrow host state. Clay-internal ops are absent, not hidden by facades.
- Cross-domain communication uses typed, bounded, inert Rust-mediated values. No V8 object, function, global, module instance, or promise crosses domains.
- Third-party mutation of first-party behavior requires both a target-declared extension point and explicit user approval. Full approved replacement withdraws the first-party package while replacement code remains third-party and keeps its provenance.
- Third-party packages form a disclosed shared trust cohort and are not isolated from each other.
- Decision log source: `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.

## Long-Lived Package Document Analysis

- Bind each worker to exact package provenance, contribution grant, canonical workspace root, and runtime generation; use resolver-recorded package modules rather than callback values or arbitrary module URLs.
- Full canonical open/reset text and accepted unsaved deltas require both `parse-document` and the exact `language-server` grant. Expose only server-stamped identity/version, canonical root path, validated root-relative path, UTF-8 text, and accepted byte-range deltas—not client intent, rope/filesystem handles, environment values, or unrelated documents.
- Keep local edit acknowledgment and paint wait-free. Submit analysis state only after canonical acceptance through bounded non-blocking mailboxes; pressure coalesces to one latest reset rather than blocking or growing without bound.
- Core owns generic event ordering, worker/session lifecycle, limits, provenance, cancellation, stale rejection, validated publication, and cleanup. Package code owns protocol framing/state, capability negotiation, synchronization mapping, positions, URIs, and server policy.
- Above document/worker/queue/frame ceilings, never create partial child state: clear package outputs, emit one sanitized status, and retain mode, Tier 1 syntax, base completion, commands, and previews.
- Disable/revoke/remove/update/root removal/last close/runtime replacement/failure/shutdown cancels work, clears cached outputs, and terminates worker and child within bounded graceful/kill deadlines. Do not add automatic restart loops.
- Worker module/op confinement does not sandbox the approved same-user child; retain trusted-subprocess disclosure.
- Decision log source: `decision-logs/2026-07-15-1750-lsp-document-sync-and-package-worker-authority.md`.

## Package Editor Control (`editor-control`)

- Package access to client-local editor state (caret/selection) requires BOTH an approved permission (`editor-control`) AND an exact-mode declaration (`clay.editorControl.modes`); enforce per call, deny-by-default. Never grant "editor access" as a whole — only named modes.
- Gate in the shared op path (`require_current_package_capability` provenance + active-major-mode membership), not per-caller branches: same gate for first- and third-party. Trusted callers without a package context (user configuration) are the only gate-free path.
- The third-party worker holds no mode registry/document scope: replicate the active mode snapshot across the existing worker bridge (push on state change + on worker rewire) instead of synchronous cross-domain queries.
- Server→client programmatic triggers use an advisory bounded push (`EditorCommandRequest`, boxed wire variant); the client re-parses command IDs deny-by-default through the keybinding dispatch path and drops unknown/non-editor IDs. Advisory channels never block editing (lagged/stale → drop).
- Conflicts between packages in one mode: coexist, no automatic arbitration — the user deactivates packages. Revocation takes effect on runtime generation replacement.
- Decision log source: `decision-logs/2026-08-03-1859-editor-control-trust-boundary-for-editor-ops.md`.

## Built-In Browse Grant (Command Centre Path Mode)

- User-driven navigation inside a Clay-owned built-in surface (e.g. Command Centre path mode) implicitly authorizes filesystem traversal outside granted workspace roots; packages never receive equivalent authority.
- Converting a browsed path into durable access still requires an explicit grant: opening a file creates a `SingleFile` grant, opening a folder as workspace creates a `Directory` root grant via the existing tab/workspace binding path.
- Browse listings stay bounded (depth 1, entry caps) and are never read on the paint/layout path.
- Decision log source: `decision-logs/2026-08-11-1711-command-centre-surface-path-mode-and-sequence-keybindings.md`.

## Language-Server Grant Degradation (load-time tolerance)

- `loadPackage` tolerates a missing (or stale/revoked) `language-server`
  grant: the capability stays inert because session start and every
  analyzer invocation re-check a current exact grant covering the
  document's workspace root. All other capability grants keep their hard
  load-time requirement.
- Analyzer registration (`language.serverRegisterDocumentAnalyzer`)
  requires the package to be enabled and the contribution to name a fixed
  package language server, but not a current grant; a generation reload
  skips analyzers without a current grant (re-registers once the grant
  lands) instead of failing the generation.
- Bundled defaults never auto-grant language-server authority; a
  replacement package never inherits the replaced target's grant.
- Decision log source:
  `decision-logs/2026-08-13-2223-degraded-language-server-grant-tolerated-at-load-package.md`.

## Planning Guidance

- Do not describe the server as a stateless behavior service.
- Do not make the client the canonical owner for convenience.
- If a phase uses a simplified in-memory server document, call it a minimal server-canonical placeholder.
- Prefer per-document owner/actor boundaries over global document locks or global serialization.
- Do not add a runtime configuration knob for built-in fallback mode or transform toggles (YAGNI); declaring a package mode is the override path, and `setPackageOption`'s closed suffix allowlist rejects `core.preferredFallbackMode`/`electricCharacters`/`pairInsertion`/`commentContinuation`.
