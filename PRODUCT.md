# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Primary:** Software engineers, systems programmers, technical writers, and power users who spend long continuous editing sessions working with codebases, text documents, and configuration files.
- **Secondary:** Package authors extending Clay with language modes, formatters, analyzers, themes, and UI contributions.

## Product Purpose

Clay is a high-performance, hackable desktop text and code editor combining Emacs-grade user configurability, sub-millisecond local input latency, and a safe, modern, accessible architecture.

Success means users can operate without cognitive distraction for hours, configure every aspect of their workflow via `~/.config/clay/init.js` and typed Clay JS APIs, and install third-party packages without compromising renderer security or editor performance.

## Positioning

Unlike Electron-based editors that incur high memory footprint and open-ended DOM mutation, and unlike traditional terminal editors that lack native accessible GUI primitives, Clay pairs a dedicated high-performance Rust core server with a sandboxed Tauri v2 + React/CodeMirror client.

Extensibility is achieved through inert declarative manifests and typed IPC APIs rather than arbitrary in-process JavaScript execution in the UI render thread.

## Operating Context

- **Surface Mode:** Operate. The user's focus is on completing technical tasks, writing code, reading documentation, and navigating project trees.
- **Environment:** Desktop application. Primary development and CI host is Linux (with Windows/macOS compatibility long-term).
- **Workflows:** Long continuous editing sessions, keyboard-driven navigation, multi-pane split layouts, file and workspace exploration, git status inspection, syntax-aware diagnostics, and user-driven configuration through `~/.config/clay/init.js`.

## Capabilities and Constraints

### Confirmed Capabilities
- Multi-pane split tree layout with fixed panel slots (left, right, top, bottom) and transient overlays (menus, completion popups, command centre).
- Bounded per-document syntax sessions with tiered syntax engines (Tree-sitter, native lexers).
- Incremental position indexing (`BytePositionIndex`) and atomic viewport render patches (`ViewportRenderPatch`).
- Inert Server-Driven UI (SDUI) component catalog and host-owned React component registry.
- Separation of content themes, user-owned typography (`UiTextVariant`), and UI design systems.

### Technical and Security Constraints
- **Process Boundary:** Headless Rust core server communicates via local IPC / DTO bridge with sandboxed Tauri v2 webview client.
- **Package Provenance & Trust Domains:** First-party bundled packages vs adopted third-party packages. Adopted packages run in a restricted trust domain and are denied raw CSS, DOM selectors, JSX, scripts, renderer callbacks, URLs, or direct Tauri APIs.
- **Color Authority Invariant:** Content themes are the sole normal-rendering color authority for surfaces, text, borders, focus rings, selections, and diagnostics. UI design systems map semantic color roles but may not declare palettes, literals, or package-owned color values (forced-colors mode is the only browser/OS color exception).
- **Performance Constraints:** Sub-millisecond keystroke latency, zero runtime parsing or selector matching in hot render/input paths, bounded DOM depth, and strict layout determinism.

## Brand Commitments

- **Name:** Clay.
- **Voice and Personality:** Restrained, utilitarian, mathematically precise, distraction-free, reliable.
- **Visual Authority Commitments:**
  - **Default UI Design System:** Restrained utilitarian Neobrutal geometry, visible compartmentalization (1px/2px solid borders, strict blueprint grid, 90-degree corners), bimodal density, and subtle spring motion.
  - **Reference Replacement System:** Glass design system package proving that the same host components can adopt squircle radii, translucency, backdrop blur, layered borders, and diffused ambient depth without source changes or bundled color themes.
  - **Theme Invariant:** Both design systems remain strictly colorized by whichever content theme the user selects.

## Evidence on Hand

- **Architecture Decisions:**
  - `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`
  - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
  - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
  - `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`
  - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
- **Design-System Recipe Separation:** Separation of content themes, user-owned typography (`UiTextVariant`), and UI design-system component recipes.
- **Recipe Matrix & Catalogs:**
  - Recipe matrix: `docs/development/ui-design-system-recipe-matrix.md`
  - Component catalog: `.agents/skills/clay-ui/references/components.md`
  - Token catalog: `.agents/skills/clay-ui/references/tokens.md`
  - React UI mapping: `docs/development/react-ui-catalog-mapping.md`
- **Absences & Fabrication Prohibitions:** No artificial testimonials, fabricated user counts, or invented commercial benchmarks.

## Product Principles

1. **Zero-Latency Long-Session Focus:** The editor must never stutter, drop frames during typing, or introduce visual noise that tires the user during extended work sessions.
2. **Inert Declarative Contributions:** Packages contribute versioned, typed data structures; the host owns all rendering, layout calculation, and accessibility trees.
3. **Separation of Color and Structure:** Content themes own color; typography configuration owns concrete font families; UI design systems own geometry, material, state mapping, and motion.
4. **Hard Security Boundaries:** Sandboxed client webview and restricted package trust domains prevent malicious or buggy extensions from compromising user files or system resources.
5. **Host-Owned Accessibility:** Accessible semantics (names, roles, focus management, announcements) are guaranteed by host-owned React Aria components and cannot be stripped or overridden by styling.

## Accessibility & Inclusion

- WCAG 2.1 AA compliance for all standard UI controls and text contrast.
- Full keyboard navigability across all surfaces without pointer requirements.
- Explicit support for OS-level forced-colors (high contrast) mode, `prefers-reduced-motion` (instant transitions), and `prefers-reduced-transparency` (solid material fallbacks for translucent/glass effects).
- AccessKit / ARIA tree integrity with polite live screen-reader announcements for tabs, panes, and status notifications.
