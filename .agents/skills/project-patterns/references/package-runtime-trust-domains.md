# Package Runtime Trust Domains

- Use exactly two persistent JavaScript package-runtime trust domains: trusted Clay/bundled code and one shared adopted-third-party runtime.
- Classify trusted packages by compiled bundled inventory plus exact provenance/integrity, never package name/prefix or normal user promotion.
- Install distinct `deno_core` op extensions, module-loader allowlists, state, heap/time budgets, and generation lifecycles. Third-party runtime must not contain Clay-internal ops or full privileged op state.
- Cross domains only through typed, bounded, inert Rust-mediated APIs. Never share V8 objects, functions, globals, module instances, promises, or raw internal ops.
- Third-party packages are one disclosed shared trust cohort; host APIs still stamp package provenance and enforce current grants, graph edges, generations, and revocation.
- A third-party mutation of first-party behavior requires both a first-party-declared extension point and an exact user-approved relationship. Preserve requester and target provenance; extension/replacement never becomes impersonation.
- User-approved third-party replacement may atomically withdraw a package-managed first-party package while replacement code remains in third-party runtime. Clay core/bootstrap is not package-replaceable.
- Preserve fixed-contribution external-process authority and truthful same-user subprocess disclosure across both domains.
- Plans changing package runtime, package APIs, first-party packages, or composition must include domain-boundary, extension-point, adoption/revocation, cross-domain deny, and third-party shared-cohort tests.
- Decision source: `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md` (partially supersedes `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`).
