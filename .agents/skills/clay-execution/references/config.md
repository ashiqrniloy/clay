# Configuration, Theme, Typography, and Modernization

## Configuration System

Sources: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`, `2026-07-19-0328-configuration-keymaps-survive-mode-activation.md`, `2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md`.

- Dotted-ID convention: core Clay command/API/option IDs are bare `<domain>.<name>` (`shell.*`, `editor.*`, `documents.*`, `workspace.*`, `runtime.*`, …), NEVER `clay.<domain>.*`. Package configuration and package command IDs always start with the package's own `apiPrefix` (`<package>.<name>`, e.g. `markdown.layout.defaultVisibility`); `setPackageOption` rejects `clay.`-prefixed and non-package-prefixed options. Third-party packages cannot claim a reserved core domain as prefix (`RESERVED_CORE_API_DOMAINS`, `src/packages/manifest.rs`). `clay:` module specifiers and `package.json` `clay.*` manifest key paths are the only surviving `clay` prefixes. See `js-api.md`.
- User configuration loads from `~/.clay/init.js`; `init.js` may load other local configuration files for modularity.
- Each configuration option is a Clay JS API, not a separate undocumented key system, and must follow the Clay JS API schema (stable ID, user-facing name, key binding metadata, custom properties, permissions/security notes, Markdown docs, master-index link, generated registry entry, lookup access).
- Plans adding configurable behavior include a configuration task: review the phase implementation, propose necessary configuration APIs for extensibility/customization/key binding, implement or document them, update `docs/reference/clay-js-api/**`, update `docs/index.md`, regenerate registry artifacts, add coverage tests.
- Configuration must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority. Permission-bearing configuration APIs need explicit documented permissions and server-side validation.
- Keymaps registered during configuration evaluation are durable overlays: package/mode activation applies mode bindings first, then configuration bindings — user chords survive document classification and win same-chord conflicts without carrying old mode-only bindings forward.
- Configuration-root changes use a bounded polling watcher that delegates to serialized `runtime.reloadConfiguration`; optional module failures become bounded diagnostics, required module failures preserve the previous generation. `runtime.reloadConfiguration` ships with default global `Ctrl+Shift+R`; users can unbind/override through normal keymap overlays.

## UI Design-System Packages

Source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.

- Keep content themes, user-owned typography, and UI design systems as separate configuration and invalidation layers.
- Content themes are the sole normal-rendering color authority. UI design systems may map component slots/states to semantic theme color roles and apply typed opacity/effects, but may not declare palettes, literals, or independent color values. Browser/OS system colors are allowed only for forced-colors accessibility behavior.
- UI design systems are versioned, inert `clay.contributions` data mapping host-owned component kinds, semantic slots, variants, and interaction states to typed visual recipes.
- Clay validates and atomically resolves recipes before projecting bounded DTOs to the React client; hot render/input paths consume cached CSS custom properties only.
- Packages cannot inject raw CSS, selectors, JSX, scripts, renderer callbacks, URLs, or direct Tauri APIs into the main webview. React Aria and Clay own behavior, accessibility semantics, focus, state; recipes may style semantic states but cannot remove required affordances.
- Component kinds, slots, recipe properties, tokens, and style variables are additive and versioned; existing theme packages and `theme.setTheme` remain compatible.
- Theme *appearance* changes ride the theme package's typed `clay.contributions.designTokens` overrides (core token names, `#rrggbbaa` colour roles, scalar/opacity/level variants) — never core catalogs, host CSS, or component source. The shipped language's color half is thirteen of those roles (`border.hairline`, `border.subtle`, `border.strong`, `surface.scrim`, `accent.primary`, `accent.muted`, `focus.ring`, `border.focus`, `text.muted`, `text.disabled`, `surface.hover`, `surface.active`, `surface.selected`), derived per theme from its own palette; table and rationale in [`tokens.md`](tokens.md#shipped-theme-ui-roles-plan-118-tasks-1314). Adding or changing a shipped design system is user-owned: retention is the default, and removing one requires explicit user direction, a decision log, and the same-phase cleanup of its fixtures, harness states, settings entries, tests and docs.
- Adopted design systems retain exact third-party provenance and runtime-domain status; activation grants no renderer authority and must support revocation and fallback.
- The structural-boundary gate is part of theme validation, not review: every required role pair is measured **composited** (alpha over its backdrop) and must clear prose 4.5 / boundary-accent-focus-state-fill 3.0, with a 1.2 visibility floor for the decorative `border.hairline` and a strictly monotonic hairline < subtle ladder on the same surface. A theme that misses a floor is refused activation with the pair and ratio named; the previously active theme stays installed.
- Plans include: design-language conformance to `DESIGN.md` (Quiet Instrument: hairline zoning, radius ladder, transient-only elevation, accent-as-state, mono for data, retired patterns; the former Neobrutal and Glass packages were removed by plan 118 task 9), primitive/catalog audit, bounded schema and fallback rules, package activation/configuration, all-surface migration, conformance fixtures for the shipped design system (Quiet Instrument plus the `@clay/core` baseline), color-source deny tests, theme/design-system cross-product tests, performance budgets, visual/accessibility review, public docs, manual tests, final wiki maintenance.

## Typography Role Ownership

Source: `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`.

- Clay user configuration owns concrete font-family fallback stacks and logical-pixel sizes for `monospace`, `proportional`, and `ui` profiles.
- Packages/modes declare semantic roles only; they must not select concrete families or absolute sizes. One role resolves both family and size.
- Defaults: `core.code` → monospace; `core.text` and Markdown → proportional; Markdown code ranges → monospace; Clay/package component text → UI.
- Keep typography in a layout-affecting client registry and protocol snapshot separate from theme color/style state; include typography/style revisions in layout invalidation.
- Resolve installed fonts on the client, retain generic fallbacks, keep package JavaScript/IPC out of paint, input, and layout hot paths.
- Plans changing typography cover editor shaping, mixed-role ranges, scrolling/viewport geometry, UI row/hit/accessibility geometry, package contracts, configuration/API docs, deterministic fallback/invalidation tests.

## UI Modernization

Sources: `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`, `2026-08-23-0052-tauri-react-client-architecture.md`, `2026-08-28-2234-package-defined-ui-design-systems.md`.

- Modernize defaults and token consumption without replacing Clay's existing user-configurable content-theme model. Component recipe replacement follows the design-system-packages rules above.
- Every normal-rendering color uses an active content-theme role (shell/component surfaces, text, borders, focus, selection, diagnostics, overlays, solid effect fallbacks). Design-system recipes may select those roles but never define literal or package-owned colors; browser/OS system colors are reserved for forced-colors mode.
- Every non-color modernized visual property uses an existing typed theme token, a typed design-system recipe property, or a justified additive typed token; never hardcode one fixed redesign.
- Preserve `theme.setTheme`, validated theme-package `designTokens`, fallback behavior, and existing theme compatibility; native `ResolvedUiTheme` paint reads are replaced by one cached frontend projection to CSS custom properties and CodeMirror theme extensions.
- Keep concrete typography user-owned through `theme.setTypography`; React components, CodeMirror adapters, and packages select semantic roles/variants only.
- Visual acceptance exercises dark/light themes plus representative typed overrides and contrast validation.