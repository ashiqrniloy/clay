---
date: 2026-06-27 20:14
status: approved
decision_about: "Unified user-authorized package authority for Clay packages"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Unified user-authorized package authority

## Decision

Clay will use one package authority model for Clay-shipped and user-installed packages. Package source (`@clay/*`, npm, GitHub, git URL, or local path) affects default trust prompts and provenance display, but not the capabilities a package can ultimately receive after user approval.

Users and package authors retain control: third-party packages may request the same Clay-defined capabilities as first-party packages, including package control, extension/replacement of other packages, workspace mutation, filesystem/network/shell/WASM/AI/native/client/runtime capabilities when those host APIs exist and the user grants them.

## Context

Plan 035 had been drafted around a strict third-party policy: non-`@clay/*` execution stayed blocked, many authorities were categorically denied, exact trust/integrity records were required before execution, and tests enforced that no third-party package could run until another approval gate. The user reviewed that direction and rejected it as conflicting with Clay's goal of being infinitely extensible.

The code already has useful generic primitives: package-manager delegation, install/enable/load separation, manifest validation, package records, conflict detection, runtime/load entry separation, server-side JavaScript execution, and inert client delivery. The problem was the policy layered on top: it treated third-party packages as a permanently lower-trust class instead of as user-authorized packages.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: User said, "Yes create the decision log. Then overwrite the plan 035 ... Make sure whatever has been implemented so far to enforce the strictness is also changed to the agreed upon model."

## Alternatives Considered

1. **Keep Plan 035 strict deny-first policy** — rejected. It maximizes default safety but makes Clay package extensibility dependent on central approval and prevents third-party packages from being true peers.
2. **Trust npm/GitHub packages automatically** — rejected. Installation source is not user approval. Clay should still show requested capabilities and require explicit user/admin authorization before behavior-changing enablement.
3. **Separate first-party and third-party runtimes** — rejected as the target architecture. Runtime profile may vary by user choice, but Clay should not hard-code lower capability ceilings for non-`@clay/*` packages.
4. **Single broad `trusted` switch** — rejected. It hides what was granted. Capabilities should be explicit and visible, even when the user chooses to grant powerful authority.

## Rationale and Evidence

Project evidence reviewed:

- `src/packages/manager.rs`: `PackageManagerBackend` and `PnpmBackend` already delegate npm-compatible fetching/resolution/lockfile/integrity/caching; `pnpm add <spec>` can handle registry, GitHub/git, URL, and local path style specs through the package manager.
- `src/packages/service.rs`: `PackageService` already centralizes install, enable, disable, remove, list, inspect, and installed/enabled state. This is the right place for package source/provenance and user authorization state.
- `src/packages/manifest.rs`: `validate_manifest_value` already enforces Clay metadata, `apiPrefix`, entry/loadEntry shape, permissions, and modes before enable/load.
- `src/packages/conflict.rs`: `check_enabled_packages` already provides deterministic conflict diagnostics. The target model should evolve this from hard rejection to user/package-declared override/extend/replace policy.
- `src/server/ops/packages.rs` and `src/server/js_runtime.rs`: current `loadPackage` and module loading are first-party-only. That is an implementation gap to remove in the new plan, not the long-term authority model.
- `docs/reference/primitives/package-loading.md` and `docs/reference/primitives/package-security.md`: existing docs already separate install, enable/load, runtime execution, package-manager execution, and client behavior delivery. That separation remains useful.

Target model:

- **Install**: delegate to pnpm/npm-compatible tooling; allow npm, GitHub, git URL, tarball, and local path specs as user intent.
- **Enable/load**: Clay validates package metadata and requested capabilities, then asks/uses user authorization.
- **Runtime execution**: packages run through the same Clay package runtime path regardless of source.
- **Capabilities**: same grantable capability vocabulary for all packages; source does not impose a hard ceiling.
- **Package graph**: packages may depend on, import/use, extend, disable, or replace other packages when granted package-control/import capabilities.
- **Conflict resolution**: conflicts are detected with provenance, then resolved by explicit user config, package replace/extend declarations, priority, or diagnostic fallback; no silent load-order wins.
- **Client boundary**: Rust clients still receive validated protocol/manifest/UI state unless a future user-granted client-runtime/native capability intentionally expands that boundary.
- **Hot paths**: install, authorization, enable/load, reload, conflict resolution, and package graph changes stay off typing/paint/layout/scroll/text-event hot paths.

## References

- `src/packages/manager.rs` — package-manager delegation and lifecycle-script option boundary.
- `src/packages/service.rs` — shared install/enable/disable/list/inspect package service.
- `src/packages/manifest.rs` — Clay metadata and permission validation.
- `src/packages/conflict.rs` — deterministic conflict detection to evolve into override/replace policy.
- `src/server/ops/packages.rs` — current first-party-only `loadPackage` resolver to replace.
- `src/server/js_runtime.rs` — current module-loader allowlist/root confinement to generalize.
- `docs/reference/primitives/package-security.md` — package security/provenance docs to update to unified authority.
- `docs/reference/primitives/package-loading.md` — package loading boundary docs to update.
- `plans/035-Third-Party-Package-Runtime-Authority-Policy.md` — overwritten with implementation plan for this decision.

## Consequences

- Clay stops treating third-party packages as categorically less capable than Clay-shipped packages.
- Users gain responsibility for approving powerful package capabilities.
- Implementation must add clear prompts/config/API docs so users can understand and revoke granted authority.
- Current strict docs/tests/comments from Plan 035 must be removed or rewritten so they no longer require non-`@clay/*` deny-by-default behavior.
- Existing first-party-only resolver code remains a current implementation limitation until Plan 035 tasks replace it with source-aware package resolution.
- Revisit this decision if user-authorized packages cannot be made debuggable/revocable, or if capability grants become too coarse to explain safely.
