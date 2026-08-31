# UI Design-System Recipe Matrix

Decision source: `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`.
Pattern reference: `.agents/skills/project-patterns/references/ui-design-system-packages.md`.

## Overview and Authority Invariants

This matrix defines the stable, versioned contract between host-owned React components, semantic slots, interaction states, and package-defined UI design-system recipes.

### Core Invariants

1. **Content Themes as Sole Color Authority:** Normal-rendering colors for surfaces, text, borders, focus rings, selections, diagnostics, overlays, and solid material fallbacks resolve exclusively from the active content theme via semantic color-role references (e.g., `surface.control`, `text.primary`, `border.focus`). Design systems may choose which theme color role maps to a slot/state, but they may never declare palettes, literal colors (`#hex`, `rgb`, `hsl`, named colors), or independent color values.
2. **Forced-Colors Accessibility Exception:** Browser/OS system colors (`Canvas`, `CanvasText`, `Highlight`, `HighlightText`, `ButtonFace`, `ButtonText`) are applied only under `forced-colors: active` media queries.
3. **Inert Declarative Schema:** Design systems are declarative data contributions (`clay.contributions.uiDesignSystem`). Packages cannot inject raw CSS strings, CSS selectors, URLs, network assets, arbitrary filter/transform pipelines, JSX, renderer callbacks, client scripts, or direct Tauri APIs into the webview.
4. **Deterministic Fallback:** Any property or state omitted by a package design system resolves deterministically through Clay's core fallback recipes. A package recipe cannot produce undefined or missing styling.
5. **Separation of Concerns:**
   - **Content Themes:** Own color values, contrast pairs, and syntax highlights.
   - **Typography Hierarchy:** Owns user-configured font family stacks and scale ratios.
   - **UI Design Systems:** Own geometry (radii, border widths), materials (opacity, blur, shadows), state mappings, and motion timing.
   - **React Aria & Clay Core:** Own accessibility semantics, keyboard navigation, focus management, ARIA roles/states, and intent dispatch.

---

## Property Families and Domain Bounds

Properties in recipes are grouped into closed families. Each property is classified as **Layout-Neutral** (safe to animate or change without container reflow) or **Layout-Affecting** (bounded strictly to prevent layout jitter).

| Property Family | Type / Domain | Valid Range / Allowed Values | Layout Impact | Fallback / Default |
| --- | --- | --- | --- | --- |
| `backgroundColor` | Theme color-role ref | Known theme role (e.g. `surface.control`, `surface.panel`, `surface.main`, `accent.primary`, `transparent`) | Layout-neutral | Slot-defined theme role |
| `backgroundOpacity` | Finite float | `[0.0, 1.0]` | Layout-neutral | `1.0` (or `opacity.scrim` for scrims) |
| `textColor` | Theme color-role ref | Known theme role (e.g. `text.primary`, `text.muted`, `text.disabled`, `text.badge`, `text.kbd`, `text.tooltip`) | Layout-neutral | Slot-defined theme role |
| `borderColor` | Theme color-role ref | Known theme role (e.g. `border.hairline`, `border.subtle`, `border.strong`, `border.focus`, `transparent`) | Layout-neutral | Slot-defined theme role |
| `borderWidth` | Dimension (px) | Integer `[0, 8]` px | Layout-affecting | `1` px (`dimension.border.hairline`) |
| `borderStyle` | Enum | `none`, `solid`, `dashed`, `dotted` | Layout-neutral | `solid` |
| `borderRadius` | Radius (px / enum) | Integer `[0, 32]` px or `9999` (full pill) | Layout-neutral | `0` (Neobrutal) / `4` (Soft) |
| `padding` | Spacing token ref | Known spacing token (`spacing.none`, `spacing.xxs`, `spacing.xs`, `spacing.sm`, `spacing.md`, `spacing.lg`, `spacing.panel`, `spacing.badge`, `spacing.tooltip`) | Layout-affecting | Slot-defined spacing token |
| `gap` | Spacing token ref | Known spacing token (`spacing.none`, `spacing.xxs`, `spacing.xs`, `spacing.sm`, `spacing.md`, `spacing.inline`) | Layout-affecting | Slot-defined spacing token |
| `minHeight` / `minWidth` | Dimension (px) | Integer `[0, 8192]` px | Layout-affecting | Slot-defined dimension |
| `shadow` | Structured shadow | Max 3 layers: `{ x: [-32..32], y: [-32..32], blur: [0..64], spread: [-16..16], colorRole: ThemeRole, opacity: [0..1], inset: bool }` | Layout-neutral | `none` |
| `backdropBlur` | Dimension (px) | Integer `[0, 32]` px (disabled under `prefers-reduced-transparency`) | Layout-neutral | `0` px |
| `backdropSaturate` | Finite float | `[1.0, 2.0]` (disabled under `prefers-reduced-transparency`) | Layout-neutral | `1.0` |
| `innerHighlight` | Structured rim highlight | `{ colorRole: ThemeRole, opacity: [0..1], width: [1..4] }` | Layout-neutral | `none` |
| `opacity` | Opacity token / float | Finite `[0.0, 1.0]` | Layout-neutral | `1.0` (or `opacity.disabled` for disabled) |
| `outlineColor` | Theme color-role ref | Known theme role (`focus.ring`, `border.focus`) | Layout-neutral | `focus.ring` |
| `outlineWidth` | Dimension (px) | Integer `[0, 4]` px | Layout-neutral | `2` px |
| `outlineOffset` | Dimension (px) | Integer `[-4, 4]` px | Layout-neutral | `1` px |
| `outlineStyle` | Enum | `none`, `solid` | Layout-neutral | `solid` |
| `transitionDuration` | Motion token ref | Known motion token (`motion.instant`, `motion.fast`, `motion.normal`, `motion.slow`) | Layout-neutral | `motion.fast` (100ms) |
| `transitionTiming` | Enum | `linear`, `ease-out`, `spring-snappy`, `spring-smooth` | Layout-neutral | `linear` |
| `transformPreset` | Enum | `none`, `press-subtle` (scale 0.98), `press-shift-down` (translate 1px 1px), `hover-lift` (translateY -1px) | Layout-neutral | `none` |

