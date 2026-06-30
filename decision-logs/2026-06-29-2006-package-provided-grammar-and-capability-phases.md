---
date: 2026-06-29 20:06
status: approved
decision_about: "Package-provided syntax grammars and post-Phase-18 capability sequencing"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Package-provided grammar packages before post-Phase-18 capability phases

## Decision

After Phase 18, Clay will insert a Phase 18.8-18.14 capability sequence before Phase 19 hot reload and later hardening. Syntax highlighting will not start by bundling Rust, TypeScript, and JavaScript grammars in Clay core; it will start with grammar-only first-party packages (`@clay/rust`, `@clay/typescript`, and `@clay/javascript`) that provide Tree-sitter grammar/query contributions through a generic package primitive.

Those language packages are grammar-only at first. They do not provide full modes until a later expansion phase after generic command execution, transient menus, fallback modes/key behavior, syntax highlighting, completion, workspace/file navigation, and Git package foundations exist.

## Context

The user listed near-term Clay capability requirements: syntax highlighting primitives, completion framework, key behavior exposure, generic text/code modes, mode discovery, file browser, workspace discovery, Git/branch discovery, first-party Git package, Control Center, and bottom-pane transient menu workflows. The initial recommendation sequenced these as new phases but proposed core-bundled grammars for syntax highlighting. The user approved the direction except for that point, requiring first-party packages to provide grammars immediately so Clay proves grammar contributions can come from packages.

Current roadmap already puts hot reload in Phase 19 after the Phase 18 package/mode/runtime sequence. This decision inserts the new capability sequence between Phase 18.7 and Phase 19 so hot reload is exercised later against several real package contribution types, not only Markdown.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: User said, "Yes I agree with the direction mostly except for Point 1, Syntax Highlighting ... change that one to already start building the first party packages for Rust, Typescript and Javascript. These packages now will not provide modes but rather only provide the grammar. We will expand them later. This also proves the grammar being provided by external packages work. Otherwise, create the phases as proposed but start from after Phase 18. Phase 19 onwards we will do after these are done. Also create the decision log."

## Alternatives Considered

1. **Bundle Rust/TypeScript/JavaScript grammars in Clay core** — Rejected because it would not prove package-provided grammar contributions and would couple core editor code to specific languages too early.
2. **Build full Rust/TypeScript/JavaScript modes immediately** — Rejected because generic primitives for command execution, transient menus, fallback modes, syntax, completion, workspace navigation, and Git should exist first. Full modes should consume primitives, not force one-off Rust branches.
3. **Allow arbitrary third-party grammar/native artifact loading now** — Deferred because loading parser artifacts can create security/trust concerns. First-party grammar packages prove the package path without opening general third-party grammar loading before a dedicated trust/integrity decision.
4. **Keep Phase 19 hot reload immediately after Phase 18.7** — Rejected by sequencing decision. Hot reload now waits until the inserted Phase 18.8-18.14 capability sequence creates enough real package contributions to validate reload semantics.

## Rationale and Evidence

- Clay's existing primitive model already separates server-side package work from client hot paths. `docs/reference/primitives/registry.md` defines `IncrementalParseUpdate`, `DecorationRange`, and deferred `CompletionTriggerAndResult` as server-validated, bounded primitives, and forbids package JavaScript in client keypress/paint/text-event handlers.
- `docs/wiki/modules/parse-coordinator.md` records that parse work is background, cancellable, versioned, bounded by parse/window budgets, and publishes inert parse/decorations rather than executable parser code to the client.
- `docs/wiki/modules/rendering-primitives.md` records that inline decorations are inert data validated by the server and rendered locally by the Rust client with Parley/Vello; this matches a Tree-sitter query-to-decoration pipeline.
- Tree-sitter official documentation supports the editor use case: the Rust binding uses `InputEdit` plus `parser.parse(new_source, Some(&old_tree))` for incremental parsing, and Tree-sitter highlight queries map syntax nodes to capture names that Clay can convert to style tokens/decorations.
- `docs/reference/primitives/backlog.md` already lists `CompletionTriggerAndResult` as a deferred primitive and records mode/package primitives as generic reusable backlog rows, supporting the decision to promote completion after transient menu and syntax foundations.
- `.agents/skills/project-patterns/references/mode-primitive-first.md` requires future modes to inventory existing primitives, add only generic reusable gaps, and avoid mode-specific Rust branches.
- `.agents/skills/project-patterns/references/package-distribution.md` requires package behavior to be explicitly loaded and to flow through Clay-owned validation, permissions, provenance, docs, and runtime boundaries.

## References

- `roadmap.md` — updated with Phase 18.8 through Phase 18.14 before Phase 19.
- `docs/reference/primitives/registry.md` — primitive taxonomy for parse, decoration, completion, commands, mode activation, and package security.
- `docs/reference/primitives/backlog.md` — completion and language/mode primitive backlog evidence.
- `docs/wiki/modules/parse-coordinator.md` — current parse coordinator authority, budget, and hot-path constraints.
- `docs/wiki/modules/rendering-primitives.md` — current decoration/rendering authority and inert client-rendering contract.
- `.agents/skills/project-patterns/references/mode-primitive-first.md` — reusable project rule to build modes on generic primitives.
- `.agents/skills/project-patterns/references/package-distribution.md` — package validation, loading, and authority boundary guidance.
- `npx ctx7@latest library tree-sitter "Clay roadmap decision: use Tree-sitter syntax highlighting where first-party @clay/rust, @clay/typescript, and @clay/javascript packages provide grammars as package contributions; need docs on parser languages, queries, incremental parsing, Rust binding API, external grammar packages."` — selected official `/tree-sitter/tree-sitter` docs.
- `npx ctx7@latest docs /tree-sitter/tree-sitter "Tree-sitter editor syntax highlighting design: parser set language, parse with old tree for incremental parsing, edit tree with InputEdit, Query/QueryCursor highlight queries, byte/point ranges; package grammars loaded/provided externally."` — official evidence for incremental parsing and highlight queries.

## Consequences

- `roadmap.md` now inserts Phase 18.8-18.14 before Phase 19: transient menu/command execution, generic text/code modes and key behavior, package-provided syntax grammars, completion, workspace/file browser, Git/`@clay/git`, and later Rust/TypeScript/JavaScript package expansion.
- Clay core owns the syntax grammar contribution contract, validation, parse scheduling, and decoration transport; first-party language packages own grammar/query assets.
- `@clay/rust`, `@clay/typescript`, and `@clay/javascript` initially prove grammar-only packages. Full language modes, LSP, formatter/toolchain execution, semantic indexing, and AI code actions remain later work.
- Arbitrary third-party grammar/native loading remains out of scope until a later security/trust decision defines allowed artifacts, integrity, sandboxing, and user authorization.
- Future phase plans for language packages must include primitive review, package-provided grammar tests, docs/registry coverage, and explicit fallback behavior when packages are disabled or invalid.
