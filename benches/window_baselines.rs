// Phase 22.6 (plan 077 task 5): window-model performance baselines — pane
// paint chrome geometry and tab-switch layout geometry. Both are pure
// geometry math over the pane tree (shell chrome pieces per pane count);
// editor-surface paint is viewport-bounded and benched separately in
// editor_baselines. Results pin the advisory PANE_PAINT_P95_BUDGET_MS and
// TAB_SWITCH_P95_BUDGET_MS constants (docs/development/performance.md).
use std::hint::black_box;

use clay::perf::baselines::{
    AccessibilityTreeBench, centered_overlay_geometry_work, command_centre_open_projection_work,
    completion_layout_work, completion_open_projection_work, completion_selection_work,
    pane_chrome_piece_count, responsive_layout_work, tab_switch_geometry_work,
    transient_menu_filter_work,
};
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

fn pane_paint_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("pane_paint_baselines");
    for panes in [1usize, 2, 4] {
        group.bench_with_input(BenchmarkId::from_parameter(panes), &panes, |b, &panes| {
            b.iter(|| pane_chrome_piece_count(panes));
        });
    }
    group.finish();
}

fn tab_switch_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("tab_switch_baselines");
    for panes in [1usize, 2, 4] {
        group.bench_with_input(BenchmarkId::from_parameter(panes), &panes, |b, &panes| {
            b.iter(|| tab_switch_geometry_work(panes));
        });
    }
    group.finish();
}

// Plan 088 Task 6: the real SDUI sidebar/editor slot decision across narrow,
// normal, wide, and large-typography inputs. Timings are local/advisory; the
// returned flags and module tests provide the deterministic layout contract.
fn responsive_layout_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("responsive_layout_baselines");
    for (width, ui_size) in [
        (320.0_f64, 12.0_f32),
        (900.0, 12.0),
        (1200.0, 12.0),
        (900.0, 24.0),
        (900.0, 96.0),
        (1200.0, 96.0),
    ] {
        let id = BenchmarkId::new(format!("{ui_size}px-ui"), width as u32);
        group.bench_with_input(id, &(width, ui_size), |b, &(width, ui_size)| {
            b.iter(|| responsive_layout_work(black_box(width), black_box(ui_size)));
        });
    }
    group.finish();
}

// Phase 24.4: centered Command Centre surface geometry — one scrim fill rect
// plus one surface rect and one rect per hosted overlay, O(overlay_count),
// independent of document size (advisory wall-clock, like the other groups).
fn centered_overlay_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("centered_overlay_baselines");
    for overlays in [1usize, 4, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(overlays),
            &overlays,
            |b, &overlays| {
                b.iter(|| centered_overlay_geometry_work(overlays));
            },
        );
    }
    group.finish();
}

// Plan 087: advisory completion/menu baselines. Hard bounds are covered by
// structural tests; these groups provide local timing signals only.
fn completion_open_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("completion_open_baselines");
    for items in [1usize, 8, 60, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, &items| {
            b.iter(|| completion_open_projection_work(black_box(items)));
        });
    }
    group.finish();
}

fn completion_filter_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("completion_filter_baselines");
    for (items, query) in [(16usize, ""), (60, "split"), (256, "split pane")] {
        group.bench_with_input(
            BenchmarkId::new(query, items),
            &(items, query),
            |b, &(items, query)| {
                b.iter(|| transient_menu_filter_work(black_box(items), black_box(query)));
            },
        );
    }
    group.finish();
}

// Command Centre and completion share the bounded transient-menu matcher and
// projection model, but their open/selection paths are distinct measurements.
fn command_centre_open_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_centre_open_baselines");
    for items in [16usize, 60, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(items), &items, |b, &items| {
            b.iter(|| command_centre_open_projection_work(black_box(items)));
        });
    }
    group.finish();
}

fn completion_selection_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("completion_selection_baselines");
    for (items, selected) in [(1usize, 0usize), (8, 7), (60, 59), (256, 255)] {
        group.bench_with_input(
            BenchmarkId::new(format!("{items} items"), selected),
            &(items, selected),
            |b, &(items, selected)| {
                b.iter(|| completion_selection_work(black_box(items), black_box(selected)));
            },
        );
    }
    group.finish();
}

fn accessibility_tree_update_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("accessibility_tree_update_baselines");
    for tabs in [2usize, 4, 8, 16] {
        group.bench_with_input(BenchmarkId::from_parameter(tabs), &tabs, |b, &tabs| {
            b.iter_batched(
                || AccessibilityTreeBench::new(tabs),
                |mut fixture| fixture.update(),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn completion_layout_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("completion_layout_baselines");
    for (items, caret_y) in [(1usize, 20.0), (8, 280.0), (256, 560.0)] {
        group.bench_with_input(
            BenchmarkId::new(format!("{items} items"), caret_y as u32),
            &(items, caret_y),
            |b, &(items, caret_y)| {
                b.iter(|| completion_layout_work(black_box(items), black_box(caret_y)));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    pane_paint_baselines,
    tab_switch_baselines,
    responsive_layout_baselines,
    centered_overlay_baselines,
    completion_open_baselines,
    completion_filter_baselines,
    command_centre_open_baselines,
    completion_selection_baselines,
    accessibility_tree_update_baselines,
    completion_layout_baselines
);
criterion_main!(benches);
