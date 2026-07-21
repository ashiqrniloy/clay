---
date: 2026-07-21 00:01
status: approved
decision_about: "Two JavaScript package-runtime trust domains and user-approved package composition"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Two package-runtime trust domains

## Decision

Clay will use exactly two persistent JavaScript package-runtime trust domains: a trusted runtime for Clay and integrity-verified bundled first-party packages, and a separate shared runtime for adopted third-party packages. Cross-domain communication occurs only through typed, bounded public Rust/Clay APIs; third-party changes to first-party behavior require both a first-party-declared extension point and explicit user approval.

Third-party packages remain a shared trust cohort rather than receiving one isolate each. Normal user approval cannot promote a third-party package into the trusted runtime. With explicit user approval, a third-party package may disable and fully replace a first-party package through the host-owned package graph while remaining in the third-party runtime and retaining its own provenance.

This decision supersedes the same-runtime-path and source-independent capability-ceiling portions of `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`. It retains that decision's user-controlled extensibility, explicit capabilities, provenance, package graph, deterministic conflicts, and revocation model.

## Context

The code review found package-facing ops that accept caller-supplied manifests or package names. Exact record lookup prevents permission inflation but does not authenticate a sibling JavaScript caller in the current shared runtime. One isolate per enabled package would create a strong sibling boundary, but the user rejected its repeated V8 heaps, module caches, lifecycle machinery, and inability to support direct package composition efficiently.

Clay is intended to let packages build on, extend, override, disable, and replace other packages when users choose. The desired boundary is therefore not isolation between every package. It is a hard trust boundary between Clay/bundled code and adopted third-party code, paired with inspectable package-scoped provenance and explicit composition records inside the third-party ecosystem.

Current project primitives already support much of this model: `PackageService`, exact authorization and language-server grant records, package provenance, generation/revocation records, `dependsOn`/`extends`/`disables`/`replaces`, package-control/import permissions, deterministic conflict diagnostics, confined package module loading, runtime timeout/heap limits, and typed contribution registries. Current gaps are runtime-domain separation, caller provenance, distinct op/module allowlists, declared first-party extension points, durable mutation approvals, and cross-domain request routing.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user agreed to shared-runtime package-scoped provenance and an explicit approved mutation graph, then approved the two-runtime refinement, including no normal promotion into the trusted runtime and disclosure that third-party packages are not isolated from each other. The user additionally required an explicitly approved option to disable and fully replace a first-party package with a third-party package.

## Alternatives Considered

1. **One shared runtime for all packages** — lowest resource cost and maximal direct composition, but third-party code can reach the same JavaScript globals, modules, and installed ops as Clay/bundled code; it cannot provide the requested first-party security boundary.
2. **One isolate/runtime per package** — strongest JavaScript sibling isolation and simplest runtime-derived principal, but repeats V8 resources per package and prevents direct package composition without pervasive host mediation. Rejected by the user.
3. **JavaScript-visible bearer token or resource ID in one runtime** — small implementation but copyable/replayable and does not isolate globals or internal ops. Rejected.
4. **OS process per package** — stronger crash/resource boundary but adds IPC, serialization, lifecycle, deployment, and platform complexity; a same-user process is still not an OS sandbox without additional enforcement. Rejected for this architecture.
5. **User-selectable shared versus isolated package modes** — appears flexible but creates two package execution contracts, compatibility paths, test matrices, and inconsistent mutation semantics. Rejected as unnecessary complexity.
6. **Two fixed runtime trust domains** — fixed resource overhead, strong first/third-party JavaScript separation, and shared third-party composition. Selected.

## Rationale and Evidence

