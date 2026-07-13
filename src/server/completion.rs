//! Phase 18.11 server-side completion provider registry and cancellable
//! UI-reactive priority lane.
//!
//! Mirrors the parse-coordinator lifecycle in a smaller form: one generic
//! provider trait/adapter serves both built-in Rust providers and
//! resolver-validated package providers, completion work runs on a
//! UI-reactive priority lane via `tokio::spawn`/`abort`, newer
//! edit/cursor/mode/provider-generation requests cancel or stale-drop older
//! work, and scheduling returns without blocking edit acknowledgement or local
//! paint.
//!
//! # Authority boundary
//!
//! Package providers receive only a server-prepared, bounded
//! [`CompletionDocumentWindow`] (open-document metadata + a bounded text slice)
//! and the inert [`CompletionRequest`]. They cannot access filesystem, network,
//! shell, AI, raw ops, package manager, native widgets, client runtime, or
//! arbitrary workspace indexes. Built-in Rust providers use the same trait and
//! the same bounded context; they gain no extra authority through the trait.
//! No language-specific provider branches or parallel provider registries
//! exist here.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::{sync::mpsc, task::JoinHandle, time::timeout};

use crate::{
    packages::{permissions::PackagePermission, record::PackageRecord},
    perf::budgets::{COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES},
    protocol::{
        BehaviorVersion, ClientId, CompletionItem, CompletionStatus, DocumentId, DocumentVersion,
        completion::{
            CompletionProvenance, CompletionProviderGeneration, CompletionRejection,
            CompletionReplacementRange, CompletionRequest, CompletionRequestRejection,
            CompletionResultSet, check_result_payload_budget, estimated_result_payload_bytes,
        },
    },
};

pub use crate::protocol::completion::CompletionRequestId;

/// Bounded, server-prepared document window handed to a completion provider.
///
/// The coordinator constructs this from already-open server-canonical document
/// text. Package providers observe only this bounded slice plus inert request
/// metadata; they never receive the full document, file paths, or any
/// filesystem/network/shell/AI handle. Built-in Rust providers receive the
/// same context through the same trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionDocumentWindow {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    /// Package-prefixed API identifier of the package provider, or `core` for a
    /// built-in provider. Carried for provenance/diagnostics only; it is not
    /// executable.
    pub package_prefix: String,
    /// Inclusive byte start of the window in the document.
    pub byte_start: u64,
    /// Exclusive byte end of the window in the document.
    pub byte_end: u64,
    /// Bounded UTF-8 text slice covering `[byte_start, byte_end)`.
    pub text: String,
}

impl CompletionDocumentWindow {
    pub fn byte_range(&self) -> (u64, u64) {
        (self.byte_start, self.byte_end)
    }

    pub fn text_len_bytes(&self) -> usize {
        self.text.len()
    }
}

pub type CompletionProviderFuture =
    Pin<Box<dyn Future<Output = Result<CompletionResultSet, CompletionProviderError>> + Send>>;

/// Generic completion provider trait. One trait serves both built-in Rust
/// providers (e.g. the buffer-word provider) and resolver-validated package
/// providers via a JS-runtime adapter. No language-specific branches.
///
/// The provider receives owned, bounded request/window data so it never
/// observes the full document, file paths, or any filesystem/network/shell/AI
/// handle, and so the blanket closure impl does not require higher-ranked
/// lifetime bounds.
pub trait CompletionProvider: Send + Sync + 'static {
    fn complete(
        &self,
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    ) -> CompletionProviderFuture;
}

impl<F, Fut> CompletionProvider for F
where
    F: Fn(CompletionRequest, CompletionDocumentWindow) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<CompletionResultSet, CompletionProviderError>> + Send + 'static,
{
    fn complete(
        &self,
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    ) -> CompletionProviderFuture {
        Box::pin(self(request, window))
    }
}

/// Inert word-boundary rule metadata. Providers declare which characters delimit
/// words so the registry/coordinator can shape trigger classification and the
/// buffer-word provider can split tokens. Carried as inert data; never executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordBoundaryRule {
    /// Characters that end a word (e.g. whitespace, punctuation). Inert.
    pub boundary_chars: Vec<String>,
}

