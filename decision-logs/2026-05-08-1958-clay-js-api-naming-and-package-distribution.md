---
date: 2026-05-08 19:58
status: approved
decision_about: "Clay JS API naming convention and package distribution substrate"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Clay JS API naming and npm-compatible package distribution

## Decision

Clay JS APIs will use concise, behavior-oriented callable names that distinguish JS module names, callable exports, stable registry IDs, and English `user_facing_name` metadata. Clay-owned editor-core APIs must make server/client authority visible in callable names when they touch editor-core document, UI, or behavior state; pure-JS package APIs must begin with the package name or registered package prefix.

Clay package installation will be exposed through a `clay package ...` CLI and equivalent in-app package UI, but Clay will not implement its own package manager. Clay will delegate package fetching, dependency resolution, versioning, lockfiles, integrity, and registry access to an existing npm-compatible JavaScript package manager, with pnpm/npm-compatible packages as the preferred direction.

## Context

Phase 3 defines Clay's self-documenting program contract before public APIs multiply. The project needed a durable naming convention so future Clay JS API facades do not leak raw Rust paths, raw `deno_core` ops, repetitive namespace words, or implementation-focused names into the user-facing JavaScript surface.

The discussion also expanded into package distribution. Clay is intended to support installable packages later, but the user explicitly does not want Clay to implement a packaging system. Clay should provide a CLI and in-app package management surface while using another package system internally.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user said, "I approve the direction" for the naming convention, then said, "Okay. Agreed with the npm/pnpm direction. Log a decision about the API naming convention and package distribution system..."

## Alternatives Considered

1. **Verbose Clay-prefixed callable APIs such as `clay.editor.insertText` or `clayEditorInsertText`** — Rejected because the import/runtime context already establishes Clay and the editor module, creating redundant user typing and noisier examples.
2. **Raw Rust/op-derived callable names such as `opClayEditorInsertText` or implementation module names** — Rejected because raw ops and Rust modules are implementation details behind stable Clay JS facades.
3. **English UI labels as callable names** — Rejected because natural-language search/help names belong in `user_facing_name`; callable names should remain concise JavaScript identifiers.
4. **Authority-neutral editor-core names for both server and client effects** — Rejected because Clay's architecture depends on clear server/client authority boundaries and AI agents must not confuse authoritative document mutation with local UI behavior.
5. **Clay-built package manager and registry** — Rejected because package fetching, dependency resolution, semver, lockfiles, integrity, caching, and publishing are already solved by existing ecosystems and would distract from Clay's core package contract.
6. **Git-only package installation** — Useful as a possible advanced source later, but not selected as the primary model because registry discovery, semver, dependency resolution, and updates are weaker.
7. **JSR/Deno-first package distribution** — Attractive because Clay embeds `deno_core`, but not selected as the initial recommendation because `deno_core` is not the full Deno CLI/runtime and Clay would still need resolver/cache/tooling integration. JSR can remain a future or additional source.
8. **OCI artifact distribution** — Deferred because it is heavier and less natural for ordinary JavaScript package authors, despite strong artifact/signing properties.

## Rationale and Evidence

The naming convention separates four layers that serve different consumers:

- JS module names group imports, such as `clay:editor`.
- JS callable exports are concise behavior names, such as `serverInsertText`.
- Stable registry IDs remain globally namespaced for lookup and validation, such as `clay.editor.serverInsertText`.
- `user_facing_name` stores English help/search labels, such as `Insert Text`.

This satisfies the Phase 3 documentation-as-code requirement while keeping user-callable APIs ergonomic and authority-aware. Server/client prefixes are treated as authority markers for editor-core APIs, not as implementation leakage: `server*` means server-authoritative state or validation is involved, while `client*` means client-local/client-executed behavior such as transient UI state or behavior-manifest effects. Client-prefixed APIs do not imply arbitrary JavaScript execution in the Rust client.

For package distribution, npm-compatible package management gives Clay the broadest ecosystem and mature package tooling without inventing a registry. Context7 documentation for npm confirms `npm install` supports registry packages, version constraints, Git sources, local packages, aliases, lockfiles, and reproducible dependency management. Context7 documentation for pnpm confirms `pnpm add` supports package installation and version selection, and that pnpm provides strict dependency behavior and efficient isolated/global store behavior. Deno/JSR documentation confirms JSR and npm specifiers are useful in Deno-oriented ecosystems, but Clay's use of `deno_core` still requires Clay to own module loading and runtime authority boundaries.

Clay should therefore own only Clay-specific package semantics: manifest validation, package prefix/provenance, permissions, runtime versus load-time entry separation, behavior manifest contributions, documentation/registry coverage, enable/disable/remove state, and app/CLI UX. The underlying package manager should own downloads, dependency resolution, versioning, lockfiles, integrity, and registry access.

## References

- `concept.md` — Defines Clay as a server-side JavaScript programmable environment with Rust client/server authority boundaries.
- `roadmap.md` — Phase 14 describes installable packages, runtime/load-time separation, package manifests, permissions, modes, conflict handling, and documentation registry integration.
- `plans/004-Phase3-SelfDocumentingProgramContract.md` — Contains the task to define Clay JS API naming as a project pattern.
- `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md` — Establishes server/client authority boundaries and server-side JavaScript execution.
- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md` — Establishes Markdown-authoritative generated registries.
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md` — Establishes Clay JS facades over explicit `deno_core` ops.
- `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md` — Requires user-facing names, key binding metadata, and custom properties.
- Context7 `/npm/cli` docs for `npm install` package sources, aliases, version constraints, and lockfile-managed installation.
- Context7 `/websites/pnpm_io` docs for `pnpm add`, versioned package installation, strict dependency access, and isolated/global store behavior.
- Context7 `/denoland/docs` docs for JSR imports, npm specifiers, and versioned dependency imports.
- Context7 `/denoland/deno_core` docs for custom module loaders and extension/op boundaries.

## Consequences

- New Clay JS API docs and facades must distinguish JS module, callable export, registry ID, and `user_facing_name`.
- Clay-owned editor-core APIs must include server/client authority in callable names when authority could otherwise be ambiguous.
- Raw `op_*`, Rust module/function names, generated registry IDs, and repetitive namespace words must not leak into callable JS names without a documented exception.
- Pure-JS package APIs must start with a package name or registered package prefix so users and AI agents can identify provenance.
- Package manifests should include Clay-specific metadata such as package prefix, entry points, permissions, modes, docs, and runtime/load-time separation.
- Clay CLI and in-app package UI should share one package-management backend.
- Installation and execution remain separate: installing downloads/records a package; enabling/loading validates metadata and permissions; running JavaScript happens server-side; clients receive validated behavior manifests or SDUI/protocol updates.
- Future package implementation should evaluate whether to shell out to pnpm/npm, bundle/provision a package manager, or use a package-manager library, but should preserve the npm-compatible direction unless superseded by a later approved decision.
