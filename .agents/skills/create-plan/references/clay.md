# Clay Project Plan Requirements

Apply these requirements only when creating or updating plan documents for the Clay project.

## Clay JS API Task

Each Clay plan document must include a separate task near the end of the plan to create or verify Clay JavaScript APIs for Rust public functions introduced or changed by the plan.

The task should require:

- Inventory all server-side Rust public functions introduced or changed by the plan.
- For each server-side Rust public function, expose it through an explicit `deno_core` op wrapper and stable Clay JS/TS facade API.
- Do not expose arbitrary Rust public functions directly to JavaScript.
- Do not make raw `Deno.core.ops.op_*` calls the user-facing API.
- If a server-side function should not be exposed to JavaScript, make it private or `pub(crate)` instead of public.
- Add or update Markdown documentation for every Clay JS API with: what it does, why/when to use it, JavaScript usage, code example, configuration/options, return/async behavior, errors, permissions/security notes, backing Rust path, op wrapper, JS facade path, and lookup tags.
- Link every Clay JS API doc from the master Markdown documentation index.
- Update the generated documentation registry using the project command when docs change.
- Ensure `cargo test` fails when a required Clay JS API, Markdown doc, master-index link, or generated registry entry is missing/stale.

Recommended task title:

```markdown
- [ ] Create or verify Clay JS APIs for Rust public functions
```

Place this task after implementation/verification tasks and before the final project-wiki task when both are present.

Decision source: `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
