//! Phase 19 runtime-generation snapshot and acknowledgement protocol.
//!
//! One complete connection-scoped snapshot carries every mutually dependent
//! client-visible behavior/rendering field needed for atomic install. Live
//! fan-out uses a bounded Tokio broadcast; lagged receivers recover from the
//! latest complete snapshot rather than replaying intermediate generations.
//!
//! # Authority boundary
//!
//! Snapshots are inert protocol data only. They must not carry document source
//! text, absolute paths, package tokens, process handles, grants, callbacks,
//! secrets, raw ops, native handles, CSS, shell/network/AI directives, or
//! client-executable JavaScript. Package UI contributions remain versioned and
//! empty until package UI publication crosses IPC; the field exists so reload
//! installs clear and replace package UI through one generation boundary.

use crate::perf::budgets::{
    RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS, RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS,
};
use crate::protocol::{
    ActiveTheme, ActiveTypography, BehaviorManifest, ClientId, DecorationSet, DiagnosticSet,
    DocumentId, DocumentVersion, RuntimeDiagnostic, SduiTree,
};

/// Monotonic runtime-generation identity shared by server contributions and
/// client snapshots. Independently monotonic behavior/document versions remain
/// explicit inside each generation.
pub type RuntimeGenerationId = u64;

/// Versioned package-UI replacement carried inside a runtime snapshot.
///
/// `empty_tab` carries the one-winner pane-content contribution for new/empty
/// `main` slots. Absent means the core Open File / Open Folder fallback.
/// Two competing empty-tab contributions resolve to `None` plus a diagnostic.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq, Default,
)]
pub struct PackageUiSnapshot {
    pub version: u64,
    pub empty_tab: Option<EmptyTabContent>,
}

/// Server-authoritative empty-tab `main` contribution.
///
/// `component_json` is the already-validated inert catalog tree. Recursive
/// rkyv trees overflow the Archive bound; JSON stays bounded by the SDUI
/// snapshot budget and is re-parsed on the client with `from_declaration`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EmptyTabContent {
    pub id: String,
    pub package_name: String,
    pub component_json: String,
}

/// Per-document decoration/diagnostic reset and optional initial sets.
///
/// Reset flags tell the client to drop previous-generation render caches before
/// installing any initial sets. Absent initial sets leave the document empty
/// until later generation-tagged async render chunks arrive.
///
/// Phase 22.2: `behavior_manifest` carries the document's own mode layer (its
/// per-document behavior manifest) when one is published, so recovery installs
/// each pane's mode content without cross-pane bleed. Absent means the
/// document is governed by the snapshot's connection-wide manifest.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct DocumentRuntimeRenderState {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub reset_decorations: bool,
    pub reset_diagnostics: bool,
    pub initial_decorations: Option<DecorationSet>,
    pub initial_diagnostics: Option<DiagnosticSet>,
    pub behavior_manifest: Option<BehaviorManifest>,
}

/// Complete connection-scoped runtime state for one atomic client install.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct RuntimeStateSnapshot {
    pub runtime_generation_id: RuntimeGenerationId,
    pub client_id: ClientId,
    pub behavior: BehaviorManifest,
    pub active_theme: ActiveTheme,
    pub active_typography: ActiveTypography,
    pub sdui_tree: SduiTree,
    pub package_ui: PackageUiSnapshot,
    pub documents: Vec<DocumentRuntimeRenderState>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

/// Why a runtime snapshot failed validation before install or fan-out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStateSnapshotValidationError {
    InvalidTypography,
    EmptyBehaviorManifestId,
    TooManyDocuments { count: usize, max: usize },
    DuplicateDocumentId { document_id: DocumentId },
    DecorationDocumentMismatch { document_id: DocumentId },
    DiagnosticsDocumentMismatch { document_id: DocumentId },
    BehaviorManifestDocumentMismatch { document_id: DocumentId },
    TooManyRuntimeDiagnostics { count: usize, max: usize },
}

impl RuntimeStateSnapshot {
    /// Validate inert snapshot structure before encode, fan-out, or client install.
    pub fn validate(&self) -> Result<(), RuntimeStateSnapshotValidationError> {
        self.active_typography
            .validate()
            .map_err(|_| RuntimeStateSnapshotValidationError::InvalidTypography)?;
        if self.behavior.manifest_id.trim().is_empty() {
            return Err(RuntimeStateSnapshotValidationError::EmptyBehaviorManifestId);
        }
        if self.documents.len() > RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS {
            return Err(RuntimeStateSnapshotValidationError::TooManyDocuments {
                count: self.documents.len(),
                max: RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS,
            });
        }
        if self.diagnostics.len() > RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS {
            return Err(
                RuntimeStateSnapshotValidationError::TooManyRuntimeDiagnostics {
                    count: self.diagnostics.len(),
                    max: RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS,
                },
            );
        }

        let mut seen = Vec::with_capacity(self.documents.len());
        for document in &self.documents {
            if seen.contains(&document.document_id) {
                return Err(RuntimeStateSnapshotValidationError::DuplicateDocumentId {
                    document_id: document.document_id,
                });
            }
            seen.push(document.document_id);
            if let Some(set) = &document.initial_decorations
                && set.document_id != document.document_id
            {
                return Err(
                    RuntimeStateSnapshotValidationError::DecorationDocumentMismatch {
                        document_id: document.document_id,
                    },
                );
            }
            if let Some(manifest) = &document.behavior_manifest
                && !matches!(
                    manifest.scope,
                    crate::protocol::BehaviorScope::Document {
                        document_id: scope_document_id
                    } if scope_document_id == document.document_id
                )
            {
                return Err(
                    RuntimeStateSnapshotValidationError::BehaviorManifestDocumentMismatch {
                        document_id: document.document_id,
                    },
                );
            }
            if let Some(set) = &document.initial_diagnostics
                && set.document_id != document.document_id
            {
                return Err(
                    RuntimeStateSnapshotValidationError::DiagnosticsDocumentMismatch {
                        document_id: document.document_id,
                    },
                );
            }
        }
        Ok(())
    }

    /// Stamp the connection identity without cloning mutual behavior/render state twice.
    pub fn for_client(mut self, client_id: ClientId) -> Self {
        self.client_id = client_id;
        self
    }
}
