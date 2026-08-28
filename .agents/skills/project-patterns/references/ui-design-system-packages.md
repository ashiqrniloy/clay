# UI Design-System Packages

- Keep content themes, user-owned typography, and UI design systems as separate configuration and invalidation layers.
- Content themes are the sole normal-rendering color authority. UI design systems may map component slots/states to semantic theme color roles and apply typed opacity/effects, but may not declare palettes, literals, or independent color values. Browser/OS system colors are allowed only for forced-colors accessibility behavior.
- UI design systems are versioned, inert `clay.contributions` data that map host-owned component kinds, semantic slots, variants, and interaction states to typed visual recipes.
- Clay validates and atomically resolves recipes before projecting bounded DTOs to the React client. Hot render/input paths consume cached CSS custom properties only.
- Packages cannot inject raw CSS, selectors, JSX, scripts, renderer callbacks, URLs, or direct Tauri APIs into the main webview.
- React Aria and Clay own behavior, accessibility semantics, focus, and state. Recipes may style semantic states but cannot remove required affordances.
- Component kinds, slots, recipe properties, tokens, and style variables are additive and versioned. Existing theme packages and `theme.setTheme` remain compatible.
- Adopted design systems retain exact third-party provenance and runtime-domain status. Activation grants no renderer authority and must support revocation and fallback.
- Plans must include a primitive/catalog audit, bounded schema and fallback rules, package activation/configuration, all-surface migration, Neobrutal and Glass conformance fixtures, color-source deny tests, theme/design-system cross-product tests, performance budgets, visual/accessibility review, public docs, manual tests, and final wiki maintenance.
- Every UI task loads and lists `clay-ui`, its component/token catalogs, `impeccable`, `full-output-enforcement`, `high-end-visual-design`, and `design-taste-frontend` independently.
- Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.
