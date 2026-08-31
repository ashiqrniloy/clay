# Maintenance Validation

Use tests or deterministic checks for workflow-maintained artifacts whenever practical.

- Prefer automated checks over instruction-only maintenance for docs, wiki indexes, generated registries, and public API coverage.
- Planned Clay checks include: wiki index links every wiki page, Clay JS API docs are linked from `docs/index.md`, generated documentation registry is current, server-side Rust public functions have Clay JS APIs, and Clay JS APIs have Markdown docs plus generated registry entries.
- Checks should fail with actionable repair commands when available, e.g. `cargo run --bin update-doc-registry` for stale registry artifacts.
- Checks should detect stale artifacts during test/CI runs; they should not silently mutate files.
- Wiki pages and code must not contradict each other. When code and a wiki page disagree (e.g. a page claims backends were removed while the code remains), reconcile by matching reality, and prefer deleting dead code over re-documenting it when there are zero callers.
- Platform-gated code that Linux CI never compiles must be either wired to a real caller or deleted; unreachable platform code is rot, not support. Native picker implementations for pending platform targets stay out until the platform phase lands, and docs must name them as targets, not as shipped behavior.
- When deleting a Rust module, also clear every metadata citation of it: plan for the `clay_js_api_inventory` path-existence guards (`source_paths_named_by_public_metadata_exist`, `inventory_rust_paths_name_existing_source_files`) by updating the affected Markdown docs' `backing_rust` and `api-inventory.toml`'s `backing_rust`/`current_rust_owner` in the same change, then rerun `cargo run --bin update-doc-registry`. Decision source: decision-logs/2026-08-31-1745-delete-dead-native-dialog-backends.md.
