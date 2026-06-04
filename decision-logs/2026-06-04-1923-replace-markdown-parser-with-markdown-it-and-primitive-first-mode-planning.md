---
date: 2026-06-04 19:23
status: approved
decision_about: "Replace Markdown parser implementation with markdown-it and require primitive-first mode planning"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Replace Markdown parsing with markdown-it and plan modes primitive-first

## Decision

Clay will fully remove the `mdast-util-from-markdown`-based Markdown parser implementation and replace it with a rewritten `markdown-it` adapter inside the first-party `@clay/markdown` JavaScript package. Rust server/client code must remain Markdown-agnostic: Rust may add or extend generic editor primitives, but Markdown-specific parsing, token interpretation, style choices, and mode behavior must live in the Markdown mode package.

Clay phase plans that implement new mode packages must include a dedicated primitive-review task before package implementation. That task must inventory existing Rust-side primitives, assess what the package can already do, plan only generic new primitives when needed, and then build package functionality on top of that primitive library.

## Context

Phase 18 initially used `mdast-util-from-markdown` because positioned mdast nodes mapped naturally to decoration byte ranges. A later benchmark task measured actual parser performance on large Markdown corpora built from existing repository Markdown files instead of dummy documents. The result showed full-document `mdast-util-from-markdown` parsing and the current mdast adapter path are not acceptable for durable large-file editing.

The user also clarified an architectural requirement: no Markdown-specific logic should live on the Rust server or client. Rust should provide reusable editor primitives that can support Markdown now and future modes, such as Python mode, without mode-specific branches.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: The user requested: “I would like to fully remove mdast-util-from-markdown based implementation and fully replace it with markdown-it” and “I also want you to log a decision on this topic.”

## Alternatives Considered

1. **Keep `mdast-util-from-markdown` for the Phase 18 POC only** — rejected. Although it provided convenient positioned AST nodes, benchmark evidence showed poor large-file parser throughput, and retaining it would leave the package with a parser path already known to be unsuitable for ordinary large-file editing.
2. **Keep both mdast and markdown-it adapters** — rejected for this phase. Dual adapters would preserve deprecated code, complicate package tests, and dilute the proof that the Markdown package can be implemented cleanly on one parser boundary.
3. **Move Markdown-specific parsing or decoration logic into Rust for performance** — rejected. This violates the package/mode architecture goal and would not help future modes reuse the same primitives.
4. **Rewrite `@clay/markdown` around `markdown-it` and generic Rust primitives** — selected. It follows benchmark evidence, keeps Markdown logic in the JS package, and exercises the primitive library in a way that benefits later language modes.

## Rationale and Evidence

- Large-file parser benchmark evidence from `tools/bench/markdown-parser.mjs` showed:
  - `mdast-util-from-markdown` took about 1.28 s on a 1.01 MiB repository-Markdown corpus and about 16.24 s on a 5.03 MiB corpus.
  - `mdast-util-from-markdown` did not complete a 16.03 MiB parse within a 120 s local guard window; an earlier combined 16 MiB run exceeded 600 s.
  - `markdown-it` completed comparable 1.01 MiB, 5.03 MiB, and 16.03 MiB corpora in about 66.5 ms, 397.6 ms, and 849.7 ms.
  - The existing mdast adapter path took about 49.3 s for 1.01 MiB because byte-offset conversion repeatedly scanned from the start of the document.
- Context7 official markdown-it documentation lookup for `/markdown-it/markdown-it` describes markdown-it as a token-stream parser, not a traditional AST. Top-level parsing produces block tokens, opening and closing tags are represented as separate tokens, and inline container tokens have `children` token streams for inline markup.
- Context7 markdown-it token documentation shows token fields such as `type`, `tag`, `attrs`, `map`, `nesting`, `level`, `children`, `content`, `markup`, `info`, `meta`, `block`, and `hidden`. This implies the Clay adapter must be rewritten around token traversal and source/line mapping rather than mdast node traversal.
- The existing project pattern already requires no full-document IPC for ordinary edits and no synchronous JavaScript/IPC before normal typing paint. A `markdown-it` adapter must still publish viewport-bounded inert decoration spans through generic parse/decorations primitives.

## References

- `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md` — Phase 18 plan being rewritten around `markdown-it`.
- `tools/bench/markdown-parser.mjs` — actual parser benchmark harness using existing repository Markdown files.
- `docs/development/performance.md` — benchmark result documentation and parser recommendation.
- `docs/wiki/modules/first-party-markdown-package.md` — implementation wiki page to update for the parser replacement.
- `docs/wiki/modules/performance-fixtures.md` — parser benchmark documentation.
- `.agents/skills/project-patterns/references/protocol-and-performance.md` — no full-document IPC and no hot-path JavaScript/IPC rules.
- Context7 CLI lookup: `/markdown-it/markdown-it` docs for token stream architecture and Token object shape.

## Consequences

- The Phase 18 plan must include cleanup work that removes mdast dependency declarations, mdast adapter code, mdast-specific tests/docs, and decision references that would imply mdast remains active.
- `@clay/markdown` must get a new `markdown-it` adapter. The adapter should use markdown-it token streams and package-owned source/line mapping to produce Clay `DecorationSpan`s without rendering HTML.
- Rust may add generic primitives, such as parse/decorations, mode activation, inert editor rules, line/range metadata, or primitive documentation/testing infrastructure, but must not add Markdown-specific parser branches or style logic.
- Project guidance must teach agents to review primitive inventory before implementing new JS mode packages and to keep the primitive wiki/index/test coverage current.
- Revisit this decision only if `markdown-it` cannot provide correct byte ranges or package-owned token adaptation after implementation/testing, or if a future approved decision selects another parser with stronger correctness/performance evidence.
