# Package Principal and Result Routing Primitive Review

## Status

Review completed on 2026-07-20 and finalized on 2026-07-21. The approved architecture uses two fixed runtime trust domains, package-scoped provenance, and explicit user-approved composition. See `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md` and Plan 061.

## Source

- `src/packages/{service,authorization,record}.rs`
- `src/server/ops/{mod,packages,decorations,diagnostics,parse,syntax,document_analysis,completion,language_intelligence,language_server}.rs`
- `src/server/{js_runtime,connection,parse_coordinator,completion,document_analysis,language_intelligence,language_server}.rs`
- `src/protocol/{mod,completion,language_intelligence}.rs`
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
- `decision-logs/2026-07-14-2023-language-server-package-authority.md`
- `docs/reference/primitives/{registry,package-security,package-loading}.md`
- `tests/primitives_docs.rs`

## Overview

Current package records, grants, package generations, handler generations, request IDs, cancellation, and language-server grants are useful enforcement primitives. They validate what a named package may do, but most package-facing ops still trust JavaScript to name that package by supplying a manifest or package string. All loaded packages currently share one `JsRuntime`, one broad `ClayOpState`, raw `Deno.core.ops`, module caches, and `globalThis` handler registries.

Approved remediation deliberately does not isolate every package. Clay and integrity-verified bundled packages share a trusted runtime; adopted third-party packages share a second runtime. Distinct op/module allowlists create the hard first/third-party boundary. Host-stamped package provenance plus exact approved graph edges governs supported mutations within the third-party cohort, which is explicitly not a hostile sibling sandbox.

Current result producers validate versions and package provenance well, but parse, parse diagnostics, document-analysis outputs, and completion results use shared receivers. Every connection competes to drain those receivers. Language intelligence already demonstrates the correct request-specific primitive: a `oneshot::Receiver` returned by `schedule`.

## Existing Primitive and Gap Matrix

| Area | Existing primitive and owner | What it already enforces | Generic gap |
| --- | --- | --- | --- |
| Installed/enabled packages | `PackageService::{installed,enabled}` and `PackageRecord` | Validated manifest, provenance, prefix, declared permissions, contributions, conflicts. | Enabled record has no authenticated executing principal. |
| User authorization | `PackageAuthorizationRecord` | Exact source/name/version/prefix grants and runtime profile. | Ops can select another package's authorization by self-asserting its manifest/name. |
| Language-server grants | `LanguageServerGrant` | Contribution fingerprint, canonical executable, roots, package provenance, pre-load seal. | Session start/send/read still accept caller-supplied package identity before exact grant lookup. |
| Package generations | `PackageService::package_generation`, `PackageRevocationRecord`, runtime/provider generations | Monotonic audit, cancellation, stale-output rejection, reload withdrawal. | No per-enabled-package generation is bound to an executing runtime principal. |
| Package control | `enable_graph`, `ensure_package_control_grant`, disable/replace conflict records | Requires `package-control` on the named controller and records withdrawal. | Named controller is not authenticated when package-facing control APIs arrive. |
| Module loading | `PackageLoadEntryAllowlist` + `ClayModuleLoader` | Canonical package-root confinement and package-owned allowlist revocation. | One runtime loads all package modules; loader provenance is available during resolve/load, not ordinary op calls. |
| Runtime state | One `JsRuntime` and one `ClayOpState` per runtime generation | Timeout, heap limit, configuration seal, registries, bounded runtime worker command serialization. | `ClayOpState` has runtime/document/config context but no package principal; package globals and raw ops are shared. |
| Publication/registration ops | Typed validators for modes, commands, UI, parse, syntax, decorations, diagnostics, completion, intelligence, analyzers | Schema, prefix, permission, payload, version, and provenance validation after constructing a `PackageRecord`. | Construction commonly starts from caller-supplied `packageManifest`; validation proves record shape, not caller identity. |
| Parse coordinator | Handler key, runtime generation, document version, stable window, cancellation | Stale/generation rejection and package-stamped inert outputs. | Global update/diagnostic receivers have no authorized connection owner. |
| Completion coordinator | Client/document/provider task key, generation, stale checks | Cancellable request work and result validation. | One unbounded global result receiver is drained by request tasks from all connections. |
| Document analysis | Package/contribution/root/generation worker key, bounded mailbox/output queue | Exact worker authority, version checks, request `oneshot`s for completion/intelligence. | Streaming decoration/diagnostic output has one shared receiver and no client subscription owner. |
| Language intelligence | Client/document/feature task key and per-request `oneshot` | Request ownership, cancellation, limits, stale checks, registered provenance. | Keep this pattern; only connection/document access checks remain outside this review. |
| IPC connection | Hello-owned server `client_id`, document access/leases, runtime broadcast subscriptions | Completion/intelligence requests are restamped with connection identity; runtime snapshots use broadcast receivers. | Parse/analysis consumers are not subscriptions; completion uses an ad hoc per-connection unbounded relay after racing on the global result receiver. |
| Request IDs | Completion/intelligence request ID plus client/document/version/generation | Client can reject stale UI results and coordinator can correlate work. | IDs are correlation data, not authority; routing must not trust a guessed ID. |
| Language-server router | Exact session identity, bounded central command channel/table, session cap, oneshot replies | Fixed launch metadata, direct spawn, bounded I/O, cleanup, typed errors. | Awaited read/write in the central router causes cross-session head-of-line blocking; actor ownership may change scheduling only. |

