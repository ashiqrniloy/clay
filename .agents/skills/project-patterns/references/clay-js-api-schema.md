# Clay JS API Schema Pattern

- Every Clay JS API must have a stable ID, JS module/export, JS facade path, backing Rust function path, `deno_core` op path/name, summary, owner, phase, visibility, permissions/security notes, agent guidance, lookup tags, and app/help visibility.
- Apply `clay-js-api-naming.md`: distinguish JS module, callable/export name, stable registry ID, and English `user_facing_name`; avoid raw op/Rust names and redundant Clay/module words in callable exports.
- Every Clay JS API must have a searchable `user_facing_name` for help, command search, configuration UIs, and AI-agent discovery.
- Every Clay JS API must include `key_bindings`; use an empty list when there is no default key binding. Users may map key bindings to documented APIs through configuration.
- Every Clay JS API must include `custom_properties`; list every behavior-changing configurable property with type, default, allowed values when relevant, and description. Use an empty list only when the API has no behavior-changing properties.
- Markdown under `docs/reference/clay-js-api/` plus `docs/index.md` is authoritative; generated registries and lookup APIs derive from it.
- Coverage tests should fail for missing or malformed user-facing names, key binding metadata, custom property metadata, docs, index links, generated registry entries, or lookup access.
- Decision log sources: `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`, `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`.
