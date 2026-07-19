---
date: 2026-07-19 03:51
status: approved
decision_about: "Low-latency incremental syntax parsing and provisional decoration rendering"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Parse once per edit and preserve provisional token decoration

## Decision

Clay will process every accepted text edit through one cancellable/coalesced incremental Tree-sitter parse per document grammar stream, using stable bounded windows, exact `InputEdit` metadata, `Tree::changed_ranges`, and changed-range highlight queries. The client will interpolate existing inert decoration spans through optimistic local edits so token ranges remain visually stable while authoritative versioned decorations are pending.

Visible decoration transitions follow parser token/capture boundaries, not letters and not whitespace alone. Insertions strictly inside an existing capture extend it provisionally; broad captures such as comments, strings, prose, and code blocks may provisionally inherit at their edge. Current authoritative syntax replaces affected token/range spans atomically. Syntax stays available beneath slower semantic decoration.

## Context

Clay currently shows newly typed or invalidated text with the base brush until asynchronous syntax decoration arrives. The visible white-to-token-color transition is especially distracting inside comments, strings, and prose where the surrounding capture strongly predicts the provisional style.

The current server pipeline also amplifies work. First-party grammars use a 4 KiB parse-window budget. `schedule_parse_window` divides that window into contiguous 256-byte viewports and schedules up to 16 sibling handler jobs. Each `TreeSitterSyntaxHandler` job parses the same window through a shared parser mutex before querying one viewport. Edit-centered window starts move as typing advances, while cached-tree reuse requires an identical `window_start`; even when the start is stable, sibling jobs for the same document version do not all reuse the first result. Decoration transport chunking has therefore become parse-task chunking.

The user proposed word-by-word decoration, with immediate predictable treatment for long same-style regions such as text and comments. Industry evidence supports token-range updates and foreground interpolation, but not delaying analysis solely until spaces/newlines: punctuation, delimiters, operators, cursor movement, and EOF are also syntax boundaries, and single characters can change comment/string structure.

## Approval

- Proposed by: both. User proposed word/boundary-level decoration and predictable broad-span inheritance; agent proposed parse amplification removal, exact incremental Tree-sitter edits, changed-range querying, and provisional client interpolation.
- Approved by user: Yes.
- Approval evidence: User stated, "The architecture is approved."

## Alternatives Considered

1. **Keep clearing intersecting decoration spans and only prioritize the edited chunk** — rejected because it preserves the base-color flash and leaves repeated same-window parsing intact.
2. **Parse only after space or newline** — rejected because lexical/syntactic boundaries include punctuation, quotes, brackets, operators, cursor movement, and EOF; comments/strings may change on one delimiter character.
3. **Debounce all parsing until idle** — rejected because it deliberately increases freshness latency and can leave structurally important edits stale. Newer versions should coalesce/cancel old work, but the latest accepted edit is always eligible for parsing.
4. **Run a second Tree-sitter parser in the client** — deferred. It could remove IPC latency but duplicates grammar/tree state and complicates package provenance, memory budgets, and server-authoritative syntax selection. Revisit only if measurements show one optimized server parse still misses the visible-latency target.
5. **Keep stale spans unchanged without interpolation** — rejected because ranges become misaligned after edits. Edit-aware interpolation preserves geometry while acknowledging that provisional style may briefly be semantically wrong.
6. **Language-specific comment/string heuristics in Rust** — rejected. Existing generic `TokenType`/capture metadata is sufficient; language grammar/query data remains package-owned.

## Rationale and Evidence

Tree-sitter's documented incremental path is to edit the old tree with exact change coordinates and parse again with that tree so unchanged structure is shared. Its query cursor can restrict matching to byte ranges intersecting syntactic changes.

Zed applies edits to/interpolates its foreground syntax state immediately, attempts a very small synchronous parse budget, and completes incremental parsing in the background. It computes `old_tree.changed_ranges(new_tree)`, joins those with explicit invalidations, and limits follow-up query work to changed ranges. VS Code tokenizes with carried line state and can retokenize a line as if a character were inserted to determine the inserted character's token type. These implementations support immediate provisional state plus authoritative token-range correction rather than whitespace-only parsing.

Clay can adopt the useful parts without adding client parser execution: local span interpolation remains inert, bounded rendering state; server parsing remains asynchronous and authoritative for package grammar selection/provenance; stale versions remain rejected; paint consumes cached spans only.

## References

- [Tree-sitter incremental editing](https://github.com/tree-sitter/tree-sitter/blob/f45a488dea5c98a93721566a2098a658dea73ecd/docs/src/using-parsers/3-advanced-parsing.md#L3-L22)
- [Tree-sitter restricted query ranges](https://github.com/tree-sitter/tree-sitter/blob/f45a488dea5c98a93721566a2098a658dea73ecd/docs/src/using-parsers/queries/4-api.md#L65-L84)
- [Zed foreground interpolation/background reparse](https://github.com/zed-industries/zed/blob/edeaf598c7495bd7b9e9a05d68e61f08ad275d16/crates/language/src/buffer.rs#L1832-L1922)
- [Zed incremental changed ranges](https://github.com/zed-industries/zed/blob/edeaf598c7495bd7b9e9a05d68e61f08ad275d16/crates/language/src/syntax_map.rs#L664-L829)
- [VS Code inserted-character tokenization](https://github.com/microsoft/vscode/blob/56d6f639fb09e6610c9eb8f56439496b9536e283/src/vs/editor/common/model/textModelTokens.ts#L75-L101)
- `src/server/connection.rs::schedule_parse_window`
- `src/server/syntax.rs::TreeSitterSyntaxHandler::parse_sync`
- `src/server/parse_coordinator.rs::schedule_parse_with_windows`
- `src/editor/surface.rs::EditorDecorationState::apply_edit`
- `docs/wiki/modules/{parse-coordinator,parse-task-lifecycle,decoration-transport,syntax-grammar-registry}.md`
- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`

## Consequences

- Ordinary text still paints immediately without waiting for IPC, parser work, package JavaScript, or semantic analysis.
- One parse produces all affected visible decoration output; 256-byte transport/cache chunks no longer create repeated parse jobs.
- Rapid typing cancels or coalesces superseded versions while preserving latest-version work.
- Comments, strings, prose, code blocks, and edits inside existing token spans avoid most base-color flashes.
- Provisional spans can be briefly wrong after a closing delimiter or structural edit; current authoritative results correct them atomically.
- Server remains canonical for document versions and syntax selection; client receives/interpolates only inert spans and gains no package parser or JavaScript authority.
- No new package, filesystem, network, shell, AI, raw-op, native-widget, or client-JavaScript authority is introduced.
- Revisit client-side native parsing only after measured optimized server latency remains visibly insufficient.
