# UI Modernization

- Before reviewing or changing any UI surface, run `npx ui-skills start`, inspect the relevant category, and load the smallest useful skill set (prefer 1, max 3); record selected slugs in the plan/review evidence.
- Modernize defaults and token consumption without replacing Clay's existing user-configurable theme model.
- Every modernized visual property uses an existing typed theme token or a justified additive typed token; never hardcode one fixed redesign.
- Preserve `theme.setTheme`, validated theme-package `designTokens`, cached `ResolvedUiTheme` hot-path reads, fallback behavior, and existing theme compatibility.
- Keep concrete typography user-owned through `theme.setTypography`; components and packages continue to select semantic roles/variants only.
- Visual acceptance must exercise dark/light themes plus representative typed overrides and contrast validation.
- Decision source: `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`.
