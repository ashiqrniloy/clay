# Documentation as Code Pattern

## Core Rule

Clay must be self-documenting. Documentation is part of the code contract and must be inspectable by users and AI agents through generated artifacts, indexed files, and app/programmatic lookup paths.

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
- Separate indexed lookup artifact keyed by stable ID, kind, owner, and tags.
- App/programmatic lookup APIs for UI help, command palette, extension tooling, and AI tool discovery.
- Extension author documentation.
- Tests that verify required documentation exists for every public interface surface.

Acceptable sources include Rust attributes/macros, structured registry files, or generated docs, as long as the source is close to the code and hard to forget.

## Planning Guidance

When a plan adds a public surface, include:

- Where its documentation metadata lives.
- How generated docs, agent indexes, and lookup indexes are updated.
- What public-interface tests fail if docs are missing.
- How an AI agent discovers the capability through machine-readable/indexed docs.
- How a user inspects it through generated docs or the app.
- How app/programmatic lookup resolves it by stable ID, kind, owner, or tag.

## Anti-Patterns

- Free-floating docs that drift from code.
- Protocol or command additions without docs.
- Documentation that exists only as Markdown and cannot be indexed or queried programmatically.
- AI tools whose capabilities are only implicit in source code.
- User-facing behavior that cannot be discovered from the app.
- Public interface tests that pass even when documentation metadata or generated indexes are missing.
