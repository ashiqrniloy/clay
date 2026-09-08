# Clay UI Component and Primitive Catalog

Single source of truth for reusable UI components and primitives. Update this file in the same change that adds/modifies/removes any entry. Rendering is React/frontend-primary (Tauri v2 client); the Rust server validates every package-facing contract (kind, style variables, tokens, payloads) at parse/install/theme-apply time. The native Masonry client was removed in Plan 097 Phase 12; historical reconciliation/interaction notes live in `docs/wiki/archive/`.

Status legend: **implemented** (usable now), **reserved** (name locked, validation rejects use until its phase), **planned** (approved for a future UI revamp phase), **internal** (Clay-native surface, not package-facing).

## Package-Facing Component Kinds

Declared in `src/shell/components.rs` (`ComponentKind`). Packages compose these; Clay renders them via `frontend/src/sdui/registry.tsx` and `frontend/src/packages/PackageWorkspace.tsx`. All emit inert command intents. Phase 20.4 made every implemented kind state-complete; interaction states derive from pointer/focus state and route through the active `ResolvedUiTheme` (React renders the equivalent states from host-resolved tokens).

| Kind | Status | Purpose | Notes |
|------|--------|---------|-------|
| `editorView` | implemented | Editor surface placed in a pane `main` slot | One content host per pane leaf; Phase 22.1/22.2: generic content hosts serving live per-pane document views (`PaneDocumentView` — independent `EditorSurface`, caret/selection/viewport, status line; mapping client-local, duplicate opens focus the existing pane); chrome CodeMirror-`StyleRegistry`-driven |
| `panel` | implemented | Container for slot content (`left`/`right`/`top`/`bottom`) | Fixed or transient; chrome via `paint_panel_chrome`; size user-configurable via slot state |
| `label` | implemented | Static text | Supports text font role; disabled → `text.disabled` × `opacity.disabled` |
| `button` | implemented | Action trigger | Variants: `default`, `muted`, `primary`, `danger`; fill via `component_state_color("surface.control", state)`; focus ring on `Focus`; disabled gates action |
| `list` | implemented | Row collection | Row items can carry title + detail text; row fill via `list_row_fill_color(state, selected)`; disabled rows gate action |
| `flex` | implemented | 1D layout container | Row/column with `gap` token; container, no chrome of its own |
| `stack` | implemented | Z-stacked container | Base for overlay compositions; container, no chrome of its own |
| `overlay` | implemented | Anchored floating layer | Anchor + dismissal + focus policy; chrome via `paint_tooltip_shell` |
| `scroll` | implemented | Scrollable region | Scrollbar chrome from `paint_scroll_chrome`; container, no body chrome |
| `portal` | implemented | Renders outside normal slot flow | For transient surfaces; container, no chrome of its own |
| `statusItem` | implemented | Status bar entry | Supports text font role; disabled → `text.disabled` × `opacity.disabled` |
| `dropdown` | implemented | Single-select drop-down | Phase 20.5: trigger row; `Role::ComboBox`; ArrowUp/Down cycles `selected_index`, Enter/Space confirms; open list painted by `PackageDropdown`; React `ClayDropdown` supports `groups` (plan 109 I3) |
| `collapse` | implemented | Expand/collapse section | Phase 20.5: title row with `clay.ui.collapseToggle` action; `Role::Group`; Enter/Space toggles `PackageCollapse.expanded`; content shown/hidden via a layout clip |
| `modal` | implemented | Blocking dialog | Phase 20.5: `paint_tooltip_shell` chrome (painted by the overlay host) + title + children; `Role::Dialog`; Tab focus-trap cycles the modal's widget-local focusable descendants; `z.modal` stacking |
| `textInput` | implemented | Single-line editable text field | Phase 20.5: bordered field, `text.muted` placeholder, focus ring, validation-state border (`diagnostic.error`/`warning`/`success` or `border.subtle`); `Role::TextInput`; `style.validationState`/`style.placeholderColor`; `multiline: true` → textarea substrate (plan 108 G3) |
| `tabList` | implemented | Tab strip hosting per-tab children | Plan 108 G2 / Plan 110 T5: unified `ClayTabStrip` (`frontend/src/components/tab-strip.tsx`) for shell tab bar, SDUI `PackageTabList`, `CodingAgentPanel`; closed recipe attributes (`tabList.root`/`strip`/`tab`/`panel`) |
| `table` | reserved | Tabular data | Deferred; no first-party package need identified as of Phase 20.5 |

