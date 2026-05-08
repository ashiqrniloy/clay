# Configuration System Pattern

- Clay user configuration is loaded from `~/.config/clay/init.js`.
- `init.js` may load other local configuration files so users can keep configuration modular.
- Each configuration option is a Clay JS API, not a separate undocumented configuration key system.
- Configuration APIs must follow the Clay JS API schema: stable ID, user-facing name, key binding metadata, custom properties, permissions/security notes, Markdown docs, master-index link, generated registry entry, and lookup access.
- Plans that add configurable behavior must include a configuration task: review the phase implementation, propose necessary configuration APIs for extensibility/customization/key binding, implement or document them, update `docs/reference/clay-js-api/**`, update `docs/index.md`, regenerate registry artifacts, and add coverage tests.
- Configuration must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority. Permission-bearing configuration APIs need explicit documented permissions and server-side validation.
- Decision log source: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
