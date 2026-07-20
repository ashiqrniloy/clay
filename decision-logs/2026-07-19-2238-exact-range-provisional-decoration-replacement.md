---
date: 2026-07-19 22:38
status: approved
decision_about: "Exact-range authoritative replacement of provisional decoration state"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Subtract authoritative ranges from provisional decoration chunks

## Decision

When a current authoritative `DecorationSet` overlaps provisional client decoration state for the same package and layer, Clay will replace only the set's declared viewport. `EditorDecorationState` will subtract that viewport from overlapping provisional spans/chunks, preserve left/right residual geometry outside it, install authoritative spans inside it, and locally coalesce compatible residual fragments so repeated edits do not grow chunk count without bound.

This decision supersedes the whole-overlapping-provisional-chunk removal behavior approved or retained by `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`. It retains that decision's same-word interpolation, complete server replacement chunks, one parse/query pass, server syntax authority, and fixed bounded transport/cache behavior.

## Context

Manual testing after Plan 057 showed a third composed decoration defect. Per-letter base-color flashing at the insertion point was fixed, but typing inside a Rust comment caused one additional byte in following code to lose decoration per typed byte.

The server owns a current-version 128-byte replacement grid. The client optimistically transforms existing chunk geometry through edits. For an insertion before a boundary, old chunks such as `[0,128)` and `[128,256)` become provisional `[0,129)` and `[129,257)`. The server may correctly publish only the touched current-grid chunk `[0,128)`. `EditorDecorationState::apply_set` currently removes every overlapping provisional chunk wholesale, installs `[0,128)`, and retains the next chunk beginning at `129`, leaving `[128,129)` undecorated. Repeated insertions shift the retained chunk again and grow the gap by one byte per edit.

Plan 057's invariant—query coverage equals replacement coverage—holds inside each newly published server set. It does not guarantee that a new authoritative set plus retained client chunks still tile current document geometry when client chunk boundaries have shifted. Tests covered server chunk completeness and optimistic token continuity separately but did not compose repeated edits before a chunk boundary with partial authoritative replacement and downstream paint inspection.

## Approval

- Proposed by: agent.
- Approved by user: Yes.
- Approval evidence: After asking whether the strategy would introduce prior latency problems, the user stated, "Approved. Go ahead, log the decision and create the plan using @.agents/skills/create-plan/."

## Alternatives Considered

1. **Keep deleting whole overlapping provisional chunks** — rejected because overlap is broader than the authoritative viewport and deterministically creates decoration holes when optimistic chunk boundaries drift.
2. **Republish the touched server chunk plus downstream neighbors** — rejected as extra query/transport/cache work. Individually streamed members can still expose a transient gap or clear a larger provisional chunk before its neighbor arrives.
3. **Add an atomic multi-chunk transport message** — correct only when all old/new grid reconciliation chunks are included, but requires unnecessary protocol and client event changes for a local replacement-semantics defect.
4. **Rechunk all optimistic syntax state onto the server's 128-byte grid after each edit** — rejected because it duplicates server grid/UTF-8 rules in the client, couples generic editor state to syntax internals, and adds synchronous edit-path work.
5. **Increase chunk size or debounce syntax publication** — rejected because these only reduce symptom frequency and leave replacement semantics incorrect.
6. **Subtract the exact authoritative viewport from overlapping provisional state and coalesce local residuals** — chosen. It matches the declared viewport semantics, preserves geometry outside server authority, requires no parser/protocol/configuration changes, and keeps work bounded to already-overlapping near-viewport chunks/spans when a server set arrives.

## Rationale and Evidence

`DecorationSet` carries `viewport_byte_start` and `viewport_byte_end`, and `DecorationChunkKey` uses the same range. That range is the server's authority claim. Removing provisional bytes outside it violates the shape's existing meaning even when the removed chunk intersects it.

`src/editor/surface.rs::EditorDecorationState::apply_set` already scans retained chunks and filters same-package/same-layer provisional overlaps. The correction belongs there: replace whole-chunk deletion with exact range subtraction and preserve residual span fragments. This is generic across syntax packages and arbitrary chunk boundaries and does not require language names, Tree-sitter behavior, or knowledge of the 128-byte server grid.

The work occurs when an asynchronous authoritative set arrives, not before optimistic text rendering. Local editing and same-word interpolation remain immediate. No extra parse, query, IPC message, server chunk, or package JavaScript call is introduced. Implementation must avoid a global normalization pass: inspect only overlapping provisional chunks, split only affected spans/chunks, and coalesce only compatible residuals created or touched by the current replacement.

Regression coverage must compose real grammar output, repeated local comment edits before a replacement boundary, edit acknowledgements, each authoritative set, and visible paint ranges. It must cover insertion and deletion drift, empty authoritative sets, UTF-8 boundaries, and bounded residual/chunk growth.

## References

- `src/editor/surface.rs::{EditorDecorationState::apply_set,EditorDecorationState::apply_edit,interpolate_range}`
- `src/protocol/decorations.rs::{DecorationSet,DecorationChunkKey}`
- `src/server/syntax.rs::{replacement_ranges,decoration_sets_for_ranges}`
- `tests/syntax_grammar.rs`
- `tests/decoration_transport.rs`
- `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`
- `.agents/skills/project-patterns/references/{protocol-and-performance,authority-boundaries}.md`

## Consequences

- Current authoritative output remains final inside its declared viewport.
- Provisional geometry outside that viewport survives even when its original chunk overlapped the authoritative range.
- Repeated edits cannot create visible byte gaps at shifted/current chunk boundaries.
- Empty authoritative sets clear exactly their declared range, not neighboring provisional geometry.
- Residual fragments require localized splitting/coalescing and accurate cache accounting; tests must prevent unbounded fragment growth.
- Parser/query counts, transport member counts, protocol shapes, package permissions, Clay JS APIs, and configuration remain unchanged.
- Revisit atomic decoration batches only if a separately measured multi-member transition remains visible after exact-range subtraction.
