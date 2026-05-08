---
date: 2026-05-08 15:09
status: approved
decision_about: "Clay JavaScript API facade for Rust public functions"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Expose Rust capabilities to JavaScript through Clay JS APIs backed by deno_core ops

## Decision

Clay will not expose arbitrary Rust public functions directly to JavaScript. JavaScript access must go through explicitly registered `deno_core` ops and stable Clay-provided JavaScript/TypeScript API facade modules. Each server-side Rust public function must have a corresponding Clay JS API, Markdown documentation, and generated documentation-registry entry. Server-side functions that should not be exposed to JavaScript should not be Rust-public; keep them private or `pub(crate)` instead.

Client-side Rust functions are not directly exposed to JavaScript. If JavaScript needs client-visible effects, it must use server-side Clay JS APIs that validate authority and produce server-authoritative mutations, protocol updates, or behavior manifests.

## Context

Clay will run JavaScript in `deno_core` on the server side. Users and AI agents will predominantly use programmatic Clay capabilities through JavaScript, so public function documentation must describe the JavaScript-facing API: what it does, when to use it, how to call it, code examples, and configuration options. This creates a boundary decision for how Rust public functions become JavaScript-accessible.

## Approval

- Proposed by: Agent, then expanded by user.
- Approved by user: Yes.
- Approval evidence: User said, "Lock this decision as approved just as you proposed" and requested plan, pattern, and test updates based on this design.

## Alternatives Considered

1. **Expose arbitrary Rust public functions directly to JavaScript** — Rejected because it couples JS users to Rust internals, risks accidental authority exposure, and makes stability/documentation difficult.
2. **Require JavaScript to call raw `Deno.core.ops.op_*` directly** — Rejected because raw ops are an implementation boundary, not a stable user/agent API.
3. **Use stable Clay JS/TS facade modules backed by explicit `deno_core` ops** — Chosen because it provides a stable documented API, preserves Rust implementation freedom, and creates a clear validation/permission boundary.
4. **Expose client-side Rust functions to JavaScript** — Rejected because Clay's earlier architecture keeps JavaScript server-side and client hot paths manifest-driven, avoiding synchronous keypress or UI behavior round trips through JS.

## Rationale and Evidence

`deno_core` supports registering Rust functions as JavaScript-callable operations through extensions and op registration. This is the natural FFI boundary for server-side JavaScript, but raw op names and signatures should remain implementation details. A Clay JS facade can provide stable names, ergonomic examples, validation, versioning, and documentation while delegating to Rust op wrappers.

This aligns with prior Clay decisions:

- Server owns authoritative documents, transactions, file/workspace authority, JavaScript extension execution, and AI/tool mutation authority.
- Clients execute inert behavior manifests for hot-path behavior and do not run arbitrary JavaScript.
- Documentation-as-code must make public capabilities inspectable by users and AI agents.

## References

- Context7 `/denoland/deno_core` docs lookup for `deno_core::extension!`, `op2`, and registering Rust functions as JavaScript-callable ops.
- `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md` — server-authoritative documents and server-side JavaScript boundary.
- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md` — Markdown-authoritative documentation registry.
- `.agents/skills/project-patterns/references/documentation-as-code.md` — updated reusable documentation pattern.

## Consequences

- Server-side Rust public functions require explicit Clay JS APIs; functions that should remain internal should be private or `pub(crate)` rather than public.
- `cargo test` must eventually fail when server-side public Rust functions lack corresponding Clay JS APIs, when Clay JS APIs lack Markdown docs, or when documented APIs are absent from the generated registry.
- Documentation-as-code coverage narrows to Clay JS APIs as the public programmatic surface, while server-side Rust public functions are required to have corresponding Clay JS APIs.
- Server-side functions that should remain internal implementation details should be private or `pub(crate)`, not public.
- Future Phase 4+ IPC and extension work must preserve this boundary.
