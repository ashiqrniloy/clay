---
date: 2026-05-08 04:08
status: approved
decision_about: "Phase 3 document authority and behavior model"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Server-authoritative documents with client-executed behavior manifests

## Decision

Clay will use a server-authoritative document and behavior model with optimistic client shadows. The server owns canonical document state, document versions, edit transactions, file/workspace authority, extension execution, and behavior definitions. Clients own rendering, input handling, viewport/caret/selection transient state, local shadow ropes, pending edit queues, and execution of server-issued hot-path behavior manifests for latency-sensitive editing.

Each document may have one editable lease at a time; additional clients opening the same document are read-only observers unless the editable lease is released or transferred. Phase 3 will implement the first minimal slice of this final model rather than a temporary client-authoritative or ambiguous hybrid model.

## Context

Clay is designed as a local-first but client/server programmable editor environment. The client/server model must support multiple documents, multiple UI clients, terminal UI clients, container/toolbox/distrobox servers, SSL/remote editing, and a single long-lived server that owns workspace environment, file permissions, JavaScript extension execution, AI tools, and server-driven UI.

The Phase 3 plan required deciding where document state and editor behavior live before hard-coding protocol assumptions. The original recommended path was a Phase 3 hybrid that keeps the client responsive while preparing for long-term server-canonical synchronization. The discussion refined this into an explicit final authority model: server authority plus client-executed behavior snapshots/manifests, so ordinary typing does not synchronously require IPC -> Rust -> deno_core/V8 -> Rust -> IPC round trips.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user said, "Okay. Log the decision with @.agents/skills/create-decision-log/" after reviewing and aligning with the proposed model and refinements.

## Alternatives Considered

1. **Client-only rope with server behavior service** — Rejected. It gives the simplest local typing path but makes the server unable to safely own file saving, AI edits, JavaScript extension mutations, remote/container workspace authority, multi-client coordination, and terminal UI consistency without trusting client-provided state.
2. **Synchronous server/deno_core decision for every keypress** — Rejected for normal editing. It preserves pure server behavior authority, but ordinary typing would depend on IPC, server scheduling, V8 execution, JavaScript GC/tail latency, and the return IPC path. This is too risky for Clay's performance requirement, especially under remote/container sessions or extension load.
3. **Ambiguous Phase 3 hybrid authority** — Rejected as an architectural decision. Phase 3 may lack full stale-edit rejection and resync, but this is a temporary implementation limitation, not a different authority model. The server is still defined as canonical.
4. **Client-side arbitrary JavaScript runtime** — Not selected for the current architecture. It could make hot-path behavior programmable locally, but duplicates runtime complexity, increases security risk, complicates remote/container semantics, and can create divergence between server and client behavior.
5. **Client-side WASM behavior modules** — Deferred as a future extension option. WASM could eventually allow sandboxed hot-path behavior modules shared between server and client, but it requires a stable ABI, capability model, fuel/time limits, memory limits, deterministic host APIs, and careful hot reload semantics. The initial design should leave room for it without depending on it.

## Rationale and Evidence

Clay's long-term requirements strongly favor server authority. The server is the only component that can consistently own workspace/file permissions, container or remote environment variables, JavaScript extension execution, AI tool orchestration, file save/reload, document leases, region locks, and cross-client consistency. This matches `concept.md`, which describes the server as the authoritative source of truth and the client as a high-frequency local state owner with optimistic updates.

At the same time, performance requirements mean normal text editing cannot block on a full server and JavaScript round trip. The chosen model separates authority from execution locality: the server owns behavior definitions, but publishes a versioned behavior manifest/snapshot that the client can execute for hot-path, deterministic editing behavior such as printable insertion, deletion, Enter indentation, Tab handling, bracket/quote pairing, comment continuation, and safe keymap-local actions.

Protocol messages should carry document and behavior versions so the server can validate optimistic edits. A normal edit flow is:

1. Server sends an initial document snapshot and behavior manifest.
2. Client applies predictable local edit behavior immediately against its shadow rope.
3. Client sends the resulting operation/intent asynchronously with document version and behavior version.
4. Server validates, applies the canonical transaction, increments the document version, and confirms or corrects the client.
5. Corrections should be rare and associated with stale versions, behavior hot reload races, external file reloads, extension failures, or explicit locks.

Keypresses and commands should have explicit routing policies in the manifest. Pure predictable text edits are client-first. Commands with file, workspace, AI, extension, shell, or unknown side effects are server-first. UI-reactive features such as completion, diagnostics, hover, and inline AI suggestions should use priority lanes/workers and cancellation rather than blocking the edit path.

Hot reload should be modeled as behavior manifest replacement. JavaScript runs on the server, registers modes/commands/UI behavior, and the server compiles that into a versioned manifest. Clients atomically install new manifest versions. Behavior-changing AI or extension sessions may temporarily lock the affected document, range, behavior scope, or workspace scope depending on the mutation.

The VS Code Remote Development documentation provides an external reference point for the remote/container motivation: it emphasizes using a container, WSL, SSH, or remote machine as the development environment, with commands and extensions running where the environment and source code live rather than requiring the source code on the local machine. Clay's server-authoritative model follows the same broad requirement that environment-sensitive behavior belongs server-side, while Clay additionally preserves native client performance through local shadows and behavior manifests.

## References

- `concept.md` — Defines Clay's Thick Client / Asynchronous Server architecture, server canonical state, client shadow state, server-driven UI, JavaScript extension engine, and version/region-lock direction.
- `roadmap.md` — Defines Phase 3 IPC skeleton, Phase 4 versioned synchronization, Phase 5 file/workspace server, Phase 6 SDUI, Phase 7 embedded JavaScript runtime, and Phase 8 AI-safe mutation.
- `plans/004-Phase3-IPC-Client-Server-Skeleton.md` — Phase 3 task requiring an explicit document authority and behavior model decision before protocol implementation.
- `plans/003-Phase2-EditorInteractionModel.md` — Establishes that local editor mutations are byte-offset/range based and suitable for future protocol edit operations.
- Deno Core Context7 docs for `JsRuntime` event loop operations — Reviewed to confirm that deno_core processes event-loop ticks, timers, and async operation completions through runtime polling; this supports treating V8 as a managed server runtime rather than an unconditional per-keypress synchronous hot path.
- [VS Code Remote Development](https://code.visualstudio.com/docs/remote/remote-overview) — Documents the value of running commands/extensions in containers, WSL, SSH, or remote environments with no source code required locally.

## Consequences

- Phase 3 protocol work should include document identifiers and leave room for client IDs, lease IDs, base document versions, transaction IDs, server versions, and behavior versions, even if Phase 3 does not fully enforce them yet.
- Server code should be structured toward per-document canonical actors/tasks rather than a stateless behavior service.
- Client code should keep local editing immediate and transport asynchronous.
- Behavior should be represented through versioned server-issued manifests/snapshots for hot-path client execution.
- Commands/keybindings need explicit routing policies: client-first predictable, server-first, server-first with lock, UI-reactive priority lane, or background.
- AI/extension behavior changes should use scoped locks and manifest updates rather than allowing concurrent ambiguous editing behavior.
- WASM hot-path behavior modules remain a possible future architecture extension, but not part of Phase 3.
- This decision should be revisited only if measurements show that client-executed behavior manifests are insufficient, or if the product intentionally changes away from server-authoritative remote/container/workspace semantics.
