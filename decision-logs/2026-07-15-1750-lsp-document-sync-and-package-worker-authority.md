---
date: 2026-07-15 17:50
status: approved
decision_about: "LSP document synchronization and long-lived package-worker authority"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Bounded document synchronization and package analysis workers

## Decision

Clay will give a resolver-validated language bridge one dedicated, bounded server-side JavaScript analysis worker for each package + language-server contribution + approved workspace root + runtime generation. The worker receives server-canonical open/reset snapshots and ordered accepted edit deltas, owns package-side protocol/capability state, and emits only existing validated Clay completion, language-intelligence, semantic-decoration, diagnostic, command, and navigation shapes.

This expands `parse-document` only when combined with an exact `language-server` grant: eligible open-document text, accepted unsaved changes, canonical root path, and root-relative document paths may cross to the approved package worker and trusted same-user child. It does not add raw filesystem, network, shell, arbitrary process, client-JavaScript, workspace-mutation, or direct-edit authority.

LSP remains package-owned. Rust owns generic document events, worker/session identity and lifecycle, budgets, provenance, cancellation, validation, publication, and cleanup; package JavaScript owns LSP framing, JSON-RPC, initialize/capabilities, synchronization mapping, position/URI conversion, cancellation messages, and server policy.

## Exact Authority and Data Contract

### Worker identity and module boundary

- One worker is keyed by exact package provenance, contribution descriptor/grant, canonical `WorkspaceRootId`, and runtime generation.
- A worker starts lazily on the first eligible document open and stops after its last synchronized document closes. No package load or manifest validation starts a worker or child.
- At most 4 analysis workers may exist globally. This supports the four approved first-party bridge packages for one root while bounding isolate and child-process cost. A fifth worker fails closed to baseline behavior.
- Worker creation accepts only a resolver-recorded package-root-confined module and named export. It does not accept a JavaScript callback value, arbitrary module URL, filesystem path, executable, argv, cwd, environment value, or process handle.
- The worker module loader exposes only curated Clay facades and package-root-confined modules. It exposes no general Deno filesystem, network, shell, process, raw-op, native-widget, or client-runtime API.
- Each worker has a 64 MiB JavaScript heap ceiling. Heap exhaustion, timeout, or worker poison terminates the worker/session and clears its outputs.

### Document content crossing the boundary

An eligible initial open/reset contains only:

- server-stamped package/contribution/root/runtime-generation identity;
- `DocumentId`, active mode ID, and canonical document version;
- the canonical absolute workspace-root path and validated root-relative document path needed for package-owned `file://` conversion;
- one coherent server-canonical UTF-8 text snapshot.

An accepted change contains only:

- server-stamped document and worker identity;
- base and new canonical document versions;
- the pre-edit UTF-8 byte start/end range;
- inserted UTF-8 text.

Close, reload/reset, root removal, revocation, runtime replacement, and shutdown are explicit analyzer-neutral lifecycle events. No client ID, lease ID, unvalidated client intent, arbitrary absolute document path, environment value, credential, unrelated document text, or direct rope/filesystem handle crosses the worker boundary.

The package maintains its own synchronized document mirror so it can convert Clay UTF-8 byte deltas to the negotiated LSP position encoding. Server capability state comes only from the authorized child's byte stream and remains package-worker state; Rust does not model LSP capabilities.

### Permission composition

- Full bounded snapshots and accepted deltas require both `parse-document` and the exact active `language-server` grant for the package/contribution/root. `parse-document` alone remains the current bounded parse-window authority; `language-server` alone grants no document text.
- Dynamic completion additionally requires `completion-provider` and routes through the existing completion coordinator.
- Semantic and diagnostic output additionally requires `render-decorations` and routes through existing validators/caches.
- Hover, definition, code action, and signature help route through the existing language-intelligence provider contract and its permission/provenance checks.
- Commands, root-relative navigation, and inert edit previews retain their existing independent authority. Direct workspace edits, rename/refactor/format/import-management, and external/out-of-root navigation remain denied.

