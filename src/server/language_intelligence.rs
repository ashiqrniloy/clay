//! Phase 18.20 language-intelligence validation, provider registry, and
//! cancellable UI-reactive request lane.
//!
//! One feature-tagged coordinator serves hover, go-to-definition, code action,
//! and signature help. Built-in Rust providers and resolver-validated package
//! providers share the same trait. Scheduling returns immediately; newer
//! edit/cursor/request/provider-generation work cancels or stale-drops older
//! tasks. Provider registration alone grants no process/filesystem/network/
//! shell authority — process use separately requires `language-server`.
//!
//! Canonical positions remain UTF-8 byte offsets. No LSP method name, JSON-RPC
//! ID, `file://` URI, or UTF-16 line/character encoding enters this module.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

use crate::{
    packages::{permissions::PackagePermission, record::PackageRecord},
    perf::budgets::*,
    protocol::{
        BehaviorVersion, ClientId, CompletionProvenance, DocumentId, DocumentVersion,
        LanguageIntelligenceFeature, LanguageIntelligencePayload,
        LanguageIntelligenceProviderGeneration, LanguageIntelligenceRejection,
        LanguageIntelligenceRequest, LanguageIntelligenceResult, LanguageIntelligenceStatus,
        TextByteRange, TextLocation,
        language_intelligence::estimated_intelligence_result_payload_bytes,
    },
};

// ── Validation (Task 6) ─────────────────────────────────────────────────────

/// Returns `Ok(())` when the result passes all structural budget/range/path/
/// control-character/count/index checks, or the typed rejection reason.
pub fn validate_result(
    result: &LanguageIntelligenceResult,
) -> Result<(), LanguageIntelligenceRejection> {
    let payload_bytes = estimated_intelligence_result_payload_bytes(result);
    if payload_bytes > LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES {
        return Err(LanguageIntelligenceRejection::PayloadTooLarge {
            payload_bytes,
            budget_bytes: LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES,
        });
    }
    validate_payload(&result.payload)
}

