use super::*;
use crate::{
    perf::metrics::PerfRecorder,
    protocol::{ParseByteRange, ParseWindowSnapshot},
};

fn notification(version: u64, text: &str) -> ParseEditNotification {
    ParseEditNotification {
        document_id: 7,
        document_version: version,
        behavior_version: 1,
        package_prefix: "rust".to_string(),
        mode_id: "rust.rust".to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        accepted_edit: (version == 2).then_some(crate::protocol::ParseInputEdit {
            base_document_version: 1,
            document_version: 2,
            start_byte: 0,
            old_end_byte: 13,
            new_end_byte: text.len() as u64,
            start_position: crate::protocol::ParsePoint::new(0, 0),
            old_end_position: crate::protocol::ParsePoint::new(1, 0),
            new_end_position: crate::protocol::ParsePoint::new(1, 0),
        }),
        parse_windows: vec![ParseWindowSnapshot {
            document_id: 7,
            document_version: version,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            window_id: 0,
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            base_column: 0,
            incremental_edit: version == 2,
            text: text.to_string(),
        }],
        memory_budget: None,
        trace_id: None,
        request_id: None,
    }
}

#[test]
fn split_decoration_payloads_claim_only_their_span_range() {
    let text = "fn item() {}\n".repeat(400);
    let notification = notification(1, &text);
    let provenance = crate::protocol::DecorationProvenance {
        package_name: "@clay/rust".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "rust".to_string(),
    };
    let spans = (0..200)
        .map(|index| crate::protocol::DecorationSpan {
            byte_start: (index * 12) as u64,
            byte_end: (index * 12 + 2) as u64,
            kind: DecorationKind::Syntax,
            token_type: TokenType::Keyword,
            modifiers: crate::protocol::Modifiers::NONE,
            scope: None,
            font_role: None,
            priority: 70,
            provenance: provenance.clone(),
            target: None,
            inlay: None,
        })
        .collect();
    let set = DecorationSet {
        document_id: 7,
        document_version: 1,
        package_prefix: "rust".to_string(),
        kind: DecorationKind::Syntax,
        viewport_byte_start: 0,
        viewport_byte_end: text.len() as u64,
        spans,
        trace_id: None,
    };
    let parts = split_decoration_set_to_update_budget(&notification, set);
    assert!(
        parts.len() > 1,
        "dense set must split under the update budget"
    );
    for part in &parts {
        let start = part
            .spans
            .iter()
            .map(|span| span.byte_start)
            .min()
            .expect("split keeps spans");
        let end = part
            .spans
            .iter()
            .map(|span| span.byte_end)
            .max()
            .expect("split keeps spans");
        assert_eq!(part.viewport_byte_start, start);
        assert_eq!(part.viewport_byte_end, end);
        assert!(end <= text.len() as u64);
    }
}