## What Existing Primitives Can Enforce

Without adding new concepts, Clay can continue to use:

1. `PackageService` as the sole source for current records, grants, graph authority, and revocation.
2. Existing runtime/provider/package generations for stale-output cancellation.
3. Existing package-prefixed contribution validators after caller identity is derived by the host.
4. Existing `oneshot` request replies for completion and language intelligence.
5. Existing document access/lease state to authorize subscriptions before output fan-out.
6. Existing language-server grant/session identity unchanged while session I/O ownership is refactored.

These primitives cannot authenticate sibling JavaScript in the current shared runtime. A generic caller principal is the only package-authority gap. A bounded subscription dispatcher plus request-scoped replies is the only result-routing gap. No per-op manifest parser, duplicate coordinator, generic actor framework, or new subprocess capability is justified.

## Threat Model

### Trust boundaries

- **Adversarial adopted package versus trusted domain:** third-party code may call any op installed in its runtime, pass arbitrary values, retain stale state, and attempt to reach trusted modules/internal ops. Distinct runtime extension/module allowlists must deny this.
- **Shared third-party cohort:** adopted packages may intentionally build on or mutate one another. Host APIs retain provenance and enforce approved graph relations, but Clay does not claim hostile JavaScript-memory isolation among them.
- **Adversarial same-user IPC client:** after Hello, a client may forge IDs, guess documents/requests, subscribe without access, disconnect, and race another client.
- **Approved language-server child:** trusted same-user subprocess authority under its exact grant. It is not an OS sandbox and receives no package principal or Clay op access.
- **Trusted host:** Rust package service, runtime generation store, coordinators, workspace access state, and dispatcher.

### Required adversarial outcomes

| Attempt | Required result |
| --- | --- |
| Package A submits package B's current or old manifest/name/prefix. | Reject before registration/publication/session access; B's metadata is never selected from caller input. |
| Package A adds a permission to its submitted manifest. | Ignore submitted authority fields; resolve grants from A's exact current enabled record. |
| Package A retains an old runtime/principal across disable, update, or reload. | Principal resolution fails and all owned handlers/outputs/sessions are cancelled or withdrawn. |
| Package A disables/replaces B. | Permit only when authenticated A's current record has `package-control`; target B remains explicit data. |
| Package A starts or accesses B's language-server session. | Reject; derive caller A, then require A's exact fixed-contribution grant/session identity. |
| Third-party package imports trusted implementation modules, accesses trusted globals, or calls Clay-internal ops. | Impossible across runtime domains; only public typed APIs are installed/routable. |
| Third-party package A changes first-party B. | Permit only through a B-declared extension point plus exact user approval; preserve A-as-requester/B-as-target provenance. |
| Third-party package A changes third-party B's shared JavaScript state. | Outside hostile sibling isolation; disclose shared trust while host API mutations remain graph-authorized and auditable. |
| Client A guesses client B's request/document ID. | No subscription/reply route exists for A; reject access without leaking payload/metadata. |
| Two clients produce parse/completion/analysis output concurrently. | Each request reply or authorized subscription receives its own output exactly once. |
| One LSP session hangs. | Session B remains responsive, with grant identity and revocation semantics unchanged. |

