# UI Rules (Clay-Adapted)

For UI, theme, typography, tokens, components, layout, SDUI, accessibility, or visual-review tasks. Read this plus `references/components.md`, `references/tokens.md`, and the design system specification [`DESIGN.md`](../../../../DESIGN.md) before reviewing or editing implementation. Source: `decision-logs/2026-08-23-0115-mandatory-project-local-ui-skill-stack.md`.

## Design-Skill Routing

- Every Clay UI planning, implementation, review, theme, typography, layout, SDUI, and accessibility task reads this file, the catalogs, and `DESIGN.md`. Plans list these files under each UI task's `Approach -> Documentation Reviewed`; plan-level evidence and evidence from other tasks do not substitute.
- **Substantial new-surface design tasks** additionally load the four project-local design skills before source review or edits: `impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend`. Ordinary component/theme work follows the distilled rules here and does not load them.
- Use the four skills as complementary lenses when loaded; resolve conflicts through the user brief, `DESIGN.md`, accessibility, security, authority boundaries, component/catalog compatibility, and typed theme-token ownership. `DESIGN.md` wins over generic aesthetic advice.
- Adapt marketing-site guidance to Clay's Operate-mode desktop application. Do not force AIDA, hero sections, fixed palettes, hardcoded fonts/colors, GSAP, or decorative motion when they conflict with task needs.

## Design Language: Quiet Instrument

`DESIGN.md` is normative. The five laws, in review order:

1. **One surface, hairline zones** — whitespace and a single 1px `border.hairline` separate zones; nothing is a box inside a box.
2. **Two elevations** — canvas and overlay. Static regions never float; only transient surfaces (popover, sheet, palette, toast, modal) carry a shadow.
3. **Accent is state, never decoration** — focus, selection, active navigation, running work. Never a panel edge, heading, or decorative icon.
4. **Type does the structure** — 20/15/13/12/11/10px ladder, monospace for every datum (counts, paths, keys, timestamps, ids) with tabular figures, 10px/0.14em uppercase micro-labels for section eyebrows; prose is never mono, data is never proportional.
5. **Motion carries meaning** — 150ms `ease-out` state changes, 240ms `spring-snappy` surface entrances, one-shot 620ms accent pulse for keyboard focus moves; no hover lift, no loops except the running-work indicator.

Binding values (normative numbers in `DESIGN.md` §4/§5/§11):

- Radius ladder 5 / 8 / 12 / 16 / pill; **no 0px corners**, and no radius on a full-bleed region edge.
- One border weight: 1px hairline. The only state mark that a single-value property cannot express is the focused-field accent halo (an inset zero-blur shadow layer at `spread: 3`, 0.15 opacity) — the 2px leading selection bar and the tab underline are retired (`DESIGN.md` §14.13): selection is the `accent.primary` @0.15 fill plus a text-role change, never a second edge.
- Fills: veil planes `surface.panel` @ 0.55, inset wells `surface.control` @ 1.0, transient layers `surface.overlay` @ 1.0, accent state fills `accent.primary` @ 0.15.
- Elevation: the `overlay` and `pop` shadow recipes only, on transient surfaces only. Editor canvas, gutter, scroll track, panels, rows, and lists are always `backdropBlur == 0`.
- Reading measure: editor 92ch, agent transcript 72ch, empty-state prose ≤ 48ch.
- Retired patterns (`DESIGN.md` §14) are review blockers: hard offset shadows, 0px radii, borders wider than 1px, competing border weights on one surface, blur outside scrim/toast, decorative accent, gradients/inner-highlight rims/grain/textures, uppercase or tracked body text, hover lift on static controls, fabricated data.

## Design Artifacts (prototype → approval → conformance)

- **Prototypes** live in `design-artifacts/prototypes/<slug>/`: exploratory HTML opened over `file://` with no build step, rendered against the four shipped content themes, covering every component state and the applicable empty/loading/error/recovery states. A prototype has **no authority** and may be replaced at any time.
- **Approved artifacts** live in `design-artifacts/approved/<slug>/`: user-approved, append-only (a change is a new variant plus a new approval), and binding for implementation. `DESIGN.md` remains normative for the language itself; the approved artifact is normative for the surface, states, IA, and geometry it shows.
- Implementation and review read the approved artifact before editing and compare against it. Every deviation is either fixed to match or explicitly re-approved with the reason recorded — silent drift is a defect.
- Changing the language's appearance is a `DESIGN.md` edit plus design-system package data (never host CSS or a component rewrite), and requires re-approval through the artifact loop before host CSS changes land.
- Contract page: `design-artifacts/README.md`. Planning duty: `create-plan/references/clay.md` → UI Prototype and Explicit User Approval Task. Sources: user instruction 2026-09-11.

