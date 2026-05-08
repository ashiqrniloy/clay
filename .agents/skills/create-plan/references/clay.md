# Clay Project Plan Requirements

Apply these requirements only when creating or updating plan documents for the Clay project.

## Clay JS API Task

Each Clay plan document must include a separate task near the end of the plan to create or verify Clay JavaScript APIs for public programmatic behavior and Rust public functions introduced or changed by the plan.

The task should require:

- Review the phase implementation and propose the Clay JS APIs needed for extensibility, configuration, customization, user search/help, key binding, AI-agent discovery, and future public programmatic use.
- Inventory all server-side Rust public functions introduced or changed by the plan.
- For each server-side Rust public function that is a public programmatic capability, expose it through an explicit `deno_core` op wrapper and stable Clay JS/TS facade API.
- Do not expose arbitrary Rust public functions directly to JavaScript.
- Do not make raw `Deno.core.ops.op_*` calls the user-facing API.
- If a server-side function should not be exposed to JavaScript, make it private or `pub(crate)` instead of public.
- Add or update Markdown documentation for every Clay JS API with: stable ID, searchable user-facing name, default key bindings or an empty key binding list, custom properties for behavior-changing settings, what it does, why/when to use it, JavaScript usage, code example, configuration/options, return/async behavior, errors, permissions/security notes, backing Rust path, op wrapper, JS facade path, and lookup tags.
- Link every Clay JS API doc from the master Markdown documentation index.
- Update the generated documentation registry using the project command when docs change.
- Ensure `cargo test` fails when a required Clay JS API, Markdown doc, master-index link, generated registry entry, key binding/custom property field, or lookup entry is missing/stale.

Recommended task title:

```markdown
- [ ] Create or verify Clay JS APIs for public programmatic surfaces
```

Place this task after implementation/verification tasks and before the final project-wiki task when both are present.

Decision sources:

- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
- `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`

## Clay Configuration Task

Each Clay plan document that adds or changes user-visible behavior, commands, key bindings, customization, extension points, server APIs, protocol capabilities, or public programmatic surfaces must include a separate configuration task.

The task should require:

- Review the phase implementation and propose configuration APIs needed for extensibility, customization, key binding, and user/agent discovery.
- Treat every configuration option as a Clay JS API, not as an undocumented configuration key.
- Use `~/.config/clay/init.js` as the user configuration entry point.
- Allow `init.js` to load other local configuration files for modular configuration when configuration loading is implemented.
- Add or update Clay JS API docs for configuration APIs, including user-facing name, key bindings, custom properties, examples, permissions/security notes, and lookup tags.
- Link configuration API docs from `docs/index.md` and update generated registry artifacts.
- Add tests or coverage gates that fail for undocumented configuration APIs or behavior-changing settings missing from `custom_properties`.
- Preserve security boundaries: configuration must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority.

Recommended task title:

```markdown
- [ ] Create or verify Clay configuration APIs
```

Place this task near the Clay JS API task and before the final project-wiki task when present.

Decision source: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
