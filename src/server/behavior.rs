use crate::{
    behavior::manifest::{ManifestValidationError, validate_manifest},
    protocol::{
        BehaviorManifest, BehaviorVersion, DocumentId, EditRejection, ServerMessage, TransactionId,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct ActiveBehaviorManifest {
    manifest: BehaviorManifest,
}

impl Default for ActiveBehaviorManifest {
    fn default() -> Self {
        Self::new(BehaviorManifest::minimal_text_editing(1))
            .expect("default text editing manifest must validate")
    }
}

impl ActiveBehaviorManifest {
    pub(crate) fn new(manifest: BehaviorManifest) -> Result<Self, ManifestValidationError> {
        validate_manifest(&manifest)?;
        Ok(Self { manifest })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read-only manifest access is used by tests now and future server publishing hooks"
        )
    )]
    pub(crate) fn manifest(&self) -> &BehaviorManifest {
        &self.manifest
    }

    pub(crate) fn version(&self) -> BehaviorVersion {
        self.manifest.behavior_version
    }

    pub(crate) fn manifest_message(&self) -> ServerMessage {
        ServerMessage::BehaviorManifest(self.manifest.clone())
    }

    pub(crate) fn validate_message_version(
        &self,
        document_id: DocumentId,
        transaction_id: TransactionId,
        behavior_version: BehaviorVersion,
    ) -> Result<(), ServerMessage> {
        if behavior_version == self.version() {
            return Ok(());
        }

        Err(ServerMessage::EditRejected {
            document_id,
            transaction_id,
            reason: EditRejection::InvalidBehaviorVersion {
                behavior_version,
                server_behavior_version: self.version(),
            },
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "manifest replacement is internal Phase 6 server ownership plumbing for future hot reload"
        )
    )]
    pub(crate) fn publish_replacement(
        &mut self,
        mut replacement: BehaviorManifest,
    ) -> Result<BehaviorManifest, ManifestValidationError> {
        replacement.behavior_version = self.version().saturating_add(1);
        validate_manifest(&replacement)?;
        self.manifest = replacement;
        Ok(self.manifest.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveBehaviorManifest;
    use crate::{
        behavior::manifest::ManifestValidationError,
        protocol::{BehaviorManifest, CommandDeclaration, EditRejection, ServerMessage},
    };

    #[test]
    fn server_publish_replacement_increments_behavior_version() {
        let mut state = ActiveBehaviorManifest::default();
        let mut replacement = BehaviorManifest::minimal_text_editing(99);
        replacement.manifest_id = "clay.default.text.replacement".to_string();

        let published = state.publish_replacement(replacement).unwrap();

        assert_eq!(published.behavior_version, 2);
        assert_eq!(state.version(), 2);
        assert_eq!(
            state.manifest().manifest_id,
            "clay.default.text.replacement"
        );
    }

    #[test]
    fn server_rejects_invalid_replacement_without_advancing_behavior_version() {
        let mut state = ActiveBehaviorManifest::default();
        let mut replacement = BehaviorManifest::minimal_text_editing(1);
        replacement.commands.push(CommandDeclaration::client_edit(
            "text.insert",
            "Duplicate Insert",
        ));

        let error = state.publish_replacement(replacement).unwrap_err();

        assert_eq!(
            error,
            ManifestValidationError::DuplicateCommandId {
                command_id: "text.insert".to_string()
            }
        );
        assert_eq!(state.version(), 1);
    }

    #[test]
    fn server_behavior_version_validation_reports_client_and_server_versions() {
        let state = ActiveBehaviorManifest::default();

        let rejection = state.validate_message_version(7, 44, 0).unwrap_err();

        assert_eq!(
            rejection,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 44,
                reason: EditRejection::InvalidBehaviorVersion {
                    behavior_version: 0,
                    server_behavior_version: 1,
                },
            }
        );
    }
}