---

## Component & Surface Recipe Matrix

### 1. Package-Facing Component Kinds (`src/shell/components.rs`)

| Component Kind | Slot | Variants | Applicable States | React Owner / Headless Primitive | React Aria State Mapping | Allowed Property Families | Layout Impact | Active-Theme Color Role Source | Required Fallback Recipe ID | Accessibility Invariant |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `button` | `root` | `default`, `muted`, `primary`, `danger` | `rest`, `hover`, `active`, `focus`, `disabled` | `ClayButton` (`frontend/src/components/button.tsx`) / React Aria `Button` | `[data-hovered]`, `[data-pressed]`, `[data-focused]`, `[data-disabled]` | Geometry, Material, Borders, Outlines, Motion | Mixed | `surface.control`, `surface.hover`, `surface.active`, `surface.disabled`, `accent.primary`, `focus.ring` | `core.button.<variant>.root` | Focus outline retained in focus state; `aria-disabled` gates action |
| `button` | `label` | `default`, `muted`, `primary`, `danger` | `rest`, `hover`, `active`, `focus`, `disabled` | `ClayButton` text child | Same as root | Foreground (textColor, opacity) | Layout-neutral | `text.primary`, `text.muted`, `text.disabled`, `surface.main`, `diagnostic.error` | `core.button.<variant>.label` | WCAG AA contrast against button root background |
| `button` | `icon` | `default`, `muted`, `primary`, `danger` | `rest`, `hover`, `active`, `focus`, `disabled` | `ClayButton` icon child / nested island | Same as root | Geometry, Material, Foreground | Mixed | `text.icon`, `text.primary`, `text.muted`, `text.disabled` | `core.button.<variant>.icon` | Scaled to `dimension.icon.size` |
| `textInput` | `field` | `default` | `rest`, `disabled` | `ClayTextField` (`frontend/src/components/text-field.tsx`) / React Aria `TextField` | `[data-disabled]` | Geometry (gap), Layout | Layout-affecting | None (layout wrapper) | `core.textInput.field` | Groups label, input, description, and error message |
| `textInput` | `label` | `default` | `rest`, `disabled` | `ClayTextField` `<Label>` | `[data-disabled]` | Foreground (textColor, opacity), Typography | Layout-neutral | `text.muted`, `text.disabled` | `core.textInput.label` | Accessible label wired via `aria-labelledby` |
| `textInput` | `input` | `default` (states: `none`, `error`, `warning`, `success`) | `rest`, `hover`, `focus`, `disabled`, `invalid` | `ClayTextField` `<Input>` | `[data-focused]`, `[data-invalid]`, `[data-disabled]` | Geometry, Material, Borders, Outlines | Mixed | `surface.control`, `surface.disabled`, `border.subtle`, `border.focus`, `diagnostic.error`, `diagnostic.warning`, `diagnostic.success`, `text.primary`, `text.muted` (placeholder) | `core.textInput.input` | `aria-invalid` set on validation error; focus ring visible |
| `textInput` | `description` | `default` | `rest` | `ClayTextField` `<Text slot="description">` | None | Foreground (textColor), Typography | Layout-neutral | `text.muted` | `core.textInput.description` | Linked via `aria-describedby` |
| `textInput` | `error` | `default` | `rest` (when invalid) | `ClayTextField` `<FieldError>` | `[data-invalid]` | Foreground (textColor), Typography | Layout-neutral | `diagnostic.error` | `core.textInput.error` | Linked via `aria-describedby` |
| `dropdown` | `trigger` | `default` | `rest`, `hover`, `active`, `focus`, `disabled`, `open` | `ClayDropdown` (`frontend/src/components/controls.tsx`) / React Aria `Select` | `[data-hovered]`, `[data-pressed]`, `[data-focused]`, `[data-disabled]`, `[data-open]` | Geometry, Material, Borders, Outlines, Motion | Mixed | `surface.control`, `surface.hover`, `surface.active`, `surface.disabled`, `border.subtle`, `focus.ring` | `core.dropdown.trigger` | Button trigger role; ArrowDown/Up opens menu |
| `dropdown` | `triggerLabel` | `default` | `rest`, `disabled` | `ClayDropdown` `<SelectValue>` | `[data-disabled]` | Foreground (textColor, opacity) | Layout-neutral | `text.primary`, `text.disabled` | `core.dropdown.triggerLabel` | Reflects selected item label |
| `dropdown` | `indicator` | `default` | `rest`, `hover`, `open`, `disabled` | `ClayDropdown` chevron icon | `[data-open]`, `[data-disabled]` | Foreground, Motion (rotation) | Layout-neutral | `text.muted`, `text.primary` | `core.dropdown.indicator` | `aria-hidden` decorative indicator |
| `dropdown` | `popover` | `default` | `rest` | `ClayDropdown` `<Popover>` | `[data-open]` | Geometry, Material, Borders, Shadows | Mixed | `surface.overlay`, `border.hairline` | `core.dropdown.popover` | Non-modal popover; anchored to trigger; focus contained |
| `dropdown` | `list` | `default` | `rest` | `ClayDropdown` `<ListBox>` | None | Geometry (padding), Material | Layout-affecting | `surface.overlay` | `core.dropdown.list` | `role="listbox"`; arrow key navigation |
| `dropdown` | `item` | `default` | `rest`, `hover`, `active`, `focus`, `selected`, `disabled` | `ClayDropdown` `<ListBoxItem>` | `[data-hovered]`, `[data-pressed]`, `[data-focused]`, `[data-selected]`, `[data-disabled]` | Geometry, Material, Borders, Outlines | Mixed | `surface.list`, `surface.hover`, `surface.active`, `surface.selected`, `text.primary`, `text.disabled`, `focus.ring` | `core.dropdown.item` | `role="option"`; `aria-selected` set when selected |
| `list` | `root` | `default` | `rest` | `ClayList` (`frontend/src/components/controls.tsx`) / React Aria `ListBox` | None | Geometry, Material, Borders | Mixed | `surface.list`, `border.hairline` | `core.list.root` | `role="listbox"`; roving tabindex |
| `list` | `row` | `default` | `rest`, `hover`, `active`, `focus`, `selected`, `disabled` | `ClayList` `<ListBoxItem>` | `[data-hovered]`, `[data-pressed]`, `[data-focused]`, `[data-selected]`, `[data-disabled]` | Geometry, Material, Borders, Outlines | Mixed | `surface.list`, `surface.hover`, `surface.active`, `surface.selected`, `border.hairline`, `focus.ring` | `core.list.row` | `role="option"`; `aria-selected` set |
| `list` | `rowTitle` | `default` | `rest`, `disabled` | `ClayList` row title text | `[data-disabled]` | Foreground (textColor, opacity), Typography | Layout-neutral | `text.primary`, `text.disabled` | `core.list.rowTitle` | Primary row label |
| `list` | `rowDetail` | `default` | `rest`, `disabled` | `ClayList` row detail text | `[data-disabled]` | Foreground (textColor, opacity), Typography | Layout-neutral | `text.muted`, `text.disabled` | `core.list.rowDetail` | Secondary row label (`typography.detail`) |
| `collapse` | `root` | `default` | `rest` | `ClayCollapse` (`frontend/src/components/controls.tsx`) / React Aria `Disclosure` | `[data-expanded]` | Geometry, Material, Borders | Mixed | `transparent`, `border.hairline` | `core.collapse.root` | Disclosure container |
| `collapse` | `header` | `default` | `rest`, `hover`, `focus`, `expanded` | `ClayCollapse` `<Button slot="trigger">` | `[data-hovered]`, `[data-focused]`, `[data-expanded]` | Geometry, Material, Borders, Outlines | Mixed | `transparent`, `surface.hover`, `border.hairline`, `focus.ring` | `core.collapse.header` | Disclosure button (`aria-expanded`) |
| `collapse` | `title` | `default` | `rest`, `hover` | `ClayCollapse` title text | `[data-hovered]` | Foreground (textColor), Typography | Layout-neutral | `text.primary` | `core.collapse.title` | Section title (`typography.section`) |
| `collapse` | `chevron` | `default` | `rest`, `expanded` | `ClayCollapse` chevron icon | `[data-expanded]` | Foreground, Motion (rotation) | Layout-neutral | `text.muted`, `text.primary` | `core.collapse.chevron` | `aria-hidden` disclosure indicator |
| `collapse` | `body` | `default` | `rest`, `expanded` | `ClayCollapse` `<DisclosurePanel>` | `[data-expanded]` | Geometry (padding), Borders, Material | Layout-affecting | `transparent`, `border.hairline` | `core.collapse.body` | Content hidden when collapsed via layout clip |
| `modal` | `scrim` | `default` | `rest` | `ClayModal` (`frontend/src/components/modal.tsx`) / React Aria `ModalOverlay` | `[data-entering]`, `[data-exiting]` | Material (backgroundOpacity, backdropBlur) | Layout-neutral | `surface.scrim`, `opacity.scrim` | `core.modal.scrim` | `aria-hidden` backdrop; dims window |
| `modal` | `dialog` | `default` | `rest` | `ClayModal` `<Dialog>` | None | Geometry, Material, Borders, Shadows | Mixed | `surface.overlay`, `text.primary`, `border.strong`, `elevation.overlay` | `core.modal.dialog` | `role="dialog"`, `aria-modal="true"`, focus trap + restore |
| `modal` | `title` | `default` | `rest` | `ClayModal` `<Heading slot="title">` | None | Foreground (textColor), Typography | Layout-neutral | `text.primary` | `core.modal.title` | Dialog title (`typography.title`) wired to `aria-labelledby` |
| `modal` | `body` | `default` | `rest` | `ClayModal` content wrapper | None | Geometry (padding), Material | Layout-affecting | `transparent` | `core.modal.body` | Bounded scrolling region |
| `panel` | `root` | `fixed`, `transient` | `rest` | `Panel` / `PackageWorkspace` (`frontend/src/packages/PackageWorkspace.tsx`) | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle`, `border.hairline` | `core.panel.<variant>.root` | `role="region"` or `role="complementary"` landmark |
| `panel` | `header` | `default` | `rest` | `Panel` title/actions bar | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle` | `core.panel.header` | Panel header bar |
| `panel` | `title` | `default` | `rest` | `Panel` title text | None | Foreground (textColor), Typography | Layout-neutral | `text.primary` | `core.panel.title` | `typography.title` |
| `panel` | `body` | `default` | `rest` | `Panel` content slot | None | Geometry (padding), Material | Layout-affecting | `transparent` | `core.panel.body` | Panel content container |
| `label` | `root` | `body`, `title`, `status`, `display`, `section`, `detail`, `caption` | `rest`, `disabled` | `ClayText` (`frontend/src/components/text.tsx`) | `[data-disabled]` | Foreground (textColor, opacity), Typography | Layout-neutral | `text.primary`, `text.muted`, `text.disabled` | `core.label.<variant>.root` | Accessible static text; semantic variant scale |
| `statusItem` | `root` | `default` | `rest`, `disabled` | `PackageComponent` (`frontend/src/sdui/registry.tsx`) | `[data-disabled]` | Foreground (textColor, opacity), Typography | Layout-neutral | `text.muted`, `text.disabled` | `core.statusItem.root` | `role="status"` live region when dynamic |
| `flex` | `root` | `row`, `column` | `rest` | `PackageComponent` flex `<div>` | None | Geometry (gap), Layout | Layout-affecting | `transparent` | `core.flex.root` | Layout container; transparent to AT |
| `stack` | `root` | `default` | `rest` | `PackageComponent` stack `<div>` | None | Geometry, Layout | Layout-neutral | `transparent` | `core.stack.root` | Z-stack container (`z.*` stacking) |
| `overlay` | `root` | `default` | `rest` | `PackageComponent` overlay `<div>` / React Aria `Popover` | None | Geometry, Material, Borders, Shadows | Mixed | `surface.overlay`, `border.hairline`, `z.overlay` | `core.overlay.root` | Floating layer; anchored to target; dismissal policy |
| `portal` | `root` | `default` | `rest` | `PackageComponent` `createPortal` | None | Geometry, Layout | Layout-neutral | `transparent` | `core.portal.root` | Fixed z-layer target |
| `scroll` | `root` | `default` | `rest` | `PackageComponent` scroll `<div>` | None | Geometry (padding), Material | Layout-affecting | `transparent` | `core.scroll.root` | `tabindex="0"`; keyboard scrollable region |
| `scroll` | `scrollbarTrack` | `default` | `rest` | Scrollbar CSS / webkit pseudo | None | Material (backgroundColor), Geometry (width) | Layout-neutral | `surface.scrollbar.track`, `transparent` | `core.scroll.scrollbarTrack` | Scrollbar track |
| `scroll` | `scrollbarThumb` | `default` | `rest`, `hover`, `active` | Scrollbar CSS / webkit pseudo | `:hover`, `:active` | Material (backgroundColor, opacity), Geometry (radius) | Layout-neutral | `surface.scrollbar`, `opacity.disabled` (rest), `opacity.full` (active) | `core.scroll.scrollbarThumb` | Near-invisible at rest; solid on hover/active |
| `editorView` | `root` | `default` | `rest` | `ClayEditor` (`frontend/src/editor/ClayEditor.tsx`) | None | Geometry, Material | Layout-neutral | `shellBg` (editor StyleRegistry) | `core.editorView.root` | One content host per pane leaf |
| `table` | `root` | `default` | `reserved` | Reserved for React Aria `Table` | `[data-disabled]` | Geometry, Material, Borders | Mixed | `surface.list`, `border.subtle` | `core.table.root` | Reserved kind; rejected at validation until unlocked |

