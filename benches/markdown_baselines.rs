use clay::{
    editor::{EditorCommand, EditorSurface},
    packages::{
        modes::{DocumentClassificationInput, ModeDeclaration, ModeRegistry},
        record::{PackageRecord, assemble_package_record},
    },
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentAccess,
        IncrementalParseUpdate, ParseByteRange, ParseUnit,
    },
    server::{decorations::validate_decoration_publication, parse_coordinator::ParseCoordinator},
};
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

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
    .map(
        |(byte_start, byte_end, style_token, priority)| DecorationSpan {
            byte_start,
            byte_end,
            kind: DecorationKind::Syntax,
            style_token: style_token.to_string(),
            priority,
            provenance: provenance.clone(),
        },
    )
    .collect();

    DecorationSet {
        document_id: 7,
        document_version,
        viewport_byte_start: 0,
        viewport_byte_end: viewport_end,
        spans,
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

criterion_group!(
    benches,
    markdown_activation_baselines,
    markdown_parse_and_decoration_baselines,
    markdown_decorated_editor_baselines
);
criterion_main!(benches);