### Synchronization ordering

- Initial open occurs only after the server has opened the canonical document, selected/activated its mode, verified worker permissions/grant/root identity, and captured one coherent versioned snapshot.
- Changes are derived only from accepted canonical edits. They preserve per-document base/new version ordering and never use raw client intent.
- Edit acknowledgment and local client paint never await worker JavaScript, a queue slot, a child write/read, or LSP response. Submission uses a bounded non-blocking server-side mailbox after canonical acceptance; no worker/process work enters typing, paint, layout, scroll, or native text-event paths.
- A normal change carries the exact accepted delta. Inserted text above 64 KiB or mailbox pressure collapses pending state for that document to one latest canonical reset rather than blocking or growing an unbounded queue.
- Package code maps generic open/change/reset/close events to the server's negotiated synchronization mode. Rust contains no LSP method, `Content-Length`, JSON-RPC, URI, position-encoding, or server-specific branch.
- Reload/resync produces a latest canonical reset. If package policy cannot safely apply a reset to its negotiated protocol state, it closes and reopens that document package-side.

## Approved Budgets

| Resource | Limit | Failure behavior |
| --- | ---: | --- |
| Analysis workers | 4 globally | Do not start worker/child; retain baseline and emit one sanitized status. |
| Synchronized documents | 32 per worker | Additional document remains baseline-only. |
| Raw document text | 256 KiB per document | Never send partial `didOpen`; close/clear bridge state if document grows past limit. |
| Mirrored synchronized text | 8 MiB per worker | Additional/larger document remains baseline-only. |
| Serialized language-server frame | 1 MiB | Reject before unbounded allocation/publication; terminate malformed protocol state when required. |
| Normal inserted-text delta | 64 KiB | Collapse to latest canonical reset. |
| Input mailbox | 64 pending delta records and 2 MiB aggregate per worker | Coalesce affected document to latest reset; never block edit acknowledgment. |
| Output queue | 64 events and 512 KiB aggregate per worker | Reject/terminate noisy worker output and clear stale package state. |
| Pending child requests | 8 per worker | Reject or cancel newest excess request; existing coordinator/global limits still apply. |
| Worker JavaScript heap | 64 MiB | Terminate worker/session and clear outputs. |
| Initialize and individual handler work | 5 seconds | Cancel/terminate failed work; baseline remains. |
| Graceful shutdown | 2 seconds | Close stdin and escalate to kill. |
| Total shutdown/kill/wait | 5 seconds | Force cleanup and record bounded diagnostic. |

The current 256 KiB opaque language-server message budget will become a 1 MiB exact frame/transport ceiling during implementation. Existing narrower result budgets remain authoritative after package conversion: completion and language-intelligence results stay at 16 KiB, decoration/diagnostic sets at 8 KiB, item/span counts remain unchanged, and the 30-second low-level read cap remains a transport ceiling rather than a UI request timeout.

A raw document at or below 256 KiB can still be ineligible if its serialized initial frame exceeds 1 MiB. Both limits must pass before the child enters open-document state.

## Oversize, Backpressure, and Failure Fallback

- An ineligible document never receives a partial initial snapshot and never enters partial child state.
- If an already synchronized document grows beyond the document, worker-total, or serialized-frame limit, Clay sends/attempts bounded close through the package lifecycle, cancels requests, clears that package's semantic/diagnostic/provider output for the document, and returns to the base mode.
- Worker-cap, document-count, memory, queue, frame, initialization, child-exit, protocol, timeout, and permission failures produce one source-keyed, bounded, sanitized status. They do not echo source text, absolute document paths, environment values, credentials, raw child payloads, or unbounded stderr.
- Tier 1 Tree-sitter syntax, behavior manifests, base keyword/snippet completion, commands, and Markdown preview remain active. A later canonical change/reload/open may retry a fresh synchronization when all limits and grants pass.
- There is no automatic restart loop. A fresh eligible open/reload or explicit package/runtime reload may start a new worker after prior cleanup.