## Distilled Binding Rules

- Primitives and cataloged components first; custom components outside the catalog require explicit justification in the task's `Options Considered`.
- Token-only styling: every color from an active content-theme role; every non-color visual property from an existing typed token, recipe property, or a justified additive token. No literals, no hardcoded redesigns (`config.md` → UI Modernization).
- Typography: semantic roles only, concrete families/sizes user-owned (`config.md` → Typography Role Ownership). The mono-for-data rule selects a font **role**, never a family.
- Design-system geometry/material/motion changes belong in the design-system package (declarative data), never in host CSS modules, React components, or CodeMirror adapters.
- Contributions are inert and additive-only: no raw CSS, selectors, JSX, scripts, URLs, or renderer callbacks in the host; contracts are versioned and additive (`packages.md` → Package UI and Shell Layout).
- Components are state-complete: every interaction state (hover/focus/active/disabled/selected/error) is implemented, typed, and validated; accessibility semantics, focus, and keyboard flow are part of the component, not decoration.
- Keyboard-first: every action has a key path, every key is shown in the UI (kbd chips, hint rows, `?` map), and no affordance is hover-only.
- Operate-mode adaptation: no marketing-page aesthetics; density, efficiency, and low attention cost for frequent operations. The default answer to "add a frame/shadow/divider" is **no** unless a boundary or an elevation is load-bearing.
- Visual and accessibility review: see `planning-checklist.md` (screenshots, `get_app_state` first, blocker recording) and the `DESIGN.md` §15 checklist.

## Shell Layout Model

Working area → pane/split tree → mandatory `main` container plus optional `left`/`right`/`top`/`bottom` slots → Clay React components. Panels are fixed or transient (transient policy: explicit anchor, dismissal, focus); sizes stay user-configurable — never hardcode panel extents. Full contract in `packages.md` → Package UI and Shell Layout.

Surface composition follows `DESIGN.md` §12: workspace = sidebar + editor column (gutter, 92ch measure) + optional document-facts/outline rail, with the path field on demand (`⌘O`) and all document actions in one bar; agent = header (the agent-type picker as the title, model/usage/effort) + 72ch transcript + one-boundary composer + right inspector (Files, Memory, Context, Session Info, Settings) holding reference data such as skills and MCP servers; one global command palette (`⌘K`) — commands, not starting work.

**Agent session state is per tab, never process-global.** One `AgentSessionModule` per `TabRuntime`: the agent view creates it on first mount with that tab's own sender, the runtime adopts it (`attachAgentStore`) and disposes it on tab close, and a standalone mount (fixture, component test) keeps its own. The creation stays inside the lazy agent chunk — the shell may reference the store's *type* and lifetime hook, never construct it, or the whole AG-UI stack lands in the startup preload. The AG-UI relay is a process-wide broadcast — every connection's pump relays every session's frames with the *receiving* connection stamped — so a store accepts a frame only when its `clientId` is the tab's own and, when the frame carries a `sessionId`, when that session is the tab's **server-answered** binding (`session.bound`, never inferred from the first snapshot that arrives). See `../../../../docs/wiki/modules/react-agui-chat-stream.md`.

