---
date: 2026-08-28 22:34
status: approved
decision_about: "Package-defined UI design systems"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Clay supports package-defined UI design systems through typed component recipes

## Decision

Clay will separate content themes, user-owned typography, and UI design systems. A new versioned, inert UI design-system contribution will map host-owned component kinds, semantic slots, variants, and interaction states to typed visual recipes. UI design systems may choose semantic theme color roles for component slots and states, but they may not declare palettes, literal colors, or independent color values. Every normal-rendering UI color comes from the active content theme; recipe opacity and effects may alter presentation without introducing a color source. Browser/OS system colors remain the mandatory exception in forced-colors mode. Clay will validate and atomically install those recipes, project them to the React client through bounded DTOs, and apply them through host-owned CSS custom properties without allowing packages to inject raw CSS, JSX, scripts, selectors, or Tauri APIs.

Clay will preserve `theme.setTheme` and `theme.setTypography` compatibility and add a documented configuration surface for selecting one adopted UI design-system package. The default design-system package will provide restrained utilitarian Neobrutal geometry, material, state, and motion recipes, and a Glass reference package will prove that the same host components can adopt a materially different visual system without source changes or a bundled stock color theme. Both remain colorized by whichever content theme the user selects.

## Context

Clay aims to provide Emacs-level user configurability while retaining a safe, accessible, host-owned desktop UI. The current React client already has a strong value layer: typed theme tokens, CSS custom-property projection, React Aria behavior primitives, semantic typography, package component validation, and atomic runtime snapshots. However, component recipes remain fixed in CSS Modules. Hardcoded radius, border geometry, filter, transition, dimension, and layout choices prevent a design-system package from changing the application from Neobrutal styling to Glass styling while the active content theme continues supplying every color.

Existing `designTokens` can replace semantic values but cannot replace the mapping from component state to those values. Existing first-party theme packages also contribute `textStyles` but currently contribute no `designTokens`. `theme.setTheme` accepts bundled first-party themes only, so an adopted user package cannot currently become the active application design system.

The user approved the recommended direction and requested numbered implementation plans, documentation work, project-pattern updates, Clay UI catalog alignment, and retention of the complete mandatory UI skill stack for every UI redesign or implementation task.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user said, "The direction is approved," and requested conversion of the implementation path into plan documents with the full UI skill mandate retained. The user later clarified, "UI components are still using themes. Not implementing any stock theme. Rather all colors should come from created themes," establishing content themes as the sole normal-rendering color authority.

## Alternatives Considered

1. **Continue expanding value-only theme tokens** - Rejected. More values do not let packages redefine which material, border, motion, or state recipe a component uses.
2. **Allow packages to inject raw CSS into the main webview** - Rejected. This exposes unstable DOM selectors, global cascade effects, URL/resource loading, unbounded paint costs, accessibility regressions, and spoofing risks while making internal markup a public compatibility contract.
3. **Allow packages to replace host React components in the main webview** - Rejected. Same-realm package code would inherit renderer authority and conflict with the approved Tauri security boundary. Arbitrary custom UI remains limited to separately isolated surfaces with typed messaging and no direct Tauri IPC.
4. **Bundle several fixed design systems in frontend source** - Rejected as the long-term model. It proves multiple styles but does not provide user-overwritable package authority or a stable extension contract.
5. **Typed, inert component recipes compiled by the host** - Selected. It preserves host behavior, accessibility, package provenance, validation, atomic switching, and stable component contracts while making visual rules replaceable.

## Rationale and Evidence

- `src/shell/theme.rs` already validates typed semantic tokens, fallbacks, bounds, contrast pairs, and resolved theme state.
- `frontend/src/theme/adapter.ts` already projects resolved theme values into deterministic CSS custom properties.
- `frontend/src/state/theme-store.ts` already installs theme snapshots atomically in the React client.
- `frontend/src/sdui/registry.tsx` already maps inert package component kinds to host-owned React components and stable React keys.
- `src/shell/components.rs`, `src/packages/record/ui.rs`, and `src/server/ui.rs` already reject raw CSS/colors and validate component kinds, typed style variables, tree bounds, actions, and package provenance.
- Current component recipes live in CSS Modules such as `frontend/src/components/button.module.css`, `controls.module.css`, `modal.module.css`, and `text-field.module.css`; these files contain fixed visual decisions that token overrides cannot remap.
- React Aria documentation states that its components provide behavior without default styling and expose interaction state through data attributes, render props, and slots. This supports a stable separation between Clay-owned behavior and replaceable recipe-driven presentation.
- CSS custom properties participate in the cascade and provide a native mechanism for applying resolved, host-generated values without executing package code.
- GNU Emacs faces demonstrate the durable value of named semantic display attributes and inheritance. Clay extends that principle to component slots and interaction states.
- VS Code color themes demonstrate the ceiling of value-only workbench token customization. Clay needs an additional recipe layer to support complete visual-system replacement.

