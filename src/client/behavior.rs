use crate::{
    behavior::manifest::{ManifestValidationError, validate_manifest},
    protocol::{BehaviorManifest, RoutingPolicy},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClientBehaviorState {
    active: BehaviorManifest,
}

impl ClientBehaviorState {
    pub(crate) fn new(manifest: BehaviorManifest) -> Result<Self, ManifestValidationError> {
        validate_manifest(&manifest)?;
        Ok(Self { active: manifest })
    }

    pub(crate) fn install_replacement(
        &mut self,
        manifest: BehaviorManifest,
    ) -> Result<(), ManifestValidationError> {
        validate_manifest(&manifest)?;
        self.active = manifest;
        Ok(())
    }
}

/// Maps built-in `language.*` command IDs to language-intelligence features.
pub fn language_intelligence_feature_for_command(
    command_id: &str,
) -> Option<crate::protocol::LanguageIntelligenceFeature> {
    use crate::protocol::LanguageIntelligenceFeature;
    match command_id {
        "language.hover" => Some(LanguageIntelligenceFeature::Hover),
        "language.goToDefinition" => Some(LanguageIntelligenceFeature::GoToDefinition),
        "language.codeActions" => Some(LanguageIntelligenceFeature::CodeAction),
        "language.signatureHelp" => Some(LanguageIntelligenceFeature::SignatureHelp),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientUiCommandRoute {
    pub command_id: String,
    pub routing_policy: RoutingPolicy,
}