// Regression: code grammars parse full-file context so a viewport
// landing mid-expression no longer collapses into a recovery ERROR node
// (white text) or invents tokens (e.g. bogus string spans).
#[test]
fn code_grammars_parse_full_file_context_for_viewport_output() {
    for descriptor in FIRST_PARTY_NATIVE_GRAMMARS {
        assert_eq!(
            descriptor.max_window_bytes, NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
            "{}",
            descriptor.id
        );
    }

    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "rust.rust")
        .unwrap();
    let handler = native_handler(&contribution_from_native_descriptor(descriptor))
        .unwrap()
        .unwrap();
    // Large source with a long expression; viewport lands mid-expression.
    let text = format!(
        "fn head() {{}}\nlet value = RuntimeOptions {{\n{}\n    ..Default::default()\n}};\nfn tail() -> usize {{ 42 }}\n",
        (0..120)
            .map(|index| format!("    field_{index}: Some(\"value_{index}\".to_string()),"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(text.len() > 4 * 1024);
    let viewport = ParseByteRange::new(1_024, 3_072);
    let scroll_notification = |viewport: ParseByteRange| ParseEditNotification {
        document_id: 7,
        document_version: 1,
        behavior_version: 1,
        package_prefix: "rust".to_string(),
        mode_id: "rust.rust".to_string(),
        viewport,
        invalidated_ranges: vec![viewport],
        accepted_edit: None,
        parse_windows: vec![ParseWindowSnapshot {
            document_id: 7,
            document_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            window_id: 0,
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            base_column: 0,
            incremental_edit: false,
            text: text.clone(),
        }],
        memory_budget: None,
        trace_id: None,
        request_id: None,
    };

    let update = handler
        .parse_sync(scroll_notification(viewport))
        .expect("full-file window parses");
    let string_spans = update
        .decoration_updates
        .iter()
        .flat_map(|set| set.spans.iter())
        .filter(|span| span.token_type == TokenType::String)
        .count();
    assert!(
        string_spans > 8,
        "mid-expression viewport keeps string highlights: {string_spans}"
    );
    assert!(
        update
            .decoration_updates
            .iter()
            .flat_map(|set| set.spans.iter())
            .all(|span| span.byte_start >= viewport.start && span.byte_end <= viewport.end),
        "decoration output stays viewport-limited"
    );

    // Scrolling the same version reuses the cached full-file tree.
    let scrolled = handler
        .parse_sync(scroll_notification(ParseByteRange::new(2_048, 4_096)))
        .expect("scroll reuses cached tree");
    assert_eq!(
        scrolled.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:cached")
    );
    assert!(!scrolled.decoration_updates.is_empty());
}

#[test]
fn markdown_native_descriptor_enables_inline_injection() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "markdown.markdown")
        .expect("Markdown descriptor");
    assert!(descriptor.injections_query.is_some());

    let contribution = contribution_from_native_descriptor(descriptor);
    assert_eq!(
        contribution.injections_query_path.as_deref(),
        Some("packages/markdown/queries/injections.scm")
    );
    let handler = native_handler(&contribution)
        .expect("native handler builds")
        .expect("markdown descriptor resolves");
    assert!(handler.injections.is_some());

    // Non-composite grammars stay single-language.
    let rust = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "rust.rust")
        .expect("Rust descriptor");
    let handler = native_handler(&contribution_from_native_descriptor(rust))
        .expect("native handler builds")
        .expect("rust descriptor resolves");
    assert!(handler.injections.is_none());
}

/// Plan 099: the per-document tree/parser cache is bounded — cold
/// documents are evicted beyond `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES`
/// entries, and oversize windows still fail before parsing.
#[test]
fn document_tree_cache_is_bounded_and_windows_respect_byte_budget() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "markdown.markdown")
        .unwrap();
    let handler = native_handler(&contribution_from_native_descriptor(descriptor))
        .unwrap()
        .unwrap();
    let text = "# Heading\n\nProse.\n";

    // Oversize window fails closed before any parse state is built.
    let mut oversize = notification(1, text);
    oversize.package_prefix = "markdown".to_string();
    oversize.mode_id = "markdown.markdown".to_string();
    let huge_window_bytes = handler.contribution.max_window_bytes() + 1;
    oversize.parse_windows[0].text = "x".repeat(huge_window_bytes);
    assert!(matches!(
        handler.parse_sync(oversize),
        Err(TreeSitterSyntaxError::WindowTooLarge { .. })
    ));

    // Fill the cache beyond its bound with distinct documents.
    let documents = crate::perf::budgets::SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES + 8;
    for document_id in 1..=documents as u64 {
        let mut doc_notification = notification(1, text);
        doc_notification.package_prefix = "markdown".to_string();
        doc_notification.mode_id = "markdown.markdown".to_string();
        doc_notification.document_id = document_id;
        doc_notification.parse_windows[0].document_id = document_id;
        handler.parse_sync(doc_notification).unwrap();
    }
    let trees = handler
        .trees
        .lock()
        .expect("syntax tree cache lock poisoned");
    assert!(
        trees.len() <= crate::perf::budgets::SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES,
        "bounded tree cache must not retain every cold document, got {}",
        trees.len()
    );
    // The most recent document survives; eviction is arbitrary beyond
    // the bound (documented ceiling).
    assert_eq!(
        trees
            .get(&(documents as u64))
            .map(|state| state.document_version),
        Some(1)
    );
}