---

### 2. Clay-Native Internal Surfaces

| Surface | Slot | Variants | Applicable States | React Owner / Headless Primitive | React Aria State Mapping | Allowed Property Families | Layout Impact | Active-Theme Color Role Source | Required Fallback Recipe ID | Accessibility Invariant |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `tabBar` | `bar` | `default` | `rest` | `TabBar` (`frontend/src/app/layout/tab-bar.tsx`) / React Aria `Tabs` | None | Geometry, Material, Borders | Mixed | `surface.main`, `border.hairline` | `core.tabBar.bar` | `role="tablist"`; keyboard left/right arrow navigation |
| `tabBar` | `card` | `default` | `rest`, `hover`, `active`, `focus`, `selected` | `TabBar` `<Tab>` | `[data-hovered]`, `[data-pressed]`, `[data-focused]`, `[data-selected]` | Geometry, Material, Borders, Outlines | Mixed | `surface.panel`, `surface.hover`, `surface.selected`, `border.hairline`, `focus.ring` | `core.tabBar.card` | `role="tab"`; `aria-selected="true"` for active document |
| `tabBar` | `cardLabel` | `default` | `rest`, `hover`, `selected` | `TabBar` document label text | `[data-selected]` | Foreground (textColor), Typography | Layout-neutral | `text.muted`, `text.primary` | `core.tabBar.cardLabel` | `typography.detail` |
| `tabBar` | `closeButton` | `default` | `rest`, `hover`, `active`, `focus` | `TabBar` close `<button>` | `:hover`, `:active`, `:focus-visible` | Geometry, Material, Foreground | Layout-neutral | `transparent`, `surface.hover`, `text.muted`, `text.primary` | `core.tabBar.closeButton` | Nested button with `aria-label="Close tab"` |
| `tabBar` | `dirtyIndicator` | `default` | `rest` | `TabBar` dirty dot `<span>` | None | Geometry (radius, size), Material | Layout-neutral | `accent.primary` | `core.tabBar.dirtyIndicator` | `aria-label="Unsaved changes"` |
| `paneSplitTree` | `group` | `horizontal`, `vertical` | `rest` | `WorkspacePanes` (`frontend/src/shell/WorkspacePanes.tsx`) / react-resizable-panels `PanelGroup` | None | Geometry, Layout | Layout-affecting | `transparent` | `core.paneSplitTree.group` | Multi-pane layout container |
| `paneSplitTree` | `pane` | `default` | `rest` | `PaneTree` (`frontend/src/shell/PaneTree.tsx`) / react-resizable-panels `Panel` | None | Geometry, Material, Borders | Mixed | `surface.main` | `core.paneSplitTree.pane` | Leaf pane container; clamped 0.05–0.95 ratio |
| `paneSplitTree` | `handle` | `horizontal`, `vertical` | `rest`, `hover`, `active`, `focus` | `WorkspacePanes` `PanelResizeHandle` | `[data-resize-handle-state="hover"]`, `[data-resize-handle-state="drag"]`, `:focus-visible` | Geometry, Material, Borders, Outlines | Mixed | `border.hairline`, `border.subtle`, `accent.primary`, `focus.ring` | `core.paneSplitTree.handle` | `role="separator"`; arrow key resizing |
| `statusBar` | `root` | `default` | `rest` | `AppShell` (`frontend/src/app/layout/app-shell.tsx`) `<footer>` | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.hairline` | `core.statusBar.root` | Landmark footer |
| `statusBar` | `item` | `default` | `rest`, `disabled` | `AppShell` status item text | None | Foreground (textColor, opacity), Typography | Layout-neutral | `text.muted`, `text.primary`, `text.disabled` | `core.statusBar.item` | `typography.status`; tabular numbers |
| `commandCentre` | `scrim` | `default` | `rest` | `CommandCentre` (`frontend/src/command-centre/CommandCentre.tsx`) | `[data-entering]`, `[data-exiting]` | Material (backgroundOpacity, backdropBlur) | Layout-neutral | `surface.scrim`, `opacity.scrim` | `core.commandCentre.scrim` | Window dim behind centered dialog |
| `commandCentre` | `dialog` | `default` | `rest` | `CommandCentre` `<Dialog>` | None | Geometry, Material, Borders, Shadows | Mixed | `surface.overlay`, `border.strong`, `elevation.overlay` | `core.commandCentre.dialog` | `role="dialog"`, `aria-modal="true"`, 640px centered width default |
| `commandCentre` | `input` | `default` | `rest`, `focus` | `CommandCentre` search `<Input>` | `[data-focused]` | Geometry, Material, Borders, Outlines | Mixed | `surface.control`, `text.primary`, `border.focus` | `core.commandCentre.input` | Query input field; server-owned results |
| `commandCentre` | `listBox` | `default` | `rest` | `CommandCentre` `<ListBox>` | None | Geometry (max-height), Material | Layout-affecting | `surface.overlay` | `core.commandCentre.listBox` | `role="listbox"`; capped at 256 items |
| `commandCentre` | `item` | `default` | `rest`, `hover`, `active`, `focus`, `selected` | `CommandCentre` `<ListBoxItem>` | `[data-hovered]`, `[data-focused]`, `[data-selected]` | Geometry, Material, Borders | Mixed | `surface.list`, `surface.hover`, `surface.selected`, `text.primary` | `core.commandCentre.item` | `role="option"`; typeahead navigation |
| `commandCentre` | `status` | `default` | `rest` | `CommandCentre` count/status line | None | Foreground (textColor), Typography | Layout-neutral | `text.muted` | `core.commandCentre.status` | `typography.status`; live result count |
| `commandCentre` | `empty` | `default` | `rest` | `CommandCentre` empty view | None | Geometry (padding), Material, Borders | Mixed | `surface.overlay`, `border.subtle`, `text.muted` | `core.commandCentre.empty` | Empty query results view |
| `fileBrowser` | `root` | `default` | `rest` | `FileBrowser` / Left Slot | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle` | `core.fileBrowser.root` | Workspace file tree container |
| `fileBrowser` | `header` | `default` | `rest` | `FileBrowser` header | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle`, `text.primary` | `core.fileBrowser.header` | Workspace name and root path |
| `fileBrowser` | `tree` | `default` | `rest` | `FileBrowser` `<Tree>` | None | Geometry, Material | Layout-affecting | `surface.panel` | `core.fileBrowser.tree` | `role="tree"`; keyboard tree navigation |
| `fileBrowser` | `item` | `file`, `directory` | `rest`, `hover`, `active`, `focus`, `selected` | `FileBrowser` `<TreeItem>` | `[data-hovered]`, `[data-focused]`, `[data-selected]` | Geometry, Material, Borders | Mixed | `surface.panel`, `surface.hover`, `surface.selected`, `text.primary`, `focus.ring` | `core.fileBrowser.item` | `role="treeitem"`; expandable directories |
| `settingsPanel` | `panel` | `default` | `rest` | `SettingsPanel` (`frontend/src/settings/SettingsPanel.tsx`) | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle` | `core.settingsPanel.panel` | Trusted compiled presentation module for `@clay/settings` |
| `settingsPanel` | `heading` | `default` | `rest` | `SettingsPanel` header | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle`, `text.primary` | `core.settingsPanel.heading` | Title and close actions bar |
| `settingsPanel` | `actions` | `default` | `rest` | `SettingsPanel` actions bar | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.subtle` | `core.settingsPanel.actions` | Footer save/reset actions |
| `settingsPanel` | `fields` | `default` | `rest` | `SettingsPanel` fields container | None | Geometry (gap), Layout | Layout-affecting | `transparent` | `core.settingsPanel.fields` | Composes `textInput`, `dropdown`, `collapse`, `button` |
| `chatPanel` | `root` | `default` | `rest` | `ChatPanel` (`frontend/src/chat/ChatPanel.tsx`) | None | Geometry, Material, Borders | Mixed | `surface.main`, `border.hairline` | `core.chatPanel.root` | Main empty-tab pane content |
| `chatPanel` | `header` | `default` | `rest` | `ChatPanel` header | None | Geometry, Material, Borders | Mixed | `surface.main`, `border.strong` | `core.chatPanel.header` | Chat agent header bar |
| `chatPanel` | `transcript` | `default` | `rest` | `ChatPanel` transcript area | None | Geometry (padding, gap), Material | Layout-affecting | `surface.panel` | `core.chatPanel.transcript` | Scrollable message history |
| `chatPanel` | `userMessage` | `default` | `rest` | `ChatPanel` user message bubble | None | Geometry, Material, Borders | Mixed | `surface.badge`, `border.strong`, `text.primary` | `core.chatPanel.userMessage` | User prompt bubble |
| `chatPanel` | `assistantMessage` | `default` | `rest` | `ChatPanel` assistant message | None | Geometry, Material, Borders | Mixed | `surface.panel`, `accent.primary` (border-left), `text.primary` | `core.chatPanel.assistantMessage` | Assistant response card |
| `chatPanel` | `thinking` | `default` | `rest` | `ChatPanel` thinking block | None | Geometry, Material, Typography | Mixed | `surface.panel`, `text.muted`, `font.monospace` | `core.chatPanel.thinking` | Collapsible reasoning block |
| `chatPanel` | `composer` | `default` | `rest`, `focus` | `ChatPanel` composer area | `:focus-within` | Geometry, Material, Borders | Mixed | `surface.main`, `border.strong` | `core.chatPanel.composer` | Input composer area |
| `chatPanel` | `statusLine` | `default` | `rest` | `ChatPanel` status bar | None | Geometry, Material, Borders | Mixed | `surface.panel`, `border.hairline`, `text.muted` | `core.chatPanel.statusLine` | Chat session status line |
| `welcome` | `root` | `default` | `rest` | `WelcomeView` / Empty pane fallback | None | Geometry, Material, Borders | Mixed | `surface.main`, `text.primary` | `core.welcome.root` | Core fallback when no empty-tab package is loaded |
| `transientMenu` | `root` | `context`, `menuBar` | `rest` | React Aria `Menu` / `Popover` | `[data-open]` | Geometry, Material, Borders, Shadows | Mixed | `surface.overlay`, `border.hairline`, `z.overlay` | `core.transientMenu.root` | Context menu and menu bar popups |
| `completion` | `root` | `default` | `rest` | CodeMirror autocompletion pop-up | None | Geometry, Material, Borders, Shadows | Mixed | `surface.overlay`, `border.hairline`, `z.tooltip` | `core.completion.root` | Modeless caret-adjacent picker; 8-row / 480px cap |
| `editorChrome` | `gutter` | `default` | `rest` | CodeMirror `@codemirror/view` gutter | None | Geometry, Material, Borders | Mixed | `panelBg`, `gutterFg`, `gutterFgActive` | `core.editorChrome.gutter` | Line numbers, fold chevrons; `StyleRegistry`-driven |
| `editorChrome` | `activeLine` | `default` | `rest` | CodeMirror line highlighter | None | Material (backgroundColor) | Layout-neutral | `lineHighlight` | `core.editorChrome.activeLine` | Active editor line background |
| `editorChrome` | `indentGuide` | `default` | `rest` | CodeMirror indent guide extension | None | Borders (borderColor, borderWidth) | Layout-neutral | `indentGuide` | `core.editorChrome.indentGuide` | Indentation depth guides |
| `editorChrome` | `bracketMatch` | `default` | `rest` | CodeMirror `matchBrackets` | None | Material, Borders | Layout-neutral | `bracketMatch` | `core.editorChrome.bracketMatch` | Matching bracket highlight |
| `editorChrome` | `diagnostics` | `error`, `warning`, `info` | `rest`, `hover` | CodeMirror lint extension | None | Material, Borders, Squiggle | Layout-neutral | `diagnosticError`, `diagnosticWarning`, `diagnosticInfo` | `core.editorChrome.diagnostics` | Diagnostic squiggles and gutter markers |

