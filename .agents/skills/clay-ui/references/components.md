# Clay UI Component and Primitive Catalog

Single source of truth for reusable UI components and primitives. Update this file in the same change that adds/modifies/removes any entry.

Status legend: **implemented** (usable now), **reserved** (name locked, validation rejects use until its phase), **planned** (approved for a future UI revamp phase), **internal** (Clay-native surface, not package-facing).

## Package-Facing Component Kinds

Declared in `src/shell/components.rs` (`ComponentKind`). Packages compose these; Clay renders them. All emit inert command intents.

| Kind | Status | Purpose | Notes |
|------|--------|---------|-------|
| `editorView` | implemented | Editor surface placed in a pane `main` slot | One editor component binding per working area |
| `panel` | implemented | Container for slot content (`left`/`right`/`top`/`bottom`) | Fixed or transient; size user-configurable via slot state |
| `label` | implemented | Static text | Supports text font role |
| `button` | implemented | Action trigger | Variants: `default`, `muted`, `primary`, `danger` |
| `list` | implemented | Row collection | Row items can carry title + detail text |
| `flex` | implemented | 1D layout container | Row/column with `gap` token |
| `stack` | implemented | Z-stacked container | Base for overlay compositions |
| `overlay` | implemented | Anchored floating layer | Anchor + dismissal + focus policy |
| `scroll` | implemented | Scrollable region | Scrollbar chrome from tokens |
| `portal` | implemented | Renders outside normal slot flow | For transient surfaces |
| `statusItem` | implemented | Status bar entry | Supports text font role |
| `table` | reserved | Tabular data | Deferred; reserved for a later catalog phase |
| `dropdown` | reserved | Single-select drop-down | Planned in UI revamp (Phase 20.5) |
| `collapse` | reserved | Expand/collapse section | Planned in UI revamp (Phase 20.5) |
| `modal` | reserved | Blocking dialog | Planned in UI revamp (Phase 20.5) |

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
| `typography` | typography token | Text hierarchy level |
| `opacity` | opacity token | Disabled/muted states |
| `fontRole` | enum: `ui`, `monospace`, `proportional` | Text components |
| `variant` | enum: `default`, `muted`, `primary`, `danger` | `button`, emphasis |

## Clay-Native Surfaces (internal)

| Surface | Status | File | Purpose |
|---------|--------|------|---------|
| Shell root widget | internal | `src/masonry_shell.rs` | `ClayShellWidget`, owns working area above editor |
| Pane split tree | internal | `src/shell/layout.rs` | Horizontal/vertical splits, ratio 0.05–0.95 |
| Fixed panel slots | internal | `src/shell/layout.rs` | `left`/`right`/`top`/`bottom` with size/min/max/visible/collapsed/resized_by_user |
| Status bar | internal | editor/shell paint | Uses `statusBg`/`statusText` theme keys |
| Transient menu | internal | `src/shell/transient_menu.rs` | Bottom-pane prompt + filtered item list, focus policy, package provenance |
| Inline completion pop-up | internal | `src/shell/transient_menu.rs` | Completion results rendered through the transient menu session (`completion_result_to_menu_session`) |
| Fixed package panels | internal | `src/shell/package_ui.rs` | Slot-bound package panels with visibility |
| Transient package overlays | internal | `src/shell/package_ui.rs` | Anchored overlays (`PackageOverlayAnchor`) |
| File browser | internal | `src/shell/file_browser.rs` | Workspace/selected-file browsing surface |
| Editor chrome | internal | `src/editor/surface.rs` | Caret, selection, scrollbar, diagnostics paint |

## Planned Components (UI Revamp Phases 20.2/20.5)

Reuse-first: before adding any of these, confirm no implemented kind composes to the same result. Each planned entry must ship token-driven, state-complete (hover/active/focus/disabled), accessible, and cataloged here.

| Component | Status | Purpose | Composition notes |
|-----------|--------|---------|-------------------|
| Pop-up / dialog | planned | Non-blocking anchored pop-up and blocking dialog | Build on `overlay` + `portal`; `modal` kind for blocking |
| Dropdown / select | planned | Single-choice selection | Reserved kind `dropdown`; list-in-overlay composition |
| Multi-select | planned | Multi-choice selection with tags | `dropdown` variant + badge primitive |
| Text input field | planned | Single-line editable field | Prompt line of transient menu is the interim input; needs focus, placeholder, validation states |
| Menu (context / menu bar) | planned | Command menus | Generalize transient menu session beyond bottom pane |
| Completion pop-up (uplift) | planned | Inline completion restyle | Existing session; uplift rendering to shared overlay primitive |
| Command palette | planned | Command Centre surface | Transient menu session with command provenance |
| Tooltip | planned | Hover hint | `overlay` anchored, `detail` typography |
| Tabs | planned | Pane/panel tab strip | Shell-level, not package-facing initially |
| Split divider | planned | Draggable pane/slot separator | Shell layout primitive with resize intents |
| Badge / tag | planned | Status/count marker | `label` + muted pastel tokens |
| Toast / notification | planned | Transient feedback | `overlay` + portal, auto-dismiss |
| `kbd` hint | planned | Shortcut rendering | `label` with monospace role + bordered token style |
| Icon slot | planned | Standardized icon placeholder | Token-sized glyph slot; no package image assets initially |

## Rules for Adding Components

1. Prefer composing existing kinds (`flex`, `stack`, `overlay`, `scroll`, `list`, `label`, `button`) before adding a kind.
2. New kinds are additive; never rename or remove an implemented kind.
3. New style variables must be token-typed or closed enums — no raw values.
4. Every component ships with all interaction states styled from tokens.
5. Update this catalog, `docs/reference/packages/creating-packages.md`, and the component validation tests together.
