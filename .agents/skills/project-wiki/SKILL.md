---
name: project-wiki
description: Maintain a Markdown code wiki for implementation knowledge. Use whenever an AI agent writes, modifies, reviews, or plans code changes in any project. The skill ensures public and internal implementation details are documented in a master-indexed Markdown wiki so a developer unfamiliar with the project can learn what the code does, how it works, and how to use or extend it.
---

# Project Wiki

Keep a project-local Markdown code wiki current with implementation knowledge. The wiki is educational: a developer unfamiliar with the project should understand, use, debug, and extend the codebase by reading it.

## Workflow

Use this workflow whenever writing or changing code:

1. Find the wiki location. Prefer the project convention; otherwise use `docs/wiki/` with `docs/wiki/index.md` as the master index.
2. Read relevant existing wiki pages before coding, especially pages for files/modules/components being changed.
3. Update the wiki once after the completed change or plan passes tests, unless the user asks for per-task updates.
4. Keep the master index navigable. Link every discoverable wiki page and briefly state what it teaches.
5. Keep code and wiki synchronized when behavior, architecture, data flow, dependencies, examples, or tests change.
6. When creating or substantially rewriting a page, use `.agents/skills/project-wiki/references/page-template.md` if it exists.

## Wiki Scope and Boundary

Document implementation units and behaviors at enough depth for onboarding; do not document every trivial line.

Include:

- Public surfaces at an implementation level: APIs, CLIs, protocols, configuration, commands, extension points, UI surfaces, and user-visible behavior.
- Internal modules, components, functions, data structures, state machines, algorithms, control flow, and interactions.
- Cross-cutting concerns: error handling, validation, security boundaries, performance constraints, concurrency, persistence, and testing strategy.
- Important tradeoffs, invariants, assumptions, and known limitations.

For public programmatic APIs, link to the authoritative API/reference documentation instead of duplicating it. The wiki should explain the implementation behind those APIs. In Clay, Clay JS API docs in `docs/reference/` and `docs/index.md` are authoritative for public programmatic usage; `docs/wiki/` explains internals and links to those docs when relevant.

## Required Wiki Content

Each relevant wiki page should explain:

- What files/modules/components/functions it covers.
- What the implementation does and its responsibilities.
- How it works: flow, data structures, algorithms, state transitions, and code interactions.
- Why it is shaped this way: constraints, tradeoffs, invariants, and assumptions.
- How to use or extend it, with examples where useful.
- How it is tested, including test paths and commands.
- Related wiki pages, reference docs, and source paths.

## Default Structure

When the project has no convention, prefer:

```text
docs/wiki/
  index.md
  architecture.md
  modules/<module-name>.md
  flows/<flow-name>.md
  concepts/<concept-name>.md
```

## Quality Bar

A wiki update is good enough when:

- The master index links to the page.
- The page covers what the implementation does and how it does it.
- Internal implementation details are documented, not only public interfaces.
- Public API usage docs are linked instead of duplicated when another authoritative reference exists.
- Helpful code examples or command examples are included.
- Source paths and test paths are listed.
- The documentation matches the final code after tests pass.

## Avoid

- Updating only public API docs while leaving internal implementation undocumented.
- Duplicating authoritative public API reference docs instead of linking to them.
- Creating disconnected Markdown pages not linked from the master index.
- Copying large source files into the wiki instead of explaining the implementation.
- Writing vague summaries that do not explain how the code works.
- Letting generated docs or comments replace the educational code wiki unless the project explicitly uses those generated artifacts as wiki pages.
