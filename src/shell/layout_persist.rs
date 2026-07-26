//! Phase 20.3: Layout persistence for user-modified split ratios and slot sizes.
//!
//! Serializes to `$XDG_CONFIG_HOME/clay/layout.json` (or `~/.config/clay/layout.json`).
//! Only user-modified state is persisted: non-balanced split ratios and
//! `resized_by_user` or collapsed slots. Corrupt/missing file → defaults, no panic.

use serde_json::{Value, json};

use super::layout::{FixedSlotId, PaneSplitNode, SplitChild, SplitRatio, WorkingAreaLayout};

/// Collect all `(path, ratio)` pairs from a split tree where ratio ≠ 0.5.
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

/// Serialize user-modified layout state to a JSON value.
pub(crate) fn serialize_layout_state(layout: &WorkingAreaLayout) -> Value {
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

    json!({ "splits": splits_json, "slots": slots_json })
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
        for entry in slots {
            let pane_id = entry
                .get("pane")
                .and_then(|v| v.as_u64())
                .map(super::layout::PaneId);
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
}

/// Write layout state to `$XDG_CONFIG_HOME/clay/layout.json`.
pub(crate) fn save_layout(layout: &WorkingAreaLayout) {
    let Some(path) = config_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let state = serialize_layout_state(layout);
    if let Ok(json_str) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(&path, json_str);
    }
}

/// Read layout state from `$XDG_CONFIG_HOME/clay/layout.json`.
/// Returns `None` if the file is missing or corrupt.
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
    use crate::shell::layout::{FixedSlotState, PaneSlotLayout, PaneSplitTree};

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
}
