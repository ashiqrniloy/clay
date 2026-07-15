//! Phase 18.11 completion provider registry and cancellable UI-reactive lane
//! integration tests.
//!
//! Covers the acceptance criteria from plans/039 task "Implement server-side
//! completion provider registry and cancellable UI-reactive lane":
//! - Package provider registration requires `completion-provider` permission
//!   and a package-prefixed provider ID.
//! - Newer request aborts or stale-drops older in-flight requests for the same
//!   client/document.
//! - Provider generation replacement drops old-generation results after package
//!   reload/disable.
//! - Priority ordering is deterministic and preserves package/built-in
//!   provenance.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use clay::{
    packages::record::assemble_package_record,
    protocol::{
        CompletionItem, CompletionItemTextFormat, CompletionProvenance, CompletionReplacementRange,
        CompletionResultSet, CompletionStatus, CompletionTrigger,
        completion::{CompletionProviderGeneration, CompletionRequest},
    },
    server::completion::{
        BufferWordCompletionProvider, CompletionCoordinator, CompletionDocumentWindow,
        CompletionProviderError, CompletionProviderMeta, CompletionProviderRegistryError,
        CompletionTriggerMetadata, WordBoundaryRule,
    },
};
use serde_json::json;

fn completion_package(name: &str, api_prefix: &str) -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": ["completion-provider"],
            "modes": [api_prefix],
            "docs": "./docs/index.md",
            "contributions": {}
        }
    }))
    .expect("completion package fixture validates")
}

fn package_without_permission(
    name: &str,
    api_prefix: &str,
) -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": [],
            "modes": [api_prefix],
            "docs": "./docs/index.md",
            "contributions": {}
        }
    }))
    .expect("package fixture validates")
}

fn request(generation: CompletionProviderGeneration, request_id: u64) -> CompletionRequest {
    CompletionRequest {
        request_id,
        client_id: 9,
        document_id: 7,
        document_version: 42,
        behavior_version: 3,
        cursor_byte_offset: 12,
        replacement_range: CompletionReplacementRange::new(10, 12),
        trigger: CompletionTrigger::Character(".".to_string()),
        provider_generation: generation,
    }
}

fn window_for(request: &CompletionRequest) -> CompletionDocumentWindow {
    CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "core".to_string(),
        byte_start: 0,
        byte_end: 12,
        text: "hello world.".to_string(),
    }
}

fn package_provider_meta(
    id: &str,
    api_prefix: &str,
    priority: i32,
    generation: CompletionProviderGeneration,
) -> CompletionProviderMeta {
    CompletionProviderMeta {
        id: id.to_string(),
        provenance: CompletionProvenance {
            package_name: format!("@org/{api_prefix}"),
            package_version: "0.1.0".to_string(),
            package_prefix: api_prefix.to_string(),
        },
        priority,
        exclusive: false,
        trigger_metadata: CompletionTriggerMetadata::default(),
        word_boundary: WordBoundaryRule::default(),
        items: Vec::new(),
        timeout_ms: 500,
        max_items: 64,
        generation,
    }
}

fn builtin_meta(
    id: &str,
    priority: i32,
    generation: CompletionProviderGeneration,
) -> CompletionProviderMeta {
    CompletionProviderMeta::builtin_core(
        id,
        priority,
        CompletionTriggerMetadata::default(),
        WordBoundaryRule::default(),
        500,
        64,
        generation,
    )
}

fn empty_result_for(request: &CompletionRequest) -> CompletionResultSet {
    CompletionResultSet {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        replacement_range: request.replacement_range,
        status: CompletionStatus::Ok,
        items: Vec::new(),
        provenance: CompletionProvenance::builtin_core(),
    }
}

fn result_with_items(
    request: &CompletionRequest,
    items: Vec<CompletionItem>,
) -> CompletionResultSet {
    CompletionResultSet {
        items,
        status: CompletionStatus::Ok,
        ..empty_result_for(request)
    }
}