### Phase 20.4 interaction-state and spacing rhythm notes

Phase 20.4 restyled the implemented kinds with Phase 20.1 tokens and Phase 20.2 primitives — no kind/style-variable/token-name change. States are derived per-widget from pointer/focus state (`is_disabled`/`is_active`/`is_focus_target`/`is_hovered`); precedence `Disabled` > `Active` > `Hover` > `Focus` > `Rest`.

- `button`: all five states; `Rest`=`surface.control`, `Hover`=`surface.hover`, `Active`=`surface.active`, `Focus`=`accent.primary` + `paint_focus_ring` (`border.focus`), `Disabled`=`surface.disabled`×`opacity.disabled` with `text.disabled` text and action gated.
- `list`: per-row states honor `selected` (`surface.selected` vs `surface.list`); `Hover`/`Active` override selection; `Disabled` dims and gates.
- `tabList`: per-tab five states; selected uses `surface.selected` + `accent.primary` underline; `Disabled` dims and gates.
- `label` / `statusItem`: `text.muted` at `Rest`; `Disabled` → `text.disabled`×`opacity.disabled`; `Focus` focus ring. No fill.
- `panel` / `overlay`: chrome via `paint_panel_chrome` / `paint_tooltip_shell` (state-independent; collapse/resize affordances route through the primitive, currently `Rest`).
- `editorView`: chrome is editor-`StyleRegistry`-driven; the scrollbar reflects `Hover`/`Active` via `paint_scroll_chrome`. No SDUI state-token fill.
- `flex` / `stack` / `scroll` / `portal`: containers — recurse children, no chrome of their own; `InteractionState` is not applicable.

**Spacing rhythm**: SDUI panel padding reads `spacing.md` (16) × `spacing_scale()` (`compact`=0.875 / `default`=1.0 / `spacious`=1.125). Status bar uses `spacing.sm` × `spacing_scale()` insets with a `border.hairline` top divider. Per-element `spacing.xs`/`sm`/`lg` differentiation is deferred to a later spacing pass.

**Active-theme routing**: component paint reads the active `ResolvedUiTheme` (`SduiThemeStyle::from_ui_theme`); `clay.contributions.designTokens` overrides flow through to component paint automatically.

### Phase 20.5 overlay, menu, and input component notes

New-kind states are in the kind table above (fill via `component_state_color("surface.control", state)`, open list from `items` with selection from `selected_index`, layout-clip toggle, focus trap, Escape → `PackageModalDismiss`, validation-state border, `style.validationState`/`style.placeholderColor`).

**Z-level stacking**: `z.overlay` (0) < `z.modal` (1) < `z.tooltip` (2) — completion `"z.overlay"`, modal `"z.modal"`, tooltip-anchored `"z.tooltip"`.

**Surface origins**: `TransientMenuOrigin` selects the host-owned anchor and focus policy. `CommandPalette` remains the bottom-anchored built-in menu; `ContextMenu`/`MenuBar` map to pointer/main package-compatible geometry. Plan 087 adds two Clay-owned origins — `Completion` (caret/IME-adjacent modeless picker) and `Centered` (window-level Command Centre/Path Browser) — both internal: packages keep only `working-area`, `active-pane`, `main`, `pointer`; `TransientPackageOverlay::from_menu_session` maps each internal origin without exposing it.

**Completion projection (Plan 087)**: shared retained menu renderer with a `scroll` child, caret-derived anchor clamped inside the active pane, **8 visible rows** max, **480 logical**-pixel width max; stale/empty/timeout/provider-error results never leave a blocking panel (timeout/provider-error surfaced through status diagnostics); Clay-owned anchor/focus policy; no package JavaScript in completion paint/layout/input paths.

