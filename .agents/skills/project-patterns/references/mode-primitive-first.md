# Mode Primitive-First Planning

- Every phase plan that implements a new editor mode or JS mode package must include a dedicated primitive-review task before package implementation.
- The primitive-review task must read the primitive reference docs and implementation wiki first, especially `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/rendering-primitives.md`, `docs/wiki/modules/parse-coordinator.md`, `docs/wiki/modules/decoration-transport.md`, and mode/package-specific wiki pages.
- Plan package behavior in this order: inventory existing generic Rust primitives, document what can already be achieved, identify gaps, add only generic reusable primitives when needed, then implement the JS package on top of those primitives.
- Rust server/client code must not add mode-specific branches for package behavior. New primitives must be reusable by future modes such as Markdown, Python, or other language modes.
- Plans that add or change primitives must include documentation/test work so every primitive is recorded in reference docs, wiki pages, the master wiki index, and deterministic coverage tests such as `tests/primitives_docs.rs` or a successor primitive-documentation test.
- When an AI agent works on a JS package, it should consult the primitive wiki before designing package code so the package reuses existing primitives rather than rediscovering or duplicating Rust capabilities.
- Decision log source: `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`.
