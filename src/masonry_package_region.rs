//! Package-UI component reconciliation (plan 070 step 13a).
//!
//! Mirrors the SDUI retained reconciler (`SduiRegionWidget`, plan 070 step 11)
//! but for **package_ui** component trees (`PackageUiComponentTree`), which are
//! *nested* (children inline) rather than a flat node map with incremental ops.
//! The reconciler diffs a freshly-provided tree against the retained widget
//! subtree by stable component id (`stable_package_source_id`), so surviving
//! components keep their `WidgetId` — and any Masonry-managed state (focus,
//! and later dropdown/modal open state) — across re-renders.
//!
//! Step 13a is the **foundation only**: it reconciles the non-transient kinds
//! (`panel`/`flex`/`stack` + `label`/`statusItem`/`editorView`/`button`/`list`)
//! at paint parity with the legacy `paint_package_component` immediate-mode
//! renderer, reusing the shared paint helpers (`paint_sdui_text`/
//! `component_state_color`/`list_row_fill_color`/`disabled_text_color`/
//! `paint_focus_ring`/`sdui_row_rect`). It is not wired into production paint
//! yet (that is step 13b). The transient kinds (`dropdown`/`collapse`/`modal`/
//! `textInput`) reconcile to an inert placeholder leaf here and become real
//! widgets in steps 13c–13e.
//!
//! Widget reuse note: the plan suggested reusing the SDUI widget structs, but
//! package components carry package-specific data (`selected`/`disabled`/
//! `validation_state`) that the SDUI widgets do not model, and `statusItem` has
//! no `SduiNodeKind` analogue. So the package leaves are package-specific
//! widgets that reuse the shared *paint helpers* (parity by construction)
//! rather than contorting the production SDUI widgets. Interaction follows the
//! same Masonry pointer/focus/keyboard pattern as `SduiButton`/`SduiListRow`.

#![allow(dead_code)] // Staged seam: live paths land in steps 13b+.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use masonry::accesskit::{Live, Node, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, MutateCtx, NewWidget,
    NoAction, PaintCtx, PointerEvent, Properties, PropertiesMut, PropertiesRef, RegisterCtx,
    TextEvent, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry::kurbo::{Affine, Point, Rect, Size, Stroke};
use masonry::peniko::{Color, Fill};
use masonry::properties::types::Length;
use masonry::properties::{CaretColor, ContentColor, SelectionColor};
use masonry::vello::Scene;
use masonry::widgets::{Flex, TextArea};

use crate::editor::typography::{TypographyRegistry, UiTextMetrics, UiTextVariant};
use crate::masonry_sdui::{
    overlay_z_order, package_action_intent, paint_sdui_text, sdui_row_rect,
    stable_package_source_id,
};
use crate::masonry_sdui_region::SduiScrollViewport;
use crate::protocol::{FontRole, SduiActionArgument, SduiActionIntent, SduiActionValue};
use crate::shell::package_ui::{
    MenuA11y, PackageOverlayAnchor, PackageUiComponentTree, PackageUiListItem,
    PackageUiRuntimeState, TransientPackageOverlay, completion_overlay_rect,
};
use crate::shell::primitives::{paint_scrim, paint_tooltip_shell};
use crate::shell::theme::{ResolvedUiTheme, SduiThemeStyle};
use crate::shell::{
    FixedPackagePanel, FixedSlotId, InteractionState, PanelChrome, component_state_color,
    disabled_text_color, list_row_fill_color, paint_focus_ring, paint_panel_chrome,
};

/// Child key for the package reconciler's stable-identity diff.
#[derive(Clone, PartialEq, Eq)]
enum PackageChildKey {
    /// A real child component, keyed by `stable_package_source_id(component.id)`
    /// plus its `kind`. Carrying the kind means a kind change at the same id
    /// reads as remove+add (fresh widget), not a survivor whose downcast would
    /// silently fail — the nested-kind-change rule from Step 11.
    Component(u64, String),
    /// A panel's synthetic title row (first child when the panel has a title).
    PanelTitle,
    /// A list's synthetic row, keyed by `PackageUiListItem::id`.
    ListRow(String),
}

/// The reconciled identity of one package component: the `WidgetId` its widget
/// was built with plus the component `kind` it was built as (to detect kind
/// changes, which force a rebuild rather than an in-place prop update).
struct PackagePodRecord {
    id: WidgetId,
    kind: String,
}

// Slots 1 = status, 2+ = items (legacy numbering, kept for test stability),
// derived through the shared `virtual_a11y_node_id` policy.

/// Clay-owned container reconciling a package_ui component tree into a retained
/// Masonry subtree. `pub(crate)` only; packages never see Masonry handles.
pub(crate) struct PackageRegionWidget {
    root_hash: Option<u64>,
    root_pod: Option<WidgetPod<dyn Widget>>,
    /// Stable-identity map: component id hash → its widget identity + kind.
    pods: BTreeMap<u64, PackagePodRecord>,
    /// Per-container ordered child keys, parallel to each container's `Flex`
    /// children, driving the keyed child-list diff.
    child_keys: BTreeMap<u64, Vec<PackageChildKey>>,
    /// Commit-intent base for each retained `textInput`, keyed by the inner
    /// `TextArea`'s `WidgetId` (which is what Masonry's `TextAction::Entered`
    /// reports as the source). The committed value is appended as an argument
    /// at commit time (step 13c).
    text_input_intents: HashMap<WidgetId, SduiActionIntent>,
    /// Focusable widget ids accumulated in build order while constructing a
    /// subtree, so a `modal` can capture its descendants' focus-trap set (the
    /// slice appended while its children were built). Reset per reconcile.
    focusable_sink: Vec<WidgetId>,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
    /// Plan 070 step 13f: when `Some`, this region hosts a transient menu and
    /// `accessibility()` reports a `Menu`/`MenuItem`/`Status` subtree built from
    /// it (excluding the generic reconciled subtree) instead of the default
    /// `Group` + children flow. Set by `PackageOverlayHost::sync_overlays`.
    menu_a11y: Option<MenuA11y>,
    /// Selection target consumed by any internal `scroll` wrapper in a menu
    /// projection so keyboard selection remains visible after reconciliation.
    menu_scroll_target: Rc<Cell<Option<(f64, f64)>>>,
}

impl PackageRegionWidget {
    pub(crate) fn new() -> Self {
        Self {
            root_hash: None,
            root_pod: None,
            pods: BTreeMap::new(),
            child_keys: BTreeMap::new(),
            text_input_intents: HashMap::new(),
            focusable_sink: Vec::new(),
            typography: TypographyRegistry::default(),
            ui_theme: ResolvedUiTheme::default(),
            menu_a11y: None,
            menu_scroll_target: Rc::new(Cell::new(None)),
        }
    }

    /// Build the commit intent for the `textInput` whose inner `TextArea` has
    /// widget id `area_id`, appending the committed `value` as a `"value"`
    /// argument. Returns `None` when `area_id` is not a reconciled text input.
    pub(crate) fn text_input_commit(
        &self,
        area_id: WidgetId,
        value: &str,
    ) -> Option<SduiActionIntent> {
        let base = self.text_input_intents.get(&area_id)?;
        let mut intent = base.clone();
        intent.arguments.push(SduiActionArgument {
            name: "value".to_string(),
            value: SduiActionValue::String(value.to_string()),
        });
        Some(intent)
    }

    /// Install the active typography/theme (applied to surviving widgets in
    /// place on the live path; used for fresh builds on the wholesale path).
    pub(crate) fn set_render_context(
        &mut self,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.typography = typography;
        self.ui_theme = ui_theme;
    }

    /// Plan 070 step 13f: install the hosted-menu a11y payload (`Some` for menu
    /// overlays, `None` for package-declared overlays/fixed panels).
    pub(crate) fn set_menu_a11y(&mut self, menu_a11y: Option<MenuA11y>) {
        let selected = menu_a11y
            .as_ref()
            .and_then(|menu| menu.items.iter().position(|item| item.selected));
        let style = SduiThemeStyle::from_ui_theme(&self.ui_theme);
        let body = self
            .typography
            .ui_text_metrics(FontRole::Ui, style.body_text);
        let detail = self
            .typography
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
        let row_height = body.list_height(detail);
        self.menu_scroll_target.set(selected.map(|index| {
            let y0 = index as f64 * row_height;
            (y0, y0 + row_height)
        }));
        self.menu_a11y = menu_a11y;
    }

    /// The reconciled root component's `WidgetId` (test accessor).
    pub(crate) fn root_pod_id(&self) -> Option<WidgetId> {
        self.root_pod.as_ref().map(WidgetPod::id)
    }

    /// A reconciled component's `WidgetId` by its id hash (test accessor).
    pub(crate) fn pod_id_for(&self, id_hash: u64) -> Option<WidgetId> {
        self.pods.get(&id_hash).map(|record| record.id)
    }

    /// Wholesale reconcile (no `MutateCtx`): rebuild the subtree from `tree`.
    /// Used by standalone tests and as the fallback when the root kind changes.
    pub(crate) fn reconcile_tree(&mut self, tree: &PackageUiComponentTree) {
        self.pods.clear();
        self.child_keys.clear();
        self.text_input_intents.clear();
        self.focusable_sink.clear();
        self.root_hash = Some(stable_package_source_id(&tree.id));
        self.root_pod = Some(self.build_component(tree, 0).to_pod());
        self.gc(tree);
    }

    /// Live reconcile against a retained subtree (step 13b production path):
    /// reuse the root pod in place when the root kind is unchanged, else swap.
    pub(crate) fn reconcile_tree_live(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        tree: &PackageUiComponentTree,
    ) {
        let root_hash = stable_package_source_id(&tree.id);
        self.root_hash = Some(root_hash);
        self.text_input_intents.clear();
        self.focusable_sink.clear();
        let reusable = self.root_pod.is_some()
            && self.pods.get(&root_hash).map(|record| record.kind.as_str())
                == Some(tree.kind.as_str());
        if reusable {
            let mut pod = self.root_pod.take().expect("root pod present");
            {
                let widget = ctx.get_mut(&mut pod);
                self.reconcile_component(widget, tree, 0);
            }
            self.root_pod = Some(pod);
        } else {
            if let Some(old) = self.root_pod.take() {
                ctx.remove_child(old);
            }
            self.pods.clear();
            self.child_keys.clear();
            self.text_input_intents.clear();
            self.root_pod = Some(self.build_component(tree, 0).to_pod());
            ctx.children_changed();
        }
        self.gc(tree);
    }

    /// Recursively build one component into a fresh widget, recording its
    /// `PackagePodRecord` and (for containers) its child order.
    fn build_component(
        &mut self,
        component: &PackageUiComponentTree,
        depth: usize,
    ) -> NewWidget<dyn Widget> {
        let hash = stable_package_source_id(&component.id);
        let (widget, keys): (NewWidget<dyn Widget>, Vec<PackageChildKey>) =
            match component.kind.as_str() {
                "panel" => {
                    let mut column = Flex::column().with_gap(Length::ZERO);
                    let mut keys = Vec::new();
                    if component.title.is_some() {
                        column = column.with_child(NewWidget::new(PackageLeaf::panel_title(
                            component,
                            depth,
                            self.typography.clone(),
                            self.ui_theme.clone(),
                        )));
                        keys.push(PackageChildKey::PanelTitle);
                    }
                    for child in &component.children {
                        column = column.with_child(self.build_component(child, depth + 1));
                        keys.push(Self::component_key(child));
                    }
                    (NewWidget::new(column).erased(), keys)
                }
                // Package containers all flow children vertically (the legacy
                // renderer stacks them with a shared cursor_y), so map every
                // container — including `stack`, which is *not* a z-stack here —
                // to a zero-gap column.
                "flex" | "stack" | "overlay" | "portal" => {
                    let mut column = Flex::column().with_gap(Length::ZERO);
                    let mut keys = Vec::new();
                    for child in &component.children {
                        column = column.with_child(self.build_component(child, depth));
                        keys.push(Self::component_key(child));
                    }
                    (NewWidget::new(column).erased(), keys)
                }
                "scroll" => {
                    let mut column = Flex::column().with_gap(Length::ZERO);
                    let mut keys = Vec::new();
                    for child in &component.children {
                        column = column.with_child(self.build_component(child, depth));
                        keys.push(Self::component_key(child));
                    }
                    (
                        NewWidget::new(SduiScrollViewport::with_selection_target(
                            NewWidget::new(column).erased(),
                            self.ui_theme.clone(),
                            self.menu_scroll_target.clone(),
                        ))
                        .erased(),
                        keys,
                    )
                }
                "button" => {
                    let button = NewWidget::new(PackageButton::from_component(
                        component,
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    ));
                    if component.action_command_id.is_some() && !component.disabled {
                        self.focusable_sink.push(button.id());
                    }
                    (button.erased(), Vec::new())
                }
                "list" => {
                    let mut column = Flex::column().with_gap(Length::ZERO);
                    let mut keys = Vec::new();
                    for item in &component.items {
                        let row = NewWidget::new(PackageListRow::from_item(
                            component,
                            item,
                            depth,
                            self.typography.clone(),
                            self.ui_theme.clone(),
                        ));
                        if item.action_command_id.is_some() && !item.disabled {
                            self.focusable_sink.push(row.id());
                        }
                        column = column.with_child(row);
                        keys.push(PackageChildKey::ListRow(item.id.clone()));
                    }
                    (NewWidget::new(column).erased(), keys)
                }
                "label" | "statusItem" | "editorView" => (
                    NewWidget::new(PackageLeaf::from_component(
                        component,
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    ))
                    .erased(),
                    Vec::new(),
                ),
                "collapse" => {
                    let mut column = Flex::column().with_gap(Length::ZERO);
                    let mut keys = Vec::new();
                    for child in &component.children {
                        column = column.with_child(self.build_component(child, depth + 1));
                        keys.push(Self::component_key(child));
                    }
                    let collapse = NewWidget::new(PackageCollapse::from_component(
                        component,
                        column,
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    ));
                    if !component.disabled {
                        self.focusable_sink.push(collapse.id());
                    }
                    (collapse.erased(), keys)
                }
                // A focus-trapped `Role::Dialog` modal (step 13e). Capture the
                // focusable descendants accumulated while its children were
                // built (the sink slice) for the Tab/Shift+Tab trap.
                "modal" => {
                    let start = self.focusable_sink.len();
                    let mut column = Flex::column().with_gap(Length::ZERO);
                    let mut keys = Vec::new();
                    for child in &component.children {
                        column = column.with_child(self.build_component(child, depth + 1));
                        keys.push(Self::component_key(child));
                    }
                    let focusable = self.focusable_sink[start..].to_vec();
                    (
                        NewWidget::new(PackageModal::from_component(
                            component,
                            column,
                            focusable,
                            depth,
                            self.typography.clone(),
                            self.ui_theme.clone(),
                        ))
                        .erased(),
                        keys,
                    )
                }
                // A genuinely-editable text field (step 13c). Record the commit
                // intent keyed by the inner `TextArea`'s widget id (the source id
                // Masonry reports on `TextAction::Entered`).
                "textInput" => {
                    let (input, area_id) = PackageTextInput::from_component(
                        component,
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    );
                    if let Some(intent) = input.intent.clone() {
                        self.text_input_intents.insert(area_id, intent);
                    }
                    if !component.disabled {
                        self.focusable_sink.push(area_id);
                    }
                    (NewWidget::new(input).erased(), Vec::new())
                }
                // A real ComboBox-role dropdown (step 13d): trigger + inline
                // open list, widget-local selection/open state.
                "dropdown" => {
                    let dropdown = NewWidget::new(PackageDropdown::from_component(
                        component,
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    ));
                    if !component.disabled {
                        self.focusable_sink.push(dropdown.id());
                    }
                    (dropdown.erased(), Vec::new())
                }
                // Unknown/future kinds render an inert placeholder so the
                // reconciler stays total on real package trees.
                _ => (
                    NewWidget::new(PackageLeaf::placeholder(
                        component,
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    ))
                    .erased(),
                    Vec::new(),
                ),
            };
        self.pods.insert(
            hash,
            PackagePodRecord {
                id: widget.id(),
                kind: component.kind.clone(),
            },
        );
        self.child_keys.insert(hash, keys);
        widget
    }

