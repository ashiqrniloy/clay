//! Client-side runtime-generation candidate validation for Phase 19.
//!
//! A `RuntimeStateSnapshot` is inert protocol data. This module turns it into a
//! fully validated `ClientRuntimeStateCandidate` before any live client state
//! mutates. Install happens in one editor/shell pass; acknowledgement is sent
//! only after that pass succeeds.

use crate::behavior::manifest::{ManifestValidationError, validate_manifest};
use crate::protocol::{
    ActiveTheme, ActiveTypography, ActiveTypographyValidationError, BehaviorManifest, ClientId,
    DocumentRuntimeRenderState, PackageUiSnapshot, RuntimeDiagnostic, RuntimeGenerationId,
    RuntimeStateSnapshot, RuntimeStateSnapshotValidationError, SduiTree,
};

/// Validated, install-ready replacement for one runtime generation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClientRuntimeStateCandidate {
    pub(crate) runtime_generation_id: RuntimeGenerationId,
    pub(crate) client_id: ClientId,
    pub(crate) behavior: BehaviorManifest,
    pub(crate) active_theme: ActiveTheme,
    pub(crate) active_typography: ActiveTypography,
    pub(crate) sdui_tree: SduiTree,
    pub(crate) package_ui: PackageUiSnapshot,
    pub(crate) documents: Vec<DocumentRuntimeRenderState>,
    pub(crate) diagnostics: Vec<RuntimeDiagnostic>,
}

/// Why a runtime snapshot cannot become a client install candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClientRuntimeStateInstallError {
    Snapshot(RuntimeStateSnapshotValidationError),
    Behavior(ManifestValidationError),
    Typography(ActiveTypographyValidationError),
    StaleOrDuplicateGeneration {
        current: RuntimeGenerationId,
        incoming: RuntimeGenerationId,
    },
    ClientIdMismatch {
        expected: ClientId,
        actual: ClientId,
    },
}

impl ClientRuntimeStateCandidate {
    /// Validate every inert snapshot field without mutating live client state.
    ///
    /// `current_generation` is the last successfully installed generation on
    /// this connection (0 before the first live snapshot). Incoming generations
    /// must be strictly newer.
    pub(crate) fn validate(
        snapshot: RuntimeStateSnapshot,
        expected_client_id: ClientId,
        current_generation: RuntimeGenerationId,
    ) -> Result<Self, ClientRuntimeStateInstallError> {
        if snapshot.client_id != expected_client_id {
            return Err(ClientRuntimeStateInstallError::ClientIdMismatch {
                expected: expected_client_id,
                actual: snapshot.client_id,
            });
        }
        snapshot
            .validate()
            .map_err(ClientRuntimeStateInstallError::Snapshot)?;
        if snapshot.runtime_generation_id == 0
            || snapshot.runtime_generation_id <= current_generation
        {
            return Err(ClientRuntimeStateInstallError::StaleOrDuplicateGeneration {
                current: current_generation,
                incoming: snapshot.runtime_generation_id,
            });
        }
        validate_manifest(&snapshot.behavior).map_err(ClientRuntimeStateInstallError::Behavior)?;
        snapshot
            .active_typography
            .validate()
            .map_err(ClientRuntimeStateInstallError::Typography)?;

        Ok(Self {
            runtime_generation_id: snapshot.runtime_generation_id,
            client_id: snapshot.client_id,
            behavior: snapshot.behavior,
            active_theme: snapshot.active_theme,
            active_typography: snapshot.active_typography,
            sdui_tree: snapshot.sdui_tree,
            package_ui: snapshot.package_ui,
            documents: snapshot.documents,
            diagnostics: snapshot.diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        DocumentRuntimeRenderState, PackageUiSnapshot, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
    };

    fn valid_snapshot(generation: u64, client_id: u64) -> RuntimeStateSnapshot {
        let snapshot = RuntimeStateSnapshot {
            runtime_generation_id: generation,
            client_id,
            behavior: BehaviorManifest::minimal_text_editing(generation),
            active_theme: ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: ActiveTypography::default(),
            sdui_tree: SduiTree {
                ui_version: generation,
                root_id: SduiNodeId(1),
                nodes: vec![SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Label {
                        text: format!("gen-{generation}"),
                    },
                )],
            },
            package_ui: PackageUiSnapshot {
                version: generation,
            },
            documents: vec![DocumentRuntimeRenderState {
                document_id: 1,
                document_version: 1,
                reset_decorations: true,
                reset_diagnostics: true,
                initial_decorations: None,
                initial_diagnostics: None,
                behavior_manifest: None,
            }],
            diagnostics: Vec::new(),
        };
        snapshot.validate().expect("fixture");
        snapshot
    }

    #[test]
    fn validate_accepts_newer_complete_snapshot() {
        let candidate =
            ClientRuntimeStateCandidate::validate(valid_snapshot(2, 7), 7, 1).expect("candidate");
        assert_eq!(candidate.runtime_generation_id, 2);
        assert_eq!(candidate.behavior.behavior_version, 2);
        assert_eq!(candidate.package_ui.version, 2);
    }

    #[test]
    fn validate_rejects_stale_duplicate_client_mismatch_and_invalid_behavior() {
        assert!(matches!(
            ClientRuntimeStateCandidate::validate(valid_snapshot(2, 7), 7, 2),
            Err(ClientRuntimeStateInstallError::StaleOrDuplicateGeneration { .. })
        ));
        assert!(matches!(
            ClientRuntimeStateCandidate::validate(valid_snapshot(2, 7), 9, 1),
            Err(ClientRuntimeStateInstallError::ClientIdMismatch { .. })
        ));

        let mut invalid = valid_snapshot(3, 7);
        invalid.behavior.manifest_id.clear();
        assert!(matches!(
            ClientRuntimeStateCandidate::validate(invalid, 7, 1),
            Err(ClientRuntimeStateInstallError::Snapshot(_))
                | Err(ClientRuntimeStateInstallError::Behavior(_))
        ));
    }
}