---

### 3. Chrome Primitives (`src/shell/primitives.rs`)

| Chrome Primitive | Slot | Variants | Applicable States | React Owner / Headless Primitive | React Aria State Mapping | Allowed Property Families | Layout Impact | Active-Theme Color Role Source | Required Fallback Recipe ID | Accessibility Invariant |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `badge` | `root` | `default` | `rest` | `ClayBadge` (`frontend/src/components/chrome.tsx`) | None | Geometry, Material, Borders | Mixed | `surface.badge`, `radius.xs`, `spacing.badge` | `core.badge.root` | `role="status"` or `role="note"` |
| `badge` | `label` | `default` | `rest` | `ClayBadge` label text | None | Foreground (textColor), Typography | Layout-neutral | `text.badge`, `typography.caption` | `core.badge.label` | Accessible tag text |
| `kbd` | `root` | `default` | `rest` | `ClayKbd` (`frontend/src/components/chrome.tsx`) `<kbd>` | None | Geometry, Material, Borders | Mixed | `surface.kbd`, `border.kbd`, `radius.xs`, `dimension.kbd.height` | `core.kbd.root` | Native `<kbd>` tag |
| `kbd` | `label` | `default` | `rest` | `ClayKbd` key text | None | Foreground (textColor), Typography | Layout-neutral | `text.kbd`, `font.monospace`, `typography.caption` | `core.kbd.label` | Shortcut key representation |
| `divider` | `root` | `horizontal`, `vertical` | `rest` | `ClayDivider` (`frontend/src/components/chrome.tsx`) `<hr>` | None | Geometry (borderWidth), Borders | Mixed | `border.hairline`, `dimension.border.hairline` | `core.divider.root` | `role="separator"` |
| `tooltip` | `root` | `default` | `rest` | React Aria `Tooltip` | `[data-entering]`, `[data-exiting]` | Geometry, Material, Borders, Shadows | Mixed | `surface.tooltip`, `border.hairline`, `elevation.overlay`, `z.tooltip` | `core.tooltip.root` | `role="tooltip"`; hover + focus trigger |
| `tooltip` | `content` | `default` | `rest` | React Aria `Tooltip` text | None | Foreground (textColor), Typography | Layout-neutral | `text.tooltip`, `typography.body` | `core.tooltip.content` | Tooltip content text |
| `scrim` | `root` | `default` | `rest` | Full-window scrim `<div>` | None | Material (backgroundOpacity, backdropBlur) | Layout-neutral | `surface.scrim`, `opacity.scrim` | `core.scrim.root` | `aria-hidden="true"` dialog backdrop |
| `focusRing` | `ring` | `default` | `focus` | CSS `:focus-visible` outline | `[data-focused]`, `:focus-visible` | Outlines (outlineColor, outlineWidth, outlineOffset, outlineStyle) | Layout-neutral | `focus.ring`, `border.focus`, `dimension.border.thin` | `core.focusRing.ring` | Visible focus indicator; never `outline: none` without replacement |
| `scrollChrome` | `track` | `default` | `rest` | Native scrollbar track | None | Material (backgroundColor), Geometry (width) | Layout-neutral | `surface.scrollbar.track`, `dimension.scrollbar.width` | `core.scrollChrome.track` | Native scrollbar track styling |
| `scrollChrome` | `thumb` | `default` | `rest`, `hover`, `active` | Native scrollbar thumb | `:hover`, `:active` | Material (backgroundColor, opacity), Geometry (radius) | Layout-neutral | `surface.scrollbar`, `opacity.disabled` (rest), `opacity.full` (active) | `core.scrollChrome.thumb` | Native scrollbar thumb styling |
| `iconSlot` | `root` | `default` | `rest`, `disabled` | Inline SVG / Icon placeholder | `[data-disabled]` | Geometry (size), Foreground (iconColor, opacity) | Layout-neutral | `dimension.icon.size`, `text.icon`, `opacity.disabled` | `core.iconSlot.root` | `aria-hidden="true"` or `<img alt="...">` |

