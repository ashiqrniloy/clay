# Maintenance Validation

Use tests or deterministic checks for workflow-maintained artifacts whenever practical.

- Prefer automated checks over instruction-only maintenance for docs, wiki indexes, generated registries, and public API coverage.
- Planned Clay checks include: wiki index links every wiki page, Clay JS API docs are linked from `docs/index.md`, generated documentation registry is current, server-side Rust public functions have Clay JS APIs, and Clay JS APIs have Markdown docs plus generated registry entries.
- Checks should fail with actionable repair commands when available, e.g. `cargo run --bin update-doc-registry` for stale registry artifacts.
- Checks should detect stale artifacts during test/CI runs; they should not silently mutate files.
