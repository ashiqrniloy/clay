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
/// The complete validated package projection installs atomically with its
/// runtime generation. `empty_tab` carries the one-winner pane-content
/// contribution for new/empty `main` slots; absent means the core Open File /
/// Open Folder fallback. Component trees remain bounded JSON only on the rkyv
/// wire and are parsed into typed inert Tauri DTO values before React sees them.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct PackageUiSnapshot {
    pub version: u64,
    pub empty_tab: Option<EmptyTabContent>,
    pub panels: Vec<PackagePanelContent>,
    pub overlays: Vec<PackageOverlayContent>,
    pub components: Vec<PackageComponentContent>,
    pub input_routes: Vec<PackageInputRouteContent>,
}

/// Host-stamped package identity shown by package UI projections.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct PackageUiProvenance {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub trust_domain: PackageUiTrustDomain,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PackageUiTrustDomain {
    Trusted,
    ThirdParty,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct PackagePanelContent {
    pub id: String,
    pub slot: String,
    pub visibility: String,
    pub component_json: String,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct PackageOverlayContent {
    pub id: String,
    pub anchor: String,
    pub focus_policy: String,
    pub dismissal_policy: String,
    pub component_json: String,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct PackageComponentContent {
    pub id: String,
    pub component_json: String,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct PackageInputRouteContent {
    pub id: String,
    pub scope: String,
    pub component_id: String,
    pub pointer_click: String,
    pub pointer_action: Option<String>,
    pub pointer_drag: String,
    pub focus_policy: String,
    pub selection_policy: String,
    pub context_modes: Vec<String>,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

/// Server-authoritative empty-tab `main` contribution.
///
/// `component_json` is the already-validated inert catalog tree. Recursive
/// rkyv trees overflow the Archive bound; JSON stays bounded by the SDUI
/// snapshot budget and is parsed only by host-owned Rust adapters.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct EmptyTabContent {
    pub id: String,
    pub package_name: String,
    pub component_json: String,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
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
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
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
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
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
    InvalidPackageUi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageUiSnapshotValidationError {
    TooManyEntries,
    DuplicateId,
    InvalidPolicy,
    InvalidComponent,
}

impl PackageUiSnapshot {
    /// Validate the already-server-validated wire projection before fan-out.
    pub fn validate(&self) -> Result<(), PackageUiSnapshotValidationError> {
        if self.panels.len() > 4 || self.overlays.len() > 16 || self.input_routes.len() > 64 {
            return Err(PackageUiSnapshotValidationError::TooManyEntries);
        }
        let mut ids = std::collections::BTreeSet::new();
        let surface_ids = self
            .empty_tab
            .iter()
            .map(|entry| entry.id.as_str())
            .chain(self.panels.iter().map(|entry| entry.id.as_str()))
            .chain(self.overlays.iter().map(|entry| entry.id.as_str()))
            .chain(self.components.iter().map(|entry| entry.id.as_str()))
            .chain(self.input_routes.iter().map(|entry| entry.id.as_str()));
        if surface_ids
            .into_iter()
            .any(|id| id.is_empty() || !ids.insert(id))
        {
            return Err(PackageUiSnapshotValidationError::DuplicateId);
        }
        if self.panels.iter().any(|entry| {
            !matches!(entry.slot.as_str(), "left" | "right" | "top" | "bottom")
                || !matches!(
                    entry.visibility.as_str(),
                    "visible" | "hidden" | "collapsed"
                )
        }) || self.overlays.iter().any(|entry| {
            !matches!(
                entry.anchor.as_str(),
                "working-area" | "active-pane" | "main" | "pointer"
            )
        }) {
            return Err(PackageUiSnapshotValidationError::InvalidPolicy);
        }
        let components = self
            .empty_tab
            .iter()
            .map(|entry| entry.component_json.as_str())
            .chain(
                self.panels
                    .iter()
                    .map(|entry| entry.component_json.as_str()),
            )
            .chain(
                self.overlays
                    .iter()
                    .map(|entry| entry.component_json.as_str()),
            )
            .chain(
                self.components
                    .iter()
                    .map(|entry| entry.component_json.as_str()),
            );
        for component in components {
            if component.len() > 16 * 1024
                || serde_json::from_str::<serde_json::Value>(component).is_err()
            {
                return Err(PackageUiSnapshotValidationError::InvalidComponent);
            }
        }
        Ok(())
    }

    pub fn allows_action(&self, ui_version: u64, command_id: &str) -> bool {
        self.version == ui_version
            && self
                .empty_tab
                .iter()
                .flat_map(|entry| &entry.action_targets)
                .chain(self.panels.iter().flat_map(|entry| &entry.action_targets))
                .chain(self.overlays.iter().flat_map(|entry| &entry.action_targets))
                .chain(
                    self.components
                        .iter()
                        .flat_map(|entry| &entry.action_targets),
                )
                .any(|target| target == command_id)
    }
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
        self.package_ui
            .validate()
            .map_err(|_| RuntimeStateSnapshotValidationError::InvalidPackageUi)?;

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

#[cfg(test)]
mod package_ui_tests {
    use super::*;

    fn provenance() -> PackageUiProvenance {
        PackageUiProvenance {
            package_name: "@clay/settings".into(),
            package_version: "0.1.0".into(),
            api_prefix: "settings".into(),
            trust_domain: PackageUiTrustDomain::Trusted,
        }
    }

    #[test]
    fn package_action_requires_current_version_and_declared_target() {
        let snapshot = PackageUiSnapshot {
            version: 7,
            panels: vec![PackagePanelContent {
                id: "settings.surface".into(),
                slot: "right".into(),
                visibility: "visible".into(),
                component_json: r#"{"id":"settings.root","kind":"panel"}"#.into(),
                action_targets: vec!["settings.setTheme".into()],
                provenance: provenance(),
            }],
            ..Default::default()
        };
        assert!(snapshot.allows_action(7, "settings.setTheme"));
        assert!(!snapshot.allows_action(6, "settings.setTheme"));
        assert!(!snapshot.allows_action(7, "settings.reset"));
    }
}
