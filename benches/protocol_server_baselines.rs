use clay::perf::baselines::{
    SMALL_BENCH_BYTES, client_enqueue_edit_batch, encode_decode_client_edit,
    encode_decode_initial_document, protocol_hello_roundtrip,
};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn protocol_codec_payloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_codec_payloads");
    group.bench_function("hello_roundtrip", |b| b.iter(protocol_hello_roundtrip));

    for bytes in [16usize, 1024, 16 * 1024] {
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::new("client_edit", bytes),
            &bytes,
            |b, &bytes| {
                b.iter_batched(|| bytes, encode_decode_client_edit, BatchSize::SmallInput);
            },
        );
    }

    group.throughput(Throughput::Bytes(SMALL_BENCH_BYTES as u64));
    group.bench_function("initial_document_64k", |b| {
        b.iter_batched(
            || SMALL_BENCH_BYTES,
            encode_decode_initial_document,
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn client_edit_queue_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("client_edit_queue_pressure");
    for edits in [1usize, 64, 256] {
        group.throughput(Throughput::Elements(edits as u64));
        group.bench_with_input(BenchmarkId::from_parameter(edits), &edits, |b, &edits| {
            b.iter_batched(|| edits, client_enqueue_edit_batch, BatchSize::SmallInput);
        });
    }
    group.finish();
}

#[cfg(any(unix, windows))]
fn server_document_acknowledgements(c: &mut Criterion) {
    let mut group = c.benchmark_group("server_document_acknowledgements");
    for edits in [1usize, 16, 128] {
        group.throughput(Throughput::Elements(edits as u64));
        group.bench_with_input(BenchmarkId::from_parameter(edits), &edits, |b, &edits| {
            b.iter_batched(
                || edits,
                clay::perf::baselines::server_apply_edit_ack_count,
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

#[cfg(any(unix, windows))]
fn server_stale_edit_rejections(c: &mut Criterion) {
    let mut group = c.benchmark_group("server_stale_edit_rejections");
    for edits in [1usize, 16, 128] {
        group.throughput(Throughput::Elements(edits as u64));
        group.bench_with_input(BenchmarkId::from_parameter(edits), &edits, |b, &edits| {
            b.iter_batched(
                || edits,
                clay::perf::baselines::server_rejects_stale_edit_count,
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

#[cfg(any(unix, windows))]
criterion_group!(
    benches,
    protocol_codec_payloads,
    client_edit_queue_pressure,
    server_document_acknowledgements,
    server_stale_edit_rejections
);
#[cfg(not(any(unix, windows)))]
criterion_group!(benches, protocol_codec_payloads, client_edit_queue_pressure);
criterion_main!(benches);
