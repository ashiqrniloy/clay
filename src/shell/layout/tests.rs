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
    let ratio = compute_drag_ratio(SplitOrientation::Horizontal, area, Point::new(300.0, 400.0));
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
        (compute_slot_resize_size(FixedSlotId::Left, pane_rect, Point::new(250.0, 300.0)) - 250.0)
            .abs()
            < 1e-9
    );
    // Right: distance from right edge.
    assert!(
        (compute_slot_resize_size(FixedSlotId::Right, pane_rect, Point::new(600.0, 300.0)) - 200.0)
            .abs()
            < 1e-9
    );
    // Top: distance from top edge.
    assert!(
        (compute_slot_resize_size(FixedSlotId::Top, pane_rect, Point::new(400.0, 120.0)) - 120.0)
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
    let mut layout =
        WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout_with_left());
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
    let mut layout =
        WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout_with_left());
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
    let mut layout =
        WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout_with_left());
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