#[test]
fn same_version_markdown_scroll_reuses_full_document_tree_context() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "markdown.markdown")
        .unwrap();
    let mut handler = native_handler(&contribution_from_native_descriptor(descriptor))
        .unwrap()
        .unwrap();
    let perf = PerfRecorder::for_test(true);
    handler.perf = perf.clone();
    let text = format!(
        "```text\n{}LAST CODE LINE\n```\n\nPlain prose after fence.\n",
        "code inside fence\n".repeat(300)
    );
    let scroll_start = text.find("LAST CODE LINE").unwrap() as u64;
    let prose = text.find("Plain prose after fence.").unwrap() as u64;

    let mut initial = notification(1, &text);
    initial.package_prefix = "markdown".to_string();
    initial.mode_id = "markdown.markdown".to_string();
    initial.viewport = ParseByteRange::new(0, 4_096);
    initial.invalidated_ranges = vec![initial.viewport];
    initial.parse_windows[0].package_prefix = initial.package_prefix.clone();
    initial.parse_windows[0].mode_id = initial.mode_id.clone();
    handler.parse_sync(initial).unwrap();

    let mut scrolled = notification(1, &text);
    scrolled.package_prefix = "markdown".to_string();
    scrolled.mode_id = "markdown.markdown".to_string();
    scrolled.viewport = ParseByteRange::new(scroll_start, text.len() as u64);
    scrolled.invalidated_ranges = vec![scrolled.viewport];
    scrolled.parse_windows[0].package_prefix = scrolled.package_prefix.clone();
    scrolled.parse_windows[0].mode_id = scrolled.mode_id.clone();
    let update = handler.parse_sync(scrolled).unwrap();

    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some("tree-sitter:markdown:cached")
    );
    assert_eq!(
        perf.snapshots()
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INVOCATIONS)
            .count(),
        1
    );
    assert!(
        update.decoration_updates.iter().any(|set| set
            .spans
            .iter()
            .any(|span| span.token_type == TokenType::Paragraph
                && span.byte_start <= prose
                && span.byte_end > prose))
    );
    assert!(
        !update.decoration_updates.iter().any(|set| set
            .spans
            .iter()
            .any(|span| span.token_type == TokenType::CodeBlock
                && span.byte_start <= prose
                && span.byte_end > prose))
    );
}

#[test]
fn query_ranges_merge_and_expand_utf8_safe_empty_invalidations() {
    let text = "aéz";

    let ranges = normalize_query_ranges([0..1, 1..1, 1..3, 3..3], text, 0..text.len());

    assert_eq!(ranges, vec![0..4]);
    assert!(
        ranges.iter().all(|range| {
            text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
        })
    );
}

#[test]
fn replacement_ranges_move_shared_chunk_boundaries_past_utf8_scalars() {
    let text = format!("{}é{}", "a".repeat(127), "b".repeat(130));

    let affected = 127..130;
    let ranges = replacement_ranges(std::slice::from_ref(&affected), &text);

    assert_eq!(ranges, vec![0..129, 129..256]);
    assert!(
        ranges.iter().all(|range| {
            text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
        })
    );
}