## References

- `src/shell/theme.rs` - Core token types, resolution, bounds, contrast, and resolved theme state.
- `src/shell/components.rs` - Package component catalog and typed style variables.
- `src/packages/record/ui.rs` - Package UI validation boundary.
- `src/server/ops/theme.rs` - Current theme selection and first-party restriction.
- `src-tauri/src/bridge/dto.rs` - Runtime snapshot projection boundary.
- `frontend/src/theme/adapter.ts` - CSS custom-property generation.
- `frontend/src/state/theme-store.ts` - Atomic frontend theme installation.
- `frontend/src/sdui/registry.tsx` - Host-owned React component registry.
- `.agents/skills/clay-ui/references/components.md` - Current component and primitive catalog.
- `.agents/skills/clay-ui/references/tokens.md` - Current typed token catalog.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` - Host-owned shell and inert package UI.
- `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md` - Separate user-owned typography.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md` - Package provenance and runtime isolation.
- `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md` - Existing theme compatibility requirement.
- `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md` - React rendering and main-webview security boundary.
- [React Aria styling](https://react-aria.adobe.com/styling) - Style-free components, states, render props, and slots.
- [MDN: Using CSS custom properties](https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Cascading_variables/Using_custom_properties) - Native custom-property behavior.
- [MDN: backdrop-filter](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/backdrop-filter) - Glass material primitive and transparency requirement.
- [GNU Emacs faces](https://www.gnu.org/software/emacs/manual/html_node/elisp/Faces.html) - Named semantic display attributes.
- [VS Code color themes](https://code.visualstudio.com/api/extension-guides/color-theme) - Value-level workbench and syntax theme model.
- Context7 `/websites/react-aria_adobe` - React Aria Components styling, slots, and interaction-state documentation reviewed on 2026-08-28.

## Consequences

- Theme, typography, and UI design-system selection remain separate responsibilities with separate invalidation behavior.
- Content themes remain the sole normal-rendering color authority for shell, editor, package UI, controls, borders, focus, selection, status, diagnostics, overlays, and solid material fallbacks. Design-system color properties are semantic references to active-theme color roles, never literals or package-owned color values; forced-colors mode may use browser/OS system colors.
- Existing theme packages and `theme.setTheme` remain compatible. Existing fixed non-color recipes become the fallback while migration proceeds; every fallback color resolves through active-theme roles.
- The new recipe schema is additive and versioned. Existing package component kinds, style variables, and token names are not renamed or removed.
- Design-system packages declare data only through `clay.contributions`; package load entries do not imperatively register recipes.
- Adopted third-party design systems remain in the third-party trust domain. Activation uses exact package provenance, current generation, validation, and revocation. It does not promote package code or grant renderer authority.
- React Aria continues to own accessible behavior and interaction semantics. Design systems may style semantic states but may not remove focus, disabled, validation, selection, modal, or forced-color affordances.
- Clay must define stable semantic component slots and bounded visual property types for materials, shadow, blur, saturation, border style, easing, and transform presets where existing token domains are insufficient.
- Expensive effects require bounded recipes and fallbacks for reduced motion, reduced transparency, forced colors, unsupported filters, and low-performance environments.
- The default restrained Neobrutal package and Glass reference package become conformance fixtures, not stock color themes. A design-system abstraction is incomplete if either requires host component source changes or declares a concrete color.
- Public package/configuration APIs, package authoring docs, component/token catalogs, code wiki pages, generated documentation registry, canonical `examples/init.js`, manual test plans, and deterministic drift tests must evolve with implementation.
- Revisit arbitrary component implementation replacement only if isolated webviews cannot satisfy a demonstrated custom-surface requirement. Do not weaken the main-webview boundary to solve styling needs.
