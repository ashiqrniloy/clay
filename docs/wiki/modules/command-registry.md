# Command Registry

## Source

- `src/packages/commands.rs`
- `tests/package_primitive_gate.rs`

## Overview

The command registry is the Phase 16.5 server-side primitive gate for package-owned command metadata and behavior-manifest contributions. It validates package-prefixed command declarations, load/activation-time key routing data, and inert text-transform metadata before any future package command execution path exists. Phase 18.4 package input declarations and component-scoped action metadata reuse this registry boundary: input/action records may reference only already-registered package command IDs, and declaring input metadata does not create command execution authority.

## Responsibilities

- Register package-owned command declarations with package name, version, prefix, routing policy, user-facing label, custom properties, key binding metadata, permissions, and provenance.
- Validate behavior-manifest contributions by composing package declarations into the existing inert `BehaviorManifest` schema and reusing `validate_manifest` for duplicate command and ambiguous key binding checks.
- Reject command registration without `command-registration`, invalid or reserved command IDs, undeclared command permissions, client-first package command authority, executable text transform fields, duplicate command IDs, and ambiguous key bindings.
- Provide the registered-command source of truth used by Phase 18.4 `PackageInputContribution` and layout/action defaults so component-scoped actions remain inert command intents rather than package callbacks.

It does not execute package JavaScript, install command handlers, grant filesystem/workspace/AI/shell/network authority, or add any synchronous package work to the keypress hot path.

## How It Works

`CommandRegistry::register_command` accepts a validated `ClayPackageManifest` and a `PackageCommandDeclaration`. Registration verifies that the package declared `command-registration`, that declaration provenance matches the manifest, and that the command ID uses the package `apiPrefix` or `apiPrefix.*` namespace. Package commands cannot declare `ClientFirstPredictable` or `ClientFirstRequiresAck`, because those routing policies imply built-in Rust client edit authority rather than package handler authority.

`CommandRegistry::validate_behavior_contribution` accepts `PackageBehaviorContribution` metadata for mode/package loading. It validates provenance and text transforms first, then builds a candidate `BehaviorManifest` by combining the default text manifest, package command declarations, contributed keymaps, editor rules, and already registered commands. The candidate goes through `src/behavior/manifest.rs::validate_manifest`, so existing manifest invariants continue to reject duplicate command IDs, unknown command targets, ambiguous key bindings, invalid editor rules, and authority/routing mismatches.

`PackageTextTransformDeclaration` is intentionally metadata-only. Its `kind` identifies a Rust-known transform category, while `javascript_callback` and `code` are forbidden fields used by the gate to reject executable payload shapes in fixtures before Phase 17 package loading expands the source of these declarations.

## Code Examples

```rust
let manifest = validate_manifest_value(&package_json)?;
let mut registry = CommandRegistry::new();
registry.register_command(&manifest, PackageCommandDeclaration {
    package_name: "@clay/markdown".into(),
    package_version: "0.1.0".into(),
    api_prefix: "markdown".into(),
    command_id: "markdown.togglePreview".into(),
    display_name: "Toggle Markdown Preview".into(),
    routing_policy: RoutingPolicy::ServerFirst,
    key_bindings: vec![],
    custom_properties: BTreeMap::new(),
    permissions: vec![],
})?;
```

## Invariants and Constraints

- Command IDs are package-owned and unique among enabled package commands.
- Command registration does not grant execution authority; command-specific permissions must already be present in the package manifest and are only metadata for future execution checks.
- Behavior contributions are load/activation-time validation work and only return inert manifest data for the client.
- Client-first local paint behavior remains Rust-known manifest behavior; package commands cannot become arbitrary client-first handlers.

## Tests

- `tests/package_primitive_gate.rs`: validates duplicate command rejection, package-aware key binding ambiguity rejection, successful inert behavior contribution validation, executable text-transform rejection, provenance, permissions, and budget references.

Run focused coverage with:

```text
cargo test --test package_primitive_gate
```

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Mode Registry](mode-registry.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- `docs/reference/primitives/registry.md#CommandDeclaration`
- `docs/reference/primitives/registry.md#KeyRoutingOverride`
- `docs/reference/primitives/registry.md#TextTransform`
