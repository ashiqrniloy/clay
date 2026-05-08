# Documentation Registry Tests

Decision source: `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`.

Plans and implementations that add or change Clay JS APIs must include tests or acceptance criteria for:

- A non-mutating generation/check function that `cargo test` can run.
- A Cargo update command that rewrites checked-in generated registry artifacts when Markdown changes, e.g. `cargo run --bin update-doc-registry`.
- Failure when a server-side Rust public function lacks a Clay JS API.
- Failure when a Clay JS API lacks Markdown documentation.
- Failure when Markdown docs are missing from the master Markdown index.
- Failure when required metadata is missing, including JS usage, examples, options/configuration, user-facing name, key binding metadata, custom property metadata, authority notes, and backing Rust/op/facade paths.
- Failure when the checked-in generated registry is stale.
- Failure when generated entries are unavailable through app/help/agent lookup APIs.

`cargo test` must detect stale artifacts and print the update command; it must not silently mutate files.
