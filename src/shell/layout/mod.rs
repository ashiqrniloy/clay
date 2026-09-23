#![allow(dead_code)] // Persisted layout schema retains operations exercised by compatibility tests.

// Phase 18.2 installs generic shell layout foundations before every split
// constructor/update path is exercised by non-test runtime code.
use std::collections::{BTreeMap, BTreeSet};

use kurbo::{Point, Rect};

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
    #[cfg(test)]
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
    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_resized_by_user(mut self, resized_by_user: bool) -> Self {
        self.resized_by_user = resized_by_user;
        self
    }

    /// Resize to `new_size`, clamped to `min_size..=max_size`. Sets `resized_by_user`.
    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn has_main_slot(&self) -> bool {
        let _ = self.main;
        true
    }

    pub(crate) fn contains_slot(&self, slot_id: PaneSlotId) -> bool {
        match slot_id {
            #[cfg(test)]
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
#[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
pub(crate) struct PaneSlotObservation {
    pub(crate) pane_id: PaneId,
    pub(crate) slot_id: PaneSlotId,
    pub(crate) rect: Rect,
    pub(crate) visible: bool,
    pub(crate) resized_by_user: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct ShellComponentObservation {
    pub(crate) id: ShellComponentId,
    pub(crate) kind: ShellComponentKind,
    pub(crate) pane_id: PaneId,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn working_area_id(&self) -> WorkingAreaId {
        self.working_area_id
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

    #[cfg(test)]
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

    #[cfg(test)]
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

#[cfg(test)]
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
mod tests;