#[test]
fn incremental_parse_queries_less_than_unchanged_window() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "rust.rust")
        .expect("Rust descriptor");
    let perf = PerfRecorder::for_test(true);
    let mut handler = TreeSitterSyntaxHandler::new(
        contribution_from_native_descriptor(descriptor),
        (descriptor.language)(),
        descriptor.highlights_query,
    )
    .expect("Rust handler");
    handler.perf = perf.clone();
    let suffix = " let distant = 2;".repeat(20);
    let old_text = format!("fn main() {{ le value = 1;{suffix} }}\n");
    let new_text = format!("fn main() {{ let value = 1;{suffix} }}\n");
    let insertion = old_text.find("le value").expect("partial keyword") + 2;

    handler
        .parse_sync(notification(1, &old_text))
        .expect("full parse");
    let mut incremental = notification(2, &new_text);
    incremental.invalidated_ranges =
        vec![ParseByteRange::new(insertion as u64, insertion as u64 + 1)];
    incremental.accepted_edit = Some(crate::protocol::ParseInputEdit {
        base_document_version: 1,
        document_version: 2,
        start_byte: insertion as u64,
        old_end_byte: insertion as u64,
        new_end_byte: insertion as u64 + 1,
        start_position: crate::protocol::ParsePoint::new(0, insertion as u64),
        old_end_position: crate::protocol::ParsePoint::new(0, insertion as u64),
        new_end_position: crate::protocol::ParsePoint::new(0, insertion as u64 + 1),
    });
    handler.parse_sync(incremental).expect("incremental parse");

    let queried_bytes = perf.snapshots().into_iter().find_map(|snapshot| {
        (snapshot.name == SYNTAX_QUERY_BYTES && snapshot.metadata.version == Some(2))
            .then_some(snapshot.value)
    });
    assert!(matches!(
        queried_bytes,
        Some(MetricValue::Bytes { bytes }) if bytes <= SYNTAX_DECORATION_CHUNK_BYTES as u64
            && bytes < new_text.len() as u64
    ));
}

#[test]
fn decoration_member_count_does_not_multiply_parse_or_query_invocations() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "rust.rust")
        .expect("Rust descriptor");
    let perf = PerfRecorder::for_test(true);
    let mut handler = TreeSitterSyntaxHandler::new(
        contribution_from_native_descriptor(descriptor),
        (descriptor.language)(),
        "(identifier) @keyword",
    )
    .expect("Rust handler");
    handler.perf = perf.clone();
    let text = (0..80)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");

    let update = handler
        .parse_sync(notification(1, &text))
        .expect("full parse");
    let snapshots = perf.snapshots();

    assert!(update.decoration_updates.len() > 1);
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INVOCATIONS)
            .count(),
        1
    );
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_QUERY_RANGES)
            .count(),
        1
    );
}

