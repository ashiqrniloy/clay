---
date: 2026-05-08 18:40
status: approved
decision_about: "Clay JS API discoverability, key bindings, and custom properties"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Clay JS APIs include user-facing names, key bindings, and custom properties

## Decision

Every Clay JS API must include a searchable user-facing name and documented custom properties for every behavior-changing option. Every Clay JS API must also include key binding metadata; APIs with no default key binding use an empty key binding list so users and agents can still discover that no default exists and can map one later.

## Context

Phase 3 defines Clay's self-documenting program contract. The first schema covered stable IDs, JS exports, backing Rust/op paths, options, security notes, and lookup tags, but it did not explicitly model user-facing names, key bindings, or behavior-customizing properties. The user clarified that users and AI agents should discover APIs by search, understand default key bindings when they exist, and see which properties can customize each API's behavior.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: The user said the schema should add a key binding field, that each JS API must have a user-facing name, that each JS API should have key binding metadata, and that each Clay JS API must document custom properties.

## Alternatives Considered

1. **Keep key bindings outside API documentation** — Rejected because users and agents would need a separate search path to know how an API can currently be invoked.
2. **Document key bindings only when defaults exist** — Rejected because the absence of a default binding is also useful information and users may map one.
3. **Treat behavior-changing options as prose only** — Rejected because configurable properties must be discoverable and validated through the same Markdown-derived registry as the rest of the API contract.
4. **Make the JS export name the only user-facing name** — Rejected because command/search/help surfaces need readable names while still preserving stable internal IDs.

## Rationale and Evidence

- `docs/reference/clay-js-api/schema.md` is the authoritative Phase 3 schema for generated documentation registry entries.
- `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md` requires Markdown to be the source of truth for app/agent lookup.
- `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md` defines Clay JS APIs as the public programmatic surface, so user-visible command metadata belongs on those API docs rather than raw Rust functions or raw ops.
- The user's examples, such as a cursor style API with `color`, `blinking`, and `type`, show that behavior customization must be explicit API metadata.

## References

- `docs/reference/clay-js-api/schema.md` — schema updated with `user_facing_name`, `key_bindings`, and `custom_properties`.
- `docs/index.md` — master Markdown index for registry generation.
- `.agents/skills/project-patterns/references/clay-js-api-schema.md` — reusable project pattern added from this decision.
- `plans/004-Phase3-SelfDocumentingProgramContract.md` — Phase 3 plan updated for schema and coverage expectations.

## Consequences

- Registry generation and validation must include user-facing names, key binding metadata, and custom property metadata.
- Plans that create or update Clay JS APIs must review necessary APIs with extensibility, configuration, and customization in mind.
- Missing or malformed API discovery/customization metadata should fail documentation coverage tests.
- Future command palette, help, key binding, configuration, and AI-agent discovery surfaces can use the same Markdown-derived registry.