impl WordBoundaryRule {
    pub fn new(boundary_chars: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            boundary_chars: boundary_chars.into_iter().map(Into::into).collect(),
        }
    }

    /// Default boundary rule used by the built-in buffer-word provider.
    pub fn default_buffer_word() -> Self {
        Self::new([
            " ", "\t", "\n", "\r", ".", ",", ";", ":", "(", ")", "[", "]", "{", "}",
        ])
    }
}

impl Default for WordBoundaryRule {
    fn default() -> Self {
        Self::default_buffer_word()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BufferWordCompletionProvider;

impl BufferWordCompletionProvider {
    pub const ID: &'static str = "core.bufferWords";

    pub fn meta(generation: CompletionProviderGeneration) -> CompletionProviderMeta {
        CompletionProviderMeta::builtin_core(
            Self::ID,
            0,
            CompletionTriggerMetadata::default(),
            WordBoundaryRule::default_buffer_word(),
            50,
            COMPLETION_RESULT_MAX_ITEMS,
            generation,
        )
    }
}

impl CompletionProvider for BufferWordCompletionProvider {
    fn complete(
        &self,
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    ) -> CompletionProviderFuture {
        Box::pin(async move { Ok(buffer_word_result(request, window)) })
    }
}

fn buffer_word_result(
    request: CompletionRequest,
    window: CompletionDocumentWindow,
) -> CompletionResultSet {
    let provenance = CompletionProvenance::builtin_core();
    let replacement_range = request.replacement_range;
    let prefix = window_text_range(&window, replacement_range)
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or("");
    let words = if prefix.is_empty() {
        BTreeSet::new()
    } else {
        collect_matching_words(&window.text, prefix)
    };

    let mut result = CompletionResultSet {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        replacement_range,
        status: CompletionStatus::Empty,
        items: Vec::new(),
        provenance: provenance.clone(),
    };

    for word in words {
        if word == prefix {
            continue;
        }
        let item = CompletionItem {
            label: word.clone(),
            insert_text: word,
            detail: "buffer word".to_string(),
            commit_characters: String::new(),
            provenance: provenance.clone(),
        };
        if item.label.chars().count() > crate::perf::budgets::COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS
            || item.insert_text.chars().count()
                > crate::perf::budgets::COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS
        {
            continue;
        }
        let mut candidate = result.clone();
        candidate.items.push(item.clone());
        if candidate.items.len() > COMPLETION_RESULT_MAX_ITEMS
            || estimated_result_payload_bytes(&candidate) > COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES
        {
            break;
        }
        result.items.push(item);
    }

    if !result.items.is_empty() {
        result.status = CompletionStatus::Ok;
    }
    result
}

fn window_text_range(
    window: &CompletionDocumentWindow,
    range: CompletionReplacementRange,
) -> Option<&str> {
    if range.byte_start < window.byte_start || range.byte_end > window.byte_end {
        return None;
    }
    let start = usize::try_from(range.byte_start - window.byte_start).ok()?;
    let end = usize::try_from(range.byte_end - window.byte_start).ok()?;
    window.text.get(start..end)
}

fn collect_matching_words(text: &str, prefix: &str) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        if is_buffer_word_character(character) {
            start.get_or_insert(index);
        } else if let Some(word_start) = start.take() {
            collect_word(&mut words, &text[word_start..index], prefix);
        }
    }
    if let Some(word_start) = start {
        collect_word(&mut words, &text[word_start..], prefix);
    }
    words
}

fn collect_word(words: &mut BTreeSet<String>, word: &str, prefix: &str) {
    if word.starts_with(prefix) {
        words.insert(word.to_string());
    }
}

fn is_buffer_word_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

/// Inert trigger metadata declared by a provider. Mirrors the behavior-manifest
/// autocomplete trigger classification: trigger characters are inert
/// `UiReactivePriority` metadata observed without executing provider code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionTriggerMetadata {
    /// Trigger characters that should request completion from this provider.
    /// Inert strings; never executed.
    pub trigger_characters: Vec<String>,
}

