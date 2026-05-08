# Documentation as Code Pattern

## Core Rule

Clay must be self-documenting. For public programmatic behavior, the documented public surface is the **Clay JavaScript API**, not raw Rust public functions. Markdown documentation is the source of truth for Clay JS APIs, and generated registries/lookup APIs are derived from the Markdown set.

Decision sources:

- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`

## Clay JS API Boundary

- Do not expose arbitrary Rust public functions directly to JavaScript.
- Server-side Rust public functions must be exposed through explicit `deno_core` op wrappers and stable Clay JS/TS facade modules.
- Raw `Deno.core.ops.op_*` calls are implementation details; users and AI agents should call Clay JS APIs.
- Client-side Rust functions are not directly exposed to JavaScript. JavaScript effects on clients must flow through server-authoritative APIs, protocol updates, or behavior manifests.
- Server-side functions that should remain internal implementation details should be private or `pub(crate)`, not public.

## Public Surfaces That Need Documentation-as-Code Coverage

Apply the generated Markdown registry contract to Clay JS APIs, including APIs for:

- Editor commands and capabilities exposed programmatically.
- Protocol/message helpers exposed to extensions or agents.
- Behavior manifest helpers and routing policies exposed through JS.
- Permissions and capabilities exposed through JS.
- Server APIs exposed to extensions.
- Extension APIs and AI tools.
- SDUI schema helpers exposed through JS.
- File/workspace operations exposed through JS.

Internal Rust implementation details belong in the project code wiki, not the public documentation registry, unless they are exposed through Clay JS APIs.

## Required Clay JS API Documentation Content

Each inspectable Clay JS API Markdown page must include:

- What the API/function does.
- Why and when it should be used.
- How to use it from JavaScript or TypeScript.
- A concrete code example of usage.
- Configuration/options parameters, defaults, and allowed values if any.
- Return value and async/sync behavior.
- Errors or failure modes.
- Required permissions, capabilities, authority boundaries, and security notes.
- Backing Rust function path, `deno_core` op wrapper path/name, and JS facade module path.
- Stability/versioning notes and lookup tags for app/help/agent discovery.

## Markdown-Authoritative Registry Pattern

Use one documentation source of truth:

- Author Clay JS API documentation as Markdown files.
- Include every public Clay JS API Markdown file in a master Markdown index, e.g. `docs/index.md`.
- Put required metadata in Markdown/frontmatter, including stable ID, kind, owner, visibility, security notes, agent guidance, app/help visibility, JS module/export name, backing Rust path, op name, and lookup tags.
- Generate the app/agent documentation registry from the master Markdown index and linked Markdown files.
- Expose programmatic lookup APIs over the generated registry for app help, command palette, extension tooling, and AI tool discovery.
- Do not hand-author a separate registry as the source of truth.

## Generated Registry and Tests

Plans and implementations must include:

- A non-mutating generation/check function that `cargo test` can run.
- An update function or developer command that rewrites the checked-in generated registry when Markdown changes.
- Tests that fail when a server-side Rust public function lacks a corresponding Clay JS API.
- Tests that fail when a Clay JS API lacks Markdown documentation.
- Tests that fail when Markdown docs are missing from the master index.
- Tests that fail when required Markdown metadata is missing, including JS usage, examples, options/configuration, authority notes, and backing Rust/op paths.
- Tests that fail when the checked-in generated registry is stale.
- Tests that fail when generated entries are unavailable through lookup APIs.

`cargo test` should not silently modify files. It should fail with instructions to update the documentation registry using Cargo, e.g. `cargo run --bin update-doc-registry`, when generated artifacts are stale.

## Planning Guidance

When a plan adds or changes server-side Rust public functions or public programmatic behavior, include:

- Which Clay JS API exposes each server-side Rust public function; if no JS API is appropriate, make the Rust function non-public or `pub(crate)` instead.
- Which Markdown file documents the Clay JS API.
- Where that file is linked from the master Markdown index.
- How the generated registry is updated from Markdown.
- Which `cargo test` coverage test fails if the Rust function, Clay JS API, docs, or generated registry entry are missing.
- How an AI agent discovers the capability through the generated registry.
- How a user inspects what the API does, when to use it, how to call it, examples, and options.
- How app/programmatic lookup resolves it by stable ID, kind, owner, JS module/export, backing Rust path, op name, or tag.

## Anti-Patterns

- Free-floating docs that are not linked from the master Markdown index.
- Raw Rust public functions treated as the JavaScript API surface.
- Raw `Deno.core.ops.op_*` calls documented as the user-facing API instead of Clay JS facade functions.
- Clay JS API additions without Markdown docs.
- Public programmatic behavior whose JavaScript usage, examples, options, or authority boundaries are undocumented.
- A separately authored registry becoming the source of truth instead of Markdown.
- Documentation that exists only as Markdown but cannot be generated into a registry or queried programmatically.
- AI tools whose capabilities are only implicit in source code.
- User-facing behavior that cannot be discovered from Markdown, the generated registry, or the app.
- Public interface tests that pass when Clay JS APIs, Markdown docs, master-index links, generated registry entries, or lookup APIs are missing.