## Lifecycle and Revocation

Package disable, grant revocation, package removal/update/source change, contribution/grant mismatch, workspace-root removal, last document close, package reload/withdrawal, runtime-generation replacement, worker heap/timeout failure, protocol failure, child exit, and Clay shutdown:

1. stop accepting new events/requests for the identity;
2. cancel pending/in-flight work and stale-drop late results;
3. clear package-owned cached semantic, diagnostic, completion, and intelligence state;
4. allow at most 2 seconds for package-owned graceful protocol shutdown;
5. close stdin, kill, and wait for the child and terminate the worker within 5 seconds total;
6. retain bounded audit identity and sanitized status only.

A later request cannot restore authority without a current exact grant and enabled matching package generation.

## Containment Statement

The worker is constrained by Clay's module/op allowlist. The child is not OS-sandboxed. Canonical-root identity, cwd, root-relative events, and grant checks constrain Clay's API and audit record only. The approved same-user child may still read other host files, access the network, inspect host state, or spawn descendants using operating-system permissions. Documentation must call this **trusted subprocess authority**, never workspace-confined, filesystem-confined, network-confined, process-confined, or sandboxed.

## Context

Phase 18.20 approved a fixed, deny-by-default language-server process grant and opaque bounded session. Phase 18.21's primitive review found that correct bridge packages additionally require exact bytes, a coherent full initial document snapshot, accepted unsaved deltas, explicit close/reload/revocation lifecycle, dynamic completion, and asynchronous semantic/diagnostic output.

Existing parse windows cannot satisfy LSP synchronization: first-party windows are 4 KiB, contain no exact accepted edit operation or open/change/close lifecycle, and may truncate the document. Existing package runtime commands are sequential and evaluation-scoped; an endless child-read promise would block unrelated runtime work and eventually hit timeout/poison behavior. Existing completion registration is static-only, while decoration/diagnostic publication is one evaluation-result slot rather than a long-lived bounded output channel.

LSP 3.17 requires `didOpen` to transfer full document truth to the server, versioned changes after every accepted edit, and `didClose` when the client relinquishes document truth. Incremental synchronization still requires full text on open. Position encoding is negotiated in 3.17 and defaults to UTF-16 if omitted, so package workers need a coherent versioned document mirror for conversion even though Clay core remains UTF-8-byte-based.

## Approval

- Proposed by: agent
- Approved by user: Yes
- Approval evidence: After receiving the exact worker identity, permissions, document-event data, limits, fallback, lifecycle, package/core ownership, and trusted-subprocess disclosure, the user replied, **"Agree with the decision. Go ahead with decision log and plan update"**.

## Alternatives Considered

1. **Dedicated bounded package/root worker with full initial snapshot and accepted deltas** — selected. It supports unsaved server-canonical text, isolates package failures, keeps LSP package-owned, and establishes explicit resource and revocation boundaries.
2. **Let the child reread files and omit or stale-fill `didOpen`** — rejected. LSP transfers truth to the client after open; disk rereads lose unsaved edits and can analyze a different version.
3. **Implement synchronization and JSON-RPC in Rust core** — rejected. It introduces LSP types/server policy into analyzer-neutral core and invites per-language branches.
4. **Run an endless read loop in the current persistent configuration/runtime isolate** — rejected. Current runtime commands execute sequentially with evaluation timeouts; long-lived I/O would serialize unrelated package work and poison the shared generation on failure.
5. **Use one global worker for all packages and roots** — rejected. It saves isolate overhead but creates global serialization, shared failure/heap fate, cross-package state risk, and harder revocation. Four bounded workers cover the first-party target with clearer identity.
6. **Allow arbitrary-size snapshots or unbounded queues** — rejected. They permit memory/I/O amplification and can delay authoritative edit handling. Fixed ceilings plus baseline fallback are safer and simpler.
7. **Add a new full-document permission** — rejected for this phase. Requiring the conjunction of existing `parse-document` and exact `language-server` authority expresses analysis plus subprocess scope without another permission; the decision explicitly records the expanded combined data scope.