    /// Reconcile one surviving component in place.
    fn reconcile_component(
        &mut self,
        mut widget: WidgetMut<'_, dyn Widget>,
        component: &PackageUiComponentTree,
        depth: usize,
    ) {
        match component.kind.as_str() {
            "label" | "statusItem" | "editorView" => {
                if let Some(mut leaf) = widget.try_downcast::<PackageLeaf>() {
                    leaf.widget.update_from_component(
                        component,
                        depth,
                        &self.typography,
                        &self.ui_theme,
                    );
                    leaf.ctx.request_layout();
                }
            }
            "button" => {
                if let Some(mut button) = widget.try_downcast::<PackageButton>() {
                    button.widget.update_from_component(
                        component,
                        depth,
                        &self.typography,
                        &self.ui_theme,
                    );
                    button.ctx.request_layout();
                }
            }
            "panel" | "flex" | "stack" | "overlay" | "portal" => {
                if let Some(mut flex) = widget.try_downcast::<Flex>() {
                    let keys = Self::container_keys(component);
                    self.reconcile_flex_children(&mut flex, component, keys, depth);
                }
            }
            "scroll" => {
                if let Some(mut viewport) = widget.try_downcast::<SduiScrollViewport>() {
                    viewport.widget.set_ui_theme(self.ui_theme.clone());
                    let keys = Self::container_keys(component);
                    {
                        let mut content = SduiScrollViewport::content_mut(&mut viewport);
                        if let Some(mut flex) = content.try_downcast::<Flex>() {
                            self.reconcile_flex_children(&mut flex, component, keys, depth);
                        }
                    }
                    viewport.ctx.request_layout();
                }
            }
            "list" => {
                if let Some(mut flex) = widget.try_downcast::<Flex>() {
                    let keys = Self::container_keys(component);
                    self.reconcile_flex_children(&mut flex, component, keys, depth);
                }
            }
            "collapse" => {
                if let Some(mut collapse) = widget.try_downcast::<PackageCollapse>() {
                    collapse
                        .widget
                        .update_meta(component, &self.typography, &self.ui_theme);
                    let keys = Self::container_keys(component);
                    {
                        let mut content = PackageCollapse::content_mut(&mut collapse);
                        self.reconcile_flex_children(&mut content, component, keys, depth + 1);
                    }
                    collapse.ctx.request_layout();
                }
            }
            "textInput" => {
                if let Some(mut input) = widget.try_downcast::<PackageTextInput>() {
                    PackageTextInput::update_from_component(
                        &mut input,
                        component,
                        depth,
                        &self.typography,
                        &self.ui_theme,
                    );
                    // Re-record the commit intent; the `TextArea` id is stable
                    // across an in-place reconcile.
                    let area_id = input.widget.text_area.id();
                    if let Some(intent) = input.widget.intent.clone() {
                        self.text_input_intents.insert(area_id, intent);
                    }
                }
            }
            "dropdown" => {
                if let Some(mut dropdown) = widget.try_downcast::<PackageDropdown>() {
                    dropdown
                        .widget
                        .update_meta(component, &self.typography, &self.ui_theme);
                    dropdown.ctx.request_layout();
                }
            }
            "modal" => {
                if let Some(mut modal) = widget.try_downcast::<PackageModal>() {
                    modal
                        .widget
                        .update_meta(component, &self.typography, &self.ui_theme);
                    let keys = Self::container_keys(component);
                    {
                        let mut content = PackageModal::content_mut(&mut modal);
                        self.reconcile_flex_children(&mut content, component, keys, depth + 1);
                    }
                    modal.ctx.request_layout();
                }
            }
            _ => {
                if let Some(mut leaf) = widget.try_downcast::<PackageLeaf>() {
                    leaf.widget.update_from_component(
                        component,
                        depth,
                        &self.typography,
                        &self.ui_theme,
                    );
                    leaf.ctx.request_layout();
                }
            }
        }
    }

    /// The child key for a real component (id hash + kind).
    fn component_key(component: &PackageUiComponentTree) -> PackageChildKey {
        PackageChildKey::Component(
            stable_package_source_id(&component.id),
            component.kind.clone(),
        )
    }

    /// The ordered child keys for a container component.
    fn container_keys(component: &PackageUiComponentTree) -> Vec<PackageChildKey> {
        match component.kind.as_str() {
            "panel" => {
                let mut keys = Vec::new();
                if component.title.is_some() {
                    keys.push(PackageChildKey::PanelTitle);
                }
                keys.extend(component.children.iter().map(Self::component_key));
                keys
            }
            "list" => component
                .items
                .iter()
                .map(|item| PackageChildKey::ListRow(item.id.clone()))
                .collect(),
            _ => component.children.iter().map(Self::component_key).collect(),
        }
    }

    /// Reconcile a container's `Flex` child list in place (keyed diff; same
    /// ceiling as the SDUI reconciler — a reordered survivor is rebuilt).
    fn reconcile_flex_children(
        &mut self,
        flex: &mut WidgetMut<'_, Flex>,
        parent: &PackageUiComponentTree,
        new_keys: Vec<PackageChildKey>,
        depth: usize,
    ) {
        let parent_hash = stable_package_source_id(&parent.id);
        let parent_is_panel = parent.kind == "panel";
        let old_keys = self
            .child_keys
            .get(&parent_hash)
            .cloned()
            .unwrap_or_default();

        if old_keys == new_keys {
            for (i, key) in new_keys.iter().enumerate() {
                let child_depth = Self::child_depth(key, depth, parent_is_panel);
                if let Some(child) = Flex::child_mut(flex, i) {
                    self.reconcile_child_in_place(child, parent, key, child_depth);
                }
            }
            return;
        }

        let mut current = old_keys;
        for i in (0..current.len()).rev() {
            if !new_keys.contains(&current[i]) {
                Flex::remove_child(flex, i);
                current.remove(i);
            }
        }
        for (target, key) in new_keys.iter().enumerate() {
            let child_depth = Self::child_depth(key, depth, parent_is_panel);
            if current.get(target) == Some(key) {
                if let Some(child) = Flex::child_mut(flex, target) {
                    self.reconcile_child_in_place(child, parent, key, child_depth);
                }
            } else if let Some(from) = current.iter().position(|k| k == key) {
                Flex::remove_child(flex, from);
                current.remove(from);
                let child = self.build_child(parent, key, child_depth);
                Flex::insert_child(flex, target, child);
                current.insert(target, key.clone());
            } else {
                let child = self.build_child(parent, key, child_depth);
                Flex::insert_child(flex, target, child);
                current.insert(target, key.clone());
            }
        }
        self.child_keys.insert(parent_hash, new_keys);
    }

    /// The layout depth for a container child: a panel's title sits at the
    /// panel's depth, its component children one deeper; everything else keeps
    /// depth (mirrors the legacy renderer's indentation).
    fn child_depth(key: &PackageChildKey, depth: usize, parent_is_panel: bool) -> usize {
        match key {
            PackageChildKey::PanelTitle => depth,
            PackageChildKey::Component(..) if parent_is_panel => depth + 1,
            PackageChildKey::Component(..) | PackageChildKey::ListRow(_) => depth,
        }
    }

    /// Reconcile one container child in place from the parent's data.
    fn reconcile_child_in_place(
        &mut self,
        mut child: WidgetMut<'_, dyn Widget>,
        parent: &PackageUiComponentTree,
        key: &PackageChildKey,
        depth: usize,
    ) {
        match key {
            PackageChildKey::Component(hash, _) => {
                if let Some(child_component) = parent
                    .children
                    .iter()
                    .find(|c| stable_package_source_id(&c.id) == *hash)
                {
                    self.reconcile_component(child, child_component, depth);
                }
            }
            PackageChildKey::PanelTitle => {
                if let Some(mut leaf) = child.try_downcast::<PackageLeaf>() {
                    leaf.widget
                        .update_panel_title(parent, depth, &self.typography, &self.ui_theme);
                    leaf.ctx.request_layout();
                }
            }
            PackageChildKey::ListRow(item_id) => {
                if let Some(item) = parent.items.iter().find(|item| &item.id == item_id)
                    && let Some(mut row) = child.try_downcast::<PackageListRow>()
                {
                    row.widget.update_from_item(
                        parent,
                        item,
                        depth,
                        &self.typography,
                        &self.ui_theme,
                    );
                    row.ctx.request_layout();
                }
            }
        }
    }