#[test]
fn first_party_continuity_edits_keep_one_bounded_parse_and_query() {
    for (id, old_text, new_text) in [
        ("rust.rust", "fn app() {}\n", "fn application() {}\n"),
        (
            "typescript.typescript",
            "function app() {}\n",
            "function application() {}\n",
        ),
        (
            "typescript.tsx",
            "function app() {}\n",
            "function application() {}\n",
        ),
        (
            "javascript.javascript",
            "function app() {}\n",
            "function application() {}\n",
        ),
        (
            "markdown.markdown",
            "Paragraph text.\n",
            "Paragraph texts.\n",
        ),
    ] {
        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == id)
            .unwrap_or_else(|| panic!("{id} descriptor"));
        let perf = PerfRecorder::for_test(true);
        let mut handler = TreeSitterSyntaxHandler::new(
            contribution_from_native_descriptor(descriptor),
            (descriptor.language)(),
            descriptor.highlights_query,
        )
        .unwrap_or_else(|error| panic!("{id} handler: {error}"));
        handler.perf = perf.clone();

        let prepare = |mut notification: ParseEditNotification| {
            notification.package_prefix = descriptor.package_prefix.to_string();
            notification.mode_id = descriptor.id.to_string();
            notification.parse_windows[0].package_prefix = descriptor.package_prefix.to_string();
            notification.parse_windows[0].mode_id = descriptor.id.to_string();
            notification
        };
        handler
            .parse_sync(prepare(notification(1, old_text)))
            .unwrap_or_else(|error| panic!("{id} initial parse: {error}"));

        let start = old_text
            .bytes()
            .zip(new_text.bytes())
            .position(|(old, new)| old != new)
            .unwrap_or_else(|| old_text.len().min(new_text.len()));
        let mut old_end = old_text.len();
        let mut new_end = new_text.len();
        while old_end > start
            && new_end > start
            && old_text.as_bytes()[old_end - 1] == new_text.as_bytes()[new_end - 1]
        {
            old_end -= 1;
            new_end -= 1;
        }
        let point = |text: &str, offset: usize| {
            let prefix = &text[..offset];
            crate::protocol::ParsePoint::new(
                prefix.bytes().filter(|byte| *byte == b'\n').count() as u64,
                prefix
                    .rsplit_once('\n')
                    .map_or(prefix.len(), |(_, tail)| tail.len()) as u64,
            )
        };
        let mut incremental = prepare(notification(2, new_text));
        incremental.invalidated_ranges = vec![ParseByteRange::new(start as u64, new_end as u64)];
        incremental.accepted_edit = Some(crate::protocol::ParseInputEdit {
            base_document_version: 1,
            document_version: 2,
            start_byte: start as u64,
            old_end_byte: old_end as u64,
            new_end_byte: new_end as u64,
            start_position: point(old_text, start),
            old_end_position: point(old_text, old_end),
            new_end_position: point(new_text, new_end),
        });
        incremental.parse_windows[0].incremental_edit = true;
        let update = handler
            .parse_sync(incremental)
            .unwrap_or_else(|error| panic!("{id} incremental parse: {error}"));
        let snapshots = perf.snapshots();
        let parse_count = snapshots
            .iter()
            .filter(|snapshot| {
                snapshot.name == SYNTAX_PARSE_INVOCATIONS && snapshot.metadata.version == Some(2)
            })
            .count();
        let query_bytes = snapshots
            .iter()
            .find_map(|snapshot| {
                (snapshot.name == SYNTAX_QUERY_BYTES && snapshot.metadata.version == Some(2))
                    .then_some(&snapshot.value)
            })
            .and_then(|value| match value {
                MetricValue::Bytes { bytes } => Some(*bytes),
                _ => None,
            })
            .expect("incremental query bytes");
        let query_count = snapshots
            .iter()
            .filter(|snapshot| {
                snapshot.name == SYNTAX_QUERY_RANGES && snapshot.metadata.version == Some(2)
            })
            .count();
        let members = update.decoration_updates.len();

        eprintln!(
            "{id}: parses={parse_count}, query_ranges={query_count}, query_bytes={query_bytes}, members={members}"
        );
        assert_eq!(parse_count, 1, "{id}: parser calls");
        assert_eq!(query_count, 1, "{id}: query calls");
        assert!(
            query_bytes <= SYNTAX_DECORATION_CHUNK_BYTES as u64,
            "{id}: query bytes {query_bytes}"
        );
        assert_eq!(members, 1, "{id}: touched replacement members");
    }
}

#[test]
fn native_parse_records_source_safe_work_classification_and_query_counts() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "rust.rust")
        .expect("Rust descriptor");
    let perf = PerfRecorder::for_test(true);
    let mut handler = TreeSitterSyntaxHandler::new(
        contribution_from_native_descriptor(descriptor),
        (descriptor.language)(),
        descriptor.highlights_query,
    )
    .expect("Rust handler");
    handler.perf = perf.clone();

    handler
        .parse_sync(notification(1, "fn main() {}\n"))
        .expect("full parse");
    handler
        .parse_sync(notification(2, "fn main() { let value = 1; }\n"))
        .expect("incremental parse");

    let snapshots = perf.snapshots();
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INVOCATIONS)
            .count(),
        2
    );
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_PARSE_FULL)
            .count(),
        1
    );
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INCREMENTAL)
            .count(),
        1
    );
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_QUERY_RANGES)
            .count(),
        2
    );
    assert!(snapshots.iter().all(|snapshot| {
        snapshot.metadata.document_id == Some(7)
            && snapshot.metadata.version.is_some()
            && snapshot.metadata.sanitized_path.is_none()
    }));
}

fn textobjects_rust_handler() -> TreeSitterSyntaxHandler {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "rust.rust")
        .expect("Rust descriptor");
    let mut handler = TreeSitterSyntaxHandler::new(
        contribution_from_native_descriptor(descriptor),
        (descriptor.language)(),
        descriptor.highlights_query,
    )
    .expect("Rust handler");
    handler
        .enable_textobjects(
            descriptor
                .textobjects_query
                .expect("rust ships textobjects"),
        )
        .expect("rust textobjects compile");
    handler
}