**A tab is one workspace plus one agent, with two views.** The tab record carries the picked folder, a nullable agent identity (`type` + `configRoot`, inert data) and the active view, and renders exactly one view at a time (Workspace | Agent), switched from tab chrome in the titlebar (`Ctrl+1`/`Ctrl+2`, never from inside a view); layout.json v2 round-trips all three per tab, the inactive view stays mounted (no re-fetch, no state loss), and a tab with **neither** half is skipped at restore rather than half-adopted. The switcher is `seg` recipe chrome, inert with a reason on an uncommitted tab; the strip's mono marker pulses while that tab's agent works. Positional tab activation is `Ctrl+Alt+<N>` (it yielded `Ctrl+<N>` to the view switcher). **The launcher is the landing surface** — a fresh window and every *uncommitted* tab (`⌘T`; a tab that has picked a folder or an agent shows its views, and a committed tab's empty pane shows the plain open prompt) — offering recent workspaces and configured agents, one of each per launch, with `⌘⏎` opening both into one tab; the launcher sets a tab's first state, not its only state (the agent is changed from the agent view's picker — still open, plan 118 task 35 — the folder from the workspace view, where a row pick rebinds that tab's workspace in place while it is uncommitted and opens its own tab once it holds one). The agent view's Files tab is the session's file history and opens a file in the workspace view — it is not a file browser. Approved reference: `design-artifacts/approved/quiet-instrument-migration/start.html`, `shell.html`, `agent-landing.html`; sources: `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`.

## Client Architecture

- Target desktop client: Tauri v2 + React + TypeScript + Vite + React Router Data Mode (in-memory router) + CodeMirror 6. Server stays separate and authoritative; Tauri is a narrow local presentation/OS bridge (window/webview lifecycle, narrow OS integration, server process/connection management, DTO translation) — it does not absorb documents, workspaces, packages, language services, or `deno_core` authority.

Sources: `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`, `2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`, `2026-09-14-1602-ts-rs-generated-webview-contract-from-dto-layer.md`.

- Preserve the length-prefixed `rkyv` server transport behind the Tauri Rust core; React receives bounded JSON-compatible DTOs through typed commands/channels; IDs that may exceed JS integer precision use strings.
- One hand-written definition per webview contract type: the serde-JSON DTO layer (`src-tauri/src/bridge/dto.rs`, `src-tauri/src/bridge/errors.rs`). TypeScript declarations are generated from that layer with ts-rs 12 into `frontend/src/bridge/generated/` (checked in; a staleness check in `scripts/check.sh` fails CI on stale bindings) and never hand-mirrored field-by-field; `src/protocol` and the other core crates carry no codegen derives, so a contract type the webview needs is either projected in the DTO layer or kept as a TS-side narrowing (opaque catch-all), never a copy.
- Ordinary typing applies locally in CodeMirror and queues bounded ordered deltas asynchronously; React rendering, Tauri IPC, server work, package JS, file IO, and AI never block the keystroke-to-local-paint path. CodeMirror owns local text, viewport, incremental position indexing, inert syntax projection; package-selected parsers and executable syntax-management stay in bounded server syntax sessions (`protocol-perf.md`). No package parser/highlighter JS in the webview without a separate measured decision (authority, artifact trust, server/headless parity, merge precedence).
- Main webview gets narrow Clay commands only; no broad Tauri filesystem/shell/process/network plugin capabilities; authorization stays server-side and provenance-aware. Package UI is inert/declarative, reconciled by stable node ID through the Clay-owned React component registry; first-party trusted components may compile into the frontend; arbitrary third-party UI is isolated in a sandboxed surface with no direct Tauri IPC.
- Theme packages are validated data; one frontend theme runtime maps semantic tokens to CSS custom properties and CodeMirror styles (`config.md` → UI Modernization). AG-UI is the React-facing agent event/state protocol over a custom Tauri channel transport; Prism stays Clay-owned; ACP stays out of the first-party path (`packages.md` → Agent Host).

## Catalog (read before UI work)

- Catalog paths: `DESIGN.md` (normative design language, values, per-surface recipes, rules), `references/components.md` (component kinds, style variables, chrome primitives, internal surfaces, recipe slots) and `references/tokens.md` (token types, core tokens, typography hierarchy, consumption contracts) — the catalog references are the single source of truth for reusable Clay UI primitives. Read all three before reviewing or editing any UI implementation.
- Keep the catalog current: any change that adds, modifies, or removes a component, primitive, token, layout rule, or design-language value updates `DESIGN.md`, the catalog files in the same commit/phase, `docs/reference/packages/creating-packages.md`, and the drift tests together (cargo test fails on drift).
- Additive-only: component kinds, style variables, and token names are never renamed or removed; changes need a decision log and migration path. Shipping a new design system is additive and never disturbs another package's recipes; removing a shipped one is a decision-logged migration with the user's explicit direction, which is how plan 118 removed two (`components.md` → catalog currency).
- Architecture map (live; full per-surface table in `components.md`): `src/shell/components.rs` (catalog + style-variable validation), `src/shell/theme.rs` (token resolver), `src/shell/design_system.rs` (recipe validation, bounds, fallback resolution), `frontend/src/theme/` (token → `--clay-*` projection and recipe → `--clay-ds-*` projection), `src/shell/layout/` (pane split tree, slots), `src/shell/package_ui.rs` (package UI runtime), `src/shell/transient_menu.rs` (menus + completion), `src/shell/file_browser.rs`, `frontend/src/components/chrome.tsx` (chrome primitives), `frontend/src/sdui/registry.tsx` + `frontend/src/packages/PackageWorkspace.tsx` (SDUI rendering), `src/editor/theme.rs` + `src/editor/typography.rs` (editor colors, type), `docs/reference/packages/creating-packages.md` (authoring guide).
