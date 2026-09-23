# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Primary:** Software engineers, systems programmers, technical writers, and power users who spend long continuous editing sessions working with codebases, text documents, and configuration files.
- **Secondary:** Package authors extending Clay with language modes, formatters, analyzers, themes, and UI contributions.

## Product Purpose

Clay is a high-performance, hackable desktop text and code editor combining Emacs-grade user configurability, sub-millisecond local input latency, and a safe, modern, accessible architecture.

Success means users can operate without cognitive distraction for hours, configure every aspect of their workflow via `~/.clay/init.js` and typed Clay JS APIs, and install third-party packages without compromising renderer security or editor performance.

## Positioning

Unlike Electron-based editors that incur high memory footprint and open-ended DOM mutation, and unlike traditional terminal editors that lack native accessible GUI primitives, Clay pairs a dedicated high-performance Rust core server with a sandboxed Tauri v2 + React/CodeMirror client.

Extensibility is achieved through inert declarative manifests and typed IPC APIs rather than arbitrary in-process JavaScript execution in the UI render thread.

## Operating Context

- **Surface Mode:** Operate. The user's focus is on completing technical tasks, writing code, reading documentation, and navigating project trees.
- **Environment:** Desktop application. Primary development and CI host is Linux (with Windows/macOS compatibility long-term).
- **Workflows:** Long continuous editing sessions, keyboard-driven navigation, multi-pane split layouts, file and workspace exploration, git status inspection, syntax-aware diagnostics, and user-driven configuration through `~/.clay/init.js`.
- **Surfaces:** One tab holds one workspace and one agent, with two views (Workspace, Agent) switched from tab chrome; a fresh window and every empty tab open the **launcher**, which offers recent workspaces and the configured agents (one of each per launch). The former chat surface was removed in plan 118.

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
  - **UI Design Language — Quiet Instrument (the only shipped one):** one continuous surface zoned by 1px hairlines and whitespace, two elevations (canvas and overlay), accent reserved for state, a 5/8/12/16/pill radius ladder, monospace for data, and decelerating 150ms/240ms motion. Normative spec: `DESIGN.md`; shipped as `@clay/design-instrument`, with `@clay/core` resolving the same language's host-consumed subset before a snapshot lands.
  - **One design language ships:** the earlier Restrained Neobrutal and Luminous Glass systems were the migration's comparison points and were **removed** in plan 118; no shipped surface may reference them, and the design-system choice set is `@clay/core` plus the shipped package.
  - **Four shipped content themes:** Modus Operandi (light) and Modus Vivendi (dark) as the default pair chosen by OS preference, plus Gruvbox Material Dark and Gruvbox Material Light; each declares the same typed theme-side roles and is refused activation when a composited contrast floor fails.
  - **Theme Invariant:** the design system remains strictly colorized by whichever content theme the user selects.

## Evidence on Hand

- **Architecture Decisions:**
  - `decision-logs/2026-09-11-1615-quiet-instrument-design-language.md`
  - `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`
  - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
  - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
  - `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`
  - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
- **Design System Specification:** `DESIGN.md` (Quiet Instrument) — laws, values, geometry, materials, motion, typography, state language, per-surface recipes, shell composition, accessibility invariants, retired patterns, review checklist; proposal evidence in `design-artifacts/DS/`.
- **Design-System Recipe Separation:** Separation of content themes, user-owned typography (`UiTextVariant`), and UI design-system component recipes.
- **Recipe Matrix & Catalogs:**
  - Recipe matrix: `docs/development/ui-design-system-recipe-matrix.md`
  - Component catalog: `.agents/skills/clay-execution/references/components.md`
  - Token catalog: `.agents/skills/clay-execution/references/tokens.md`
  - React UI mapping: `docs/development/react-ui-catalog-mapping.md`
- **Absences & Fabrication Prohibitions:** No artificial testimonials, fabricated user counts, or invented commercial benchmarks. UI surfaces show real state or an explicit unavailable state — never fabricated metrics, tool output, or counts.

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
