// Phase 18.2 installs generic shell layout foundations before every split
// constructor/update path is exercised by non-test runtime code.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use masonry::kurbo::{Point, Rect};

const DEFAULT_WORKING_AREA_ID: WorkingAreaId = WorkingAreaId(1);
pub(crate) const DEFAULT_PANE_ID: PaneId = PaneId(1);
const DEFAULT_EDITOR_COMPONENT_ID: ShellComponentId = ShellComponentId(1);
const DEFAULT_LAYOUT_VERSION: ShellLayoutVersion = ShellLayoutVersion(1);
const MIN_SPLIT_RATIO: f64 = 0.05;
const MAX_SPLIT_RATIO: f64 = 0.95;
pub(crate) const MAX_PANE_SPLIT_TREE_NODES: usize = 64;
const MAX_PANE_SLOT_LAYOUTS: usize = MAX_PANE_SPLIT_TREE_NODES;

/// Phase 22.1 product cap: at most 4 panes per tab (roadmap Phase 22). The tree
/// model itself stays generic (`MAX_PANE_SPLIT_TREE_NODES`); user-facing pane
/// operations enforce this cap.
pub(crate) const MAX_PANES_PER_TAB: usize = 4;
/// Default keyboard resize step for split ratios (fraction of the parent area).
pub(crate) const KEYBOARD_RESIZE_STEP: f64 = 0.05;

