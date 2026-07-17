---
date: 2026-07-17 18:41
status: approved
decision_about: "Phase 20 undo/redo, clipboard cut/paste, IME composition, and multi-document session semantics"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Client inverse-edit undo, user-mediated clipboard, paint-only IME preedit, and multi-document client sessions

## Decision

Phase 20 daily-editing product hardening will use a per-document client undo/redo stack that applies inverse ranges as ordinary optimistic `Edit` transactions, extend the existing client clipboard sink for cut/paste without a server clipboard proxy, keep IME preedit as a local paint-only overlay until `Commit`, and retain multi-document client session state keyed by server `DocumentId` while the server remains open-registry/lease/dirty authority.

Package, configuration, and AI authority over clipboard contents, filesystem, shell, network, and raw ops is **not decided here** and must be established in a later dedicated decision. Phase 20 implements the user-facing editor commands on existing client/server primitives and does not invent those package/config/AI surfaces in this phase.

## Exact Semantics

### Undo / redo

1. Ownership is client-local history, not a server undo protocol. The server remains unaware of undo/redo as distinct operations and validates inverse applications as ordinary `ClientMessage::Edit` values under the editable lease, region locks, base version, and behavior version rules.
2. History is keyed per server `DocumentId`. Each entry records enough inverse information to restore prior text and caret/selection: the applied forward operation's inverse insert/delete/replace plus caret/selection restore metadata. Undo pushes the forward inverse onto the redo stack; redo reverses that.
3. Stack depth is bounded at **256** entries per document (aligned with the existing pending-edit / previous-behavior-grace transaction ceilings). When full, drop the oldest undo entry. Redo is cleared on any new non-undo/redo user edit that mutates the document.
4. Application path: cancel unfinished IME preedit first; apply the inverse locally to the shadow rope; enqueue a normal optimistic edit through `ClientEditQueue`. No full-document IPC for ordinary undo/redo. Rejected undo/redo recovers through the existing resync path.
5. Full canonical resync or hard open-replace for a document **clears** that document's undo and redo stacks. Document switch does not clear other documents' stacks. Closing/evicting a client session may discard its history with the session.
6. Unfinished IME composition is never partially undone: cancel/discard the preedit overlay before recording or applying undo/redo.

### Clipboard cut / paste

1. Extend `ClipboardSink` / `SystemClipboard` with `get_text` beside `set_text`, using `arboard 3.6.1`'s existing APIs. Keep fake/memory sinks for tests.
2. Copy remains: non-empty selection → `set_text`. Collapsed selection is a no-op.
3. Cut: if selection is non-empty, copy then delete the selection as one user gesture that produces ordinary local edit emission. Collapsed cut is a no-op.
4. Paste: on explicit paste, `get_text`, then insert at caret or replace the selection as an ordinary local edit. Empty clipboard text is a no-op insert.
5. Clipboard OS read/write happens only on explicit cut/copy/paste command paths. Failures become sanitized runtime diagnostics; diagnostics must not include full clipboard contents.
6. Phase 20 does not add a server clipboard proxy and does not add package/config/AI clipboard-contents APIs. Whether packages, configuration, or AI may later gain clipboard (or other) authority is deferred to a later decision.

### IME / composition

1. Handle Masonry/winit `Ime::{Enabled, Preedit(String, Option<(usize, usize)>), Commit(String), Disabled}`.
2. Preedit text and optional byte-indexed cursor span are paint-only composition state on the editor surface/widget. They are not canonical document text and do not enqueue edits or IPC.
3. Empty preedit clears the overlay. Per winit/Masonry contract, `Commit` is preceded by an empty `Preedit`; Clay must tolerate that ordering.
4. `Commit(text)` clears composition state and applies **one** ordinary insert/replace edit through the existing local edit path.
5. Update the candidate-window IME area through Masonry `set_ime_area` / `ImeMoved` when caret or composition geometry changes. Clear IME area when composition ends or focus is lost as appropriate.
6. Cancel unfinished composition (discard overlay; do not commit) on focus loss, active-document switch, and before undo/redo. Do not stream preedit bytes as continuous document edits.

### Multi-document sessions

1. Server `WorkspaceState` remains the open-document registry, lease, dirty, mode, and path authority. Opening/switching documents stays server-first for authorization and metadata.
2. The client maintains a session map keyed by server `DocumentId`. Each retained session keeps local shadow text, caret, selection, viewport, pending-edit queue state, undo/redo history, and local status/dirty chrome mirrors needed for immediate switch-back.
3. Receiving `DocumentOpened` for another file must **retain** prior session state rather than destroy it. Active chrome switches to the newly active document; inactive sessions remain available until closed or evicted.
4. Bound retained client sessions at **64** (aligned with `RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS`). When over capacity, evict the least-recently-active inactive session and emit a sanitized notice. Do not silently drop the active document.
5. Save/conflict/dirty indicators continue to reflect server metadata; client chrome mirrors that metadata and must not invent a second canonical dirty authority.
6. Lease transfer/steal UX, multi-client scaling, and remote/container transport remain Phase 21+ and are out of this decision's scope.

### Deferred: package / configuration / AI authority

This decision does **not** approve a permanent rule that packages, configuration, or AI gain no clipboard, filesystem, shell, network, or raw-op authority. That broader authority model must be established later in an explicit decision.

Until that later decision:

- Phase 20 daily-editing features ship as Clay-owned user-mediated client commands and existing selected-file/workspace-root server paths.
- Phase 20 must not invent package/config/AI clipboard-contents APIs, client filesystem authority shortcuts, shell/network/raw-op grants, or undo mutation outside ordinary edit validation as part of implementing these semantics.
- Existing selected-file / workspace-root / lease / grant rules already in force remain unchanged by this decision.

## Context

Plan 055 Phase 20 must ship clipboard cut/paste, undo/redo, IME composition, and multi-document behavior on top of Clay's server-authoritative documents and optimistic client shadows. The Phase 20 primitive review found copy-only `ClipboardSink`, `Ime::Commit` without preedit handling, no undo/redo stack, and client replace-on-open that destroys prior editor state even though the server can already hold multiple open documents.

A server-owned undo protocol would enlarge collaboration surface into Phase 21 territory. Rewriting the local rope without server transactions would break server authority. Streaming IME preedit as edits would create noisy versions and break composition. Keeping replace-on-open would fail the multi-document requirement.

## Approval

- Proposed by: agent
- Approved by user: Yes, with one amendment
- Approval evidence: After receiving the recommended undo/clipboard/IME/multi-document semantics, the user replied that they do not agree with locking “Packages/config/AI gain no clipboard, filesystem, shell, network, or raw-op authority” and that this must be established later; **“Otherwise approved.”**

## Alternatives Considered

1. **Server-owned undo log with dedicated undo protocol.** — Rejected for Phase 20. Richer multi-client story, but larger protocol/collaboration surface that overlaps Phase 21.
2. **Client-only undo that rewrites local rope without server transactions.** — Rejected. Breaks server authority, leases, and multi-client consistency.
3. **Client inverse-edit undo emitted as ordinary optimistic edits.** — Selected.
4. **Commit IME preedit bytes continuously as edits.** — Rejected. Noisy versions and broken composition.
5. **Local preedit overlay until `Ime::Commit`, then one ordinary edit.** — Selected.
6. **Continue replace-on-open single buffer.** — Rejected by the Phase 20 multi-document requirement.
7. **Client multi-document session map keyed by server `DocumentId`.** — Selected; server remains registry/lease/dirty authority.
8. **Finalize package/config/AI denial of clipboard/filesystem/shell/network/raw-op authority in this log.** — Rejected by user. Deferred to a later explicit authority decision.
9. **Server clipboard proxy.** — Rejected. Clipboard is a client OS resource and must stay on the client command path for Phase 20.

## Rationale and Evidence

- `src/client/clipboard.rs` today exposes only `ClipboardSink::set_text`; `arboard 3.6.1` already provides `get_text` / `set_text`.
- `src/masonry_editor.rs` handles `Ime::Commit` only; Masonry 0.4 `Ime` also includes `Enabled`, `Preedit`, and `Disabled`, with `LayoutCtx::set_ime_area` and `RenderRootSignal::ImeMoved`.
- No History/undo/redo stack exists under `src/`; ordinary typing already uses optimistic local apply + `ClientMessage::Edit` ack/reject/resync (`docs/wiki/flows/versioned-text-synchronization.md`).
- `opening_second_file_browser_file_replaces_editor_snapshot` documents current client replace-on-open, while `WorkspaceState` / `OpenDocument` already support a multi-document server registry.
- Pending-edit and previous-behavior-grace ceilings already use 256; runtime snapshot document ceiling is 64 — reuse those magnitudes for undo depth and retained sessions.
- Authority-boundaries and protocol-and-performance patterns require server-canonical documents, no full-document IPC for ordinary edits, and no IPC/JavaScript in paint/text-event hot paths beyond existing local-edit work.
- The user explicitly deferred package/config/AI clipboard/filesystem/shell/network/raw-op authority rather than approving a permanent denial in this decision.

## References

- `plans/055-Phase20-Daily-Editing-Product-Hardening.md` — Phase 20 plan and semantics-decision task.
- `docs/wiki/modules/phase20-daily-editing-product-hardening-primitive-review.md` — entry-gate inventory and preferred defaults that this decision finalizes (with the authority amendment).
- `docs/wiki/flows/{versioned-text-synchronization,client-server-edit-ack,client-behavior-routing,document-leases-and-region-locks}.md`
- `src/client/{clipboard,mod}.rs`, `src/masonry_editor.rs`, `src/editor/surface.rs`, `src/server/{workspace,document}.rs`
- `src/perf/budgets.rs` — `PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS = 256`, `RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS = 64`
- Local `arboard 3.6.1` (`Clipboard::{get_text,set_text}`); local `masonry_core 0.4.0` `Ime` / `set_ime_area` / `ImeMoved`
- `.agents/skills/project-patterns/references/{authority-boundaries,protocol-and-performance,behavior-manifests}.md`

## Consequences

- Positive: Phase 20 can implement cut/paste, undo/redo, IME, and multi-document sessions without new server undo/clipboard protocols or replacing the document authority model.
- Follow-up: create a later decision specifically covering package/configuration/AI authority for clipboard, filesystem, shell, network, and raw ops before exposing those surfaces.
- Risks: client session memory grows with retained documents; mitigated by the 64-session ceiling and per-document 256-history bound.
- Revisit undo ownership if Phase 21 multi-client collaboration requires shared history; revisit IME if platform composition contracts change; revisit session eviction policy if 64 proves too low for real workloads.