- `src/server/js_runtime.rs` currently creates one `JsRuntime` with one `init_runtime_extension()` and one broad `ClayOpState`. A two-domain design can reuse the existing worker/runtime lifecycle while installing different extension sets, module loaders, state, timeouts, and heap budgets.
- `src/server/ops/mod.rs` currently registers configuration, package administration, workspace, Git, language-server, mode, command, publication, and provider ops in one extension. Since installed `deno_core` ops are directly reachable through `Deno.core.ops`, privileged ops must be absent—not merely hidden by facades—from the third-party runtime.
- `src/packages/manifest.rs`, `graph.rs`, `service.rs`, and `conflict.rs` already parse and enforce package graph relations and deterministic conflict handling. These should be extended with target-declared extension points and durable user-approved mutation scope instead of replaced.
- Current first-party packages under `packages/` are declarative grammar/mode packages (`markdown`, `rust`, `typescript`, `javascript`), LSP bridge packages, Git/status UI, and inert themes. Their extension needs map mostly to generic contribution APIs plus a small number of package-owned extension points.
- Separate V8 runtimes prevent direct cross-domain JavaScript object/function/global/module sharing. Typed bounded Rust mediation preserves server authority and gives each crossing schema, provenance, generation, payload, timeout, and revocation checks.
- Two runtimes remain same-process isolation. They do not contain native/V8 process crashes or same-user language-server subprocess authority. Existing truthful trusted-subprocess disclosures remain mandatory.

The boundary uses these rules:

1. Trusted classification comes from Clay's compiled/bundled inventory and exact provenance/integrity, never an `@clay/*` name or user-granted promotion.
2. Trusted and third-party runtimes install separate `deno_core` extension/op sets and separate module-loader allowlists.
3. Third-party runtime state contains only narrow public host capabilities. Clay-internal Rust functions remain private/`pub(crate)` and have no third-party op.
4. No V8 value crosses domains. Communication uses typed, bounded inert request/result values through Rust-owned state or bounded queues.
5. Every supported cross-package mutation retains requester and target provenance. A package extends or replaces another as itself; it never impersonates the target.
6. First-party mutation requires owner consent through a declared extension point and user consent through an exact approval record. A first-party package may be fully disabled/replaced with user approval, but replacement code remains in the third-party runtime.
7. Third-party packages share a runtime and are not hostile-code-isolated from each other. Adoption UI/docs must disclose this. Host APIs still enforce and audit supported mutation relationships.
8. Clay core/bootstrap and internal Rust authority are not packages and are not disabled by package replacement. Package-managed bundled first-party behavior is replaceable through the package graph.
9. External processes retain fixed-contribution grants, canonical executable/argv/environment/root identity, direct no-shell spawn, limits, revocation, and same-user trusted-subprocess disclosure.

## References

- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` — partially superseded package runtime/capability policy.
- `decision-logs/2026-07-14-2023-language-server-package-authority.md` — unchanged external-process authority.
- `src/server/js_runtime.rs` — current single persistent runtime worker and module loader.
- `src/server/ops/mod.rs` — current broad single runtime extension and `ClayOpState`.
- `src/packages/{authorization,manifest,graph,service,conflict}.rs` — existing grants, graph, provenance, conflict, and revocation primitives.
- `packages/*/package.json` and `packages/*/dist/*.js` — bundled package inventory and current Clay API dependencies.
- `docs/reference/primitives/package-security.md` — current package authority and relation contract to revise.
- `docs/wiki/modules/package-principal-and-result-routing-primitive-review.md` — package identity/routing alternatives and primitive inventory.
- Locally resolved `deno_core 0.400.0` source — `OpState` is associated with `JsRuntime`; installed ops are runtime-wide and ordinary op calls do not receive authenticated calling-module identity.

## Consequences

- Clay pays fixed overhead for two V8 runtimes instead of overhead proportional to package count.
- Third-party packages cannot access trusted JavaScript globals/modules or Clay-internal ops; public API validators become a security-critical boundary.
- Third-party packages remain mutually trusted inside their shared runtime. The product must disclose that limitation and provide restart/revocation of the whole third-party generation.
- First-party packages need explicit, versioned extension-point inventories. Cross-domain callbacks become typed provider registrations/inert payloads rather than shared JavaScript functions.
- Package adoption must occur before execution, show capabilities and graph/mutation effects, persist exact approvals, and re-prompt when authority expands or provenance becomes stale.
- A third-party replacement of a first-party package atomically withdraws the target's package-owned contributions and activates the replacement without moving replacement code into the trusted runtime.
- Existing same-authority-regardless-source docs, tests, patterns, and Plan 060's per-package-principal proposal must be revised.
- Plan 061 will implement this architecture before Plan 060 resumes authority/routing/runtime-facade work that depends on it.
- Revisit separate per-package isolation only if hostile isolation among third-party packages becomes a product requirement. Revisit OS processes only if measured same-process crash/resource containment is insufficient.
