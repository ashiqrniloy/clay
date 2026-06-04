use clay::perf::baselines::{
    apply_sdui_snapshot_and_update, behavior_manifest_route_count, encode_decode_sdui_snapshot,
};
use criterion::{Criterion, criterion_group, criterion_main};

fn runtime_configuration_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_configuration_baselines");
    group.bench_function("behavior_manifest_minimal_text_editing", |b| {
        b.iter(behavior_manifest_route_count)
    });
    group.finish();
}

fn sdui_application_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("sdui_application_baselines");
    group.bench_function("apply_snapshot_and_panel_update", |b| {
        b.iter(apply_sdui_snapshot_and_update)
    });
    group.bench_function("codec_snapshot_roundtrip", |b| {
        b.iter(encode_decode_sdui_snapshot)
    });
    group.finish();
}

criterion_group!(
    benches,
    runtime_configuration_baselines,
    sdui_application_baselines
);
criterion_main!(benches);
