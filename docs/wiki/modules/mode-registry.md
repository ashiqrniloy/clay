# Mode Registry

## Source

- `src/packages/modes.rs`
- `tests/package_primitive_gate.rs`

## Overview

The mode registry is the Phase 16.5 server-side primitive gate for document classification and major-mode activation. It accepts package-declared, static mode metadata after the package manifest validator has already approved package identity, prefix, declared modes, and permissions.

## Responsibilities

- Register package-owned major mode declarations with provenance.
- Classify open-document metadata by extension, MIME hint, exact filename, or a bounded basename wildcard pattern.
- Activate one server-owned major mode per document and assign behavior-version metadata.
- Reject undeclared modes, duplicate mode IDs, malformed static patterns, and missing `mode-registration` or `mode-activation` permissions.

It does not execute package JavaScript, scan the filesystem, install package managers, or make the Rust client authoritative for mode selection.

## How It Works

`ModeRegistry::register_mode` takes a validated `ClayPackageManifest` and a `ModeDeclaration`. Registration checks that:

1. The package declared `mode-registration`.
2. Declaration provenance matches the manifest name, version, and `apiPrefix`.
3. The mode ID is package-owned and appears in `clay.modes`.
4. Static patterns are well formed and unique.
5. No enabled package already registered the same mode ID.

`ModeRegistry::classify` receives `DocumentClassificationInput` for an open document. It uses only the supplied path basename/extension and optional MIME hint. Match priority is exact filename, filename wildcard, extension, then MIME type. Equal-priority matches from different modes are rejected as ambiguous.

`ModeRegistry::activate_major_mode` requires `mode-activation`, verifies that the classification still belongs to the package manifest, and writes `MajorModeActivation` into server-owned registry state. Re-activating a document replaces the previous major mode deterministically and increments the behavior version.

## Code Examples

```rust
let mut registry = ModeRegistry::new();
registry.register_mode(&manifest, markdown_mode_declaration)?;
let classification = registry.classify(&DocumentClassificationInput {
    document_id: 7,
    path: Some("README.md".to_string()),
    mime_type: None,
})?;
let activation = registry.activate_major_mode(&manifest, classification)?;
```

## Invariants and Constraints

- A document has at most one active major mode in `active_major_modes`.
- Mode activation references `MODE_ACTIVATION_P95_BUDGET_MS` through the registry API, but Phase 16.5 does not add a hard latency CI gate.
- Patterns are static metadata only: no callbacks, client predicates, filesystem scans, or raw ops.
- The Rust client receives future validated behavior/protocol data; it does not select or execute package modes itself.

## Tests

- `tests/package_primitive_gate.rs`: validates Markdown extension classification, duplicate mode rejection, malformed pattern rejection, one-major-mode activation replacement, behavior-version increments, and required permissions.

Run focused coverage with:

```text
cargo test --test package_primitive_gate
```

## Related

- [Package Primitive Gate](package-primitive-gate.md)
- [Primitive Architecture](primitive-architecture.md)
- `docs/reference/primitives/registry.md#DocumentClassification`
- `docs/reference/primitives/registry.md#MajorModeActivation`
- `docs/reference/primitives/markdown-mode-requirements.md`