#[test]
fn each_language_registers_a_base_keyword_completion_provider() {
    for (package, provider_id, expected_item, expected_triggers) in [
        ("rust", "rust.keywords", "fn", &[".", ":"][..]),
        ("typescript", "typescript.keywords", "interface", &["."][..]),
        ("javascript", "javascript.keywords", "function", &["."][..]),
        ("markdown", "markdown.keywords", "# ", &["#", "[", "`"][..]),
    ] {
        let path = format!(
            "{}/packages/{package}/package.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let record = assemble_package_record(&value).unwrap();
        let provider = &record.contributions.completion_providers[0];

        assert_eq!(provider.id, provider_id);
        assert_eq!(provider.priority, 0);
        assert_eq!(provider.trigger_characters, expected_triggers);
        assert!(
            provider
                .items
                .iter()
                .any(|item| item.label == expected_item)
        );
        assert!(!provider.items.is_empty());
        assert!(provider.items.len() <= provider.max_items);
        assert_eq!(provider.timeout_ms, 300);
        assert_eq!(provider.max_items, 32);
    }
}

#[test]
fn first_party_rust_and_typescript_packages_ship_dedicated_snippet_providers() {
    for (package, provider_id, expected_labels) in [
        ("rust", "rust.snippets", &["fn", "match", "impl"][..]),
        (
            "typescript",
            "typescript.snippets",
            &["interface", "type"][..],
        ),
    ] {
        let path = format!(
            "{}/packages/{package}/package.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let record = assemble_package_record(&value).unwrap();
        let provider = record
            .contributions
            .completion_providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .unwrap();

        assert_eq!(provider.priority, 0);
        assert_eq!(
            provider
                .items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            expected_labels
        );
        assert!(provider.items.iter().all(|item| {
            item.text_format == CompletionItemTextFormat::Snippet
                && !item.detail.is_empty()
                && item.insert_text.contains("$0")
        }));
        assert!(provider.items.len() <= provider.max_items);
    }
}

#[test]
fn base_keyword_provider_merges_with_future_providers_at_documented_priority() {
    let coordinator = CompletionCoordinator::new();
    let package = completion_package("@org/words", "words");
    let mut base = package_provider_meta("words.keywords", "words", 0, 1);
    base.items = vec![CompletionItem::new(
        "while",
        "while",
        base.provenance.clone(),
    )];

    coordinator
        .register_package(&package, base, immediate_provider())
        .unwrap();
    coordinator
        .register_builtin(builtin_meta("future", 20, 1), immediate_provider())
        .unwrap();

    assert_eq!(
        coordinator
            .providers()
            .iter()
            .map(|provider| (provider.id.as_str(), provider.priority))
            .collect::<Vec<_>>(),
        vec![("core.future", 20), ("words.keywords", 0)]
    );
}

#[test]
fn completion_registration_has_no_per_language_rust_branch() {
    let sources = [
        include_str!("../src/server/completion.rs"),
        include_str!("../src/server/ops/completion.rs"),
    ];
    for source in sources {
        for language_id in [
            "rust.keywords",
            "typescript.keywords",
            "javascript.keywords",
            "markdown.keywords",
        ] {
            assert!(
                !source.contains(language_id),
                "completion registration must not branch on {language_id}"
            );
        }
    }
}

fn immediate_provider() -> impl Fn(
    CompletionRequest,
    CompletionDocumentWindow,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<CompletionResultSet, CompletionProviderError>>
            + Send,
    >,
> + Send
+ Sync
+ 'static {
    move |request, _window| {
        let result = empty_result_for(&request);
        Box::pin(async move { Ok(result) })
    }
}

#[tokio::test]
async fn builtin_buffer_word_provider_returns_unique_sorted_prefix_matches() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(
            BufferWordCompletionProvider::meta(1),
            BufferWordCompletionProvider,
        )
        .unwrap();
    let mut request = request(1, 31);
    request.cursor_byte_offset = 3;
    request.replacement_range = CompletionReplacementRange::new(0, 3);
    let window = CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "core".to_string(),
        byte_start: 0,
        byte_end: 37,
        text: "pri private println prism pri println".to_string(),
    };

    coordinator
        .schedule_completion(BufferWordCompletionProvider::ID, request, window)
        .unwrap();
    let result = coordinator.next_result().await.unwrap();

    assert_eq!(result.status, CompletionStatus::Ok);
    assert_eq!(
        result.replacement_range,
        CompletionReplacementRange::new(0, 3)
    );
    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["println", "prism", "private"]
    );
    assert!(
        result
            .items
            .iter()
            .all(|item| item.provenance == CompletionProvenance::builtin_core())
    );
}

