---
date: 2026-07-14 20:23
status: approved
decision_about: "Explicit package-scoped language-server subprocess authority"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Explicit package-scoped language-server subprocess authority

## Decision

Clay will add a deny-by-default `language-server` package capability for starting a package-declared, user-approved fixed language-server contribution. Each grant binds exact package provenance, contribution metadata, resolved executable identity, fixed arguments, explicit inherited-environment names, and approved directory workspace roots; Clay exposes only bounded typed session operations and never a generic shell or process handle.

This is trusted same-user subprocess authority, not an OS sandbox. A language-server child can potentially read files outside the approved root, access the network, and start other processes using the host user's OS permissions even though Clay sets its working directory and does not grant corresponding Clay filesystem, network, or shell APIs.

## Exact Authority Contract

### Declaration and grant identity

- A package requesting process-backed intelligence declares `language-server` plus one or more inert, package-prefixed language-server contributions.
- A contribution contains a stable contribution ID, one executable program token/path, fixed bounded literal arguments, and a bounded list of environment variable names it needs inherited. It contains no shell string, interpolation, runtime-selected executable/arguments, cwd, environment values, callback, or process handle.
- During explicit authorization, Clay resolves the executable using the server host environment, canonicalizes it, and presents the package provenance, contribution ID, canonical executable path, exact arguments, inherited-environment names, and workspace roots to the user.
- A grant binds package name, requested specifier/source kind, resolved package version, API prefix, contribution ID and descriptor digest, canonical executable path, approved directory `WorkspaceRootId` values, runtime profile/generation, approver, and approval time.
- A changed package version/source, contribution descriptor/digest, canonical executable path, or workspace-root grant fails closed and requires a new approval. Replacing executable contents in place is outside Clay's integrity guarantee for this phase; revisit content hashing/signature verification if host-binary replacement becomes a demonstrated risk.
- Clay-shipped `@clay/*` packages use the same authority model. Bundled/default `NativeTrust` authorization must exclude `language-server`; `loadPackage(...)` alone never grants or launches a language server.

### Launch and environment

- The package runtime cannot pass executable, arguments, cwd, shell mode, or arbitrary environment at session start. It selects only one contribution and one root already present in its grant.
- Clay starts at most one session for each package provenance + contribution + approved directory root + runtime generation. Sessions start lazily on explicit/intelligence demand, not during manifest validation or package load.
- Clay launches the canonical executable directly with fixed arguments through `tokio::process::Command`; no shell participates.
- Clay sets `current_dir` to the selected canonical directory root, uses `env_clear`, and inherits only the environment variable names shown in and bound to the grant. There is no implicit `PATH`, `HOME`, credential, or secret inheritance. Executable resolution happens before environment clearing; packages must declare any environment names their server needs.
- stdin and stdout are piped for bounded protocol transport. stderr is separately piped, byte-capped, control-character sanitized, and surfaced only as bounded diagnostics. Other standard streams are never inherited.
- Session/message/concurrency/time limits are typed Clay budget constants. Process start, I/O, authorization, and shutdown are asynchronous server work and never block typing, paint, layout, scroll, or local text application.

### Runtime API and output authority

- Clay exposes opaque typed start/read/write/stop session operations tied to the package, grant, contribution, root, and runtime generation. It exposes no PID control, arbitrary signal API, raw `Command`, generic subprocess API, shell API, or direct filesystem/network API.
- Phase 18.20's host session is an opaque bounded byte/message conduit. LSP `Content-Length` framing, JSON-RPC, initialization/capability negotiation, document synchronization, cancellation messages, URI and position conversion, and server-specific policy remain in Phase 18.21 bridge packages.
- `language-server` does not bypass other Clay permissions. Providers still need `parse-document`; semantic decorations and diagnostics need `render-decorations`; completion needs `completion-provider`; command-backed actions need command authority; direct edits remain inert previews in Phase 18.20.
- The permission does not grant a Clay network, filesystem, shell, workspace-mutation, package-control, raw-op, native-UI, or client-runtime API. This does not constrain what the same-user child can do through the operating system.

