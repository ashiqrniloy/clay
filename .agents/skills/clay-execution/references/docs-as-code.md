# Documentation as Code, Wiki Workflow, and Privacy of History

## Documentation as Code

Clay must be self-documenting. For public programmatic behavior the documented surface is the Clay JavaScript API. Markdown is the source of truth for Clay JS APIs; generated registries/lookup APIs are derived from the Markdown set.

Sources: `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`, `2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`. Related: `js-api.md` (boundary + registry tests).

### Public Surfaces Covered

Apply the generated Markdown registry contract to Clay JS APIs for editor commands, protocol/message helpers, behavior manifest helpers, permissions/capabilities, extension APIs, AI tools, SDUI schema helpers, and file/workspace operations. Internal Rust implementation details belong in the code wiki, not the public registry, unless exposed through Clay JS APIs.

### Required Clay JS API Documentation

Each inspectable Clay JS API Markdown page must include: what it does, why/when to use it, searchable user-facing name, how to call it from JS/TS; a concrete code example; key binding metadata (empty list when no default); custom properties for every behavior-changing configurable setting (type/default/allowed values where relevant); configuration/options, defaults, allowed values, return value, async/sync behavior; errors/failure modes, permissions/capabilities, authority boundaries, security notes; backing Rust function path, `deno_core` op wrapper path/name, JS facade module path; stability/versioning notes and lookup tags for app/help/agent discovery.

### Markdown-Authoritative Registry

- Author Clay JS API docs as Markdown; include every public file in `docs/index.md` (the master Markdown index); put required metadata in Markdown/frontmatter (stable ID, kind, owner, visibility, security notes, agent guidance, app/help visibility, JS module/export, backing Rust path, op name, user-facing name, key bindings, custom properties, lookup tags).
- Generate app/agent documentation registry entries from the master index + linked Markdown; expose lookup APIs over the generated registry (app help, command palette, extension tooling, AI discovery). Never hand-author a separate registry as source of truth.

### Planning Guidance

When a plan adds or changes server-side Rust public functions or public programmatic behavior, identify: which Clay JS API exposes each Rust public function (or why it is private/`pub(crate)`); which Markdown file documents it and where it is linked from the master index; how the generated registry is updated; which `cargo test` coverage test fails if the function/API/docs/index link/registry entry/lookup is missing or stale; how users and agents discover it (stable ID, kind, owner, JS module/export, backing Rust path, op name, tag); whether the changed user-facing configuration surface must be added to the canonical example configuration `examples/init.js` (mandatory per-plan task, see `create-plan/references/clay.md`); which `test-plan/` module files need new/updated manual steps for user-visible behavior changes (mandatory per-plan manual test plan task; index and coverage matrix in `test-plan/index.md`).

### Anti-Patterns

Free-floating docs not linked from the master Markdown index; raw Rust public functions or raw `Deno.core.ops.op_*` calls as the user-facing API; Clay JS APIs without Markdown docs/examples/options/authority boundaries; a separately authored registry as source of truth; AI/tool capabilities implicit only in source; tests that pass when APIs/docs/index links/registry entries/lookup are missing; `examples/init.js` drifting behind the implemented surface; manual verification steps living only in plan documents or chat history instead of `test-plan/` module files.

## Wiki Workflow

Keep the project-local Markdown code wiki (`docs/wiki/`) current with implementation knowledge. The wiki is educational: a developer unfamiliar with the project should understand, use, debug, and extend the codebase by reading it.

Workflow whenever writing or changing code:

1. Read relevant existing wiki pages before coding, especially pages for files/modules/components being changed.
2. Update the wiki once after the completed change or plan passes tests, unless the user asks for per-task updates.
3. Keep the master index (`docs/wiki/index.md`) navigable: link every discoverable wiki page and briefly state what it teaches.
4. Keep code and wiki synchronized when behavior, architecture, data flow, dependencies, examples, or tests change.
5. When creating or substantially rewriting a page, follow the page template below.
6. When work adds or changes reusable editor primitives, package/mode primitives, protocol primitives, or JS package capabilities built on primitives, document the primitive inventory comprehensively: source paths, public/reference docs, implementation wiki pages, tests, permissions, hot-path policy, how future modes should reuse the primitive.
7. Add or update deterministic tests where practical so every primitive remains recorded in reference docs, wiki pages, and the master wiki index (e.g. `tests/primitives_docs.rs` or successor documentation-coverage tests that fail without mutating files).

