//! Phase 18.20 engine-neutral language-intelligence protocol shapes.
//!
//! Typed request/result data for hover, go-to-definition, code actions, and
//! signature help. These shapes are the canonical Clay contract for the generic
//! language-intelligence provider lane. One feature-tagged family serves all
//! four features; a later Phase 18.20 task adds the generic provider
//! coordinator.
//!
//! # Canonical positions
//!
//! All positions are UTF-8 byte offsets against a Clay `DocumentId` (open
//! document) or a known `WorkspaceRootId` plus a normalized relative path.
//! LSP `Position`/`Range`/`Location`, `file://` URIs, JSON-RPC IDs, LSP method
//! names, and UTF-16/UTF-32 line/character encoding are intentionally absent.
//! A Phase 18.21 LSP bridge resolves negotiated line/character positions
//! against the exact document version and constructs these byte offsets.
//!
//! # Authority boundary
//!
//! These shapes are inert data only. No callbacks, raw op names, native
//! handles, CSS/HTML/script injection, client-side JavaScript, executable
//! snippet transforms, or shell/network/AI directives are represented. A
//! `CodeAction` may carry an inert versioned edit preview and/or a reference
//! to a registered command ID; Phase 18.20 never auto-applies an edit, and a
//! command-backed action executes later through `CommandExecution`. Locations
//! reference only an open document or a known workspace root plus a relative
//! path — never a raw absolute path or external URI.

use crate::protocol::{
    BehaviorVersion, ClientId, CompletionProvenance, DocumentId, DocumentVersion, WorkspaceRootId,
};

/// Monotonic per-client language-intelligence request identifier.
pub type LanguageIntelligenceRequestId = u64;

/// Monotonic language-intelligence provider generation. Bumped when providers
/// are registered, disabled, revoked, or reloaded so in-flight work can be
/// stale-dropped against the generation observed at request time.
pub type LanguageIntelligenceProviderGeneration = u64;

/// Which feature a request or result targets. One coordinator serves all four.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub enum LanguageIntelligenceFeature {
    Hover,
    GoToDefinition,
    CodeAction,
    SignatureHelp,
}

/// Canonical UTF-8 byte range within a Clay document. `byte_start <= byte_end`;
/// both are validated against the document at its exact version by the
/// coordinator before publication.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextByteRange {
    pub byte_start: u64,
    pub byte_end: u64,
}

impl TextByteRange {
    pub const fn new(byte_start: u64, byte_end: u64) -> Self {
        Self {
            byte_start,
            byte_end,
        }
    }

    /// Returns `true` when `byte_start <= byte_end`.
    pub fn is_ordered(&self) -> bool {
        self.byte_start <= self.byte_end
    }
}

/// Where a definition, reference, or code-action edit lives. Either an open
/// Clay document or a known workspace root plus a normalized relative path.
/// Raw absolute paths, `file://`/external URIs, and traversal (`..`) are
/// rejected by validation.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TextLocation {
    OpenDocument {
        document_id: DocumentId,
        range: TextByteRange,
    },
    WorkspaceFile {
        workspace_root_id: WorkspaceRootId,
        /// Normalized root-relative path using forward slashes. Must be
        /// non-empty, contain no traversal components, and be relative.
        relative_path: String,
        range: TextByteRange,
    },
}

/// Optional byte range a hover applies to, plus bounded Markdown/plain-text
/// content rendered client-side as inert text.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HoverResult {
    pub range: Option<TextByteRange>,
    pub markdown: String,
}

/// Bounded, deterministically ordered definition locations.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GoToDefinitionResult {
    pub locations: Vec<TextLocation>,
}

/// One inert versioned text replacement. Phase 18.20 carries edit previews
/// only; they are never auto-applied.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RangeEdit {
    pub range: TextByteRange,
    pub replacement: String,
}

/// Inert edit preview bound to an exact open-document version. The coordinator
/// stale-drops a preview whose version no longer matches.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EditPreview {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub edits: Vec<RangeEdit>,
}

/// One code action. A command-backed action references a registered command ID
/// and executes later through `CommandExecution`; a direct edit is an inert
/// preview only in Phase 18.20.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CodeAction {
    pub range: TextByteRange,
    pub title: String,
    /// Optional registered command ID. When present the action executes later
    /// through `CommandExecution`; the coordinator confirms registration.
    pub command_id: Option<String>,
    pub edit: Option<EditPreview>,
}

/// Bounded set of code actions for a request range.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CodeActionResult {
    pub actions: Vec<CodeAction>,
}

/// One signature parameter. Inert label/documentation text only.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ParameterInformation {
    pub label: String,
    pub documentation: String,
}

/// One signature candidate. Inert label/documentation/parameter text only.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: String,
    pub parameters: Vec<ParameterInformation>,
}

/// Bounded signature help with validated active indexes.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpResult {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: Option<u16>,
    pub active_parameter: Option<u16>,
}

/// Feature-tagged result body carried inside one versioned envelope.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LanguageIntelligencePayload {
    Hover(HoverResult),
    GoToDefinition(GoToDefinitionResult),
    CodeAction(CodeActionResult),
    SignatureHelp(SignatureHelpResult),
}