#[tokio::test]
async fn builtin_buffer_word_provider_returns_empty_when_no_match() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(
            BufferWordCompletionProvider::meta(1),
            BufferWordCompletionProvider,
        )
        .unwrap();
    let mut request = request(1, 32);
    request.cursor_byte_offset = 3;
    request.replacement_range = CompletionReplacementRange::new(0, 3);
    let window = CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "core".to_string(),
        byte_start: 0,
        byte_end: 14,
        text: "xyz alpha beta".to_string(),
    };

    coordinator
        .schedule_completion(BufferWordCompletionProvider::ID, request, window)
        .unwrap();
    let result = coordinator.next_result().await.unwrap();

    assert_eq!(result.status, CompletionStatus::Empty);
    assert!(result.items.is_empty());
}

#[tokio::test]
async fn builtin_buffer_word_provider_caps_result_payload() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(
            BufferWordCompletionProvider::meta(1),
            BufferWordCompletionProvider,
        )
        .unwrap();
    let mut request = request(1, 33);
    request.cursor_byte_offset = 2;
    request.replacement_range = CompletionReplacementRange::new(0, 2);
    let mut text = "aa ".to_string();
    for index in 0..256 {
        text.push_str(&format!("aa{index:03}"));
        text.push_str(&"x".repeat(123));
        text.push(' ');
    }
    let window = CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "core".to_string(),
        byte_start: 0,
        byte_end: text.len() as u64,
        text,
    };

    coordinator
        .schedule_completion(BufferWordCompletionProvider::ID, request, window)
        .unwrap();
    let result = coordinator.next_result().await.unwrap();

    assert_eq!(result.status, CompletionStatus::Ok);
    assert!(
        clay::protocol::completion::estimated_result_payload_bytes(&result)
            <= clay::perf::budgets::COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES
    );
}

#[tokio::test]
async fn builtin_buffer_word_provider_rejects_unbounded_window() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(
            BufferWordCompletionProvider::meta(1),
            BufferWordCompletionProvider,
        )
        .unwrap();
    let mut request = request(1, 33);
    request.cursor_byte_offset = 3;
    request.replacement_range = CompletionReplacementRange::new(0, 3);
    let window = CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "core".to_string(),
        byte_start: 0,
        byte_end: 70 * 1024,
        text: "a".repeat(70 * 1024),
    };

    let error = coordinator
        .schedule_completion(BufferWordCompletionProvider::ID, request, window)
        .unwrap_err();

    assert!(matches!(
        error,
        clay::server::completion::CompletionCoordinatorError::WindowTooLarge { .. }
    ));
}

#[test]
fn package_cancellation_does_not_remove_builtin_buffer_words() {
    let coordinator = CompletionCoordinator::new();
    coordinator.register_builtin_buffer_words(1).unwrap();
    let package = completion_package("@org/words", "words");
    coordinator
        .register_package(
            &package,
            package_provider_meta("words.words", "words", 0, 1),
            immediate_provider(),
        )
        .unwrap();

    coordinator.cancel_package("words");

    assert!(
        coordinator
            .providers()
            .iter()
            .any(|meta| meta.id == BufferWordCompletionProvider::ID)
    );
    assert!(
        coordinator
            .providers()
            .iter()
            .all(|meta| meta.id != "words.words")
    );
}

#[test]
fn builtin_buffer_word_provider_uses_registry_budget_validation() {
    let mut meta = BufferWordCompletionProvider::meta(1);
    meta.max_items = clay::perf::budgets::COMPLETION_RESULT_MAX_ITEMS + 1;
    let coordinator = CompletionCoordinator::new();

    let error = coordinator
        .register_builtin(meta, BufferWordCompletionProvider)
        .unwrap_err();

    assert!(matches!(
        error,
        CompletionProviderRegistryError::InvalidMaxItems { .. }
    ));
}

