use super::*;

#[test]
fn language_intelligence_window_uses_active_behavior_mode() {
    let mut manifest = BehaviorManifest::minimal_text_editing(3);
    manifest.manifest_id = "rust.rust".to_string();
    let behavior = ActiveBehaviorManifest::new(manifest).unwrap();
    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 3,
        cursor_byte_offset: 1,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 0,
    };

    let window = language_intelligence_document_window_for_behavior(
        &request,
        &document_with_text("fn main() {}"),
        behavior.manifest(),
    );

    assert_eq!(window.active_mode, "rust");
}

#[test]
fn language_intelligence_window_resolves_per_document_mode_layer() {
    // Phase 22.2: two documents in different modes; the window builder
    // must use each document's OWN layer's mode, not a connection-wide
    // latest.
    let mut state = ActiveBehaviorManifest::default();
    let mut markdown = BehaviorManifest::minimal_text_editing(1);
    markdown.manifest_id = "markdown.markdown".to_string();
    markdown.scope = crate::protocol::BehaviorScope::Document { document_id: 7 };
    let mut rust = BehaviorManifest::minimal_text_editing(1);
    rust.manifest_id = "rust.rust".to_string();
    rust.scope = crate::protocol::BehaviorScope::Document { document_id: 9 };
    state.publish_replacement(markdown).unwrap();
    state.publish_replacement(rust).unwrap();

    let request = |document_id| crate::protocol::LanguageIntelligenceRequest {
        request_id: 1,
        client_id: 2,
        document_id,
        document_version: 4,
        behavior_version: 3,
        cursor_byte_offset: 1,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 0,
    };

    let markdown_window = language_intelligence_document_window_for_behavior(
        &request(7),
        &document_with_text("## Heading"),
        state.manifest_for(7),
    );
    assert_eq!(markdown_window.active_mode, "markdown");

    let rust_window = language_intelligence_document_window_for_behavior(
        &request(9),
        &document_with_text("fn main() {}"),
        state.manifest_for(9),
    );
    assert_eq!(rust_window.active_mode, "rust");
}

/// Plan 126 task 3: the static completion path reads only the replacement
/// range, so a 4 MiB document produces exactly the small-document result.
#[test]
fn static_completion_on_large_document_matches_small_document_results() {
    let provenance = crate::protocol::CompletionProvenance {
        package_name: "@clay/javascript".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "javascript".to_string(),
    };
    let provider = crate::server::completion::CompletionProviderMeta {
        id: "javascript.keywords".to_string(),
        provenance: provenance.clone(),
        priority: 0,
        exclusive: false,
        trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
            trigger_characters: vec![".".to_string()],
        },
        word_boundary: crate::server::completion::WordBoundaryRule::default(),
        items: ["function", "for", "return"]
            .into_iter()
            .map(|item| crate::protocol::CompletionItem::new(item, item, provenance.clone()))
            .collect(),
        timeout_ms: 300,
        max_items: 32,
        generation: 0,
    };
    let request = crate::protocol::CompletionRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
        trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
        provider_generation: 0,
        recent_completions: Vec::<String>::new().into_boxed_slice(),
    };
    // Same two-byte replacement range at the head of both documents.
    let small = document_with_text("fu");
    let large = document_with_text(&format!("fu{}", "x".repeat(4 * 1024 * 1024)));
    let result_of = |document: &DocumentState| {
        let replacement_text = document
            .text_range(
                request.replacement_range.byte_start,
                request.replacement_range.byte_end,
            )
            .expect("replacement range is inside the document");
        static_package_completion_result(
            &request,
            "javascript.javascript",
            &replacement_text,
            std::slice::from_ref(&provider),
        )
        .expect("static provider matches the replacement range")
    };

    let small_result = result_of(&small);
    let large_result = result_of(&large);

    assert_eq!(small_result.status, large_result.status);
    assert_eq!(small_result.provenance, large_result.provenance);
    assert_eq!(large_result.items, small_result.items);
    assert_eq!(
        large_result
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["function"]
    );
    assert_eq!(
        large_result.replacement_range,
        small_result.replacement_range
    );
}

