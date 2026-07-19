---
date: 2026-07-19 19:12
status: approved
decision_about: "Syntax decoration continuity and complete authoritative replacement coverage"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Preserve same-word syntax locally and replace only fully queried decoration ranges

## Decision

Clay will keep parsing every accepted edit, but visual continuity will use same-word provisional inheritance: an existing syntax span may extend through an appended Unicode alphanumeric or underscore suffix, while whitespace, newline, punctuation, and structural edits end narrow-token inheritance. Server syntax remains authoritative, and each published `DecorationSet` must contain complete capture state for exactly the UTF-8-safe range it replaces; changed-range queries must cover the full replacement chunks before publication.

This decision supersedes the visual-boundary and authoritative-replacement portions of `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md` where they conflict. It retains that decision's one parse per accepted version/window, cancellation/coalescing, server parser authority, bounded transport/cache, and no client parser defaults.

## Context

Manual testing after Plan 056 showed that the approved architecture did not solve the reported flicker and made newline behavior worse. Comments initially decorated correctly, but pressing Enter could make all decoration in a short file disappear; ordinary code still flashed from the base brush to syntax color letter by letter.

Code-path review found two composed defects:

1. `src/editor/surface.rs::interpolate_decoration_span` intentionally excludes insertion at the end of narrow syntax spans, so every appended word byte paints with the base brush until server output arrives.
2. `src/server/syntax.rs` queries only the normalized changed envelope, then `decoration_sets_for_range` aligns publication to whole 128-byte chunks. Such a set can omit unchanged captures inside the larger range it claims to replace. `EditorDecorationState::apply_set` correctly removes overlapping provisional package/layer chunks before installing authoritative data, so omitted captures disappear. A short file commonly occupies one chunk, matching the all-white newline report.

Plan 056 tests checked parser captures, synthetic interpolation, and replacement behavior separately. They did not compose real grammar output, optimistic local edit, edit acknowledgement, and each streamed authoritative set while checking visible style after every transition.

## Approval

- Proposed by: both. The user originally proposed word-by-word decoration with newline/boundary triggers; the agent proposed preserving parse-on-every-edit correctness while using word boundaries for provisional visual inheritance and complete chunks for authoritative replacement.
- Approved by user: Yes.
- Approval evidence: User stated, "Strategy approved. Create the superseding decision log and complete the first task Reproduce visual regressions through the complete decoration state machine and update the plan once done" after reviewing Plan 057.

## Alternatives Considered

1. **Keep Plan 056 behavior** — rejected because manual testing disproved its intended visual outcome and code review found replacement coverage wider than query coverage.
2. **Parse only on whitespace/newline** — rejected because quotes, comment delimiters, brackets, operators, and punctuation can change syntax immediately.
3. **Delay all authoritative publication until a word boundary** — rejected because it adds buffering and stale-state complexity while delaying structural correction.
4. **Merge partial authoritative output with stale client spans** — rejected because absence must remain authoritative; client merging would duplicate parser semantics and make stale style difficult to clear safely.
5. **Query and republish the full 4 KiB parse window after every keypress** — correct but rejected as unnecessary query, transport, and render churn.
6. **Add a client Tree-sitter parser** — deferred. It duplicates grammar/tree state and package authority; revisit only if complete bounded server replacements plus same-word interpolation still miss measured visual targets.
7. **Query complete touched replacement chunks and extend existing syntax through same-word suffixes** — chosen as the smallest root-cause correction preserving current authority and performance boundaries.

## Rationale and Evidence

Tree-sitter's `QueryCursor::set_byte_range` returns matches intersecting its configured byte range. Therefore Clay can reconstruct a replacement chunk completely by expanding query coverage to that exact UTF-8-safe chunk before capture extraction; it does not need to query the full document. `Tree::changed_ranges` remains the source for identifying affected syntax after editing the old tree and parsing the new tree.

The client already performs bounded inert span interpolation synchronously before server work. Extending an existing narrow syntax span only for same-word suffix characters reuses that path, removes repeated base-brush exposure after initial classification, and stops at the user's requested word/newline boundaries without creating a second tokenizer. The first character of a newly created token may still transition once when the server first classifies it; removing that last transition would require delayed publication or client parsing and is outside this decision.

Correctness must be tested through the composed state machine. Regression coverage will apply real first-party grammar output to `EditorSurface`, perform local edits, acknowledge versions, stream authoritative members in order, and inspect visible paint ranges after each step. This directly tests the state observed by the GUI without introducing nondeterministic GPU snapshots.

## References

- `plans/057-Syntax-Decoration-Continuity-and-Replacement-Correctness.md`
- `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
- `src/server/syntax.rs::{decorations_for_window,decoration_sets_for_range}`
- `src/editor/surface.rs::{EditorDecorationState::apply_edit,EditorDecorationState::apply_set,interpolate_decoration_span}`
- `tests/syntax_grammar.rs`
- `tests/decoration_transport.rs`
- `docs/wiki/modules/{decoration-transport,masonry-editor,parse-coordinator,syntax-grammar-registry}.md`
- Tree-sitter 0.25.10 local Rust source for `Tree::changed_ranges` and `QueryCursor::set_byte_range`
- [Tree-sitter query API](https://tree-sitter.github.io/tree-sitter/using-parsers/queries/4-api.html)

## Consequences

- Parsing remains eligible on every accepted edit; whitespace/newline does not become a scheduling gate.
- Existing classified syntax can remain continuously colored through same-word suffix typing with no server wait.
- Whitespace, newline, punctuation, and structural changes continue to receive prompt authoritative correction.
- Authoritative empty sets clear only ranges that were fully queried.
- Query coverage may expand from a tiny changed range to touched 128-byte chunks, trading a small bounded amount of capture work for replacement correctness.
- No new Clay JS API, runtime setting, package permission, client JavaScript, language-specific Rust branch, or client parser is introduced.
- The first newly created token character may still transition once on initial server classification. Revisit delayed publication or client parsing only if manual and measured validation shows that residual transition remains unacceptable.