## Package Principal Options

### 1. Exact enabled-record lookup from caller-supplied identity

This reuses `PackageService` and prevents permission inflation, but package A can name B and receive B's exact valid record. **Rejected as authentication.** Keep exact lookup only after a host-authenticated principal selects the package.

### 2. Secret string, numeric resource ID, or bearer token in JavaScript

A token passed through options, globals, module exports, or enumerable `Deno.core.ops` can be copied, guessed, leaked, or replayed. Resource IDs are handles, not caller identity. **Rejected.** No principal value is exposed to JavaScript.

### 3. Host-created package-scoped facade/capability in one runtime

This can stamp supported host API calls with package provenance, but one runtime cannot protect trusted globals/modules/ops from adopted code. **Selected only inside the shared third-party cohort for attribution, not as the first/third-party security boundary.** Raw caller manifests never grant authority.

### 4. Two fixed `JsRuntime`/V8 trust domains

One trusted runtime executes Clay and exact integrity-verified bundled packages. One separate runtime executes all adopted third-party packages. They install distinct op extensions, module allowlists, state, heaps, and generations; communication crosses only through typed bounded inert Rust APIs. **Selected.**

This preserves fixed resource cost and third-party composition while protecting trusted JavaScript and Rust op surfaces. It is same-process isolation, not native crash or OS containment.

### 5. Principal-specific isolate per enabled package

This gives hostile sibling isolation but duplicates V8 heap/module/lifecycle resources and prevents direct package composition without host mediation. **Rejected by the user.** Revisit only if hostile isolation among third-party packages becomes a product requirement.

### 6. OS process per package

A process gives a stronger crash/resource boundary but adds IPC, serialization, lifecycle, and platform cost. A same-user process is still not filesystem confinement without separate OS enforcement. **Rejected.**

## Approved Package Provenance and Trust-Domain Contract

1. Use exactly two persistent `deno_core::JsRuntime`/V8 trust domains: trusted Clay/bundled packages and one shared adopted-third-party runtime.
2. Classify trusted packages by compiled bundled inventory and exact provenance/integrity, never name/prefix or normal user promotion.
3. Give the third-party runtime only public package ops and narrow host state. Clay-internal ops and trusted module roots are absent.
4. Host-stamp every supported package registration/publication/provider/process operation with current package provenance/generation; caller manifest/name fields never select grants.
5. Cross-domain mutation requires a first-party-declared extension point plus exact user approval. Preserve requester and target provenance; never impersonate the target.
6. User-approved third-party replacement atomically withdraws a package-managed first-party package while replacement code remains in the third-party runtime. Clay core/bootstrap is not package-replaceable.
7. Third-party packages share one disclosed trust cohort and may compose directly; host APIs still enforce approved relations, scopes, conflicts, and revocation.
8. Disable, replacement, update, failed candidate, runtime-generation replacement, and shutdown revoke generations before cancelling handlers, outputs, workers, modules, and sessions.
9. Existing fixed-contribution language-server authority remains unchanged: explicit grant, descriptor fingerprint, canonical executable/argv/environment/root, direct no-shell spawn, bounded I/O, and trusted-subprocess disclosure.

## Result Routing Decision (No New Authority)

Result routing uses existing primitives:

- Completion and language-intelligence requests return request-scoped `oneshot` receivers. Completion deletes its global result receiver.
- Parse, parse diagnostics, and document-analysis streaming outputs enter one server-owned dispatcher carrying document, package, layer/source, version, and generation metadata.
- Connections register bounded subscriptions only after workspace/document access validation. Dispatcher fans out to matching subscribers; connections never compete to drain coordinator receivers.
- Disconnect, close, revocation, and generation replacement remove subscriptions. Saturation follows the later bounded-store task: latest replaceable state coalesces; request replies fail predictably; no unbounded relay is added.
- Request IDs correlate UI work but never grant access.

## External Process Actor Constraint

Refactoring each language-server session into an independent actor may change only command ownership and scheduling:

- Central service still owns the bounded session table, duplicate contribution/root check, session allocation, grant/revocation index, and global cap.
- Session actor alone owns child/stdin/stdout/stderr and a bounded mailbox.
- Every actor command carries host-derived package principal identity plus contribution fingerprint and is revalidated against the current grant before ingress/operation.
- Spawn descriptor, environment clearing, cwd/root checks, timeout/message/stderr limits, cleanup triggers, and trusted same-user subprocess disclosure remain exactly as approved.
- No generic process API, actor framework, shell authority, runtime-selected command, or per-language Rust branch is introduced.

## Migration Order

1. Execute Plan 061: establish bundled trust inventory, split runtime extensions/state/loaders, and benchmark fixed two-runtime cost.
2. Bind package-scoped provenance in the shared third-party runtime and remove caller-selected manifest/name authority.
3. Add target-declared extension points, durable user approvals, and atomic first-party disable/replacement while preserving provenance.
4. Route typed bounded cross-domain requests and migrate bundled packages to explicit public extension points.
5. Bind parse/completion/intelligence/analyzer registrations and language-server sessions to current domain/package generations.
6. Convert completion to `oneshot`; add one bounded streaming dispatcher and authorized subscriptions for parse/diagnostic/analysis output.
7. Wire disable/reload/update/shutdown revocation, run cross-domain/shared-cohort/two-client tests, then remove compatibility paths.

## Performance and Hot-Path Policy

Two runtime creation, package validation/adoption, graph resolution, and subscription mutation are load/reload/close work. Package provenance resolution is bounded metadata lookup plus exact record/grant/edge comparison at op ingress. Cross-domain messages and output dispatch use fixed payload/channel ceilings. None enters client typing, local edit application, paint, layout, scroll, pointer, or text-event paths.

Measure one-runtime versus two-runtime startup, resident memory, heap use, reload, and failure recovery. Keep exactly two domain workers/runtimes unless measurement proves a simpler shared worker sufficient; do not add per-package workers or user tuning knobs.

## Invariants and Constraints

- Caller provenance comes from host-owned package activation/capability state, never caller manifest JSON.
- Generation binding invalidates stale package execution and outputs.
- First-party mutation requires target extension-point consent, user approval, authenticated requester provenance, and required package-control/import authority.
- Exact enabled-record lookup is mandatory but insufficient without host-derived provenance.
- String bearer tokens and JavaScript-visible authority handles are forbidden.
- Trusted and third-party runtime op/module allowlists are distinct; third-party internal-op denial is structural.
- Third-party packages are a disclosed shared trust cohort, not sibling-isolated.
- One generic dispatcher owns streaming result routing; coordinators do not grow per-connection implementations.
- Language-server subprocess authority remains the approved fixed-contribution contract.

## Tests

- `tests/primitives_docs.rs`: review index and authority/routing decision-gate coverage.
- Later implementation tests: two-package impersonation/permission/control/revocation/session attempts and two-client parse/completion/analysis isolation.
- Documentation gate:

```bash
cargo test --test primitives_docs package_principal_and_result_routing_primitive_review
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Unified Package Runtime Authority](third-party-runtime-authority.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Parse Coordinator](parse-coordinator.md)
- [Language Intelligence](language-intelligence.md)
- [Language Server Process Service](language-server-process-service.md)