#[tokio::test]
async fn package_provider_registration_requires_completion_provider_permission() {
    let coordinator = CompletionCoordinator::new();
    let package = package_without_permission("@org/noperm", "noperm");
    let err = coordinator
        .register_package(
            &package,
            package_provider_meta("noperm.words", "noperm", 0, 1),
            immediate_provider(),
        )
        .unwrap_err();
    assert!(
        matches!(
            err,
            clay::server::completion::CompletionProviderRegistryError::MissingPermission { .. }
        ),
        "package without completion-provider permission must be rejected: {err:?}"
    );

    let package = completion_package("@org/words", "words");
    coordinator
        .register_package(
            &package,
            package_provider_meta("words.words", "words", 0, 1),
            immediate_provider(),
        )
        .expect("package with permission registers");
}

#[tokio::test]
async fn package_provider_id_must_be_package_prefixed_and_not_clay_namespace() {
    let coordinator = CompletionCoordinator::new();
    let package = completion_package("@org/words", "words");

    // Wrong namespace (not package-owned).
    let err = coordinator
        .register_package(
            &package,
            package_provider_meta("other.words", "words", 0, 1),
            immediate_provider(),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        clay::server::completion::CompletionProviderRegistryError::IdNotPackageOwned { .. }
    ));

    // Reserved clay.* namespace.
    let err = coordinator
        .register_package(
            &package,
            package_provider_meta("clay.words", "words", 0, 1),
            immediate_provider(),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        clay::server::completion::CompletionProviderRegistryError::ReservedClayNamespace { .. }
    ));

    // Correct package-owned ID registers.
    coordinator
        .register_package(
            &package,
            package_provider_meta("words.words", "words", 0, 1),
            immediate_provider(),
        )
        .expect("package-owned id registers");
}

#[tokio::test]
async fn newer_request_supersedes_older_for_same_client_document() {
    // A provider that records how many times it actually ran and returns a
    // result tagged with the request id. The older request must be aborted or
    // stale-dropped so only the newer result publishes.
    let run_count = Arc::new(AtomicU64::new(0));
    let run_count_clone = run_count.clone();

    let provider = move |request: CompletionRequest,
                         _window: CompletionDocumentWindow|
          -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<CompletionResultSet, CompletionProviderError>>
                + Send,
        >,
    > {
        let run_count = run_count_clone.clone();
        Box::pin(async move {
            run_count.fetch_add(1, Ordering::SeqCst);
            // Yield once so the runtime can interleave a superseding request.
            tokio::task::yield_now().await;
            Ok(empty_result_for(&request))
        })
    };

    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), provider)
        .unwrap();

    let first = request(1, 100);
    let second = request(1, 101);
    let first_window = window_for(&first);
    let second_window = window_for(&second);
    coordinator
        .schedule_completion("core.words", first, first_window)
        .unwrap();
    coordinator
        .schedule_completion("core.words", second, second_window)
        .unwrap();

    // Drain published results (bounded wait).
    let mut published_request_ids = Vec::new();
    for _ in 0..4 {
        match tokio::time::timeout(
            std::time::Duration::from_millis(200),
            coordinator.next_result(),
        )
        .await
        {
            Ok(Some(result)) => published_request_ids.push(result.request_id),
            _ => break,
        }
    }

    // At least the newer request must publish; the older may be aborted or
    // stale-dropped. Either way, only non-stale results publish.
    assert!(
        published_request_ids.iter().all(|id| *id == 101),
        "only the newer request must publish, got {published_request_ids:?}"
    );
    let stats = coordinator.stats();
    assert!(
        stats.cancelled_superseded_tasks >= 1,
        "superseded task must be cancelled, stats {stats:?}"
    );
}

#[tokio::test]
async fn provider_generation_replacement_drops_old_generation_results() {
    let coordinator = CompletionCoordinator::new();

    // Provider registered at generation 1.
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), immediate_provider())
        .unwrap();

    let old_request = request(1, 200);
    let old_window = window_for(&old_request);
    // Bump the active generation for the document to 2 before scheduling so the
    // generation-1 result is stale-dropped on finish.
    coordinator.bump_generation(old_request.document_id, 2);

    coordinator
        .schedule_completion("core.words", old_request, old_window)
        .unwrap();

    // Allow the spawned task to finish; its generation-1 result must be
    // stale-dropped, publishing nothing.
    let mut published = 0;
    for _ in 0..4 {
        match tokio::time::timeout(
            std::time::Duration::from_millis(200),
            coordinator.next_result(),
        )
        .await
        {
            Ok(Some(_)) => published += 1,
            _ => break,
        }
    }
    assert_eq!(
        published, 0,
        "old-generation result must be stale-dropped, not published"
    );
    let stats = coordinator.stats();
    assert!(
        stats.stale_results_rejected >= 1,
        "stale generation result must be rejected, stats {stats:?}"
    );
}

