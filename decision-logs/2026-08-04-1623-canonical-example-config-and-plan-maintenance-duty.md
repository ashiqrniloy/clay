---
date: 2026-08-04 16:23
status: approved
decision_about: "Canonical example configuration (examples/init.js) with per-plan maintenance duty"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Canonical `examples/init.js` maintained by every configuration-changing plan

## Decision

Clay keeps a canonical example configuration at `examples/init.js` that demonstrates every user-facing configuration surface with all documented options annotated. Every plan document that introduces or materially changes a user-facing configuration surface must include a dedicated task to update this example.

## Context

Users configuring Clay through `~/.config/clay/init.js` previously had to assemble setup fragments scattered across `docs/reference/clay-js-api/configuration.md`, phase-specific docs, and smoke fixtures. The user requested one comprehensive, easy-to-follow canonical example, plus a process guarantee that it never drifts as new configuration surfaces ship.

## Approval

- Proposed by: user
- Approved by user: Yes — direct instruction: "Create a folder called examples and inside that create an init.js file… This should be the canonical example config that users can easily follow and it must be comprehensive. Along side this, update the create-plan skill to make sure that at each plan document there is a dedicated task to update this example."

## Alternatives Considered

1. **Docs-only fragments (status quo)** — rejected: no single copy-paste artifact; fragments drift independently.
2. **Generated example** (tool emits init.js from api-inventory.toml) — rejected: options documentation, ordering constraints (LSP grants before `loadPackage` sealing), and commented variants need prose judgment; generation adds machinery for a file that changes only when configuration changes.
3. **Fold the duty into the existing Clay Configuration task** — rejected: the user asked for a dedicated task so example maintenance is visible and cannot be silently absorbed.

## Rationale and Evidence

- `examples/init.js` was written against the actual validated surface: `runtime/js/*.js` facades and `.d.ts` option shapes (setTypography/ligatures, clientSetCursorStyle enums, bindKey command-ID catalog, authorizeLanguageServer ordering, setSyntaxEnginePreference tiers), matching `docs/reference/clay-js-api/configuration.md` documented setup blocks. Validated with `node --check`.
- The mandatory task lives in `.agents/skills/create-plan/references/clay.md` (new "Example Configuration Maintenance Task" section) — the file SKILL.md already routes Clay plans through, so no SKILL.md change was needed.
- The task requires cross-checking against API docs and `api-inventory.toml` custom properties so option names/enums/defaults match server-side validators, and keeps the active part of the example safe to copy verbatim (`node --check` gate).

## References

- `examples/init.js` — the canonical example (10 sections: modular config, LSP grants, packages, theme/appearance, typography/ligatures, caret, keybindings + command-ID catalog, syntax engine preference, editor-control programmatic control, planned-API placeholders).
- `.agents/skills/create-plan/references/clay.md` — "Example Configuration Maintenance Task" section.
- `docs/reference/clay-js-api/configuration.md` — documented init.js surface the example mirrors.

## Consequences

- Positive: one copy-paste starting point for users; drift is plan-gated like docs and wiki.
- Risk: the example can still lag if a plan skips the task — mitigated by making it a named mandatory task next to the Clay Configuration task.
- Revisit if configuration surfaces grow enough to split the example into modules (loadConfigurationModule already supports that split).