---

## Fallback Resolution and Inheritance Algorithm

When a component renders, its visual properties resolve through the following deterministic 5-step fallback chain:

```text
1. Package exact recipe:
   pkg.<component>.<variant>.<slot>.<state>
   │
   ▼ (if property absent or state not defined)
2. Package slot rest default:
   pkg.<component>.<variant>.<slot>.rest
   │
   ▼ (if slot not declared in package)
3. Package component default:
   pkg.<component>.default.<slot>.<state>
   │
   ▼ (if component kind not declared in package)
4. Core design system recipe:
   core.<component>.<variant>.<slot>.<state>
   │
   ▼ (universal fallback guarantee)
5. Host CSS custom-property tokens:
   var(--clay-<token-name>)
```

### Fallback Guarantee Rules
- Every color property in both package recipes and core fallbacks maps to an active-theme color role.
- If a package declares a design-system value that references an invalid or non-existent theme role, package validation fails before runtime adoption.
- When an adopted package does not declare a recipe for a component kind (e.g., `modal`), the host cleanly applies `core.modal.*` recipes without visual corruption or partial state.

---

## Prohibited Authorities Deny List

The following capabilities are explicitly denied in `clay.contributions.uiDesignSystem` and cause package validation to fail closed:

1. **Raw CSS Strings:** No inline CSS blocks, `@import`, `@keyframes`, `@media` queries, or arbitrary rule strings.
2. **DOM Selectors:** No class names (`.button`), ID selectors (`#root`), tag names (`button`), or attribute selectors (`[data-hovered]`). State mapping is performed through closed, typed semantic state keys.
3. **Literal Color Values:** No hex codes (`#1a1a1a`), `rgb()`, `rgba()`, `hsl()`, or named web colors (`red`, `blue`).
4. **External URLs and Resources:** No `url(...)`, image paths, web font links, or remote asset fetches.
5. **Arbitrary Filters and Transforms:** No unrestricted `filter: drop-shadow(...)`, `transform: matrix(...)`, or 3D transforms. Only closed, safe transform presets and bounded blur dimensions are allowed.
6. **Executable Code / Callbacks:** No JavaScript functions, event handler names, JSX, V8 handles, or DOM nodes.
7. **Direct Tauri IPC:** Design-system contributions cannot register commands, request window mutations, or invoke backend ops.