#[tokio::test]
async fn disabling_provider_invalidates_in_flight_generation_and_blocks_reschedule() {
    let provider = move |request: CompletionRequest,
                         _window: CompletionDocumentWindow|
          -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<CompletionResultSet, CompletionProviderError>>
                + Send,
        >,
    > {
        Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(empty_result_for(&request))
        })
    };
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), provider)
        .unwrap();
    let old_request = request(1, 205);
    coordinator
        .schedule_completion("core.words", old_request.clone(), window_for(&old_request))
        .unwrap();

    coordinator.disable_completion("core.words", 2);

    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            coordinator.next_result()
        )
        .await
        .is_err(),
        "disabled provider must publish no old-generation result"
    );
    let new_request = request(2, 206);
    assert!(matches!(
        coordinator.schedule_completion(
            "core.words",
            new_request.clone(),
            window_for(&new_request)
        ),
        Err(clay::server::completion::CompletionCoordinatorError::ProviderNotRegistered { .. })
    ));
    assert!(coordinator.stats().cancelled_superseded_tasks >= 1);
}

#[tokio::test]
async fn stale_document_version_result_is_dropped_after_newer_request() {
    let provider = move |request: CompletionRequest,
                         _window: CompletionDocumentWindow|
          -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<CompletionResultSet, CompletionProviderError>>
                + Send,
        >,
    > {
        Box::pin(async move {
            if request.request_id == 210 {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Ok(empty_result_for(&request))
        })
    };

    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), provider)
        .unwrap();

    let old_request = request(1, 210);
    let old_window = window_for(&old_request);
    coordinator
        .schedule_completion("core.words", old_request, old_window)
        .unwrap();

    let mut newer_request = request(1, 211);
    newer_request.client_id = 10;
    newer_request.document_version = 43;
    newer_request.cursor_byte_offset = 12;
    let mut newer_window = window_for(&newer_request);
    newer_window.document_version = 43;
    coordinator
        .schedule_completion("core.words", newer_request, newer_window)
        .unwrap();

    let mut published_request_ids = Vec::new();
    for _ in 0..4 {
        match tokio::time::timeout(
            std::time::Duration::from_millis(200),
            coordinator.next_result(),
        )
        .await
        {
            Ok(Some(result)) => published_request_ids.push(result.request_id),
            _ => break,
        }
    }

    assert_eq!(published_request_ids, vec![211]);
    assert!(
        coordinator.stats().stale_results_rejected >= 1,
        "old document-version result must be stale-dropped"
    );
}

#[tokio::test]
async fn priority_ordering_is_deterministic_and_preserves_provenance() {
    let coordinator = CompletionCoordinator::new();
    let package = completion_package("@org/words", "words");

    coordinator
        .register_builtin(builtin_meta("buf", 1, 1), immediate_provider())
        .unwrap();
    coordinator
        .register_package(
            &package,
            package_provider_meta("words.tokens", "words", 5, 1),
            immediate_provider(),
        )
        .unwrap();
    coordinator
        .register_builtin(builtin_meta("other", 5, 1), immediate_provider())
        .unwrap();

    let providers = coordinator.providers();
    let ids: Vec<String> = providers.iter().map(|m| m.id.clone()).collect();
    // Higher priority first; ties break by ascending ID.
    assert_eq!(ids, vec!["core.other", "words.tokens", "core.buf"]);
    // Provenance is preserved on each entry.
    assert_eq!(providers[1].provenance.package_prefix, "words");
    assert_eq!(providers[0].provenance.package_prefix, "core");
}

#[tokio::test]
async fn schedule_completion_returns_without_blocking_and_publishes_result() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), immediate_provider())
        .unwrap();

    let request = request(1, 300);
    let window = window_for(&request);
    // schedule_completion must return immediately (it only spawns).
    let started = std::time::Instant::now();
    coordinator
        .schedule_completion("core.words", request, window)
        .unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "schedule_completion must not block: took {elapsed:?}"
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        coordinator.next_result(),
    )
    .await
    .expect("result must publish within timeout")
    .expect("result must publish");
    assert_eq!(result.request_id, 300);
}