### Lifecycle, revocation, and diagnostics

- Disable, grant revocation, package removal/update/source change, contribution change, workspace-root removal, package reload/withdrawal, runtime generation replacement, server shutdown, timeout, protocol failure, or child exit cancels related requests and terminates the session.
- Shutdown first allows a bounded bridge-owned graceful stop when available, then closes stdin and kills and waits for the child. `kill_on_drop(true)` is a final cleanup guard, not the primary lifecycle mechanism.
- Revocation withdraws providers, cancels in-flight work, removes cached package results, closes sessions, and records package/contribution/root/generation audit data. A later request cannot restart until a current matching grant exists.
- Spawn, timeout, exit, framing, payload, and stderr failures become typed, bounded, sanitized diagnostics retaining package/contribution/root provenance. Diagnostics must not echo document source, inherited environment values, or unbounded child output.

### Containment statement

`current_dir(workspace_root)` and root-bound grants constrain Clay's launch API and audit identity only. They do not stop a same-user process from opening other paths, connecting to the network, reading inherited/host-accessible state, or spawning descendants. Clay must call this capability **trusted subprocess authority**, never sandboxed, filesystem-confined, network-confined, or workspace-confined. OS containment is deferred to a separate cross-platform design; if strict confinement becomes required, implementation must stop and adopt that design rather than weakening this disclosure.

## Context

The approved language architecture keeps LSP out of Clay core and ships bridge packages that map external language-server responses onto engine-neutral Clay primitives. Starting a language server is materially broader than an inert analyzer provider: it creates a host process that commonly needs project files, toolchains, and environment metadata.

Clay already has package capability declarations, provenance-bound authorization records, fail-closed enablement, revocation generations, canonical workspace roots, and two useful internal process patterns. It does not yet have a `language-server` permission, contribution descriptor, contribution-scoped grant, package-facing process service, or OS sandbox.

The current first-party load path auto-authorizes all declared bundled-package permissions with `NativeTrust`. That behavior is acceptable for existing inert/bounded capabilities but must explicitly exclude `language-server`, or loading a first-party bridge would silently grant process authority.

## Approval

- Proposed by: agent
- Approved by user: Yes
- Approval evidence: After receiving the exact Option A contract—including fixed executable/arguments, `env_clear`, explicit environment names, root-bound lifecycle, revocation, and the disclosure that a same-user child can access other files/network/processes—the user replied, **"approved"**.

## Alternatives Considered

1. **Dedicated direct subprocess capability with exact contribution/root grant and honest same-user-process disclosure** — selected. It is the smallest boundary that supports Phase 18.21 while preserving explicit authorization, provenance, revocation, bounded I/O, and no-shell launch.
2. **Reuse broad `shell` + `filesystem` grants** — rejected. Those capabilities are less explainable, do not bind a language-server contribution or lifecycle, and would encourage a generic command API where only one fixed process contract is needed.
3. **Require OS-enforced filesystem/network/process sandboxing before launch** — not selected for Phase 18.20. It provides stronger confinement but requires a separate Linux/Windows containment architecture and would block the planned bridge phase. This remains the upgrade path when strict confinement is required.
4. **Spawn known language servers directly from Clay core** — rejected. It creates per-language Rust policy and violates the approved package-owned LSP bridge architecture.
5. **Allow package runtime to choose executable, argv, cwd, or environment dynamically** — rejected. It turns a narrow contribution grant into generic process authority and makes approval/audit identity unstable.

## Rationale and Evidence

