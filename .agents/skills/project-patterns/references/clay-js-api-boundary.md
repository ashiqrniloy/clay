# Clay JS API Boundary

Decision source: `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.

- The public programmatic surface is the Clay JavaScript/TypeScript API, not raw Rust public functions or raw `Deno.core.ops.op_*` calls.
- Server-side Rust public functions must be exposed through explicit `deno_core` op wrappers and stable Clay JS/TS facade modules.
- If a server-side function should remain internal, make it private or `pub(crate)` instead of public.
- Client-side Rust functions are not directly exposed to JavaScript. JavaScript effects on clients flow through server-authoritative APIs, protocol updates, or behavior manifests.
- Plans that add or change server-side Rust public functions must include a Clay JS API verification task and identify the Rust function, op wrapper, JS facade, and docs path.
