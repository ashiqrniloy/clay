use std::hint::black_box;

use clay::{
    editor::{EditorCommand, EditorSurface},
    protocol::{DocumentAccess, ParseByteRange, ParseEditNotification, ParseWindowSnapshot},
    server::syntax::{SyntaxGrammarRegistry, TreeSitterSyntaxHandler},
};
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use tree_sitter::Language;

struct LanguageFixture {
    label: &'static str,
    package: &'static str,
    contribution_id: &'static str,
    language: fn() -> Language,
    query: &'static str,
    text: &'static str,
}

fn fixtures() -> [LanguageFixture; 5] {
    [
        LanguageFixture {
            label: "rust",
            package: "rust",
            contribution_id: "rust.rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            query: include_str!("../packages/rust/queries/highlights.scm"),
            text: include_str!("../tests/fixtures/syntax/rust.rs"),
        },
        LanguageFixture {
            label: "typescript",
            package: "typescript",
            contribution_id: "typescript.typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            query: include_str!("../packages/typescript/queries/highlights.scm"),
            text: include_str!("../tests/fixtures/syntax/typescript.ts"),
        },
        LanguageFixture {
            label: "tsx",
            package: "typescript",
            contribution_id: "typescript.tsx",
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
            query: include_str!("../packages/typescript/queries/highlights.scm"),
            text: include_str!("../tests/fixtures/syntax/typescript.tsx"),
        },
        LanguageFixture {
            label: "javascript",
            package: "javascript",
            contribution_id: "javascript.javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            query: include_str!("../packages/javascript/queries/highlights.scm"),
            text: include_str!("../tests/fixtures/syntax/javascript.js"),
        },
        LanguageFixture {
            label: "markdown",
            package: "markdown",
            contribution_id: "markdown.markdown",
            language: || tree_sitter_md_025::LANGUAGE.into(),
            query: include_str!("../packages/markdown/queries/highlights.scm"),
            text: include_str!("../tests/fixtures/syntax/markdown.md"),
        },
    ]
}

fn handler(fixture: &LanguageFixture) -> TreeSitterSyntaxHandler {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry
        .get(fixture.contribution_id)
        .unwrap_or_else(|| panic!("registered {}", fixture.contribution_id))
        .clone();
    TreeSitterSyntaxHandler::new(contribution, (fixture.language)(), fixture.query)
        .unwrap_or_else(|error| panic!("compile {} query: {error}", fixture.label))
}

fn notification(
    fixture: &LanguageFixture,
    document_id: u64,
    version: u64,
) -> ParseEditNotification {
    notification_with_text(fixture, document_id, version, fixture.text)
}

fn notification_with_text(
    fixture: &LanguageFixture,
    document_id: u64,
    version: u64,
    text: &str,
) -> ParseEditNotification {
    ParseEditNotification {
        document_id,
        document_version: version,
        behavior_version: 1,
        package_prefix: fixture.package.to_string(),
        mode_id: fixture.package.to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        parse_windows: vec![ParseWindowSnapshot {
            document_id,
            document_version: version,
            package_prefix: fixture.package.to_string(),
            mode_id: fixture.package.to_string(),
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            text: text.to_string(),
        }],
        memory_budget: None,
    }
}

fn first_party_open_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_party_open_parse");
    for (index, fixture) in fixtures().iter().enumerate() {
        let handler = handler(fixture);
        group.bench_with_input(
            BenchmarkId::from_parameter(fixture.label),
            fixture,
            |b, fixture| {
                let mut document_id = index as u64 * 1_000;
                b.iter(|| {
                    document_id += 1;
                    let update = handler
                        .parse_sync(notification(fixture, document_id, 1))
                        .expect("fixture open parse succeeds");
                    black_box(update.decoration_update.map_or(0, |set| set.spans.len()))
                })
            },
        );
    }
    group.finish();
}

fn first_party_incremental_edit(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_party_incremental_edit");
    for fixture in &fixtures() {
        let handler = handler(fixture);
        handler
            .parse_sync(notification(fixture, 7, 1))
            .expect("initial fixture parse succeeds");
        let mut version = 1;
        group.bench_with_input(
            BenchmarkId::from_parameter(fixture.label),
            fixture,
            |b, fixture| {
                b.iter(|| {
                    version += 1;
                    let edited = if version % 2 == 0 {
                        format!("{} ", fixture.text)
                    } else {
                        fixture.text.to_string()
                    };
                    let update = handler
                        .parse_sync(notification_with_text(fixture, 7, version, &edited))
                        .expect("incremental fixture parse succeeds");
                    black_box(update.decoration_update.map_or(0, |set| set.spans.len()))
                })
            },
        );
    }
    group.finish();
}

fn first_party_decorated_scroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_party_decorated_scroll");
    for fixture in &fixtures() {
        let decorations = handler(fixture)
            .parse_sync(notification(fixture, 7, 1))
            .expect("fixture parse succeeds")
            .decoration_update
            .expect("fixture emits decorations");
        group.bench_with_input(
            BenchmarkId::from_parameter(fixture.label),
            fixture,
            |b, fixture| {
                b.iter_batched(
                    || {
                        let mut surface = EditorSurface::default();
                        surface.load_snapshot(
                            7,
                            1,
                            fixture.text.repeat(32),
                            DocumentAccess::Editable { lease_id: 1 },
                        );
                        let _ = surface.update_visible_line_count_for_height(432.0);
                        let _ = surface.apply_decoration_set(decorations.clone());
                        surface
                    },
                    |mut surface| {
                        let _ = surface.scroll_lines(8);
                        let _ = surface.command(EditorCommand::MoveDown);
                        black_box(surface.visible_text().len() + surface.decoration_span_count())
                    },
                    BatchSize::LargeInput,
                )
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    first_party_open_parse,
    first_party_incremental_edit,
    first_party_decorated_scroll
);
criterion_main!(benches);
