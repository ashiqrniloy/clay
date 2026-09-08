# Configuration, Theme, Typography, and Modernization

## Configuration System

Sources: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`, `2026-07-19-0328-configuration-keymaps-survive-mode-activation.md`, `2026-08-11-0352-configuration-watch-auto-reload-and-modular-structure.md`.

- Dotted-ID convention: core Clay command/API/option IDs are bare `<domain>.<name>` (`shell.*`, `editor.*`, `documents.*`, `workspace.*`, `runtime.*`, …), NEVER `clay.<domain>.*`. Package configuration and package command IDs always start with the package's own `apiPrefix` (`<package>.<name>`, e.g. `markdown.layout.defaultVisibility`); `setPackageOption` rejects `clay.`-prefixed and non-package-prefixed options. Third-party packages cannot claim a reserved core domain as prefix (`RESERVED_CORE_API_DOMAINS`, `src/packages/manifest.rs`). `clay:` module specifiers and `package.json` `clay.*` manifest key paths are the only surviving `clay` prefixes. See `js-api.md`.
- User configuration loads from `~/.config/clay/init.js`; `init.js` may load other local configuration files for modularity.
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
- Adopted design systems retain exact third-party provenance and runtime-domain status; activation grants no renderer authority and must support revocation and fallback.
- Plans include: primitive/catalog audit, bounded schema and fallback rules, package activation/configuration, all-surface migration, Neobrutal and Glass conformance fixtures, color-source deny tests, theme/design-system cross-product tests, performance budgets, visual/accessibility review, public docs, manual tests, final wiki maintenance.

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