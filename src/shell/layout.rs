// Phase 18.2 installs generic shell layout foundations before every split
// constructor/update path is exercised by non-test runtime code.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use masonry::kurbo::Rect;

const DEFAULT_WORKING_AREA_ID: WorkingAreaId = WorkingAreaId(1);
const DEFAULT_PANE_ID: PaneId = PaneId(1);
const DEFAULT_EDITOR_COMPONENT_ID: ShellComponentId = ShellComponentId(1);
const DEFAULT_LAYOUT_VERSION: ShellLayoutVersion = ShellLayoutVersion(1);
const MIN_SPLIT_RATIO: f64 = 0.05;
const MAX_SPLIT_RATIO: f64 = 0.95;
const MAX_PANE_SPLIT_TREE_NODES: usize = 64;
const MAX_PANE_SLOT_LAYOUTS: usize = MAX_PANE_SPLIT_TREE_NODES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellLayoutVersion(pub(crate) u64);

impl ShellLayoutVersion {
    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkingAreaId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PaneId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellComponentId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellComponentKind {
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellComponentBinding {
    pub(crate) id: ShellComponentId,
    pub(crate) kind: ShellComponentKind,
    pub(crate) pane_id: PaneId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PaneSlotId {
    Main,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FixedSlotId {
    Left,
    Right,
    Top,
    Bottom,
}

impl From<FixedSlotId> for PaneSlotId {
    fn from(slot_id: FixedSlotId) -> Self {
        match slot_id {
            FixedSlotId::Left => Self::Left,
            FixedSlotId::Right => Self::Right,
            FixedSlotId::Top => Self::Top,
            FixedSlotId::Bottom => Self::Bottom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MainSlotState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FixedSlotState {
    pub(crate) slot_id: FixedSlotId,
    pub(crate) size: f64,
    pub(crate) min_size: f64,
    pub(crate) max_size: f64,
    pub(crate) visible: bool,
    pub(crate) collapsed: bool,
    pub(crate) resized_by_user: bool,
}

impl FixedSlotState {
    pub(crate) fn new(
        slot_id: FixedSlotId,
        size: f64,
        min_size: f64,
        max_size: f64,
    ) -> Result<Self, PaneSlotLayoutError> {
        if !size.is_finite() || size < 0.0 {
            return Err(PaneSlotLayoutError::InvalidFixedSlotSize { slot_id, size });
        }
        if !min_size.is_finite() || !max_size.is_finite() || min_size < 0.0 || max_size < min_size {
            return Err(PaneSlotLayoutError::InvalidFixedSlotBounds {
                slot_id,
                min_size,
                max_size,
            });
        }

        Ok(Self {
            slot_id,
            size,
            min_size,
            max_size,
            visible: true,
            collapsed: false,
            resized_by_user: false,
        })
    }

    pub(crate) fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub(crate) fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub(crate) fn with_resized_by_user(mut self, resized_by_user: bool) -> Self {
        self.resized_by_user = resized_by_user;
        self
    }

    fn effective_size(self, available_extent: f64) -> f64 {
        if !self.visible || self.collapsed || available_extent <= 0.0 {
            return 0.0;
        }

        self.size
            .clamp(self.min_size, self.max_size)
            .min(available_extent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FixedSlotGeometry {
    pub(crate) slot_id: FixedSlotId,
    pub(crate) rect: Rect,
    pub(crate) resized_by_user: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneSlotGeometry {
    pub(crate) main_rect: Rect,
    pub(crate) fixed_slots: Vec<FixedSlotGeometry>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneSlotLayout {
    main: MainSlotState,
    left: Option<FixedSlotState>,
    right: Option<FixedSlotState>,
    top: Option<FixedSlotState>,
    bottom: Option<FixedSlotState>,
}

impl PaneSlotLayout {
    pub(crate) fn main_only() -> Self {
        Self {
            main: MainSlotState,
            left: None,
            right: None,
            top: None,
            bottom: None,
        }
    }

    pub(crate) fn with_fixed_slot(mut self, slot: FixedSlotState) -> Self {
        match slot.slot_id {
            FixedSlotId::Left => self.left = Some(slot),
            FixedSlotId::Right => self.right = Some(slot),
            FixedSlotId::Top => self.top = Some(slot),
            FixedSlotId::Bottom => self.bottom = Some(slot),
        }
        self
    }

    pub(crate) fn has_main_slot(&self) -> bool {
        let _ = self.main;
        true
    }

    pub(crate) fn contains_slot(&self, slot_id: PaneSlotId) -> bool {
        match slot_id {
            PaneSlotId::Main => true,
            PaneSlotId::Left => self.left.is_some(),
            PaneSlotId::Right => self.right.is_some(),
            PaneSlotId::Top => self.top.is_some(),
            PaneSlotId::Bottom => self.bottom.is_some(),
        }
    }

    pub(crate) fn compute_geometry(&self, pane_rect: Rect) -> PaneSlotGeometry {
        let mut remaining = normalized_rect(pane_rect);
        let mut fixed_slots = Vec::new();

        apply_horizontal_slot(
            self.left,
            FixedSlotId::Left,
            &mut remaining,
            &mut fixed_slots,
        );
        apply_horizontal_slot(
            self.right,
            FixedSlotId::Right,
            &mut remaining,
            &mut fixed_slots,
        );
        apply_vertical_slot(self.top, FixedSlotId::Top, &mut remaining, &mut fixed_slots);
        apply_vertical_slot(
            self.bottom,
            FixedSlotId::Bottom,
            &mut remaining,
            &mut fixed_slots,
        );

        PaneSlotGeometry {
            main_rect: normalized_rect(remaining),
            fixed_slots,
        }
    }

    fn fixed_slots(&self) -> impl Iterator<Item = FixedSlotState> + '_ {
        [self.left, self.right, self.top, self.bottom]
            .into_iter()
            .flatten()
    }
}

impl Default for PaneSlotLayout {
    fn default() -> Self {
        Self::main_only()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PaneSlotLayoutError {
    InvalidFixedSlotSize {
        slot_id: FixedSlotId,
        size: f64,
    },
    InvalidFixedSlotBounds {
        slot_id: FixedSlotId,
        min_size: f64,
        max_size: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitOrientation {
    /// Divides the pane rectangle into left and right regions.
    Horizontal,
    /// Divides the pane rectangle into top and bottom regions.
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SplitRatio(f64);

impl SplitRatio {
    pub(crate) fn new(value: f64) -> Result<Self, PaneSplitTreeError> {
        if value.is_finite() && (MIN_SPLIT_RATIO..=MAX_SPLIT_RATIO).contains(&value) {
            Ok(Self(value))
        } else {
            Err(PaneSplitTreeError::InvalidSplitRatio(value))
        }
    }

    pub(crate) fn balanced() -> Self {
        Self(0.5)
    }

    pub(crate) fn value(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaneSplitNode {
    Leaf {
        pane_id: PaneId,
    },
    Split {
        orientation: SplitOrientation,
        ratio: SplitRatio,
        first: Box<PaneSplitNode>,
        second: Box<PaneSplitNode>,
    },
}

impl PaneSplitNode {
    pub(crate) fn leaf(pane_id: PaneId) -> Self {
        Self::Leaf { pane_id }
    }

    pub(crate) fn split(
        orientation: SplitOrientation,
        ratio: SplitRatio,
        first: PaneSplitNode,
        second: PaneSplitNode,
    ) -> Self {
        Self::Split {
            orientation,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PaneGeometry {
    pub(crate) pane_id: PaneId,
    pub(crate) rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneSplitTree {
    root: PaneSplitNode,
    active_pane_id: PaneId,
    pane_count: usize,
    split_count: usize,
}

impl PaneSplitTree {
    pub(crate) fn single_leaf(pane_id: PaneId) -> Self {
        Self::new(PaneSplitNode::leaf(pane_id), pane_id)
            .expect("default pane split tree uses a valid nonzero pane ID")
    }

    pub(crate) fn new(
        root: PaneSplitNode,
        active_pane_id: PaneId,
    ) -> Result<Self, PaneSplitTreeError> {
        let mut validator = PaneSplitTreeValidator::default();
        validator.visit(&root)?;

        if !validator.pane_ids.contains(&active_pane_id) {
            return Err(PaneSplitTreeError::ActivePaneMissing(active_pane_id));
        }

        Ok(Self {
            root,
            active_pane_id,
            pane_count: validator.pane_count,
            split_count: validator.split_count,
        })
    }

    pub(crate) fn active_pane_id(&self) -> PaneId {
        self.active_pane_id
    }

    pub(crate) fn root_leaf_pane_id(&self) -> PaneId {
        self.first_leaf_pane_id(&self.root)
    }

    pub(crate) fn pane_count(&self) -> usize {
        self.pane_count
    }

    pub(crate) fn split_count(&self) -> usize {
        self.split_count
    }

    pub(crate) fn contains_pane(&self, pane_id: PaneId) -> bool {
        self.find_rect(pane_id, Rect::new(0.0, 0.0, 1.0, 1.0))
            .is_some()
    }

    pub(crate) fn pane_ids(&self) -> Vec<PaneId> {
        let mut pane_ids = Vec::with_capacity(self.pane_count);
        collect_pane_ids(&self.root, &mut pane_ids);
        pane_ids
    }

    pub(crate) fn compute_geometry(&self, area: Rect) -> Vec<PaneGeometry> {
        let mut geometry = Vec::with_capacity(self.pane_count);
        self.collect_geometry(&self.root, area, &mut geometry);
        geometry
    }

    fn observation(&self) -> PaneTreeObservation {
        observe_pane_tree_node(&self.root)
    }

    pub(crate) fn pane_rect(&self, pane_id: PaneId, area: Rect) -> Option<Rect> {
        self.find_rect(pane_id, area)
    }

    fn first_leaf_pane_id(&self, node: &PaneSplitNode) -> PaneId {
        match node {
            PaneSplitNode::Leaf { pane_id } => *pane_id,
            PaneSplitNode::Split { first, .. } => self.first_leaf_pane_id(first),
        }
    }

    fn collect_geometry(&self, node: &PaneSplitNode, area: Rect, geometry: &mut Vec<PaneGeometry>) {
        match node {
            PaneSplitNode::Leaf { pane_id } => geometry.push(PaneGeometry {
                pane_id: *pane_id,
                rect: normalized_rect(area),
            }),
            PaneSplitNode::Split {
                orientation,
                ratio,
                first,
                second,
            } => {
                let (first_rect, second_rect) = split_rect(area, *orientation, *ratio);
                self.collect_geometry(first, first_rect, geometry);
                self.collect_geometry(second, second_rect, geometry);
            }
        }
    }

    fn find_rect(&self, pane_id: PaneId, area: Rect) -> Option<Rect> {
        find_rect_in_node(&self.root, pane_id, area)
    }
}

impl Default for PaneSplitTree {
    fn default() -> Self {
        Self::single_leaf(DEFAULT_PANE_ID)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PaneSplitTreeError {
    InvalidPaneId(PaneId),
    DuplicatePaneId(PaneId),
    ActivePaneMissing(PaneId),
    InvalidSplitRatio(f64),
    TooManyNodes { nodes: usize, max: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaneTreeObservation {
    Leaf {
        pane_id: PaneId,
    },
    Split {
        orientation: SplitOrientation,
        ratio: f64,
        first: Box<PaneTreeObservation>,
        second: Box<PaneTreeObservation>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PaneSlotObservation {
    pub(crate) pane_id: PaneId,
    pub(crate) slot_id: PaneSlotId,
    pub(crate) rect: Rect,
    pub(crate) visible: bool,
    pub(crate) resized_by_user: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellComponentObservation {
    pub(crate) id: ShellComponentId,
    pub(crate) kind: ShellComponentKind,
    pub(crate) pane_id: PaneId,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkingAreaLayoutObservation {
    pub(crate) layout_version: ShellLayoutVersion,
    pub(crate) working_area_id: WorkingAreaId,
    pub(crate) pane_tree: PaneTreeObservation,
    pub(crate) pane_count: usize,
    pub(crate) split_count: usize,
    pub(crate) root_pane_id: PaneId,
    pub(crate) active_pane_id: PaneId,
    pub(crate) editor_component: ShellComponentObservation,
    pub(crate) slots: Vec<PaneSlotObservation>,
    pub(crate) editor_region_non_empty: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneSlotLayoutAssignment {
    pub(crate) pane_id: PaneId,
    pub(crate) layout: PaneSlotLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkingAreaLayoutUpdate {
    pub(crate) base_version: ShellLayoutVersion,
    pub(crate) working_area_id: WorkingAreaId,
    pub(crate) pane_tree: PaneSplitTree,
    pub(crate) editor_pane_id: PaneId,
    pub(crate) pane_slots: Vec<PaneSlotLayoutAssignment>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WorkingAreaLayoutUpdateError {
    StaleVersion {
        current: ShellLayoutVersion,
        update_base: ShellLayoutVersion,
    },
    WrongWorkingArea {
        current: WorkingAreaId,
        update: WorkingAreaId,
    },
    EditorPaneMissing(PaneId),
    SlotPaneMissing(PaneId),
    DuplicatePaneSlot(PaneId),
    TooManyPaneSlotLayouts {
        layouts: usize,
        max: usize,
    },
}

#[derive(Default)]
struct PaneSplitTreeValidator {
    node_count: usize,
    pane_count: usize,
    split_count: usize,
    pane_ids: BTreeSet<PaneId>,
}

impl PaneSplitTreeValidator {
    fn visit(&mut self, node: &PaneSplitNode) -> Result<(), PaneSplitTreeError> {
        self.node_count += 1;
        if self.node_count > MAX_PANE_SPLIT_TREE_NODES {
            return Err(PaneSplitTreeError::TooManyNodes {
                nodes: self.node_count,
                max: MAX_PANE_SPLIT_TREE_NODES,
            });
        }

        match node {
            PaneSplitNode::Leaf { pane_id } => {
                if pane_id.0 == 0 {
                    return Err(PaneSplitTreeError::InvalidPaneId(*pane_id));
                }
                if !self.pane_ids.insert(*pane_id) {
                    return Err(PaneSplitTreeError::DuplicatePaneId(*pane_id));
                }
                self.pane_count += 1;
            }
            PaneSplitNode::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if !ratio.value().is_finite()
                    || !(MIN_SPLIT_RATIO..=MAX_SPLIT_RATIO).contains(&ratio.value())
                {
                    return Err(PaneSplitTreeError::InvalidSplitRatio(ratio.value()));
                }
                self.split_count += 1;
                self.visit(first)?;
                self.visit(second)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkingAreaLayout {
    version: ShellLayoutVersion,
    working_area_id: WorkingAreaId,
    pane_tree: PaneSplitTree,
    pane_slots: BTreeMap<PaneId, PaneSlotLayout>,
    editor_component: ShellComponentBinding,
}

impl WorkingAreaLayout {
    pub(crate) fn single_editor() -> Self {
        let pane_tree = PaneSplitTree::default();
        let mut pane_slots = BTreeMap::new();
        pane_slots.insert(pane_tree.active_pane_id(), PaneSlotLayout::main_only());
        Self {
            version: DEFAULT_LAYOUT_VERSION,
            working_area_id: DEFAULT_WORKING_AREA_ID,
            editor_component: ShellComponentBinding {
                id: DEFAULT_EDITOR_COMPONENT_ID,
                kind: ShellComponentKind::Editor,
                pane_id: pane_tree.active_pane_id(),
            },
            pane_tree,
            pane_slots,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_pane_tree(
        pane_tree: PaneSplitTree,
        editor_pane_id: PaneId,
    ) -> Result<Self, PaneSplitTreeError> {
        if !pane_tree.contains_pane(editor_pane_id) {
            return Err(PaneSplitTreeError::ActivePaneMissing(editor_pane_id));
        }

        let pane_slots = pane_tree
            .pane_ids()
            .into_iter()
            .map(|pane_id| (pane_id, PaneSlotLayout::main_only()))
            .collect();

        Ok(Self {
            version: DEFAULT_LAYOUT_VERSION,
            working_area_id: DEFAULT_WORKING_AREA_ID,
            editor_component: ShellComponentBinding {
                id: DEFAULT_EDITOR_COMPONENT_ID,
                kind: ShellComponentKind::Editor,
                pane_id: editor_pane_id,
            },
            pane_tree,
            pane_slots,
        })
    }

    pub(crate) fn active_pane_id(&self) -> PaneId {
        self.pane_tree.active_pane_id()
    }

    pub(crate) fn version(&self) -> ShellLayoutVersion {
        self.version
    }

    pub(crate) fn working_area_id(&self) -> WorkingAreaId {
        self.working_area_id
    }

    pub(crate) fn root_pane_id(&self) -> PaneId {
        self.pane_tree.root_leaf_pane_id()
    }

    pub(crate) fn editor_component(&self) -> ShellComponentBinding {
        self.editor_component
    }

    #[cfg(test)]
    pub(crate) fn with_editor_pane_slot_layout(mut self, slot_layout: PaneSlotLayout) -> Self {
        self.pane_slots
            .insert(self.editor_component.pane_id, slot_layout);
        self
    }

    pub(crate) fn apply_update(
        &mut self,
        update: WorkingAreaLayoutUpdate,
    ) -> Result<(), WorkingAreaLayoutUpdateError> {
        self.validate_update(&update)?;

        let mut pane_slots = update
            .pane_tree
            .pane_ids()
            .into_iter()
            .map(|pane_id| (pane_id, PaneSlotLayout::main_only()))
            .collect::<BTreeMap<_, _>>();
        for assignment in update.pane_slots {
            pane_slots.insert(assignment.pane_id, assignment.layout);
        }

        self.version = self.version.next();
        self.working_area_id = update.working_area_id;
        self.editor_component.pane_id = update.editor_pane_id;
        self.pane_tree = update.pane_tree;
        self.pane_slots = pane_slots;
        Ok(())
    }

    pub(crate) fn observable_snapshot(&self, working_area: Rect) -> WorkingAreaLayoutObservation {
        let working_area = normalized_rect(working_area);
        let slots = self.slot_observations(working_area);
        let editor_rect = self.editor_component_rect(working_area);
        WorkingAreaLayoutObservation {
            layout_version: self.version,
            working_area_id: self.working_area_id,
            pane_tree: self.pane_tree.observation(),
            pane_count: self.pane_tree.pane_count(),
            split_count: self.pane_tree.split_count(),
            root_pane_id: self.pane_tree.root_leaf_pane_id(),
            active_pane_id: self.pane_tree.active_pane_id(),
            editor_component: ShellComponentObservation {
                id: self.editor_component.id,
                kind: self.editor_component.kind,
                pane_id: self.editor_component.pane_id,
            },
            slots,
            editor_region_non_empty: editor_rect.width() > 0.0 && editor_rect.height() > 0.0,
        }
    }

    pub(crate) fn pane_slot_geometry(
        &self,
        pane_id: PaneId,
        working_area: Rect,
    ) -> Option<PaneSlotGeometry> {
        let pane_rect = self.pane_tree.pane_rect(pane_id, working_area)?;
        Some(
            self.pane_slots
                .get(&pane_id)
                .map_or_else(PaneSlotLayout::main_only, Clone::clone)
                .compute_geometry(pane_rect),
        )
    }

    pub(crate) fn editor_component_rect(&self, working_area: Rect) -> Rect {
        self.pane_slot_geometry(self.editor_component.pane_id, working_area)
            .map(|geometry| geometry.main_rect)
            .unwrap_or_else(|| normalized_rect(working_area))
    }

    fn validate_update(
        &self,
        update: &WorkingAreaLayoutUpdate,
    ) -> Result<(), WorkingAreaLayoutUpdateError> {
        if update.base_version != self.version {
            return Err(WorkingAreaLayoutUpdateError::StaleVersion {
                current: self.version,
                update_base: update.base_version,
            });
        }
        if update.working_area_id != self.working_area_id {
            return Err(WorkingAreaLayoutUpdateError::WrongWorkingArea {
                current: self.working_area_id,
                update: update.working_area_id,
            });
        }
        if !update.pane_tree.contains_pane(update.editor_pane_id) {
            return Err(WorkingAreaLayoutUpdateError::EditorPaneMissing(
                update.editor_pane_id,
            ));
        }
        if update.pane_slots.len() > update.pane_tree.pane_count()
            || update.pane_slots.len() > MAX_PANE_SLOT_LAYOUTS
        {
            return Err(WorkingAreaLayoutUpdateError::TooManyPaneSlotLayouts {
                layouts: update.pane_slots.len(),
                max: update.pane_tree.pane_count().min(MAX_PANE_SLOT_LAYOUTS),
            });
        }

        let mut pane_ids = BTreeSet::new();
        for assignment in &update.pane_slots {
            if !update.pane_tree.contains_pane(assignment.pane_id) {
                return Err(WorkingAreaLayoutUpdateError::SlotPaneMissing(
                    assignment.pane_id,
                ));
            }
            if !pane_ids.insert(assignment.pane_id) {
                return Err(WorkingAreaLayoutUpdateError::DuplicatePaneSlot(
                    assignment.pane_id,
                ));
            }
        }
        Ok(())
    }

    fn slot_observations(&self, working_area: Rect) -> Vec<PaneSlotObservation> {
        let mut slots = Vec::new();
        for pane in self.pane_tree.compute_geometry(working_area) {
            let layout = self
                .pane_slots
                .get(&pane.pane_id)
                .map_or_else(PaneSlotLayout::main_only, Clone::clone);
            let geometry = layout.compute_geometry(pane.rect);
            slots.push(PaneSlotObservation {
                pane_id: pane.pane_id,
                slot_id: PaneSlotId::Main,
                rect: geometry.main_rect,
                visible: geometry.main_rect.width() > 0.0 && geometry.main_rect.height() > 0.0,
                resized_by_user: false,
            });
            for fixed_slot in layout.fixed_slots() {
                let fixed_geometry = geometry
                    .fixed_slots
                    .iter()
                    .find(|slot| slot.slot_id == fixed_slot.slot_id);
                slots.push(PaneSlotObservation {
                    pane_id: pane.pane_id,
                    slot_id: fixed_slot.slot_id.into(),
                    rect: fixed_geometry.map_or(Rect::ZERO, |slot| slot.rect),
                    visible: fixed_geometry.is_some(),
                    resized_by_user: fixed_slot.resized_by_user,
                });
            }
        }
        slots
    }
}

impl Default for WorkingAreaLayout {
    fn default() -> Self {
        Self::single_editor()
    }
}

fn collect_pane_ids(node: &PaneSplitNode, pane_ids: &mut Vec<PaneId>) {
    match node {
        PaneSplitNode::Leaf { pane_id } => pane_ids.push(*pane_id),
        PaneSplitNode::Split { first, second, .. } => {
            collect_pane_ids(first, pane_ids);
            collect_pane_ids(second, pane_ids);
        }
    }
}

fn observe_pane_tree_node(node: &PaneSplitNode) -> PaneTreeObservation {
    match node {
        PaneSplitNode::Leaf { pane_id } => PaneTreeObservation::Leaf { pane_id: *pane_id },
        PaneSplitNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => PaneTreeObservation::Split {
            orientation: *orientation,
            ratio: ratio.value(),
            first: Box::new(observe_pane_tree_node(first)),
            second: Box::new(observe_pane_tree_node(second)),
        },
    }
}

fn apply_horizontal_slot(
    slot: Option<FixedSlotState>,
    slot_id: FixedSlotId,
    remaining: &mut Rect,
    fixed_slots: &mut Vec<FixedSlotGeometry>,
) {
    let Some(slot) = slot else {
        return;
    };
    let size = slot.effective_size(remaining.width());
    if size <= 0.0 {
        return;
    }

    let rect = match slot_id {
        FixedSlotId::Left => {
            let rect = Rect::new(
                remaining.x0,
                remaining.y0,
                remaining.x0 + size,
                remaining.y1,
            );
            remaining.x0 = rect.x1;
            rect
        }
        FixedSlotId::Right => {
            let rect = Rect::new(
                remaining.x1 - size,
                remaining.y0,
                remaining.x1,
                remaining.y1,
            );
            remaining.x1 = rect.x0;
            rect
        }
        FixedSlotId::Top | FixedSlotId::Bottom => return,
    };

    fixed_slots.push(FixedSlotGeometry {
        slot_id,
        rect: normalized_rect(rect),
        resized_by_user: slot.resized_by_user,
    });
}

fn apply_vertical_slot(
    slot: Option<FixedSlotState>,
    slot_id: FixedSlotId,
    remaining: &mut Rect,
    fixed_slots: &mut Vec<FixedSlotGeometry>,
) {
    let Some(slot) = slot else {
        return;
    };
    let size = slot.effective_size(remaining.height());
    if size <= 0.0 {
        return;
    }

    let rect = match slot_id {
        FixedSlotId::Top => {
            let rect = Rect::new(
                remaining.x0,
                remaining.y0,
                remaining.x1,
                remaining.y0 + size,
            );
            remaining.y0 = rect.y1;
            rect
        }
        FixedSlotId::Bottom => {
            let rect = Rect::new(
                remaining.x0,
                remaining.y1 - size,
                remaining.x1,
                remaining.y1,
            );
            remaining.y1 = rect.y0;
            rect
        }
        FixedSlotId::Left | FixedSlotId::Right => return,
    };

    fixed_slots.push(FixedSlotGeometry {
        slot_id,
        rect: normalized_rect(rect),
        resized_by_user: slot.resized_by_user,
    });
}

fn find_rect_in_node(node: &PaneSplitNode, pane_id: PaneId, area: Rect) -> Option<Rect> {
    match node {
        PaneSplitNode::Leaf { pane_id: current } => {
            (*current == pane_id).then(|| normalized_rect(area))
        }
        PaneSplitNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => {
            let (first_rect, second_rect) = split_rect(area, *orientation, *ratio);
            find_rect_in_node(first, pane_id, first_rect)
                .or_else(|| find_rect_in_node(second, pane_id, second_rect))
        }
    }
}

fn split_rect(area: Rect, orientation: SplitOrientation, ratio: SplitRatio) -> (Rect, Rect) {
    let area = normalized_rect(area);
    match orientation {
        SplitOrientation::Horizontal => {
            let split_x = area.x0 + area.width() * ratio.value();
            (
                Rect::new(area.x0, area.y0, split_x, area.y1),
                Rect::new(split_x, area.y0, area.x1, area.y1),
            )
        }
        SplitOrientation::Vertical => {
            let split_y = area.y0 + area.height() * ratio.value();
            (
                Rect::new(area.x0, area.y0, area.x1, split_y),
                Rect::new(area.x0, split_y, area.x1, area.y1),
            )
        }
    }
}

fn normalized_rect(area: Rect) -> Rect {
    Rect::new(
        area.x0.min(area.x1),
        area.y0.min(area.y1),
        area.x0.max(area.x1),
        area.y0.max(area.y1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rect_eq(actual: Rect, expected: Rect) {
        const EPSILON: f64 = 0.000_001;
        assert!((actual.x0 - expected.x0).abs() < EPSILON, "x0: {actual:?}");
        assert!((actual.y0 - expected.y0).abs() < EPSILON, "y0: {actual:?}");
        assert!((actual.x1 - expected.x1).abs() < EPSILON, "x1: {actual:?}");
        assert!((actual.y1 - expected.y1).abs() < EPSILON, "y1: {actual:?}");
    }

    #[test]
    fn pane_slot_layout_requires_main_slot() {
        let layout = PaneSlotLayout::main_only();

        let geometry = layout.compute_geometry(Rect::new(0.0, 0.0, 800.0, 600.0));

        assert!(layout.has_main_slot());
        assert!(layout.contains_slot(PaneSlotId::Main));
        assert!(!layout.contains_slot(PaneSlotId::Left));
        assert_eq!(geometry.fixed_slots, Vec::new());
        assert_rect_eq(geometry.main_rect, Rect::new(0.0, 0.0, 800.0, 600.0));
    }

    #[test]
    fn pane_slot_layout_computes_main_with_left_right_top_bottom_slots() {
        let layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 50.0, 20.0, 120.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Right, 70.0, 20.0, 120.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Top, 30.0, 10.0, 100.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Bottom, 40.0, 10.0, 100.0).unwrap());

        let geometry = layout.compute_geometry(Rect::new(0.0, 0.0, 1000.0, 800.0));

        assert_rect_eq(
            geometry.fixed_slots[0].rect,
            Rect::new(0.0, 0.0, 50.0, 800.0),
        );
        assert_eq!(geometry.fixed_slots[0].slot_id, FixedSlotId::Left);
        assert_rect_eq(
            geometry.fixed_slots[1].rect,
            Rect::new(930.0, 0.0, 1000.0, 800.0),
        );
        assert_eq!(geometry.fixed_slots[1].slot_id, FixedSlotId::Right);
        assert_rect_eq(
            geometry.fixed_slots[2].rect,
            Rect::new(50.0, 0.0, 930.0, 30.0),
        );
        assert_eq!(geometry.fixed_slots[2].slot_id, FixedSlotId::Top);
        assert_rect_eq(
            geometry.fixed_slots[3].rect,
            Rect::new(50.0, 760.0, 930.0, 800.0),
        );
        assert_eq!(geometry.fixed_slots[3].slot_id, FixedSlotId::Bottom);
        assert_rect_eq(geometry.main_rect, Rect::new(50.0, 30.0, 930.0, 760.0));
    }

    #[test]
    fn pane_slot_layout_clamps_fixed_panel_sizes() {
        let layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 20.0, 48.0, 120.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Right, 500.0, 48.0, 100.0).unwrap())
            .with_fixed_slot(
                FixedSlotState::new(FixedSlotId::Top, 80.0, 40.0, 120.0)
                    .unwrap()
                    .with_collapsed(true),
            )
            .with_fixed_slot(
                FixedSlotState::new(FixedSlotId::Bottom, 80.0, 40.0, 120.0)
                    .unwrap()
                    .with_visible(false),
            );

        let geometry = layout.compute_geometry(Rect::new(0.0, 0.0, 500.0, 300.0));

        assert_eq!(geometry.fixed_slots.len(), 2);
        assert_rect_eq(
            geometry.fixed_slots[0].rect,
            Rect::new(0.0, 0.0, 48.0, 300.0),
        );
        assert_rect_eq(
            geometry.fixed_slots[1].rect,
            Rect::new(400.0, 0.0, 500.0, 300.0),
        );
        assert_rect_eq(geometry.main_rect, Rect::new(48.0, 0.0, 400.0, 300.0));
    }

    #[test]
    fn pane_slot_layout_rejects_invalid_fixed_panel_sizes() {
        assert!(matches!(
            FixedSlotState::new(FixedSlotId::Left, -1.0, 0.0, 100.0),
            Err(PaneSlotLayoutError::InvalidFixedSlotSize { .. })
        ));
        assert!(matches!(
            FixedSlotState::new(FixedSlotId::Right, f64::NAN, 0.0, 100.0),
            Err(PaneSlotLayoutError::InvalidFixedSlotSize { .. })
        ));
        assert!(matches!(
            FixedSlotState::new(FixedSlotId::Bottom, 40.0, 90.0, 20.0),
            Err(PaneSlotLayoutError::InvalidFixedSlotBounds { .. })
        ));
    }

    #[test]
    fn working_area_editor_component_uses_main_slot_rect() {
        let slot_layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 200.0, 120.0, 320.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Bottom, 80.0, 40.0, 120.0).unwrap());
        let layout = WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout);

        let editor_rect = layout.editor_component_rect(Rect::new(0.0, 0.0, 900.0, 600.0));

        assert_rect_eq(editor_rect, Rect::new(200.0, 0.0, 900.0, 520.0));
    }

    #[test]
    fn working_area_layout_applies_inert_validated_update() {
        let mut layout = WorkingAreaLayout::single_editor();
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(2),
        )
        .unwrap();
        let slot_layout = PaneSlotLayout::main_only().with_fixed_slot(
            FixedSlotState::new(FixedSlotId::Right, 180.0, 80.0, 240.0)
                .unwrap()
                .with_resized_by_user(true),
        );

        layout
            .apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree,
                editor_pane_id: PaneId(2),
                pane_slots: vec![PaneSlotLayoutAssignment {
                    pane_id: PaneId(2),
                    layout: slot_layout,
                }],
            })
            .unwrap();

        let snapshot = layout.observable_snapshot(Rect::new(0.0, 0.0, 1000.0, 600.0));

        assert_eq!(snapshot.layout_version, ShellLayoutVersion(2));
        assert_eq!(snapshot.active_pane_id, PaneId(2));
        assert_eq!(snapshot.editor_component.pane_id, PaneId(2));
        assert_eq!(snapshot.pane_count, 2);
        assert_eq!(snapshot.split_count, 1);
        assert!(matches!(
            snapshot.pane_tree,
            PaneTreeObservation::Split { .. }
        ));
        assert!(snapshot.editor_region_non_empty);
        assert!(snapshot.slots.iter().any(|slot| {
            slot.pane_id == PaneId(2)
                && slot.slot_id == PaneSlotId::Right
                && slot.visible
                && slot.resized_by_user
        }));
    }

    #[test]
    fn shell_layout_update_rejects_stale_or_oversize_payload() {
        let mut layout = WorkingAreaLayout::single_editor();
        let stale_update = WorkingAreaLayoutUpdate {
            base_version: ShellLayoutVersion(0),
            working_area_id: WorkingAreaId(1),
            pane_tree: PaneSplitTree::default(),
            editor_pane_id: PaneId(1),
            pane_slots: Vec::new(),
        };

        assert_eq!(
            layout.apply_update(stale_update),
            Err(WorkingAreaLayoutUpdateError::StaleVersion {
                current: ShellLayoutVersion(1),
                update_base: ShellLayoutVersion(0),
            })
        );

        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap();
        let oversize_update = WorkingAreaLayoutUpdate {
            base_version: ShellLayoutVersion(1),
            working_area_id: WorkingAreaId(1),
            pane_tree: tree,
            editor_pane_id: PaneId(1),
            pane_slots: vec![
                PaneSlotLayoutAssignment {
                    pane_id: PaneId(1),
                    layout: PaneSlotLayout::main_only(),
                },
                PaneSlotLayoutAssignment {
                    pane_id: PaneId(2),
                    layout: PaneSlotLayout::main_only(),
                },
                PaneSlotLayoutAssignment {
                    pane_id: PaneId(3),
                    layout: PaneSlotLayout::main_only(),
                },
            ],
        };

        assert_eq!(
            layout.apply_update(oversize_update),
            Err(WorkingAreaLayoutUpdateError::TooManyPaneSlotLayouts { layouts: 3, max: 2 })
        );
    }

    #[test]
    fn shell_layout_update_rejects_malformed_slot_and_editor_targets() {
        let mut layout = WorkingAreaLayout::single_editor();
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap();

        assert_eq!(
            layout.apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree.clone(),
                editor_pane_id: PaneId(99),
                pane_slots: Vec::new(),
            }),
            Err(WorkingAreaLayoutUpdateError::EditorPaneMissing(PaneId(99)))
        );

        assert_eq!(
            layout.apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree,
                editor_pane_id: PaneId(1),
                pane_slots: vec![
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(1),
                        layout: PaneSlotLayout::main_only(),
                    },
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(1),
                        layout: PaneSlotLayout::main_only(),
                    },
                ],
            }),
            Err(WorkingAreaLayoutUpdateError::DuplicatePaneSlot(PaneId(1)))
        );
    }

    #[test]
    fn pane_split_tree_default_has_one_leaf() {
        let tree = PaneSplitTree::default();

        assert_eq!(tree.active_pane_id(), PaneId(1));
        assert_eq!(tree.root_leaf_pane_id(), PaneId(1));
        assert_eq!(tree.pane_count(), 1);
        assert_eq!(tree.split_count(), 0);
        assert_eq!(
            tree.compute_geometry(Rect::new(0.0, 0.0, 800.0, 600.0)),
            vec![PaneGeometry {
                pane_id: PaneId(1),
                rect: Rect::new(0.0, 0.0, 800.0, 600.0),
            }]
        );
    }

    #[test]
    fn pane_split_tree_rejects_duplicate_pane_ids() {
        let tree = PaneSplitNode::split(
            SplitOrientation::Horizontal,
            SplitRatio::balanced(),
            PaneSplitNode::leaf(PaneId(7)),
            PaneSplitNode::leaf(PaneId(7)),
        );

        assert_eq!(
            PaneSplitTree::new(tree, PaneId(7)),
            Err(PaneSplitTreeError::DuplicatePaneId(PaneId(7)))
        );
    }

    #[test]
    fn pane_split_tree_rejects_invalid_ratios() {
        assert!(matches!(
            SplitRatio::new(0.0),
            Err(PaneSplitTreeError::InvalidSplitRatio(0.0))
        ));
        assert!(matches!(
            SplitRatio::new(1.0),
            Err(PaneSplitTreeError::InvalidSplitRatio(1.0))
        ));
        assert!(matches!(
            SplitRatio::new(f64::INFINITY),
            Err(PaneSplitTreeError::InvalidSplitRatio(value)) if value.is_infinite()
        ));
        assert!(matches!(
            SplitRatio::new(f64::NAN),
            Err(PaneSplitTreeError::InvalidSplitRatio(value)) if value.is_nan()
        ));
    }

    #[test]
    fn pane_split_tree_rejects_oversize_tree_payloads() {
        let mut root = PaneSplitNode::leaf(PaneId(1));
        for pane_id in 2..=33 {
            root = PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                root,
                PaneSplitNode::leaf(PaneId(pane_id)),
            );
        }

        assert_eq!(
            PaneSplitTree::new(root, PaneId(1)),
            Err(PaneSplitTreeError::TooManyNodes { nodes: 65, max: 64 })
        );
    }

    #[test]
    fn pane_split_tree_computes_horizontal_and_vertical_geometry() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::new(0.25).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::split(
                    SplitOrientation::Vertical,
                    SplitRatio::new(0.75).unwrap(),
                    PaneSplitNode::leaf(PaneId(2)),
                    PaneSplitNode::leaf(PaneId(3)),
                ),
            ),
            PaneId(2),
        )
        .unwrap();

        let geometry = tree.compute_geometry(Rect::new(0.0, 0.0, 1000.0, 800.0));

        assert_eq!(tree.active_pane_id(), PaneId(2));
        assert_eq!(tree.pane_count(), 3);
        assert_eq!(tree.split_count(), 2);
        assert_rect_eq(geometry[0].rect, Rect::new(0.0, 0.0, 250.0, 800.0));
        assert_eq!(geometry[0].pane_id, PaneId(1));
        assert_rect_eq(geometry[1].rect, Rect::new(250.0, 0.0, 1000.0, 600.0));
        assert_eq!(geometry[1].pane_id, PaneId(2));
        assert_rect_eq(geometry[2].rect, Rect::new(250.0, 600.0, 1000.0, 800.0));
        assert_eq!(geometry[2].pane_id, PaneId(3));
        assert_rect_eq(
            tree.pane_rect(PaneId(2), Rect::new(0.0, 0.0, 1000.0, 800.0))
                .unwrap(),
            Rect::new(250.0, 0.0, 1000.0, 600.0),
        );
    }
}