/// Extra pointer hit-test slop on each side of the 1px divider line.
/// Visual width comes from `dimension.border.hairline`; this is interaction-only.
const DIVIDER_HIT_SLOP: f64 = 4.0;
/// Hit-test slop for fixed slot resize handles (px each side of the inner edge).
const SLOT_HANDLE_HIT_SLOP: f64 = 4.0;

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
/// Pane leaf identity in a working-area split tree. Doc-hidden: reachable by
/// the native `clay` binary for pane routing; not a Clay JS API.
#[doc(hidden)]
#[derive(Default)]
pub struct PaneId(pub u64);

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

    /// Resize to `new_size`, clamped to `min_size..=max_size`. Sets `resized_by_user`.
    pub(crate) fn resize_to(&mut self, new_size: f64) {
        self.size = new_size.clamp(self.min_size, self.max_size);
        self.resized_by_user = true;
    }

    /// Toggle the collapsed state. When collapsed, `effective_size()` returns 0.0.
    pub(crate) fn toggle_collapse(&mut self) {
        self.collapsed = !self.collapsed;
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

    /// Mutable access to a fixed slot by ID.
    pub(crate) fn fixed_slot_mut(&mut self, slot_id: FixedSlotId) -> Option<&mut FixedSlotState> {
        match slot_id {
            FixedSlotId::Left => self.left.as_mut(),
            FixedSlotId::Right => self.right.as_mut(),
            FixedSlotId::Top => self.top.as_mut(),
            FixedSlotId::Bottom => self.bottom.as_mut(),
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

    /// Return fixed slots that have been user-modified (`resized_by_user` or `collapsed`).
    pub(crate) fn user_modified_slots(&self) -> Vec<FixedSlotState> {
        self.fixed_slots()
            .filter(|s| s.resized_by_user || s.collapsed)
            .collect()
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

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrientation {
    /// Divides the pane rectangle into left and right regions.
    Horizontal,
    /// Divides the pane rectangle into top and bottom regions.
    Vertical,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitRatio(f64);

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

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub enum PaneSplitNode {
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

    /// Access the root node of the split tree (for persistence traversal).
    pub(crate) fn root_node(&self) -> &PaneSplitNode {
        &self.root
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

    // -- Phase 20.3: focus traversal --

    /// Next pane in reading order (wraps around). Single-leaf returns self.
    pub(crate) fn next_pane(&self) -> PaneId {
        let ids = self.pane_ids();
        let idx = ids
            .iter()
            .position(|p| *p == self.active_pane_id)
            .unwrap_or(0);
        ids[(idx + 1) % ids.len()]
    }

    /// Previous pane in reading order (wraps around). Single-leaf returns self.
    pub(crate) fn prev_pane(&self) -> PaneId {
        let ids = self.pane_ids();
        let idx = ids
            .iter()
            .position(|p| *p == self.active_pane_id)
            .unwrap_or(0);
        ids[(idx + ids.len() - 1) % ids.len()]
    }

    /// Set the active pane. Returns error if `pane_id` is not in the tree.
    pub(crate) fn set_active_pane(&mut self, pane_id: PaneId) -> Result<(), PaneSplitTreeError> {
        if !self.contains_pane(pane_id) {
            return Err(PaneSplitTreeError::ActivePaneMissing(pane_id));
        }
        self.active_pane_id = pane_id;
        Ok(())
    }

    fn first_leaf_pane_id(&self, node: &PaneSplitNode) -> PaneId {
        first_leaf_pane_id(node)
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

    // -- Phase 20.3: split divider drag interaction --

    /// Read the ratio of the split node at `path`.
    pub(crate) fn split_ratio_at_path(&self, path: &[SplitChild]) -> Option<SplitRatio> {
        match split_node_at_path(&self.root, path)? {
            PaneSplitNode::Split { ratio, .. } => Some(*ratio),
            _ => None,
        }
    }

    /// Update the ratio of the split node at `path` in place.
    /// Returns `false` if the path does not resolve to a split node.
    pub(crate) fn update_split_ratio(&mut self, path: &[SplitChild], ratio: SplitRatio) -> bool {
        if let Some(PaneSplitNode::Split { ratio: current, .. }) =
            split_node_at_path_mut(&mut self.root, path)
        {
            *current = ratio;
            true
        } else {
            false
        }
    }

    /// Collect all divider rects for paint/hit-test.
    pub(crate) fn divider_rects(&self, area: Rect) -> Vec<SplitDividerRect> {
        collect_split_dividers(self, area)
    }

    /// Phase 20.3: Split an existing pane, replacing its leaf with a split node.
    ///
    /// Returns a new tree with `target_pane` split into two panes. The new pane
    /// gets `new_pane_id`. `position` controls whether the new pane is `First` or `Second`.
    /// Returns `None` if `target_pane` is not found, `new_pane_id` already exists,
    /// or the tree is already at [`MAX_PANES_PER_TAB`] panes (Phase 22.1 cap).
    pub(crate) fn split_pane(
        &self,
        target_pane: PaneId,
        new_pane_id: PaneId,
        orientation: SplitOrientation,
        ratio: SplitRatio,
        position: SplitChild,
    ) -> Option<PaneSplitTree> {
        if self.pane_count >= MAX_PANES_PER_TAB
            || self.contains_pane(new_pane_id)
            || !self.contains_pane(target_pane)
        {
            return None;
        }
        let new_root = split_node_pane(
            &self.root,
            target_pane,
            new_pane_id,
            orientation,
            ratio,
            position,
        )?;
        PaneSplitTree::new(new_root, self.active_pane_id).ok()
    }

    /// Next available pane ID (max existing + 1).
    pub(crate) fn next_pane_id(&self) -> PaneId {
        let max = self.pane_ids().iter().map(|p| p.0).max().unwrap_or(0);
        PaneId(max + 1)
    }

    // -- Phase 22.1: pane lifecycle operations --

    /// Phase 22.1: Close a pane, merging its area with its sibling subtree.
    ///
    /// The leaf for `pane_id` is replaced by its sibling subtree, so surviving
    /// panes keep reading order and fill the vacated area. Returns `None` when
    /// `pane_id` is missing or is the last pane. If the closed pane was active,
    /// focus moves to the first leaf of the promoted sibling subtree.
    pub(crate) fn close_pane(&self, pane_id: PaneId) -> Option<PaneSplitTree> {
        if self.pane_count <= 1 || !self.contains_pane(pane_id) {
            return None;
        }
        let (new_root, sibling_first_leaf) = close_node_pane(&self.root, pane_id)?;
        let active = if self.active_pane_id == pane_id {
            sibling_first_leaf
        } else {
            self.active_pane_id
        };
        PaneSplitTree::new(new_root, active).ok()
    }

    /// Phase 22.1: Redivide the whole tree into `pane_count + 1` equal-area leaves.
    ///
    /// Existing panes keep their IDs and reading order; the new empty pane is
    /// appended last with [`Self::next_pane_id`]. The redivision follows the root
    /// split orientation; a single leaf becomes two side-by-side panes
    /// (`SplitOrientation::Horizontal`). Equal areas are expressed as a
    /// right-leaning comb with ratios `1/(N+1), 1/N, ..., 1/2`. Returns `None`
    /// at [`MAX_PANES_PER_TAB`].
    pub(crate) fn add_equal_pane(&self) -> Option<PaneSplitTree> {
        if self.pane_count >= MAX_PANES_PER_TAB {
            return None;
        }
        let orientation = match &self.root {
            PaneSplitNode::Split { orientation, .. } => *orientation,
            PaneSplitNode::Leaf { .. } => SplitOrientation::Horizontal,
        };
        let mut leaves = self.pane_ids();
        leaves.push(self.next_pane_id());
        let root = equal_comb_tree(&leaves, orientation);
        PaneSplitTree::new(root, self.active_pane_id).ok()
    }

    /// Phase 22.1: Swap a pane with its neighbor in reading order.
    ///
    /// Tree shape and ratios are unchanged; only the two leaf IDs swap places.
    /// The moved pane keeps focus if it was active. Returns `None` when
    /// `pane_id` is missing or already at the reading-order end for `direction`
    /// (`First` = toward the start, `Second` = toward the end).
    pub(crate) fn move_pane(
        &self,
        pane_id: PaneId,
        direction: SplitChild,
    ) -> Option<PaneSplitTree> {
        let ids = self.pane_ids();
        let idx = ids.iter().position(|p| *p == pane_id)?;
        let neighbor_idx = match direction {
            SplitChild::First => idx.checked_sub(1)?,
            SplitChild::Second => idx.checked_add(1).filter(|i| *i < ids.len())?,
        };
        let new_root = swap_leaf_pane_ids(&self.root, pane_id, ids[neighbor_idx]);
        PaneSplitTree::new(new_root, self.active_pane_id).ok()
    }

    /// Phase 22.1: Compute one keyboard resize step for the divider bordering
    /// `pane_id` in `direction`.
    ///
    /// Finds the deepest ancestor split whose divider directly borders the pane
    /// on the requested side and returns its path plus the new clamped ratio for
    /// [`Self::update_split_ratio`]. Returns `None` when the pane is missing,
    /// `step` is not positive/finite, no divider borders the pane in that
    /// direction, or the clamped step cannot move the ratio (already at
    /// `MIN_SPLIT_RATIO`/`MAX_SPLIT_RATIO`).
    pub(crate) fn keyboard_resize(
        &self,
        pane_id: PaneId,
        direction: PaneResizeDirection,
        step: f64,
    ) -> Option<(SplitPath, SplitRatio)> {
        if !self.contains_pane(pane_id) || !step.is_finite() || step <= 0.0 {
            return None;
        }
        let leaf_path = leaf_path_in_node(&self.root, pane_id)?;
        let (axis, required_side) = match direction {
            PaneResizeDirection::Left => (SplitOrientation::Horizontal, SplitChild::Second),
            PaneResizeDirection::Right => (SplitOrientation::Horizontal, SplitChild::First),
            PaneResizeDirection::Up => (SplitOrientation::Vertical, SplitChild::Second),
            PaneResizeDirection::Down => (SplitOrientation::Vertical, SplitChild::First),
        };
        for depth in (0..leaf_path.len()).rev() {
            let child_side = leaf_path[depth];
            if child_side != required_side {
                continue;
            }
            let split_path = leaf_path[..depth].to_vec();
            let PaneSplitNode::Split {
                orientation, ratio, ..
            } = split_node_at_path(&self.root, &split_path)?
            else {
                continue;
            };
            if *orientation != axis {
                continue;
            }
            let delta = match child_side {
                SplitChild::First => step,
                SplitChild::Second => -step,
            };
            let new_value = (ratio.value() + delta).clamp(MIN_SPLIT_RATIO, MAX_SPLIT_RATIO);
            if (new_value - ratio.value()).abs() < f64::EPSILON {
                return None;
            }
            return Some((split_path, SplitRatio::new(new_value).ok()?));
        }
        None
    }
}

impl Default for PaneSplitTree {
    fn default() -> Self {
        Self::single_leaf(DEFAULT_PANE_ID)
    }
}

// ---------------------------------------------------------------------------
// Phase 20.3: Split divider drag interaction
// ---------------------------------------------------------------------------

/// Which child of a `PaneSplitNode::Split` to descend into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitChild {
    First,
    Second,
}

/// Phase 22.1: Keyboard resize direction for the divider bordering a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneResizeDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Path from the tree root to a specific split node.
/// Empty vec means the root itself (only valid when root is a Split).
pub(crate) type SplitPath = Vec<SplitChild>;

/// Result of hit-testing a point against split dividers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SplitDividerHit {
    /// Path to the split node whose divider was hit.
    pub(crate) path: SplitPath,
    /// Orientation of the split (determines drag axis).
    pub(crate) orientation: SplitOrientation,
    /// The full rect of the split node's area (parent rect for ratio computation).
    pub(crate) parent_rect: Rect,
}

/// Drag session state for split divider interaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum SplitDragState {
    #[default]
    Idle,
    Dragging {
        path: SplitPath,
        orientation: SplitOrientation,
        parent_rect: Rect,
        original_ratio: SplitRatio,
    },
}

/// Drag session state for fixed slot resize handles.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum SlotDragState {
    #[default]
    Idle,
    Resizing {
        slot_id: FixedSlotId,
        pane_id: PaneId,
        original_size: f64,
    },
}

/// A divider line rect with its split path, for paint and hit-testing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SplitDividerRect {
    pub(crate) path: SplitPath,
    pub(crate) orientation: SplitOrientation,
    /// The geometric line rect (1px-wide from `split_rect`).
    pub(crate) line_rect: Rect,
    /// The full parent rect of the split node.
    pub(crate) parent_rect: Rect,
}

/// Collect all split divider line rects from a tree.
pub(crate) fn collect_split_dividers(tree: &PaneSplitTree, area: Rect) -> Vec<SplitDividerRect> {
    let mut dividers = Vec::with_capacity(tree.split_count());
    collect_dividers_from_node(
        &tree.root,
        normalized_rect(area),
        &mut Vec::new(),
        &mut dividers,
    );
    dividers
}

fn collect_dividers_from_node(
    node: &PaneSplitNode,
    area: Rect,
    path: &mut SplitPath,
    out: &mut Vec<SplitDividerRect>,
) {
    if let PaneSplitNode::Split {
        orientation,
        ratio,
        first,
        second,
    } = node
    {
        let (first_rect, second_rect) = split_rect(area, *orientation, *ratio);
        // The divider line is the boundary between first and second.
        let line_rect = match orientation {
            SplitOrientation::Horizontal => {
                Rect::new(first_rect.x1, area.y0, first_rect.x1, area.y1)
            }
            SplitOrientation::Vertical => Rect::new(area.x0, first_rect.y1, area.x1, first_rect.y1),
        };
        out.push(SplitDividerRect {
            path: path.clone(),
            orientation: *orientation,
            line_rect,
            parent_rect: area,
        });
        path.push(SplitChild::First);
        collect_dividers_from_node(first, first_rect, path, out);
        path.pop();
        path.push(SplitChild::Second);
        collect_dividers_from_node(second, second_rect, path, out);
        path.pop();
    }
}

/// Hit-test a point against all split dividers with the given slop.
/// Returns the first (shallowest) divider whose expanded rect contains the point.
pub(crate) fn hit_test_split_divider(
    tree: &PaneSplitTree,
    area: Rect,
    point: Point,
) -> Option<SplitDividerHit> {
    collect_split_dividers(tree, area)
        .into_iter()
        .find(|d| {
            let expanded = match d.orientation {
                SplitOrientation::Horizontal => Rect::new(
                    d.line_rect.x0 - DIVIDER_HIT_SLOP,
                    d.line_rect.y0,
                    d.line_rect.x1 + DIVIDER_HIT_SLOP,
                    d.line_rect.y1,
                ),
                SplitOrientation::Vertical => Rect::new(
                    d.line_rect.x0,
                    d.line_rect.y0 - DIVIDER_HIT_SLOP,
                    d.line_rect.x1,
                    d.line_rect.y1 + DIVIDER_HIT_SLOP,
                ),
            };
            expanded.contains(point)
        })
        .map(|d| SplitDividerHit {
            path: d.path,
            orientation: d.orientation,
            parent_rect: d.parent_rect,
        })
}

/// Compute a clamped split ratio from a pointer position within the parent rect.
pub(crate) fn compute_drag_ratio(
    orientation: SplitOrientation,
    parent_rect: Rect,
    point: Point,
) -> SplitRatio {
    let raw = match orientation {
        SplitOrientation::Horizontal => {
            let w = parent_rect.width();
            if w <= 0.0 {
                0.5
            } else {
                (point.x - parent_rect.x0) / w
            }
        }
        SplitOrientation::Vertical => {
            let h = parent_rect.height();
            if h <= 0.0 {
                0.5
            } else {
                (point.y - parent_rect.y0) / h
            }
        }
    };
    let clamped = raw.clamp(MIN_SPLIT_RATIO, MAX_SPLIT_RATIO);
    SplitRatio(clamped)
}

// -- Phase 20.3: fixed slot resize handle interaction --

/// Compute the resize handle rect for a fixed slot (thin strip on the inner edge).
pub(crate) fn slot_handle_rect(slot_id: FixedSlotId, slot_rect: Rect) -> Rect {
    let t = SLOT_HANDLE_HIT_SLOP;
    match slot_id {
        FixedSlotId::Left => Rect::new(
            slot_rect.x1 - t,
            slot_rect.y0,
            slot_rect.x1 + t,
            slot_rect.y1,
        ),
        FixedSlotId::Right => Rect::new(
            slot_rect.x0 - t,
            slot_rect.y0,
            slot_rect.x0 + t,
            slot_rect.y1,
        ),
        FixedSlotId::Top => Rect::new(
            slot_rect.x0,
            slot_rect.y1 - t,
            slot_rect.x1,
            slot_rect.y1 + t,
        ),
        FixedSlotId::Bottom => Rect::new(
            slot_rect.x0,
            slot_rect.y0 - t,
            slot_rect.x1,
            slot_rect.y0 + t,
        ),
    }
}

/// Hit-test a point against all fixed slot resize handles in a pane slot geometry.
/// Returns the `FixedSlotId` of the first slot whose handle contains the point.
pub(crate) fn hit_test_slot_handle(
    geometry: &PaneSlotGeometry,
    point: Point,
) -> Option<FixedSlotId> {
    for slot in &geometry.fixed_slots {
        if slot_handle_rect(slot.slot_id, slot.rect).contains(point) {
            return Some(slot.slot_id);
        }
    }
    None
}

/// Compute the new size for a fixed slot from a pointer position within the pane rect.
pub(crate) fn compute_slot_resize_size(slot_id: FixedSlotId, pane_rect: Rect, point: Point) -> f64 {
    match slot_id {
        FixedSlotId::Left => point.x - pane_rect.x0,
        FixedSlotId::Right => pane_rect.x1 - point.x,
        FixedSlotId::Top => point.y - pane_rect.y0,
        FixedSlotId::Bottom => pane_rect.y1 - point.y,
    }
}

/// Navigate to the split node at `path` and return a mutable reference.
fn split_node_at_path_mut<'a>(
    root: &'a mut PaneSplitNode,
    path: &[SplitChild],
) -> Option<&'a mut PaneSplitNode> {
    let mut current = root;
    for child in path {
        match current {
            PaneSplitNode::Split { first, second, .. } => {
                current = match child {
                    SplitChild::First => first,
                    SplitChild::Second => second,
                };
            }
            PaneSplitNode::Leaf { .. } => return None,
        }
    }
    // The node at the path must itself be a Split (it has a divider).
    match current {
        PaneSplitNode::Split { .. } => Some(current),
        PaneSplitNode::Leaf { .. } => None,
    }
}