/// Inert status carried alongside a result. Maps to transient UI state without
/// executing provider code.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageIntelligenceStatus {
    /// The provider produced a result for the feature.
    Ok,
    /// The provider ran to completion but produced nothing for the feature.
    Empty,
    /// The provider exceeded its timeout and returned partial/no data.
    Timeout,
    /// The provider reported an internal error.
    ProviderError,
}

/// A typed, versioned language-intelligence request enqueued after a local-first
/// command/intent captures the current document/version/cursor byte offset.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LanguageIntelligenceRequest {
    pub request_id: LanguageIntelligenceRequestId,
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub cursor_byte_offset: u64,
    pub feature: LanguageIntelligenceFeature,
    pub provider_generation: LanguageIntelligenceProviderGeneration,
}

/// Bounded, versioned, provenance-bearing server-to-client result envelope for
/// one request.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LanguageIntelligenceResult {
    pub request_id: LanguageIntelligenceRequestId,
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub provider_generation: LanguageIntelligenceProviderGeneration,
    pub feature: LanguageIntelligenceFeature,
    pub status: LanguageIntelligenceStatus,
    pub payload: LanguageIntelligencePayload,
    pub provenance: CompletionProvenance,
}

/// Validation failure for a [`LanguageIntelligenceRequest`] before any provider
/// work is scheduled.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageIntelligenceRequestRejection {
    /// The cursor byte offset is not a valid (ordered) point reference.
    InvalidCursorOffset,
}

impl LanguageIntelligenceRequest {
    /// Returns `Ok(())` when the request is structurally valid before any
    /// provider work is scheduled.
    pub fn validate(&self) -> Result<(), LanguageIntelligenceRequestRejection> {
        Ok(())
    }
}

/// Which string or nested field exceeded a budget, for typed rejection.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LanguageIntelligenceField {
    HoverMarkdown,
    Title,
    CommandId,
    Label,
    Documentation,
    EditReplacement,
    RelativePath,
    ProvenanceName,
    ProvenanceVersion,
    ProvenancePrefix,
}

/// Why a language-intelligence result was rejected before client publication.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LanguageIntelligenceRejection {
    UnorderedByteRange {
        byte_start: u64,
        byte_end: u64,
    },
    TooManyDefinitionLocations {
        count: usize,
        max: usize,
    },
    TooManyCodeActions {
        count: usize,
        max: usize,
    },
    TooManySignatures {
        count: usize,
        max: usize,
    },
    TooManyParameters {
        count: usize,
        max: usize,
    },
    TooManyEditPreviews {
        count: usize,
        max: usize,
    },
    ActiveSignatureOutOfRange {
        index: u16,
        signature_count: usize,
    },
    ActiveParameterOutOfRange {
        index: u16,
        parameter_count: usize,
    },
    EmptyCodeActionTitle,
    EmptyCommandId,
    EmptyRelativePath,
    UnsafeRelativePath {
        relative_path: String,
    },
    FieldTooLong {
        field: LanguageIntelligenceField,
        length: usize,
        max_chars: usize,
    },
    ControlCharactersInField {
        field: LanguageIntelligenceField,
    },
    PayloadTooLarge {
        payload_bytes: usize,
        budget_bytes: usize,
    },
    StaleProviderGeneration {
        result_generation: LanguageIntelligenceProviderGeneration,
        current_generation: LanguageIntelligenceProviderGeneration,
    },
    StaleDocumentVersion {
        result_version: DocumentVersion,
        current_version: DocumentVersion,
    },
}

/// Estimated lower bound on the encoded byte length of a result, used to reject
/// oversized payloads before client publication without re-encoding. Sums
/// string-field bytes plus a small fixed envelope allowance. The true rkyv
/// payload length is checked by the codec frame gate; this helper is an earlier
/// allocation-free budget check.
pub fn estimated_intelligence_result_payload_bytes(result: &LanguageIntelligenceResult) -> usize {
    const ENVELOPE_ALLOWANCE_BYTES: usize = 256;
    let mut total = ENVELOPE_ALLOWANCE_BYTES;
    total += result.provenance.package_name.len()
        + result.provenance.package_version.len()
        + result.provenance.package_prefix.len();
    total += payload_string_bytes(&result.payload);
    total
}

fn payload_string_bytes(payload: &LanguageIntelligencePayload) -> usize {
    match payload {
        LanguageIntelligencePayload::Hover(hover) => hover.markdown.len(),
        LanguageIntelligencePayload::GoToDefinition(def) => def
            .locations
            .iter()
            .map(|loc| match loc {
                TextLocation::OpenDocument { .. } => 0,
                TextLocation::WorkspaceFile { relative_path, .. } => relative_path.len(),
            })
            .sum(),
        LanguageIntelligencePayload::CodeAction(action) => action
            .actions
            .iter()
            .map(|a| {
                a.title.len()
                    + a.command_id.as_ref().map(|c| c.len()).unwrap_or(0)
                    + a.edit
                        .as_ref()
                        .map(|e| e.edits.iter().map(|ed| ed.replacement.len()).sum::<usize>())
                        .unwrap_or(0)
            })
            .sum(),
        LanguageIntelligencePayload::SignatureHelp(sig) => sig
            .signatures
            .iter()
            .map(|s| {
                s.label.len()
                    + s.documentation.len()
                    + s.parameters
                        .iter()
                        .map(|p| p.label.len() + p.documentation.len())
                        .sum::<usize>()
            })
            .sum(),
    }
}