/// Registration metadata for a completion provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsCompletionProviderRegistration {
    pub package: PackageRecord,
    pub meta: CompletionProviderMeta,
    pub token: String,
    pub export_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionProviderMeta {
    /// Package-prefixed provider ID (e.g. `@org/pkg.words`) or `core.<name>`
    /// for built-in providers. Must not claim the reserved `clay.*` namespace.
    pub id: String,
    pub provenance: CompletionProvenance,
    /// Higher priority providers run first when multiple providers match a
    /// trigger. Ties break by provider ID for deterministic ordering.
    pub priority: i32,
    pub trigger_metadata: CompletionTriggerMetadata,
    pub word_boundary: WordBoundaryRule,
    /// Bounded inert static text-replacement items declared by the package.
    /// Each item carries package provenance and contains no callbacks, snippet
    /// transforms, commands, or external authority.
    pub items: Vec<CompletionItem>,
    /// Per-provider timeout in milliseconds. Providers that exceed it return a
    /// `Timeout` status result instead of blocking the lane.
    pub timeout_ms: u64,
    /// Per-provider cap on result item count. Smaller than or equal to
    /// `COMPLETION_RESULT_MAX_ITEMS`.
    pub max_items: usize,
    /// Provider generation observed at registration. Stale results whose
    /// generation differs from the active generation are dropped before UI
    /// publication.
    pub generation: CompletionProviderGeneration,
}