## Rationale and Evidence

- `docs/wiki/modules/phase18.21-lsp-bridge-primitive-review.md` identifies only four generic gaps and rejects LSP/core and per-language duplication.
- `src/server/document.rs` and `src/server/connection.rs` already own canonical edit validation/version ordering; synchronization must originate there after acceptance.
- `src/protocol/parse.rs` and first-party parse policies expose bounded windows, not coherent full-document open state or exact accepted deltas.
- `src/server/js_runtime.rs` owns one sequential persistent worker with evaluation-scoped side outputs; it is unsuitable for an endless bridge loop.
- `src/server/completion.rs` already has the generic dynamic provider/coordinator; only a resolver-token package adapter is missing.
- `src/server/decorations.rs` and `diagnostics.rs` already own output validation/cache/publication and must remain the only publication path.
- Existing typed budgets include 16 language-server sessions, a 30-second low-level read cap, 16 outstanding language-intelligence requests, 5-second maximum intelligence timeout, 16 KiB completion/intelligence results, and 8 KiB decoration/diagnostic sets. The approved worker limits compose with, rather than replace, these narrower result limits.
- The official LSP 3.17 specification states that `didOpen` makes the client responsible for document truth, `TextDocumentSyncKind.Incremental` still sends full content on open, document versions increase after every change (not necessarily consecutively), `didClose` relinquishes client truth, cancellation uses `$/cancelRequest`, and omitted `positionEncoding` defaults to UTF-16.
- The selected limits deliberately support the four first-party bridge packages for one root, ordinary source files, bounded mirrors/queues, and a serialized-frame cap aligned with Clay's existing 1 MiB IPC scale. They are initial hard ceilings, not user configuration.

## References

- `decision-logs/2026-07-14-2023-language-server-package-authority.md` — fixed process grant and trusted-subprocess boundary.
- `plans/053-Phase18.21-First-Party-LSP-Bridge-Packages.md` — task sequence and implementation requirements.
- `docs/wiki/modules/phase18.21-lsp-bridge-primitive-review.md` — source inventory and proven generic gaps.
- `docs/reference/primitives/language-intelligence.md` — canonical LSP-to-Clay package mapping.
- `src/server/{connection,document,js_runtime,language_server,completion,decorations,diagnostics}.rs` — current authority and runtime paths.
- `src/protocol/parse.rs` and `src/perf/budgets.rs` — current event shapes and budgets.
- `.agents/skills/project-patterns/references/{authority-boundaries,language-capability-sequencing,protocol-and-performance}.md` — reusable architecture constraints.
- [LSP 3.17 specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/) — lifecycle, synchronization, version, position encoding, and cancellation contracts, retrieved through Context7 library `/websites/microsoft_github_io_language-server-protocol_specifications_lsp_3_17` on 2026-07-15.

## Consequences

- Bridge packages can correctly analyze unsaved canonical text without giving package JavaScript raw document/filesystem/process handles.
- Rust implementation must add a generic bounded worker/mailbox lifecycle and exact byte transport, but must not add LSP wire types or language-specific branches.
- Implementation must raise the exact language-server frame cap from 256 KiB to 1 MiB while preserving narrower converted-result limits and adding pre-allocation rejection tests.
- Public package-disable/revoke/root-removal/runtime-replacement paths must be wired to worker, session, request, and cached-output cleanup before bridge packages ship.
- Documents or roots beyond initial ceilings keep complete baseline editing functionality but no LSP state. Limits may be raised only after measurements and a reviewed budget change; they must not become unrestricted user knobs.
- Revisit if normal projects routinely exceed four active package/root workers, 256 KiB documents require LSP support, 64 MiB isolates are insufficient, multi-root server processes become necessary, a new full-document permission is clearer to users, or strict OS containment becomes a product requirement.
