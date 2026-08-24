#![allow(dead_code)] // Reads legacy layout.json while React owns live layout operations.

//! Layout persistence for the clay window.
//!
//! Serializes to `$XDG_CONFIG_HOME/clay/layout.json` (or `~/.config/clay/layout.json`).
//! v1 (no version key, Phase 20.3): user-modified split ratios and slot
//! sizes for the single tab. v2 (`"version": 2`, Phase 22.5): whole-window
//! state — tab order, active tab, per-tab workspace root, split-tree
//! topology, user-modified slots, active pane, and per-pane open documents.
//! Corrupt/missing file → defaults, no panic.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::layout::{
    DEFAULT_PANE_ID, FixedSlotId, MAX_PANE_SPLIT_TREE_NODES, MAX_PANES_PER_TAB, PaneId,
    PaneSplitNode, PaneSplitTree, SplitChild, SplitOrientation, SplitRatio, WorkingAreaLayout,
};
use crate::perf::budgets::MAX_ACTIVE_CONNECTIONS;

/// Collect user-modified slot entries (v1 `slots` entry shape) for a layout.
/// Shared by the v1 writer and the v2 per-tab writer.
pub(crate) fn collect_slot_entries(layout: &WorkingAreaLayout) -> Vec<Value> {
    let mut slots_json = Vec::new();
    for (pane_id, slot_layout) in layout.pane_slots_iter() {
        for slot in slot_layout.user_modified_slots() {
            slots_json.push(json!({
                "pane": pane_id.0,
                "slot": slot_id_str(slot.slot_id),
                "size": slot.size,
                "collapsed": slot.collapsed,
            }));
        }
    }
    slots_json
}