**Centered Command Centre projection (Phase 24.4 / Plan 087)**: one Clay-owned window-level host, one token-driven scrim, width from `dimension.overlay.centered.width` (640 logical-pixel default), modal Dialog/Menu/Status accessibility, retained scrollable result list. Packages may register commands in the catalogue but cannot open, drive, configure, or intercept this surface.

## Plan 088 package UI/layout contract

Plan 088 consumes the existing catalog; it adds no `ComponentKind`, style variable, token, package overlay anchor, manifest field, permission, or JS API. The package authoring boundary remains declarative and inert:

- Package/SDUI hosts clip retained children to their owning bounds and expose clipped-child semantics to accessibility consumers; a package `scroll` child receives flex sizing inside bounded panel/container compositions.
- `modal` Escape emits `PackageModalDismiss` with the declared inert command intent — no modal or native-widget authority; `statusItem` and disabled controls expose their state.
- Fixed slots remain Clay-owned (`main` + optional `left`/`right`/`top`/`bottom`). The workspace browser may yield its left slot when pane width or user UI typography would leave the main editor unusable. The shell-owned tab bar follows active UI typography and logical window bounds; packages cannot own tabs, panes, the file browser, status chrome, the core `WelcomeWidget` fallback, completion, or centered Command Centre. The loaded empty-tab landing is package pane-content (`@clay/chat` by default).
- Responsive clamping, label clipping, path sanitization, focus containment, and theme/typography propagation are host responsibilities. Packages select semantic typography and typed tokens; they cannot supply breakpoints, concrete sizes/fonts, raw CSS/colors, native widgets, renderer callbacks, client JavaScript, or direct renderer mutation.
- `@clay/settings` demonstrates the composition contract (`panel` + `scroll` + existing controls). `table` remains the only reserved package kind; package overlays stay limited to `working-area`, `active-pane`, `main`, `pointer`; `completion` and `centered` are Clay-internal origins.

