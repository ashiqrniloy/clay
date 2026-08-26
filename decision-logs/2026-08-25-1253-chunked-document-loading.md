---
date: 2026-08-25 12:53
status: approved
decision_about: "Remove the per-file open size ceiling and replace full-text document transfer with chunked head+chunk loading"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Chunked document loading replaces the file size ceiling

## Decision

Clay removes `MAX_OPENABLE_FILE_BYTES` (768 KiB) as a per-file open gate. Initial tab load, selected/path `DocumentOpened`, `DocumentReloaded`, resync, and persisted-document restore opens switch from full-text-in-one-frame transfer to a bounded chunked protocol: a head message carrying the first chunk plus total byte length, followed by client-driven bounded chunk requests (256 KiB clamp) served by rope slices. The per-file size gate is replaced by a session-level resident-memory budget (256 MiB, the reserved `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB`) plus binary-content sniffing (NUL byte in the first 8 KiB), both server-owned security budgets and not user configuration. Atomic save streams rope chunks instead of materializing the whole document as a `String`. The IPC protocol version bumps 26 → 27.

## Context

The 768 KiB ceiling exists because `InitialDocument` and `ResyncSnapshot` ship the entire document as a single rkyv frame capped at 1 MiB; the ceiling keeps any legal file inside one frame. The user hit this ceiling while working with large files (their perf-testing files exceed 1 MiB): the server silently rejected the open (`FileOperationFailed`/`FileTooLarge`), the frontend surfaced nothing, and the file appeared to not load. The silent failure was fixed on 2026-08-25 (status-bar/pane diagnostics), but the ceiling itself blocks the user's real work.

Clay's architecture is otherwise already incremental: edits are bounded ordered deltas, syntax parsing is viewport-scoped rope-sliced windows, and offset math is indexed. The editor-facing whole-document operations are initial tab load, selected/path open, reload, resync, persisted-document restore opens, and save - exactly the transport/file surfaces this decision converts to bounded chunked transfer or bounded rope streaming. Existing trusted-runtime `documents.serverOpenDocument`/reload JSON APIs are separately audited in Plan 098 task 10 because they currently return full text into the deno_core heap. The 1 MiB frame cap is retained; raising it would block the connection read loop for the decode duration of giant frames and violate Clay's documented prohibition on full-document IPC.

## Approval

- Proposed by: both (user required no ceiling; agent proposed the chunked-loading mechanism)
- Approved by user: Yes
- Approval evidence: User: "I don't want to have a file size ceiling as it essentially blocks me from working with large files." After reviewing the analysis and recommended approach (chunked initial load + session memory budget + binary sniff), user instructed: "Yes create a plan document for the implementation" and then "Complete the first task Record the decision log for chunked document loading."

## Alternatives Considered

1. **Raise the ceiling and the frame cap together** — rejected: frames above ~1 MiB block the connection read loop while decoding (head-of-line blocking) and violate the documented performance rule against full-document IPC and unbounded frames.
2. **Keep the ceiling, only improve error surfacing** — rejected: the user explicitly requires large-file support; surfacing alone still blocks real work.
3. **User-configurable per-file ceiling via init.js** — rejected: consistent with the JS runtime heap limit precedent, memory bounds are server-owned security budgets, not user configuration; a per-file config invites unsafe defaults.
4. **Push-based server streaming of chunks after the head** — rejected: requires per-client streaming state and flow control; pull-based client-driven requests are stateless server-side, resumable, and match the existing viewport-request precedent.
5. **True lazy viewport-only documents (server never sends unseen text)** — deferred: drags search, save, and off-screen editing into server round-trips; not justified while local chunk fetch is fast.

## Rationale and Evidence

- `src/perf/budgets.rs:331-352`: `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB = 256` was already reserved as "the future resident-memory budget for that chunked path"; the ceiling comment names chunked/viewport-first loading as the documented follow-up.
- `src/protocol/codec.rs:13,1075-1083`: `DEFAULT_MAX_FRAME_SIZE = 1 MiB`; guard test pins `MAX_OPENABLE_FILE_BYTES < DEFAULT_MAX_FRAME_SIZE` — the structural coupling this decision removes.
- `src/server/workspace.rs:1871-1940` (`check_openable_size`, `read_file_bounded`): the gate and bounded read to replace; `src/server/workspace.rs:2209-2245` (`save_io`): whole-document `String` materialization to replace with streamed rope chunks.
- `src/server/workspace.rs:1254-1267` (`open_document_snapshots`): runtime-generation reload materializes every open document's full text simultaneously. This is an internal refresh path, not frontend bootstrap; it is converted to metadata plus bounded rope-derived classification/parse/analysis inputs.
- `src/server/document.rs:526` (`parse_windows_covering`): existing rope-sliced range primitive demonstrating the chunk-serving pattern.
- Frontend `frontend/src/editor/sync/session.ts:152` (`replaceText(bootstrap.initialDocument.text)`): the consumer that becomes a progressive chunk-assembly state machine; CodeMirror's `Text` is rope-backed so ordered appends are O(chunk).
- Live reproduction (2026-08-25): opening a > 768 KiB file produced no visible feedback; second dialog attempts died on the busy mutex — fixed by the diagnostic-surfacing change, but the ceiling remained.
- Tradeoff accepted: editing gates on load completion per document (version/ack semantics assume whole-document consistency); local IPC chunk fetch makes this window short, and typing is unaffected once ready.

## References

- `plans/098-Chunked-Document-Loading.md` — implementation plan with task breakdown, acceptance criteria, and tests.
- `src/perf/budgets.rs` — ceiling rationale comment and reserved 256 MiB budget.
- `src/protocol/mod.rs`, `src/protocol/codec.rs` — message vocabulary, frame bounds, version-bump precedent (v25→v26 in Plan 097).
- `.agents/skills/project-patterns/references/protocol-and-performance.md` — full-document IPC prohibition and snapshot-exception rules.
- `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md` — retained server authority and transport model this decision builds upon.

## Consequences

- Positive: files of any size open (bounded by the 256 MiB session budget); first paint shows the first chunk immediately; memory stays bounded on server and client; save avoids whole-document allocation; the 1 MiB frame cap and all existing performance rules survive intact.
- Risks and follow-up work: protocol v27 requires the supervisor protocol probe to gate mixed-version adoption (already implemented in Plan 097); initial/open/reload/resync and persisted-restore paths must convert in the same effort or large files break on one workflow; trusted-runtime public open/reload APIs must retain a bounded heap-safe contract; binary sniff only inspects the first 8 KiB (NUL after that is a documented ceiling); parallel chunk fetch across documents deferred until measured need.
- Revisit conditions: if the session budget blocks legitimate workflows (e.g. many large documents open at once), a user-facing budget configuration decision can be made separately; if remote/high-latency transports arrive, re-evaluate push-based streaming and chunk compression.