/// Plan 126 task 3: the language-intelligence window is built from the rope,
/// stays inside its budget, and lands on scalar boundaries on a multi-MiB
/// multibyte document.
#[test]
fn language_intelligence_window_budget_honored() {
    use crate::perf::budgets::LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES;

    let text = "日本語 🦀 fn main() {}\n".repeat(4 * 1024 * 1024 / 28);
    let document = document_with_text(&text);
    let cursor_byte_offset = (text.len() / 2) as u64;
    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 3,
        cursor_byte_offset,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 0,
    };

    let window = language_intelligence_document_window(&request, &document, "rust");

    assert!(
        window.text.len() <= LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES + 3,
        "window of {} bytes exceeds the {} byte budget",
        window.text.len(),
        LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES
    );
    assert_eq!(
        window.byte_end - window.byte_start,
        window.text.len() as u64
    );
    assert!(window.byte_start <= cursor_byte_offset);
    assert!(cursor_byte_offset <= window.byte_end);
    // Indexing panics unless both bounds are scalar boundaries; the equality
    // then proves the window is the document's own text, uncorrupted.
    assert_eq!(
        window.text,
        text[window.byte_start as usize..window.byte_end as usize]
    );
}

#[test]
fn static_package_completion_filters_active_provider_items_by_prefix() {
    let provenance = crate::protocol::CompletionProvenance {
        package_name: "@clay/javascript".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "javascript".to_string(),
    };
    let provider = crate::server::completion::CompletionProviderMeta {
        id: "javascript.keywords".to_string(),
        provenance: provenance.clone(),
        priority: 0,
        exclusive: false,
        trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
            trigger_characters: vec![".".to_string()],
        },
        word_boundary: crate::server::completion::WordBoundaryRule::default(),
        items: ["function", "for", "return"]
            .into_iter()
            .map(|item| crate::protocol::CompletionItem::new(item, item, provenance.clone()))
            .collect(),
        timeout_ms: 300,
        max_items: 32,
        generation: 0,
    };
    let request = crate::protocol::CompletionRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
        trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
        provider_generation: 0,
        recent_completions: Vec::<String>::new().into_boxed_slice(),
    };

    let result =
        static_package_completion_result(&request, "javascript.javascript", "fu", &[provider])
            .unwrap();

    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["function"]
    );
    assert_eq!(result.provenance, provenance);
}

#[test]
fn static_package_completion_equal_priority_merge_uses_shared_score() {
    let provenance = crate::protocol::CompletionProvenance {
        package_name: "@clay/rust".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "rust".to_string(),
    };
    let provider = |id: &str, item: crate::protocol::CompletionItem| {
        crate::server::completion::CompletionProviderMeta {
            id: id.to_string(),
            provenance: provenance.clone(),
            priority: 0,
            exclusive: false,
            trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
                trigger_characters: vec![".".to_string()],
            },
            word_boundary: crate::server::completion::WordBoundaryRule::default(),
            items: vec![item],
            timeout_ms: 300,
            max_items: 32,
            generation: 0,
        }
    };
    let keyword = crate::protocol::CompletionItem::new("fn", "fn", provenance.clone());
    let snippet = crate::protocol::CompletionItem::new(
        "fn",
        "fn ${1:name}(${2:args}) {\n\t$0\n}",
        provenance.clone(),
    )
    .with_snippet();
    let providers = [
        provider("rust.keywords", keyword),
        provider("rust.snippets", snippet),
    ];
    let request = crate::protocol::CompletionRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
        trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
        provider_generation: 0,
        recent_completions: vec!["fn ${1:name}(${2:args}) {\n\t$0\n}".to_string()]
            .into_boxed_slice(),
    };

    let result = static_package_completion_result(&request, "rust.rust", "fn", &providers).unwrap();

    assert_eq!(result.items.len(), 2);
    assert_eq!(
        result.items[0].text_format,
        crate::protocol::CompletionItemTextFormat::Snippet
    );
    assert_eq!(
        result.items[1].text_format,
        crate::protocol::CompletionItemTextFormat::PlainText
    );
    assert!(result.validate().is_ok());
}
