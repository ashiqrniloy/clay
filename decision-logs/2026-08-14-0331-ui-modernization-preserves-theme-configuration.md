---
date: 2026-08-14 03:31
status: approved
decision_about: "Theme configurability during UI modernization"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: UI modernization preserves theme configuration

## Decision

Clay's UI modernization must preserve the current user-configurable theme model. Modernized shell, editor chrome, overlays, dialogs, settings, diagnostics, and package surfaces must continue to resolve visual values through the existing `setTheme` flow, typed design tokens, and validated fallbacks rather than replacing them with fixed styling.

## Context

The 2026-08-14 implementation/UI review requires a broad visual modernization within Masonry's capabilities. Clay already supports first-party theme selection through `theme.setTheme`, typed theme-package `designTokens`, cached client-side `ResolvedUiTheme`, and separate user-owned typography configuration. A redesign that hardcodes one visual treatment would regress an existing public configuration contract.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: “During UI modernization work, keep in mind that theme should still be configurable as it is today.”

## Alternatives Considered

1. **Preserve current theme APIs and token resolution** — selected; keeps compatibility and lets modernization improve defaults without removing user choice.
2. **Ship one fixed modernized theme** — rejected; would regress `theme.setTheme`, theme-package overrides, and existing user configuration.
3. **Replace current theming with a new configuration system during redesign** — rejected; expands scope, creates migration risk, and is unnecessary because typed tokens already cover the required surfaces.

## Rationale and Evidence

- `theme.setTheme` is a documented public Clay JS API used from `~/.config/clay/init.js` to choose a bundled first-party theme.
- Theme packages can provide validated typed `designTokens`; the client resolves them into cached `ResolvedUiTheme` values before paint/layout.
- The Clay UI catalog requires token-only styling and forbids hardcoded colors, raw CSS, concrete font families, and concrete point sizes.
- Preserving the existing model lets modernization change core defaults and token consumption while keeping authority, validation, hot-path caching, and package compatibility intact.

## References

- `docs/reference/clay-js-api/theme/set-theme.md` — current public theme-selection API and security contract.
- `docs/reference/clay-js-api/theme/set-typography.md` — separate user-owned typography configuration.
- `.agents/skills/clay-ui/references/tokens.md` — typed token catalog and cached resolution policy.
- `.agents/skills/clay-ui/SKILL.md` — token-only UI rule.
- `.agents/skills/project-patterns/references/configuration-system.md` — `init.js` and documented-API configuration convention.
- `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md` — modernization scope.

## Consequences

- Every modernized visual property must map to an existing typed token or a justified additive typed token.
- Existing theme packages and `setTheme` behavior must remain compatible; defaults may improve without removing overrides.
- Modernization tests must cover at least dark/light themes and representative typed overrides, including contrast validation and fallback behavior.
- A future replacement of the theme system requires a separate approved migration decision and compatibility plan.
