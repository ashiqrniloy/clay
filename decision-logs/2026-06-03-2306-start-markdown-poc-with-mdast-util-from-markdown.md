---
date: 2026-06-03 23:06
status: approved
decision_about: "Markdown parser package for Phase 18 proof of concept"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Start Markdown POC with mdast-util-from-markdown

## Decision

Clay Phase 18 will start the first-party `@clay/markdown` proof of concept with `mdast-util-from-markdown` as the Markdown parsing dependency. The package will expose a narrow parser-adapter boundary that converts positioned mdast nodes into Clay `DecorationSpan` data; if this approach fails performance or implementation constraints, a later package or adapter can use `markdown-it` without changing the Rust UI rendering boundary.

## Context

Phase 18 aims to prove that Markdown editing and rendering can be implemented as a first-party JavaScript package while the Rust client renders only validated inert declarations. The project specifically wants to avoid implementing Markdown parsing from scratch when an existing package can perform the heavy parsing work.

The main alternatives compared were `mdast-util-from-markdown` and `markdown-it`:

- `mdast-util-from-markdown` produces a Markdown AST with source positions/offsets, making it easier to map Markdown constructs to Clay byte-range decorations.
- `markdown-it` is mature and likely faster in raw parsing, but its token model is more renderer-oriented and inline tokens do not consistently expose absolute byte offsets, requiring more Clay-specific offset reconstruction.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user said, "Let's start with mdast-util-from-markdown. My reasoning is that it is anyhow a package and if this does not work, I can create a new package with markdown-it."

## Alternatives Considered

1. **Hand-written minimal Markdown parser** — Not selected as the default because it duplicates existing package functionality and increases parser correctness risk before Clay has proven the package-controlled rendering path.
2. **`markdown-it` token adapter** — Kept as a viable fallback. It appears faster in quick local benchmarking and is mature, but it requires more adapter work to reconstruct exact byte ranges for inline tokens and therefore carries higher UI-visible range-correctness risk.
3. **`mdast-util-from-markdown` AST adapter** — Selected for the initial POC because positioned mdast nodes align naturally with Clay's decoration span model and the package can be replaced later behind the adapter boundary if performance fails.

## Rationale and Evidence

- Clay's rendering model needs exact, inert byte-range decorations, not HTML output. A positioned AST is a closer fit to this model than a renderer-first token stream.
- Context7 documentation for `/syntax-tree/mdast-util-from-markdown` shows the core `fromMarkdown` API and mdast node structures for headings, code, lists, and other Markdown constructs.
- Local inspection of `mdast-util-from-markdown@2.0.3` output showed `position.start.offset` and `position.end.offset` on Markdown nodes such as headings, emphasis, strong text, inline code, code blocks, and list structures.
- Context7 documentation for `/markdown-it/markdown-it` shows token objects with fields such as `type`, `map`, `markup`, `content`, and `children`. These are useful for block parsing, but inline child tokens require additional source-offset reconstruction for Clay decoration spans.
- Quick local Node benchmarking suggested `markdown-it` is significantly faster for full-document parses than `mdast-util-from-markdown`. This supports keeping `markdown-it` as a performance fallback, but not enough to outweigh the adapter simplicity and correctness benefits for the first POC.
- npm metadata checked during research:
  - `mdast-util-from-markdown@2.0.3`: MIT license, built on `micromark`, parses Markdown into mdast.
  - `markdown-it@14.2.0`: MIT license, mature Markdown parser/renderer package.

## References

- `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md` — updated to use `mdast-util-from-markdown` for the initial Markdown parser adapter.
- `docs/reference/primitives/markdown-mode-requirements.md` — Phase 18 Markdown parser/decorator requirements and required span kinds.
- `docs/reference/primitives/parse-update-strategy.md` — server-side background parse, viewport prioritization, stale-version rejection, and budget constraints.
- `docs/reference/primitives/rendering-strategy.md` — inert decoration span and Rust-client rendering constraints.
- `.agents/skills/project-patterns/references/package-distribution.md` — Clay package distribution and server-side JavaScript execution boundary.
- Context7 `/syntax-tree/mdast-util-from-markdown` docs — `fromMarkdown` API and mdast structure.
- Context7 `/markdown-it/markdown-it` docs — token structure and parser/renderer architecture considered as fallback.
- Commands used during research:
  - `npx ctx7@latest library mdast-util-from-markdown "Compare mdast-util-from-markdown with markdown-it for parsing Markdown into positioned AST/tokens for server-side JavaScript package decoration spans rendered by Rust UI"`
  - `MSYS_NO_PATHCONV=1 npx ctx7@latest docs /syntax-tree/mdast-util-from-markdown "API for parsing Markdown to mdast with positional offsets; extension model; using fromMarkdown for syntax trees and decoration spans"`
  - `npm view mdast-util-from-markdown version license dist.unpackedSize description dependencies`
  - `npm view markdown-it version dependencies license dist.unpackedSize description`

## Consequences

- The first implementation should add `mdast-util-from-markdown` as a dependency of `packages/markdown/package.json` and keep parser-specific logic behind `packages/markdown/src/parser.js` or equivalent adapter files.
- Tests must verify exact UTF-8 byte ranges, marker-derived ranges, viewport filtering, stale-version safety, and payload budgets.
- Benchmarks should explicitly record `mdast-util-from-markdown` activation/edit-region parse costs. If sustained performance issues appear, Clay can introduce a `markdown-it`-based adapter or separate package without changing the Rust UI decoration rendering path.
- The Rust client must remain parser-agnostic and continue rendering only validated Clay decoration spans.
