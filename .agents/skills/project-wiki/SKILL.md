---
name: project-wiki
description: Maintain a Markdown code wiki for implementation knowledge. Use whenever an AI agent writes, modifies, reviews, or plans code changes in any project. The skill ensures public and internal implementation details are documented in a master-indexed Markdown wiki so a developer unfamiliar with the project can learn what the code does, how it works, and how to use or extend it.
---

# Project Wiki

## Purpose

Keep a project-local Markdown code wiki current with the implementation. The wiki is educational, not just API reference: a developer unfamiliar with the project should be able to gain the implementation knowledge needed to understand, use, debug, and extend the codebase by reading the wiki.

## When Working on Code

Use this workflow whenever writing or changing code:

1. **Find the wiki location.** Prefer the project's existing convention. If none exists, use `docs/wiki/` with `docs/wiki/index.md` as the master index.
2. **Read relevant wiki pages before coding** when they exist, especially pages covering files/modules/components being changed.
3. **Update the wiki after implementation and tests pass.** Do this once for the completed change or plan, not after every small task unless the user asks.
4. **Keep the master index navigable.** Add or update links in the master index for every wiki page that should be discoverable.
5. **Keep code and wiki synchronized.** If code behavior, architecture, data flow, dependencies, or examples change, update the corresponding wiki pages in the same work.

## Wiki Scope

Document both public and internal implementation:

- Public APIs, CLIs, protocols, configuration, commands, extension points, UI surfaces, and user-visible behavior.
- Internal modules, components, functions, data structures, state machines, algorithms, and control flow.
- Cross-cutting concerns such as error handling, validation, security boundaries, performance constraints, concurrency, persistence, and testing strategy.
- Important implementation tradeoffs, invariants, assumptions, and known limitations.

Do not document every trivial line. Document implementation units and behaviors at enough depth that a new developer can understand what the code does and how it achieves it.

## Required Wiki Content

Each relevant wiki page should explain:

- **What it covers:** files/modules/components/functions included.
- **What it does:** behavior and responsibilities.
- **How it works:** implementation flow, important data structures, algorithms, state transitions, and interactions with other code.
- **Why it is shaped this way:** constraints, tradeoffs, invariants, and design assumptions.
- **How to use or extend it:** concrete code examples, command examples, or integration examples where useful.
- **How it is tested:** test files, important test cases, and how to run them.
- **Related pages/code:** links to wiki pages and paths to source files.

## Markdown Structure

Prefer this default structure when the project has no convention:

```text
docs/wiki/
  index.md
  architecture.md
  modules/
    <module-name>.md
  flows/
    <flow-name>.md
  concepts/
    <concept-name>.md
```

The master index should be a Markdown navigation page with sections by domain/module/flow. It should link every wiki page and briefly state what each page teaches.

## Page Template

Use or adapt this template:

```markdown
# <Implementation Topic>

## Source

- `<path/to/source.ext>`
- `<path/to/tests.ext>`

## Overview

What this implementation does and where it fits.

## Responsibilities

- Main responsibility.
- Boundary or non-responsibility.

## How It Works

Explain the implementation step by step. Include data flow, control flow, state, algorithms, concurrency, or IO details as relevant.

## Code Examples

```<language>
<minimal realistic example>
```

## Invariants and Constraints

- Important invariant or assumption.
- Performance/security/concurrency constraint.

## Tests

- `<test path>`: what it validates.
- Command to run relevant tests.

## Related

- [Related wiki page](../path.md)
- `<related/source/path>`
```

## Quality Bar

A wiki update is good enough when:

- The master index links to the page.
- The page covers both what the implementation does and how it does it.
- Internal implementation details are documented, not only public interfaces.
- Code examples or command examples are included where they help understanding.
- Source paths and test paths are listed.
- The documentation matches the final code after tests pass.

## Avoid

- Updating only public API docs while leaving internal implementation undocumented.
- Creating disconnected Markdown pages not linked from the master index.
- Copying large source files into the wiki instead of explaining the implementation.
- Writing vague summaries that do not explain how the code works.
- Letting generated docs or comments replace the educational code wiki unless the project explicitly uses those generated artifacts as wiki pages.