Complete package-facing explanation and validation/test commands: [Creating Clay Packages — Plan 088 UI modernization authoring contract](../../../../docs/reference/packages/creating-packages.md#plan-088-ui-modernization-authoring-contract). Host-owned layout and accessibility guarantees, not new package APIs.

## Plan 097 React renderer mapping (client cutover)

The Tauri client projects the unchanged catalog through `frontend/src/sdui/registry.tsx` and `frontend/src/packages/PackageWorkspace.tsx`; every implemented kind maps to the React target in `docs/development/react-ui-catalog-mapping.md` (`table` rejected). Stable node IDs are React keys, preserving local input/disclosure/dropdown/focus/scroll state across unrelated updates.

Rust still validates kind, tree bounds, styles, actions, package provenance, generation, and trust domain before Tauri parses component JSON into a typed DTO. React receives inert values only, resolves typed token names to host-owned `--clay-*` variables, and sends one versioned `SduiAction` for interaction. No package JSX, event callback, CSS, script, V8 object, raw op, or Tauri API enters the main webview. Renderer cutover only — no kind, style variable, token, permission, anchor, or manifest shape changed.

`frontend/src/command-centre/CommandCentre.tsx` projects server-owned command/path/picker sessions through the Command Centre surface (React Aria `Modal`/`Dialog`, `TextField`, `ListBox`; results capped at 256). `frontend/src/settings/SettingsPanel.tsx` is the compiled trusted presentation for `@clay/settings`: existing primitives only, host-resolved tokens, versioned `settings.*` intents.

## Phase 28 editor-intelligence chrome

Phase 28 adds no package-facing UI kind. Packages publish inert editor data; Clay owns presentation and interaction:

- Fold ranges: validated background data; Clay paints `paint_gutter` chevrons, owns the collapsed set, hides interior lines, routes `editor.clientToggleFold` (chevrons not tab stops).
- Links: existing `TokenType::Link`/underline vocabulary; Clay hit-tests, paints hover with `paint_tooltip_shell`, routes activation through typed decoration intent; no package link chrome or pointer callbacks.
- Inlays: existing decoration transport, inert bounded label, `Before`/`After` placement; muted post-layout text, decorative/`aria-hidden`, toggled by `editor.toggleInlayHints`.
- No package JavaScript in paint/layout/pointer/scroll/keypress/text-event paths; no new `ComponentKind`, token, style variable, or native widget.

## Typed Style Variables

Validated in `src/shell/components.rs`. Token-backed variables must reference a known token of the matching type; raw colors/CSS are rejected.

| Variable | Token type / enum | Applies to |
|----------|-------------------|------------|
| `background` | color-role token | Surfaces |
| `contentColor` | color-role token | Foreground content |
| `borderColor` | color-role token | Borders/dividers |
| `accentColor` | color-role token | Accents, focus |
| `padding` | spacing token | Inner spacing |
| `gap` | spacing token | Sibling spacing in `flex` |
| `rowHeight` | spacing token | `list` rows |
| `inset` | spacing token | Overlay offset |
| `radius` | radius token | Corner radius |
| `typography` | typography token | Text hierarchy level (`typography.body`, `typography.title`, `typography.status`, `typography.display`, `typography.section`, `typography.detail`, `typography.caption`) |
| `opacity` | opacity token | Disabled/muted states |
| `fontRole` | enum: `ui`, `monospace`, `proportional` | Text components |
| `variant` | enum: `default`, `muted`, `primary`, `danger` | `button`, emphasis |
| `placeholderColor` | color-role token | `textInput` placeholder text (Phase 20.5) |
| `validationState` | enum: `none`, `error`, `warning`, `success` | `textInput` border state (Phase 20.5) |

## Clay-Native Surfaces (internal)

| Surface | Status | File | Purpose |
|---------|--------|------|---------|
| Shell root / workspace chrome | internal | `frontend/src/app/layout/` | Window chrome, working area, tab bar |
| Pane split tree | internal | `src/shell/layout/` | Horizontal/vertical splits, ratio 0.05–0.95 |
| Fixed panel slots | internal | `src/shell/layout/` | `left`/`right`/`top`/`bottom` with size/min/max/visible/collapsed/resized_by_user; workspace browser can be absent entirely (per-tab visibility flag) and yields its left slot when pane width or user UI typography makes the main region unusable |
| Status bar | internal | frontend shell chrome | `statusBg`/`statusText` theme keys |
| Welcome entry surface | internal | frontend welcome component | Plan 087 Clay-owned empty/local-fallback when no `empty-tab` pane-content is loaded. Sanitized workspace/status copy, `Open File`/`Open Folder` via existing client commands, Group/Status accessibility, no package-facing replacement of this fallback or dialog authority. Loaded landing is package pane-content (`@clay/chat`) |
| Transient menu | internal | `src/shell/transient_menu.rs` | Bounded prompt/item list scored by the shared fuzzy matcher (`src/shell/fuzzy.rs`), focus policy, package provenance; server-owned Control Center/Path Browser sessions plus Plan 087/Phase 24.4 hosts |
| Inline completion pop-up | internal | `src/shell/transient_menu.rs` | Plan 087 `TransientMenuOrigin::Completion` projection: caret/IME anchor, modeless selection, `scroll` composition, 8 visible-row cap, 480 logical-pixel width cap, stale/empty/error dismissal |
| Fixed package panels | internal | `src/shell/package_ui.rs` | Slot-bound package panels with visibility |
| Transient package overlays | internal | `src/shell/package_ui.rs` | Package-declared overlays using only `working-area`, `active-pane`, `main`, `pointer`; centered and completion hosts are Clay-internal |
| File browser | internal | `src/shell/file_browser.rs` | Workspace/selected-file browsing; per-tab left panel with sanitized workspace name + workspace-relative location |
| Editor chrome | internal | CodeMirror adapter + `src/editor/` | Caret, selection, scrollbar, diagnostics, gutter/active-line/indent-guide/bracket-match paint; `editorRules.chrome`/`document_font_role` toggles; colors from `StyleRegistry` chrome keys, never SDUI tokens. Wrap (`none`/`viewport`/`column`), asymmetric insets, fold/link/inlay chrome all Clay-owned (see Phase 28 section). No new package-facing `ComponentKind`, token, style variable, or paint callback |

## Clay-Native Chrome Primitives (internal)

Phase 20.2 introduced a chrome primitive layer — pre-cutover the `pub(crate)` paint helpers in `src/shell/primitives.rs` (removed with the native client); the same catalog contract is now realized as token-driven React components/CSS classes in `frontend/src/components/chrome.tsx` plus state-color helpers (`component_state_color`, `list_row_fill_color`, `disabled_text_color`). Packages cannot call primitives directly; package-declared kinds map onto primitives by construction. Full token/a11y detail: [UI Chrome Primitives](../../../../docs/reference/primitives/ui-chrome-primitives.md).

| Primitive | Status | Purpose | Token mapping | Accessibility role |
|-----------|--------|---------|---------------|-------------------|
| `paint_divider` | internal | Horizontal/vertical separator | `border.hairline`, `dimension.border.hairline` | `separator` |
| `paint_focus_ring` | internal | Focus indicator ring | `border.focus`, `dimension.border.thin`, `radius.xs` | Applied to focused element |
| `paint_panel_chrome` | internal | Panel background/border with optional title/collapse/resize | `surface.panel`, `border.subtle`, `dimension.border.hairline`, `radius.sm`, `spacing.panel` | `region` or `complementary` |
| `paint_scroll_chrome` | internal | Scrollbar track/thumb with interaction states | `surface.scrollbar.track`, `surface.scrollbar`, `dimension.scrollbar.width`, `radius.xs` | `scrollbar` |
| `paint_badge` | internal | Badge/tag with label and interaction states | `surface.badge`, `text.badge`, `radius.xs`, `spacing.badge`, `typography.detail`/`caption` | `status` or `note` |
| `paint_kbd_hint` | internal | Keyboard shortcut hint | `surface.kbd`, `text.kbd`, `border.kbd`, `radius.xs`, `dimension.kbd.height`, `typography.caption` | `kbd` (via label) |
| `paint_icon_slot` | internal | Standardized icon placeholder | `dimension.icon.size`, `text.icon`, `opacity.disabled` | `img` or `presentation` |
| `paint_tooltip_shell` | internal | Tooltip background/border | `surface.tooltip`, `text.tooltip`, `border.hairline`, `dimension.border.hairline`, `radius.sm`, `elevation.overlay`, `z.tooltip`, `spacing.tooltip`, `typography.body` | `tooltip` |
| `paint_scrim` | internal | Full-window dim behind the centered Command Centre surface (Phase 24.4) | `surface.scrim`, `opacity.scrim` | `dialog` backdrop (Clay-internal; no package-facing surface) |

All primitives: read color/dimension/opacity/typography from `ResolvedUiTheme` tokens (no hardcoded values); render all declared `InteractionState` variants; apply `opacity.disabled` for `Disabled`; deterministic and allocation-free in paint paths. React equivalents keep the same token mapping via host CSS custom properties.

Conformance contract (enforced by `tests/package_ui_conformance.rs` and frontend consumption tests):

- **Contrast / legibility:** active-theme status-chrome pairs must meet `TEXT_CONTRAST_MIN` (4.5) and `UI_CONTRAST_MIN` (3.0); a below-AA theme is not activated (`validate_active_theme_contrast`, `src/shell/theme.rs`; `enforce_contrast`, `src/server/ops/theme.rs`).
- **State-completeness:** `applicable_states(kind)` (`src/shell/components.rs`) is the per-kind interaction-state contract.
- **Payload budgets:** SDUI snapshot ≤ 4096 B, update ≤ 1024 B; runtime tree ≤ 16 KiB / ≤ 128 nodes / ≤ 16 depth / ≤ 4096-char text node (`src/packages/record/mod.rs`, `src/server/ui.rs`, `src/server/ops/sdui.rs`).
- **Code-vs-catalog drift:** `ComponentKind` enum ↔ the `Package-Facing Component Kinds` table; typed-style-variable match arms ↔ the `Typed Style Variables` table; `core_theme_value` arms ↔ `tokens.md` Core Tokens. Drift guards live in `tests/package_ui_conformance.rs`.
- **Trust domains:** no `clay.ui.validate*` op or `clay:*` facade exposes conformance; third-party raw values and oversized payloads are rejected at `assemble_package_record` without reaching the trusted runtime.
- **Consumption test:** every shipped design-system recipe key must be consumed by host CSS — `frontend/src/test/design-system-consumption.test.ts` fails when a `tokens.css` fallback variable has no CSS consumer or a CSS recipe variable has no shipped key.

Conformance is host authority, not package-facing: validation runs inside Clay's Rust host validator at parse/install/theme-apply time. Plan 087 keeps the package contract additive-only (no new kind/token/style variable/anchor/manifest field/JS API); transient-menu accessibility labels pass one bounded host normalization step (control characters/path separators removed, empty labels get a safe fallback, selected-state suffixes stay inside the 256-character ceiling).

## Planned Components (UI Revamp Phases 20.2/20.5)

Reuse-first: before adding any of these, confirm no implemented kind composes to the same result. Each planned entry must ship token-driven, state-complete (hover/active/focus/disabled), accessible, and cataloged here.

| Component | Status | Purpose | Composition notes |
|-----------|--------|---------|-------------------|
| Pop-up / dialog | implemented | Non-blocking anchored pop-up and blocking dialog | Phase 20.5: `overlay`+`portal` composition; `modal` kind for blocking; z-level stacking |
| Dropdown / select | implemented | Single-choice selection | Phase 20.5: `dropdown` kind; keyboard nav via `selected_index` |
| Multi-select | implemented | Multi-choice selection with tags | Phase 20.5: `list`+`checkbox` composition; no new kind |

| Text input field | implemented | Single-line editable field | Phase 20.5: `textInput` kind; focus, placeholder, validation states |
| Menu (context / menu bar) | implemented | Command menus | Phase 20.5: `TransientMenuOrigin::ContextMenu`/`MenuBar`; anchor selected by origin |
| Completion pop-up (uplift) | implemented | Clay-internal caret-adjacent completion projection | Plan 087: `TransientMenuOrigin::Completion`, IME/caret anchor, modeless focus, retained `scroll`, 8 visible rows, 480 logical-pixel cap, stale/empty/error dismissal. Not a package overlay anchor or kind |
| Command palette | implemented | Clay-owned Command Centre surface | Phase 24.2/24.4: `controlCenter.open`, generation-stamped live command catalogue with keybindings/provenance, centered 640-logical-pixel default width, retained scroll; centered origin Clay-internal, packages cannot open or drive it |
| Tooltip | implemented | Hover hint | Plan 112: React `ClayTooltip` over React Aria `TooltipTrigger`/`Tooltip` (`frontend/src/components/tooltip.tsx`); hover + keyboard-focus trigger, Escape dismiss, `role="tooltip"` + `aria-describedby`; consumes `tooltip.default.root.rest` recipe; host-owned string content only |
| Tabs | implemented | Tab strip (window tab bar) | Phase 22.3 / plan 110 task 5: unified `ClayTabStrip` (`frontend/src/components/tab-strip.tsx`); cards shrink-to-fit until `TAB_BAR_CARD_MIN_WIDTH` (100px) binds, then the strip scrolls; activation auto-scrolls the active card into view; generic paint contract for later reuse (panel/pane tabs); shell-level, not package-facing |
| Split divider | implemented | Draggable pane/slot separator | Phase 20.3: `paint_divider` + drag interaction on the pane split tree; resize handles via `paint_panel_chrome` |
| Badge / tag | planned | Status/count marker | `label` + muted pastel tokens |
| Toast / notification | planned | Transient feedback | `overlay` + portal, auto-dismiss |
| `kbd` hint | planned | Shortcut rendering | `label` with monospace role + bordered token style |
| Icon slot | implemented | Standardized icon placeholder | Plan 112: React `ClayIcon` (`frontend/src/components/icon.tsx`) renders bounded host-validated vector geometry from the active icon pack (`frontend/src/state/icon-store.ts`) with the bundled Regular subset fallback; sized via `dimension.icon.size`, colored by `text.icon`/`currentColor`, `aria-hidden` by default or `role="img"` with a label; `ClayIconButton` composes icon + required accessible label + tooltip + ≥24px hit target (WCAG 2.2) |

## Typography Variants (Phase 20.1)

The `typography` style variable references one of seven semantic `UiTextVariant` tokens (see [tokens.md](tokens.md#typography)): `body`, `title`, `status`, `display`, `section`, `detail`, `caption`. Variants are scale ratios over the selected role base, resolved through the user-owned `UiTypographyHierarchy` — never absolute point sizes. Phase 20.1 added `display`, `section`, `detail`, `caption` additively; existing `body`/`title`/`status` usage is unchanged.

## UI Design-System Recipe Slots (Plan 101)

Package contributions (`clay.contributions.uiDesignSystem`) style host components by targeting stable, semantic slot names. All visual colors resolve from the active content theme; recipes define geometry, materials, states, and motion without injecting raw CSS. The full slot/state/fallback matrix is authoritative in [`docs/development/ui-design-system-recipe-matrix.md`](../../../../docs/development/ui-design-system-recipe-matrix.md); this section is a compact surface→slots index.

| Surface | Recipe slots |
| --- | --- |
| `button` | `root`, `label`, `icon` (variants `default`/`muted`/`primary`/`danger`) |
| `textInput` | `field`, `label`, `input`, `description`, `error` (validation `none`/`error`/`warning`/`success`) |
| `dropdown` | `trigger`, `triggerLabel`, `indicator`, `popover`, `list`, `item`, `itemLabel` |
| `list` | `root`, `row`, `rowTitle`, `rowDetail` |
| `collapse` | `root`, `header`, `title`, `chevron`, `body` |
| `modal` | `scrim`, `dialog`, `title`, `body` — z-modal stacking, focus trap, Escape dismissal |
| `panel` | `root`, `header`, `title`, `body` — fixed and transient panel slots |
| `label` / `statusItem` | `root` — semantic typography variant scales |
| `flex` / `stack` / `overlay` / `portal` | `root` — layout and layer containers |
| `scroll` | `root`, `scrollbarTrack`, `scrollbarThumb` |
| `editorView` | `root`, `canvas` — host editor pane container and CodeMirror view |
| `tab` / `tabBar` | `item` (states `rest`/`hover`/`selected`/`focus`/`disabled`) + `root` |
| `paneSplitTree` | `group`, `pane`, `handle`, `indicator` |
| `statusBar` | `root`, `item` |
| `commandCentre` | `scrim`, `dialog`, `input`, `listBox`, `item`, `status`, `empty` |
| `fileBrowser` | `root`, `header`, `tree`, `item`, `itemIcon`, `itemLabel` |
| `settingsPanel` | `panel`, `heading`, `actions`, `fields` |
| `chatPanel` | `root`, `header`, `transcript`, `userMessage`, `assistantMessage`, `thinking`, `composer`, `statusLine` |
| `shell` / `editor` / `chat` / `menu` / `card` / `popover` | See matrix — plan 110 task 6/8 package recipe kinds; 142 recipes per reference package; core fallbacks stay at `<surface>.default.root.rest` |
| `badge` / `kbd` / `divider` / `tooltip` / `scrim` / `focusRing` / `scrollChrome` / `iconSlot` | `root`, `label`, `ring`, `track`, `thumb`, `content` — chrome primitives |

Canonicalization (plan 110 task 3): package slot for `modal` is `modal.default.dialog.*` (the `modal.default.root.*` alias was deleted); for tabs it is `tab.default.item.*` (was `tab.default.root.*`; the `bar`/`card`/`cardLabel`/`closeButton`/`dirtyIndicator` package slots were removed). Host component: `ClayTabStrip`.

**Consumption-tested contract (plan 110 task 3):** every shipped recipe key must be consumed by host CSS — `frontend/src/test/design-system-consumption.test.ts` fails when a `tokens.css` fallback variable has no CSS consumer or a CSS recipe variable has no shipped key. The matrix and this index are the source of truth for slot names; packages shipping unconsumed or misspelled keys are drift, not extensibility.

## Rules for Adding Components

1. Prefer composing existing kinds (`flex`, `stack`, `overlay`, `scroll`, `list`, `label`, `button`) before adding a kind.
2. New kinds are additive; never rename or remove an implemented kind.
3. New style variables must be token-typed or closed enums — no raw values.
4. Every component ships with all interaction states styled from tokens.
5. Update this catalog, `docs/reference/packages/creating-packages.md`, and the component validation tests together.