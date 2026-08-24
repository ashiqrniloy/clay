# UI Modernization

- Before reviewing or changing any UI surface, load `clay-ui` plus the mandatory `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend` skills; record all of them in the task's plan/review evidence.
- Modernize defaults and token consumption without replacing Clay's existing user-configurable theme model.
- Every modernized visual property uses an existing typed theme token or a justified additive typed token; never hardcode one fixed redesign.
- During the Tauri/React migration, preserve `theme.setTheme`, validated theme-package `designTokens`, fallback behavior, and existing theme compatibility while replacing native `ResolvedUiTheme` paint reads with one cached frontend projection to CSS custom properties and CodeMirror theme extensions.
- Keep concrete typography user-owned through `theme.setTypography`; React components, CodeMirror adapters, and packages continue to select semantic roles/variants only.
- Visual acceptance must exercise dark/light themes plus representative typed overrides and contrast validation.
- Decision sources: `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`, `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.