fn query_cursor(anchor: u64, focus: u64) -> SelectionQueryCursor {
    SelectionQueryCursor { anchor, focus }
}

#[test]
fn first_party_textobject_queries_compile() {
    // Plan 071 task 10: every shipped textobjects.scm must compile against
    // its grammar fail-closed, or the handler construction test fails here.
    for descriptor in FIRST_PARTY_NATIVE_GRAMMARS {
        let mut handler = TreeSitterSyntaxHandler::new(
            contribution_from_native_descriptor(descriptor),
            (descriptor.language)(),
            descriptor.highlights_query,
        )
        .unwrap_or_else(|error| panic!("{} handler failed: {error:?}", descriptor.id));
        if let Some(textobjects) = descriptor.textobjects_query {
            handler
                .enable_textobjects(textobjects)
                .unwrap_or_else(|error| panic!("{} textobjects failed: {error:?}", descriptor.id));
        }
    }
}

#[test]
fn first_party_highlight_captures_resolve_through_native_style_maps() {
    // Phase 26.2: every highlight capture must resolve through the closed
    // vocabulary map. Unmapped names are silently dropped at emit time.
    for descriptor in FIRST_PARTY_NATIVE_GRAMMARS {
        let query = Query::new(&(descriptor.language)(), descriptor.highlights_query)
            .unwrap_or_else(|error| panic!("{} highlights query failed: {error}", descriptor.id));
        let mapped: std::collections::HashSet<&str> = descriptor
            .style_map
            .iter()
            .map(|(name, ..)| *name)
            .collect();
        for name in query.capture_names() {
            assert!(
                mapped.contains(name),
                "{} capture @{name} missing from native style map",
                descriptor.id
            );
        }
    }
    for embedded in FIRST_PARTY_EMBEDDED_GRAMMARS {
        let query = Query::new(&(embedded.language)(), embedded.highlights_query)
            .unwrap_or_else(|error| panic!("{} highlights query failed: {error}", embedded.name));
        let mapped: std::collections::HashSet<&str> = MARKDOWN_NATIVE_STYLE_MAP
            .iter()
            .map(|(name, ..)| *name)
            .collect();
        for name in query.capture_names() {
            assert!(
                mapped.contains(name),
                "{} capture @{name} missing from markdown style map",
                embedded.name
            );
        }
    }
}

#[test]
fn rust_textobject_function_inner_around_and_directions() {
    let handler = textobjects_rust_handler();
    let text = "fn add(a: u32) -> u32 {\n    a + 1\n}\n\nfn sub() {}\n";
    let body_open = text.find('{').expect("body brace");
    let first_end = text.find("\n\n").expect("first function end");
    let second_start = text.find("fn sub").expect("second function");
    let caret = text.find("a + 1").expect("body expression") as u64;

    let run = |around: bool, direction: TextobjectDirection, focus: u64| {
        handler
            .selection_query_ranges(
                7,
                1,
                text,
                SelectionQuery::Textobject {
                    kind: TextobjectKind::Function,
                    around,
                    direction,
                },
                &[query_cursor(focus, focus)],
            )
            .expect("query runs")
    };

    // Inner at a caret inside the body: the block without the signature.
    let inner = run(false, TextobjectDirection::Current, caret);
    assert_eq!(
        inner[0],
        Some(SelectionQueryRange {
            start: body_open as u64,
            end: first_end as u64,
        })
    );
    // Around at the same caret covers the whole function.
    let around = run(true, TextobjectDirection::Current, caret);
    assert_eq!(
        around[0],
        Some(SelectionQueryRange {
            start: 0,
            end: first_end as u64,
        })
    );
    // Next from inside the first function jumps to the second.
    let next = run(true, TextobjectDirection::Next, caret);
    assert_eq!(
        next[0],
        Some(SelectionQueryRange {
            start: second_start as u64,
            end: text.len() as u64 - 1,
        })
    );
    // Previous from inside the second function returns the first.
    let inside_second = (second_start + 10) as u64;
    let previous = run(true, TextobjectDirection::Previous, inside_second);
    assert_eq!(
        previous[0],
        Some(SelectionQueryRange {
            start: 0,
            end: first_end as u64,
        })
    );
    // Misses stay None (no wrap, deny-by-default absence).
    let none = run(true, TextobjectDirection::Next, inside_second);
    assert_eq!(none[0], None);
}

