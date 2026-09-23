---
date: 2026-08-26 18:38
status: approved
decision_about: "Server-side per-document syntax sessions, atomic viewport patches, and deferred client-local parsing"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Keep package-flexible syntax server-side and publish atomic viewport patches

## Decision

Clay will keep package-selected syntax parsing and syntax-management behavior server-side. Each document/grammar stream will use one bounded, latest-wins syntax session off the connection and Tokio worker hot paths, and the server will publish explicit request-scoped atomic viewport patches to CodeMirror.

CodeMirror remains the local text, viewport, incremental position-index, and inert rendering/projection owner, not the authoritative package syntax runtime. Client-local parsing remains deferred and may be reconsidered only as a separately approved accelerator or fallback after the optimized server path is measured; it must not silently replace package-defined syntax behavior.

## Context

The Plan 099 performance review found that current delays are dominated by document-sized frontend position indexing, duplicate document sessions, fragmented decoration updates, connection-loop follow-up work, synchronous native parsing on Tokio, grammar-global parser mutexes, and heuristic viewport completion. These problems do not require moving syntax authority into the webview.

Clay's syntax architecture intentionally supports TypeScript/JavaScript packages that declare or execute syntax behavior in the controlled server runtime. Packages can select tiered parser implementations, contribute grammar/query/style-map metadata, publish validated syntax/semantic decorations, and preserve package provenance. Theme packages separately contribute inert `textStyles` that map Clay's `TokenType + Modifiers` vocabulary to presentation. A theme package itself does not manage parsing; a language/syntax package manages classification, then the active theme styles its vocabulary.

CodeMirror can technically parse and highlight locally. Its language packages install parser extensions; `ParseContext` supports incremental/background viewport-aware parsing; `syntaxHighlighting` installs a `Highlighter`; `StateField` and decorations support custom projected ranges; and `Compartment.reconfigure` changes installed extensions at runtime. This preserves theme styling and fixed host-defined extension points.

It does not preserve Clay's full server-package flexibility automatically. Arbitrary package-defined parsing or syntax-management code can affect a client-local CodeMirror parser only if Clay does one of the following:

1. loads package JavaScript and parser objects into the webview;
2. loads package artifacts into a separate frontend worker and defines a new worker trust/protocol model; or
3. restricts packages to an inert schema that a fixed client parser adapter can interpret.

The first two add client execution/artifact authority and duplicate server parser state. The third preserves safety but is less flexible than the current tiered server package runtime. A hybrid local parser plus server package overlay also creates two syntax owners, ordering/parity rules, duplicate trees, and temporary divergence.

Theme flexibility is not a reason to move parsing client-side. Clay's vocabulary already separates syntax classification from styling: a server parser emits inert `TokenType + Modifiers`, and the active theme maps that vocabulary to CodeMirror classes/styles. Theme changes can therefore remain client-local and instant without moving package parser execution into CodeMirror.

## Approval

- Proposed by: Agent, after repository audit and current CodeMirror documentation review.
- Approved by user: Yes.
- Approval evidence: User stated, "Architecture is approved."
- Clarification captured: User explained that server-side syntax was designed to preserve TypeScript package flexibility, including theme-integrated syntax management, and asked whether equivalent flexibility would survive client-local CodeMirror parsing.

## Alternatives Considered

1. **Optimized server syntax sessions with atomic viewport patches** — selected. Preserves package runtime flexibility, grammar provenance, tiered parser support, one syntax authority, headless parity, and the existing trust boundary while fixing measured scheduling/transport/application costs.
2. **CodeMirror/Lezer as authoritative parser for bundled languages** — not selected. Excellent local incremental integration and full theme support, but package-defined parser behavior would require client execution or a narrower inert adapter; server/headless syntax would diverge or remain duplicated.
3. **Tree-sitter in a frontend worker** — not selected. Could preserve grammar family and remove server round trips, but requires a new artifact integrity, worker sandbox, package permission, lifecycle, memory, and duplicate-tree architecture.
4. **Local bundled parser beneath authoritative server package overlays** — deferred. Could improve provisional color latency, but introduces two syntax owners and merge/flicker/parity costs. Consider only after optimized server measurements show a remaining visible gap.
5. **Keep current fragmented server path** — rejected. It preserves flexibility but retains proven frontend O(document) work, duplicate state, parser fan-out, Tokio blocking, and lossy/heuristic viewport delivery.

## Rationale and Evidence

- Clay's existing two-axis vocabulary decouples parsing from themes. Theme packages remain inert style data whether parsing occurs server-side or locally.
- CodeMirror supports local parsers, highlighters, decoration state, and dynamic extension reconfiguration, so fixed host-controlled local parsing is technically possible.
- Equivalent arbitrary TypeScript package behavior is not free: CodeMirror parser/highlighter extensions are runtime objects/functions. Package-provided behavior must execute in the editor runtime or be translated into a deliberately bounded inert contract.
- Current Clay policy explicitly denies package JavaScript in client input/render/layout paths and keeps package/runtime authority server-side. Moving arbitrary parser extensions into the webview would reverse that boundary.
- The performance review identified shared algorithm/scheduling defects before server-to-client latency. Fixing those defects is the smaller and safer route to the target.
- One server syntax session per document/grammar with explicit atomic patches is consistent with the already-approved parse-once-per-edit and complete-authoritative-replacement decisions.

## References

- [CodeMirror reference: syntax highlighting, parsing, state fields, decorations, and compartments](https://codemirror.net/docs/ref)
- [CodeMirror extension overview: language data, highlighting, folding, indentation, and linting](https://codemirror.net/docs/extensions/)
- [CodeMirror migration guide: decoration StateField and dynamic Compartment reconfiguration](https://codemirror.net/docs/migration/)
- Context7 library `/websites/codemirror_net`, queried 2026-08-26 for client-local parser/package extension points.
- `docs/development/editor-performance-review-2026-08-26.md`
- `plans/099-Clay-Editor-Performance-Overhaul.md`
- `docs/reference/primitives/syntax-vocabulary.md`
- `docs/reference/packages/creating-packages.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
- `.agents/skills/project-patterns/references/tauri-react-client.md`
- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
- `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
- `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`

## Consequences

- TypeScript/JavaScript syntax packages retain current server-runtime flexibility and package provenance.
- Themes remain independent inert mappings from syntax vocabulary to presentation and can update CodeMirror styles without reparsing.
- Server connection dispatch returns after canonical work/required responses; advisory parsing runs in bounded per-document sessions.
- Native parser work moves to a bounded blocking executor with one active latest-wins job per document/grammar.
- Viewport results become explicit, complete, versioned atomic patches; Tauri coalesces obsolete whole patches only.
- CodeMirror receives no arbitrary package parser, JavaScript, raw CSS, native handle, Tauri, filesystem, network, or process authority.
- Client-local parser work requires a new decision if measurements justify it. That decision must specify whether it is provisional-only or authoritative, package/artifact trust, worker/webview isolation, headless parity, memory duplication, merge precedence, and fallback behavior.
- Revisit when optimized minimum-device traces still exceed the approved viewport-to-current-syntax target and show server/bridge latency, rather than frontend patch application or query cost, as the remaining cause.