### Scope and Boundary

Document implementation units and behaviors at enough depth for onboarding; do not document every trivial line.

Include:

- Public surfaces at an implementation level: APIs, CLIs, protocols, configuration, commands, extension points, UI surfaces, user-visible behavior.
- Reusable primitives at an implementation level: primitive name/category, owning Rust module, JS facade/op when present, package permission, protocol shape, hot-path policy, validation rules, source/test paths, reference-doc links, examples of package or mode reuse.
- Internal modules, components, functions, data structures, state machines, algorithms, control flow, interactions.
- Cross-cutting concerns: error handling, validation, security boundaries, performance constraints, concurrency, persistence, testing strategy.
- Important tradeoffs, invariants, assumptions, known limitations.

For public programmatic APIs, link to the authoritative API/reference documentation instead of duplicating it; explain the implementation behind those APIs. In Clay: Clay JS API docs in `docs/reference/` and `docs/index.md` are authoritative for public programmatic usage; `docs/wiki/` explains internals and links to those docs when relevant.

### Required Wiki Content

Each relevant page explains: what files/modules/components/functions it covers; what the implementation does and its responsibilities; how it works (flow, data structures, algorithms, state transitions, code interactions); why it is shaped this way (constraints, tradeoffs, invariants, assumptions); how to use or extend it, with examples where useful; how it is tested (paths and commands); related wiki pages, reference docs, source paths.

### Page Template

```markdown
# <Implementation Topic>

## Source
- `<path/to/source.ext>`, `<path/to/tests.ext>`

## Overview
What this implementation does and where it fits.

## Responsibilities
- Main responsibility; boundary or non-responsibility.

## How It Works
Data flow, control flow, state, algorithms, concurrency, or IO details.

## Code Examples
```<language>
<minimal realistic example>
```

## Primitive Coverage   (when the page covers reusable primitives)
- Primitive/category name and owning Rust/source module.
- JS facade/op/protocol shape when present.
- Permissions, validation rules, hot-path policy, payload/performance budgets.
- How current and future modes/packages should reuse it without mode-specific Rust branches.

## Invariants and Constraints
- Important invariant or assumption; performance/security/concurrency constraint.

## Tests
- `<test path>`: what it validates; command to run them.

## Related
- [Related wiki page](../path.md); related source path.
```

### Quality Bar

A wiki update is good enough when: the master index links the page; the page covers what the implementation does and how it does it; internal implementation details are documented, not only public interfaces; public API usage docs are linked instead of duplicated; helpful code/command examples are included; source and test paths are listed; documentation matches the final code after tests pass.

### Avoid

Updating only public API docs while leaving internals undocumented; duplicating authoritative public API reference docs instead of linking; disconnected Markdown pages not linked from the master index; copying large source files into the wiki instead of explaining; vague summaries that do not explain how the code works; letting generated docs or comments replace the educational wiki unless the project explicitly uses those artifacts as wiki pages; adding or changing primitive implementations without wiki/reference coverage and deterministic tests; using the retired `clay.<domain>.*` spelling for core Clay command/API IDs (core IDs are bare `<domain>.<name>`, package-owned IDs start with the package prefix; only `clay:` import specifiers and `package.json` `clay.*` manifest key paths keep the prefix — see `js-api.md`).

## Privacy of History

- `plans/`, `decision-logs/`, `code-reviews/`, and `docs/wiki/archive/` hold historical records: keep them immutable, never delete, never link from hot-path indexes, never scan proactively. Superseding decisions get new entries referencing earlier ones.
- Completed-phase review records go to `docs/wiki/archive/`, not `docs/wiki/modules/`. The master wiki index carries one `## Archive` pointer line only: historical phase/review/bugfix records live in `archive/` and are pull-only.
- When code is removed, correct every surviving document at removal time (see archive rule above and the metadata-citation rule in `protocol-perf.md`); removal annotations do not keep a page on the hot path.