fn validate_payload(
    payload: &LanguageIntelligencePayload,
) -> Result<(), LanguageIntelligenceRejection> {
    match payload {
        LanguageIntelligencePayload::Hover(hover) => {
            if let Some(range) = hover.range {
                require_ordered(range)?;
            }
            require_string(
                &hover.markdown,
                LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS,
                crate::protocol::LanguageIntelligenceField::HoverMarkdown,
            )?;
        }
        LanguageIntelligencePayload::GoToDefinition(def) => {
            if def.locations.len() > LANGUAGE_INTELLIGENCE_MAX_DEFINITION_LOCATIONS {
                return Err(LanguageIntelligenceRejection::TooManyDefinitionLocations {
                    count: def.locations.len(),
                    max: LANGUAGE_INTELLIGENCE_MAX_DEFINITION_LOCATIONS,
                });
            }
            for loc in &def.locations {
                validate_location(loc)?;
            }
        }
        LanguageIntelligencePayload::CodeAction(action) => {
            if action.actions.len() > LANGUAGE_INTELLIGENCE_MAX_CODE_ACTIONS {
                return Err(LanguageIntelligenceRejection::TooManyCodeActions {
                    count: action.actions.len(),
                    max: LANGUAGE_INTELLIGENCE_MAX_CODE_ACTIONS,
                });
            }
            for a in &action.actions {
                require_ordered(a.range)?;
                if a.title.is_empty() {
                    return Err(LanguageIntelligenceRejection::EmptyCodeActionTitle);
                }
                require_string(
                    &a.title,
                    LANGUAGE_INTELLIGENCE_MAX_TITLE_CHARS,
                    crate::protocol::LanguageIntelligenceField::Title,
                )?;
                if let Some(command_id) = &a.command_id {
                    if command_id.is_empty() {
                        return Err(LanguageIntelligenceRejection::EmptyCommandId);
                    }
                    require_string(
                        command_id,
                        LANGUAGE_INTELLIGENCE_MAX_TITLE_CHARS,
                        crate::protocol::LanguageIntelligenceField::CommandId,
                    )?;
                }
                if let Some(edit) = &a.edit {
                    if edit.edits.len() > LANGUAGE_INTELLIGENCE_MAX_EDITS_PER_PREVIEW {
                        return Err(LanguageIntelligenceRejection::TooManyEditPreviews {
                            count: edit.edits.len(),
                            max: LANGUAGE_INTELLIGENCE_MAX_EDITS_PER_PREVIEW,
                        });
                    }
                    for ed in &edit.edits {
                        require_ordered(ed.range)?;
                        require_string(
                            &ed.replacement,
                            LANGUAGE_INTELLIGENCE_MAX_EDIT_CHARS,
                            crate::protocol::LanguageIntelligenceField::EditReplacement,
                        )?;
                    }
                }
            }
        }
        LanguageIntelligencePayload::SignatureHelp(sig) => {
            if sig.signatures.len() > LANGUAGE_INTELLIGENCE_MAX_SIGNATURES {
                return Err(LanguageIntelligenceRejection::TooManySignatures {
                    count: sig.signatures.len(),
                    max: LANGUAGE_INTELLIGENCE_MAX_SIGNATURES,
                });
            }
            if let Some(active) = sig.active_signature
                && active as usize >= sig.signatures.len()
            {
                return Err(LanguageIntelligenceRejection::ActiveSignatureOutOfRange {
                    index: active,
                    signature_count: sig.signatures.len(),
                });
            }
            for s in &sig.signatures {
                require_string(
                    &s.label,
                    LANGUAGE_INTELLIGENCE_MAX_LABEL_CHARS,
                    crate::protocol::LanguageIntelligenceField::Label,
                )?;
                require_string(
                    &s.documentation,
                    LANGUAGE_INTELLIGENCE_MAX_DOCUMENTATION_CHARS,
                    crate::protocol::LanguageIntelligenceField::Documentation,
                )?;
                if s.parameters.len() > LANGUAGE_INTELLIGENCE_MAX_PARAMETERS {
                    return Err(LanguageIntelligenceRejection::TooManyParameters {
                        count: s.parameters.len(),
                        max: LANGUAGE_INTELLIGENCE_MAX_PARAMETERS,
                    });
                }
                if let Some(active) = sig.active_parameter
                    && active as usize > s.parameters.len()
                {
                    return Err(LanguageIntelligenceRejection::ActiveParameterOutOfRange {
                        index: active,
                        parameter_count: s.parameters.len(),
                    });
                }
                for p in &s.parameters {
                    require_string(
                        &p.label,
                        LANGUAGE_INTELLIGENCE_MAX_LABEL_CHARS,
                        crate::protocol::LanguageIntelligenceField::Label,
                    )?;
                    require_string(
                        &p.documentation,
                        LANGUAGE_INTELLIGENCE_MAX_DOCUMENTATION_CHARS,
                        crate::protocol::LanguageIntelligenceField::Documentation,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_location(loc: &TextLocation) -> Result<(), LanguageIntelligenceRejection> {
    match loc {
        TextLocation::OpenDocument { range, .. } => require_ordered(*range),
        TextLocation::WorkspaceFile {
            workspace_root_id,
            relative_path,
            range,
        } => {
            if *workspace_root_id == 0 {
                return Err(LanguageIntelligenceRejection::UnsafeRelativePath {
                    relative_path: relative_path.clone(),
                });
            }
            require_ordered(*range)?;
            if relative_path.is_empty() {
                return Err(LanguageIntelligenceRejection::EmptyRelativePath);
            }
            if !is_safe_relative_path(relative_path) {
                return Err(LanguageIntelligenceRejection::UnsafeRelativePath {
                    relative_path: relative_path.clone(),
                });
            }
            require_string(
                relative_path,
                LANGUAGE_INTELLIGENCE_MAX_RELATIVE_PATH_CHARS,
                crate::protocol::LanguageIntelligenceField::RelativePath,
            )
        }
    }
}

fn require_ordered(range: TextByteRange) -> Result<(), LanguageIntelligenceRejection> {
    if range.is_ordered() {
        Ok(())
    } else {
        Err(LanguageIntelligenceRejection::UnorderedByteRange {
            byte_start: range.byte_start,
            byte_end: range.byte_end,
        })
    }
}

fn require_string(
    value: &str,
    max_chars: usize,
    field: crate::protocol::LanguageIntelligenceField,
) -> Result<(), LanguageIntelligenceRejection> {
    let len = value.chars().count();
    if len > max_chars {
        return Err(LanguageIntelligenceRejection::FieldTooLong {
            field,
            length: len,
            max_chars,
        });
    }
    if has_disallowed_control_chars(value) {
        return Err(LanguageIntelligenceRejection::ControlCharactersInField { field });
    }
    Ok(())
}

fn has_disallowed_control_chars(value: &str) -> bool {
    value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
}

/// A workspace-root-relative path is safe when it is non-empty, relative (no
/// leading slash, no Windows drive prefix), uses forward slashes, and contains
/// no traversal (`..`) components. Backslashes are normalized to forward
/// slashes before the check so a Windows-style traversal cannot slip through.
pub(crate) fn is_safe_relative_path(path: &str) -> bool {
    let normalized: String = path.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return false;
    }
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    for component in normalized.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return false;
        }
    }
    true
}

// ── Provider trait / document window ────────────────────────────────────────

/// Bounded, server-prepared open-document window handed to a language-
/// intelligence provider. Providers never receive filesystem handles, shell,
/// network, raw ops, or language-server process authority through this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageIntelligenceDocumentWindow {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub byte_start: u64,
    pub byte_end: u64,
    pub text: String,
    /// Active mode id used for provider selection. Empty string means no mode
    /// filter was supplied by the scheduler.
    pub active_mode: String,
}

impl LanguageIntelligenceDocumentWindow {
    pub fn text_len_bytes(&self) -> usize {
        self.text.len()
    }
}

pub type LanguageIntelligenceProviderFuture = Pin<
    Box<
        dyn Future<Output = Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError>>
            + Send,
    >,
>;

/// Generic provider trait shared by built-in Rust fakes and token-backed
/// package handlers. No language name or LSP method appears in the signature.
pub trait LanguageIntelligenceProvider: Send + Sync + 'static {
    fn provide(
        &self,
        request: LanguageIntelligenceRequest,
        window: LanguageIntelligenceDocumentWindow,
    ) -> LanguageIntelligenceProviderFuture;
}

impl<F, Fut> LanguageIntelligenceProvider for F
where
    F: Fn(LanguageIntelligenceRequest, LanguageIntelligenceDocumentWindow) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError>>
        + Send
        + 'static,
{
    fn provide(
        &self,
        request: LanguageIntelligenceRequest,
        window: LanguageIntelligenceDocumentWindow,
    ) -> LanguageIntelligenceProviderFuture {
        Box::pin(self(request, window))
    }
}

/// Error returned by a language-intelligence provider. Surfaced as inert
/// status; never carries raw stderr, absolute paths, or process output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageIntelligenceProviderError {
    Timeout,
    ProviderFailed(String),
}