- `src/packages/permissions.rs` has a closed 19-value permission enum and no `language-server`; the new capability can therefore fail closed through existing parsing and grant checks rather than overloading `shell`.
- `src/packages/authorization.rs::PackageAuthorizationRecord` already binds package name, requested specifier/source, resolved version, API prefix, approved capabilities, runtime profile, and approver. The language-server grant needs a narrower companion identity for contribution, executable, environment names, and roots rather than weakening the package-level record.
- `src/packages/service.rs::authorize_package` and `ensure_capability_grants` already separate install from authorization/enable and reject ungranted requested capabilities. Disable/revocation already records a package generation and withdrawal counts.
- `src/server/ops/packages.rs::ensure_first_party_record_locked` currently seeds bundled packages with all manifest permissions under `NativeTrust`; implementation must filter `language-server` from this automatic path.
- `src/server/workspace.rs::WorkspaceState` canonicalizes directory roots and assigns `WorkspaceRootId`; grants should reference those known directory roots rather than package-provided paths or single-file parent directories.
- `src/server/runtime_sandbox.rs::RuntimeSandboxSupervisor` demonstrates persistent piped stdio, bounded frames, handshake, timeout, kill/wait, and `kill_on_drop`. Despite its type name, this decision does not treat its child-process pattern as OS sandboxing.
- `src/server/git.rs::GitDiscoveryService` demonstrates direct `tokio::process::Command`, a closed argument table, canonical workspace cwd, controlled environment, capped stdout/stderr, timeout, and sanitized diagnostics.
- Context7's current Tokio process documentation confirms that `tokio::process::Command` is the asynchronous process builder, `Stdio::piped()` supports async stream handling, `kill_on_drop(true)` requests cleanup when the child handle is dropped, and cancellation can select between `Child::wait` and `Child::kill`.
- Locally resolved Tokio is 1.52.2 with the `process`, `io-util`, and `time` features. Version-exact source confirms `Command::env_clear`, `current_dir`, and `kill_on_drop`, plus `Child::start_kill`, async `kill`, and async `wait` in `tokio-1.52.2/src/process/mod.rs`.

## References

- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` — requires explicit opt-in LSP process authority and a separate decision before bridge packages.
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` — requires one source-neutral, explicit, visible, revocable capability model.
- `plans/052-Phase18.20-Language-Intelligence-Primitives-and-LSP-Authority.md` — Phase 18.20 authority and implementation sequence.
- `src/packages/permissions.rs`, `authorization.rs`, and `service.rs` — current capability, grant, enable, and revocation model.
- `src/server/ops/packages.rs` — current bundled-package auto-authorization path.
- `src/server/workspace.rs` — canonical directory-root authority and IDs.
- `src/server/runtime_sandbox.rs` and `src/server/git.rs` — existing bounded process precedents.
- `docs/wiki/modules/phase18.20-language-intelligence-primitive-review.md` — engine-neutral provider/session inventory and authority split.
- `.agents/skills/project-patterns/references/authority-boundaries.md` and `language-capability-sequencing.md` — reusable architecture guidance updated from this decision.
- [Tokio process documentation](https://docs.rs/tokio/latest/tokio/process/index.html) — asynchronous command/child lifecycle and piped I/O.
- Cargo metadata/tree output — Tokio 1.52.2 resolves locally with `process`, `io-util`, and `time` support.
- Local Tokio source: `~/.cargo/registry/src/*/tokio-1.52.2/src/process/mod.rs` — version-exact APIs used by the planned boundary.

## Consequences

- LSP bridge packages can start external language servers without adding LSP or per-language process policy to Clay core.
- Users receive an exact, provenance-aware process approval instead of implicit first-party trust or a broad shell grant.
- Implementers must add contribution-scoped grants, descriptor validation, automatic-authorization exclusion, environment allowlisting, bounded process sessions, revocation hooks, diagnostics, and tests before any bridge package launches a process.
- Some language servers may require users to approve environment names such as `HOME`, `PATH`, or toolchain-specific variables. Clay will not silently inherit them for convenience.
- Workspace-root grants improve auditability but provide no OS filesystem boundary. Users must treat an approved language server and its package bridge as trusted host code.
- Linux remains the required implementation/CI host. Windows process/environment behavior should not regress, but strict cross-platform sandboxing is a later decision.
- Revisit when strict filesystem/network/process confinement becomes a product requirement, executable content integrity must be grant-bound, multi-root servers require one process spanning several roots, or environment-name approval is too cumbersome for real first-party servers.