#[test]
fn rust_textobject_comment_inner_falls_back_to_around() {
    let handler = textobjects_rust_handler();
    let text = "// note\nfn main() {}\n";
    let caret = 3u64;
    let ranges = handler
        .selection_query_ranges(
            7,
            1,
            text,
            SelectionQuery::Textobject {
                kind: TextobjectKind::Comment,
                around: false,
                direction: TextobjectDirection::Current,
            },
            &[query_cursor(caret, caret)],
        )
        .expect("query runs");
    assert_eq!(
        ranges[0],
        Some(SelectionQueryRange {
            start: 0,
            end: text.find('\n').expect("comment line end") as u64,
        })
    );
}

#[test]
fn smart_select_expand_walks_up_and_shrink_walks_down() {
    let handler = textobjects_rust_handler();
    let text = "fn add(a: u32) -> u32 {\n    a + 1\n}\n";
    let caret = text.find("a + 1").expect("body expression") as u64;
    let run = |action: SmartSelectAction, anchor: u64, focus: u64| {
        handler
            .selection_query_ranges(
                7,
                1,
                text,
                SelectionQuery::SmartSelect { action },
                &[query_cursor(anchor, focus)],
            )
            .expect("query runs")[0]
    };

    // Expand from a collapsed caret: strictly growing node ranges until
    // the whole document, then None.
    let mut anchor = caret;
    let mut focus = caret;
    let mut previous_len = 0usize;
    let mut steps = 0;
    while let Some(range) = run(SmartSelectAction::Expand, anchor, focus) {
        let len = (range.end - range.start) as usize;
        assert!(range.start <= anchor && focus <= range.end);
        assert!(len > previous_len, "expand must strictly grow");
        previous_len = len;
        anchor = range.start;
        focus = range.end;
        steps += 1;
        assert!(steps < 64, "expand must terminate");
    }
    assert_eq!(previous_len, text.len(), "last expand covers the document");

    // Shrink from the full document: a strict subrange; shrink again from
    // a collapsed selection is a no-op.
    let full = text.len() as u64;
    let shrunk = run(SmartSelectAction::Shrink, 0, full);
    let shrunk = shrunk.expect("full-document selection can shrink");
    assert!((shrunk.start, shrunk.end) != (0, full));
    assert!(shrunk.end - shrunk.start < full);
    assert_eq!(run(SmartSelectAction::Shrink, caret, caret), None);
}

#[test]
fn markdown_handler_without_textobjects_degrades_to_no_ranges() {
    let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == "markdown.markdown")
        .expect("Markdown descriptor");
    assert!(descriptor.textobjects_query.is_none());
    let handler = TreeSitterSyntaxHandler::new(
        contribution_from_native_descriptor(descriptor),
        (descriptor.language)(),
        descriptor.highlights_query,
    )
    .expect("Markdown handler");
    let text = "# Title\n\npara\n";
    let ranges = handler
        .selection_query_ranges(
            7,
            1,
            text,
            SelectionQuery::Textobject {
                kind: TextobjectKind::Function,
                around: true,
                direction: TextobjectDirection::Current,
            },
            &[query_cursor(2, 2)],
        )
        .expect("query runs");
    assert_eq!(ranges, vec![None]);
    // Smart select still works off the block tree.
    let expanded = handler
        .selection_query_ranges(
            7,
            1,
            text,
            SelectionQuery::SmartSelect {
                action: SmartSelectAction::Expand,
            },
            &[query_cursor(2, 2)],
        )
        .expect("query runs");
    assert!(expanded[0].is_some());
}
