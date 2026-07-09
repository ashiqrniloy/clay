use clay::{
    editor::{EditorCommand, EditorSurface},
    packages::{
        modes::{DocumentClassificationInput, ModeDeclaration, ModeRegistry},
        record::{PackageRecord, assemble_package_record},
    },
    perf::budgets::{DECORATION_NEAR_VIEWPORT_GUARD_BYTES, SYNTAX_CACHE_BUDGET_BYTES},
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentAccess,
        IncrementalParseUpdate, ParseByteRange, ParsePolicy, ParseUnit, ParseWindowRequest,
        ParseWindowSnapshot, SyntaxMemoryBudget,
    },
    server::{decorations::validate_decoration_publication, parse_coordinator::ParseCoordinator},
};
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

fn markdown_package_record() -> PackageRecord {
    let text = std::fs::read_to_string("packages/markdown/package.json")
        .expect("packages/markdown/package.json must exist");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("package.json must be valid JSON");
    assemble_package_record(&value).expect("@clay/markdown package contract must validate")
}

fn register_markdown_mode(record: &PackageRecord) -> ModeRegistry {
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(
            &record.manifest,
            ModeDeclaration {
                package_name: record.manifest.name.clone(),
                package_version: record.manifest.version.clone(),
                api_prefix: record.manifest.clay.api_prefix.clone(),
                mode_id: "markdown".to_string(),
                display_name: "Markdown".to_string(),
                extensions: vec![
                    "md".to_string(),
                    "markdown".to_string(),
                    "mdown".to_string(),
                ],
                mime_types: vec!["text/markdown".to_string()],
                file_names: vec![],
                file_name_patterns: vec![],
                shebang_patterns: vec![],
                content_probes: vec![],
            },
        )
        .expect("markdown mode pattern must register");
    registry
}

fn markdown_text(repetitions: usize) -> String {
    let mut text = String::new();
    for index in 0..repetitions {
        text.push_str(&format!(
            "# Heading {index}\n\nThis **strong** paragraph has _emphasis_, `inline code`, and Hé 🦀.\n\n- first item\n- second item\n\n```rust\nfn main() {{}}\n```\n\n"
        ));
    }
    text
}

fn markdown_text_for_bytes(target_bytes: usize) -> String {
    const BLOCK: &str = "# Windowed heading\n\nThis **strong** paragraph has _emphasis_ and `inline code`.\n\n- first item\n- second item\n\n```rust\nfn main() {}\n```\n\n";
    let mut text = String::with_capacity(target_bytes.saturating_add(BLOCK.len()));
    while text.len() < target_bytes {
        text.push_str(BLOCK);
    }
    text.truncate(target_bytes);
    text
}

fn large_file_sizes() -> [(&'static str, usize); 5] {
    [
        ("64KiB", 64 * 1024),
        ("256KiB", 256 * 1024),
        ("1MiB", 1024 * 1024),
        ("5MiB", 5 * 1024 * 1024),
        ("16MiB", 16 * 1024 * 1024),
    ]
}

fn markdown_decoration_set(document_version: u64, viewport_end: u64) -> DecorationSet {
    let provenance = DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "markdown".to_string(),
    };
    let spans = vec![
        (0, 11, "markup.heading.1", 90),
        (28, 38, "markup.strong", 60),
        (53, 63, "markup.emphasis", 50),
        (65, 78, "markup.inline-code", 65),
        (91, 92, "markup.list-marker", 80),
        (104, 105, "markup.list-marker", 80),
        (118, 144, "markup.code-block", 70),
    ]
    .into_iter()
    .map(|(byte_start, byte_end, style_token, priority)| {
        DecorationSpan::from_style_token(
            byte_start,
            byte_end,
            DecorationKind::Syntax,
            style_token,
            priority,
            provenance.clone(),
        )
    })
    .collect();

    DecorationSet {
        document_id: 7,
        document_version,
        viewport_byte_start: 0,
        viewport_byte_end: viewport_end,
        spans,
    }
}

fn large_file_decoration_set(document_version: u64, viewport_start: u64) -> DecorationSet {
    let mut set = markdown_decoration_set(
        document_version,
        viewport_start + DECORATION_NEAR_VIEWPORT_GUARD_BYTES,
    );
    set.viewport_byte_start = viewport_start;
    for span in &mut set.spans {
        span.byte_start += viewport_start;
        span.byte_end += viewport_start;
    }
    set
}

fn parse_window_request_for_size(document_bytes: usize) -> ParseWindowRequest {
    let viewport_start = (document_bytes as u64 / 2).saturating_sub(8 * 1024);
    ParseWindowRequest {
        document_id: 7,
        document_version: 3,
        behavior_version: 3,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        requested_ranges: vec![ParseByteRange::new(
            viewport_start,
            viewport_start + 16 * 1024,
        )],
        viewport: ParseByteRange::new(viewport_start, viewport_start + 16 * 1024),
        policy: ParsePolicy::new(64 * 1024, 4 * 1024, SYNTAX_CACHE_BUDGET_BYTES as u64, 50),
    }
}

fn parse_window_snapshot_for_size(document_bytes: usize) -> ParseWindowSnapshot {
    let text = markdown_text_for_bytes(64 * 1024);
    let byte_start = (document_bytes as u64 / 2).saturating_sub(32 * 1024);
    ParseWindowSnapshot {
        document_id: 7,
        document_version: 3,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        byte_start,
        byte_end: byte_start + text.len() as u64,
        base_line: 0,
        text,
    }
}

