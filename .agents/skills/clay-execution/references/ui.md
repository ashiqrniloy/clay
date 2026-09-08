# UI Rules (Clay-Adapted)

For UI, theme, typography, tokens, components, layout, SDUI, or accessibility tasks. Read this plus `references/components.md` and `references/tokens.md` before reviewing or editing implementation. Source: `decision-logs/2026-08-23-0115-mandatory-project-local-ui-skill-stack.md`.

## Design-Skill Routing

- Every Clay UI planning, implementation, review, theme, typography, layout, SDUI, and accessibility task reads this file plus the catalogs. Plans list these files under each UI task's `Approach -> Documentation Reviewed`; plan-level evidence and evidence from other tasks do not substitute.
- **Substantial new-surface design tasks** additionally load the four project-local design skills before source review or edits: `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend`. Ordinary component/theme work follows the distilled rules here and does not load them.
- Use the four skills as complementary lenses when loaded; resolve conflicts through the user brief, existing Clay identity, accessibility, security, authority boundaries, component/catalog compatibility, and typed theme-token ownership.
- Adapt marketing-site guidance to Clay's Operate-mode desktop application. Do not force AIDA, hero sections, fixed palettes, hardcoded fonts/colors, GSAP, or decorative motion when they conflict with task needs.

## Distilled Binding Rules (from the four design skills)

- Primitives and cataloged components first; custom components outside the catalog require explicit justification in the task's `Options Considered`.
- Token-only styling: every color from an active content-theme role; every non-color visual property from an existing typed token, recipe property, or a justified additive token. No literals, no hardcoded redesigns (`config.md` → UI Modernization).
- Typography: semantic roles only, concrete families/sizes user-owned (`config.md` → Typography Role Ownership).
- Contributions are inert and additive-only: no raw CSS, selectors, JSX, scripts, URLs, or renderer callbacks in the host; contracts are versioned and additive (`packages.md` → Package UI and Shell Layout).
- Components are state-complete: every interaction state (hover/focus/active/disabled/selected/error) is implemented, typed, and validated; accessibility semantics, focus, and keyboard flow are part of the component, not decoration.
- Operate-mode adaptation: no marketing-page aesthetics; density, efficiency, and low attention cost for frequent operations.
- Visual and accessibility review: see `planning-checklist.md` (screenshots, `get_app_state` first, blocker recording).

## Shell Layout Model

Working area → pane/split tree → mandatory `main` container plus optional `left`/`right`/`top`/`bottom` slots → Clay React components. Panels are fixed or transient (transient policy: explicit anchor, dismissal, focus); sizes stay user-configurable — never hardcode panel extents. Full contract in `packages.md` → Package UI and Shell Layout.

## Client Architecture

- Target desktop client: Tauri v2 + React + TypeScript + Vite + React Router Data Mode (in-memory router) + CodeMirror 6. Server stays separate and authoritative; Tauri is a narrow local presentation/OS bridge (window/webview lifecycle, narrow OS integration, server process/connection management, DTO translation) — it does not absorb documents, workspaces, packages, language services, or `deno_core` authority.

Sources: `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`, `2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`.

- Preserve the length-prefixed `rkyv` server transport behind the Tauri Rust core; React receives bounded JSON-compatible DTOs through typed commands/channels; IDs that may exceed JS integer precision use strings.
- Ordinary typing applies locally in CodeMirror and queues bounded ordered deltas asynchronously; React rendering, Tauri IPC, server work, package JS, file IO, and AI never block the keystroke-to-local-paint path. CodeMirror owns local text, viewport, incremental position indexing, inert syntax projection; package-selected parsers and executable syntax-management stay in bounded server syntax sessions (`protocol-perf.md`). No package parser/highlighter JS in the webview without a separate measured decision (authority, artifact trust, server/headless parity, merge precedence).
- Main webview gets narrow Clay commands only; no broad Tauri filesystem/shell/process/network plugin capabilities; authorization stays server-side and provenance-aware. Package UI is inert/declarative, reconciled by stable node ID through the Clay-owned React component registry; first-party trusted components may compile into the frontend; arbitrary third-party UI is isolated in a sandboxed surface with no direct Tauri IPC.
- Theme packages are validated data; one frontend theme runtime maps semantic tokens to CSS custom properties and CodeMirror styles (`config.md` → UI Modernization). AG-UI is the React-facing agent event/state protocol over a custom Tauri channel transport; Prism stays Clay-owned; ACP stays out of the first-party path (`packages.md` → Agent Host).

## Catalog (read before UI work)

- Catalog paths: `references/components.md` (component kinds, style variables, chrome primitives, internal surfaces, recipe slots) and `references/tokens.md` (token types, core tokens, typography hierarchy, consumption contracts) — the single source of truth for reusable Clay UI primitives. Read both before reviewing or editing any UI implementation.
- Keep the catalog current: any change that adds, modifies, or removes a component, primitive, token, or layout rule updates the catalog files in the same commit/phase, `docs/reference/packages/creating-packages.md`, and the drift tests together (cargo test fails on drift).
- Additive-only: component kinds, style variables, and token names are never renamed or removed; changes need a decision log and migration path.
- Architecture map (live; full per-surface table in `components.md`): `src/shell/components.rs` (catalog + style-variable validation), `src/shell/theme.rs` (token resolver), `src/shell/layout/` (pane split tree, slots), `src/shell/package_ui.rs` (package UI runtime), `src/shell/transient_menu.rs` (menus + completion), `src/shell/file_browser.rs`, `frontend/src/components/chrome.tsx` (chrome primitives), `frontend/src/sdui/registry.tsx` + `frontend/src/packages/PackageWorkspace.tsx` (SDUI rendering), `frontend/src/theme/` (token → `--clay-*` projection), `src/editor/theme.rs` + `src/editor/typography.rs` (editor colors, type), `docs/reference/packages/creating-packages.md` (authoring guide).