#[tokio::test]
async fn cancel_package_removes_package_provider() {
    let coordinator = CompletionCoordinator::new();
    let package = completion_package("@org/words", "words");
    coordinator
        .register_package(
            &package,
            package_provider_meta("words.words", "words", 0, 1),
            immediate_provider(),
        )
        .unwrap();
    assert_eq!(coordinator.providers().len(), 1);

    coordinator.cancel_package("words");
    assert!(
        coordinator.providers().is_empty(),
        "cancel_package must remove the package provider"
    );
}

#[tokio::test]
async fn duplicate_provider_id_is_rejected_as_conflict_diagnostic() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), immediate_provider())
        .unwrap();

    let error = coordinator
        .register_builtin(builtin_meta("words", 5, 2), immediate_provider())
        .unwrap_err();

    assert!(matches!(
        error,
        CompletionProviderRegistryError::ProviderAlreadyRegistered { id } if id == "core.words"
    ));
}

#[tokio::test]
async fn disabled_package_provider_falls_back_to_builtin_buffer_words() {
    let coordinator = CompletionCoordinator::new();
    coordinator.register_builtin_buffer_words(1).unwrap();
    let package = completion_package("@org/words", "words");
    coordinator
        .register_package(
            &package,
            package_provider_meta("words.words", "words", 10, 1),
            immediate_provider(),
        )
        .unwrap();

    coordinator.cancel_package("words");

    let mut request = request(1, 410);
    request.cursor_byte_offset = 3;
    request.replacement_range = CompletionReplacementRange::new(0, 3);
    let window = CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "core".to_string(),
        byte_start: 0,
        byte_end: 19,
        text: "pri private println".to_string(),
    };
    coordinator
        .schedule_completion(BufferWordCompletionProvider::ID, request, window)
        .unwrap();
    let result = coordinator.next_result().await.unwrap();

    assert_eq!(result.status, CompletionStatus::Ok);
    assert!(result.items.iter().any(|item| item.label == "println"));
    assert!(
        coordinator
            .providers()
            .iter()
            .all(|meta| meta.id != "words.words")
    );
}

#[tokio::test]
async fn oversized_result_is_rejected_before_publication() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), move |request, _window| {
            Box::pin(async move {
                Ok(result_with_items(
                    &request,
                    vec![CompletionItem {
                        label: "x".repeat(
                            clay::perf::budgets::COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS + 1,
                        ),
                        insert_text: "x".to_string(),
                        detail: String::new(),
                        commit_characters: String::new(),
                        text_format: CompletionItemTextFormat::PlainText,
                        provenance: CompletionProvenance::builtin_core(),
                    }],
                ))
            })
        })
        .unwrap();

    let request = request(1, 420);
    let window = window_for(&request);
    coordinator
        .schedule_completion("core.words", request, window)
        .unwrap();

    let published = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        coordinator.next_result(),
    )
    .await;
    assert!(
        published.is_err(),
        "invalid oversized result must not publish"
    );
    assert!(coordinator.stats().stale_results_rejected >= 1);
}

#[tokio::test]
async fn schedule_completion_rejects_unregistered_provider() {
    let coordinator = CompletionCoordinator::new();
    let request = request(1, 400);
    let window = window_for(&request);
    let err = coordinator
        .schedule_completion("core.missing", request, window)
        .unwrap_err();
    assert!(matches!(
        err,
        clay::server::completion::CompletionCoordinatorError::ProviderNotRegistered { .. }
    ));
}

#[tokio::test]
async fn window_metadata_mismatch_is_rejected() {
    let coordinator = CompletionCoordinator::new();
    coordinator
        .register_builtin(builtin_meta("words", 0, 1), immediate_provider())
        .unwrap();
    let request = request(1, 500);
    let window = window_for(&request);
    // Window version does not match request version.
    let mut window = window;
    window.document_version = 42;
    let mut request = request;
    request.document_version = 99;
    let err = coordinator
        .schedule_completion("core.words", request, window)
        .unwrap_err();
    assert!(matches!(
        err,
        clay::server::completion::CompletionCoordinatorError::WindowMetadataMismatch
    ));
}