/// Navigate to the split node at `path` (immutable).
fn split_node_at_path<'a>(
    root: &'a PaneSplitNode,
    path: &[SplitChild],
) -> Option<&'a PaneSplitNode> {
    let mut current = root;
    for child in path {
        match current {
            PaneSplitNode::Split { first, second, .. } => {
                current = match child {
                    SplitChild::First => first,
                    SplitChild::Second => second,
                };
            }
            PaneSplitNode::Leaf { .. } => return None,
        }
    }
    match current {
        PaneSplitNode::Split { .. } => Some(current),
        PaneSplitNode::Leaf { .. } => None,
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

    // -- Phase 20.3: focus navigation --

    /// Move focus to the next pane in reading order. Returns the new active pane.
    pub(crate) fn focus_next_pane(&mut self) -> PaneId {
        let next = self.pane_tree.next_pane();
        let _ = self.pane_tree.set_active_pane(next);
        next
    }

    /// Move focus to the previous pane in reading order. Returns the new active pane.
    pub(crate) fn focus_prev_pane(&mut self) -> PaneId {
        let prev = self.pane_tree.prev_pane();
        let _ = self.pane_tree.set_active_pane(prev);
        prev
    }

    /// Set focus to a specific pane. Returns error if not a member.
    pub(crate) fn set_focus_pane(&mut self, pane_id: PaneId) -> Result<(), PaneSplitTreeError> {
        self.pane_tree.set_active_pane(pane_id)
    }

    /// Transient surface anchor rect: the focused pane's geometry within the working area.
    /// Overlays, menus, and completion pop-ups anchor here, not to the full working area.
    pub(crate) fn focused_pane_rect(&self, working_area: Rect) -> Option<Rect> {
        self.pane_tree
            .pane_rect(self.active_pane_id(), working_area)
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

    // -- Phase 20.3: split divider drag interaction --

    /// Access the pane split tree (for hit-testing, divider paint).
    pub(crate) fn pane_tree(&self) -> &PaneSplitTree {
        &self.pane_tree
    }

    /// Mutable access to the pane split tree (for live drag preview).
    pub(crate) fn pane_tree_mut(&mut self) -> &mut PaneSplitTree {
        &mut self.pane_tree
    }

    /// Phase 22.1: replace the pane tree with a new topology (split/close/move
    /// commands). Bumps the layout version so observers see the change.
    pub(crate) fn replace_pane_tree(&mut self, new_tree: PaneSplitTree) {
        self.pane_tree = new_tree;
        self.version = self.version.next();
    }

    /// Commit a split divider drag: set the ratio at `path` and bump the layout version.
    /// Returns `false` if the path is invalid.
    pub(crate) fn commit_split_drag(&mut self, path: &[SplitChild], ratio: SplitRatio) -> bool {
        if self.pane_tree.update_split_ratio(path, ratio) {
            self.version = self.version.next();
            true
        } else {
            false
        }
    }

    /// Cancel a split divider drag: restore the original ratio without bumping the version.
    pub(crate) fn cancel_split_drag(&mut self, path: &[SplitChild], original_ratio: SplitRatio) {
        self.pane_tree.update_split_ratio(path, original_ratio);
    }

    // -- Phase 20.3: fixed slot resize/collapse interaction --

    /// Mutable access to the slot layout for a pane.
    pub(crate) fn slot_layout_mut(&mut self, pane_id: PaneId) -> Option<&mut PaneSlotLayout> {
        self.pane_slots.get_mut(&pane_id)
    }

    /// Iterate over all pane slot layouts (for persistence).
    pub(crate) fn pane_slots_iter(&self) -> impl Iterator<Item = (PaneId, &PaneSlotLayout)> {
        self.pane_slots.iter().map(|(&k, v)| (k, v))
    }

    /// Resize a fixed slot (live preview, no version bump). Clamps to min/max.
    pub(crate) fn resize_slot_live(
        &mut self,
        pane_id: PaneId,
        slot_id: FixedSlotId,
        new_size: f64,
    ) {
        if let Some(slot) = self
            .pane_slots
            .get_mut(&pane_id)
            .and_then(|l| l.fixed_slot_mut(slot_id))
        {
            slot.size = new_size.clamp(slot.min_size, slot.max_size);
        }
    }

    /// Commit a slot resize: set `resized_by_user` and bump the layout version.
    pub(crate) fn commit_slot_resize(&mut self, pane_id: PaneId, slot_id: FixedSlotId) {
        if let Some(slot) = self
            .pane_slots
            .get_mut(&pane_id)
            .and_then(|l| l.fixed_slot_mut(slot_id))
        {
            slot.resized_by_user = true;
        }
        self.version = self.version.next();
    }

    /// Cancel a slot resize: restore the original size without bumping the version.
    pub(crate) fn cancel_slot_resize(
        &mut self,
        pane_id: PaneId,
        slot_id: FixedSlotId,
        original_size: f64,
    ) {
        if let Some(slot) = self
            .pane_slots
            .get_mut(&pane_id)
            .and_then(|l| l.fixed_slot_mut(slot_id))
        {
            slot.size = original_size;
        }
    }

    /// Toggle collapse on a fixed slot and bump the layout version.
    pub(crate) fn toggle_slot_collapse(&mut self, pane_id: PaneId, slot_id: FixedSlotId) {
        if let Some(slot) = self
            .pane_slots
            .get_mut(&pane_id)
            .and_then(|l| l.fixed_slot_mut(slot_id))
        {
            slot.toggle_collapse();
        }
        self.version = self.version.next();
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

/// Replace the leaf for `target_pane` with a split node containing the original
/// leaf and a new leaf for `new_pane_id`. Returns `None` if the target is not found.
fn split_node_pane(
    node: &PaneSplitNode,
    target_pane: PaneId,
    new_pane_id: PaneId,
    orientation: SplitOrientation,
    ratio: SplitRatio,
    position: SplitChild,
) -> Option<PaneSplitNode> {
    match node {
        PaneSplitNode::Leaf { pane_id } => {
            if *pane_id != target_pane {
                return None;
            }
            let original = PaneSplitNode::leaf(target_pane);
            let new_leaf = PaneSplitNode::leaf(new_pane_id);
            let (first, second) = match position {
                SplitChild::First => (new_leaf, original),
                SplitChild::Second => (original, new_leaf),
            };
            Some(PaneSplitNode::split(orientation, ratio, first, second))
        }
        PaneSplitNode::Split {
            orientation: o,
            ratio: r,
            first,
            second,
        } => {
            let new_first = split_node_pane(
                first,
                target_pane,
                new_pane_id,
                orientation,
                ratio,
                position,
            );
            let new_second = split_node_pane(
                second,
                target_pane,
                new_pane_id,
                orientation,
                ratio,
                position,
            );
            match (new_first, new_second) {
                (Some(f), None) => Some(PaneSplitNode::split(*o, *r, f, *second.clone())),
                (None, Some(s)) => Some(PaneSplitNode::split(*o, *r, *first.clone(), s)),
                _ => None,
            }
        }
    }
}

/// Phase 22.1: Remove the leaf for `pane_id`, promoting its sibling subtree.
///
/// Returns the rebuilt root and the first leaf pane ID of the promoted sibling
/// subtree (focus handoff when the closed pane was active).
fn close_node_pane(node: &PaneSplitNode, pane_id: PaneId) -> Option<(PaneSplitNode, PaneId)> {
    let PaneSplitNode::Split {
        orientation,
        ratio,
        first,
        second,
    } = node
    else {
        return None;
    };
    if let PaneSplitNode::Leaf { pane_id: child } = first.as_ref()
        && *child == pane_id
    {
        return Some((*second.clone(), first_leaf_pane_id(second)));
    }
    if let PaneSplitNode::Leaf { pane_id: child } = second.as_ref()
        && *child == pane_id
    {
        return Some((*first.clone(), first_leaf_pane_id(first)));
    }
    if let Some((rebuilt, handoff)) = close_node_pane(first, pane_id) {
        return Some((
            PaneSplitNode::split(*orientation, *ratio, rebuilt, *second.clone()),
            handoff,
        ));
    }
    let (rebuilt, handoff) = close_node_pane(second, pane_id)?;
    Some((
        PaneSplitNode::split(*orientation, *ratio, *first.clone(), rebuilt),
        handoff,
    ))
}

/// First (reading-order) leaf pane ID under `node`.
fn first_leaf_pane_id(node: &PaneSplitNode) -> PaneId {
    match node {
        PaneSplitNode::Leaf { pane_id } => *pane_id,
        PaneSplitNode::Split { first, .. } => first_leaf_pane_id(first),
    }
}

/// Phase 22.1: Right-leaning comb giving every leaf equal area along `orientation`.
///
/// Ratios are `1/(N+1), 1/N, ..., 1/2` from root toward the tail, so each leaf
/// receives exactly `1/(N+1)` of the parent area (within f64 tolerance).
fn equal_comb_tree(leaves: &[PaneId], orientation: SplitOrientation) -> PaneSplitNode {
    debug_assert!(leaves.len() >= 2, "equal comb needs at least two leaves");
    let ratio = SplitRatio::new(1.0 / leaves.len() as f64)
        .expect("equal comb ratios stay in bounds for capped pane counts");
    let first = PaneSplitNode::leaf(leaves[0]);
    let second = if leaves.len() == 2 {
        PaneSplitNode::leaf(leaves[1])
    } else {
        equal_comb_tree(&leaves[1..], orientation)
    };
    PaneSplitNode::split(orientation, ratio, first, second)
}

/// Phase 22.1: Swap two leaf pane IDs within the tree shape (shape/ratios unchanged).
fn swap_leaf_pane_ids(node: &PaneSplitNode, a: PaneId, b: PaneId) -> PaneSplitNode {
    match node {
        PaneSplitNode::Leaf { pane_id } => PaneSplitNode::leaf(if *pane_id == a {
            b
        } else if *pane_id == b {
            a
        } else {
            *pane_id
        }),
        PaneSplitNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => PaneSplitNode::split(
            *orientation,
            *ratio,
            swap_leaf_pane_ids(first, a, b),
            swap_leaf_pane_ids(second, a, b),
        ),
    }
}

/// Child-step path from `node` to the leaf for `pane_id` (empty when `node` is the leaf).
fn leaf_path_in_node(node: &PaneSplitNode, pane_id: PaneId) -> Option<SplitPath> {
    match node {
        PaneSplitNode::Leaf { pane_id: id } => (*id == pane_id).then(Vec::new),
        PaneSplitNode::Split { first, second, .. } => {
            if let Some(mut path) = leaf_path_in_node(first, pane_id) {
                path.insert(0, SplitChild::First);
                return Some(path);
            }
            let mut path = leaf_path_in_node(second, pane_id)?;
            path.insert(0, SplitChild::Second);
            Some(path)
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

    // -- Phase 20.3: split divider drag interaction tests --

    fn two_pane_horizontal_tree() -> PaneSplitTree {
        PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap()
    }

    fn nested_tree() -> PaneSplitTree {
        // Horizontal split at root, vertical split in second child.
        PaneSplitTree::new(
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
        .unwrap()
    }

    #[test]
    fn split_divider_hit_test_identifies_correct_split() {
        let tree = two_pane_horizontal_tree();
        let area = Rect::new(0.0, 0.0, 1000.0, 800.0);
        // Divider at x=500 (ratio 0.5). Point on the line.
        let hit = hit_test_split_divider(&tree, area, Point::new(500.0, 400.0));
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.path, vec![]);
        assert_eq!(hit.orientation, SplitOrientation::Horizontal);
        assert_rect_eq(hit.parent_rect, area);
    }

    #[test]
    fn split_divider_hit_test_misses_outside_thickness() {
        let tree = two_pane_horizontal_tree();
        let area = Rect::new(0.0, 0.0, 1000.0, 800.0);
        // 20px away from divider at x=500 — well outside 4px slop.
        assert!(hit_test_split_divider(&tree, area, Point::new(520.0, 400.0)).is_none());
        assert!(hit_test_split_divider(&tree, area, Point::new(480.0, 400.0)).is_none());
    }

    #[test]
    fn split_divider_hit_test_nested_tree_finds_inner_split() {
        let tree = nested_tree();
        let area = Rect::new(0.0, 0.0, 1000.0, 800.0);
        // Root horizontal divider at x=250.
        let hit = hit_test_split_divider(&tree, area, Point::new(250.0, 400.0)).unwrap();
        assert_eq!(hit.path, vec![]);
        assert_eq!(hit.orientation, SplitOrientation::Horizontal);
        // Inner vertical divider: second child area is (250,0)-(1000,800),
        // vertical split at ratio 0.75 → y=600.
        let hit = hit_test_split_divider(&tree, area, Point::new(600.0, 600.0)).unwrap();
        assert_eq!(hit.path, vec![SplitChild::Second]);
        assert_eq!(hit.orientation, SplitOrientation::Vertical);
    }

    #[test]
    fn split_drag_ratio_clamps_to_bounds() {
        let area = Rect::new(0.0, 0.0, 1000.0, 800.0);
        // Drag far left → clamps to MIN.
        let ratio = compute_drag_ratio(
            SplitOrientation::Horizontal,
            area,
            Point::new(-100.0, 400.0),
        );
        assert!((ratio.value() - MIN_SPLIT_RATIO).abs() < 1e-9);
        // Drag far right → clamps to MAX.
        let ratio = compute_drag_ratio(
            SplitOrientation::Horizontal,
            area,
            Point::new(2000.0, 400.0),
        );
        assert!((ratio.value() - MAX_SPLIT_RATIO).abs() < 1e-9);
        // Normal position.
        let ratio =
            compute_drag_ratio(SplitOrientation::Horizontal, area, Point::new(300.0, 400.0));
        assert!((ratio.value() - 0.3).abs() < 1e-9);
        // Vertical.
        let ratio = compute_drag_ratio(SplitOrientation::Vertical, area, Point::new(500.0, 200.0));
        assert!((ratio.value() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn split_drag_commit_bumps_layout_version() {
        let mut layout = WorkingAreaLayout::single_editor();
        let tree = two_pane_horizontal_tree();
        layout
            .apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree,
                editor_pane_id: PaneId(1),
                pane_slots: Vec::new(),
            })
            .unwrap();
        assert_eq!(layout.version(), ShellLayoutVersion(2));

        let new_ratio = SplitRatio::new(0.7).unwrap();
        assert!(layout.commit_split_drag(&[], new_ratio));
        assert_eq!(layout.version(), ShellLayoutVersion(3));
        assert_eq!(layout.pane_tree().split_ratio_at_path(&[]), Some(new_ratio));
    }

    #[test]
    fn split_drag_cancel_restores_original_ratio() {
        let mut layout = WorkingAreaLayout::single_editor();
        let tree = two_pane_horizontal_tree();
        layout
            .apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree,
                editor_pane_id: PaneId(1),
                pane_slots: Vec::new(),
            })
            .unwrap();

        let original = SplitRatio::balanced();
        // Simulate drag: update ratio without version bump.
        layout
            .pane_tree
            .update_split_ratio(&[], SplitRatio::new(0.8).unwrap());
        assert_eq!(
            layout.pane_tree().split_ratio_at_path(&[]),
            Some(SplitRatio::new(0.8).unwrap())
        );
        // Cancel: restore original.
        layout.cancel_split_drag(&[], original);
        assert_eq!(layout.pane_tree().split_ratio_at_path(&[]), Some(original));
        // Version unchanged (still 2 from apply_update).
        assert_eq!(layout.version(), ShellLayoutVersion(2));
    }

    #[test]
    fn split_divider_hit_test_single_leaf_returns_none() {
        let tree = PaneSplitTree::default();
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);
        assert!(hit_test_split_divider(&tree, area, Point::new(400.0, 300.0)).is_none());
    }

    #[test]
    fn update_split_ratio_invalid_path_returns_false() {
        let mut tree = two_pane_horizontal_tree();
        // Path to a non-existent child.
        assert!(!tree.update_split_ratio(&[SplitChild::First], SplitRatio::balanced()));
        assert!(!tree.update_split_ratio(
            &[SplitChild::Second, SplitChild::First],
            SplitRatio::balanced()
        ));
    }

    #[test]
    fn collect_split_dividers_returns_correct_count() {
        let tree = nested_tree();
        let area = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let dividers = collect_split_dividers(&tree, area);
        assert_eq!(dividers.len(), 2);
        assert_eq!(dividers[0].path, vec![]);
        assert_eq!(dividers[0].orientation, SplitOrientation::Horizontal);
        assert_eq!(dividers[1].path, vec![SplitChild::Second]);
        assert_eq!(dividers[1].orientation, SplitOrientation::Vertical);
    }

    // -- Phase 20.3: fixed slot resize/collapse tests --

    fn slot_layout_with_left() -> PaneSlotLayout {
        PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap())
    }

    #[test]
    fn slot_handle_hit_test_identifies_correct_slot() {
        let layout = slot_layout_with_left();
        let pane_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        let geometry = layout.compute_geometry(pane_rect);
        // Left slot rect is (0,0)-(200,600). Handle at x=200 ± 4.
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(200.0, 300.0)),
            Some(FixedSlotId::Left)
        );
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(203.0, 300.0)),
            Some(FixedSlotId::Left)
        );
        // Miss: too far from handle.
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(220.0, 300.0)),
            None
        );
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(100.0, 300.0)),
            None
        );
    }

    #[test]
    fn slot_handle_hit_test_all_four_slots() {
        let layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 100.0, 50.0, 200.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Right, 100.0, 50.0, 200.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Top, 80.0, 40.0, 160.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Bottom, 60.0, 30.0, 120.0).unwrap());
        let pane_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        let geometry = layout.compute_geometry(pane_rect);
        // Left handle at x=100, Right handle at x=700, Top handle at y=80, Bottom handle at y=540.
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(100.0, 300.0)),
            Some(FixedSlotId::Left)
        );
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(700.0, 300.0)),
            Some(FixedSlotId::Right)
        );
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(400.0, 80.0)),
            Some(FixedSlotId::Top)
        );
        assert_eq!(
            hit_test_slot_handle(&geometry, Point::new(400.0, 540.0)),
            Some(FixedSlotId::Bottom)
        );
    }

    #[test]
    fn slot_resize_clamps_to_min_max() {
        let mut slot = FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap();
        slot.resize_to(50.0);
        assert_eq!(slot.size, 100.0); // clamped to min
        slot.resize_to(500.0);
        assert_eq!(slot.size, 400.0); // clamped to max
        slot.resize_to(300.0);
        assert_eq!(slot.size, 300.0); // within bounds
    }

    #[test]
    fn slot_resize_sets_resized_by_user() {
        let mut slot = FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap();
        assert!(!slot.resized_by_user);
        slot.resize_to(250.0);
        assert!(slot.resized_by_user);
    }

    #[test]
    fn slot_collapse_restore_toggles_effective_size() {
        let mut slot = FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap();
        assert_eq!(slot.effective_size(800.0), 200.0);
        slot.toggle_collapse();
        assert!(slot.collapsed);
        assert_eq!(slot.effective_size(800.0), 0.0);
        slot.toggle_collapse();
        assert!(!slot.collapsed);
        assert_eq!(slot.effective_size(800.0), 200.0);
    }

    #[test]
    fn slot_collapse_restore_round_trip() {
        let mut slot = FixedSlotState::new(FixedSlotId::Bottom, 80.0, 40.0, 160.0).unwrap();
        let original_size = slot.size;
        slot.toggle_collapse();
        slot.toggle_collapse();
        assert_eq!(slot.size, original_size);
        assert!(!slot.collapsed);
    }

    #[test]
    fn compute_slot_resize_size_maps_pointer_correctly() {
        let pane_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        // Left: distance from left edge.
        assert!(
            (compute_slot_resize_size(FixedSlotId::Left, pane_rect, Point::new(250.0, 300.0))
                - 250.0)
                .abs()
                < 1e-9
        );
        // Right: distance from right edge.
        assert!(
            (compute_slot_resize_size(FixedSlotId::Right, pane_rect, Point::new(600.0, 300.0))
                - 200.0)
                .abs()
                < 1e-9
        );
        // Top: distance from top edge.
        assert!(
            (compute_slot_resize_size(FixedSlotId::Top, pane_rect, Point::new(400.0, 120.0))
                - 120.0)
                .abs()
                < 1e-9
        );
        // Bottom: distance from bottom edge.
        assert!(
            (compute_slot_resize_size(FixedSlotId::Bottom, pane_rect, Point::new(400.0, 500.0))
                - 100.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn working_area_slot_resize_commit_bumps_version() {
        let mut layout = WorkingAreaLayout::single_editor()
            .with_editor_pane_slot_layout(slot_layout_with_left());
        let pane_id = layout.active_pane_id();
        let v_before = layout.version();

        layout.resize_slot_live(pane_id, FixedSlotId::Left, 300.0);
        assert_eq!(layout.version(), v_before); // no bump during live

        layout.commit_slot_resize(pane_id, FixedSlotId::Left);
        assert_eq!(layout.version(), v_before.next());

        // Verify the slot was resized and marked.
        let slot = layout
            .slot_layout_mut(pane_id)
            .unwrap()
            .fixed_slot_mut(FixedSlotId::Left)
            .unwrap();
        assert_eq!(slot.size, 300.0);
        assert!(slot.resized_by_user);
    }

    #[test]
    fn working_area_slot_resize_cancel_restores_size() {
        let mut layout = WorkingAreaLayout::single_editor()
            .with_editor_pane_slot_layout(slot_layout_with_left());
        let pane_id = layout.active_pane_id();
        let v_before = layout.version();

        layout.resize_slot_live(pane_id, FixedSlotId::Left, 350.0);
        layout.cancel_slot_resize(pane_id, FixedSlotId::Left, 200.0);
        assert_eq!(layout.version(), v_before); // no bump on cancel

        let slot = layout
            .slot_layout_mut(pane_id)
            .unwrap()
            .fixed_slot_mut(FixedSlotId::Left)
            .unwrap();
        assert_eq!(slot.size, 200.0);
    }

    #[test]
    fn working_area_toggle_slot_collapse_bumps_version() {
        let mut layout = WorkingAreaLayout::single_editor()
            .with_editor_pane_slot_layout(slot_layout_with_left());
        let pane_id = layout.active_pane_id();
        let v_before = layout.version();

        layout.toggle_slot_collapse(pane_id, FixedSlotId::Left);
        assert_eq!(layout.version(), v_before.next());

        let slot = layout
            .slot_layout_mut(pane_id)
            .unwrap()
            .fixed_slot_mut(FixedSlotId::Left)
            .unwrap();
        assert!(slot.collapsed);

        layout.toggle_slot_collapse(pane_id, FixedSlotId::Left);
        let slot = layout
            .slot_layout_mut(pane_id)
            .unwrap()
            .fixed_slot_mut(FixedSlotId::Left)
            .unwrap();
        assert!(!slot.collapsed);
    }

    // -- Phase 20.3: layout intent composition tests --

    #[test]
    fn split_pane_replaces_leaf_with_split() {
        let tree = PaneSplitTree::default(); // single leaf PaneId(1)
        let new_tree = tree
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap();
        assert_eq!(new_tree.pane_count(), 2);
        assert_eq!(new_tree.split_count(), 1);
        assert!(new_tree.contains_pane(PaneId(1)));
        assert!(new_tree.contains_pane(PaneId(2)));
        assert_eq!(new_tree.active_pane_id(), PaneId(1));
    }

    #[test]
    fn split_pane_position_first_puts_new_pane_first() {
        let tree = PaneSplitTree::default();
        let new_tree = tree
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Vertical,
                SplitRatio::new(0.3).unwrap(),
                SplitChild::First,
            )
            .unwrap();
        // Geometry: new pane (2) is first (top), original (1) is second (bottom).
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);
        let geom = new_tree.compute_geometry(area);
        let pane2 = geom.iter().find(|g| g.pane_id == PaneId(2)).unwrap();
        let pane1 = geom.iter().find(|g| g.pane_id == PaneId(1)).unwrap();
        assert!(pane2.rect.y0 < pane1.rect.y0); // pane2 is above pane1
    }

    #[test]
    fn split_pane_rejects_duplicate_new_pane_id() {
        let tree = two_pane_horizontal_tree();
        assert!(
            tree.split_pane(
                PaneId(1),
                PaneId(2), // already exists
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .is_none()
        );
    }

    #[test]
    fn split_pane_rejects_missing_target_pane() {
        let tree = PaneSplitTree::default();
        assert!(
            tree.split_pane(
                PaneId(99), // doesn't exist
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .is_none()
        );
    }

    #[test]
    fn split_pane_nested_tree_targets_correct_leaf() {
        let tree = nested_tree(); // panes 1, 2, 3
        let new_tree = tree
            .split_pane(
                PaneId(3),
                PaneId(4),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap();
        assert_eq!(new_tree.pane_count(), 4);
        assert_eq!(new_tree.split_count(), 3);
        assert!(new_tree.contains_pane(PaneId(4)));
    }

    #[test]
    fn next_pane_id_returns_max_plus_one() {
        let tree = nested_tree(); // panes 1, 2, 3
        assert_eq!(tree.next_pane_id(), PaneId(4));
        let single = PaneSplitTree::default(); // pane 1
        assert_eq!(single.next_pane_id(), PaneId(2));
    }

    // -- Phase 20.3: focus traversal tests --

    #[test]
    fn focus_next_pane_cycles_through_splits() {
        let tree = nested_tree(); // panes 1, 2, 3 in reading order; active = 2
        assert_eq!(tree.active_pane_id(), PaneId(2));
        assert_eq!(tree.next_pane(), PaneId(3));
        assert_eq!(tree.prev_pane(), PaneId(1));

        // Mutate active and check wrapping.
        let mut tree2 = nested_tree();
        tree2.set_active_pane(PaneId(3)).unwrap();
        assert_eq!(tree2.next_pane(), PaneId(1)); // wraps from 3 to 1
        assert_eq!(tree2.prev_pane(), PaneId(2));

        // Wrap the other direction.
        let mut tree3 = nested_tree();
        tree3.set_active_pane(PaneId(1)).unwrap();
        assert_eq!(tree3.prev_pane(), PaneId(3)); // wraps from 1 to 3
        assert_eq!(tree3.next_pane(), PaneId(2));
    }

    #[test]
    fn focus_single_pane_returns_self() {
        let tree = PaneSplitTree::default(); // single leaf PaneId(1)
        assert_eq!(tree.next_pane(), PaneId(1));
        assert_eq!(tree.prev_pane(), PaneId(1));
    }

    #[test]
    fn focus_set_active_pane_validates_membership() {
        let mut tree = two_pane_horizontal_tree();
        assert!(tree.set_active_pane(PaneId(2)).is_ok());
        assert_eq!(tree.active_pane_id(), PaneId(2));
        assert!(tree.set_active_pane(PaneId(99)).is_err());
        assert_eq!(tree.active_pane_id(), PaneId(2)); // unchanged
    }

    #[test]
    fn transient_anchor_uses_focused_pane_geometry() {
        let tree = two_pane_horizontal_tree();
        let layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(1)).unwrap();
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);

        // Focused pane (1) is the left half.
        let anchor = layout.focused_pane_rect(area).unwrap();
        assert!(anchor.width() < 800.0); // not the full working area
        assert!(anchor.x0 == 0.0); // left half

        // Full working area is wider.
        assert!(area.width() > anchor.width());
    }

    #[test]
    fn working_area_focus_next_prev_updates_active_pane() {
        let tree = two_pane_horizontal_tree();
        let mut layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(1)).unwrap();
        assert_eq!(layout.active_pane_id(), PaneId(1));

        let next = layout.focus_next_pane();
        assert_eq!(next, PaneId(2));
        assert_eq!(layout.active_pane_id(), PaneId(2));

        let prev = layout.focus_prev_pane();
        assert_eq!(prev, PaneId(1));
        assert_eq!(layout.active_pane_id(), PaneId(1));
    }

    #[test]
    fn input_routing_scoped_to_focused_pane() {
        // PackageInputRouting.scope is a string; Clay checks it against the
        // active pane ID. This test verifies the scope-matching logic.
        let route = crate::shell::package_ui::PackageInputRouting::new(
            "test.route",
            "pane:1", // scoped to pane 1
            "test.component",
            "activate",
            None,
            "none",
            "focus-on-click",
            "single",
            vec![],
            vec![],
        );
        let tree = two_pane_horizontal_tree();
        let mut layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(1)).unwrap();

        // Route scoped to pane:1 is active when pane 1 is focused.
        let scope_pane = route.scope.strip_prefix("pane:").unwrap();
        assert_eq!(scope_pane, layout.active_pane_id().0.to_string());

        // After focus moves to pane 2, the route is no longer active.
        layout.focus_next_pane();
        assert_ne!(scope_pane, layout.active_pane_id().0.to_string());
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

    // -- Phase 20.3: structural layout invariant tests --

    fn four_pane_grid_tree() -> PaneSplitTree {
        // 2x2 grid: horizontal root, vertical children.
        PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::split(
                    SplitOrientation::Vertical,
                    SplitRatio::balanced(),
                    PaneSplitNode::leaf(PaneId(1)),
                    PaneSplitNode::leaf(PaneId(2)),
                ),
                PaneSplitNode::split(
                    SplitOrientation::Vertical,
                    SplitRatio::balanced(),
                    PaneSplitNode::leaf(PaneId(3)),
                    PaneSplitNode::leaf(PaneId(4)),
                ),
            ),
            PaneId(1),
        )
        .unwrap()
    }

    #[test]
    fn geometry_invariants_non_overlapping_panes() {
        for tree in [
            two_pane_horizontal_tree(),
            nested_tree(),
            four_pane_grid_tree(),
        ] {
            let geo = tree.compute_geometry(Rect::new(0.0, 0.0, 1920.0, 1080.0));
            for i in 0..geo.len() {
                for j in (i + 1)..geo.len() {
                    let overlap = geo[i].rect.intersect(geo[j].rect);
                    assert!(
                        overlap.area() <= 1.0,
                        "panes {:?} and {:?} overlap by {} px²",
                        geo[i].pane_id,
                        geo[j].pane_id,
                        overlap.area()
                    );
                }
            }
        }
    }

    #[test]
    fn geometry_invariants_panes_sum_to_working_area() {
        let area = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        for tree in [
            two_pane_horizontal_tree(),
            nested_tree(),
            four_pane_grid_tree(),
        ] {
            let geo = tree.compute_geometry(area);
            let total: f64 = geo.iter().map(|g| g.rect.area()).sum();
            assert!(
                (total - area.area()).abs() < 1.0,
                "pane areas sum to {total}, expected {}",
                area.area()
            );
        }
    }

    #[test]
    fn geometry_invariants_non_negative_rects() {
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);
        for tree in [
            two_pane_horizontal_tree(),
            nested_tree(),
            four_pane_grid_tree(),
        ] {
            let geo = tree.compute_geometry(area);
            for g in &geo {
                assert!(
                    g.rect.width() >= 0.0,
                    "pane {:?} has negative width",
                    g.pane_id
                );
                assert!(
                    g.rect.height() >= 0.0,
                    "pane {:?} has negative height",
                    g.pane_id
                );
            }
        }
    }

    #[test]
    fn resize_clamping_split_ratio_sequence() {
        let mut tree = two_pane_horizontal_tree();
        let path: Vec<SplitChild> = vec![]; // root split
        // SplitRatio::new rejects out-of-bounds; compute_drag_ratio clamps.
        // Verify the tree always holds a valid ratio after updates.
        for ratio in [0.05, 0.1, 0.5, 0.9, 0.95] {
            let r = SplitRatio::new(ratio).unwrap();
            tree.update_split_ratio(&path, r);
            let actual = tree.split_ratio_at_path(&path).unwrap();
            assert!(
                (0.05..=0.95).contains(&actual.value()),
                "ratio {} out of bounds after update to {}",
                actual.value(),
                ratio
            );
        }
        // Out-of-bounds values are rejected by SplitRatio::new.
        assert!(SplitRatio::new(0.0).is_err());
        assert!(SplitRatio::new(0.04).is_err());
        assert!(SplitRatio::new(0.96).is_err());
        assert!(SplitRatio::new(1.0).is_err());
        assert!(SplitRatio::new(-0.1).is_err());
    }

    #[test]
    fn resize_clamping_slot_size_sequence() {
        let mut slot = FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap();
        // Resize to extreme values; must clamp to [min, max].
        for size in [0.0, -10.0, 1.0, 50.0, 200.0, 500.0, 1000.0, f64::MAX] {
            slot.resize_to(size);
            assert!(
                slot.size >= slot.min_size && slot.size <= slot.max_size,
                "slot size {} out of [{}, {}] after resize to {}",
                slot.size,
                slot.min_size,
                slot.max_size,
                size
            );
        }
    }

    #[test]
    fn collapse_restore_effective_size_zero_and_back() {
        let mut slot = FixedSlotState::new(FixedSlotId::Right, 250.0, 100.0, 500.0).unwrap();
        let original = slot.size;
        let available = 800.0;
        slot.toggle_collapse();
        assert!(slot.collapsed);
        assert_eq!(slot.effective_size(available), 0.0);
        slot.toggle_collapse();
        assert!(!slot.collapsed);
        assert!((slot.effective_size(available) - original).abs() < f64::EPSILON);
    }

    #[test]
    fn no_layout_mutation_during_compute_geometry() {
        let tree = nested_tree();
        let before = tree.clone();
        let area = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let _ = tree.compute_geometry(area);
        let _ = tree.pane_rect(PaneId(1), area);
        let _ = tree.divider_rects(area);
        assert_eq!(tree, before, "compute_geometry must not mutate tree state");

        let layout = WorkingAreaLayout::with_pane_tree(nested_tree(), PaneId(2)).unwrap();
        let before_layout = layout.clone();
        let _ = layout.editor_component_rect(area);
        let _ = layout.focused_pane_rect(area);
        let _ = layout.pane_slot_geometry(PaneId(2), area);
        assert_eq!(
            layout, before_layout,
            "geometry reads must not mutate layout"
        );
    }

    #[test]
    fn versioned_update_rejection_matrix() {
        let layout = WorkingAreaLayout::single_editor();
        let area_id = layout.working_area_id();
        let version = layout.version();
        let pane_id = layout.active_pane_id();

        // Stale version.
        let stale = WorkingAreaLayoutUpdate {
            base_version: ShellLayoutVersion(version.0.wrapping_add(1)),
            working_area_id: area_id,
            pane_tree: PaneSplitTree::single_leaf(pane_id),
            editor_pane_id: pane_id,
            pane_slots: vec![PaneSlotLayoutAssignment {
                pane_id,
                layout: PaneSlotLayout::main_only(),
            }],
        };
        assert!(matches!(
            layout.clone().apply_update(stale),
            Err(WorkingAreaLayoutUpdateError::StaleVersion { .. })
        ));

        // Wrong working area.
        let wrong_area = WorkingAreaLayoutUpdate {
            base_version: version,
            working_area_id: WorkingAreaId(999),
            pane_tree: PaneSplitTree::single_leaf(pane_id),
            editor_pane_id: pane_id,
            pane_slots: vec![PaneSlotLayoutAssignment {
                pane_id,
                layout: PaneSlotLayout::main_only(),
            }],
        };
        assert!(matches!(
            layout.clone().apply_update(wrong_area),
            Err(WorkingAreaLayoutUpdateError::WrongWorkingArea { .. })
        ));

        // Missing editor pane.
        let missing_editor = WorkingAreaLayoutUpdate {
            base_version: version,
            working_area_id: area_id,
            pane_tree: PaneSplitTree::single_leaf(pane_id),
            editor_pane_id: PaneId(999),
            pane_slots: vec![PaneSlotLayoutAssignment {
                pane_id,
                layout: PaneSlotLayout::main_only(),
            }],
        };
        assert!(matches!(
            layout.clone().apply_update(missing_editor),
            Err(WorkingAreaLayoutUpdateError::EditorPaneMissing(_))
        ));

        // Duplicate slot pane (2-pane tree, 2 slots both for pane 1).
        let two_pane = two_pane_horizontal_tree();
        let dup_slot = WorkingAreaLayoutUpdate {
            base_version: version,
            working_area_id: area_id,
            pane_tree: two_pane.clone(),
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
        };
        assert!(matches!(
            layout.clone().apply_update(dup_slot),
            Err(WorkingAreaLayoutUpdateError::DuplicatePaneSlot(_))
        ));

        // Slot pane not in tree.
        let missing_slot_pane = WorkingAreaLayoutUpdate {
            base_version: version,
            working_area_id: area_id,
            pane_tree: PaneSplitTree::single_leaf(pane_id),
            editor_pane_id: pane_id,
            pane_slots: vec![PaneSlotLayoutAssignment {
                pane_id: PaneId(999),
                layout: PaneSlotLayout::main_only(),
            }],
        };
        assert!(matches!(
            layout.clone().apply_update(missing_slot_pane),
            Err(WorkingAreaLayoutUpdateError::SlotPaneMissing(_))
        ));
    }

    // -- Phase 22.1: pane lifecycle operations --

    fn three_pane_tree() -> PaneSplitTree {
        PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap()
            .split_pane(
                PaneId(2),
                PaneId(3),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap()
    }

    fn four_pane_tree() -> PaneSplitTree {
        three_pane_tree()
            .split_pane(
                PaneId(3),
                PaneId(4),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .expect("fourth pane fits under the cap")
    }

    fn assert_areas_equal(geometry: &[PaneGeometry]) {
        let expected = geometry[0].rect.area();
        for pane in geometry {
            assert!(
                (pane.rect.area() - expected).abs() < 1.0,
                "pane {:?} area {} deviates from {}",
                pane.pane_id,
                pane.rect.area(),
                expected
            );
        }
    }

    #[test]
    fn split_pane_rejects_fifth_pane_at_cap() {
        let tree = four_pane_tree();
        assert_eq!(tree.pane_count(), MAX_PANES_PER_TAB);
        assert!(
            tree.split_pane(
                PaneId(4),
                PaneId(5),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .is_none()
        );
    }

    #[test]
    fn add_equal_pane_from_single_leaf_creates_two_equal_side_by_side_panes() {
        let tree = PaneSplitTree::single_leaf(PaneId(1));

        let updated = tree.add_equal_pane().expect("single leaf can add a pane");

        assert_eq!(updated.pane_ids(), vec![PaneId(1), PaneId(2)]);
        assert_eq!(updated.active_pane_id(), PaneId(1));
        assert_eq!(updated.next_pane_id(), PaneId(3));
        match updated.root_node() {
            PaneSplitNode::Split { orientation, .. } => {
                assert_eq!(*orientation, SplitOrientation::Horizontal)
            }
            _ => panic!("expected root split"),
        }
        let geometry = updated.compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0));
        assert_rect_eq(geometry[0].rect, Rect::new(0.0, 0.0, 450.0, 600.0));
        assert_rect_eq(geometry[1].rect, Rect::new(450.0, 0.0, 900.0, 600.0));
    }

    #[test]
    fn add_equal_pane_redivides_two_panes_into_three_equal_areas() {
        let tree = PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::new(0.3).unwrap(),
                SplitChild::Second,
            )
            .unwrap();

        let updated = tree.add_equal_pane().unwrap();

        assert_eq!(updated.pane_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
        match updated.root_node() {
            PaneSplitNode::Split { orientation, .. } => {
                assert_eq!(*orientation, SplitOrientation::Horizontal)
            }
            _ => panic!("expected root split"),
        }
        assert_areas_equal(&updated.compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0)));
    }

    #[test]
    fn add_equal_pane_from_vertical_root_keeps_vertical_orientation() {
        let tree = PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Vertical,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap();

        let updated = tree.add_equal_pane().unwrap();
        let geometry = updated.compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0));

        assert_eq!(updated.pane_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
        assert_areas_equal(&geometry);
        assert_rect_eq(geometry[0].rect, Rect::new(0.0, 0.0, 900.0, 200.0));
    }

    #[test]
    fn add_equal_pane_four_panes_have_equal_areas() {
        let updated = three_pane_tree().add_equal_pane().unwrap();

        assert_eq!(updated.pane_count(), MAX_PANES_PER_TAB);
        assert_areas_equal(&updated.compute_geometry(Rect::new(0.0, 0.0, 800.0, 600.0)));
    }

    #[test]
    fn add_equal_pane_at_cap_returns_none() {
        assert!(four_pane_tree().add_equal_pane().is_none());
    }

    #[test]
    fn close_pane_merges_two_panes_and_hands_off_focus() {
        let mut tree = PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap();
        tree.set_active_pane(PaneId(2)).unwrap();

        // Closing the active pane hands focus to the surviving leaf.
        let closed_active = tree.close_pane(PaneId(2)).unwrap();
        assert_eq!(closed_active.pane_ids(), vec![PaneId(1)]);
        assert_eq!(closed_active.active_pane_id(), PaneId(1));

        // Closing a non-active pane preserves focus.
        let closed_inactive = tree.close_pane(PaneId(1)).unwrap();
        assert_eq!(closed_inactive.pane_ids(), vec![PaneId(2)]);
        assert_eq!(closed_inactive.active_pane_id(), PaneId(2));
    }

    #[test]
    fn close_pane_on_comb_preserves_reading_order_and_fills_area() {
        let tree = four_pane_tree(); // comb [1 | [2 | [3 | 4]]]

        let closed = tree.close_pane(PaneId(2)).unwrap();
        assert_eq!(closed.pane_ids(), vec![PaneId(1), PaneId(3), PaneId(4)]);
        let total: f64 = closed
            .compute_geometry(Rect::new(0.0, 0.0, 800.0, 600.0))
            .iter()
            .map(|p| p.rect.area())
            .sum();
        assert!((total - 800.0 * 600.0).abs() < 1.0);

        // Closing the first pane promotes the sibling subtree and hands off focus.
        let closed_first = tree.close_pane(PaneId(1)).unwrap();
        assert_eq!(
            closed_first.pane_ids(),
            vec![PaneId(2), PaneId(3), PaneId(4)]
        );
        assert_eq!(closed_first.active_pane_id(), PaneId(2));
    }

    #[test]
    fn close_pane_single_leaf_and_missing_pane_return_none() {
        let tree = PaneSplitTree::single_leaf(PaneId(1));
        assert!(tree.close_pane(PaneId(1)).is_none());

        let two = tree
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap();
        assert!(two.close_pane(PaneId(99)).is_none());
    }

    #[test]
    fn move_pane_swaps_with_neighbors_in_reading_order() {
        let tree = three_pane_tree();

        let moved = tree.move_pane(PaneId(1), SplitChild::Second).unwrap();
        assert_eq!(moved.pane_ids(), vec![PaneId(2), PaneId(1), PaneId(3)]);

        let moved_back = tree.move_pane(PaneId(3), SplitChild::First).unwrap();
        assert_eq!(moved_back.pane_ids(), vec![PaneId(1), PaneId(3), PaneId(2)]);

        // Tree shape and ratios are unchanged by a move.
        assert_eq!(
            moved.split_ratio_at_path(&[]),
            tree.split_ratio_at_path(&[])
        );
        assert_eq!(
            moved.split_ratio_at_path(&[SplitChild::Second]),
            tree.split_ratio_at_path(&[SplitChild::Second])
        );
    }

    #[test]
    fn move_pane_at_reading_order_ends_returns_none() {
        let tree = three_pane_tree();
        assert!(tree.move_pane(PaneId(1), SplitChild::First).is_none());
        assert!(tree.move_pane(PaneId(3), SplitChild::Second).is_none());
        assert!(tree.move_pane(PaneId(99), SplitChild::First).is_none());
        assert!(
            PaneSplitTree::single_leaf(PaneId(1))
                .move_pane(PaneId(1), SplitChild::Second)
                .is_none()
        );
    }

    #[test]
    fn move_pane_keeps_focus_on_moved_pane() {
        let tree = three_pane_tree();

        let moved = tree.move_pane(PaneId(1), SplitChild::Second).unwrap();

        assert_eq!(moved.active_pane_id(), PaneId(1));
    }

    #[test]
    fn keyboard_resize_moves_bordering_divider() {
        let tree = PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                SplitChild::Second,
            )
            .unwrap();

        // Pane 1 (left side): only a divider to its right; Right grows it.
        let (path, ratio) = tree
            .keyboard_resize(PaneId(1), PaneResizeDirection::Right, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert_eq!(path, Vec::new());
        assert!((ratio.value() - 0.55).abs() < 1e-9);
        assert!(
            tree.keyboard_resize(PaneId(1), PaneResizeDirection::Left, KEYBOARD_RESIZE_STEP)
                .is_none()
        );

        // Pane 2 (right side): only a divider to its left; Left shrinks pane 1.
        let (path, ratio) = tree
            .keyboard_resize(PaneId(2), PaneResizeDirection::Left, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert_eq!(path, Vec::new());
        assert!((ratio.value() - 0.45).abs() < 1e-9);
        assert!(
            tree.keyboard_resize(PaneId(2), PaneResizeDirection::Right, KEYBOARD_RESIZE_STEP)
                .is_none()
        );

        // No vertical divider anywhere in the tree.
        assert!(
            tree.keyboard_resize(PaneId(1), PaneResizeDirection::Down, KEYBOARD_RESIZE_STEP)
                .is_none()
        );
    }

    #[test]
    fn keyboard_resize_targets_deepest_bordering_split() {
        let tree = three_pane_tree(); // comb [1 | [2 | 3]]

        // Pane 3's bordering divider on the left belongs to the 2|3 split, not root.
        let (path, _) = tree
            .keyboard_resize(PaneId(3), PaneResizeDirection::Left, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert_eq!(path, vec![SplitChild::Second]);

        // Pane 2 borders the root divider on its left, the 2|3 divider on its right.
        let (path, _) = tree
            .keyboard_resize(PaneId(2), PaneResizeDirection::Left, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert_eq!(path, Vec::new());
        let (path, _) = tree
            .keyboard_resize(PaneId(2), PaneResizeDirection::Right, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert_eq!(path, vec![SplitChild::Second]);
    }

    #[test]
    fn keyboard_resize_clamps_at_ratio_bounds() {
        let near_max = PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::new(0.93).unwrap(),
                SplitChild::Second,
            )
            .unwrap();

        let (_, ratio) = near_max
            .keyboard_resize(PaneId(1), PaneResizeDirection::Right, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert!((ratio.value() - MAX_SPLIT_RATIO).abs() < 1e-9);

        // Already at the bound: the step cannot move the ratio.
        let mut at_max = near_max.clone();
        assert!(at_max.update_split_ratio(&[], SplitRatio::new(MAX_SPLIT_RATIO).unwrap()));
        assert!(
            at_max
                .keyboard_resize(PaneId(1), PaneResizeDirection::Right, KEYBOARD_RESIZE_STEP)
                .is_none()
        );

        let near_min = PaneSplitTree::single_leaf(PaneId(1))
            .split_pane(
                PaneId(1),
                PaneId(2),
                SplitOrientation::Horizontal,
                SplitRatio::new(0.07).unwrap(),
                SplitChild::Second,
            )
            .unwrap();
        let (_, ratio) = near_min
            .keyboard_resize(PaneId(2), PaneResizeDirection::Left, KEYBOARD_RESIZE_STEP)
            .unwrap();
        assert!((ratio.value() - MIN_SPLIT_RATIO).abs() < 1e-9);

        // Invalid steps and missing panes are rejected.
        assert!(
            near_max
                .keyboard_resize(PaneId(1), PaneResizeDirection::Right, 0.0)
                .is_none()
        );
        assert!(
            near_max
                .keyboard_resize(PaneId(1), PaneResizeDirection::Right, f64::NAN)
                .is_none()
        );
        assert!(
            near_max
                .keyboard_resize(PaneId(99), PaneResizeDirection::Right, KEYBOARD_RESIZE_STEP)
                .is_none()
        );
    }
}