fn markdown_parse_update(document_version: u64) -> IncrementalParseUpdate {
    IncrementalParseUpdate {
        document_id: 7,
        document_version,
        behavior_version: 3,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        parse_unit: ParseUnit::LineGroup,
        viewport: ParseByteRange::new(0, 160),
        invalidated_ranges: vec![ParseByteRange::new(0, 80)],
        syntax_tree_delta: Some("decorations:viewport-spans=5".to_string()),
        decoration_update: Some(markdown_decoration_set(document_version, 160)),
    }
}

fn markdown_activation_baselines(c: &mut Criterion) {
    let record = markdown_package_record();
    let mut group = c.benchmark_group("markdown_activation_baselines");
    group.bench_function("classify_activate_select_manifest", |b| {
        b.iter(|| {
            let mut registry = register_markdown_mode(&record);
            let classification = registry
                .classify(&DocumentClassificationInput {
                    document_id: 7,
                    path: Some("notes.md".to_string()),
                    mime_type: None,
                    shebang: None,
                    leading_content: None,
                })
                .expect("markdown document must classify");
            let activation = registry
                .activate_major_mode(&record.manifest, classification)
                .expect("markdown mode must activate");
            let enabled = vec![&record];
            let selection = registry
                .select_behavior_manifest_for_document(7, &enabled)
                .expect("manifest selection must succeed");
            black_box(activation.behavior_version as usize + selection.manifest.commands.len())
        })
    });
}

fn markdown_parse_and_decoration_baselines(c: &mut Criterion) {
    let record = markdown_package_record();
    let mut group = c.benchmark_group("markdown_parse_and_decoration_baselines");
    group.bench_function("validate_representative_decoration_update", |b| {
        b.iter(|| {
            let validated = validate_decoration_publication(
                &record,
                3,
                black_box(markdown_decoration_set(3, 160)),
            )
            .expect("representative Markdown decorations must validate");
            black_box(validated.spans.len())
        })
    });
    group.bench_function("validate_representative_parse_update", |b| {
        b.iter(|| {
            let coordinator = ParseCoordinator::new();
            coordinator
                .validate_update(black_box(&markdown_parse_update(3)))
                .expect("representative Markdown parse update must validate");
            black_box(1usize)
        })
    });
}

fn markdown_decorated_editor_baselines(c: &mut Criterion) {
    let text = markdown_text(64);
    let mut group = c.benchmark_group("markdown_decorated_editor_baselines");
    for repetitions in [8usize, 64usize] {
        group.bench_with_input(
            BenchmarkId::from_parameter(repetitions),
            &repetitions,
            |b, &repetitions| {
                b.iter(|| {
                    let text = if repetitions == 64 {
                        text.clone()
                    } else {
                        markdown_text(repetitions)
                    };
                    let mut surface = EditorSurface::default();
                    surface.load_snapshot(7, 3, text, DocumentAccess::Editable { lease_id: 1 });
                    let _ = surface.apply_decoration_set(markdown_decoration_set(3, 160));
                    let _ = surface.command(EditorCommand::MoveDown);
                    let _ = surface.command(EditorCommand::MoveUp);
                    let _ = surface.command(EditorCommand::SelectRight);
                    black_box(surface.visible_text().len() + surface.decoration_span_count())
                })
            },
        );
    }
}

fn markdown_large_file_windowed_baselines(c: &mut Criterion) {
    let record = markdown_package_record();
    let mut group = c.benchmark_group("markdown_large_file_windowed_baselines");
    for (label, document_bytes) in large_file_sizes() {
        group.bench_with_input(
            BenchmarkId::new("parse_window_request_metadata", label),
            &document_bytes,
            |b, &document_bytes| {
                b.iter(|| {
                    let request = parse_window_request_for_size(document_bytes);
                    let snapshot = parse_window_snapshot_for_size(document_bytes);
                    let budget = SyntaxMemoryBudget::new(
                        request.policy.memory_budget_bytes,
                        snapshot.text.len() as u64,
                    );
                    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request)
                        .expect("parse window request serializes")
                        .len();
                    black_box(bytes + budget.remaining_bytes() as usize + snapshot.text.len())
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("visible_decoration_chunk_validation", label),
            &document_bytes,
            |b, &document_bytes| {
                b.iter(|| {
                    let viewport_start = (document_bytes as u64 / 2).saturating_sub(8 * 1024);
                    let validated = validate_decoration_publication(
                        &record,
                        3,
                        black_box(large_file_decoration_set(3, viewport_start)),
                    )
                    .expect("large-file visible Markdown decoration chunk validates");
                    black_box(validated.spans.len())
                })
            },
        );
    }
}

fn markdown_large_file_visible_render_baselines(c: &mut Criterion) {
    let document_bytes = 16 * 1024 * 1024;
    let text = markdown_text_for_bytes(document_bytes);
    let mut group = c.benchmark_group("markdown_large_file_visible_render_baselines");
    group.bench_function("render_adjacent_16m_windowed_chunk", |b| {
        b.iter_batched(
            || {
                let mut surface = EditorSurface::default();
                surface.load_snapshot(7, 3, text.clone(), DocumentAccess::Editable { lease_id: 1 });
                let _ = surface.update_visible_line_count_for_height(48.0 * 2.0 + 12.0 * 28.0);
                let _ = surface.apply_decoration_set(large_file_decoration_set(3, 0));
                surface
            },
            |mut surface| {
                let _ = surface.command(EditorCommand::MoveDown);
                let _ = surface.command(EditorCommand::MoveUp);
                let _ = surface.command(EditorCommand::SelectRight);
                black_box(surface.visible_text().len() + surface.decoration_span_count())
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(
    benches,
    markdown_activation_baselines,
    markdown_parse_and_decoration_baselines,
    markdown_decorated_editor_baselines,
    markdown_large_file_windowed_baselines,
    markdown_large_file_visible_render_baselines
);
criterion_main!(benches);