    /// Build a fresh child widget for a container child key from the parent's data.
    fn build_child(
        &mut self,
        parent: &PackageUiComponentTree,
        key: &PackageChildKey,
        depth: usize,
    ) -> NewWidget<dyn Widget> {
        match key {
            PackageChildKey::Component(hash, _) => {
                let child_component = parent
                    .children
                    .iter()
                    .find(|c| stable_package_source_id(&c.id) == *hash)
                    .expect("child component present in its parent");
                self.build_component(child_component, depth)
            }
            PackageChildKey::PanelTitle => NewWidget::new(PackageLeaf::panel_title(
                parent,
                depth,
                self.typography.clone(),
                self.ui_theme.clone(),
            ))
            .erased(),
            PackageChildKey::ListRow(item_id) => {
                let item = parent
                    .items
                    .iter()
                    .find(|item| &item.id == item_id)
                    .cloned()
                    .unwrap_or(PackageUiListItem {
                        id: item_id.clone(),
                        label: String::new(),
                        detail: None,
                        action_command_id: None,
                        selected: false,
                        disabled: false,
                    });
                NewWidget::new(PackageListRow::from_item(
                    parent,
                    &item,
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased()
            }
        }
    }

    /// Drop identity records for components no longer reachable from the root.
    fn gc(&mut self, root: &PackageUiComponentTree) {
        let mut reachable = BTreeSet::new();
        let mut stack = vec![root];
        while let Some(component) = stack.pop() {
            if reachable.insert(stable_package_source_id(&component.id)) {
                stack.extend(component.children.iter());
            }
        }
        self.pods.retain(|hash, _| reachable.contains(hash));
        self.child_keys.retain(|hash, _| reachable.contains(hash));
    }
}

impl Widget for PackageRegionWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        if let Some(pod) = &mut self.root_pod {
            ctx.register_child(pod);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let Some(pod) = &mut self.root_pod else {
            return Size::ZERO;
        };
        let child_size = ctx.run_layout(pod, bc);
        ctx.place_child(pod, Point::ZERO);
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(child_size)
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {
        // The reconciled subtree paints itself through the Masonry render pass.
    }

    fn accessibility_role(&self) -> Role {
        if self.menu_a11y.is_some() {
            Role::Menu
        } else {
            Role::Group
        }
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        // Plan 070 step 13f + Phase 24.4: a hosted transient menu reports a
        // stable `Menu`/`MenuItem`/`Status` subtree built from the menu
        // payload. The reconciled pod ALWAYS stays attached (when present):
        // Masonry's walk emits every `children_ids` child and the consumer
        // rejects nodes the region does not attach, so the pod must be
        // listed even while the semantic menu nodes are exposed (the pod
        // subtree remains in the tree alongside them — the semantic nodes
        // are the screen-reader surface).
        let mut children = Vec::new();
        if let Some(pod) = &self.root_pod {
            children.push(pod.id().into());
        }
        if let Some(menu) = &self.menu_a11y {
            node.set_label(menu.prompt.clone());
            if menu.result_count.is_some() {
                node.set_modal();
            }
            for (index, item) in menu.items.iter().enumerate() {
                let id = crate::editor::accessibility::virtual_a11y_node_id(
                    ctx.widget_id(),
                    crate::editor::accessibility::virtual_a11y_slots::REGION_MENU_ITEM_BASE
                        + index as u16,
                );
                let mut item_node = Node::new(Role::MenuItem);
                item_node.set_label(item.label.clone());
                item_node.set_selected(item.selected);
                ctx.tree_update().nodes.push((id, item_node));
                children.push(id);
            }
            if let Some(status) = menu.result_count.as_ref().or(menu.status.as_ref()) {
                let id = crate::editor::accessibility::virtual_a11y_node_id(
                    ctx.widget_id(),
                    crate::editor::accessibility::virtual_a11y_slots::REGION_MENU_STATUS,
                );
                let mut status_node = Node::new(Role::Status);
                status_node.set_label(status.clone());
                if menu.result_count.is_some() {
                    status_node.set_live(Live::Polite);
                }
                ctx.tree_update().nodes.push((id, status_node));
                children.push(id);
            }
            node.set_children(children);
        } else {
            node.set_label("Package UI region");
            node.set_children(children);
        }
    }

    fn children_ids(&self) -> ChildrenIds {
        match &self.root_pod {
            Some(pod) => ChildrenIds::from_slice(&[pod.id()]),
            None => ChildrenIds::new(),
        }
    }
}

/// A text-only package leaf: `label`, `statusItem`, `editorView`, and the
/// synthetic panel title row. Paints via `paint_sdui_text` for parity with the
/// legacy renderer.
pub(crate) struct PackageLeaf {
    text: String,
    font_role: FontRole,
    variant: UiTextVariant,
    /// Panel title rows use the primary text color; other leaves use muted.
    title: bool,
    disabled: bool,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl PackageLeaf {
    #[allow(clippy::too_many_arguments)] // private builder; resolved by the from_* constructors
    fn new(
        text: String,
        font_role: FontRole,
        variant: UiTextVariant,
        title: bool,
        disabled: bool,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            text,
            font_role,
            variant,
            title,
            disabled,
            depth,
            typography,
            ui_theme,
        }
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(self.font_role, self.variant)
    }

    /// Resolve a text leaf from a `label`/`statusItem`/`editorView` component.
    fn from_component(
        component: &PackageUiComponentTree,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        let (text, font_role, variant) = match component.kind.as_str() {
            "statusItem" => (
                component
                    .text
                    .clone()
                    .or(component.label.clone())
                    .unwrap_or_else(|| component.id.clone()),
                component.font_role,
                component.text_variant.unwrap_or(style.status_text),
            ),
            // The legacy editorView paint forces Ui/body regardless of style.
            "editorView" => (
                format!("Editor view · {}", component.id),
                FontRole::Ui,
                style.body_text,
            ),
            _ => (
                component
                    .text
                    .clone()
                    .or(component.label.clone())
                    .unwrap_or_else(|| component.id.clone()),
                component.font_role,
                component.text_variant.unwrap_or(style.body_text),
            ),
        };
        Self::new(
            text,
            font_role,
            variant,
            false,
            component.disabled,
            depth,
            typography,
            ui_theme,
        )
    }

    /// Resolve the synthetic panel title row from a panel component.
    fn panel_title(
        component: &PackageUiComponentTree,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        Self::new(
            component.title.clone().unwrap_or_default(),
            component.font_role,
            component.text_variant.unwrap_or(style.title_text),
            true,
            component.disabled,
            depth,
            typography,
            ui_theme,
        )
    }

    /// An inert placeholder for the not-yet-migrated transient kinds.
    fn placeholder(
        component: &PackageUiComponentTree,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        Self::new(
            component
                .label
                .clone()
                .or(component.title.clone())
                .or(component.text.clone())
                .unwrap_or_else(|| component.id.clone()),
            component.font_role,
            component.text_variant.unwrap_or(style.body_text),
            false,
            true, // render transient placeholders dimmed until migrated
            depth,
            typography,
            ui_theme,
        )
    }

    fn update_from_component(
        &mut self,
        component: &PackageUiComponentTree,
        depth: usize,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        let updated = Self::from_component(component, depth, typography.clone(), ui_theme.clone());
        *self = updated;
    }

    fn update_panel_title(
        &mut self,
        component: &PackageUiComponentTree,
        depth: usize,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        let updated = Self::panel_title(component, depth, typography.clone(), ui_theme.clone());
        *self = updated;
    }
}

impl Widget for PackageLeaf {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let height = self.metrics().row_height;
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let color = if self.disabled {
            disabled_text_color(&self.ui_theme)
        } else if self.title {
            style.text_color
        } else {
            style.muted_text_color
        };
        paint_sdui_text(
            &self.typography,
            style.panel_padding,
            ctx,
            scene,
            &self.text,
            self.depth,
            0.0,
            ctx.size().width,
            0.0,
            self.font_role,
            self.metrics(),
            color,
        );
    }

    fn accessibility_role(&self) -> Role {
        Role::Label
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.text.clone());
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// Action emitted when a package button is activated (click, Enter/Space, or
/// accessibility click). Routed through the server-first command path in 13b+.
#[derive(Debug, Clone)]
pub struct PackageButtonPress {
    pub intent: SduiActionIntent,
}

/// A retained package `button`, mirroring `SduiButton`'s Masonry event/focus
/// plumbing while carrying the package action intent + disabled data.
pub(crate) struct PackageButton {
    label: String,
    intent: Option<SduiActionIntent>,
    font_role: FontRole,
    variant: UiTextVariant,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl PackageButton {
    fn from_component(
        component: &PackageUiComponentTree,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        let intent = match &component.action_command_id {
            Some(command_id) if !component.disabled => {
                Some(package_action_intent(command_id, &component.id))
            }
            _ => None,
        };
        Self {
            label: component
                .label
                .clone()
                .unwrap_or_else(|| component.id.clone()),
            intent,
            font_role: component.font_role,
            variant: component.text_variant.unwrap_or(style.body_text),
            depth,
            typography,
            ui_theme,
        }
    }

    fn update_from_component(
        &mut self,
        component: &PackageUiComponentTree,
        depth: usize,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        *self = Self::from_component(component, depth, typography.clone(), ui_theme.clone());
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(self.font_role, self.variant)
    }

    fn press(&self, ctx: &mut EventCtx<'_>) {
        if let Some(intent) = &self.intent {
            ctx.submit_action::<PackageButtonPress>(PackageButtonPress {
                intent: intent.clone(),
            });
        }
    }

    /// A button with no actionable intent (no command or disabled) paints and
    /// behaves as Disabled; otherwise it follows Masonry's pointer/focus state.
    fn interaction_state(&self, ctx: &PaintCtx<'_>) -> InteractionState {
        if self.intent.is_none() {
            InteractionState::Disabled
        } else if ctx.is_active() {
            InteractionState::Active
        } else if ctx.is_focus_target() {
            InteractionState::Focus
        } else if ctx.is_hovered() {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }
}

impl Widget for PackageButton {
    type Action = PackageButtonPress;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if self.intent.is_none() {
            return;
        }
        match event {
            PointerEvent::Down(..) => {
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(..) => {
                if ctx.is_active() && ctx.is_hovered() {
                    self.press(ctx);
                }
                ctx.request_paint_only();
            }
            _ => (),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(event)
                if event.state.is_up()
                    && (matches!(&event.key, Key::Character(c) if c == " ")
                        || event.key == Key::Named(NamedKey::Enter)) =>
            {
                self.press(ctx);
            }
            _ => (),
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == masonry::accesskit::Action::Click {
            self.press(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => {
                ctx.request_paint_only();
            }
            _ => {}
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let height = self.metrics().button_height();
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let width = ctx.size().width;
        let metrics = self.metrics();
        let height = metrics.button_height();
        let state = self.interaction_state(ctx);
        let rect = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, height);
        let fill = component_state_color(&self.ui_theme, "surface.control", state);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
        let text_color = if state == InteractionState::Disabled {
            disabled_text_color(&self.ui_theme)
        } else {
            style.text_color
        };
        paint_sdui_text(
            &self.typography,
            padding,
            ctx,
            scene,
            &self.label,
            self.depth,
            (height - metrics.line_height) / 2.0,
            width,
            0.0,
            self.font_role,
            metrics,
            text_color,
        );
        if state == InteractionState::Focus {
            paint_focus_ring(scene, rect, &self.ui_theme);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Button
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.label.clone());
        if self.intent.is_some() {
            node.add_action(masonry::accesskit::Action::Click);
        }
    }

    fn accepts_focus(&self) -> bool {
        self.intent.is_some()
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// Action emitted when a package list row is activated (click, Enter/Space, or
/// accessibility click). Routed through the server-first command path in 13b+.
#[derive(Debug, Clone)]
pub struct PackageListRowPress {
    pub intent: SduiActionIntent,
}

/// A retained package `list` row, mirroring `SduiListRow` while carrying the
/// package `selected`/`disabled` data the legacy renderer uses.
pub(crate) struct PackageListRow {
    label: String,
    detail: Option<String>,
    intent: Option<SduiActionIntent>,
    selected: bool,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl PackageListRow {
    fn from_item(
        component: &PackageUiComponentTree,
        item: &PackageUiListItem,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let intent = match &item.action_command_id {
            Some(command_id) if !item.disabled => {
                let source_id = format!("{}.{}", component.id, item.id);
                Some(package_action_intent(command_id, &source_id))
            }
            _ => None,
        };
        Self {
            label: item.label.clone(),
            detail: item.detail.clone(),
            intent,
            selected: item.selected,
            depth,
            typography,
            ui_theme,
        }
    }

    fn update_from_item(
        &mut self,
        component: &PackageUiComponentTree,
        item: &PackageUiListItem,
        depth: usize,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        *self = Self::from_item(component, item, depth, typography.clone(), ui_theme.clone());
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn body_metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(FontRole::Ui, self.style().body_text)
    }

    fn detail_metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Detail)
    }

    fn row_height(&self) -> f64 {
        self.body_metrics().list_height(self.detail_metrics())
    }

    fn press(&self, ctx: &mut EventCtx<'_>) {
        if let Some(intent) = &self.intent {
            ctx.submit_action::<PackageListRowPress>(PackageListRowPress {
                intent: intent.clone(),
            });
        }
    }

    fn interaction_state(&self, ctx: &PaintCtx<'_>) -> InteractionState {
        if self.intent.is_none() {
            InteractionState::Disabled
        } else if ctx.is_active() {
            InteractionState::Active
        } else if ctx.is_focus_target() {
            InteractionState::Focus
        } else if ctx.is_hovered() {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }
}

impl Widget for PackageListRow {
    type Action = PackageListRowPress;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if self.intent.is_none() {
            return;
        }
        match event {
            PointerEvent::Down(..) => {
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(..) => {
                if ctx.is_active() && ctx.is_hovered() {
                    self.press(ctx);
                }
                ctx.request_paint_only();
            }
            _ => (),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(event)
                if event.state.is_up()
                    && (matches!(&event.key, Key::Character(c) if c == " ")
                        || event.key == Key::Named(NamedKey::Enter)) =>
            {
                self.press(ctx);
            }
            _ => (),
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == masonry::accesskit::Action::Click {
            self.press(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => {
                ctx.request_paint_only();
            }
            _ => {}
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let height = self.row_height();
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let width = ctx.size().width;
        let body = self.body_metrics();
        let detail = self.detail_metrics();
        let height = body.list_height(detail);
        let state = self.interaction_state(ctx);
        let rect = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, height);
        let fill = list_row_fill_color(&self.ui_theme, state, self.selected);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
        let label_color = if state == InteractionState::Disabled {
            disabled_text_color(&self.ui_theme)
        } else {
            style.text_color
        };
        paint_sdui_text(
            &self.typography,
            padding,
            ctx,
            scene,
            &self.label,
            self.depth,
            0.0,
            width,
            0.0,
            FontRole::Ui,
            body,
            label_color,
        );
        if let Some(detail_text) = &self.detail {
            paint_sdui_text(
                &self.typography,
                padding,
                ctx,
                scene,
                detail_text,
                self.depth,
                body.line_height,
                width,
                0.0,
                FontRole::Ui,
                detail,
                style.muted_text_color,
            );
        }
        if state == InteractionState::Focus {
            paint_focus_ring(scene, rect, &self.ui_theme);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::ListItem
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.label.clone());
        if self.intent.is_some() {
            node.add_action(masonry::accesskit::Action::Click);
        }
    }

    fn accepts_focus(&self) -> bool {
        self.intent.is_some()
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// A retained package `collapse` section: a focusable title row plus a children
/// column, with the expanded/collapsed state held in the widget (retained across
/// reconciles via stable identity) instead of the legacy client `collapse_expanded`
/// map. Collapsed content stays laid out but is hidden by a clip path (title-row
/// height), so no show/hide re-registration is needed. Toggling is client-local
/// (matches the legacy behavior — collapse is not a server command).
pub(crate) struct PackageCollapse {
    title: String,
    disabled: bool,
    expanded: bool,
    content: WidgetPod<Flex>,
    font_role: FontRole,
    variant: UiTextVariant,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl PackageCollapse {
    fn from_component(
        component: &PackageUiComponentTree,
        content: Flex,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        Self {
            title: component
                .title
                .clone()
                .or(component.label.clone())
                .unwrap_or_else(|| component.id.clone()),
            disabled: component.disabled,
            // Collapsed by default, matching the legacy `collapse_expanded`
            // default (absent = collapsed).
            expanded: false,
            content: WidgetPod::new(content),
            font_role: component.font_role,
            variant: component.text_variant.unwrap_or(style.title_text),
            depth,
            typography,
            ui_theme,
        }
    }

    /// Update title/style on reconcile, preserving the retained `expanded` state.
    fn update_meta(
        &mut self,
        component: &PackageUiComponentTree,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        let style = SduiThemeStyle::from_ui_theme(ui_theme);
        self.title = component
            .title
            .clone()
            .or(component.label.clone())
            .unwrap_or_else(|| component.id.clone());
        self.disabled = component.disabled;
        self.font_role = component.font_role;
        self.variant = component.text_variant.unwrap_or(style.title_text);
        self.typography = typography.clone();
        self.ui_theme = ui_theme.clone();
    }

    /// Access the inner children column for in-place reconcile.
    pub(crate) fn content_mut<'w>(this: &'w mut WidgetMut<'_, Self>) -> WidgetMut<'w, Flex> {
        this.ctx.get_mut(&mut this.widget.content)
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn title_metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(self.font_role, self.variant)
    }

    fn toggle(&mut self, ctx: &mut EventCtx<'_>) {
        self.expanded = !self.expanded;
        ctx.request_layout();
        ctx.request_paint_only();
    }
}

impl Widget for PackageCollapse {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.content);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if self.disabled {
            return;
        }
        // Only the title row toggles; pointer events over the (expanded) children
        // belong to them even when they bubble up unhandled.
        let title_height = self.title_metrics().row_height;
        match event {
            PointerEvent::Down(e) if ctx.local_position(e.state.position).y < title_height => {
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(e)
                if ctx.is_active() && ctx.local_position(e.state.position).y < title_height =>
            {
                self.toggle(ctx);
            }
            _ => (),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if self.disabled {
            return;
        }
        match event {
            TextEvent::Keyboard(event)
                if event.state.is_up()
                    && (matches!(&event.key, Key::Character(c) if c == " ")
                        || event.key == Key::Named(NamedKey::Enter)) =>
            {
                self.toggle(ctx);
            }
            _ => (),
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if !self.disabled && event.action == masonry::accesskit::Action::Click {
            self.toggle(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::FocusChanged(_) | Update::DisabledChanged(_)) {
            ctx.request_paint_only();
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        let title_height = self.title_metrics().row_height;
        let content_bc =
            BoxConstraints::new(Size::new(width, 0.0), Size::new(width, f64::INFINITY));
        let content_size = ctx.run_layout(&mut self.content, &content_bc);
        ctx.place_child(&mut self.content, Point::new(0.0, title_height));
        let height = title_height
            + if self.expanded {
                content_size.height
            } else {
                0.0
            };
        // Hide collapsed children via the clip (title-row height); no show/hide
        // re-registration needed and hit-testing is clipped along with paint.
        ctx.set_clip_path(Rect::new(0.0, 0.0, width, height));
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let metrics = self.title_metrics();
        let width = ctx.size().width;
        let text_color = if self.disabled {
            disabled_text_color(&self.ui_theme)
        } else {
            style.text_color
        };
        paint_sdui_text(
            &self.typography,
            style.panel_padding,
            ctx,
            scene,
            &self.title,
            self.depth,
            0.0,
            width,
            0.0,
            self.font_role,
            metrics,
            text_color,
        );
        if ctx.is_focus_target() && !self.disabled {
            let rect = sdui_row_rect(
                style.panel_padding,
                self.depth,
                0.0,
                width,
                0.0,
                metrics.row_height,
            );
            paint_focus_ring(scene, rect, &self.ui_theme);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Button
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.title.clone());
        node.set_expanded(self.expanded);
        if !self.disabled {
            node.add_action(masonry::accesskit::Action::Click);
        }
    }

    fn accepts_focus(&self) -> bool {
        !self.disabled
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.content.id()])
    }
}

/// A package `dropdown` selection emitted when the user confirms an item
/// (Enter/Space on the open list, or clicking a row). Carries the confirmed
/// item's command intent; routed through the server-first command path in 13d.
#[derive(Debug, Clone)]
pub struct PackageDropdownSelect {
    pub intent: SduiActionIntent,
}

/// A retained package `dropdown` (plan 070 step 13d): a real ComboBox-role
/// widget replacing the client-local `dropdown_selected` map + the hand-rolled
/// `ui.dropdownToggle` keyboard route. The closed trigger shows the
/// selected item's label; clicking (or Enter/Space) opens the inline item list
/// below the trigger; ArrowUp/Down cycles the highlight; Enter/Space confirms
/// (emitting the item's command); Escape closes. Selection + open state are
/// widget-local and survive reconcile (stable identity), so no client-side map
/// is needed.
///
/// The open list is painted inline by this widget (matching the legacy
/// `paint_package_component` behavior); row hover/active come from the tracked
/// pointer position — the same pattern `SduiScrollViewport` uses for its
/// scrollbar — rather than from per-row child widgets, keeping the dropdown a
/// single self-contained widget.
pub(crate) struct PackageDropdown {
    fallback_label: String,
    items: Vec<DropdownItem>,
    selected: usize,
    open: bool,
    disabled: bool,
    font_role: FontRole,
    variant: UiTextVariant,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
    pointer_pos: Option<Point>,
    pointer_pressed: bool,
}

struct DropdownItem {
    label: String,
    intent: Option<SduiActionIntent>,
}

impl PackageDropdown {
    fn from_component(
        component: &PackageUiComponentTree,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        Self {
            fallback_label: component
                .label
                .clone()
                .or(component.text.clone())
                .or(component.title.clone())
                .unwrap_or_else(|| component.id.clone()),
            items: component
                .items
                .iter()
                .map(|item| DropdownItem {
                    label: item.label.clone(),
                    intent: item.action_command_id.as_ref().and_then(|command_id| {
                        (!item.disabled).then(|| {
                            package_action_intent(
                                command_id,
                                &format!("{}.{}", component.id, item.id),
                            )
                        })
                    }),
                })
                .collect(),
            // Honor the server-marked `selected` item as the initial selection,
            // falling back to the first item (the legacy `dropdown_selected`
            // default).
            selected: component.items.iter().position(|i| i.selected).unwrap_or(0),
            open: false,
            disabled: component.disabled,
            font_role: component.font_role,
            variant: component.text_variant.unwrap_or(style.body_text),
            depth,
            typography,
            ui_theme,
            pointer_pos: None,
            pointer_pressed: false,
        }
    }

    /// Update items/style on reconcile, preserving the retained `selected` +
    /// `open` state (clamped if the item list shrank).
    fn update_meta(
        &mut self,
        component: &PackageUiComponentTree,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        let selected = self.selected;
        let open = self.open;
        let pointer_pos = self.pointer_pos;
        let pointer_pressed = self.pointer_pressed;
        *self = Self::from_component(component, self.depth, typography.clone(), ui_theme.clone());
        self.selected = selected.min(self.items.len().saturating_sub(1));
        self.open = open;
        self.pointer_pos = pointer_pos;
        self.pointer_pressed = pointer_pressed;
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(self.font_role, self.variant)
    }

    fn trigger_height(&self) -> f64 {
        self.metrics().button_height()
    }

    fn row_height(&self) -> f64 {
        self.metrics().row_height
    }

    fn selected_label(&self) -> &str {
        self.items
            .get(self.selected)
            .map(|item| item.label.as_str())
            .unwrap_or(&self.fallback_label)
    }

    /// The open-list row under local y (below the trigger), if open.
    fn row_at(&self, local_y: f64) -> Option<usize> {
        if !self.open || local_y < self.trigger_height() {
            return None;
        }
        let idx = ((local_y - self.trigger_height()) / self.row_height()) as usize;
        (idx < self.items.len()).then_some(idx)
    }

    fn row_at_pointer(&self) -> Option<usize> {
        self.pointer_pos.and_then(|p| self.row_at(p.y))
    }

    fn over_trigger(&self) -> bool {
        self.pointer_pos
            .is_some_and(|p| p.y < self.trigger_height())
    }

    fn trigger_state(&self, ctx: &PaintCtx<'_>) -> InteractionState {
        if self.disabled {
            InteractionState::Disabled
        } else if self.pointer_pressed && self.over_trigger() {
            InteractionState::Active
        } else if ctx.is_focus_target() {
            InteractionState::Focus
        } else if self.over_trigger() {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }

    fn row_state(&self, idx: usize) -> InteractionState {
        if self.disabled {
            InteractionState::Disabled
        } else if self.pointer_pressed && self.row_at_pointer() == Some(idx) {
            InteractionState::Active
        } else if self.row_at_pointer() == Some(idx) {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }

    fn set_open(&mut self, ctx: &mut EventCtx<'_>, open: bool) {
        if self.open != open {
            self.open = open;
            // Opening/closing changes this widget's height (the inline list), so
            // the parent column must re-layout.
            ctx.request_layout();
        }
    }

    fn cycle(&mut self, ctx: &mut EventCtx<'_>, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len();
        self.selected = if delta >= 0 {
            (self.selected + 1) % n
        } else {
            self.selected.checked_sub(1).unwrap_or(n - 1)
        };
        // Reveal the list while navigating so the highlight is visible.
        if !self.open {
            self.open = true;
            ctx.request_layout();
        } else {
            ctx.request_paint_only();
        }
    }

    /// Confirm the highlighted item: emit its command (if any) and close.
    fn confirm(&mut self, ctx: &mut EventCtx<'_>) {
        if let Some(intent) = self
            .items
            .get(self.selected)
            .and_then(|item| item.intent.clone())
        {
            ctx.submit_action::<PackageDropdownSelect>(PackageDropdownSelect { intent });
        }
        self.set_open(ctx, false);
    }

    /// Enter/Space: open when closed, confirm when open.
    fn activate(&mut self, ctx: &mut EventCtx<'_>) {
        if self.open {
            self.confirm(ctx);
        } else {
            self.set_open(ctx, true);
        }
    }
}

impl Widget for PackageDropdown {
    type Action = PackageDropdownSelect;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(e) => {
                if self.disabled {
                    return;
                }
                self.pointer_pos = Some(ctx.local_position(e.state.position));
                self.pointer_pressed = true;
                // Take keyboard focus on click so ArrowUp/Down + Enter/Space nav
                // reaches this widget.
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(e) => {
                let local_y = ctx.local_position(e.state.position).y;
                let was_pressed = self.pointer_pressed;
                self.pointer_pressed = false;
                ctx.request_paint_only();
                if self.disabled || !was_pressed {
                    return;
                }
                if local_y < self.trigger_height() {
                    let next = !self.open;
                    self.set_open(ctx, next);
                } else if let Some(idx) = self.row_at(local_y) {
                    self.selected = idx;
                    self.confirm(ctx);
                }
                ctx.set_handled();
            }
            PointerEvent::Move(e) => {
                self.pointer_pos = Some(ctx.local_position(e.current.position));
                ctx.request_paint_only();
            }
            PointerEvent::Leave(..) | PointerEvent::Cancel(..) => {
                self.pointer_pos = None;
                self.pointer_pressed = false;
                ctx.request_paint_only();
            }
            _ => (),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if self.disabled {
            return;
        }
        let TextEvent::Keyboard(event) = event else {
            return;
        };
        match (&event.key, event.state) {
            (Key::Named(NamedKey::ArrowDown), KeyState::Down) => self.cycle(ctx, 1),
            (Key::Named(NamedKey::ArrowUp), KeyState::Down) => self.cycle(ctx, -1),
            (Key::Named(NamedKey::Enter), KeyState::Up) => self.activate(ctx),
            (Key::Character(c), KeyState::Up) if c == " " => self.activate(ctx),
            (Key::Named(NamedKey::Escape), KeyState::Down) if self.open => {
                self.set_open(ctx, false);
            }
            _ => return,
        }
        ctx.set_handled();
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if !self.disabled && event.action == masonry::accesskit::Action::Click {
            self.activate(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => {
                ctx.request_paint_only();
            }
            _ => {}
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        let height = self.trigger_height()
            + if self.open {
                self.items.len() as f64 * self.row_height()
            } else {
                0.0
            };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let width = ctx.size().width;
        let metrics = self.metrics();
        let trigger_height = metrics.button_height();
        let row_height = metrics.row_height;
        // Trigger row.
        let trigger_state = self.trigger_state(ctx);
        let trigger_rect = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, trigger_height);
        let fill = component_state_color(&self.ui_theme, "surface.control", trigger_state);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &trigger_rect);
        let text_color = if trigger_state == InteractionState::Disabled {
            disabled_text_color(&self.ui_theme)
        } else {
            style.text_color
        };
        let selected_label = self.selected_label().to_string();
        paint_sdui_text(
            &self.typography,
            padding,
            ctx,
            scene,
            &selected_label,
            self.depth,
            (trigger_height - metrics.line_height) / 2.0,
            width,
            0.0,
            self.font_role,
            metrics,
            text_color,
        );
        if trigger_state == InteractionState::Focus {
            paint_focus_ring(scene, trigger_rect, &self.ui_theme);
        }
        // Inline open list.
        if self.open {
            for (idx, item) in self.items.iter().enumerate() {
                let row_y = trigger_height + idx as f64 * row_height;
                let row_rect = sdui_row_rect(padding, self.depth, row_y, width, 0.0, row_height);
                let row_fill =
                    list_row_fill_color(&self.ui_theme, self.row_state(idx), idx == self.selected);
                scene.fill(Fill::NonZero, Affine::IDENTITY, row_fill, None, &row_rect);
                paint_sdui_text(
                    &self.typography,
                    padding,
                    ctx,
                    scene,
                    &item.label,
                    self.depth,
                    row_y + (row_height - metrics.line_height) / 2.0,
                    width,
                    0.0,
                    self.font_role,
                    metrics,
                    text_color,
                );
            }
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::ComboBox
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.fallback_label.clone());
        node.set_value(self.selected_label().to_string());
        node.set_expanded(self.open);
        if !self.disabled {
            node.add_action(masonry::accesskit::Action::Click);
        }
    }

    fn accepts_focus(&self) -> bool {
        !self.disabled
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// Emitted when a retained `modal` requests dismissal (Escape / backdrop
/// click). Carries the modal component's id hash. The overlay host treats this
/// as the modal's dismiss signal (step 13e); the overlay's removal stays
/// server-driven (the package re-renders without it), so this action is the
/// client-side hook a real package modal would route.
#[derive(Debug)]
pub(crate) struct PackageModalDismiss {
    pub(crate) id_hash: u64,
}

/// A retained package `modal` (plan 070 step 13e): a focus-trapped `Role::Dialog`
/// surface wrapping its reconciled children. Tab/Shift+Tab cycles focus among
/// the modal's focusable descendants (the reconciler records their widget ids in
/// `focusable`) instead of leaking to the editor/sidebar — the modal handles the
/// bubbled Tab and marks it handled so Masonry's global Tab traversal (a
/// fallback for unhandled Tabs) never runs. Escape emits `PackageModalDismiss`.
///
/// ponytail: the trap tracks `focus_index` rather than querying which child is
/// focused (Masonry exposes no per-child focus query from a widget), so a
/// pointer-click that focuses a child directly desyncs the next Tab's start
/// point by one step. Upgrade path: sync `focus_index` from focus events when a
/// real package modal ships (none do yet — the only modal-focus overlay is the
/// transient menu, whose focus is editor-driven, not this widget).
pub(crate) struct PackageModal {
    id_hash: u64,
    title: String,
    disabled: bool,
    content: WidgetPod<Flex>,
    /// Focusable descendant widget ids in tree order, recorded by the
    /// reconciler (buttons, list rows, collapses, dropdowns, text areas).
    focusable: Vec<WidgetId>,
    focus_index: Option<usize>,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl PackageModal {
    fn from_component(
        component: &PackageUiComponentTree,
        content: Flex,
        focusable: Vec<WidgetId>,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            id_hash: stable_package_source_id(&component.id),
            title: component
                .title
                .clone()
                .or(component.label.clone())
                .unwrap_or_else(|| component.id.clone()),
            disabled: component.disabled,
            content: WidgetPod::new(content),
            focusable,
            focus_index: None,
            depth,
            typography,
            ui_theme,
        }
    }

    /// Update title/style on reconcile, preserving the retained focus position
    /// and the built `focusable` set (stable-identity reconcile keeps surviving
    /// descendants' widget ids, so the built set stays valid; added/removed
    /// focusable children across a reconcile are the documented ponytail
    /// ceiling).
    fn update_meta(
        &mut self,
        component: &PackageUiComponentTree,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        self.title = component
            .title
            .clone()
            .or(component.label.clone())
            .unwrap_or_else(|| component.id.clone());
        self.disabled = component.disabled;
        self.typography = typography.clone();
        self.ui_theme = ui_theme.clone();
    }

    /// Access the inner children column for in-place reconcile.
    pub(crate) fn content_mut<'w>(this: &'w mut WidgetMut<'_, Self>) -> WidgetMut<'w, Flex> {
        this.ctx.get_mut(&mut this.widget.content)
    }

    /// Move focus to the next/previous focusable descendant (wrapping),
    /// trapping Tab within the dialog.
    fn cycle_focus(&mut self, ctx: &mut EventCtx<'_>, forward: bool) {
        if self.focusable.is_empty() {
            return;
        }
        let len = self.focusable.len();
        let next = match self.focus_index {
            None => 0,
            Some(i) if forward => (i + 1) % len,
            Some(i) => (i + len - 1) % len,
        };
        self.focus_index = Some(next);
        ctx.set_focus(self.focusable[next]);
        ctx.request_paint_only();
    }

    fn dismiss(&mut self, ctx: &mut EventCtx<'_>) {
        ctx.resign_focus();
        self.focus_index = None;
        ctx.submit_action::<PackageModalDismiss>(PackageModalDismiss {
            id_hash: self.id_hash,
        });
        ctx.set_handled();
    }
}

impl Widget for PackageModal {
    type Action = PackageModalDismiss;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.content);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        // A pointer Down anywhere inside the dialog keeps focus within it: take
        // focus when nothing inside is focused yet, so Tab/keyboard nav has a
        // starting point. Children that handle their own clicks still get them
        // (this runs on the bubbled event only when the child did not handle).
        if let PointerEvent::Down(..) = event
            && !self.disabled
            && !ctx.has_focus_target()
        {
            ctx.request_focus();
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if self.disabled {
            return;
        }
        let TextEvent::Keyboard(key) = event else {
            return;
        };
        if key.state != KeyState::Down {
            return;
        }
        match &key.key {
            Key::Named(NamedKey::Tab) => {
                self.cycle_focus(ctx, !key.modifiers.shift());
                ctx.set_handled();
            }
            Key::Named(NamedKey::Escape) => {
                self.dismiss(ctx);
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::DisabledChanged(_)) {
            ctx.request_paint_only();
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let content_size = ctx.run_layout(&mut self.content, bc);
        ctx.place_child(&mut self.content, Point::ZERO);
        bc.constrain(content_size)
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {
        // The overlay host paints the tooltip-shell chrome behind the dialog;
        // the children paint their own content in the child pass.
    }

    fn accessibility_role(&self) -> Role {
        Role::Dialog
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.title.clone());
        node.set_modal();
    }

    fn accepts_focus(&self) -> bool {
        !self.disabled
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.content.id()])
    }
}

/// A genuinely-editable package `textInput` (plan 070 step 13c): a retained
/// Masonry `TextArea<true>` (single-line, so Enter commits rather than inserting
/// a newline) wrapped in Clay-owned chrome paint. The `TextArea` supplies
/// editing/selection/clipboard/IME and reports `Role::TextInput`; Clay paints
/// the background, the validation/focus border, and the placeholder from theme
/// tokens. Editing is optimistic-local (the `TextArea` updates itself per
/// keystroke); the committed value (Enter) reaches the server through the
/// region's `text_input_intents` map, and the server stays authoritative — a
/// changed `component.text` is adopted on reconcile when the field is not
/// focused (revert-on-reject without clobbering an in-progress edit).
pub(crate) struct PackageTextInput {
    text_area: WidgetPod<TextArea<true>>,
    /// Base commit intent (value argument appended at commit); `None` when
    /// disabled so a disabled field never routes a command.
    intent: Option<SduiActionIntent>,
    placeholder: String,
    validation_state: Option<String>,
    /// Last `component.text` seen so reconcile adopts only on an actual change.
    server_text: Option<String>,
    disabled: bool,
    /// Whether the inner `TextArea` currently holds focus (drives the focus
    /// border + guards server-value adoption).
    is_focused: bool,
    /// Whether the field is empty (drives placeholder paint), refreshed in layout.
    is_empty: bool,
    font_role: FontRole,
    variant: UiTextVariant,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl PackageTextInput {
    fn from_component(
        component: &PackageUiComponentTree,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> (Self, WidgetId) {
        let style = SduiThemeStyle::from_ui_theme(&ui_theme);
        let initial = component.text.clone().unwrap_or_default();
        let area = NewWidget::new_with_props(
            TextArea::new_editable(&initial),
            Self::text_props(&ui_theme),
        );
        let area_id = area.id();
        (
            Self {
                text_area: area.to_pod(),
                intent: Self::intent_for(component),
                placeholder: Self::placeholder_for(component),
                validation_state: component.validation_state.clone(),
                server_text: component.text.clone(),
                disabled: component.disabled,
                is_focused: false,
                is_empty: initial.is_empty(),
                font_role: component.font_role,
                variant: component.text_variant.unwrap_or(style.body_text),
                depth,
                typography,
                ui_theme,
            },
            area_id,
        )
    }

    fn intent_for(component: &PackageUiComponentTree) -> Option<SduiActionIntent> {
        if component.disabled {
            None
        } else {
            Some(package_action_intent(
                component
                    .action_command_id
                    .as_deref()
                    .unwrap_or("ui.textInputCommit"),
                &component.id,
            ))
        }
    }

    fn placeholder_for(component: &PackageUiComponentTree) -> String {
        component
            .label
            .clone()
            .or_else(|| component.title.clone())
            .unwrap_or_default()
    }

    /// Theme-driven text/caret/selection colors for the inner `TextArea`.
    fn text_props(ui_theme: &ResolvedUiTheme) -> Properties {
        let text = ui_theme.color("text.primary").unwrap_or(Color::WHITE);
        let selection = ui_theme.color("surface.selected").unwrap_or(Color::WHITE);
        let mut props = Properties::new();
        props.insert(ContentColor::new(text));
        props.insert(CaretColor { color: text });
        props.insert(SelectionColor { color: selection });
        props
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(self.font_role, self.variant)
    }

    /// Border color precedence: validation state > focus > subtle (step 13c).
    fn border_color(&self) -> Color {
        match self.validation_state.as_deref() {
            Some("error") => self.ui_theme.color("diagnostic.error"),
            Some("warning") => self.ui_theme.color("diagnostic.warning"),
            Some("success") => self.ui_theme.color("diagnostic.success"),
            _ if self.is_focused => self.ui_theme.color("border.focus"),
            _ => self.ui_theme.color("border.subtle"),
        }
        .unwrap_or(Color::TRANSPARENT)
    }

    /// Re-apply theme/meta in place (stable identity preserved) and adopt a
    /// changed server value when the field is not focused.
    fn update_from_component(
        this: &mut WidgetMut<'_, Self>,
        component: &PackageUiComponentTree,
        depth: usize,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
    ) {
        let style = SduiThemeStyle::from_ui_theme(ui_theme);
        this.widget.intent = Self::intent_for(component);
        this.widget.placeholder = Self::placeholder_for(component);
        this.widget.validation_state = component.validation_state.clone();
        this.widget.disabled = component.disabled;
        this.widget.font_role = component.font_role;
        this.widget.variant = component.text_variant.unwrap_or(style.body_text);
        this.widget.depth = depth;
        this.widget.typography = typography.clone();
        this.widget.ui_theme = ui_theme.clone();
        // Server authority: adopt a changed `component.text`, but never clobber
        // an in-progress edit.
        if this.widget.server_text != component.text {
            this.widget.server_text = component.text.clone();
            if !this.widget.is_focused {
                let text = component.text.clone().unwrap_or_default();
                let mut area = this.ctx.get_mut(&mut this.widget.text_area);
                TextArea::reset_text(&mut area, &text);
            }
        }
        // Re-apply theme-driven text colors on the inner `TextArea`.
        {
            let text = ui_theme.color("text.primary").unwrap_or(Color::WHITE);
            let selection = ui_theme.color("surface.selected").unwrap_or(Color::WHITE);
            let mut area = this.ctx.get_mut(&mut this.widget.text_area);
            area.insert_prop(ContentColor::new(text));
            area.insert_prop(CaretColor { color: text });
            area.insert_prop(SelectionColor { color: selection });
        }
        this.ctx.request_layout();
    }
}

impl Widget for PackageTextInput {
    // The inner `TextArea` emits `TextAction` (commit/change) directly with its
    // own widget id; this wrapper emits no action of its own.
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.text_area);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::ChildFocusChanged(focused) = event {
            self.is_focused = *focused;
            ctx.request_paint_only();
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let style = self.style();
        let padding = style.panel_padding;
        let metrics = self.metrics();
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        let height = metrics.button_height();
        let field = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, height);
        let hpad = 6.0;
        let area_width = (field.width() - hpad * 2.0).max(1.0);
        // An empty `TextArea` computes a zero content height, so pin it to the
        // line height — otherwise the field has no hit area to click into.
        let area_bc = BoxConstraints::new(
            Size::new(area_width, metrics.line_height),
            Size::new(area_width, metrics.line_height),
        );
        let _ = ctx.run_layout(&mut self.text_area, &area_bc);
        ctx.place_child(
            &mut self.text_area,
            Point::new(field.x0 + hpad, (height - metrics.line_height) / 2.0),
        );
        self.is_empty = ctx.get_raw(&mut self.text_area).0.is_empty();
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let metrics = self.metrics();
        let width = ctx.size().width;
        let height = metrics.button_height();
        let rect = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, height);
        let fill = component_state_color(
            &self.ui_theme,
            "surface.control",
            if self.disabled {
                InteractionState::Disabled
            } else {
                InteractionState::Rest
            },
        );
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
        // Border: validation state > focus > subtle (plan 070 step 13c).
        let border_color = self.border_color();
        scene.stroke(
            &Stroke::new(1.0),
            Affine::IDENTITY,
            border_color,
            None,
            &rect,
        );
        // Placeholder hint when the field is empty.
        if self.is_empty && !self.placeholder.is_empty() {
            let color = if self.disabled {
                disabled_text_color(&self.ui_theme)
            } else {
                self.ui_theme
                    .color("text.muted")
                    .unwrap_or(Color::TRANSPARENT)
            };
            paint_sdui_text(
                &self.typography,
                padding,
                ctx,
                scene,
                &self.placeholder,
                self.depth,
                (height - metrics.line_height) / 2.0,
                width,
                0.0,
                self.font_role,
                metrics,
                color,
            );
        }
    }

    fn accessibility_role(&self) -> Role {
        // The inner `TextArea` reports `Role::TextInput`; this wrapper groups it.
        Role::Group
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        if !self.placeholder.is_empty() {
            node.set_label(self.placeholder.clone());
        }
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.text_area.id()])
    }
}

/// A hosted fixed panel: its slot identity plus the reconciled region child.
struct HostedPanel {
    slot_id: FixedSlotId,
    pod: WidgetPod<PackageRegionWidget>,
}

/// Hosts the package_ui *fixed panels* as real Masonry children (plan 070 step
/// 13b), replacing the legacy `paint_package_fixed_panels` immediate-mode pass.
/// The host fills the working area (so its panel-region children can be placed at
/// their absolute slot rects) but paints only the panel chrome — the editor and
/// SDUI sidebar show through, and pointer events outside a panel region bubble up
/// to `EditorWidget` (the SDUI region child is ordered above it so the sidebar
/// keeps priority). Each visible panel is a `PackageRegionWidget` reconciled in
/// place by slot id, so surviving panels keep widget identity (and `collapse`
/// expanded state) across package_ui updates.
pub(crate) struct PackagePanelHost {
    panels: Vec<HostedPanel>,
    /// Cloned package_ui state for layout-time rect computation (rects depend on
    /// the working-area size, known only in layout).
    package_ui: PackageUiRuntimeState,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
    /// Panel rects computed in layout, read in paint for the chrome.
    panel_rects: Vec<(FixedSlotId, Rect)>,
}

impl PackagePanelHost {
    pub(crate) fn new() -> Self {
        Self {
            panels: Vec::new(),
            package_ui: PackageUiRuntimeState::default(),
            typography: TypographyRegistry::default(),
            ui_theme: ResolvedUiTheme::default(),
            panel_rects: Vec::new(),
        }
    }

    /// Route a package `textInput` commit (Enter) to its server intent,
    /// appending the committed `value`. `area_id` is the inner `TextArea`'s
    /// widget id (the source id Masonry reports on `TextAction::Entered`).
    /// Searches every hosted panel's reconciled region (step 13c).
    pub(crate) fn text_input_commit(
        this: &mut WidgetMut<'_, Self>,
        area_id: WidgetId,
        value: &str,
    ) -> Option<SduiActionIntent> {
        for hosted in &mut this.widget.panels {
            let intent = {
                let region = this.ctx.get_mut(&mut hosted.pod);
                region.widget.text_input_commit(area_id, value)
            };
            if intent.is_some() {
                return intent;
            }
        }
        None
    }

    /// Reconcile the hosted panel set against the latest package_ui state.
    /// Existing panels reconcile their region in place; new panels build fresh;
    /// removed panels are dropped. Called from `EditorWidget::sync_panels` with a
    /// live `MutateCtx` when package_ui/theme/typography changed.
    pub(crate) fn sync_panels(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        package_ui: &PackageUiRuntimeState,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.package_ui = package_ui.clone();
        self.typography = typography;
        self.ui_theme = ui_theme;
        let typography = self.typography.clone();
        let ui_theme = self.ui_theme.clone();
        let visible: Vec<(FixedSlotId, FixedPackagePanel)> = self
            .package_ui
            .visible_fixed_panel_components()
            .into_iter()
            .map(|(slot_id, panel)| (slot_id, panel.clone()))
            .collect();

        let existing = std::mem::take(&mut self.panels);
        let mut kept: Vec<HostedPanel> = Vec::with_capacity(visible.len());
        let mut changed = false;
        for hosted in existing {
            if visible
                .iter()
                .any(|(slot_id, _)| *slot_id == hosted.slot_id)
            {
                kept.push(hosted);
            } else {
                ctx.remove_child(hosted.pod);
                changed = true;
            }
        }
        for (slot_id, panel) in &visible {
            if let Some(hosted) = kept.iter_mut().find(|h| h.slot_id == *slot_id) {
                let mut region = ctx.get_mut(&mut hosted.pod);
                region
                    .widget
                    .set_render_context(typography.clone(), ui_theme.clone());
                region
                    .widget
                    .reconcile_tree_live(&mut region.ctx, &panel.component);
            } else {
                let mut region = PackageRegionWidget::new();
                region.set_render_context(typography.clone(), ui_theme.clone());
                region.reconcile_tree(&panel.component);
                kept.push(HostedPanel {
                    slot_id: *slot_id,
                    pod: WidgetPod::new(region),
                });
                changed = true;
            }
        }
        self.panels = kept;
        if changed {
            ctx.children_changed();
        }
        ctx.request_layout();
    }

    /// Compute the panel rects for a working-area size (also stored for paint).
    fn compute_rects(&mut self, size: Size) {
        let defaults = self.ui_theme.panel_defaults();
        self.panel_rects = self
            .package_ui
            .visible_fixed_panels(size.to_rect(), &defaults)
            .into_iter()
            .map(|(rect, panel)| (panel.slot_id, rect))
            .collect();
    }
}

impl Widget for PackagePanelHost {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        for hosted in &mut self.panels {
            ctx.register_child(&mut hosted.pod);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        };
        self.compute_rects(size);
        let padding = SduiThemeStyle::from_ui_theme(&self.ui_theme).panel_padding;
        for hosted in &mut self.panels {
            let Some((_, rect)) = self
                .panel_rects
                .iter()
                .find(|(slot_id, _)| *slot_id == hosted.slot_id)
            else {
                continue;
            };
            // Content area: the panel rect below its top padding (parity with the
            // legacy cursor_y = rect.y0 + panel_padding content start).
            let content_size = Size::new(rect.width(), (rect.height() - padding).max(1.0));
            let _ = ctx.run_layout(
                &mut hosted.pod,
                &BoxConstraints::new(content_size, content_size),
            );
            ctx.place_child(&mut hosted.pod, Point::new(rect.x0, rect.y0 + padding));
        }
        size
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        // Panel chrome (bg/border) behind each panel-region child; the children
        // paint their component content on top during the child pass.
        for (_, rect) in &self.panel_rects {
            paint_panel_chrome(
                scene,
                *rect,
                &PanelChrome {
                    title: None,
                    collapse: InteractionState::Rest,
                    resize: InteractionState::Rest,
                },
                &self.ui_theme,
            );
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Group
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        // Children (the panel regions) flow into the a11y tree by default.
        node.set_label("Package panels");
    }

    fn children_ids(&self) -> ChildrenIds {
        self.panels.iter().map(|hosted| hosted.pod.id()).collect()
    }
}

/// One transient overlay hosted as a child: its reconciled component region
/// plus the anchor + stacking metadata used to place and layer it.
struct HostedOverlay {
    id: String,
    anchor: PackageOverlayAnchor,
    completion_item_count: usize,
    z_order: u8,
    pod: WidgetPod<PackageRegionWidget>,
}

fn menu_item_count(component: &PackageUiComponentTree) -> usize {
    if component.kind == "list" {
        return component.items.len();
    }
    component.children.iter().map(menu_item_count).sum()
}

/// Hosts the transient overlays (package transient overlays + the active menu
/// projected as one) as real Masonry children layered *above* the SDUI region
/// (plan 070 step 13e, "Composition A"). Each overlay is a `PackageRegionWidget`
/// reconciled in place by overlay id, placed at its anchor rect (sized to it so
/// it does not block the region outside — the Step-7 bounding-rect caveat) and
/// stacked `z.overlay` < `z.modal` < `z.tooltip` (children_ids order; Masonry
/// paints first→last and hit-tests in reverse, so the last child is topmost).
/// The host fills its parent bounds for absolute placement. Editor-local hosts
/// paint only tooltip-shell chrome and remain pointer-transparent outside their
/// overlay rects; the centered root-layer host additionally paints one full
/// window scrim and shields the base layer.
pub(crate) struct PackageOverlayHost {
    overlays: Vec<HostedOverlay>,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
    /// The editor main rect, shared with `EditorWidget` (computed there from the
    /// SDUI sidebar geometry) so main-pane-anchored overlays resolve correctly.
    /// Set during `EditorWidget`'s layout before this host's children are laid
    /// out.
    main_rect: Rc<Cell<Rect>>,
    /// Current pane-local caret bounds for the Clay-native completion anchor.
    completion_anchor: Rc<Cell<Option<Rect>>>,
    /// Overlay rects computed in layout, read in paint for the chrome.
    overlay_rects: Vec<Rect>,
    /// Whether this host is the Clay-owned window-level centered layer.
    centered: bool,
    /// Full root bounds cached by layout for the centered scrim fill.
    window_rect: Rect,
    /// Sanitized accessible name for the centered dialog.
    dialog_label: Option<String>,
    /// Focus target to restore after scrim pointer events. The centered menu
    /// routes keys through the originating pane instead of creating a second
    /// focus stack.
    focus_restore_target: Option<WidgetId>,
}

impl PackageOverlayHost {
    pub(crate) fn new(main_rect: Rc<Cell<Rect>>) -> Self {
        Self::with_completion_anchor(main_rect, Rc::new(Cell::new(None)))
    }

    pub(crate) fn with_completion_anchor(
        main_rect: Rc<Cell<Rect>>,
        completion_anchor: Rc<Cell<Option<Rect>>>,
    ) -> Self {
        Self {
            overlays: Vec::new(),
            typography: TypographyRegistry::default(),
            ui_theme: ResolvedUiTheme::default(),
            main_rect,
            completion_anchor,
            overlay_rects: Vec::new(),
            centered: false,
            window_rect: Rect::ZERO,
            dialog_label: None,
            focus_restore_target: None,
        }
    }

    /// Create the Clay-owned root-layer host for a centered Command Centre
    /// menu. Its root bounds are the window, not an editor pane.
    pub(crate) fn new_centered() -> Self {
        Self {
            centered: true,
            ..Self::new(Rc::new(Cell::new(Rect::ZERO)))
        }
    }

    pub(crate) fn set_focus_restore_target(&mut self, target: Option<WidgetId>) {
        if self.focus_restore_target.is_none() {
            self.focus_restore_target = target;
        }
    }

    /// Reconcile the hosted overlay set against the latest transient overlays.
    /// Existing overlays reconcile their region in place (keeping widget
    /// identity + widget-local state such as dropdown selection); new overlays
    /// build fresh; removed overlays are dropped. Called from
    /// `EditorWidget::sync_overlays` with a live `MutateCtx`.
    pub(crate) fn sync_overlays(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        overlays: Vec<TransientPackageOverlay>,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.typography = typography;
        self.ui_theme = ui_theme;
        self.dialog_label = self.centered.then(|| {
            overlays
                .iter()
                .find_map(|overlay| overlay.menu_a11y.as_ref().map(|menu| menu.prompt.clone()))
                .unwrap_or_else(|| "Command Centre".to_string())
        });
        let typography = self.typography.clone();
        let ui_theme = self.ui_theme.clone();

        let existing = std::mem::take(&mut self.overlays);
        let mut kept: Vec<HostedOverlay> = Vec::with_capacity(overlays.len());
        let mut changed = false;
        for hosted in existing {
            if overlays.iter().any(|o| o.id == hosted.id) {
                kept.push(hosted);
            } else {
                ctx.remove_child(hosted.pod);
                changed = true;
            }
        }
        for overlay in &overlays {
            if let Some(hosted) = kept.iter_mut().find(|h| h.id == overlay.id) {
                hosted.anchor = overlay.anchor;
                hosted.completion_item_count = menu_item_count(&overlay.component);
                hosted.z_order = overlay_z_order(overlay.z_level_token);
                let mut region = ctx.get_mut(&mut hosted.pod);
                region
                    .widget
                    .set_render_context(typography.clone(), ui_theme.clone());
                region.widget.set_menu_a11y(overlay.menu_a11y.clone());
                region
                    .widget
                    .reconcile_tree_live(&mut region.ctx, &overlay.component);
                region.ctx.request_accessibility_update();
            } else {
                let mut region = PackageRegionWidget::new();
                region.set_render_context(typography.clone(), ui_theme.clone());
                region.set_menu_a11y(overlay.menu_a11y.clone());
                region.reconcile_tree(&overlay.component);
                kept.push(HostedOverlay {
                    id: overlay.id.clone(),
                    anchor: overlay.anchor,
                    completion_item_count: menu_item_count(&overlay.component),
                    z_order: overlay_z_order(overlay.z_level_token),
                    pod: WidgetPod::new(region),
                });
                changed = true;
            }
        }
        kept.sort_by_key(|h| h.z_order);
        self.overlays = kept;
        if changed {
            ctx.children_changed();
        }
        ctx.request_layout();
        ctx.request_accessibility_update();
    }
}

impl Widget for PackageOverlayHost {
    type Action = NoAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if !self.centered {
            return;
        }
        if matches!(event, PointerEvent::Down(..))
            && let Some(target) = self.focus_restore_target
        {
            // Scrim clicks are modal but do not move focus into the transient
            // layer; keep keyboard routing and close-time restoration on the
            // originating pane.
            ctx.set_focus(target);
        }
        ctx.set_handled();
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        for hosted in &mut self.overlays {
            ctx.register_child(&mut hosted.pod);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        };
        let working_area = size.to_rect();
        self.window_rect = working_area;
        let main_rect = self.main_rect.get();
        // Content inset matching the legacy overlay paint (`spacing.panel` from
        // the top edge; the row rect supplies the left indent).
        let overlay_padding = self.ui_theme.scalar_f64("spacing.panel").unwrap_or(16.0);
        let centered_width = self
            .ui_theme
            .dimension("dimension.overlay.centered.width")
            .unwrap_or(640.0);
        self.overlay_rects.clear();
        for hosted in &mut self.overlays {
            let rect = match hosted.anchor {
                PackageOverlayAnchor::Completion => completion_overlay_rect(
                    main_rect,
                    self.completion_anchor.get(),
                    hosted.completion_item_count,
                    &self.typography,
                    &self.ui_theme,
                ),
                _ if self.centered => {
                    hosted
                        .anchor
                        .rect_with_centered_width(working_area, main_rect, centered_width)
                }
                _ => hosted.anchor.rect(working_area, main_rect),
            };
            self.overlay_rects.push(rect);
            let content_size = Size::new(rect.width(), (rect.height() - overlay_padding).max(1.0));
            let _ = ctx.run_layout(
                &mut hosted.pod,
                &BoxConstraints::new(content_size, content_size),
            );
            ctx.place_child(
                &mut hosted.pod,
                Point::new(rect.x0, rect.y0 + overlay_padding),
            );
        }
        size
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        if self.centered {
            // Root-layer paint runs above the complete shell/base layer; the
            // scrim therefore dims splits and tab chrome before the menu shell.
            paint_scrim(scene, self.window_rect, &self.ui_theme);
        }
        // Tooltip-shell chrome behind each overlay; the children paint their
        // component content on top during the child pass.
        for rect in &self.overlay_rects {
            paint_tooltip_shell(scene, *rect, &self.ui_theme);
        }
    }

    fn accessibility_role(&self) -> Role {
        if self.centered {
            Role::Dialog
        } else {
            Role::Group
        }
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        // Children (the overlay regions) flow into the a11y tree by default.
        if self.centered {
            node.set_label(self.dialog_label.as_deref().unwrap_or("Command Centre"));
            node.set_modal();
        } else {
            node.set_label("Package overlays");
        }
    }

    fn children_ids(&self) -> ChildrenIds {
        self.overlays.iter().map(|hosted| hosted.pod.id()).collect()
    }

    fn accepts_pointer_interaction(&self) -> bool {
        // Local hosts stay transparent outside their bounded overlay rects;
        // the centered root layer is a modal window shield, so scrim clicks do
        // not reach the editor/base layer.
        self.centered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SduiActionSource;
    use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
    use masonry::core::WidgetRef;
    use masonry::dpi::PhysicalSize;
    use masonry::theme::default_property_set;
    use serde_json::json;

    fn default_style() -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&ResolvedUiTheme::default())
    }

    fn render_root_options() -> RenderRootOptions {
        RenderRootOptions {
            default_properties: default_property_set().into(),
            use_system_fonts: false,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(900, 600),
            scale_factor: 1.0,
            test_font: None,
        }
    }

    fn component(decl: serde_json::Value) -> PackageUiComponentTree {
        PackageUiComponentTree::from_declaration(&decl).expect("valid component declaration")
    }

    fn collect_leaf_heights(widget: WidgetRef<'_, dyn Widget>, out: &mut Vec<f64>) {
        let children = widget.children();
        if children.is_empty() {
            out.push(widget.ctx().size().height);
        } else {
            for child in children {
                collect_leaf_heights(child, out);
            }
        }
    }

    fn hosted_region(tree: &PackageUiComponentTree) -> (RenderRoot, WidgetId) {
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(tree);
        let region_new = NewWidget::new(region);
        let region_id = region_new.id();
        let mut rr = RenderRoot::new(region_new, |_| {}, render_root_options());
        let _ = rr.redraw();
        (rr, region_id)
    }

    fn reconcile_live(rr: &mut RenderRoot, region_id: WidgetId, tree: &PackageUiComponentTree) {
        rr.edit_widget(region_id, |mut w| {
            let mut region = w.try_downcast::<PackageRegionWidget>().expect("region");
            region.widget.reconcile_tree_live(&mut region.ctx, tree);
        });
        let _ = rr.redraw();
    }

    fn pod_id(rr: &mut RenderRoot, region_id: WidgetId, id_hash: u64) -> Option<WidgetId> {
        let mut result = None;
        rr.edit_widget(region_id, |mut w| {
            let region = w.try_downcast::<PackageRegionWidget>().expect("region");
            result = region.widget.pod_id_for(id_hash);
        });
        result
    }

    #[test]
    fn every_nontransient_kind_reconciles_to_a_root_pod() {
        for decl in [
            json!({"kind": "panel", "id": "p", "title": "T", "children": []}),
            json!({"kind": "flex", "id": "f", "children": []}),
            json!({"kind": "stack", "id": "s", "children": []}),
            json!({"kind": "label", "id": "l", "text": "hi"}),
            json!({"kind": "statusItem", "id": "si", "text": "ready"}),
            json!({"kind": "editorView", "id": "ev"}),
            json!({"kind": "button", "id": "b", "label": "Go", "action": {"commandId": "x.y"}}),
            json!({"kind": "list", "id": "li", "items": [{"id": "r0", "label": "R0"}]}),
        ] {
            let tree = component(decl);
            let mut region = PackageRegionWidget::new();
            region.reconcile_tree(&tree);
            assert!(
                region.root_pod_id().is_some(),
                "kind {} should reconcile",
                tree.kind
            );
            assert_eq!(region.children_ids().len(), 1);
        }
    }

    #[test]
    fn panel_children_reconcile_at_geometry_parity() {
        // Plan 070 step 13a parity gate: the reconciled panel/label/button/list
        // leaves report the exact heights the legacy immediate-mode renderer
        // advances its cursor by (title row, label row, button height, list row
        // heights).
        let tree = component(json!({
            "kind": "panel", "id": "p", "title": "Files",
            "children": [
                {"kind": "label", "id": "p.l", "text": "hint"},
                {"kind": "button", "id": "p.b", "label": "Open", "action": {"commandId": "doc.open"}},
                {"kind": "list", "id": "p.li", "items": [
                    {"id": "r0", "label": "A"},
                    {"id": "r1", "label": "B", "detail": "d"}
                ]}
            ]
        }));
        let (mut rr, region_id) = hosted_region(&tree);
        let root = {
            let mut id = None;
            rr.edit_widget(region_id, |mut w| {
                let region = w.try_downcast::<PackageRegionWidget>().expect("region");
                id = region.widget.root_pod_id();
            });
            id.expect("root pod")
        };
        let mut heights = Vec::new();
        collect_leaf_heights(rr.get_widget(root).expect("root widget"), &mut heights);

        let typography = TypographyRegistry::default();
        let style = default_style();
        let title = typography.ui_text_metrics(FontRole::Ui, style.title_text);
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
        let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
        let expected = [
            title.row_height,
            body.row_height,
            body.button_height(),
            body.list_height(detail),
            body.list_height(detail),
        ];
        assert_eq!(heights.len(), expected.len(), "leaf count");
        for (got, want) in heights.iter().zip(expected.iter()) {
            assert!(
                (got - want).abs() <= 1.0,
                "leaf height {got} != legacy {want}"
            );
        }
    }

    #[test]
    fn stable_identity_across_prop_update() {
        // A prop-only change (label text) keeps the component's WidgetId.
        let label_hash = stable_package_source_id("p.l");
        let v1 = component(json!({
            "kind": "panel", "id": "p", "title": "T",
            "children": [{"kind": "label", "id": "p.l", "text": "before"}]
        }));
        let v2 = component(json!({
            "kind": "panel", "id": "p", "title": "T",
            "children": [{"kind": "label", "id": "p.l", "text": "after"}]
        }));
        let (mut rr, region_id) = hosted_region(&v1);
        let before = pod_id(&mut rr, region_id, label_hash).expect("label pod");
        reconcile_live(&mut rr, region_id, &v2);
        let after = pod_id(&mut rr, region_id, label_hash).expect("label pod");
        assert_eq!(before, after, "prop-only update keeps WidgetId");
    }

    #[test]
    fn kind_change_forces_rebuild() {
        // Changing a component's kind (label -> button) at the same id forces a
        // fresh widget (new WidgetId), per the Step-11 kind-discriminant rule.
        let hash = stable_package_source_id("p.x");
        let v1 = component(json!({
            "kind": "panel", "id": "p",
            "children": [{"kind": "label", "id": "p.x", "text": "hi"}]
        }));
        let v2 = component(json!({
            "kind": "panel", "id": "p",
            "children": [{"kind": "button", "id": "p.x", "label": "hi", "action": {"commandId": "a.b"}}]
        }));
        let (mut rr, region_id) = hosted_region(&v1);
        let before = pod_id(&mut rr, region_id, hash).expect("pod");
        reconcile_live(&mut rr, region_id, &v2);
        let after = pod_id(&mut rr, region_id, hash).expect("pod");
        assert_ne!(before, after, "kind change forces a fresh widget");
    }

    #[test]
    fn child_list_add_remove_reconciles_survivors() {
        // Pure add/remove preserves surviving children identity.
        let a_hash = stable_package_source_id("f.a");
        let c_hash = stable_package_source_id("f.c");
        let v1 = component(json!({
            "kind": "flex", "id": "f",
            "children": [
                {"kind": "label", "id": "f.a", "text": "a"},
                {"kind": "label", "id": "f.b", "text": "b"}
            ]
        }));
        let v2 = component(json!({
            "kind": "flex", "id": "f",
            "children": [
                {"kind": "label", "id": "f.a", "text": "a"},
                {"kind": "label", "id": "f.c", "text": "c"}
            ]
        }));
        let (mut rr, region_id) = hosted_region(&v1);
        let a_before = pod_id(&mut rr, region_id, a_hash).expect("a pod");
        reconcile_live(&mut rr, region_id, &v2);
        let a_after = pod_id(&mut rr, region_id, a_hash).expect("a pod survives");
        assert_eq!(a_before, a_after, "surviving child keeps WidgetId");
        assert!(pod_id(&mut rr, region_id, c_hash).is_some(), "added child");
    }

    #[test]
    fn package_button_action_emits_server_intent() {
        // Clicking the reconciled package button emits `PackageButtonPress`
        // carrying the exact intent the legacy hit-test would enqueue.
        use masonry::app::RenderRootSignal;
        use masonry::core::{
            PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo, PointerState,
            PointerType, PointerUpdate,
        };
        use masonry::dpi::PhysicalPosition;
        use std::cell::RefCell;
        use std::rc::Rc;

        let expected = package_action_intent("doc.open", "b");
        let tree = component(
            json!({"kind": "button", "id": "b", "label": "Open", "action": {"commandId": "doc.open"}}),
        );

        let captured: Rc<RefCell<Vec<SduiActionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(&tree);
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(press) = action.downcast::<PackageButtonPress>()
                {
                    sink.borrow_mut().push(press.intent.clone());
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        let state = PointerState {
            position: PhysicalPosition::new(450.0, body.button_height() / 2.0),
            ..Default::default()
        };
        rr.handle_pointer_event(PointerEvent::Move(PointerUpdate {
            pointer: info,
            current: state.clone(),
            coalesced: vec![],
            predicted: vec![],
        }));
        rr.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state: state.clone(),
        }));
        rr.handle_pointer_event(PointerEvent::Up(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state,
        }));

        let captured = captured.borrow();
        assert_eq!(captured.len(), 1, "exactly one button action emitted");
        assert_eq!(captured[0], expected);
        assert_eq!(
            captured[0].source,
            SduiActionSource::Button {
                node_id: crate::protocol::SduiNodeId(stable_package_source_id("b"))
            }
        );
    }

    #[test]
    fn package_list_row_action_emits_server_intent() {
        // Clicking an actionable package list row emits `PackageListRowPress`
        // carrying the intent addressed at `component.id.item.id`.
        use masonry::app::RenderRootSignal;
        use masonry::core::{
            PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo, PointerState,
            PointerType, PointerUpdate,
        };
        use masonry::dpi::PhysicalPosition;
        use std::cell::RefCell;
        use std::rc::Rc;

        let expected = package_action_intent("doc.open", "li.r0");
        let tree = component(json!({
            "kind": "list", "id": "li",
            "items": [{"id": "r0", "label": "A", "action": {"commandId": "doc.open"}}]
        }));

        let captured: Rc<RefCell<Vec<SduiActionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(&tree);
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(press) = action.downcast::<PackageListRowPress>()
                {
                    sink.borrow_mut().push(press.intent.clone());
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        let typography = TypographyRegistry::default();
        let style = default_style();
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
        let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
        let row_h = body.list_height(detail);
        let state = PointerState {
            position: PhysicalPosition::new(450.0, row_h / 2.0),
            ..Default::default()
        };
        rr.handle_pointer_event(PointerEvent::Move(PointerUpdate {
            pointer: info,
            current: state.clone(),
            coalesced: vec![],
            predicted: vec![],
        }));
        rr.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state: state.clone(),
        }));
        rr.handle_pointer_event(PointerEvent::Up(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state,
        }));

        let captured = captured.borrow();
        assert_eq!(captured.len(), 1, "exactly one list-row action emitted");
        assert_eq!(captured[0], expected);
    }

    fn click_at(rr: &mut RenderRoot, x: f64, y: f64) {
        use masonry::core::{
            PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo, PointerState,
            PointerType, PointerUpdate,
        };
        use masonry::dpi::PhysicalPosition;
        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        let state = PointerState {
            position: PhysicalPosition::new(x, y),
            ..Default::default()
        };
        rr.handle_pointer_event(PointerEvent::Move(PointerUpdate {
            pointer: info,
            current: state.clone(),
            coalesced: vec![],
            predicted: vec![],
        }));
        rr.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state: state.clone(),
        }));
        rr.handle_pointer_event(PointerEvent::Up(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state,
        }));
    }

    /// Pointer move only (no click) — drives hover state.
    fn move_to(rr: &mut RenderRoot, x: f64, y: f64) {
        use masonry::core::{
            PointerEvent, PointerId, PointerInfo, PointerState, PointerType, PointerUpdate,
        };
        use masonry::dpi::PhysicalPosition;
        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        let state = PointerState {
            position: PhysicalPosition::new(x, y),
            ..Default::default()
        };
        rr.handle_pointer_event(PointerEvent::Move(PointerUpdate {
            pointer: info,
            current: state,
            coalesced: vec![],
            predicted: vec![],
        }));
    }

    fn widget_height(rr: &mut RenderRoot, id: WidgetId) -> f64 {
        rr.get_widget(id)
            .expect("widget present")
            .ctx()
            .size()
            .height
    }

    #[test]
    fn collapse_toggles_children_visibility() {
        // Plan 070 step 13b: the retained `PackageCollapse` hides its children
        // when collapsed (title height only) and reveals them when expanded.
        // Wrap in a flex so the collapse is a non-root child with loose main-axis
        // constraints (a tight root would be forced to the window size, hiding the
        // content-height change).
        let tree = component(json!({
            "kind": "flex", "id": "root",
            "children": [{
                "kind": "collapse", "id": "c", "title": "Section",
                "children": [
                    {"kind": "label", "id": "c.a", "text": "a"},
                    {"kind": "label", "id": "c.b", "text": "b"}
                ]
            }]
        }));
        let (mut rr, region_id) = hosted_region(&tree);
        let collapse_id =
            pod_id(&mut rr, region_id, stable_package_source_id("c")).expect("collapse pod");

        let typography = TypographyRegistry::default();
        let style = default_style();
        let title_h = typography
            .ui_text_metrics(FontRole::Ui, style.title_text)
            .row_height;
        let body_h = typography
            .ui_text_metrics(FontRole::Ui, style.body_text)
            .row_height;

        let collapsed = widget_height(&mut rr, collapse_id);
        assert!(
            (collapsed - title_h).abs() <= 1.0,
            "collapsed = title row only ({collapsed} vs {title_h})"
        );

        click_at(&mut rr, 450.0, title_h / 2.0);
        let _ = rr.redraw();
        let expanded = widget_height(&mut rr, collapse_id);
        assert!(
            (expanded - (title_h + 2.0 * body_h)).abs() <= 1.0,
            "expanded = title + children ({expanded})"
        );

        click_at(&mut rr, 450.0, title_h / 2.0);
        let _ = rr.redraw();
        let recollapsed = widget_height(&mut rr, collapse_id);
        assert!(
            (recollapsed - title_h).abs() <= 1.0,
            "re-collapsed = title row only ({recollapsed})"
        );
    }

    #[test]
    fn collapse_expanded_state_survives_reconcile() {
        // The expanded state is retained in the widget across a prop-only
        // reconcile (same component ids), replacing the client `collapse_expanded`
        // map. The widget identity is preserved too.
        let v1 = component(json!({
            "kind": "flex", "id": "root",
            "children": [{
                "kind": "collapse", "id": "c", "title": "S",
                "children": [{"kind": "label", "id": "c.a", "text": "a"}]
            }]
        }));
        let (mut rr, region_id) = hosted_region(&v1);
        let collapse_id =
            pod_id(&mut rr, region_id, stable_package_source_id("c")).expect("collapse pod");
        let typography = TypographyRegistry::default();
        let style = default_style();
        let title_h = typography
            .ui_text_metrics(FontRole::Ui, style.title_text)
            .row_height;
        let body_h = typography
            .ui_text_metrics(FontRole::Ui, style.body_text)
            .row_height;

        click_at(&mut rr, 450.0, title_h / 2.0);
        let _ = rr.redraw();
        let expanded = widget_height(&mut rr, collapse_id);
        assert!((expanded - (title_h + body_h)).abs() <= 1.0);

        let v2 = component(json!({
            "kind": "flex", "id": "root",
            "children": [{
                "kind": "collapse", "id": "c", "title": "S",
                "children": [{"kind": "label", "id": "c.a", "text": "a-changed"}]
            }]
        }));
        reconcile_live(&mut rr, region_id, &v2);

        let collapse_id_after =
            pod_id(&mut rr, region_id, stable_package_source_id("c")).expect("collapse pod");
        assert_eq!(collapse_id, collapse_id_after, "collapse keeps identity");
        let still_expanded = widget_height(&mut rr, collapse_id_after);
        assert!(
            (still_expanded - (title_h + body_h)).abs() <= 1.0,
            "expanded state survives reconcile ({still_expanded})"
        );
    }

    #[test]
    fn panel_host_hosts_and_removes_panels() {
        // Plan 070 step 13b: the panel host reconciles the visible fixed-panel
        // set — hosting a region per visible panel and dropping it when the panel
        // becomes hidden/absent.
        use crate::shell::package_ui::{PackagePanelVisibility, PackageUiRuntimeUpdate};
        let build_ui = |visible: bool| {
            let mut runtime = PackageUiRuntimeState::new();
            runtime
                .apply_update(PackageUiRuntimeUpdate {
                    base_version: 0,
                    fixed_panels: vec![FixedPackagePanel::new(
                        "settings.surface",
                        FixedSlotId::Right,
                        if visible {
                            PackagePanelVisibility::Visible
                        } else {
                            PackagePanelVisibility::Hidden
                        },
                        component(
                            json!({"kind": "panel", "id": "s.root", "title": "S", "children": [
                                {"kind": "label", "id": "s.l", "text": "hi"}
                            ]}),
                        ),
                        Vec::new(),
                    )],
                    transient_overlays: Vec::new(),
                    input_routing: Vec::new(),
                })
                .unwrap();
            runtime
        };

        let host_new = NewWidget::new(PackagePanelHost::new());
        let host_id = host_new.id();
        let mut rr = RenderRoot::new(host_new, |_| {}, render_root_options());
        let _ = rr.redraw();

        let sync = |rr: &mut RenderRoot, ui: &PackageUiRuntimeState| {
            rr.edit_widget(host_id, |mut w| {
                let mut host = w.try_downcast::<PackagePanelHost>().expect("host");
                host.widget.sync_panels(
                    &mut host.ctx,
                    ui,
                    TypographyRegistry::default(),
                    ResolvedUiTheme::default(),
                );
            });
            let _ = rr.redraw();
        };
        let panel_count = |rr: &mut RenderRoot| {
            let mut count = 0;
            rr.edit_widget(host_id, |mut w| {
                let host = w.try_downcast::<PackagePanelHost>().expect("host");
                count = host.widget.panels.len();
            });
            count
        };

        let ui = build_ui(true);
        sync(&mut rr, &ui);
        assert_eq!(panel_count(&mut rr), 1, "visible panel hosted");

        // Reconcile same panel again (in place, still one host).
        sync(&mut rr, &ui);
        assert_eq!(panel_count(&mut rr), 1, "panel reconciled in place");

        let hidden = build_ui(false);
        sync(&mut rr, &hidden);
        assert_eq!(panel_count(&mut rr), 0, "hidden panel dropped");
    }

    fn type_key(rr: &mut RenderRoot, key: Key, code: masonry::core::keyboard::Code) {
        use masonry::core::TextEvent;
        use masonry::core::keyboard::{KeyState, KeyboardEvent, Modifiers};
        rr.handle_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key,
            code,
            modifiers: Modifiers::empty(),
            ..KeyboardEvent::default()
        }));
    }

    /// Send a key Down with a modifier set (e.g. Shift+Tab).
    fn type_key_mods(
        rr: &mut RenderRoot,
        key: Key,
        code: masonry::core::keyboard::Code,
        modifiers: masonry::core::keyboard::Modifiers,
    ) {
        use masonry::core::TextEvent;
        use masonry::core::keyboard::{KeyState, KeyboardEvent};
        rr.handle_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key,
            code,
            modifiers,
            ..KeyboardEvent::default()
        }));
    }

    /// A full keypress (Down then Up) for keys handled on release (Enter/Space).
    fn press_key(rr: &mut RenderRoot, key: Key, code: masonry::core::keyboard::Code) {
        use masonry::core::TextEvent;
        use masonry::core::keyboard::{KeyState, KeyboardEvent, Modifiers};
        for state in [KeyState::Down, KeyState::Up] {
            rr.handle_text_event(TextEvent::Keyboard(KeyboardEvent {
                state,
                key: key.clone(),
                code,
                modifiers: Modifiers::empty(),
                ..KeyboardEvent::default()
            }));
        }
    }

    #[test]
    fn text_input_typing_is_optimistic_and_commit_emits_intent() {
        // Plan 070 step 13c: typing updates the field optimistically (Masonry
        // `TextArea` emits `TextAction::Changed`/`Entered`); the committed value
        // is appended to the registered-command intent via the region's map.
        use masonry::app::RenderRootSignal;
        use masonry::core::keyboard::Code;
        use masonry::widgets::TextAction;
        use std::cell::RefCell;
        use std::rc::Rc;

        let tree = component(json!({
            "kind": "flex", "id": "root",
            "children": [{
                "kind": "textInput", "id": "ti", "title": "Monospace families",
                "action": {"commandId": "settings.setTypography"}
            }]
        }));
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(&tree);
        let area_id = *region
            .text_input_intents
            .keys()
            .next()
            .expect("text input registered");

        // The commit intent appends the value as a `"value"` argument.
        let intent = region
            .text_input_commit(area_id, "ab")
            .expect("commit intent");
        assert_eq!(intent.command_id, "settings.setTypography");
        assert_eq!(
            intent.arguments,
            vec![SduiActionArgument {
                name: "value".to_string(),
                value: SduiActionValue::String("ab".to_string()),
            }]
        );

        let captured: Rc<RefCell<Vec<(&'static str, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(text_action) = action.downcast::<TextAction>()
                {
                    match *text_action {
                        TextAction::Entered(v) => sink.borrow_mut().push(("entered", v)),
                        TextAction::Changed(v) => sink.borrow_mut().push(("changed", v)),
                    }
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        // Focus the field, type "ab", commit with Enter.
        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        click_at(&mut rr, 450.0, body.button_height() / 2.0);
        let _ = rr.redraw();
        type_key(&mut rr, Key::Character("a".into()), Code::KeyA);
        type_key(&mut rr, Key::Character("b".into()), Code::KeyB);
        type_key(&mut rr, Key::Named(NamedKey::Enter), Code::Enter);

        let actions = captured.borrow();
        let entered: Vec<&String> = actions
            .iter()
            .filter(|(kind, _)| *kind == "entered")
            .map(|(_, v)| v)
            .collect();
        assert_eq!(entered.len(), 1, "exactly one commit, got {actions:?}");
        assert_eq!(entered[0], "ab");
    }

    #[test]
    fn text_input_border_color_resolves_validation_over_focus_over_subtle() {
        let theme = ResolvedUiTheme::default();
        let tree = component(json!({
            "kind": "textInput", "id": "ti", "style": {"validationState": "error"}
        }));
        let (mut input, _) = PackageTextInput::from_component(
            &tree,
            0,
            TypographyRegistry::default(),
            theme.clone(),
        );
        let error = theme.color("diagnostic.error").unwrap();
        let warning = theme.color("diagnostic.warning").unwrap();
        let success = theme.color("diagnostic.success").unwrap();
        let focus = theme.color("border.focus").unwrap();
        let subtle = theme.color("border.subtle").unwrap();

        assert_eq!(input.border_color(), error);
        // Validation wins over focus.
        input.is_focused = true;
        assert_eq!(input.border_color(), error);
        // No validation: focus > subtle.
        input.validation_state = None;
        assert_eq!(input.border_color(), focus);
        input.is_focused = false;
        assert_eq!(input.border_color(), subtle);
        // Warning + success resolve too.
        input.validation_state = Some("warning".to_string());
        assert_eq!(input.border_color(), warning);
        input.validation_state = Some("success".to_string());
        assert_eq!(input.border_color(), success);
    }

    #[test]
    fn text_input_placeholder_shown_only_when_empty() {
        let theme = ResolvedUiTheme::default();
        let empty =
            component(json!({"kind": "textInput", "id": "ti", "title": "Monospace families"}));
        let (input, _) = PackageTextInput::from_component(
            &empty,
            0,
            TypographyRegistry::default(),
            theme.clone(),
        );
        assert_eq!(input.placeholder, "Monospace families");
        assert!(input.is_empty, "empty field shows the placeholder");

        let filled =
            component(json!({"kind": "textInput", "id": "ti", "title": "Hint", "text": "value"}));
        let (input, _) =
            PackageTextInput::from_component(&filled, 0, TypographyRegistry::default(), theme);
        assert!(!input.is_empty, "non-empty field hides the placeholder");
    }

    #[test]
    fn text_input_adopts_changed_server_value_when_unfocused() {
        // Server authority: a changed `component.text` is adopted into the field
        // on reconcile (revert-on-reject) when the user is not mid-edit.
        use masonry::widgets::TextArea;
        let v1 = component(json!({"kind": "textInput", "id": "ti", "text": "initial"}));
        let (mut rr, region_id) = hosted_region(&v1);
        let v2 = component(json!({"kind": "textInput", "id": "ti", "text": "updated"}));
        reconcile_live(&mut rr, region_id, &v2);

        let mut area_id = None;
        rr.edit_widget(region_id, |mut w| {
            let region = w.try_downcast::<PackageRegionWidget>().expect("region");
            area_id = region.widget.text_input_intents.keys().next().copied();
        });
        let area_id = area_id.expect("text input registered");
        let text = rr
            .get_widget(area_id)
            .expect("text area present")
            .downcast::<TextArea<true>>()
            .expect("a text area")
            .text()
            .to_string();
        assert_eq!(text, "updated", "unfocused field adopts the server value");
    }

    fn dropdown_tree() -> PackageUiComponentTree {
        component(json!({
            "kind": "flex", "id": "root",
            "children": [{
                "kind": "dropdown", "id": "dd", "title": "Theme",
                "items": [
                    { "id": "a", "label": "Alpha", "action": {"commandId": "settings.setTheme"} },
                    { "id": "b", "label": "Beta", "action": {"commandId": "settings.setTheme"} },
                    { "id": "c", "label": "Gamma", "action": {"commandId": "settings.setTheme"} }
                ]
            }]
        }))
    }

    #[test]
    fn dropdown_arrow_keys_cycle_and_enter_confirms() {
        // Plan 070 step 13d: ArrowUp/Down cycles the selection, Enter confirms
        // and emits the selected item's command intent.
        use masonry::app::RenderRootSignal;
        use masonry::core::keyboard::Code;
        use std::cell::RefCell;
        use std::rc::Rc;

        let tree = dropdown_tree();
        let captured: Rc<RefCell<Vec<SduiActionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(&tree);
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(select) = action.downcast::<PackageDropdownSelect>()
                {
                    sink.borrow_mut().push(select.intent.clone());
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        // Focus + open the dropdown by clicking the trigger, then cycle and
        // confirm with the keyboard.
        click_at(&mut rr, 450.0, body.button_height() / 2.0);
        type_key(&mut rr, Key::Named(NamedKey::ArrowDown), Code::ArrowDown);
        press_key(&mut rr, Key::Named(NamedKey::Enter), Code::Enter);

        let got = captured.borrow();
        assert_eq!(got.len(), 1, "exactly one confirm, got {got:?}");
        // ArrowDown from the initial selection (0) lands on item 1 (`b`/`Beta`).
        assert_eq!(
            got[0],
            package_action_intent("settings.setTheme", "dd.b"),
            "confirm emits the cycled-to item's command"
        );
    }

    #[test]
    fn dropdown_selection_persists_across_unrelated_update() {
        // Plan 070 step 13d: the widget-local selection survives an unrelated
        // reconcile (stable identity) — the `dropdown_selected` map is gone.
        use masonry::core::keyboard::Code;

        let dropdown_decl = || {
            json!({
                "kind": "dropdown", "id": "dd", "title": "Theme",
                "items": [
                    { "id": "a", "label": "Alpha", "action": {"commandId": "settings.setTheme"} },
                    { "id": "b", "label": "Beta", "action": {"commandId": "settings.setTheme"} },
                    { "id": "c", "label": "Gamma", "action": {"commandId": "settings.setTheme"} }
                ]
            })
        };
        let tree = component(json!({
            "kind": "flex", "id": "root",
            "children": [ dropdown_decl(), { "kind": "label", "id": "lbl", "text": "before" } ]
        }));
        let (mut rr, region_id) = hosted_region(&tree);
        let dd_id = pod_id(&mut rr, region_id, stable_package_source_id("dd")).expect("dropdown");

        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        click_at(&mut rr, 450.0, body.button_height() / 2.0);
        type_key(&mut rr, Key::Named(NamedKey::ArrowDown), Code::ArrowDown);

        // Unrelated change: the sibling label text updates; the dropdown keeps
        // its selection + widget identity.
        let updated = component(json!({
            "kind": "flex", "id": "root",
            "children": [ dropdown_decl(), { "kind": "label", "id": "lbl", "text": "after" } ]
        }));
        reconcile_live(&mut rr, region_id, &updated);

        let dd_id_after =
            pod_id(&mut rr, region_id, stable_package_source_id("dd")).expect("dropdown");
        assert_eq!(dd_id, dd_id_after, "dropdown keeps its widget id");
        let selected = rr
            .get_widget(dd_id_after)
            .expect("dropdown present")
            .downcast::<PackageDropdown>()
            .expect("a dropdown")
            .selected;
        assert_eq!(selected, 1, "selection survives the unrelated update");
    }

    #[test]
    fn dropdown_open_list_row_hover_active_and_click_emits_intent() {
        // Plan 070 step 13d: the open list gives rows hover/active feedback and
        // clicking a row selects + confirms it (emitting its command intent).
        use masonry::app::RenderRootSignal;
        use std::cell::RefCell;
        use std::rc::Rc;

        let tree = dropdown_tree();
        let captured: Rc<RefCell<Vec<SduiActionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(&tree);
        let dd_hash = stable_package_source_id("dd");
        let dd_widget_id = region
            .pod_id_for(dd_hash)
            .expect("dropdown registered before host");
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(select) = action.downcast::<PackageDropdownSelect>()
                {
                    sink.borrow_mut().push(select.intent.clone());
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        let trigger_h = body.button_height();
        let row_h = body.row_height;
        // Open the dropdown (click the trigger).
        click_at(&mut rr, 450.0, trigger_h / 2.0);
        let _ = rr.redraw();
        let row_state = |rr: &RenderRoot, idx: usize| {
            rr.get_widget(dd_widget_id)
                .expect("dropdown present")
                .downcast::<PackageDropdown>()
                .expect("a dropdown")
                .row_state(idx)
        };

        // Hover row 1 (the second open-list row).
        let row1_y = trigger_h + row_h * 1.5;
        move_to(&mut rr, 450.0, row1_y);
        assert_eq!(row_state(&rr, 1), InteractionState::Hover, "row 1 hovered");
        assert_eq!(row_state(&rr, 0), InteractionState::Rest, "row 0 at rest");

        // Press + release row 1: active during press, confirm on release.
        click_at(&mut rr, 450.0, row1_y);
        let got = captured.borrow();
        assert_eq!(got.len(), 1, "row click confirms exactly once, got {got:?}");
        assert_eq!(
            got[0],
            package_action_intent("settings.setTheme", "dd.b"),
            "clicking row 1 emits its command"
        );
        let open = rr
            .get_widget(dd_widget_id)
            .expect("dropdown present")
            .downcast::<PackageDropdown>()
            .expect("a dropdown")
            .open;
        assert!(!open, "dropdown closes after the row click");
    }

    #[test]
    fn modal_tab_traps_focus_and_escape_dismisses() {
        // Plan 070 step 13e: Tab/Shift+Tab cycle focus among the dialog's
        // focusable children (trapped, never leaking out); Escape emits a
        // `PackageModalDismiss` carrying the modal's id hash.
        use masonry::app::RenderRootSignal;
        use masonry::core::keyboard::{Code, Modifiers};
        use std::cell::RefCell;
        use std::rc::Rc;

        let tree = component(json!({
            "kind": "modal", "id": "m", "title": "Confirm",
            "children": [
                { "kind": "button", "id": "ok", "label": "OK", "action": {"commandId": "app.ok"} },
                { "kind": "button", "id": "cancel", "label": "Cancel", "action": {"commandId": "app.cancel"} }
            ]
        }));
        let dismissed: Rc<RefCell<Vec<u64>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = dismissed.clone();
        let mut region = PackageRegionWidget::new();
        region.reconcile_tree(&tree);
        let modal_id = region
            .pod_id_for(stable_package_source_id("m"))
            .expect("modal registered");
        let ok_id = region
            .pod_id_for(stable_package_source_id("ok"))
            .expect("ok registered");
        let cancel_id = region
            .pod_id_for(stable_package_source_id("cancel"))
            .expect("cancel registered");
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(d) = action.downcast::<PackageModalDismiss>()
                {
                    sink.borrow_mut().push(d.id_hash);
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();
        let focused = |rr: &RenderRoot, id: WidgetId| {
            rr.get_widget(id)
                .expect("widget present")
                .ctx()
                .is_focus_target()
        };

        rr.focus_on(Some(modal_id));
        assert!(focused(&rr, modal_id), "dialog takes focus");
        type_key(&mut rr, Key::Named(NamedKey::Tab), Code::Tab);
        assert!(focused(&rr, ok_id), "Tab 1 cycles to OK");
        type_key(&mut rr, Key::Named(NamedKey::Tab), Code::Tab);
        assert!(focused(&rr, cancel_id), "Tab 2 cycles to Cancel");
        type_key(&mut rr, Key::Named(NamedKey::Tab), Code::Tab);
        assert!(focused(&rr, ok_id), "Tab 3 wraps back to OK");
        type_key_mods(
            &mut rr,
            Key::Named(NamedKey::Tab),
            Code::Tab,
            Modifiers::SHIFT,
        );
        assert!(
            focused(&rr, cancel_id),
            "Shift+Tab cycles backwards to Cancel"
        );

        type_key(&mut rr, Key::Named(NamedKey::Escape), Code::Escape);
        assert_eq!(
            dismissed.borrow().as_slice(),
            &[stable_package_source_id("m")],
            "Escape emits the dismiss action for the modal"
        );
    }

    fn make_overlay(
        id: &str,
        anchor: PackageOverlayAnchor,
        z: &'static str,
        decl: serde_json::Value,
    ) -> TransientPackageOverlay {
        TransientPackageOverlay::new(
            id,
            anchor,
            "modeless",
            "escape",
            component(decl),
            Vec::new(),
            z,
        )
    }

    fn hosted_centered_overlay_host(
        overlays: Vec<TransientPackageOverlay>,
    ) -> (RenderRoot, WidgetId) {
        let host_widget = NewWidget::new(PackageOverlayHost::new_centered());
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        rr.edit_widget(host_id, |mut w| {
            let mut host = w
                .try_downcast::<PackageOverlayHost>()
                .expect("centered overlay host");
            host.widget.sync_overlays(
                &mut host.ctx,
                overlays,
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            );
        });
        let _ = rr.redraw();
        (rr, host_id)
    }

    #[test]
    fn centered_overlay_host_clamps_width_and_reuses_layer_on_resize() {
        let overlays = vec![make_overlay(
            "menu",
            PackageOverlayAnchor::Centered,
            "z.overlay",
            json!({"kind": "label", "id": "menu.l", "text": "centered"}),
        )];
        let (mut rr, host_id) = hosted_centered_overlay_host(overlays);
        let region_id = rr
            .get_widget(host_id)
            .expect("host present")
            .downcast::<PackageOverlayHost>()
            .expect("a centered overlay host")
            .overlays[0]
            .pod
            .id();
        let initial = *rr
            .get_widget(region_id)
            .expect("centered region present")
            .ctx();
        let initial_origin = initial.to_window(Point::ZERO);
        assert!((initial_origin.x - 130.0).abs() < 1.0);
        assert!((initial_origin.y - 204.0).abs() < 1.0);
        assert!((initial.size().width - 640.0).abs() < 1.0);
        assert!(
            rr.get_widget(host_id)
                .expect("host present")
                .ctx()
                .accepts_pointer_interaction(),
            "centered root layer shields scrim clicks"
        );

        rr.handle_window_event(masonry::core::WindowEvent::Resize(PhysicalSize::new(
            300, 200,
        )));
        let _ = rr.redraw();
        let resized = *rr
            .get_widget(region_id)
            .expect("same centered region remains")
            .ctx();
        let resized_origin = resized.to_window(Point::ZERO);
        assert!((resized_origin.x - 0.0).abs() < 1.0);
        assert!((resized_origin.y - 14.0).abs() < 1.0);
        assert!((resized.size().width - 300.0).abs() < 1.0);
        assert!(
            rr.get_widget(host_id).is_some(),
            "resize keeps one root layer and retained host"
        );
    }

    #[test]
    fn centered_overlay_host_paints_full_window_scrim_before_surface() {
        let overlays = vec![make_overlay(
            "menu",
            PackageOverlayAnchor::Centered,
            "z.overlay",
            json!({"kind": "label", "id": "menu.l", "text": "centered"}),
        )];
        let (mut rr, host_id) = hosted_centered_overlay_host(overlays);
        let (scene, _) = rr.redraw();
        assert!(
            !scene.encoding().is_empty(),
            "centered host paints scrim and retained menu content"
        );
        let host = rr
            .get_widget(host_id)
            .expect("host present")
            .downcast::<PackageOverlayHost>()
            .expect("a centered overlay host");
        assert_eq!(host.window_rect, Rect::new(0.0, 0.0, 900.0, 600.0));
        assert_eq!(host.overlay_rects.len(), 1);
    }

    fn hosted_overlay_host(
        overlays: Vec<TransientPackageOverlay>,
        main_rect: Rect,
    ) -> (RenderRoot, WidgetId) {
        let cell = Rc::new(Cell::new(main_rect));
        let host_widget = NewWidget::new(PackageOverlayHost::new(cell));
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        rr.edit_widget(host_id, |mut w| {
            let mut host = w
                .try_downcast::<PackageOverlayHost>()
                .expect("overlay host");
            host.widget.sync_overlays(
                &mut host.ctx,
                overlays,
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            );
        });
        let _ = rr.redraw();
        (rr, host_id)
    }

    #[test]
    fn overlay_host_stacks_overlays_by_z_level() {
        // Plan 070 step 13e: transient z-order `z.overlay < z.modal < z.tooltip`
        // is preserved — the host sorts the children so the highest-z overlay is
        // last (topmost in Masonry's paint + reverse hit-test order).
        let overlays = vec![
            make_overlay(
                "tip",
                PackageOverlayAnchor::Pointer,
                "z.tooltip",
                json!({"kind": "label", "id": "tip.l", "text": "tip"}),
            ),
            make_overlay(
                "dlg",
                PackageOverlayAnchor::Pointer,
                "z.modal",
                json!({"kind": "label", "id": "dlg.l", "text": "dlg"}),
            ),
            make_overlay(
                "base",
                PackageOverlayAnchor::Pointer,
                "z.overlay",
                json!({"kind": "label", "id": "base.l", "text": "base"}),
            ),
        ];
        let (rr, host_id) = hosted_overlay_host(overlays, Rect::new(240.0, 0.0, 1140.0, 600.0));
        let host = rr
            .get_widget(host_id)
            .expect("host present")
            .downcast::<PackageOverlayHost>()
            .expect("an overlay host");
        let order: Vec<(&str, u8)> = host
            .overlays
            .iter()
            .map(|o| (o.id.as_str(), o.z_order))
            .collect();
        assert_eq!(
            order,
            [("base", 0), ("dlg", 1), ("tip", 2)],
            "overlays sorted ascending so z.tooltip is topmost (last)"
        );
    }

    #[test]
    fn overlay_host_places_overlay_at_anchor_rect() {
        // Plan 070 step 13e: each overlay is sized to its anchor rect (so it does
        // not block the region outside — the bounding-rect caveat). A Pointer
        // overlay centers a 320x220 rect within the main pane.
        let overlays = vec![make_overlay(
            "menu",
            PackageOverlayAnchor::Pointer,
            "z.overlay",
            json!({"kind": "label", "id": "menu.l", "text": "hi"}),
        )];
        let main_rect = Rect::new(240.0, 0.0, 1140.0, 600.0);
        let (rr, host_id) = hosted_overlay_host(overlays, main_rect);
        let region_id = rr
            .get_widget(host_id)
            .expect("host present")
            .downcast::<PackageOverlayHost>()
            .expect("an overlay host")
            .overlays[0]
            .pod
            .id();
        let ctx = *rr
            .get_widget(region_id)
            .expect("overlay region present")
            .ctx();
        let origin = ctx.to_window(Point::ZERO);
        let width = ctx.size().width;
        // centered_rect(main_rect, 320, 220): x0 = (240+1140)/2 - 160 = 530.
        assert!(
            (origin.x - 530.0).abs() < 1.0,
            "overlay x0 centers in the main pane, got {}",
            origin.x
        );
        assert!(
            (width - 320.0).abs() < 1.0,
            "overlay width matches the anchor rect, got {}",
            width
        );
    }

    #[test]
    fn overlay_host_is_transparent_to_pointer_outside_overlay_rects() {
        // Plan 070 step 13e (bounding-rect caveat): the host fills the working
        // area but must not intercept pointer events itself, so clicks outside an
        // overlay's rect fall through to the region/editor below.
        let overlays = vec![make_overlay(
            "menu",
            PackageOverlayAnchor::Pointer,
            "z.overlay",
            json!({"kind": "label", "id": "menu.l", "text": "hi"}),
        )];
        let (rr, host_id) = hosted_overlay_host(overlays, Rect::new(240.0, 0.0, 1140.0, 600.0));
        let accepts = rr
            .get_widget(host_id)
            .expect("host present")
            .ctx()
            .accepts_pointer_interaction();
        assert!(
            !accepts,
            "host is transparent to pointer hit-testing so clicks outside an \
             overlay's rect fall through to the region/editor below"
        );
    }

    /// Navigate host → overlay region → `menu.list` → row `index`, returning its
    /// retained `selected` flag.
    fn menu_row_selected(rr: &RenderRoot, host_id: WidgetId, index: usize) -> bool {
        let host = rr
            .get_widget(host_id)
            .expect("host present")
            .downcast::<PackageOverlayHost>()
            .expect("an overlay host");
        let region_id = host.overlays[0].pod.id();
        let region = rr
            .get_widget(region_id)
            .expect("overlay region present")
            .downcast::<PackageRegionWidget>()
            .expect("a package region");
        let list_id = region
            .pod_id_for(stable_package_source_id("menu.list"))
            .expect("menu list present");
        let row_id = *rr
            .get_widget(list_id)
            .expect("menu list present")
            .children_ids()
            .get(index)
            .expect("row present");
        rr.get_widget(row_id)
            .expect("menu row present")
            .downcast::<PackageListRow>()
            .expect("a package list row")
            .selected
    }

    fn menu_scroll_offset(rr: &RenderRoot, host_id: WidgetId) -> f64 {
        let host = rr
            .get_widget(host_id)
            .expect("host present")
            .downcast::<PackageOverlayHost>()
            .expect("an overlay host");
        let region_id = host.overlays[0].pod.id();
        let region = rr
            .get_widget(region_id)
            .expect("overlay region present")
            .downcast::<PackageRegionWidget>()
            .expect("a package region");
        let scroll_id = region
            .pod_id_for(stable_package_source_id("menu.9.scroll"))
            .expect("menu scroll viewport present");
        rr.get_widget(scroll_id)
            .expect("menu scroll viewport present")
            .downcast::<SduiScrollViewport>()
            .expect("menu scroll viewport")
            .scroll_offset_for_test()
    }

    #[test]
    fn menu_selection_keeps_selected_row_in_scroll_viewport() {
        use crate::shell::transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
            TransientMenuSessionId,
        };

        let items = (0..20)
            .map(|index| {
                TransientMenuItem::new(
                    format!("item-{index}"),
                    format!("Item {index}"),
                    TransientMenuAction::new("app.item"),
                )
            })
            .collect();
        let menu = TransientMenuSession::new(TransientMenuSessionId(9), "Completion")
            .with_origin(TransientMenuOrigin::Completion)
            .with_completion_anchor(Rect::new(200.0, 40.0, 201.0, 60.0))
            .with_items(items)
            .with_selected_index(19);
        let overlay = TransientPackageOverlay::from_menu_session(&menu);
        let (mut rr, host_id) =
            hosted_overlay_host(vec![overlay], Rect::new(0.0, 0.0, 900.0, 600.0));
        assert!(
            menu_scroll_offset(&rr, host_id) > 0.0,
            "last selected row must be scrolled into view"
        );

        let first = TransientPackageOverlay::from_menu_session(&menu.with_selected_index(0));
        rr.edit_widget(host_id, |mut w| {
            let mut host = w
                .try_downcast::<PackageOverlayHost>()
                .expect("overlay host");
            host.widget.sync_overlays(
                &mut host.ctx,
                vec![first],
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            );
        });
        let _ = rr.redraw();
        assert_eq!(
            menu_scroll_offset(&rr, host_id),
            0.0,
            "selection at list start returns viewport to its top"
        );
    }

    #[test]
    fn centered_command_center_scrolls_60_results_without_overflow() {
        use crate::shell::transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
            TransientMenuSessionId,
        };

        let items = (0..60)
            .map(|index| {
                TransientMenuItem::new(
                    format!("command-{index}"),
                    format!("Command {index}"),
                    TransientMenuAction::new("app.command"),
                )
            })
            .collect();
        let menu = TransientMenuSession::new(TransientMenuSessionId(9), "Control Center")
            .with_origin(TransientMenuOrigin::Centered)
            .with_items(items)
            .with_selected_index(59);
        let overlay = TransientPackageOverlay::from_menu_session(&menu);
        let (rr, host_id) = hosted_centered_overlay_host(vec![overlay]);
        let host = rr
            .get_widget(host_id)
            .expect("centered host")
            .downcast::<PackageOverlayHost>()
            .expect("centered overlay host");
        let region_widget = rr
            .get_widget(host.overlays[0].pod.id())
            .expect("centered region");
        let region = region_widget.ctx();
        let origin = region.to_window(Point::ZERO);
        assert!(origin.x >= 0.0 && origin.y >= 0.0);
        assert!(origin.x + region.size().width <= 900.0);
        assert!(origin.y + region.size().height <= 600.0);
        assert!(menu_scroll_offset(&rr, host_id) > 0.0);
    }

    #[test]
    fn overlay_host_reconcile_updates_menu_selection() {
        // Plan 070 step 13e: a transient menu's selection (which changes via the
        // keyboard, driving `MenuStateChanged` → `sync_overlays`) must update the
        // hosted rows' `selected` highlight through an in-place reconcile.
        let menu = |sel_a: bool, sel_b: bool| {
            make_overlay(
                "menu",
                PackageOverlayAnchor::Bottom,
                "z.overlay",
                json!({"kind": "stack", "id": "menu.root", "children": [
                    {"kind": "list", "id": "menu.list", "items": [
                        {"id": "a", "label": "Alpha", "selected": sel_a, "action": {"commandId": "app.a"}},
                        {"id": "b", "label": "Beta", "selected": sel_b, "action": {"commandId": "app.b"}}
                    ]}
                ]}),
            )
        };
        let cell = Rc::new(Cell::new(Rect::new(0.0, 0.0, 900.0, 600.0)));
        let host_widget = NewWidget::new(PackageOverlayHost::new(cell));
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        let sync = |rr: &mut RenderRoot, overlay: TransientPackageOverlay| {
            rr.edit_widget(host_id, |mut w| {
                let mut host = w
                    .try_downcast::<PackageOverlayHost>()
                    .expect("overlay host");
                host.widget.sync_overlays(
                    &mut host.ctx,
                    vec![overlay],
                    TypographyRegistry::default(),
                    ResolvedUiTheme::default(),
                );
            });
            let _ = rr.redraw();
        };

        sync(&mut rr, menu(true, false));
        assert!(
            menu_row_selected(&rr, host_id, 0),
            "row 0 selected initially"
        );
        assert!(
            !menu_row_selected(&rr, host_id, 1),
            "row 1 not selected initially"
        );

        // Keyboard nav flips the selection; re-syncing reconciles the highlight.
        sync(&mut rr, menu(false, true));
        assert!(
            !menu_row_selected(&rr, host_id, 0),
            "row 0 deselected after nav"
        );
        assert!(
            menu_row_selected(&rr, host_id, 1),
            "row 1 selected after nav"
        );
    }

    /// Plan 070 step 13f: the hosted menu overlay reports `Menu`/`MenuItem`/
    /// `Status` a11y (with the active item's "selected" suffix + custom
    /// accessibility labels), matching the legacy
    /// `collect_active_menu_accessibility_entries` contract that this replaces.
    #[test]
    fn hosted_menu_overlay_exposes_menu_role_and_item_accessibility_labels() {
        use crate::shell::transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuSession, TransientMenuSessionId,
        };

        let menu = TransientMenuSession::new(TransientMenuSessionId(9), "Conflict recovery")
            .with_items(vec![
                TransientMenuItem::new(
                    "reload",
                    "Reload",
                    TransientMenuAction::new("documents.serverReloadDocument"),
                )
                .with_accessibility_label("Reload from disk"),
                TransientMenuItem::new(
                    "keep",
                    "Keep editing",
                    TransientMenuAction::new("documents.dismissConflict"),
                )
                .with_accessibility_label("Keep dirty buffer"),
            ]);
        let overlay = TransientPackageOverlay::from_menu_session(&menu);

        let cell = Rc::new(Cell::new(Rect::new(0.0, 0.0, 900.0, 600.0)));
        let host_widget = NewWidget::new(PackageOverlayHost::new(cell));
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        rr.edit_widget(host_id, |mut w| {
            let mut host = w
                .try_downcast::<PackageOverlayHost>()
                .expect("overlay host");
            host.widget.sync_overlays(
                &mut host.ctx,
                vec![overlay],
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            );
        });
        rr.handle_window_event(masonry::core::WindowEvent::EnableAccessTree);
        let (_, tree_update) = rr.redraw();
        let tree_update = tree_update.expect("access tree active after EnableAccessTree");

        // Menu container labelled with the (sanitized) prompt.
        assert!(
            tree_update
                .nodes
                .iter()
                .any(|(_, n)| { n.role() == Role::Menu && n.label() == Some("Conflict recovery") }),
            "hosted menu reports a Menu node labelled with the prompt"
        );
        // Active item carries its custom accessibility label + "selected" suffix.
        assert!(
            tree_update.nodes.iter().any(|(_, n)| {
                n.role() == Role::MenuItem && n.label() == Some("Reload from disk selected")
            }),
            "selected menu item uses its accessibility label + selected suffix"
        );
        // Non-selected item uses its accessibility label without the suffix.
        assert!(
            tree_update.nodes.iter().any(|(_, n)| {
                n.role() == Role::MenuItem && n.label() == Some("Keep dirty buffer")
            }),
            "non-selected menu item uses its accessibility label"
        );
    }

    #[test]
    fn completion_menu_accessibility_is_modeless_and_consumer_valid() {
        use crate::protocol::{
            CompletionItem, CompletionProvenance, CompletionReplacementRange, CompletionResultSet,
            CompletionStatus,
        };
        use crate::shell::transient_menu::TransientMenuFocusPolicy;

        let result = CompletionResultSet {
            request_id: 12,
            client_id: 1,
            document_id: 7,
            document_version: 1,
            behavior_version: 0,
            provider_generation: 1,
            replacement_range: CompletionReplacementRange::new(0, 0),
            status: CompletionStatus::Ok,
            items: vec![
                CompletionItem::new("alpha", "alpha", CompletionProvenance::builtin_core()),
                CompletionItem::new("beta", "beta", CompletionProvenance::builtin_core()),
            ],
            provenance: CompletionProvenance::builtin_core(),
        };
        let menu = crate::shell::completion_result_to_menu_session(&result)
            .with_focus_policy(TransientMenuFocusPolicy::Modeless)
            .with_completion_anchor(Rect::new(700.0, 500.0, 701.0, 520.0))
            .with_selected_index(1);
        let overlay = TransientPackageOverlay::from_menu_session(&menu);
        assert_eq!(overlay.anchor, PackageOverlayAnchor::Completion);
        assert_eq!(overlay.focus_policy, "modeless");
        assert!(overlay.action_targets.is_empty());
        assert_eq!(
            overlay
                .menu_a11y
                .as_ref()
                .and_then(|menu| menu.result_count.as_deref()),
            None
        );

        let main_rect = Rc::new(Cell::new(Rect::new(0.0, 0.0, 900.0, 600.0)));
        let caret = Rc::new(Cell::new(Some(Rect::new(700.0, 500.0, 701.0, 520.0))));
        let host_widget =
            NewWidget::new(PackageOverlayHost::with_completion_anchor(main_rect, caret));
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        rr.edit_widget(host_id, |mut w| {
            let mut host = w
                .try_downcast::<PackageOverlayHost>()
                .expect("overlay host");
            host.widget.sync_overlays(
                &mut host.ctx,
                vec![overlay],
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            );
        });
        rr.handle_window_event(masonry::core::WindowEvent::EnableAccessTree);
        let (_, update) = rr.redraw();
        let update = update.expect("access tree active after EnableAccessTree");
        assert!(
            update.nodes.iter().any(|(_, node)| {
                node.role() == Role::Menu && node.label() == Some("Completion")
            })
        );
        assert!(update.nodes.iter().any(|(_, node)| {
            node.role() == Role::MenuItem
                && node.label() == Some("Completion beta selected")
                && node.is_selected() == Some(true)
        }));
        assert!(!update.nodes.iter().any(|(_, node)| {
            node.role() == Role::Dialog && node.label() == Some("Completion")
        }));
        let _tree = accesskit_consumer::Tree::new(update, false);
    }

    #[test]
    fn package_menu_accessibility_labels_are_sanitized_bounded_and_consumer_valid() {
        use crate::shell::transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuSession, TransientMenuSessionId,
        };

        let mut items = vec![
            TransientMenuItem::new(
                "long",
                "Display fallback",
                TransientMenuAction::new("package.long"),
            )
            .with_accessibility_label("x".repeat(256))
            .with_package_provenance("@clay/untrusted", "0.1.0"),
            TransientMenuItem::new(
                "path",
                "Safe fallback",
                TransientMenuAction::new("package.path"),
            )
            .with_accessibility_label("/\\\n")
            .with_package_provenance("@clay/untrusted", "0.1.0"),
            TransientMenuItem::new("empty", "", TransientMenuAction::new("package.empty"))
                .with_package_provenance("@clay/untrusted", "0.1.0"),
        ];
        items.extend((3..256).map(|index| {
            TransientMenuItem::new(
                format!("item-{index}"),
                format!("Item {index}"),
                TransientMenuAction::new("package.item"),
            )
            .with_package_provenance("@clay/untrusted", "0.1.0")
        }));
        let menu = TransientMenuSession::new(TransientMenuSessionId(11), "Package menu")
            .with_items(items)
            .with_selected_index(0);
        let overlay = TransientPackageOverlay::from_menu_session(&menu);
        let menu_a11y = overlay.menu_a11y.as_ref().expect("hosted menu a11y");
        assert_eq!(menu_a11y.items.len(), 256);
        assert_eq!(menu_a11y.items[1].label, "Safe fallback");
        assert_eq!(menu_a11y.items[2].label, "Menu item");
        assert_eq!(menu_a11y.items[0].label.chars().count(), 256);
        assert!(menu_a11y.items[0].label.ends_with(" selected"));
        assert!(menu_a11y.items.iter().all(|item| {
            item.label.chars().count() <= 256
                && !item.label.contains('/')
                && !item.label.contains('\\')
                && !item.label.chars().any(char::is_control)
        }));

        let cell = Rc::new(Cell::new(Rect::new(0.0, 0.0, 900.0, 600.0)));
        let host_widget = NewWidget::new(PackageOverlayHost::new(cell));
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        rr.edit_widget(host_id, |mut w| {
            let mut host = w
                .try_downcast::<PackageOverlayHost>()
                .expect("overlay host");
            host.widget.sync_overlays(
                &mut host.ctx,
                vec![overlay],
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            );
        });
        rr.handle_window_event(masonry::core::WindowEvent::EnableAccessTree);
        let (_, update) = rr.redraw();
        let update = update.expect("access tree active after EnableAccessTree");
        let labels: Vec<String> = update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::MenuItem)
            .map(|(_, node)| node.label().unwrap_or_default().to_string())
            .collect();
        assert_eq!(labels.len(), 256);
        let _tree = accesskit_consumer::Tree::new(update, false);
    }

    /// Plan 086 task 3: menu updates stay consumer-valid. Query/selection
    /// churn reuses stable node ids; closing the menu removes the subtree
    /// from the reachable tree (no panic, no stale nodes).
    #[test]
    fn consumer_accepts_menu_query_selection_and_close_updates() {
        use crate::shell::transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuSession, TransientMenuSessionId,
        };

        struct NoopChangeHandler;
        impl accesskit_consumer::TreeChangeHandler for NoopChangeHandler {
            fn node_added(&mut self, _node: &accesskit_consumer::Node) {}
            fn node_updated(
                &mut self,
                _old: &accesskit_consumer::Node,
                _new: &accesskit_consumer::Node,
            ) {
            }
            fn focus_moved(
                &mut self,
                _old: Option<&accesskit_consumer::Node>,
                _new: Option<&accesskit_consumer::Node>,
            ) {
            }
            fn node_removed(&mut self, _node: &accesskit_consumer::Node) {}
        }

        fn reachable_menu_item_labels(tree: &accesskit_consumer::Tree) -> Vec<String> {
            let mut out = Vec::new();
            let mut stack = vec![tree.state().root_id()];
            let mut seen = std::collections::HashSet::new();
            while let Some(id) = stack.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(node) = tree.state().node_by_id(id) else {
                    continue;
                };
                let data = node.data();
                if data.role() == Role::MenuItem {
                    out.push(data.label().unwrap_or("").to_string());
                }
                stack.extend(node.child_ids());
            }
            out
        }

        let menu = TransientMenuSession::new(TransientMenuSessionId(9), "Conflict recovery")
            .with_items(vec![
                TransientMenuItem::new(
                    "reload",
                    "Reload",
                    TransientMenuAction::new("documents.serverReloadDocument"),
                )
                .with_accessibility_label("Reload from disk"),
                TransientMenuItem::new(
                    "keep",
                    "Keep editing",
                    TransientMenuAction::new("documents.dismissConflict"),
                )
                .with_accessibility_label("Keep dirty buffer"),
            ]);
        let cell = Rc::new(Cell::new(Rect::new(0.0, 0.0, 900.0, 600.0)));
        let host_widget = NewWidget::new(PackageOverlayHost::new(cell));
        let host_id = host_widget.id();
        let mut rr = RenderRoot::new(host_widget, |_| {}, render_root_options());
        let sync = |rr: &mut RenderRoot, overlays: Vec<TransientPackageOverlay>| {
            rr.edit_widget(host_id, |mut w| {
                let mut host = w
                    .try_downcast::<PackageOverlayHost>()
                    .expect("overlay host");
                host.widget.sync_overlays(
                    &mut host.ctx,
                    overlays,
                    TypographyRegistry::default(),
                    ResolvedUiTheme::default(),
                );
            });
        };
        sync(
            &mut rr,
            vec![TransientPackageOverlay::from_menu_session(&menu)],
        );
        rr.handle_window_event(masonry::core::WindowEvent::EnableAccessTree);

        let (_, update) = rr.redraw();
        let mut tree = accesskit_consumer::Tree::new(update.expect("tree active"), false);
        let initial = reachable_menu_item_labels(&tree);
        assert_eq!(initial.len(), 2);
        assert!(
            initial
                .iter()
                .any(|label| label == "Reload from disk selected")
        );

        // Menu-selection: re-sync the same session with the second row
        // selected; item ids stay stable (no churn) and the suffix moves.
        let selected = menu.clone().with_selected_index(1);
        sync(
            &mut rr,
            vec![TransientPackageOverlay::from_menu_session(&selected)],
        );
        let (_, update) = rr.redraw();
        tree.update_and_process_changes(update.expect("tree active"), &mut NoopChangeHandler);
        let after_selection = reachable_menu_item_labels(&tree);
        assert_eq!(after_selection.len(), 2);
        assert!(
            after_selection
                .iter()
                .any(|label| label == "Keep dirty buffer selected")
        );

        // Menu-query: a narrowed item list reuses item slots 2+.
        let narrowed = TransientMenuSession::new(TransientMenuSessionId(9), "Conflict recovery")
            .with_items(vec![TransientMenuItem::new(
                "keep",
                "Keep editing",
                TransientMenuAction::new("documents.dismissConflict"),
            )]);
        sync(
            &mut rr,
            vec![TransientPackageOverlay::from_menu_session(&narrowed)],
        );
        let (_, update) = rr.redraw();
        tree.update_and_process_changes(update.expect("tree active"), &mut NoopChangeHandler);
        assert_eq!(reachable_menu_item_labels(&tree).len(), 1);

        // Menu-close: the overlay leaves the tree; no stale menu nodes stay
        // reachable.
        sync(&mut rr, vec![]);
        let (_, update) = rr.redraw();
        tree.update_and_process_changes(update.expect("tree active"), &mut NoopChangeHandler);
        assert!(
            reachable_menu_item_labels(&tree).is_empty(),
            "closed menu leaves no reachable items"
        );
    }
}