// ── Provider metadata / registration ────────────────────────────────────────

/// Registration metadata for a token-backed package language-intelligence
/// handler. The token identifies a resolver-validated module export stored in
/// the persistent runtime; no function value crosses the op boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsLanguageIntelligenceProviderRegistration {
    pub package: PackageRecord,
    pub meta: LanguageIntelligenceProviderMeta,
    pub token: String,
    pub export_name: String,
}

/// Registration metadata for one language-intelligence provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageIntelligenceProviderMeta {
    /// Package-prefixed provider ID (e.g. `@org/pkg.intelligence`) or
    /// `core.<name>` for built-in providers. Must not claim `clay.*`.
    pub id: String,
    pub provenance: CompletionProvenance,
    /// Modes this provider serves. Empty means all modes.
    pub modes: Vec<String>,
    /// Feature kinds this provider serves. Must be non-empty.
    pub features: Vec<LanguageIntelligenceFeature>,
    /// Higher priority providers are preferred; ties break by ascending ID.
    pub priority: i32,
    /// Per-provider timeout in milliseconds (`1..=LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS`).
    pub timeout_ms: u64,
    /// Provider generation observed at registration.
    pub generation: LanguageIntelligenceProviderGeneration,
}

impl LanguageIntelligenceProviderMeta {
    pub fn builtin_core(
        id: impl Into<String>,
        features: Vec<LanguageIntelligenceFeature>,
        priority: i32,
        timeout_ms: u64,
        generation: LanguageIntelligenceProviderGeneration,
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
            modes: Vec::new(),
            features,
            priority,
            timeout_ms,
            generation,
        }
    }

    pub fn supports_feature(&self, feature: LanguageIntelligenceFeature) -> bool {
        self.features.contains(&feature)
    }

    pub fn supports_mode(&self, active_mode: &str) -> bool {
        self.modes.is_empty()
            || active_mode.is_empty()
            || self.modes.iter().any(|mode| mode == active_mode)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageIntelligenceProviderRegistryError {
    MissingPermission { package_prefix: String },
    ProviderAlreadyRegistered { id: String },
    ReservedClayNamespace { id: String },
    IdNotPackageOwned { id: String, api_prefix: String },
    EmptyFeatures { id: String },
    InvalidTimeout { timeout_ms: u64 },
}

struct RegisteredProvider {
    meta: LanguageIntelligenceProviderMeta,
    provider: Arc<dyn LanguageIntelligenceProvider>,
}

/// Standalone language-intelligence provider registry. One generic path serves
/// all four feature kinds; selection is deterministic by feature/mode/priority/
/// provenance.
#[derive(Default)]
pub struct LanguageIntelligenceProviderRegistry {
    providers: BTreeMap<String, RegisteredProvider>,
    disabled: BTreeSet<String>,
}

impl fmt::Debug for LanguageIntelligenceProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanguageIntelligenceProviderRegistry")
            .field("len", &self.providers.len())
            .field("ids", &self.providers.keys().collect::<Vec<_>>())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl LanguageIntelligenceProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_builtin(
        &mut self,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        if !meta.id.starts_with("core.") {
            return Err(
                LanguageIntelligenceProviderRegistryError::ReservedClayNamespace {
                    id: meta.id.clone(),
                },
            );
        }
        self.register(meta, provider)
    }

    /// Register a package provider. Requires `parse-document`. Registration
    /// alone grants no `language-server` / filesystem / network / shell
    /// authority.
    pub fn register_package(
        &mut self,
        package: &PackageRecord,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        validate_package_provider(package, &meta)?;
        self.register(meta, provider)
    }

    fn register_package_replacing_older(
        &mut self,
        package: &PackageRecord,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        validate_package_provider(package, &meta)?;
        validate_provider_meta(&meta)?;
        if let Some(existing) = self.providers.get(&meta.id)
            && existing.meta.generation >= meta.generation
        {
            return Err(
                LanguageIntelligenceProviderRegistryError::ProviderAlreadyRegistered {
                    id: meta.id.clone(),
                },
            );
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

    fn register(
        &mut self,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        validate_provider_meta(&meta)?;
        if self.providers.contains_key(&meta.id) {
            return Err(
                LanguageIntelligenceProviderRegistryError::ProviderAlreadyRegistered {
                    id: meta.id.clone(),
                },
            );
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

    pub fn unregister(&mut self, id: &str) -> bool {
        self.providers.remove(id).is_some()
    }

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

    pub fn remove_older_generations(
        &mut self,
        active_generation: LanguageIntelligenceProviderGeneration,
    ) {
        self.providers
            .retain(|_, registered| registered.meta.generation >= active_generation);
    }

    pub fn list_ordered(&self) -> Vec<&LanguageIntelligenceProviderMeta> {
        let mut metas: Vec<&LanguageIntelligenceProviderMeta> =
            self.providers.values().map(|r| &r.meta).collect();
        metas.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        metas
    }

    pub fn get(&self, id: &str) -> Option<&LanguageIntelligenceProviderMeta> {
        self.providers.get(id).map(|r| &r.meta)
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    fn disable(&mut self, target: String) -> bool {
        self.disabled.insert(target)
    }

    /// Deterministic selection for one feature + optional active mode.
    pub fn providers_for_feature(
        &self,
        feature: LanguageIntelligenceFeature,
        active_mode: &str,
    ) -> Vec<&LanguageIntelligenceProviderMeta> {
        self.list_ordered()
            .into_iter()
            .filter(|meta| {
                !provider_is_disabled(meta, &self.disabled)
                    && meta.supports_feature(feature)
                    && meta.supports_mode(active_mode)
            })
            .collect()
    }

    /// Highest-priority matching provider for a feature/mode, if any.
    pub fn select_provider(
        &self,
        feature: LanguageIntelligenceFeature,
        active_mode: &str,
    ) -> Option<(
        LanguageIntelligenceProviderMeta,
        Arc<dyn LanguageIntelligenceProvider>,
    )> {
        let id = self
            .providers_for_feature(feature, active_mode)
            .first()
            .map(|meta| meta.id.clone())?;
        self.provider_clone(&id)
    }

    fn provider_clone(
        &self,
        id: &str,
    ) -> Option<(
        LanguageIntelligenceProviderMeta,
        Arc<dyn LanguageIntelligenceProvider>,
    )> {
        self.providers
            .get(id)
            .filter(|registered| !provider_is_disabled(&registered.meta, &self.disabled))
            .map(|registered| (registered.meta.clone(), registered.provider.clone()))
    }
}

pub(crate) fn provider_is_disabled(
    meta: &LanguageIntelligenceProviderMeta,
    disabled: &BTreeSet<String>,
) -> bool {
    disabled.contains(&meta.id)
        || disabled.contains(&meta.provenance.package_name)
        || disabled.contains(&meta.provenance.package_prefix)
}

fn validate_package_provider(
    package: &PackageRecord,
    meta: &LanguageIntelligenceProviderMeta,
) -> Result<(), LanguageIntelligenceProviderRegistryError> {
    if !package
        .manifest
        .clay
        .permissions
        .contains(&PackagePermission::ParseDocument)
    {
        return Err(
            LanguageIntelligenceProviderRegistryError::MissingPermission {
                package_prefix: package.manifest.clay.api_prefix.clone(),
            },
        );
    }
    if meta.id.starts_with("clay.") {
        return Err(
            LanguageIntelligenceProviderRegistryError::ReservedClayNamespace {
                id: meta.id.clone(),
            },
        );
    }
    let api_prefix = package.manifest.clay.api_prefix.as_str();
    if !is_package_owned_id(&meta.id, api_prefix) && !meta.id.starts_with("core.") {
        return Err(
            LanguageIntelligenceProviderRegistryError::IdNotPackageOwned {
                id: meta.id.clone(),
                api_prefix: api_prefix.to_string(),
            },
        );
    }
    Ok(())
}

fn validate_provider_meta(
    meta: &LanguageIntelligenceProviderMeta,
) -> Result<(), LanguageIntelligenceProviderRegistryError> {
    if meta.features.is_empty() {
        return Err(LanguageIntelligenceProviderRegistryError::EmptyFeatures {
            id: meta.id.clone(),
        });
    }
    if meta.timeout_ms == 0 || meta.timeout_ms > LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS {
        return Err(LanguageIntelligenceProviderRegistryError::InvalidTimeout {
            timeout_ms: meta.timeout_ms,
        });
    }
    Ok(())
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

// ── Coordinator ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageIntelligenceCoordinatorError {
    MissingPermission { package_prefix: String },
    ProviderNotRegistered { id: String },
    NoProviderForFeature,
    OutstandingRequestLimit { limit: usize },
    WindowMetadataMismatch,
    WindowTooLarge { bytes: usize, budget: usize },
    PayloadBudgetExceeded { bytes: usize, budget: usize },
    ResultValidation(LanguageIntelligenceRejection),
}

impl fmt::Display for LanguageIntelligenceCoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for LanguageIntelligenceCoordinatorError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageIntelligenceCoordinatorStats {
    pub scheduled_tasks: usize,
    pub cancelled_superseded_tasks: usize,
    pub published_results: usize,
    pub stale_results_rejected: usize,
    pub failed_tasks: usize,
    pub timed_out_tasks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TaskKey {
    generation: LanguageIntelligenceProviderGeneration,
    client_id: ClientId,
    document_id: DocumentId,
    feature: LanguageIntelligenceFeature,
    provider_id: String,
}

struct LanguageIntelligenceCoordinatorInner {
    registry: LanguageIntelligenceProviderRegistry,
    active_tasks: HashMap<TaskKey, JoinHandle<()>>,
    current_versions: HashMap<DocumentId, DocumentVersion>,
    current_generations: HashMap<DocumentId, LanguageIntelligenceProviderGeneration>,
    stats: LanguageIntelligenceCoordinatorStats,
}

/// Cancellable language-intelligence coordinator. One lane serves all four
/// features. `schedule` returns immediately with a per-request reply receiver;
/// superseded/stale/invalid results drop the receiver so the caller learns no
/// result will arrive.
#[derive(Clone)]
pub struct LanguageIntelligenceCoordinator {
    inner: Arc<Mutex<LanguageIntelligenceCoordinatorInner>>,
}

impl LanguageIntelligenceCoordinator {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LanguageIntelligenceCoordinatorInner {
                registry: LanguageIntelligenceProviderRegistry::new(),
                active_tasks: HashMap::new(),
                current_versions: HashMap::new(),
                current_generations: HashMap::new(),
                stats: LanguageIntelligenceCoordinatorStats::default(),
            })),
        }
    }

    pub fn register_builtin(
        &self,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .registry
            .register_builtin(meta, provider)
    }

    pub fn register_package(
        &self,
        package: &PackageRecord,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .registry
            .register_package(package, meta, provider)
    }

    pub(crate) fn register_package_for_generation(
        &self,
        package: &PackageRecord,
        meta: LanguageIntelligenceProviderMeta,
        provider: impl LanguageIntelligenceProvider,
    ) -> Result<(), LanguageIntelligenceProviderRegistryError> {
        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .registry
            .register_package_replacing_older(package, meta, provider)
    }

    pub fn bump_generation(
        &self,
        document_id: DocumentId,
        generation: LanguageIntelligenceProviderGeneration,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        inner.current_generations.insert(document_id, generation);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| key.document_id == document_id && key.generation < generation)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    pub fn document_changed(&self, document_id: DocumentId, version: DocumentVersion) {
        let mut inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        inner.current_versions.insert(document_id, version);
        let task_keys = inner
            .active_tasks
            .keys()
            .filter(|key| key.document_id == document_id)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    /// Tear down document-scoped state when the final access holder closes a
    /// document: version/generation tracking and active intelligence work for
    /// the document (Plan 060 T6, P1-4).
    pub(crate) fn remove_document(&self, document_id: DocumentId) {
        let mut inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        inner.current_versions.remove(&document_id);
        inner.current_generations.remove(&document_id);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| key.document_id == document_id)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    pub fn disable_provider(
        &self,
        target: impl Into<String>,
        generation: LanguageIntelligenceProviderGeneration,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        if !inner.registry.disable(target.into()) {
            return;
        }
        for active in inner.current_generations.values_mut() {
            *active = (*active).max(generation);
        }
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| key.generation < generation)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
    }

    pub fn cancel_package(&self, package_prefix: &str) {
        let mut inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
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

    pub fn cancel_generation(&self, generation: LanguageIntelligenceProviderGeneration) {
        self.cancel_older_generations(generation.saturating_add(1));
    }

    /// After a successful runtime-generation commit, keep only providers at or
    /// above `active_generation` and abort older in-flight work. Late results
    /// from aborted tasks never reach clients because schedule uses oneshot
    /// replies that drop when the task is cancelled.
    pub fn cancel_older_generations(
        &self,
        active_generation: LanguageIntelligenceProviderGeneration,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        inner.registry.remove_older_generations(active_generation);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|key| key.generation < active_generation)
            .cloned()
            .collect();
        abort_tasks(&mut inner, task_keys);
        for current in inner.current_generations.values_mut() {
            *current = (*current).max(active_generation);
        }
    }

    /// Snapshot of provider generations currently retained by the registry.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "generation introspection is used by reload cleanup tests"
        )
    )]
    pub(crate) fn registered_generations(&self) -> Vec<LanguageIntelligenceProviderGeneration> {
        let mut generations = self
            .providers()
            .into_iter()
            .map(|meta| meta.generation)
            .collect::<Vec<_>>();
        generations.sort_unstable();
        generations.dedup();
        generations
    }

    pub fn providers(&self) -> Vec<LanguageIntelligenceProviderMeta> {
        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .registry
            .list_ordered()
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn providers_for_feature(
        &self,
        feature: LanguageIntelligenceFeature,
        active_mode: &str,
    ) -> Vec<LanguageIntelligenceProviderMeta> {
        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .registry
            .providers_for_feature(feature, active_mode)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Schedule provider work for a request. Selects the highest-priority
    /// matching provider when `provider_id` is `None`. Returns immediately
    /// after spawning; never blocks edit acknowledgement or local paint.
    pub fn schedule(
        &self,
        provider_id: Option<&str>,
        mut request: LanguageIntelligenceRequest,
        window: LanguageIntelligenceDocumentWindow,
    ) -> Result<oneshot::Receiver<LanguageIntelligenceResult>, LanguageIntelligenceCoordinatorError>
    {
        let _ = request.validate();
        validate_window(&request, &window)?;

        let (provider, meta, task_key) = {
            let mut inner = self
                .inner
                .lock()
                .expect("language intelligence coordinator lock poisoned");

            if inner.active_tasks.len() >= LANGUAGE_INTELLIGENCE_MAX_OUTSTANDING_REQUESTS {
                return Err(
                    LanguageIntelligenceCoordinatorError::OutstandingRequestLimit {
                        limit: LANGUAGE_INTELLIGENCE_MAX_OUTSTANDING_REQUESTS,
                    },
                );
            }

            let (meta, provider) = match provider_id {
                Some(id) => inner.registry.provider_clone(id).ok_or_else(|| {
                    LanguageIntelligenceCoordinatorError::ProviderNotRegistered {
                        id: id.to_string(),
                    }
                })?,
                None => inner
                    .registry
                    .select_provider(request.feature, &window.active_mode)
                    .ok_or(LanguageIntelligenceCoordinatorError::NoProviderForFeature)?,
            };

            request.provider_generation = meta.generation;

            // Abort superseded in-flight work for the same client/document/feature.
            let superseded_keys: Vec<_> = inner
                .active_tasks
                .keys()
                .filter(|key| {
                    key.client_id == request.client_id
                        && key.document_id == request.document_id
                        && key.feature == request.feature
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
                feature: request.feature,
                provider_id: meta.id.clone(),
            };
            (provider, meta, task_key)
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        let coordinator = self.clone();
        let spawned_key = task_key.clone();
        let timeout_ms = meta.timeout_ms;
        let request_for_task = request.clone();
        let task = tokio::spawn(async move {
            let result_future = provider.provide(request, window);
            let outcome =
                match timeout(std::time::Duration::from_millis(timeout_ms), result_future).await {
                    Ok(inner_result) => inner_result,
                    Err(_elapsed) => Err(LanguageIntelligenceProviderError::Timeout),
                };
            coordinator.finish_task(
                spawned_key,
                request_for_task,
                meta.provenance,
                outcome,
                reply_tx,
            );
        });

        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .active_tasks
            .insert(task_key, task);
        Ok(reply_rx)
    }

    fn finish_task(
        &self,
        task_key: TaskKey,
        request: LanguageIntelligenceRequest,
        provenance: CompletionProvenance,
        result: Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError>,
        reply_tx: oneshot::Sender<LanguageIntelligenceResult>,
    ) {
        let result = match result {
            Ok(result) => result,
            Err(LanguageIntelligenceProviderError::Timeout) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("language intelligence coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.timed_out_tasks += 1;
                inner.stats.failed_tasks += 1;
                drop(inner);
                let _ = reply_tx.send(status_result(
                    &request,
                    LanguageIntelligenceStatus::Timeout,
                    provenance,
                ));
                return;
            }
            Err(LanguageIntelligenceProviderError::ProviderFailed(_)) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("language intelligence coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.failed_tasks += 1;
                drop(inner);
                let _ = reply_tx.send(status_result(
                    &request,
                    LanguageIntelligenceStatus::ProviderError,
                    provenance,
                ));
                return;
            }
        };

        if self.validate_task_freshness(&task_key).is_err() {
            let mut inner = self
                .inner
                .lock()
                .expect("language intelligence coordinator lock poisoned");
            inner.active_tasks.remove(&task_key);
            inner.stats.stale_results_rejected += 1;
            // Drop reply_tx without sending: caller learns no result will arrive.
            return;
        }

        match self.validate_published_result(&result) {
            Ok(()) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("language intelligence coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.published_results += 1;
                drop(inner);
                // Stamp registered provenance; providers cannot forge package identity.
                let mut published = result;
                published.provenance = provenance;
                let _ = reply_tx.send(published);
            }
            Err(_) => {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("language intelligence coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.stale_results_rejected += 1;
                // Drop reply_tx without sending: caller learns no result will arrive.
            }
        }
    }

    fn validate_task_freshness(
        &self,
        task_key: &TaskKey,
    ) -> Result<(), LanguageIntelligenceCoordinatorError> {
        let inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        let current_generation = inner
            .current_generations
            .get(&task_key.document_id)
            .copied()
            .unwrap_or(task_key.generation);
        if current_generation != task_key.generation {
            return Err(
                LanguageIntelligenceCoordinatorError::ProviderNotRegistered {
                    id: task_key.provider_id.clone(),
                },
            );
        }
        Ok(())
    }

    pub fn validate_published_result(
        &self,
        result: &LanguageIntelligenceResult,
    ) -> Result<(), LanguageIntelligenceCoordinatorError> {
        let inner = self
            .inner
            .lock()
            .expect("language intelligence coordinator lock poisoned");
        let current_generation = inner
            .current_generations
            .get(&result.document_id)
            .copied()
            .unwrap_or(result.provider_generation);
        if result.provider_generation != current_generation {
            return Err(LanguageIntelligenceCoordinatorError::ResultValidation(
                LanguageIntelligenceRejection::StaleProviderGeneration {
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
            return Err(LanguageIntelligenceCoordinatorError::ResultValidation(
                LanguageIntelligenceRejection::StaleDocumentVersion {
                    result_version: result.document_version,
                    current_version,
                },
            ));
        }
        drop(inner);

        validate_result(result).map_err(LanguageIntelligenceCoordinatorError::ResultValidation)?;
        let payload_bytes = estimated_intelligence_result_payload_bytes(result);
        if payload_bytes > LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES {
            return Err(
                LanguageIntelligenceCoordinatorError::PayloadBudgetExceeded {
                    bytes: payload_bytes,
                    budget: LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES,
                },
            );
        }
        Ok(())
    }

    pub fn stats(&self) -> LanguageIntelligenceCoordinatorStats {
        self.inner
            .lock()
            .expect("language intelligence coordinator lock poisoned")
            .stats
            .clone()
    }
}

impl Default for LanguageIntelligenceCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for LanguageIntelligenceCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanguageIntelligenceCoordinator")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

fn abort_tasks(inner: &mut LanguageIntelligenceCoordinatorInner, task_keys: Vec<TaskKey>) {
    for key in task_keys {
        if let Some(task) = inner.active_tasks.remove(&key) {
            task.abort();
            inner.stats.cancelled_superseded_tasks += 1;
        }
    }
}

fn validate_window(
    request: &LanguageIntelligenceRequest,
    window: &LanguageIntelligenceDocumentWindow,
) -> Result<(), LanguageIntelligenceCoordinatorError> {
    if window.document_id != request.document_id
        || window.document_version != request.document_version
        || window.behavior_version != request.behavior_version
    {
        return Err(LanguageIntelligenceCoordinatorError::WindowMetadataMismatch);
    }
    if window.byte_start > window.byte_end {
        return Err(LanguageIntelligenceCoordinatorError::WindowMetadataMismatch);
    }
    let window_bytes = window.text_len_bytes();
    if (window.byte_end - window.byte_start) as usize != window_bytes {
        return Err(LanguageIntelligenceCoordinatorError::WindowMetadataMismatch);
    }
    if window_bytes > LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES {
        return Err(LanguageIntelligenceCoordinatorError::WindowTooLarge {
            bytes: window_bytes,
            budget: LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES,
        });
    }
    Ok(())
}

fn empty_payload(feature: LanguageIntelligenceFeature) -> LanguageIntelligencePayload {
    match feature {
        LanguageIntelligenceFeature::Hover => {
            LanguageIntelligencePayload::Hover(crate::protocol::HoverResult {
                range: None,
                markdown: String::new(),
            })
        }
        LanguageIntelligenceFeature::GoToDefinition => {
            LanguageIntelligencePayload::GoToDefinition(crate::protocol::GoToDefinitionResult {
                locations: Vec::new(),
            })
        }
        LanguageIntelligenceFeature::CodeAction => {
            LanguageIntelligencePayload::CodeAction(crate::protocol::CodeActionResult {
                actions: Vec::new(),
            })
        }
        LanguageIntelligenceFeature::SignatureHelp => {
            LanguageIntelligencePayload::SignatureHelp(crate::protocol::SignatureHelpResult {
                signatures: Vec::new(),
                active_signature: None,
                active_parameter: None,
            })
        }
    }
}

fn status_result(
    request: &LanguageIntelligenceRequest,
    status: LanguageIntelligenceStatus,
    provenance: CompletionProvenance,
) -> LanguageIntelligenceResult {
    LanguageIntelligenceResult {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        feature: request.feature,
        status,
        payload: empty_payload(request.feature),
        provenance,
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn safe_relative_paths_are_accepted() {
        assert!(is_safe_relative_path("src/lib.rs"));
        assert!(is_safe_relative_path("a/b/c.md"));
    }

    #[test]
    fn unsafe_relative_paths_are_rejected() {
        assert!(!is_safe_relative_path(""));
        assert!(!is_safe_relative_path("/etc/passwd"));
        assert!(!is_safe_relative_path("../escape"));
        assert!(!is_safe_relative_path("a/../../b"));
        assert!(!is_safe_relative_path("C:/Windows"));
        assert!(!is_safe_relative_path("a\\..\\b"));
        assert!(!is_safe_relative_path("a//b"));
    }
}
