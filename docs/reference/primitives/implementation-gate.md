# Primitive Implementation Gate

Phase 16.5 adds the smallest server-side implementation gate between the Phase 16 primitive architecture and the Phase 17 package/mode loading work. The gate validates package and mode inputs, records deterministic package provenance, and exposes only validated manifests, mode state, command metadata, or inert behavior-manifest data to later runtime paths.

This page is a public reference for Phase 17 implementers. It does **not** describe package installation, package fetching, dependency resolution, or user enable/disable workflows.

## Scope Boundary

The gate covers load-time and activation-time primitives only:

- package manifest validation for `name`, `version`, `clay.apiPrefix`, `clay.permissions`, `clay.modes`, `clay.entry`, and optional `clay.loadEntry`;
- permission validation against Clay's known primitive permission table;
- static document classification patterns for extensions, MIME types, literal filenames, and bounded filename patterns;
- server-owned major-mode activation with exactly one active major mode per document;
- package-prefixed command declarations, key binding metadata, routing policies, custom properties, and permission metadata;
- inert behavior-manifest contribution validation for key routing and Rust-known text transform declarations.

Phase 17 package installation and package-manager integration remain out of scope. Clay may later delegate package download, dependency resolution, lockfiles, integrity, cache, and registry access to an npm-compatible package manager, but Phase 16.5 only validates Clay-owned metadata and primitive declarations after a fixture or future package manifest is available.

## Supported Fixture Format

Tests and future package loading can feed the gate with a package metadata object shaped like package `package.json` plus Clay metadata:

```json
{
  "name": "@clay/markdown",
  "version": "0.1.0",
  "clay": {
    "apiPrefix": "markdown",
    "permissions": ["mode-registration", "mode-activation", "command-registration"],
    "modes": ["markdown"],
    "entry": "./dist/index.js",
    "loadEntry": "./dist/load.js"
  }
}
```

Accepted records keep `package_name`, `package_version`, and `api_prefix` provenance so later diagnostics, conflict handling, generated documentation, and AI-agent discovery can identify the owning package.

## Validation Failures

The implementation gate rejects invalid inputs before they become active. Diagnostics are structured around the package name, version, prefix, primitive category, contribution ID when available, and failed rule.

Required load-time failures include:

- invalid `apiPrefix` values that do not match `^[a-z][a-z0-9-]{1,31}$`;
- duplicate package prefixes;
- package-owned IDs that claim reserved `clay.*` names;
- unknown permissions;
- prohibited authorities such as filesystem, network, shell, AI mutation, WASM execution, raw `Deno.core.ops`, client-side JavaScript, package installation, package enable/disable mutation, or workspace mutation;
- raw op exposure metadata or public raw `Deno.core.ops.op_*` surfaces;
- client-side JavaScript hooks or executable text transform callbacks;
- malformed mode patterns;
- undeclared mode registration or mode activation authority;
- duplicate mode names;
- duplicate command IDs;
- ambiguous package key bindings.

The canonical package validation, permission validation, conflict handling, and prohibited-authority baseline is [Package Primitive Security and Provenance Requirements](package-security.md).

## Performance and Hot-Path Policy

Package validation and mode activation are load/open/reload/configuration-time operations. They must not be documented or wired as ordinary typing, paint, Masonry layout, or text-event handler work.

- Manifest and permission validation run at package fixture/load time.
- Document classification and major-mode activation run when a document is opened, reloaded, or explicitly reclassified.
- Command registration and behavior-manifest contribution validation run before activation or load completes.
- Client-first text transforms remain Rust-known inert manifest data and preserve `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` expectations.
- Major-mode activation references `MODE_ACTIVATION_P95_BUDGET_MS`; Phase 16.5 does not add a hard latency CI gate.

## Phase 17 Handoff

Phase 17 should build package install/enable/load workflows on this gate instead of re-deriving primitive security rules. The handoff is:

1. discover or receive package metadata from the future package-management workflow;
2. pass the Clay package metadata through manifest and permission validation;
3. register declared mode patterns, commands, and inert behavior-manifest contributions;
4. classify open-document metadata using server-owned static patterns;
5. activate exactly one major mode per document and publish only validated behavior manifests, SDUI, or protocol data to clients.

Phase 17 still needs the user-facing package installation and enable/load workflow, persistent package state, compatibility checks, documentation registry integration for package-provided APIs, and any approved configuration surfaces.

## Phase 18 Handoff

Phase 18 Markdown work can depend on the gate for Markdown package identity, `.md`/`.markdown`/`.mdown` classification, major-mode activation, command metadata, key routing, and inert text transform declarations.

Phase 18 still owns Markdown-specific rendering and parse primitives such as `DecorationRange`, `IncrementalParseUpdate`, `FoldingRange`, Markdown SDUI/status extensions, parser execution, decoration publication, and any syntax-aware background work. Those APIs must stay behind Clay JS facade modules and typed Rust validators; raw ops, client package JavaScript, filesystem, network, shell, AI mutation, and package-manager authority remain prohibited unless a future approved decision log grants an explicit exception.

## Deterministic Verification

Run the focused implementation and documentation checks after changing this gate:

```text
cargo test --test package_primitive_gate
cargo test --test primitives_docs
```

`tests/primitives_docs.rs` verifies that this page is linked from `docs/reference/primitives/index.md` and `docs/index.md`, covers the scope/security/handoff contract, references the canonical security document, and keeps package validation and mode activation out of typing-hot-path documentation. `tests/package_primitive_gate.rs` verifies manifest validation, permission validation, mode classification/activation, command registration, ambiguous key binding rejection, and inert text transform validation.
