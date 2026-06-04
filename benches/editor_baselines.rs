use clay::perf::baselines::{
    LARGE_BENCH_BYTES, SMALL_BENCH_BYTES, editor_insert_at_end, editor_render_adjacent_update,
    editor_resize_viewport_visible_text_len, editor_scroll_visible_text_len,
    editor_scroll_window_signature, editor_visible_text_len,
};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn editor_visible_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_visible_extraction");
    for bytes in [SMALL_BENCH_BYTES, LARGE_BENCH_BYTES] {
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(bytes), &bytes, |b, &bytes| {
            b.iter_batched(|| bytes, editor_visible_text_len, BatchSize::LargeInput);
        });
    }
    group.finish();
}

fn editor_editing(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_editing");
    for bytes in [SMALL_BENCH_BYTES, LARGE_BENCH_BYTES] {
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("insert_at_end", bytes),
            &bytes,
            |b, &bytes| {
                b.iter_batched(|| bytes, editor_insert_at_end, BatchSize::LargeInput);
            },
        );
    }
    group.finish();
}

fn editor_scroll_viewport(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_scroll_viewport");
    for bytes in [SMALL_BENCH_BYTES, LARGE_BENCH_BYTES] {
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("scroll_visible_text", bytes),
            &bytes,
            |b, &bytes| {
                b.iter_batched(
                    || bytes,
                    editor_scroll_visible_text_len,
                    BatchSize::LargeInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("scroll_window_signature", bytes),
            &bytes,
            |b, &bytes| {
                b.iter_batched(
                    || bytes,
                    |size| editor_scroll_window_signature(size, 2_048),
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn editor_layout_viewport_bounds(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_layout_viewport_bounds");
    for bytes in [SMALL_BENCH_BYTES, LARGE_BENCH_BYTES] {
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("resize_viewport", bytes),
            &bytes,
            |b, &bytes| {
                b.iter_batched(
                    || bytes,
                    |size| editor_resize_viewport_visible_text_len(size, 1080.0),
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn editor_render_adjacent(c: &mut Criterion) {
    let mut group = c.benchmark_group("editor_render_adjacent");
    for bytes in [SMALL_BENCH_BYTES, LARGE_BENCH_BYTES] {
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("selection_and_caret_updates", bytes),
            &bytes,
            |b, &bytes| {
                b.iter_batched(
                    || bytes,
                    editor_render_adjacent_update,
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    editor_visible_extraction,
    editor_editing,
    editor_scroll_viewport,
    editor_layout_viewport_bounds,
    editor_render_adjacent
);
criterion_main!(benches);