#[tokio::test]
async fn lsp_compatible_completion_mapping_preserves_snippet_priority_exclusive_and_disable() {
    // Phase 18.20 handoff: LSP CompletionItem maps onto CompletionResultSet with
    // PlainText|Snippet text_format, deterministic priority, exclusive claim, and
    // user/package disable via cancel_package. language-server never bypasses
    // completion-provider permission.
    let coordinator = CompletionCoordinator::new();
    let exclusive_package = completion_package("@org/lspcomp", "lspcomp");
    let peer_package = completion_package("@org/peercomp", "peercomp");

    let mut exclusive = package_provider_meta("lspcomp.completions", "lspcomp", 20, 1);
    exclusive.exclusive = true;
    let snippet_item = CompletionItem::new(
        "fn",
        "fn ${1:name}() {\n\t$0\n}",
        exclusive.provenance.clone(),
    )
    .with_snippet();
    exclusive.items = vec![snippet_item.clone()];
    exclusive.trigger_metadata.trigger_characters = vec![".".to_string()];

    let mut peer = package_provider_meta("peercomp.completions", "peercomp", 5, 1);
    peer.items = vec![CompletionItem::new(
        "plain",
        "plain",
        peer.provenance.clone(),
    )];
    peer.trigger_metadata.trigger_characters = vec![".".to_string()];

    let snippet_provider = {
        let item = snippet_item.clone();
        let provenance = exclusive.provenance.clone();
        move |request: CompletionRequest, _window: CompletionDocumentWindow| {
            let mut result = result_with_items(&request, vec![item.clone()]);
            result.provenance = provenance.clone();
            Box::pin(async move { Ok(result) })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<CompletionResultSet, CompletionProviderError>,
                            > + Send,
                    >,
                >
        }
    };

    coordinator
        .register_package(&exclusive_package, exclusive, snippet_provider)
        .unwrap();
    coordinator
        .register_package(&peer_package, peer, immediate_provider())
        .unwrap();

    let ordered = coordinator.providers();
    assert_eq!(ordered[0].id, "lspcomp.completions");
    assert_eq!(ordered[0].priority, 20);
    assert!(ordered[0].exclusive);
    assert_eq!(ordered[1].id, "peercomp.completions");
    assert!(!ordered[1].exclusive);

    let request = {
        let mut request = request(1, 700);
        request.trigger = CompletionTrigger::Character(".".to_string());
        request.cursor_byte_offset = 1;
        request.replacement_range = CompletionReplacementRange::new(0, 1);
        request
    };
    let window = CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: "lspcomp".to_string(),
        byte_start: 0,
        byte_end: 2,
        text: "x.".to_string(),
    };
    coordinator
        .schedule_completion("lspcomp.completions", request, window)
        .unwrap();
    let result = coordinator.next_result().await.unwrap();
    assert_eq!(result.status, CompletionStatus::Ok);
    assert_eq!(result.items.len(), 1);
    assert_eq!(
        result.items[0].text_format,
        CompletionItemTextFormat::Snippet
    );
    assert!(result.items[0].insert_text.contains("$0"));
    assert_eq!(result.provenance.package_prefix, "lspcomp");

    coordinator.cancel_package("lspcomp");
    assert!(
        coordinator
            .providers()
            .iter()
            .all(|meta| meta.id != "lspcomp.completions"),
        "user/package disable must remove the exclusive LSP-mapped provider"
    );
    assert!(
        coordinator
            .providers()
            .iter()
            .any(|meta| meta.id == "peercomp.completions"),
        "disabling one package must not remove peer providers"
    );

    let missing_permission = assemble_package_record(&json!({
        "name": "@org/noperm",
        "version": "1.0.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "noperm",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "capabilities": ["language-server"],
            "modes": [],
            "docs": "./docs/index.md",
            "contributions": {
                "languageServers": [{
                    "id": "noperm.server",
                    "executable": "/bin/true",
                    "args": ["--stdio"]
                }]
            }
        }
    }))
    .unwrap();
    let err = coordinator
        .register_package(
            &missing_permission,
            package_provider_meta("noperm.completions", "noperm", 1, 1),
            immediate_provider(),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        CompletionProviderRegistryError::MissingPermission { .. }
    ));
}
