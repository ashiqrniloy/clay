# Documentation as Code Pattern

Clay must be self-documenting. For public programmatic behavior, the documented public surface is the Clay JavaScript API. Markdown documentation is the source of truth for Clay JS APIs, and generated registries/lookup APIs are derived from the Markdown set.

Decision sources:

- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`

Related patterns:

- `clay-js-api-boundary.md`: Rust-to-JS exposure boundary and facade rule.
- `doc-registry-tests.md`: Required registry freshness and coverage tests.

## Public Surfaces Covered

Apply the generated Markdown registry contract to Clay JS APIs, including APIs for editor commands, protocol/message helpers, behavior manifest helpers, permissions/capabilities, extension APIs, AI tools, SDUI schema helpers, and file/workspace operations.

Internal Rust implementation details belong in the project code wiki, not the public documentation registry, unless they are exposed through Clay JS APIs.

## Required Clay JS API Documentation

Each inspectable Clay JS API Markdown page must include:

- What the API does, why/when to use it, searchable user-facing name, and how to call it from JavaScript or TypeScript.
- A concrete code example.
- Key binding metadata, using an empty list when no default key binding exists.
- Custom properties for every behavior-changing configurable setting, including type/default/allowed values where relevant.
- Configuration/options, defaults, allowed values, return value, and async/sync behavior.
- Errors/failure modes, permissions/capabilities, authority boundaries, and security notes.
- Backing Rust function path, `deno_core` op wrapper path/name, and JS facade module path.
- Stability/versioning notes and lookup tags for app/help/agent discovery.

## Markdown-Authoritative Registry

- Author Clay JS API documentation as Markdown files.
- Include every public Clay JS API Markdown file in `docs/index.md` or the project’s master Markdown index.
- Put required metadata in Markdown/frontmatter: stable ID, kind, owner, visibility, security notes, agent guidance, app/help visibility, JS module/export, backing Rust path, op name, user-facing name, key bindings, custom properties, and lookup tags.
- Generate app/agent documentation registry entries from the master index and linked Markdown files.
- Expose lookup APIs over the generated registry for app help, command palette, extension tooling, and AI discovery.
- Do not hand-author a separate registry as the source of truth.

## Planning Guidance

When a plan adds or changes server-side Rust public functions or public programmatic behavior, identify:

- Which Clay JS API exposes each server-side Rust public function, or why the Rust function is made private/`pub(crate)`.
- Which Markdown file documents the Clay JS API and where it is linked from the master index.
- How the generated registry is updated from Markdown.
- Which `cargo test` coverage test fails if the Rust function, Clay JS API, docs, index link, registry entry, or lookup API is missing/stale.
- How users and AI agents discover and inspect the capability by stable ID, kind, owner, JS module/export, backing Rust path, op name, or tag.

## Anti-Patterns

- Free-floating docs not linked from the master Markdown index.
- Raw Rust public functions or raw `Deno.core.ops.op_*` calls treated as the user-facing JavaScript API.
- Clay JS APIs without Markdown docs, examples, options, or authority boundaries.
- A separately authored registry becoming the source of truth.
- AI/tool capabilities only implicit in source code.
- Tests that pass when Clay JS APIs, docs, master-index links, generated registry entries, or lookup APIs are missing.
