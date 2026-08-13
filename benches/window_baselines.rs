// Phase 22.6 (plan 077 task 5): window-model performance baselines — pane
// paint chrome geometry and tab-switch layout geometry. Both are pure
// geometry math over the pane tree (shell chrome pieces per pane count);
// editor-surface paint is viewport-bounded and benched separately in
// editor_baselines. Results pin the advisory PANE_PAINT_P95_BUDGET_MS and
// TAB_SWITCH_P95_BUDGET_MS constants (docs/development/performance.md).
use clay::perf::baselines::{
    centered_overlay_geometry_work, pane_chrome_piece_count, tab_switch_geometry_work,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

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

criterion_group!(
    benches,
    pane_paint_baselines,
    tab_switch_baselines,
    centered_overlay_baselines
);
criterion_main!(benches);
