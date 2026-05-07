# Documentation as Code Pattern

## Core Rule

Clay must be self-documenting. Documentation is part of the code contract and must be inspectable by users and AI agents.

## Public Surfaces That Need Documentation

- Protocol messages and fields.
- Commands and keybindings.
- Behavior manifest entries and routing policies.
- Permissions and capabilities.
- Server APIs exposed to extensions.
- Client APIs exposed to server-driven UI or manifests.
- Extension APIs and AI tools.
- SDUI schema elements.
- File/workspace operations.

## Registry Pattern

Prefer one source of truth that can produce:

- Human-readable Markdown reference.
- Machine-readable agent index.
- UI help/command palette descriptions.
- Extension author documentation.
- Tests that verify required documentation exists.

Acceptable sources include Rust attributes/macros, structured registry files, or generated docs, as long as the source is close to the code and hard to forget.

## Planning Guidance

When a plan adds a public surface, include:

- Where its documentation metadata lives.
- How generated docs or indexes are updated.
- What tests fail if docs are missing.
- How an AI agent discovers the capability.
- How a user inspects it.

## Anti-Patterns

- Free-floating docs that drift from code.
- Protocol or command additions without docs.
- AI tools whose capabilities are only implicit in source code.
- User-facing behavior that cannot be discovered from the app.