---

## Activation and Runtime Pipeline Ownership

The end-to-end activation and execution pipeline maps across explicit subsystem owners:

| Pipeline Stage | Subsystem Owner | Responsibility |
| --- | --- | --- |
| **1. Configuration Selection** | `src/server/ops/theme.rs` (`setDesignSystem`) | Receives specifier from `init.js`, validates package format, and checks package record availability. |
| **2. Package Record Lookup & Provenance** | `src/packages/service.rs` / `src/packages/record/` | Looks up installed/bundled record, enforces third-party vs bundled trust domain without promotion. |
| **3. Recipe Validation & Fallback Resolution** | `src/shell/design_system.rs` | Resolves inheritance, enforces property bounds and theme-color role references, merges with built-in fallbacks. |
| **4. Runtime Generation Commit** | `src/server/configuration.rs` / `src/protocol/runtime.rs` | Commits atomic generation with `ActiveDesignSystem`; on error retains previous valid generation. |
| **5. Desktop Bridge DTO Projection** | `src-tauri/src/bridge/dto.rs` | Serializes clean, bounded DTO (`ActiveDesignSystemDto`); denies literal colors, raw CSS, selectors, and scripts. |
| **6. Frontend Session Ingestion** | `frontend/src/app/use-clay-session.ts` | Ingests snapshot via bridge, forwards to dedicated `designSystemStore` alongside `themeStore`. |
| **7. Custom Property Adapter & Installation** | `frontend/src/theme/design-system-adapter.ts` / `frontend/src/state/design-system-store.ts` | Converts recipes to deterministic `--clay-recipe-*` CSS custom properties; batches root DOM updates and removes obsolete keys. |
| **8. Component Visual Consumption** | Host CSS modules (`frontend/src/components/*.module.css`) | Consumes `--clay-recipe-*` variables; colors resolve indirectly through active content theme variables (`var(--clay-*)`). |

