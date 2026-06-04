# Project Wiki Page Template

Use or adapt this template when creating or substantially rewriting implementation wiki pages.

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

## Primitive Coverage

Use this section when the page covers reusable editor/package primitives.

- Primitive/category name and owning Rust/source module.
- JS facade/op/protocol shape when present.
- Permissions, validation rules, hot-path policy, and payload/performance budgets.
- How current and future modes/packages should reuse it without mode-specific Rust branches.

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