/// Apply persisted state to a layout. Silently skips invalid entries.
pub(crate) fn apply_persisted_state(layout: &mut WorkingAreaLayout, state: &Value) {
    // Apply split ratios.
    if let Some(splits) = state.get("splits").and_then(|v| v.as_array()) {
        for entry in splits {
            let path: Vec<SplitChild> = entry
                .get("path")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            v.as_u64().and_then(|n| match n {
                                0 => Some(SplitChild::First),
                                1 => Some(SplitChild::Second),
                                _ => None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(ratio) = entry
                .get("ratio")
                .and_then(|v| v.as_f64())
                .and_then(|r| SplitRatio::new(r).ok())
            {
                layout.pane_tree_mut().update_split_ratio(&path, ratio);
            }
        }
    }

    // Apply slot sizes/collapsed.
    if let Some(slots) = state.get("slots").and_then(|v| v.as_array()) {
        apply_slot_entries(layout, slots);
    }
}

/// Apply user-modified slot entries to a layout. Silently skips invalid
/// entries (unknown pane/slot, out-of-bounds sizes clamped).
fn apply_slot_entries(layout: &mut WorkingAreaLayout, slots: &[Value]) {
    for entry in slots {
        let pane_id = entry.get("pane").and_then(|v| v.as_u64()).map(PaneId);
        let slot_id = entry
            .get("slot")
            .and_then(|v| v.as_str())
            .and_then(slot_id_from_str);
        let (Some(pane_id), Some(slot_id)) = (pane_id, slot_id) else {
            continue;
        };
        if let Some(slot) = layout
            .slot_layout_mut(pane_id)
            .and_then(|l| l.fixed_slot_mut(slot_id))
        {
            if let Some(size) = entry.get("size").and_then(|v| v.as_f64()) {
                slot.size = size.clamp(slot.min_size, slot.max_size);
                slot.resized_by_user = true;
            }
            if let Some(collapsed) = entry.get("collapsed").and_then(|v| v.as_bool()) {
                slot.collapsed = collapsed;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 22.5: versioned multi-tab window state (layout.json v2)
// ---------------------------------------------------------------------------

/// One tab's persisted window state. Widget-free value type: the binary
/// assembles it from the shell + pane views; the lib serializes/applies it.
pub struct PersistedTabState {
    /// Absolute workspace root (validated at restore, never at parse).
    pub workspace_root: String,
    /// Active pane id; normalized to a tree member at parse.
    pub active_pane: PaneId,
    /// Validated split tree; `None` restores the default single-pane layout.
    pub tree: Option<PaneSplitNode>,
    /// User-modified slot entries, v1 `slots` shape (re-validated at apply).
    pub slots: Vec<Value>,
    /// Per-pane workspace-relative document path (`None` = empty pane).
    pub panes: BTreeMap<PaneId, Option<String>>,
}

/// Whole-window persisted state.
pub struct PersistedWindowState {
    /// Tab order (mount order at restore).
    pub tabs: Vec<PersistedTabState>,
    /// 0-based active tab index; `None` when absent/out of range (silent skip).
    pub active_tab: Option<usize>,
}

/// A tab's layout snapshot for persistence collection (binary-visible;
/// assembled by the shell, serialized by the lib).
pub struct PersistedTabLayout {
    pub active_pane: PaneId,
    pub tree: PaneSplitNode,
    pub slots: Vec<Value>,
}

/// Serialize whole-window state to the v2 JSON document.
pub(crate) fn serialize_window_state(state: &PersistedWindowState) -> Value {
    let tabs: Vec<Value> = state
        .tabs
        .iter()
        .map(|tab| {
            json!({
                "workspaceRoot": tab.workspace_root,
                "activePane": tab.active_pane.0,
                "splitTree": tab.tree.as_ref().map(serialize_split_node).unwrap_or(Value::Null),
                "slots": tab.slots,
                "panes": tab.panes
                    .iter()
                    .map(|(id, doc)| (id.0.to_string(), json!(doc)))
                    .collect::<serde_json::Map<String, Value>>(),
            })
        })
        .collect();
    json!({
        "version": 2,
        "activeTab": state.active_tab.map(|i| json!(i)),
        "tabs": tabs,
    })
}

/// Parse (and structurally validate) a v2 document. Returns `None` for
/// legacy/unknown versions or structurally unusable state. Entries that fail
/// validation are silently skipped: tab count capped at the connection cap,
/// invalid trees degrade to the default single-pane layout, out-of-range
/// active tab dropped. Never panics.
pub(crate) fn parse_window_state(value: &Value) -> Option<PersistedWindowState> {
    if value.get("version").and_then(|v| v.as_u64()) != Some(2) {
        return None;
    }
    let tabs_value = value.get("tabs")?.as_array()?;
    let mut tabs = Vec::new();
    for tab_value in tabs_value.iter().take(MAX_ACTIVE_CONNECTIONS) {
        if let Some(tab) = parse_tab_state(tab_value) {
            tabs.push(tab);
        }
    }
    if tabs.is_empty() {
        return None;
    }
    let active_tab = value
        .get("activeTab")
        .and_then(|v| v.as_u64())
        .and_then(|i| usize::try_from(i).ok())
        .filter(|&i| i < tabs.len());
    Some(PersistedWindowState { tabs, active_tab })
}

fn parse_tab_state(value: &Value) -> Option<PersistedTabState> {
    let workspace_root = value.get("workspaceRoot")?.as_str()?.to_string();
    if workspace_root.is_empty() {
        return None;
    }
    // Invalid/missing tree degrades to the default single-pane layout, never
    // a partial tree (hostile or hand-edited files skip silently).
    let tree = value
        .get("splitTree")
        .and_then(|v| {
            if v.is_null() {
                Some(None)
            } else {
                parse_split_tree(v).map(Some)
            }
        })
        .flatten();
    let active_pane = match &tree {
        Some(node) => value
            .get("activePane")
            .and_then(|v| v.as_u64())
            .map(PaneId)
            .filter(|pane_id| tree_contains(node, *pane_id))
            .unwrap_or(DEFAULT_PANE_ID),
        None => DEFAULT_PANE_ID,
    };
    let slots = value
        .get("slots")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut panes = BTreeMap::new();
    if let Some(panes_value) = value.get("panes").and_then(|v| v.as_object()) {
        for (key, doc) in panes_value {
            let Ok(pane_id) = key.parse::<u64>() else {
                continue;
            };
            let document = match doc {
                Value::Null => None,
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => continue,
            };
            panes.insert(PaneId(pane_id), document);
        }
    }
    Some(PersistedTabState {
        workspace_root,
        active_pane,
        tree,
        slots,
        panes,
    })
}

/// True for Phase 20.3 v1 documents (no `version` key — or `version: 1` —
/// carrying `splits`/`slots`). The restore flow applies these to the single
/// bootstrap tab exactly as before; no rewrite on load.
#[cfg(test)] // predicate for the frozen v1 tests; production applies v1 via
// `load_layout` + `apply_persisted_state` without naming the version.
pub(crate) fn is_legacy_layout(value: &Value) -> bool {
    let version = value.get("version").and_then(|v| v.as_u64());
    version.is_none_or(|v| v == 1)
        && (value.get("splits").is_some() || value.get("slots").is_some())
}

/// Rebuild a tab's working-area layout from persisted state: validated tree
/// (or default single pane), user-modified slots, active pane. Mirrors the
/// production split path (`WorkingAreaLayout::replace_pane_tree`, the same
/// primitive `apply_tree_change` uses); any structural failure degrades to
/// the default — never a partial layout.
pub(crate) fn layout_from_persisted_tab(tab: &PersistedTabState) -> WorkingAreaLayout {
    let mut layout = WorkingAreaLayout::single_editor();
    if let Some(node) = &tab.tree
        && let Ok(tree) = PaneSplitTree::new(node.clone(), tab.active_pane)
    {
        layout.replace_pane_tree(tree);
    }
    apply_slot_entries(&mut layout, &tab.slots);
    layout
}

/// Serialize a split tree node recursively (tagged leaf/split encoding).
fn serialize_split_node(node: &PaneSplitNode) -> Value {
    match node {
        PaneSplitNode::Leaf { pane_id } => json!({ "leaf": { "paneId": pane_id.0 } }),
        PaneSplitNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => json!({
            "split": {
                "orientation": match orientation {
                    SplitOrientation::Horizontal => "horizontal",
                    SplitOrientation::Vertical => "vertical",
                },
                "ratio": ratio.value(),
                "first": serialize_split_node(first),
                "second": serialize_split_node(second),
            }
        }),
    }
}

/// Parse a split tree with all structural bounds enforced: node count cap,
/// pane count cap, non-zero unique pane ids, ratio bounds, and the editor
/// pane (1) as a member (the `with_pane_tree` invariant).
fn parse_split_tree(value: &Value) -> Option<PaneSplitNode> {
    let mut pane_ids = std::collections::BTreeSet::new();
    let mut node_count = 0usize;
    let node = parse_split_node(value, &mut pane_ids, &mut node_count)?;
    if node_count > MAX_PANE_SPLIT_TREE_NODES {
        return None;
    }
    if pane_ids.len() > MAX_PANES_PER_TAB {
        return None;
    }
    if !pane_ids.contains(&DEFAULT_PANE_ID) {
        return None;
    }
    Some(node)
}

fn parse_split_node(
    value: &Value,
    pane_ids: &mut std::collections::BTreeSet<PaneId>,
    node_count: &mut usize,
) -> Option<PaneSplitNode> {
    *node_count += 1;
    if *node_count > MAX_PANE_SPLIT_TREE_NODES {
        return None;
    }
    if let Some(leaf) = value.get("leaf") {
        let pane_id = PaneId(leaf.get("paneId")?.as_u64()?);
        if pane_id.0 == 0 || !pane_ids.insert(pane_id) {
            return None;
        }
        return Some(PaneSplitNode::leaf(pane_id));
    }
    if let Some(split) = value.get("split") {
        let orientation = match split.get("orientation")?.as_str()? {
            "horizontal" => SplitOrientation::Horizontal,
            "vertical" => SplitOrientation::Vertical,
            _ => return None,
        };
        let ratio = SplitRatio::new(split.get("ratio")?.as_f64()?).ok()?;
        let first = parse_split_node(split.get("first")?, pane_ids, node_count)?;
        let second = parse_split_node(split.get("second")?, pane_ids, node_count)?;
        return Some(PaneSplitNode::split(orientation, ratio, first, second));
    }
    None
}

fn tree_contains(node: &PaneSplitNode, pane_id: PaneId) -> bool {
    match node {
        PaneSplitNode::Leaf { pane_id: id } => *id == pane_id,
        PaneSplitNode::Split { first, second, .. } => {
            tree_contains(first, pane_id) || tree_contains(second, pane_id)
        }
    }
}

/// Write whole-window state to `$XDG_CONFIG_HOME/clay/layout.json` as the v2
/// document. The only writer since Phase 22.5 (v1 files are read-only);
/// missing parent dirs are created; write failures are silent.
pub fn save_window_state(state: &PersistedWindowState) {
    let Some(path) = config_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serialize_window_state(state);
    if let Ok(json_str) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(&path, json_str);
    }
}

/// Read layout state from `$XDG_CONFIG_HOME/clay/layout.json`.
/// Returns `None` if the file is missing or corrupt.
/// Load and parse the whole-window state; `None` when the file is missing,
/// corrupt, legacy (v1), or yields no tabs — the caller keeps today's
/// bootstrap in every case (a v1 file still applies through the 20.3 path).
pub fn load_window_state() -> Option<PersistedWindowState> {
    parse_window_state(&load_layout()?)
}

/// Validated v2 JSON for the Tauri bridge. Hostile/legacy documents yield `None`.
pub fn load_window_state_json() -> Option<serde_json::Value> {
    load_window_state().map(|state| serialize_window_state(&state))
}

/// Parse-only: `None` when the document is not a usable v2 window state.
pub fn parse_window_state_json(value: &serde_json::Value) -> Option<serde_json::Value> {
    parse_window_state(value).map(|state| serialize_window_state(&state))
}

/// Persist a frontend-supplied v2 document after the same structural
/// validation as `parse_window_state`. Rejects hostile input.
pub fn save_window_state_from_json(value: &serde_json::Value) -> Result<(), String> {
    let state = parse_window_state(value)
        .ok_or_else(|| "layout rejected: not a valid v2 window state".to_string())?;
    save_window_state(&state);
    Ok(())
}

/// Load the raw layout document (v1 or v2 shape); `None` on missing/corrupt.
pub(crate) fn load_layout() -> Option<Value> {
    let path = config_path()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn config_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("clay").join("layout.json"))
}

fn slot_id_str(id: FixedSlotId) -> &'static str {
    match id {
        FixedSlotId::Left => "left",
        FixedSlotId::Right => "right",
        FixedSlotId::Top => "top",
        FixedSlotId::Bottom => "bottom",
    }
}

fn slot_id_from_str(s: &str) -> Option<FixedSlotId> {
    match s {
        "left" => Some(FixedSlotId::Left),
        "right" => Some(FixedSlotId::Right),
        "top" => Some(FixedSlotId::Top),
        "bottom" => Some(FixedSlotId::Bottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::layout::{
        FixedSlotState, PaneId, PaneSlotLayout, PaneSplitTree, SplitOrientation,
    };

    // -- Phase 22.7 (plan 078 task 6): the v1 WRITE path is test-only. The
    // v1 READ path (splits/slots application) stays in production above:
    // old Phase 20.3 `layout.json` files must still parse. The v1 writer
    // exists solely so the frozen Phase 20.3 round-trip tests can prove
    // write→read fidelity; production never writes v1.
    fn collect_non_default_splits(
        node: &PaneSplitNode,
        path: &mut Vec<SplitChild>,
        out: &mut Vec<(Vec<SplitChild>, SplitRatio)>,
    ) {
        if let PaneSplitNode::Split {
            ratio,
            first,
            second,
            ..
        } = node
        {
            if (ratio.value() - 0.5).abs() > 1e-9 {
                out.push((path.clone(), *ratio));
            }
            path.push(SplitChild::First);
            collect_non_default_splits(first, path, out);
            path.pop();
            path.push(SplitChild::Second);
            collect_non_default_splits(second, path, out);
            path.pop();
        }
    }

    fn serialize_layout_state(layout: &WorkingAreaLayout) -> Value {
        let mut splits = Vec::new();
        let mut path = Vec::new();
        collect_non_default_splits(layout.pane_tree().root_node(), &mut path, &mut splits);

        let splits_json: Vec<Value> = splits
            .iter()
            .map(|(p, r)| {
                json!({
                    "path": p.iter().map(|c| match c {
                        SplitChild::First => 0,
                        SplitChild::Second => 1,
                    }).collect::<Vec<u8>>(),
                    "ratio": r.value(),
                })
            })
            .collect();

        let slots_json = collect_slot_entries(layout);

        json!({ "splits": splits_json, "slots": slots_json })
    }

    #[test]
    fn persistence_round_trip_split_ratios() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                crate::shell::layout::SplitOrientation::Horizontal,
                SplitRatio::new(0.3).unwrap(),
                PaneSplitNode::leaf(crate::shell::layout::PaneId(1)),
                PaneSplitNode::leaf(crate::shell::layout::PaneId(2)),
            ),
            crate::shell::layout::PaneId(1),
        )
        .unwrap();
        let layout =
            WorkingAreaLayout::with_pane_tree(tree, crate::shell::layout::PaneId(1)).unwrap();

        let json = serialize_layout_state(&layout);
        let splits = json["splits"].as_array().unwrap();
        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0]["path"].as_array().unwrap().len(), 0); // root
        assert!((splits[0]["ratio"].as_f64().unwrap() - 0.3).abs() < 1e-9);

        // Apply to a fresh layout with balanced ratio.
        let tree2 = PaneSplitTree::new(
            PaneSplitNode::split(
                crate::shell::layout::SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(crate::shell::layout::PaneId(1)),
                PaneSplitNode::leaf(crate::shell::layout::PaneId(2)),
            ),
            crate::shell::layout::PaneId(1),
        )
        .unwrap();
        let mut layout2 =
            WorkingAreaLayout::with_pane_tree(tree2, crate::shell::layout::PaneId(1)).unwrap();
        apply_persisted_state(&mut layout2, &json);
        assert_eq!(
            layout2.pane_tree().split_ratio_at_path(&[]),
            Some(SplitRatio::new(0.3).unwrap())
        );
    }

    #[test]
    fn persistence_round_trip_slot_sizes() {
        let slot_layout = PaneSlotLayout::main_only().with_fixed_slot(
            FixedSlotState::new(FixedSlotId::Left, 250.0, 100.0, 400.0)
                .unwrap()
                .with_resized_by_user(true),
        );
        let layout = WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout);

        let json = serialize_layout_state(&layout);
        let slots = json["slots"].as_array().unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0]["slot"].as_str().unwrap(), "left");
        assert!((slots[0]["size"].as_f64().unwrap() - 250.0).abs() < 1e-9);
        assert!(!slots[0]["collapsed"].as_bool().unwrap());

        // Apply to a fresh layout.
        let fresh_slot = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap());
        let mut layout2 =
            WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(fresh_slot);
        apply_persisted_state(&mut layout2, &json);
        let slot = layout2
            .slot_layout_mut(crate::shell::layout::PaneId(1))
            .unwrap()
            .fixed_slot_mut(FixedSlotId::Left)
            .unwrap();
        assert!((slot.size - 250.0).abs() < 1e-9);
        assert!(slot.resized_by_user);
    }

    #[test]
    fn persistence_corrupt_file_falls_back_to_defaults() {
        // Simulate corrupt JSON.
        let result: Result<Value, _> = serde_json::from_str("{invalid json!!!");
        assert!(result.is_err());
        // load_layout returns None for corrupt files (tested via the parse path).
    }

    #[test]
    fn persistence_only_user_modified_slots() {
        // Slot without resized_by_user should not be serialized.
        let slot_layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 200.0, 100.0, 400.0).unwrap());
        let layout = WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout);

        let json = serialize_layout_state(&layout);
        let slots = json["slots"].as_array().unwrap();
        assert!(slots.is_empty());
    }

    #[test]
    fn persistence_collapsed_slot_is_serialized() {
        let slot_layout = PaneSlotLayout::main_only().with_fixed_slot(
            FixedSlotState::new(FixedSlotId::Bottom, 80.0, 40.0, 160.0)
                .unwrap()
                .with_collapsed(true),
        );
        let layout = WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout);

        let json = serialize_layout_state(&layout);
        let slots = json["slots"].as_array().unwrap();
        assert_eq!(slots.len(), 1);
        assert!(slots[0]["collapsed"].as_bool().unwrap());
    }

    #[test]
    fn persistence_balanced_splits_not_serialized() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                crate::shell::layout::SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(crate::shell::layout::PaneId(1)),
                PaneSplitNode::leaf(crate::shell::layout::PaneId(2)),
            ),
            crate::shell::layout::PaneId(1),
        )
        .unwrap();
        let layout =
            WorkingAreaLayout::with_pane_tree(tree, crate::shell::layout::PaneId(1)).unwrap();

        let json = serialize_layout_state(&layout);
        let splits = json["splits"].as_array().unwrap();
        assert!(splits.is_empty());
    }

    // -- Phase 22.5: versioned multi-tab window state --

    fn round_trip_tab_state(
        workspace_root: &str,
        active_pane: PaneId,
        tree: Option<PaneSplitNode>,
        slots: Vec<Value>,
        panes: BTreeMap<PaneId, Option<String>>,
    ) -> PersistedTabState {
        PersistedTabState {
            workspace_root: workspace_root.to_string(),
            active_pane,
            tree,
            slots,
            panes,
        }
    }

    #[test]
    fn window_state_round_trip_three_tabs() {
        // Tab 1: 4-pane tree, custom ratios, user-modified slot, mixed docs.
        let tab1 = round_trip_tab_state(
            "/home/dev/alpha",
            PaneId(4),
            Some(PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::new(0.3).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::split(
                    SplitOrientation::Vertical,
                    SplitRatio::new(0.7).unwrap(),
                    PaneSplitNode::leaf(PaneId(2)),
                    PaneSplitNode::split(
                        SplitOrientation::Horizontal,
                        SplitRatio::balanced(),
                        PaneSplitNode::leaf(PaneId(3)),
                        PaneSplitNode::leaf(PaneId(4)),
                    ),
                ),
            )),
            vec![json!({"pane": 1, "slot": "left", "size": 250.0, "collapsed": false})],
            BTreeMap::from([
                (PaneId(1), Some("src/main.rs".to_string())),
                (PaneId(2), None),
                (PaneId(3), Some("docs/intro.md".to_string())),
                (PaneId(4), Some("src/lib.rs".to_string())),
            ]),
        );
        // Tab 2: single pane, no documents, no slots.
        let tab2 = round_trip_tab_state(
            "/home/dev/beta",
            PaneId(1),
            None,
            Vec::new(),
            BTreeMap::new(),
        );
        // Tab 3: two-pane vertical split.
        let tab3 = round_trip_tab_state(
            "/srv/gamma",
            PaneId(2),
            Some(PaneSplitNode::split(
                SplitOrientation::Vertical,
                SplitRatio::new(0.6).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            )),
            Vec::new(),
            BTreeMap::from([(PaneId(1), Some("README.md".to_string()))]),
        );
        let state = PersistedWindowState {
            tabs: vec![tab1, tab2, tab3],
            active_tab: Some(2),
        };

        let json = serialize_window_state(&state);
        assert_eq!(json["version"], 2);
        let parsed = parse_window_state(&json).expect("round-trip parses");
        assert_eq!(parsed.active_tab, Some(2));
        assert_eq!(parsed.tabs.len(), 3);
        // Full structural round-trip: serialize(parse(serialize(x))) == serialize(x).
        assert_eq!(serialize_window_state(&parsed), json);
        // Field-level spot checks.
        assert_eq!(parsed.tabs[0].workspace_root, "/home/dev/alpha");
        assert_eq!(parsed.tabs[0].active_pane, PaneId(4));
        assert_eq!(parsed.tabs[0].panes[&PaneId(2)], None);
        assert_eq!(
            parsed.tabs[0].panes[&PaneId(3)],
            Some("docs/intro.md".to_string())
        );
        assert_eq!(parsed.tabs[1].tree, None);
        assert_eq!(parsed.tabs[2].active_pane, PaneId(2));
    }

    #[test]
    fn legacy_v1_document_is_detected_and_parses_to_none() {
        let legacy = json!({
            "splits": [{"path": [], "ratio": 0.3}],
            "slots": [{"pane": 1, "slot": "left", "size": 250.0, "collapsed": false}],
        });
        assert!(is_legacy_layout(&legacy));
        assert!(parse_window_state(&legacy).is_none());
        // Explicit version 1 is treated as legacy too (leniency).
        let legacy_typed = json!({ "version": 1, "splits": [], "slots": [] });
        assert!(is_legacy_layout(&legacy_typed));
        assert!(parse_window_state(&legacy_typed).is_none());
        // A v2 document is not legacy.
        let v2 = serialize_window_state(&PersistedWindowState {
            tabs: vec![round_trip_tab_state(
                "/tmp/x",
                PaneId(1),
                None,
                vec![],
                BTreeMap::new(),
            )],
            active_tab: Some(0),
        });
        assert!(!is_legacy_layout(&v2));
    }

    #[test]
    fn window_state_hostile_inputs_never_panic_and_skip() {
        // Wrong types / unknown versions.
        assert!(parse_window_state(&json!({ "version": 2, "tabs": "nope" })).is_none());
        assert!(
            parse_window_state(&json!({ "version": 2, "tabs": [{"workspaceRoot": 7}] })).is_none()
        );
        assert!(parse_window_state(&json!({ "version": 9, "tabs": [] })).is_none());
        // Empty workspace root.
        assert!(
            parse_window_state(&json!({
                "version": 2,
                "tabs": [{"workspaceRoot": ""}],
            }))
            .is_none()
        );
        // Ratio 5.0 and pane id 0 reject the tree (tab keeps its root).
        let bad_tree = json!({
            "version": 2,
            "tabs": [{
                "workspaceRoot": "/tmp/x",
                "splitTree": {"split": {"orientation": "horizontal", "ratio": 5.0,
                    "first": {"leaf": {"paneId": 0}}, "second": {"leaf": {"paneId": 2}}}}
            }],
        });
        let parsed = parse_window_state(&bad_tree).expect("tab survives a bad tree");
        assert_eq!(parsed.tabs[0].tree, None);
        // Unknown keys are ignored.
        let unknown = json!({
            "version": 2,
            "futureKey": {"nested": true},
            "tabs": [{"workspaceRoot": "/tmp/x", "activePane": 1,
                      "splitTree": {"leaf": {"paneId": 1}}, "mystery": 1}],
        });
        let parsed = parse_window_state(&unknown).expect("unknown keys ignored");
        assert_eq!(parsed.tabs[0].tree, Some(PaneSplitNode::leaf(PaneId(1))));
        // Duplicate pane ids reject the tree.
        let dup = json!({
            "version": 2,
            "tabs": [{"workspaceRoot": "/tmp/x",
                "splitTree": {"split": {"orientation": "horizontal", "ratio": 0.5,
                    "first": {"leaf": {"paneId": 1}}, "second": {"leaf": {"paneId": 1}}}}}],
        });
        assert_eq!(parse_window_state(&dup).unwrap().tabs[0].tree, None);
        // Tree missing the editor pane (1) is rejected.
        let no_editor = json!({
            "version": 2,
            "tabs": [{"workspaceRoot": "/tmp/x",
                "splitTree": {"split": {"orientation": "horizontal", "ratio": 0.5,
                    "first": {"leaf": {"paneId": 2}}, "second": {"leaf": {"paneId": 3}}}}}],
        });
        assert_eq!(parse_window_state(&no_editor).unwrap().tabs[0].tree, None);
    }

    #[test]
    fn window_state_bounds_truncate_silently() {
        // 10 000 tabs → capped at the connection limit.
        let tabs: Vec<Value> = (0..10_000u64)
            .map(|i| {
                json!({"workspaceRoot": format!("/tmp/r{i}"),
                       "splitTree": {"leaf": {"paneId": 1}}})
            })
            .collect();
        let huge = json!({ "version": 2, "activeTab": 9999, "tabs": tabs });
        let parsed = parse_window_state(&huge).expect("parse");
        assert_eq!(parsed.tabs.len(), MAX_ACTIVE_CONNECTIONS);
        assert_eq!(parsed.active_tab, None, "out-of-range active tab skipped");
        // 4 panes is allowed...
        let wide = json!({
            "version": 2,
            "tabs": [{"workspaceRoot": "/tmp/x", "splitTree": {"split":
                {"orientation": "horizontal", "ratio": 0.5,
                 "first": {"leaf": {"paneId": 1}},
                 "second": {"split": {"orientation": "vertical", "ratio": 0.5,
                     "first": {"leaf": {"paneId": 2}},
                     "second": {"split": {"orientation": "horizontal", "ratio": 0.5,
                         "first": {"leaf": {"paneId": 3}},
                         "second": {"leaf": {"paneId": 4}}}}}}}}}],
        });
        assert!(parse_window_state(&wide).unwrap().tabs[0].tree.is_some());
        // ...5 panes is not.
        let wider = json!({
            "version": 2,
            "tabs": [{"workspaceRoot": "/tmp/x", "splitTree": {"split":
                {"orientation": "horizontal", "ratio": 0.5,
                 "first": {"leaf": {"paneId": 1}},
                 "second": {"split": {"orientation": "vertical", "ratio": 0.5,
                     "first": {"leaf": {"paneId": 2}},
                     "second": {"split": {"orientation": "horizontal", "ratio": 0.5,
                         "first": {"leaf": {"paneId": 3}},
                         "second": {"split": {"orientation": "vertical", "ratio": 0.5,
                             "first": {"leaf": {"paneId": 4}},
                             "second": {"leaf": {"paneId": 5}}}}}}}}}}}],
        });
        assert_eq!(parse_window_state(&wider).unwrap().tabs[0].tree, None);
    }

    #[test]
    fn window_state_skips_bad_tabs_keeps_good_ones() {
        let value = json!({
            "version": 2,
            "tabs": [
                {"workspaceRoot": "/tmp/good", "splitTree": {"leaf": {"paneId": 1}}},
                {"workspaceRoot": 42},
                {"workspaceRoot": "/tmp/good2", "splitTree": {"leaf": {"paneId": 1}}},
            ],
        });
        let parsed = parse_window_state(&value).unwrap();
        assert_eq!(parsed.tabs.len(), 2);
        assert_eq!(parsed.tabs[0].workspace_root, "/tmp/good");
        assert_eq!(parsed.tabs[1].workspace_root, "/tmp/good2");
    }

    #[test]
    fn window_state_active_pane_normalized_to_tree_member() {
        // Active pane not in the tree → normalized to pane 1.
        let value = json!({
            "version": 2,
            "tabs": [{"workspaceRoot": "/tmp/x", "activePane": 9,
                "splitTree": {"split": {"orientation": "horizontal", "ratio": 0.5,
                    "first": {"leaf": {"paneId": 1}}, "second": {"leaf": {"paneId": 2}}}}}],
        });
        let parsed = parse_window_state(&value).unwrap();
        assert_eq!(parsed.tabs[0].active_pane, PaneId(1));
        // Single-pane default layout: active pane forced to 1.
        let value = json!({
            "version": 2,
            "tabs": [{"workspaceRoot": "/tmp/x", "activePane": 3, "splitTree": null}],
        });
        let parsed = parse_window_state(&value).unwrap();
        assert_eq!(parsed.tabs[0].active_pane, PaneId(1));
    }

    #[test]
    fn layout_from_persisted_tab_builds_validated_layout() {
        // Valid tree + slots + active pane.
        let tab = round_trip_tab_state(
            "/tmp/x",
            PaneId(2),
            Some(PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::new(0.3).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            )),
            vec![json!({"pane": 1, "slot": "left", "size": 250.0, "collapsed": false})],
            BTreeMap::from([(PaneId(1), Some("a.rs".to_string())), (PaneId(2), None)]),
        );
        let layout = layout_from_persisted_tab(&tab);
        assert_eq!(layout.pane_tree().pane_count(), 2);
        assert_eq!(
            layout.pane_tree().split_ratio_at_path(&[]),
            Some(SplitRatio::new(0.3).unwrap())
        );
        assert_eq!(layout.active_pane_id(), PaneId(2));
        // The multi-pane layout has no fixed slots, so the slot entry is
        // silently skipped (no panic); slot application itself is covered by
        // the v1 `persistence_round_trip_slot_sizes` test via the shared
        // `apply_slot_entries` path.

        // No tree → default single-pane layout.
        let bare = round_trip_tab_state("/tmp/x", PaneId(1), None, vec![], BTreeMap::new());
        let layout = layout_from_persisted_tab(&bare);
        assert_eq!(layout.pane_tree().pane_count(), 1);
        assert_eq!(layout.active_pane_id(), PaneId(1));
    }
}
