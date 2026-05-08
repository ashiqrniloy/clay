---
date: 2026-05-08 18:41
status: approved
decision_about: "Clay configuration through init.js and Clay JS APIs"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Clay configuration is loaded from init.js and expressed through Clay JS APIs

## Decision

Clay user configuration will be based on `~/.config/clay/init.js`. That file may load other local files for modular configuration, and each configuration option is itself a documented Clay JS API with Markdown documentation, master-index inclusion, generated registry coverage, lookup access, key binding metadata where applicable, and custom property metadata for behavior-changing settings.

## Context

While extending the Clay JS API schema, the user introduced the configuration model needed for customization and key binding. Because Clay's public programmatic surface is already the Clay JS API, configuration should not become a second undocumented API system. Instead, configuration options, key binding APIs, and behavior customization APIs should follow the same documentation-as-code contract.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: The user said configuration should be based on `~/.config/clay/init.js`, that `init.js` can load other files, that each configuration option is essentially a Clay JS API, and that these must also be documented.

## Alternatives Considered

1. **Use a non-JavaScript configuration format only** — Rejected for this decision because the user selected `init.js` as the configuration entry point and wants configuration to interact directly with Clay JS APIs.
2. **Create a separate configuration registry** — Rejected because it would duplicate the Markdown-authoritative registry and create drift between APIs, configuration, and documentation.
3. **Allow undocumented configuration keys** — Rejected because configuration is a public user/agent surface and must be discoverable, searchable, and testable.
4. **Store all configuration in one file with no modular loading** — Rejected because users should be able to organize configuration by loading other files from `init.js`.

## Rationale and Evidence

- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md` requires Markdown-indexed documentation to drive generated app/agent registries.
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md` makes Clay JS APIs the public programmatic surface.
- Treating configuration options as Clay JS APIs keeps help, command search, key binding, AI-agent discovery, and validation on one contract.
- `~/.config/clay/init.js` follows the common user-config convention of storing application configuration under the user's config directory while keeping actual behavior constrained by documented Clay APIs and server-side validation.

## References

- `docs/reference/clay-js-api/configuration.md` — initial project documentation for the configuration model.
- `docs/reference/clay-js-api/schema.md` — schema updated so configuration APIs can expose user-facing names, key bindings, and custom properties.
- `.agents/skills/project-patterns/references/configuration-system.md` — reusable project pattern added from this decision.
- `.agents/skills/create-plan/references/clay.md` — recurring plan requirements updated with a configuration task.

## Consequences

- Future configuration work must implement loading from `~/.config/clay/init.js` and support modular configuration loading from that entry point.
- Every configuration option must be represented as a Clay JS API and documented through the Markdown registry contract.
- Key binding and configuration customization must not grant filesystem, network, shell, extension, AI mutation, or workspace authority unless a specific documented permission-bearing API allows it and validates authority.
- Plans must include a configuration review/implementation task when phase work introduces user-configurable behavior.
