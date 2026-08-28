# UI Modernization

- Before reviewing or changing any UI surface, load `clay-ui` plus the mandatory `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend` skills; record all of them in the task's plan/review evidence.
- Modernize defaults and token consumption without replacing Clay's existing user-configurable content-theme model.
- Keep content themes, user-owned typography, and UI design-system packages separate. Component recipe replacement follows `ui-design-system-packages.md`.
- Every normal-rendering color uses an active content-theme role, including shell/component surfaces, text, borders, focus, selection, diagnostics, overlays, and solid effect fallbacks. Design-system recipes may select those roles but never define literal or package-owned colors; browser/OS system colors are reserved for forced-colors mode.
- Every non-color modernized visual property uses an existing typed theme token, a typed design-system recipe property, or a justified additive typed token; never hardcode one fixed redesign.
- During the Tauri/React migration, preserve `theme.setTheme`, validated theme-package `designTokens`, fallback behavior, and existing theme compatibility while replacing native `ResolvedUiTheme` paint reads with one cached frontend projection to CSS custom properties and CodeMirror theme extensions.
- Keep concrete typography user-owned through `theme.setTypography`; React components, CodeMirror adapters, and packages continue to select semantic roles/variants only.
- Visual acceptance must exercise dark/light themes plus representative typed overrides and contrast validation.
- Decision sources: `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`, `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`, `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.
