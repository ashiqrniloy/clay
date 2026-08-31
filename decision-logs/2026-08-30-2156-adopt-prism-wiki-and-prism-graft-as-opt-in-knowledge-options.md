---
date: 2026-08-30 21:56
status: approved
decision_about: "Knowledge-base options: prism-wiki and prism-graft"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Adopt `@arnilo/prism-wiki` and `@arnilo/prism-graft` as opt-in knowledge options

## Decision

Clay adopts `@arnilo/prism-wiki` 0.0.3 (per-codebase `.wiki/` knowledge
base, OKF v0.2 bundles, SHA-256 Merkle-diff `/wiki-refresh`,
`wiki_search`/`wiki_read_page`/`wiki_record_insight` tools,
`wiki-maintainer`/`wiki-searcher` skills, optional `qmd` hybrid search
with catalog fallback) and `@arnilo/prism-graft` 0.0.1 (context-graph
code search: `graft_ask`/`graft_grep`/`graft_callers`/`graft_skeleton`/
`graft_map`/`graft_blast` pull tools, gated push retrieval packs,
first-turn orientation, post-edit blast radius via `graft:dirty`) as
**opt-in, per-workspace options** for the clay coding agent. Both load
via `kernel.load(...)` only when the user enables them; both surface to
`st` by inheritance. The Graft CLI resolves fail-closed (`cliPath` /
host `packageRoot` / optional `@nanonets/graft` peer) and its tools are
hidden when the CLI is absent (mirrors `agy` handling).

## Context

The user wanted llm-wiki-style knowledge bases and graft-style efficient
code search available to the clay coding agent and thus to `st`.
Research found `@arnilo/llm-wiki` does not exist on npm; the Prism
monorepo ships `@arnilo/prism-wiki` (the adapted llm-wiki) and
`@arnilo/prism-graft` as first-party packages, so no third-party
adoption is needed.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: user stated they want to implement the llm-wiki
  package to initiate/update a per-codebase wiki, check out the
  prism-graft package, and that they need to adopt both for the clay
  coding agent so they are available as options and thus to `st`
  (2026-08-30). Approved for logging: "Go ahead and log the decisions as
  decided already" (2026-08-30).

## Alternatives Considered

1. **kaijunhe/llm-wiki (npm `llm-wiki` 0.1.3)** — rejected: third-party,
   not Prism-integrated; `@arnilo/prism-wiki` is the same concept
   adapted into Prism with skills/tools wired for the registry.
2. **Always-on knowledge bases** — rejected; opt-in per workspace keeps
   the base agent minimal and leaves no residue when disabled.
3. **`@arnilo/prism-rag` + `prism-memory` (pgvector)** — rejected for
   now: server-side scale beyond need; wiki + graft cover the knowledge
   and code-search cases (recorded as not-adopted in the roadmap
   adoption map).

## Rationale and Evidence

- npm registry: `@arnilo/prism-wiki@0.0.3` ("Karpathy LLM Wiki system
  with local qmd hybrid search and Context7 navigation for Prism"),
  `@arnilo/prism-graft@0.0.1` ("Graft context-graph integration for
  Prism"), both published and current.
- Phase 2 exit gate already specifies fixture checks: wiki
  init/refresh/lint + `wiki_search` answering from an OKF bundle; graft
  ask/callers returning ranked spans; no residue when disabled.

## References

- `roadmap.md` — Phase 2 (knowledge-base options), Phase 0 (pins),
  Prism Adoption Map rows for `prism-wiki` / `prism-graft`, resolved
  item 4.
- npm: `@arnilo/prism-wiki`, `@arnilo/prism-graft`, `llm-wiki`
  (searched 2026-08-30).

## Consequences

- Positive: wiki gives durable per-codebase knowledge; graft gives
  sub-linear code search and blast-radius safety; both inherited by
  `st` without extra wiring.
- Risks: graft CLI availability varies by host — fail-closed hiding
  handles it; `qmd` optional with catalog fallback.
- Revisit if: per-workspace opt-in proves too frictional for `st`
  autonomous runs (could default-on per project).