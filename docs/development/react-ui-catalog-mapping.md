# React UI Catalog Mapping (Tauri/React Migration)

Plan 097 Phase 1/4 — locked before any React UI implementation. Sources:
`.agents/skills/clay-ui/references/components.md`,
`.agents/skills/clay-ui/references/tokens.md`, `docs/reference/ui-components.md`,
React Aria Components, and WAI-ARIA Authoring Practices. Architecture decision:
`decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.

## Locked decisions

1. **Native HTML first.** Plain `button`, `input`, `textarea`, headings,
   landmarks (`header`/`nav`/`main`/`aside`/`footer`) wherever behavior is
   trivial; no component exists for styling alone.
2. **React Aria Components is the headless behavior layer** for complex
   collection/menu/combobox/dialog/tab semantics, rendered beneath thin
   Clay-owned styled wrappers. It is an implementation detail — packages never
   see React Aria names, props, or DOM contracts beyond validated SDUI kinds.
3. **CodeMirror 6 owns the editor surface only.** Editor text state never lives
   in React state; React mounts one host element per pane document view.
4. **react-resizable-panels** owns split/divider drag + keyboard resize inside
   the pane/split tree (headless, roving-focus separators).
5. **Native bounded collections first.** Command and file-browser snapshots are
   capped at 256 rows; virtualize only after measured render cost warrants the
   extra focus/accessibility machinery. Completion stays capped at 8 rows.
6. **Semantic token names and component kinds are preserved 1:1 during parity.**
   Schema changes require migration tests, never silent reinterpretation.

## Implemented (Phase 4)

Locked mapping above is now backed by these files. Later phases fill editor,
SDUI, and split-tree rows; they do not rename kinds or tokens.

| Surface | Implementation |
| --- | --- |
| Memory router + landmarks | `frontend/src/app/router.tsx`, `layout/app-shell.tsx` |
| Tab strip | `frontend/src/app/layout/tab-bar.tsx` (RAC `Tabs`/`TabList`) |
| Working area / optional left slot | `frontend/src/app/layout/working-area.tsx` |
| Theme/typography adapter | `frontend/src/theme/adapter.ts`, `state/theme-store.ts` |
| Resolved snapshot authority | `src/shell/theme.rs::resolve_theme_token_snapshot`, `src-tauri/src/bridge/dto.rs` |
| button kind | `frontend/src/components/button.tsx` |
| label / text variants | `frontend/src/components/text.tsx` |
| textInput kind | `frontend/src/components/text-field.tsx` |
| dropdown / list / collapse | `frontend/src/components/controls.tsx` |
| modal + scrim | `frontend/src/components/modal.tsx` |
| badge / kbd / divider | `frontend/src/components/chrome.tsx` |
| DEV fixtures | `frontend/src/routes/fixture.tsx` (`/fixture/states`, `/fixture/controls`, `/fixture/editor`, `/fixture/splits`, `/fixture/intelligence`, `/fixture/package-ui`, `/fixture/command-centre`, `/fixture/command-centre-empty`, `/fixture/path-browser`, `/fixture/settings`, `/fixture/chat`) |
| pane split tree | `frontend/src/shell/{split-tree.ts,PaneTree.tsx,WorkspacePanes.tsx}` |
| window tabs + persist | `frontend/src/shell/{tab-store,persist,workspace-controller}.ts` |
| editor interaction/intelligence | `frontend/src/editor/extensions/*` (CodeMirror native semantics + generic server-result adapters) |
| resolved editor vocabulary | `ThemeSnapshotDto.editor_styles` → `--clay-editor-*` variables; Rust `StyleRegistry` remains authority |
| SDUI stable-ID state | `frontend/src/sdui/{state,renderer}.ts*`; stale version denial and targeted map replacement |
| package component registry | `frontend/src/sdui/registry.tsx`; all 15 implemented kinds, no package JSX/CSS |
| slots/overlays/status | `frontend/src/packages/PackageWorkspace.tsx`; fixed slots, contained overlays, package status |
| package provenance | Rust `PackageUiProvenance` + bridge-parsed `PackageUiSnapshotDto`; exact trusted/third-party labels |
| Host fallback tokens | `frontend/src/styles/tokens.css` + `global.css` |
| editorView kind | `frontend/src/editor/ClayEditor.tsx` + `create-editor.ts` |

`z.*` CSS values are stacking integers (`0/10/20/40/50`), not the catalog
level names — CSS `z-index` cannot accept `modal`.

## Component mapping (package-facing `ComponentKind`s)

| Kind | Target renderer | Accessibility contract | Notes |
| --- | --- | --- | --- |
| `editorView` | CodeMirror 6 `EditorView` in Clay host (`frontend/src/editor/ClayEditor.tsx`) | CM built-in textbox/paragraph semantics; host carries `aria-label` = document name | One content host per pane leaf; React re-renders only chrome, never text |
| `panel` | Clay `Panel` wrapper over `<section>` | `region`/`complementary` landmark + `aria-labelledby` title | Slot-bound fixed/transient panels; collapse affordance uses `Disclosure` pattern |
| `label` | Native text node / `<p>` / `<span>` with variant class | Plain text (no role) | Variant from `typography.*`; disabled → `text.disabled` × `opacity.disabled` |
| `button` | React Aria `Button` over native `<button>` | Native button semantics + `aria-disabled`; visible focus ring (`focus.ring`) | Variants `default`/`muted`/`primary`/`danger`; all five states from tokens |
| `list` | React Aria `ListBox` + `ListBoxItem` | `listbox`/`option`, `aria-selected`, roving tabindex, typeahead | Row states via `surface.selected`/`hover`/`active` tokens |
| `flex` | Plain flexbox `<div>` | Transparent to AT | `gap` from spacing tokens |
| `stack` | Positioned `<div>` layers | Transparent to AT | z-order via `z.*` levels |
| `overlay` | React Aria `Popover` (non-modal) | Focus containment/restoration per anchor policy | Anchors `working-area`/`active-pane`/`main`/`pointer`; centered host stays internal |
| `scroll` | Native overflow container + token-styled scrollbars | Keyboard scrollable region; `role="region"` only when labelled | Rest scrollbar near-invisible per `opacity.disabled` rest contract |
| `portal` | `createPortal` into fixed z-layer hosts | Transparent to AT | Escape hatch for transient surfaces; same layer tokens |
| `statusItem` | Footer text node in status bar | `role="status"` only when dynamically announced | Disabled → dimmed text pair |
| `dropdown` | React Aria `Select` (trigger + hidden `ListBox`) | `combobox`-free single-select: button + `listbox`, arrow/typeahead nav | `selected_index` maps to RAC `selectedKey` |
| `collapse` | React Aria `Disclosure` | Disclosure button pattern (`aria-expanded`/`aria-controls`) | Toggle emits declared `clay.ui.collapseToggle` intent |
| `modal` | React Aria `Modal` + `Dialog` | `role="dialog"`, `aria-modal`, focus trap + restore, Escape → declared dismiss intent | Scrim via `paint_scrim` projection; `z.modal` |
| `textInput` | React Aria `TextField` over native `<input type="text">` | Label/description wiring; validation state → `aria-invalid` + `aria-describedby` | Multiline `textArea` variant is a justified gap below |
| `table` | **Justified gap (reserved kind)** — target React Aria `Table` when unlocked | n/a until reserved→implemented | No first-party consumer today |

Planned catalog entries (not yet kinds) get targets now so parity work composes
instead of improvising: Tooltip → React Aria `Tooltip`+`TooltipTrigger`;
badge/tag → native `<span>` with badge tokens (`status`/`note`);
kbd hint → native `<kbd>` with kbd tokens; icon slot → inline SVG `aria-hidden`
or `<img alt>` at `dimension.icon.size`; toast/notification → internal overlay +
timer on `z.overlay` (React Aria `Toast` if product need lands).

## Clay-native surfaces and chrome primitives

| Current surface / primitive (`components.md`) | Target renderer | Accessibility contract |
| --- | --- | --- |
| Shell root working area | Clay shell layout component (landmark `main`) | Single `main` landmark per window |
| Pane split tree | react-resizable-panels `PanelGroup`/`Panel`/`PanelResizeHandle` | Separator role + arrow-key resize; ratios clamp 0.05–0.95 unchanged |
| Fixed panel slots | Slot containers in shell layout | `complementary` landmarks; hidden slot removes landmark |
| Status bar | `<footer>` strip | Landmark footer; items plain text/live-region |
| Welcome entry surface | React page composition | Heading hierarchy + Group/Status semantics preserved; Open File/Folder route through existing command intents |
| Transient menu (prompt/item list) | React Aria `Menu`/`ComboBox`/`Autocomplete` by origin | Combobox/listbox patterns; sanitized labels pass through unchanged |
| Inline completion pop-up | CodeMirror autocompletion extension DOM (internal) | CM listbox semantics; 8-row cap kept |
| Command Centre (centered) | `frontend/src/command-centre/CommandCentre.tsx`: React Aria `Dialog` + bounded scrollable `ListBox` | Dialog/focus restoration, server-owned selection/query, live result count; package-inert |
| Path browser | Same Command Centre projection with semantic backspace and secondary activation | Server-held entry activation and browse-to-grant contract unchanged |
| File browser | React Aria `Tree` + TanStack Virtual rows | Treeitem nesting, selection/focus model from server snapshot |
| Window tab bar | React Aria `Tabs` (`TabList`/`Tab`) + scrollable strip | `tab`/`tablist` semantics; close glyph is labelled nested button; shrink-to-fit + scroll offset contract preserved |
| `paint_divider` | `<hr>`/separator element or resize handle | `separator` role |
| `paint_focus_ring` | `:focus-visible` outline from `border.focus`/`focus.ring` | Never `outline: none` without replacement |
| `paint_panel_chrome` | `Panel` chrome styles | Region/complementary naming |
| `paint_scroll_chrome` | `scrollbar-color`/`scrollbar-width` + `::-webkit-scrollbar` styles | Native scrollbar keeps keyboard/AT behavior free |
| `paint_badge` / `paint_kbd_hint` / `paint_icon_slot` | Badge / `<kbd>` / icon components above | Status/note/kbd/img roles as listed |
| `paint_tooltip_shell` | Shared tooltip surface styles | Tooltip pattern (hover+focus trigger) |
| `paint_scrim` | Full-window scrim div behind centered dialog | Dialog backdrop, inert background (`aria-hidden`) |
| Editor chrome (gutter, active line, indent guides, bracket match, folds, scrollbar, diagnostics) | CodeMirror extensions (`@codemirror/view` gutter/highlighter, `@codemirror/language` foldGutter, `matchBrackets`) themed from StyleRegistry projection | CM accessibility; fold chevrons are gutter buttons with expanded/collapsed state |

## Token projections

Resolution happens once per theme snapshot install: the Rust-resolved theme DTO
is projected by one adapter into CSS custom properties on the app root. Paint,
layout, pointer, scroll, keypress, and text-event paths read cached CSS vars
only — no re-resolution, parsing, or IPC in hot paths.

### Core semantic tokens → CSS custom properties

Naming rule: `token.name.sub` → `--clay-token-name-sub`. Complete locked table:

<!-- 91 core tokens; regenerate with the extractor in tests/documentation_coverage.rs -->
| `accent.muted` | `--clay-accent-muted` |
| `accent.primary` | `--clay-accent-primary` |
| `border.focus` | `--clay-border-focus` |
| `border.hairline` | `--clay-border-hairline` |
| `border.kbd` | `--clay-border-kbd` |
| `border.strong` | `--clay-border-strong` |
| `border.subtle` | `--clay-border-subtle` |
| `density.compact` | `--clay-density-compact` |
| `density.default` | `--clay-density-default` |
| `density.spacious` | `--clay-density-spacious` |
| `diagnostic.error` | `--clay-diagnostic-error` |
| `diagnostic.info` | `--clay-diagnostic-info` |
| `diagnostic.success` | `--clay-diagnostic-success` |
| `diagnostic.warning` | `--clay-diagnostic-warning` |
| `dimension.border.hairline` | `--clay-dimension-border-hairline` |
| `dimension.border.thick` | `--clay-dimension-border-thick` |
| `dimension.border.thin` | `--clay-dimension-border-thin` |
| `dimension.icon.size` | `--clay-dimension-icon-size` |
| `dimension.kbd.height` | `--clay-dimension-kbd-height` |
| `dimension.overlay.centered.width` | `--clay-dimension-overlay-centered-width` |
| `dimension.panel.side.default` | `--clay-dimension-panel-side-default` |
| `dimension.panel.side.max` | `--clay-dimension-panel-side-max` |
| `dimension.panel.side.min` | `--clay-dimension-panel-side-min` |
| `dimension.panel.vertical.default` | `--clay-dimension-panel-vertical-default` |
| `dimension.panel.vertical.max` | `--clay-dimension-panel-vertical-max` |
| `dimension.panel.vertical.min` | `--clay-dimension-panel-vertical-min` |
| `dimension.scrollbar.width` | `--clay-dimension-scrollbar-width` |
| `dimension.sidebar.default` | `--clay-dimension-sidebar-default` |
| `elevation.none` | `--clay-elevation-none` |
| `elevation.overlay` | `--clay-elevation-overlay` |
| `elevation.raised` | `--clay-elevation-raised` |
| `focus.ring` | `--clay-focus-ring` |
| `motion.fast` | `--clay-motion-fast` |
| `motion.instant` | `--clay-motion-instant` |
| `motion.normal` | `--clay-motion-normal` |
| `motion.slow` | `--clay-motion-slow` |
| `opacity.disabled` | `--clay-opacity-disabled` |
| `opacity.full` | `--clay-opacity-full` |
| `opacity.scrim` | `--clay-opacity-scrim` |
| `radius.lg` | `--clay-radius-lg` |
| `radius.none` | `--clay-radius-none` |
| `radius.panel` | `--clay-radius-panel` |
| `radius.sm` | `--clay-radius-sm` |
| `radius.xs` | `--clay-radius-xs` |
| `spacing.badge` | `--clay-spacing-badge` |
| `spacing.inline` | `--clay-spacing-inline` |
| `spacing.lg` | `--clay-spacing-lg` |
| `spacing.md` | `--clay-spacing-md` |
| `spacing.none` | `--clay-spacing-none` |
| `spacing.panel` | `--clay-spacing-panel` |
| `spacing.row` | `--clay-spacing-row` |
| `spacing.sm` | `--clay-spacing-sm` |
| `spacing.tooltip` | `--clay-spacing-tooltip` |
| `spacing.xl` | `--clay-spacing-xl` |
| `spacing.xs` | `--clay-spacing-xs` |
| `spacing.xxl` | `--clay-spacing-xxl` |
| `spacing.xxs` | `--clay-spacing-xxs` |
| `surface.active` | `--clay-surface-active` |
| `surface.badge` | `--clay-surface-badge` |
| `surface.control` | `--clay-surface-control` |
| `surface.disabled` | `--clay-surface-disabled` |
| `surface.hover` | `--clay-surface-hover` |
| `surface.kbd` | `--clay-surface-kbd` |
| `surface.list` | `--clay-surface-list` |
| `surface.main` | `--clay-surface-main` |
| `surface.overlay` | `--clay-surface-overlay` |
| `surface.panel` | `--clay-surface-panel` |
| `surface.scrim` | `--clay-surface-scrim` |
| `surface.scrollbar` | `--clay-surface-scrollbar` |
| `surface.scrollbar.track` | `--clay-surface-scrollbar-track` |
| `surface.selected` | `--clay-surface-selected` |
| `surface.tooltip` | `--clay-surface-tooltip` |
| `text.badge` | `--clay-text-badge` |
| `text.disabled` | `--clay-text-disabled` |
| `text.icon` | `--clay-text-icon` |
| `text.kbd` | `--clay-text-kbd` |
| `text.muted` | `--clay-text-muted` |
| `text.primary` | `--clay-text-primary` |
| `text.tooltip` | `--clay-text-tooltip` |
| `typography.body` | `--clay-typography-body` |
| `typography.caption` | `--clay-typography-caption` |
| `typography.detail` | `--clay-typography-detail` |
| `typography.display` | `--clay-typography-display` |
| `typography.section` | `--clay-typography-section` |
| `typography.status` | `--clay-typography-status` |
| `typography.title` | `--clay-typography-title` |
| `z.base` | `--clay-z-base` |
| `z.modal` | `--clay-z-modal` |
| `z.overlay` | `--clay-z-overlay` |
| `z.panel` | `--clay-z-panel` |
| `z.tooltip` | `--clay-z-tooltip` |

### Derived variables (emitted at install alongside core tokens)

- Font roles: `--clay-font-ui`, `--clay-font-monospace`, `--clay-font-proportional`
  (user-configured family stacks).
- Text variants: `--clay-text-{display,title,section,body,status,detail,caption}-size`
  computed once from role base × user-owned hierarchy scales.
- Density: spacing rhythm values are emitted pre-scaled by `spacing_scale()`
  (0.875/1.0/1.125); density never rescales panel dimensions or document text.

### Explicitly internal CodeMirror values (no public CSS custom properties)

Editor base UI colors and syntax colors (`src/editor/theme.rs` `BaseUiColors` /
`StyleRegistry`: `shellBg`, `panelBg`, `text`, `placeholder`, `selection`,
`caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`,
`diagnosticError`, `diagnosticWarning`, `diagnosticInfo`, `searchMatch`,
`unused`, `gutterFg`, `gutterFgActive`, `lineHighlight`, `indentGuide`,
`bracketMatch`, plus syntax tokens) are **internal: CodeMirror** values fed
directly into the CM theme object by the adapter. They never become CSS custom
properties and are not part of the package-facing token surface.

## Interaction-state mapping

Native-client five-state contract → DOM mechanisms (all reads from tokens):

| State | Mechanism |
| --- | --- |
| Rest | Base token values (`surface.control`, `surface.list`, …) |
| Hover | `:hover` / `[data-hovered]` (React Aria) → `surface.hover` etc. |
| Active | `:active` / `[data-pressed]` → `surface.active` |
| Focus | `:focus-visible` / `[data-focused]` → ring via `focus.ring`/`border.focus` |
| Disabled | `[data-disabled]` + `aria-disabled` → `surface.disabled`, `text.disabled` × `opacity.disabled` |

Precedence `Disabled > Active > Hover > Focus > Rest` matches
`applicable_states(kind)`; state completeness per kind is pinned by existing
conformance tests and carried forward unchanged.

## Performance locks

- Editor text lives in CodeMirror; React renders chrome around it. No store
  subscription fires per keystroke; Zustand slices use narrow selectors keyed
  by document/tab ID.
- Decoration/diagnostic/folding chunks apply through CM transaction effects;
  viewport requests reuse server dedup/background scheduling.
- Command and file-browser collections remain capped at 256. They use native
  scroll/collection semantics until profiling shows virtualization is needed;
  completion keeps its hard 8-row cap.
- CodeMirror language packages and any future heavy renderer (KaTeX/IPynb work)
  load conditionally (dynamic import) on first relevant document open.
- Theme/typography snapshots install once per revision; layout invalidation
  semantics (single invalidation per change, atomic partial rejection) carry
  forward unchanged.

## Security locks (package UI)

Packages contribute only validated declarative snapshots; the frontend:

- Renders package trees exclusively from validated SDUI/contribution data —
  no `dangerouslySetInnerHTML`, no arbitrary style strings, no event-handler
  props from packages anywhere in projected trees.
- Dispatches every package action through the single typed intent dispatcher;
  package code cannot import Tauri APIs, fetch privileged commands, reach
  global CSS, or touch secrets/native handles.
- Typed style variables accept token names/enums only; raw values were already
  rejected server-side and cannot be reintroduced client-side.
- Trust domains and provenance labels render exactly as validated server-side.

## Justified target gaps

| Gap | Justification | Target |
| --- | --- | --- |
| `table` kind | Reserved in catalog; no first-party consumer | React Aria `Table` when unlocked |
| Multiline `textArea` composer field | Chat composer needs newline chords; single-line `textInput` cannot host them (Phase 25 review gap) | React Aria `TextField` multiline over native `<textarea>`; generic catalog addition in its own phase |
| Generic pane-content contribution | Empty-tab landing needs a generic package pane host, not product-named kinds (Phase 25 review gap) | Generic `Package` pane-content variant in its own phase |
| Toast/notifications | Planned, no current consumer driving urgency | Internal overlay + timer now; React Aria `Toast` if needed |

## Verification

- `tests/documentation_coverage.rs::react_catalog_maps_every_component_kind` —
  every package-facing `ComponentKind` in `components.md` appears exactly once
  above with non-empty renderer and accessibility cells.
- `tests/documentation_coverage.rs::core_tokens_project_to_css_variables_or_internal_codemirror_values`
  — every core token in `tokens.md` has its `--clay-*` projection above, and
  the editor `StyleRegistry` keys are marked internal: CodeMirror.