impl CompletionProviderMeta {
    /// Built-in `core` provider meta. Uses the `core.<id>` namespace and
    /// `builtin_core` provenance.
    pub fn builtin_core(
        id: impl Into<String>,
        priority: i32,
        trigger_metadata: CompletionTriggerMetadata,
        word_boundary: WordBoundaryRule,
        timeout_ms: u64,
        max_items: usize,
        generation: CompletionProviderGeneration,
    ) -> Self {
        let id = id.into();
        let full_id = if id.starts_with("core.") {
            id
        } else {
            format!("core.{id}")
        };
        Self {
            id: full_id,
            provenance: CompletionProvenance::builtin_core(),
            priority,
            trigger_metadata,
            word_boundary,
            items: Vec::new(),
            timeout_ms,
            max_items,
            generation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionProviderRegistryError {
    MissingPermission {
        package_prefix: String,
    },
    ProviderAlreadyRegistered {
        id: String,
    },
    ReservedClayNamespace {
        id: String,
    },
    /// A package provider ID must use the package's `apiPrefix` namespace.
    IdNotPackageOwned {
        id: String,
        api_prefix: String,
    },
    InvalidMaxItems {
        max_items: usize,
        budget: usize,
    },
    InvalidTimeout {
        timeout_ms: u64,
    },
}

/// One registered provider: metadata + the generic provider implementation.
struct RegisteredProvider {
    meta: CompletionProviderMeta,
    provider: Arc<dyn CompletionProvider>,
}

/// Standalone completion provider registry. Built-in Rust providers and
/// resolver-validated package providers register through one generic path.
#[derive(Default)]
pub struct CompletionProviderRegistry {
    providers: BTreeMap<String, RegisteredProvider>,
}

impl fmt::Debug for CompletionProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionProviderRegistry")
            .field("len", &self.providers.len())
            .field("ids", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CompletionProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a built-in `core` Rust provider. The meta must use the `core.*`
    /// namespace (enforced by `CompletionProviderMeta::builtin_core`).
    pub fn register_builtin(
        &mut self,
        meta: CompletionProviderMeta,
        provider: impl CompletionProvider,
    ) -> Result<(), CompletionProviderRegistryError> {
        if !meta.id.starts_with("core.") {
            return Err(CompletionProviderRegistryError::ReservedClayNamespace {
                id: meta.id.clone(),
            });
        }
        self.register(meta, provider)
    }

    /// Register a package-provided provider. Validates the `completion-provider`
    /// permission, package-prefixed ID, reserved-namespace exclusion, and
    /// per-provider budgets. The package record supplies provenance and the
    /// api prefix used for ID ownership.
    pub fn register_package(
        &mut self,
        package: &PackageRecord,
        meta: CompletionProviderMeta,
        provider: impl CompletionProvider,
    ) -> Result<(), CompletionProviderRegistryError> {
        if !package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::CompletionProvider)
        {
            return Err(CompletionProviderRegistryError::MissingPermission {
                package_prefix: package.manifest.clay.api_prefix.clone(),
            });
        }
        if meta.id.starts_with("clay.") {
            return Err(CompletionProviderRegistryError::ReservedClayNamespace {
                id: meta.id.clone(),
            });
        }
        let api_prefix = package.manifest.clay.api_prefix.as_str();
        if !is_package_owned_id(&meta.id, api_prefix) && !meta.id.starts_with("core.") {
            return Err(CompletionProviderRegistryError::IdNotPackageOwned {
                id: meta.id.clone(),
                api_prefix: api_prefix.to_string(),
            });
        }
        self.register(meta, provider)
    }

    fn register(
        &mut self,
        meta: CompletionProviderMeta,
        provider: impl CompletionProvider,
    ) -> Result<(), CompletionProviderRegistryError> {
        if meta.max_items == 0 || meta.max_items > COMPLETION_RESULT_MAX_ITEMS {
            return Err(CompletionProviderRegistryError::InvalidMaxItems {
                max_items: meta.max_items,
                budget: COMPLETION_RESULT_MAX_ITEMS,
            });
        }
        if meta.timeout_ms == 0 || meta.timeout_ms > 5_000 {
            return Err(CompletionProviderRegistryError::InvalidTimeout {
                timeout_ms: meta.timeout_ms,
            });
        }
        if self.providers.contains_key(&meta.id) {
            return Err(CompletionProviderRegistryError::ProviderAlreadyRegistered {
                id: meta.id.clone(),
            });
        }
        self.providers.insert(
            meta.id.clone(),
            RegisteredProvider {
                meta,
                provider: Arc::new(provider),
            },
        );
        Ok(())
    }

    /// Remove and return a provider by ID.
    pub fn unregister(&mut self, id: &str) -> bool {
        self.providers.remove(id).is_some()
    }

    /// Remove all providers belonging to a package prefix. Used by the
    /// package-scoped disable/revoke hook. Returns the number removed.
    pub fn remove_package(&mut self, package_prefix: &str) -> usize {
        let to_remove: Vec<String> = self
            .providers
            .iter()
            .filter(|(_, registered)| registered.meta.provenance.package_prefix == package_prefix)
            .map(|(id, _)| id.clone())
            .collect();
        let count = to_remove.len();
        for id in to_remove {
            self.providers.remove(&id);
        }
        count
    }

    /// Remove all providers registered at a generation older than the active
    /// generation. Returns the number removed.
    pub fn remove_older_generations(&mut self, active_generation: CompletionProviderGeneration) {
        self.providers
            .retain(|_, registered| registered.meta.generation >= active_generation);
    }

    /// Deterministic, priority-ordered iterator over registered providers.
    /// Higher `priority` first; ties break by ascending provider ID so ordering
    /// is stable and preserves package/built-in provenance.
    pub fn list_ordered(&self) -> Vec<&CompletionProviderMeta> {
        let mut metas: Vec<&CompletionProviderMeta> =
            self.providers.values().map(|r| &r.meta).collect();
        metas.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        metas
    }

    pub fn get(&self, id: &str) -> Option<&CompletionProviderMeta> {
        self.providers.get(id).map(|r| &r.meta)
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Providers whose trigger metadata includes the given trigger character,
    /// deterministically priority-ordered.
    pub fn providers_for_trigger_character(&self, trigger: &str) -> Vec<&CompletionProviderMeta> {
        self.list_ordered()
            .into_iter()
            .filter(|meta| {
                meta.trigger_metadata
                    .trigger_characters
                    .iter()
                    .any(|c| c == trigger)
            })
            .collect()
    }

    fn provider_clone(
        &self,
        id: &str,
    ) -> Option<(CompletionProviderMeta, Arc<dyn CompletionProvider>)> {
        self.providers
            .get(id)
            .map(|r| (r.meta.clone(), r.provider.clone()))
    }
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionCoordinatorError {
    MissingPermission { package_prefix: String },
    ProviderNotRegistered { id: String },
    InvalidRequest(CompletionRequestRejection),
    NoProviderForTrigger,
    PayloadBudgetExceeded { bytes: usize, budget: usize },
    ResultValidation(CompletionRejection),
    WindowMetadataMismatch,
    WindowTooLarge { bytes: usize, budget: usize },
}

impl fmt::Display for CompletionCoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CompletionCoordinatorError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionCoordinatorStats {
    pub scheduled_tasks: usize,
    pub cancelled_superseded_tasks: usize,
    pub published_results: usize,
    pub stale_results_rejected: usize,
    pub failed_tasks: usize,
    pub timed_out_tasks: usize,
}

/// Per-client/document active request key. Newer requests for the same client/document
/// abort or stale-drop older in-flight work. The task key below carries these
/// fields plus generation and provider id.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TaskKey {
    generation: CompletionProviderGeneration,
    client_id: ClientId,
    document_id: DocumentId,
    provider_id: String,
}

struct CompletionCoordinatorInner {
    registry: CompletionProviderRegistry,
    active_tasks: HashMap<TaskKey, JoinHandle<()>>,
    current_versions: HashMap<DocumentId, DocumentVersion>,
    current_generations: HashMap<DocumentId, CompletionProviderGeneration>,
    stats: CompletionCoordinatorStats,
}

/// Cancellable UI-reactive completion coordinator. Holds the provider registry
/// and per-client/document active request state. Scheduling spawns a background
/// task and returns immediately without blocking edit acknowledgement or local
/// paint.
#[derive(Clone)]
pub struct CompletionCoordinator {
    inner: Arc<Mutex<CompletionCoordinatorInner>>,
    results_tx: mpsc::UnboundedSender<CompletionResultSet>,
    results_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<CompletionResultSet>>>,
}

impl CompletionCoordinator {
    pub fn new() -> Self {
        let (results_tx, results_rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(Mutex::new(CompletionCoordinatorInner {
                registry: CompletionProviderRegistry::new(),
                active_tasks: HashMap::new(),
                current_versions: HashMap::new(),
                current_generations: HashMap::new(),
                stats: CompletionCoordinatorStats::default(),
            })),
            results_tx,
            results_rx: Arc::new(tokio::sync::Mutex::new(results_rx)),
        }
    }

    /// Register a built-in `core` Rust provider.
    pub fn register_builtin(
        &self,
        meta: CompletionProviderMeta,
        provider: impl CompletionProvider,
    ) -> Result<(), CompletionProviderRegistryError> {
        self.inner
            .lock()
            .expect("completion coordinator lock poisoned")
            .registry
            .register_builtin(meta, provider)
    }

    pub fn register_builtin_buffer_words(
        &self,
        generation: CompletionProviderGeneration,
    ) -> Result<(), CompletionProviderRegistryError> {
        self.register_builtin(
            BufferWordCompletionProvider::meta(generation),
            BufferWordCompletionProvider,
        )
    }

    /// Register a package-provided provider. Validates the `completion-provider`
    /// permission and package-prefixed ID.
    pub fn register_package(
        &self,
        package: &PackageRecord,
        meta: CompletionProviderMeta,
        provider: impl CompletionProvider,
    ) -> Result<(), CompletionProviderRegistryError> {
        self.inner
            .lock()
            .expect("completion coordinator lock poisoned")
            .registry
            .register_package(package, meta, provider)
    }

    /// Bump the active provider generation for a document and drop older-
    /// generation providers. Stale results whose generation differs are
    /// dropped before UI publication.
    pub fn bump_generation(
        &self,
        document_id: DocumentId,
        generation: CompletionProviderGeneration,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("completion coordinator lock poisoned");
        inner.current_generations.insert(document_id, generation);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| key.document_id == document_id && key.generation < generation)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    /// Cancel all providers and active work owned by one package prefix. This
    /// is the package-scoped disable/revoke hook; it reuses the same abort
    /// path as generation replacement and never waits for provider completion.
    pub fn cancel_package(&self, package_prefix: &str) {
        let mut inner = self
            .inner
            .lock()
            .expect("completion coordinator lock poisoned");
        inner.registry.remove_package(package_prefix);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| {
                key.provider_id.starts_with(&format!("{package_prefix}."))
                    || key.provider_id == package_prefix
            })
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    /// Cancel providers and active work at a specific generation (used when
    /// package reload replaces a generation).
    pub fn cancel_generation(&self, generation: CompletionProviderGeneration) {
        let mut inner = self
            .inner
            .lock()
            .expect("completion coordinator lock poisoned");
        inner
            .registry
            .remove_older_generations(generation.saturating_add(1));
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| key.generation <= generation)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    /// Snapshot the registry metadata, deterministically priority-ordered.
    pub fn providers(&self) -> Vec<CompletionProviderMeta> {
        self.inner
            .lock()
            .expect("completion coordinator lock poisoned")
            .registry
            .list_ordered()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Schedule completion work for a request after the edit/cursor/trigger has
    /// already been accepted locally. This method records metadata, aborts
    /// superseded work for the same client/document, and spawns a background
    /// task; it does not wait for provider completion and never blocks edit
    /// acknowledgement or local paint.
    ///
    /// `provider_id` selects the provider to run. `window` is the server-
    /// prepared bounded document window the provider may read.
    pub fn schedule_completion(
        &self,
        provider_id: &str,
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    ) -> Result<(), CompletionCoordinatorError> {
        request
            .validate()
            .map_err(CompletionCoordinatorError::InvalidRequest)?;
        validate_window(&request, &window)?;

        let (provider, meta, task_key) = {
            let mut inner = self
                .inner
                .lock()
                .expect("completion coordinator lock poisoned");
            let (meta, provider) = inner.registry.provider_clone(provider_id).ok_or_else(|| {
                CompletionCoordinatorError::ProviderNotRegistered {
                    id: provider_id.to_string(),
                }
            })?;

            // Abort any superseded in-flight task for the same client/document.
            let superseded_keys: Vec<_> = inner
                .active_tasks
                .keys()
                .filter(|key| {
                    key.client_id == request.client_id && key.document_id == request.document_id
                })
                .cloned()
                .collect();
            for key in superseded_keys {
                if let Some(task) = inner.active_tasks.remove(&key) {
                    task.abort();
                    inner.stats.cancelled_superseded_tasks += 1;
                }
            }

            inner
                .current_versions
                .insert(request.document_id, request.document_version);
            // Track the active provider generation, but never clobber a newer
            // generation (e.g. one bumped by reload/disable) with a stale
            // request's older generation. A request whose generation is older
            // than the active one is stale-dropped on finish.
            inner
                .current_generations
                .entry(request.document_id)
                .and_modify(|active| *active = (*active).max(request.provider_generation))
                .or_insert(request.provider_generation);
            inner.stats.scheduled_tasks += 1;

            let task_key = TaskKey {
                generation: request.provider_generation,
                client_id: request.client_id,
                document_id: request.document_id,
                provider_id: provider_id.to_string(),
            };
            (provider, meta, task_key)
        };

        let coordinator = self.clone();
        let spawned_key = task_key.clone();
        let max_items = meta.max_items;
        let timeout_ms = meta.timeout_ms;
        let task = tokio::spawn(async move {
            let result_future = provider.complete(request, window);
            let outcome =
                match timeout(std::time::Duration::from_millis(timeout_ms), result_future).await {
                    Ok(inner_result) => inner_result,
                    Err(_elapsed) => Err(CompletionProviderError::Timeout),
                };
            coordinator.finish_task(spawned_key, outcome, max_items);
        });

        self.inner
            .lock()
            .expect("completion coordinator lock poisoned")
            .active_tasks
            .insert(task_key, task);
        Ok(())
    }

    fn finish_task(
        &self,
        task_key: TaskKey,
        result: Result<CompletionResultSet, CompletionProviderError>,
        max_items: usize,
    ) {
        let result = match result {
            Ok(result) => result,
            Err(CompletionProviderError::Timeout) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("completion coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.timed_out_tasks += 1;
                inner.stats.failed_tasks += 1;
                return;
            }
            Err(_) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("completion coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.failed_tasks += 1;
                return;
            }
        };

        if self.validate_task_freshness(&task_key).is_err() {
            let mut inner = self
                .inner
                .lock()
                .expect("completion coordinator lock poisoned");
            inner.active_tasks.remove(&task_key);
            inner.stats.stale_results_rejected += 1;
            return;
        }

        match self.validate_result(&result, max_items) {
            Ok(()) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("completion coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.published_results += 1;
                drop(inner);
                let _ = self.results_tx.send(result);
            }
            Err(_) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("completion coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.stale_results_rejected += 1;
            }
        }
    }

    fn validate_task_freshness(
        &self,
        task_key: &TaskKey,
    ) -> Result<(), CompletionCoordinatorError> {
        let inner = self
            .inner
            .lock()
            .expect("completion coordinator lock poisoned");
        let current_generation = inner
            .current_generations
            .get(&task_key.document_id)
            .copied()
            .unwrap_or(task_key.generation);
        if current_generation != task_key.generation {
            return Err(CompletionCoordinatorError::ProviderNotRegistered {
                id: task_key.provider_id.clone(),
            });
        }
        // Document-version staleness is checked in `validate_result` against
        // the tracked current version; the task-key freshness check only
        // guards provider generation.
        Ok(())
    }

    /// Validate a result against the active generation, current document
    /// version, payload budget, item-count cap, and structural validation
    /// before UI publication.
    pub fn validate_result(
        &self,
        result: &CompletionResultSet,
        max_items: usize,
    ) -> Result<(), CompletionCoordinatorError> {
        let inner = self
            .inner
            .lock()
            .expect("completion coordinator lock poisoned");
        let current_generation = inner
            .current_generations
            .get(&result.document_id)
            .copied()
            .unwrap_or(result.provider_generation);
        if result.provider_generation != current_generation {
            return Err(CompletionCoordinatorError::ResultValidation(
                CompletionRejection::StaleProviderGeneration {
                    result_generation: result.provider_generation,
                    current_generation,
                },
            ));
        }
        let current_version = inner
            .current_versions
            .get(&result.document_id)
            .copied()
            .unwrap_or(result.document_version);
        if result.document_version != current_version {
            return Err(CompletionCoordinatorError::ResultValidation(
                CompletionRejection::StaleDocumentVersion {
                    result_version: result.document_version,
                    current_version,
                },
            ));
        }
        drop(inner);

        result
            .validate()
            .map_err(CompletionCoordinatorError::ResultValidation)?;
        if result.items.len() > max_items {
            return Err(CompletionCoordinatorError::ResultValidation(
                CompletionRejection::TooManyItems {
                    item_count: result.items.len(),
                    max_items,
                },
            ));
        }
        check_result_payload_budget(result)
            .map_err(CompletionCoordinatorError::ResultValidation)?;
        let payload_bytes = estimate_result_payload_bytes(result);
        if payload_bytes > COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES {
            return Err(CompletionCoordinatorError::PayloadBudgetExceeded {
                bytes: payload_bytes,
                budget: COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
            });
        }
        Ok(())
    }

    pub async fn next_result(&self) -> Option<CompletionResultSet> {
        self.results_rx.lock().await.recv().await
    }

    pub fn stats(&self) -> CompletionCoordinatorStats {
        self.inner
            .lock()
            .expect("completion coordinator lock poisoned")
            .stats
            .clone()
    }
}

impl Default for CompletionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CompletionCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionCoordinator")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

fn abort_tasks(inner: &mut CompletionCoordinatorInner, task_keys: Vec<TaskKey>) {
    for key in task_keys {
        if let Some(task) = inner.active_tasks.remove(&key) {
            task.abort();
            inner.stats.cancelled_superseded_tasks += 1;
        }
    }
}

fn validate_window(
    request: &CompletionRequest,
    window: &CompletionDocumentWindow,
) -> Result<(), CompletionCoordinatorError> {
    if window.document_id != request.document_id
        || window.document_version != request.document_version
        || window.behavior_version != request.behavior_version
    {
        return Err(CompletionCoordinatorError::WindowMetadataMismatch);
    }
    if window.byte_start > window.byte_end {
        return Err(CompletionCoordinatorError::WindowMetadataMismatch);
    }
    let window_bytes = window.text_len_bytes();
    if (window.byte_end - window.byte_start) as usize != window_bytes {
        return Err(CompletionCoordinatorError::WindowMetadataMismatch);
    }
    // Cap the provider-visible window so package providers never see an
    // unbounded document slice. 64 KiB is the generic completion window budget;
    // it is well under the result payload budget and large enough for buffer-
    // word and token-based providers.
    const COMPLETION_WINDOW_BUDGET_BYTES: usize = 64 * 1024;
    if window_bytes > COMPLETION_WINDOW_BUDGET_BYTES {
        return Err(CompletionCoordinatorError::WindowTooLarge {
            bytes: window_bytes,
            budget: COMPLETION_WINDOW_BUDGET_BYTES,
        });
    }
    Ok(())
}

/// Estimated encoded byte length of a result set (re-exported from the protocol
/// module shape so the coordinator can budget without re-encoding).
fn estimate_result_payload_bytes(result: &CompletionResultSet) -> usize {
    crate::protocol::completion::estimated_result_payload_bytes(result)
}

/// Error returned by a completion provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionProviderError {
    /// The provider exceeded its timeout budget.
    Timeout,
    /// The provider reported an internal error. Carried as inert status; the
    /// coordinator records a failed task and publishes nothing.
    ProviderFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

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
            status: crate::protocol::CompletionStatus::Ok,
            items: Vec::new(),
            provenance: CompletionProvenance::builtin_core(),
        }
    }

    fn ok_provider() -> impl Fn(
        CompletionRequest,
        CompletionDocumentWindow,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<CompletionResultSet, CompletionProviderError>> + Send>,
    > + Send
    + Sync
    + 'static {
        move |request, _window| {
            let result = empty_result_for(&request);
            Box::pin(async move { Ok(result) })
        }
    }

    #[test]
    fn registry_registers_builtin_provider() {
        let mut registry = CompletionProviderRegistry::new();
        registry
            .register_builtin(builtin_meta("words", 0, 1), ok_provider())
            .expect("builtin provider registers");
        assert_eq!(registry.len(), 1);
        assert!(registry.get("core.words").is_some());
    }

    #[test]
    fn registry_rejects_duplicate_builtin_id() {
        let mut registry = CompletionProviderRegistry::new();
        registry
            .register_builtin(builtin_meta("words", 0, 1), ok_provider())
            .unwrap();
        let err = registry
            .register_builtin(builtin_meta("words", 0, 1), ok_provider())
            .unwrap_err();
        assert!(matches!(
            err,
            CompletionProviderRegistryError::ProviderAlreadyRegistered { .. }
        ));
    }

    #[test]
    fn registry_priority_ordering_is_deterministic() {
        let mut registry = CompletionProviderRegistry::new();
        registry
            .register_builtin(builtin_meta("low", 1, 1), ok_provider())
            .unwrap();
        registry
            .register_builtin(builtin_meta("high", 5, 1), ok_provider())
            .unwrap();
        registry
            .register_builtin(builtin_meta("mid", 5, 1), ok_provider())
            .unwrap();
        let ordered: Vec<String> = registry
            .list_ordered()
            .into_iter()
            .map(|m| m.id.clone())
            .collect();
        // Higher priority first; ties break by ascending ID.
        assert_eq!(ordered, vec!["core.high", "core.mid", "core.low"]);
    }

    #[test]
    fn registry_rejects_invalid_max_items_and_timeout() {
        let mut registry = CompletionProviderRegistry::new();
        let mut meta = builtin_meta("words", 0, 1);
        meta.max_items = 0;
        assert!(matches!(
            registry.register_builtin(meta, ok_provider()),
            Err(CompletionProviderRegistryError::InvalidMaxItems { .. })
        ));
        let mut meta = builtin_meta("words", 0, 1);
        meta.timeout_ms = 0;
        assert!(matches!(
            registry.register_builtin(meta, ok_provider()),
            Err(CompletionProviderRegistryError::InvalidTimeout { .. })
        ));
    }

    #[test]
    fn registry_remove_package_drops_only_matching_provenance() {
        let mut registry = CompletionProviderRegistry::new();
        registry
            .register_builtin(builtin_meta("words", 0, 1), ok_provider())
            .unwrap();
        assert_eq!(registry.remove_package("core"), 1);
        assert!(registry.is_empty());
        assert_eq!(registry.remove_package("core"), 0);
    }
}
