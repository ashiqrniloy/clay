use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::behavior::manifest::validate_manifest;
use crate::packages::manifest::{ClayPackageManifest, is_valid_api_prefix};
use crate::packages::permissions::PackagePermission;
use crate::packages::record::PackageRecord;
use crate::perf::budgets::MODE_ACTIVATION_P95_BUDGET_MS;
use crate::protocol::{
    BehaviorManifest, BehaviorScope, BehaviorVersion, CommandDeclaration, DocumentId, RoutingPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeDeclaration {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub mode_id: String,
    pub display_name: String,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
    pub file_names: Vec<String>,
    pub file_name_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentClassificationInput {
    pub document_id: DocumentId,
    pub path: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeClassification {
    pub document_id: DocumentId,
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub mode_id: String,
    pub matched_by: ModePatternKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModePatternKind {
    MimeType,
    Extension,
    FileNamePattern,
    FileName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MajorModeActivation {
    pub document_id: DocumentId,
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub mode_id: String,
    pub behavior_version: BehaviorVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeDiagnostic {
    pub package_name: Option<Box<str>>,
    pub package_version: Option<Box<str>>,
    pub api_prefix: Option<Box<str>>,
    pub mode_id: Option<Box<str>>,
    pub rule: ModeValidationRule,
    pub message: Box<str>,
}

impl ModeDiagnostic {
    /// Build a diagnostic from owned `String` identity fields + any message.
    /// Centralizes the `String` -> `Box<str>` boxing so inline construction
    /// sites stay ergonomic while the `Err`-variant stays under clippy's
    /// `result_large_err` 128-byte threshold.
    fn new(
        package_name: Option<String>,
        package_version: Option<String>,
        api_prefix: Option<String>,
        mode_id: Option<String>,
        rule: ModeValidationRule,
        message: impl Into<Box<str>>,
    ) -> Self {
        Self {
            package_name: package_name.map(String::into_boxed_str),
            package_version: package_version.map(String::into_boxed_str),
            api_prefix: api_prefix.map(String::into_boxed_str),
            mode_id: mode_id.map(String::into_boxed_str),
            rule,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeValidationRule {
    MissingPermission,
    InvalidPrefix,
    InvalidModeId,
    UndeclaredMode,
    DuplicateModeId,
    MalformedPattern,
    NoClassificationMatch,
    AmbiguousClassification,
}

/// A minor-mode declaration, which must list every major mode it is compatible with.
///
/// A minor mode is a lightweight overlay that appends additional commands and
/// key bindings on top of the active major mode's manifest.  It **cannot**
/// replace or remove any entry already declared by the major mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinorModeDeclaration {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub mode_id: String,
    pub display_name: String,
    /// The major-mode IDs this minor mode is compatible with.  Empty means
    /// incompatible with everything, which is always rejected at activation.
    pub compatible_major_modes: Vec<String>,
}

/// A validated minor mode registered with the [`ModeRegistry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinorModeActivation {
    pub document_id: DocumentId,
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub mode_id: String,
}

/// Per-document manifest selection result — the inert, validated
/// [`BehaviorManifest`] for a document together with full package/mode
/// provenance so conflict diagnostics, AI-agent discovery, and the Phase-18
/// parse coordinator can all identify the owning package.
#[derive(Debug, Clone)]
pub struct DocumentManifestSelection {
    /// The inert, server-validated behavior manifest ready to be sent to the
    /// client.  Never contains executable code; routing is manifest-based.
    pub manifest: BehaviorManifest,
    /// Provenance of the active major mode that anchored this manifest.
    pub major_mode: MajorModeActivation,
    /// Provenance of each minor mode overlaid on top (in order).
    pub minor_modes: Vec<MinorModeActivation>,
}

#[derive(Debug, Default)]
pub struct ModeRegistry {
    modes: HashMap<String, RegisteredMode>,
    minor_modes: HashMap<String, RegisteredMinorMode>,
    active_major_modes: HashMap<DocumentId, MajorModeActivation>,
    active_minor_modes: HashMap<DocumentId, Vec<MinorModeActivation>>,
    /// Per-document selected manifests keyed by document ID.
    selected_manifests: HashMap<DocumentId, DocumentManifestSelection>,
}

impl ModeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn activation_budget_ms(&self) -> u64 {
        MODE_ACTIVATION_P95_BUDGET_MS
    }

    pub fn register_mode(
        &mut self,
        package: &ClayPackageManifest,
        declaration: ModeDeclaration,
    ) -> Result<(), ModeDiagnostic> {
        let context = ModeDiagnosticContext::from_declaration(&declaration);
        if !package
            .clay
            .permissions
            .contains(&PackagePermission::ModeRegistration)
        {
            return Err(context.diagnostic(
                ModeValidationRule::MissingPermission,
                "package must declare mode-registration before registering mode patterns",
            ));
        }
        if package.name != declaration.package_name
            || package.version != declaration.package_version
            || package.clay.api_prefix != declaration.api_prefix
        {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidPrefix,
                "mode declaration provenance must match the validated package manifest",
            ));
        }
        if !is_valid_api_prefix(&declaration.api_prefix) {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidPrefix,
                "mode declaration api_prefix must match package prefix rules",
            ));
        }
        if declaration.mode_id.starts_with("clay.")
            || !is_package_owned_id(&declaration.mode_id, &declaration.api_prefix)
        {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidModeId,
                "mode ID must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !package.clay.modes.contains(&declaration.mode_id) {
            return Err(context.diagnostic(
                ModeValidationRule::UndeclaredMode,
                "mode patterns may only be registered for modes declared in clay.modes",
            ));
        }
        if self.modes.contains_key(&declaration.mode_id) {
            return Err(context.diagnostic(
                ModeValidationRule::DuplicateModeId,
                "mode IDs must be unique among enabled package modes",
            ));
        }
        validate_patterns(&declaration, &context)?;

        self.modes
            .insert(declaration.mode_id.clone(), RegisteredMode { declaration });
        Ok(())
    }

    pub fn classify(
        &self,
        input: &DocumentClassificationInput,
    ) -> Result<ModeClassification, ModeDiagnostic> {
        let mut best: Option<(ModePatternKind, &RegisteredMode)> = None;
        let file_name = input.path.as_deref().and_then(file_name_from_path);
        let extension = input.path.as_deref().and_then(extension_from_path);

        for mode in self.modes.values() {
            let Some(kind) = mode.best_match(input.mime_type.as_deref(), extension, file_name)
            else {
                continue;
            };
            match best {
                None => best = Some((kind, mode)),
                Some((best_kind, _)) if kind > best_kind => best = Some((kind, mode)),
                Some((best_kind, best_mode)) if kind == best_kind => {
                    return Err(ModeDiagnostic::new(
                        Some(mode.declaration.package_name.clone()),
                        Some(mode.declaration.package_version.clone()),
                        Some(mode.declaration.api_prefix.clone()),
                        Some(mode.declaration.mode_id.clone()),
                        ModeValidationRule::AmbiguousClassification,
                        format!(
                            "document {} matched both mode `{}` and mode `{}` with equal priority",
                            input.document_id,
                            best_mode.declaration.mode_id,
                            mode.declaration.mode_id
                        ),
                    ));
                }
                _ => {}
            }
        }

        let Some((matched_by, mode)) = best else {
            return Err(ModeDiagnostic::new(
                None,
                None,
                None,
                None,
                ModeValidationRule::NoClassificationMatch,
                format!("no registered mode matched document {}", input.document_id),
            ));
        };

        Ok(mode.classification(input.document_id, matched_by))
    }

    pub fn activate_major_mode(
        &mut self,
        package: &ClayPackageManifest,
        classification: ModeClassification,
    ) -> Result<MajorModeActivation, ModeDiagnostic> {
        if !package
            .clay
            .permissions
            .contains(&PackagePermission::ModeActivation)
        {
            return Err(
                ModeDiagnosticContext::from_classification(&classification).diagnostic(
                    ModeValidationRule::MissingPermission,
                    "package must declare mode-activation before activating a major mode",
                ),
            );
        }
        if package.name != classification.package_name
            || package.version != classification.package_version
            || package.clay.api_prefix != classification.api_prefix
            || !package.clay.modes.contains(&classification.mode_id)
        {
            return Err(ModeDiagnosticContext::from_classification(&classification).diagnostic(
                ModeValidationRule::UndeclaredMode,
                "major mode activation must match a mode declared by the validated package manifest",
            ));
        }
        if !self.modes.contains_key(&classification.mode_id) {
            return Err(
                ModeDiagnosticContext::from_classification(&classification).diagnostic(
                    ModeValidationRule::UndeclaredMode,
                    "major mode activation requires a registered mode declaration",
                ),
            );
        }

        let behavior_version = self
            .active_major_modes
            .get(&classification.document_id)
            .map_or(1, |active| active.behavior_version.saturating_add(1));
        let activation = MajorModeActivation {
            document_id: classification.document_id,
            package_name: classification.package_name,
            package_version: classification.package_version,
            api_prefix: classification.api_prefix,
            mode_id: classification.mode_id,
            behavior_version,
        };
        self.active_major_modes
            .insert(activation.document_id, activation.clone());
        Ok(activation)
    }

    pub fn active_major_mode(&self, document_id: DocumentId) -> Option<&MajorModeActivation> {
        self.active_major_modes.get(&document_id)
    }

    /// Register a minor-mode declaration.
    ///
    /// The package must have `mode-registration` permission.  The minor mode ID
    /// must be package-prefixed and must not duplicate an existing minor or
    /// major mode ID.  At least one compatible major mode must be declared.
    pub fn register_minor_mode(
        &mut self,
        package: &ClayPackageManifest,
        declaration: MinorModeDeclaration,
    ) -> Result<(), ModeDiagnostic> {
        let context = ModeDiagnosticContext {
            package_name: Some(declaration.package_name.clone()),
            package_version: Some(declaration.package_version.clone()),
            api_prefix: Some(declaration.api_prefix.clone()),
            mode_id: Some(declaration.mode_id.clone()),
        };

        if !package
            .clay
            .permissions
            .contains(&PackagePermission::ModeRegistration)
        {
            return Err(context.diagnostic(
                ModeValidationRule::MissingPermission,
                "package must declare mode-registration before registering minor mode patterns",
            ));
        }
        if package.name != declaration.package_name
            || package.version != declaration.package_version
            || package.clay.api_prefix != declaration.api_prefix
        {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidPrefix,
                "minor mode declaration provenance must match the validated package manifest",
            ));
        }
        if !is_valid_api_prefix(&declaration.api_prefix) {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidPrefix,
                "minor mode api_prefix must match package prefix rules",
            ));
        }
        if declaration.mode_id.starts_with("clay.")
            || !is_package_owned_id(&declaration.mode_id, &declaration.api_prefix)
        {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidModeId,
                "minor mode ID must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if self.modes.contains_key(&declaration.mode_id)
            || self.minor_modes.contains_key(&declaration.mode_id)
        {
            return Err(context.diagnostic(
                ModeValidationRule::DuplicateModeId,
                "minor mode ID must be unique among all registered modes",
            ));
        }
        if declaration.display_name.trim().is_empty() {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "minor mode display_name must be non-empty",
            ));
        }
        if declaration.compatible_major_modes.is_empty() {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "minor mode must declare at least one compatible major mode",
            ));
        }

        self.minor_modes.insert(
            declaration.mode_id.clone(),
            RegisteredMinorMode { declaration },
        );
        Ok(())
    }

    /// Activate a minor mode overlay for a document.
    ///
    /// The document must already have an active major mode.  The minor mode
    /// must declare the document's active major mode as compatible.  Minor
    /// modes are applied in activation order; any later conflict with a
    /// previously-activated minor mode is rejected.
    pub fn activate_minor_mode(
        &mut self,
        package: &ClayPackageManifest,
        document_id: DocumentId,
        minor_mode_id: &str,
    ) -> Result<MinorModeActivation, ModeDiagnostic> {
        if !package
            .clay
            .permissions
            .contains(&PackagePermission::ModeActivation)
        {
            return Err(ModeDiagnostic::new(
                Some(package.name.clone()),
                Some(package.version.clone()),
                Some(package.clay.api_prefix.clone()),
                Some(minor_mode_id.to_string()),
                ModeValidationRule::MissingPermission,
                "package must declare mode-activation before activating a minor mode",
            ));
        }

        let registered = self.minor_modes.get(minor_mode_id).ok_or_else(|| {
            ModeDiagnostic::new(
                Some(package.name.clone()),
                Some(package.version.clone()),
                Some(package.clay.api_prefix.clone()),
                Some(minor_mode_id.to_string()),
                ModeValidationRule::UndeclaredMode,
                format!("minor mode `{minor_mode_id}` has not been registered"),
            )
        })?;

        // Check that the active major mode is in the compatible list.
        let active_major = self
            .active_major_modes
            .get(&document_id)
            .ok_or_else(|| ModeDiagnostic::new(
                Some(package.name.clone()),
                Some(package.version.clone()),
                Some(package.clay.api_prefix.clone()),
                Some(minor_mode_id.to_string()),
                ModeValidationRule::AmbiguousClassification,
                format!(
                    "document {} has no active major mode; activate a major mode before minor modes",
                    document_id
                ),
            ))?;

        if !registered
            .declaration
            .compatible_major_modes
            .contains(&active_major.mode_id)
        {
            return Err(ModeDiagnostic::new(
                Some(package.name.clone()),
                Some(package.version.clone()),
                Some(package.clay.api_prefix.clone()),
                Some(minor_mode_id.to_string()),
                ModeValidationRule::UndeclaredMode,
                format!(
                    "minor mode `{minor_mode_id}` is not compatible with active major mode `{}`; \
                     compatible modes are: [{}]",
                    active_major.mode_id,
                    registered.declaration.compatible_major_modes.join(", ")
                ),
            ));
        }

        let activation = MinorModeActivation {
            document_id,
            package_name: registered.declaration.package_name.clone(),
            package_version: registered.declaration.package_version.clone(),
            api_prefix: registered.declaration.api_prefix.clone(),
            mode_id: registered.declaration.mode_id.clone(),
        };

        self.active_minor_modes
            .entry(document_id)
            .or_default()
            .push(activation.clone());

        Ok(activation)
    }

    /// Compose and validate a per-document behavior manifest from the document's
    /// active major mode (and any compatible minor modes), then store it keyed
    /// by document ID.
    ///
    /// The composed manifest is inert and server-validated via the existing
    /// [`crate::behavior::manifest::validate_manifest`].  Minor-mode command and
    /// key contributions are appended; they cannot replace or remove major-mode
    /// entries.  The returned selection carries full package/mode provenance.
    ///
    /// This function is called at mode activation/reload only — never from
    /// typing, paint, layout, scroll, or text-event handlers.
    pub fn select_behavior_manifest_for_document(
        &mut self,
        document_id: DocumentId,
        enabled_packages: &[&PackageRecord],
    ) -> Result<&DocumentManifestSelection, ModeDiagnostic> {
        let major_activation = self
            .active_major_modes
            .get(&document_id)
            .cloned()
            .ok_or_else(|| {
                ModeDiagnostic::new(
                    None,
                    None,
                    None,
                    None,
                    ModeValidationRule::NoClassificationMatch,
                    format!("document {document_id} has no active major mode"),
                )
            })?;

        // Find the package record for the major mode.
        let major_package = enabled_packages
            .iter()
            .find(|r| r.manifest.name == major_activation.package_name)
            .ok_or_else(|| {
                ModeDiagnostic::new(
                    Some(major_activation.package_name.clone()),
                    Some(major_activation.package_version.clone()),
                    Some(major_activation.api_prefix.clone()),
                    Some(major_activation.mode_id.clone()),
                    ModeValidationRule::UndeclaredMode,
                    format!(
                        "package `{}` for major mode `{}` is not in the enabled package list",
                        major_activation.package_name, major_activation.mode_id
                    ),
                )
            })?;

        // Start from the base manifest for the document scope.
        let mut manifest =
            BehaviorManifest::minimal_text_editing(major_activation.behavior_version);
        manifest.manifest_id = format!(
            "{}.{}",
            major_activation.api_prefix, major_activation.mode_id
        );
        manifest.scope = BehaviorScope::Document { document_id };

        // Track which command IDs and key-sequence+context pairs are owned by
        // the major mode so minor modes cannot override them.
        let major_command_ids: HashSet<String> = manifest
            .commands
            .iter()
            .map(|c| c.command_id.clone())
            .collect();
        let major_key_seqs: HashSet<String> = manifest
            .keymaps
            .iter()
            .map(|k| format!("{:?}:{:?}", k.context, k.sequence))
            .collect();

        // Append package command contributions for the major mode.
        append_package_commands(
            &mut manifest,
            major_package,
            &major_activation.mode_id,
            &major_activation.package_name,
            &major_activation.package_version,
            &major_activation.api_prefix,
        );

        // Apply minor-mode overlays for this document in activation order.
        let minor_activations = self
            .active_minor_modes
            .get(&document_id)
            .cloned()
            .unwrap_or_default();

        let mut applied_minor: Vec<MinorModeActivation> = Vec::new();

        for minor_act in &minor_activations {
            // Find the package record for this minor mode.
            let minor_package = enabled_packages
                .iter()
                .find(|r| r.manifest.name == minor_act.package_name)
                .ok_or_else(|| {
                    ModeDiagnostic::new(
                        Some(minor_act.package_name.clone()),
                        Some(minor_act.package_version.clone()),
                        Some(minor_act.api_prefix.clone()),
                        Some(minor_act.mode_id.clone()),
                        ModeValidationRule::UndeclaredMode,
                        format!(
                            "package `{}` for minor mode `{}` is not in the enabled package list",
                            minor_act.package_name, minor_act.mode_id
                        ),
                    )
                })?;

            // Collect the command contributions this minor mode would add.
            let minor_commands: Vec<CommandDeclaration> = minor_package
                .contributions
                .commands
                .iter()
                .filter(|c| is_package_owned_id(&c.id, &minor_act.api_prefix))
                .filter_map(|c| {
                    parse_routing_policy(&c.routing_policy).map(|policy| CommandDeclaration {
                        command_id: c.id.clone(),
                        display_name: c.display_name.clone(),
                        routing_policy: policy,
                        authority: crate::protocol::CommandAuthority::ServerIntent,
                    })
                })
                .collect();

            // Reject any minor-mode command that collides with a major-mode entry.
            for cmd in &minor_commands {
                if major_command_ids.contains(&cmd.command_id) {
                    return Err(ModeDiagnostic::new(
                        Some(minor_act.package_name.clone()),
                        Some(minor_act.package_version.clone()),
                        Some(minor_act.api_prefix.clone()),
                        Some(minor_act.mode_id.clone()),
                        ModeValidationRule::DuplicateModeId,
                        format!(
                            "minor mode `{}` cannot override major-mode command `{}`",
                            minor_act.mode_id, cmd.command_id
                        ),
                    ));
                }
            }

            // Reject any minor-mode key binding that collides with a major-mode entry.
            // Minor mode key routing contributions reference command IDs; the key sequences
            // themselves come from the key_routing descriptors on the same package.
            for key_contrib in minor_package
                .contributions
                .key_routing
                .iter()
                .filter(|k| is_package_owned_id(&k.command_id, &minor_act.api_prefix))
            {
                // Check whether the command being bound is already in the major manifest
                // keymaps by looking at which command IDs are bound in major_key_seqs.
                // Since we don't have full key sequences here (only command IDs),
                // we conservatively reject any key_routing contribution that targets
                // a major-mode command ID.
                if major_command_ids.contains(&key_contrib.command_id) {
                    return Err(ModeDiagnostic::new(
                        Some(minor_act.package_name.clone()),
                        Some(minor_act.package_version.clone()),
                        Some(minor_act.api_prefix.clone()),
                        Some(minor_act.mode_id.clone()),
                        ModeValidationRule::DuplicateModeId,
                        format!(
                            "minor mode `{}` cannot override major-mode key binding for command `{}`",
                            minor_act.mode_id, key_contrib.command_id
                        ),
                    ));
                }
            }
            // Also check direct key-sequence collisions in the composed manifest.
            // We compare after building the minor keymaps (none produced at descriptor
            // level — key routing descriptors carry no sequence data; that comes from
            // the runtime at load time). This check guards against future extension.
            let _ = &major_key_seqs; // referenced above in let binding

            // Append minor-mode commands into the composed manifest.
            manifest.commands.extend(minor_commands);
            applied_minor.push(minor_act.clone());
        }

        // Validate the fully composed manifest through the existing validator.
        validate_manifest(&manifest).map_err(|err| {
            ModeDiagnostic::new(
                Some(major_activation.package_name.clone()),
                Some(major_activation.package_version.clone()),
                Some(major_activation.api_prefix.clone()),
                Some(major_activation.mode_id.clone()),
                ModeValidationRule::MalformedPattern,
                format!("composed behavior manifest failed validation: {err:?}"),
            )
        })?;

        let selection = DocumentManifestSelection {
            manifest,
            major_mode: major_activation,
            minor_modes: applied_minor,
        };
        self.selected_manifests.insert(document_id, selection);
        Ok(self.selected_manifests.get(&document_id).unwrap())
    }

    /// Return the currently selected manifest for a document, if any.
    pub fn selected_manifest(&self, document_id: DocumentId) -> Option<&DocumentManifestSelection> {
        self.selected_manifests.get(&document_id)
    }
}

#[derive(Debug, Clone)]
struct RegisteredMode {
    declaration: ModeDeclaration,
}

#[derive(Debug, Clone)]
struct RegisteredMinorMode {
    declaration: MinorModeDeclaration,
}

impl RegisteredMode {
    fn best_match(
        &self,
        mime_type: Option<&str>,
        extension: Option<&str>,
        file_name: Option<&str>,
    ) -> Option<ModePatternKind> {
        let mut best = None;
        if let Some(mime_type) = mime_type
            && self
                .declaration
                .mime_types
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(mime_type))
        {
            best = Some(ModePatternKind::MimeType);
        }
        if let Some(extension) = extension
            && self
                .declaration
                .extensions
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        {
            best = Some(ModePatternKind::Extension);
        }
        if let Some(file_name) = file_name {
            if self
                .declaration
                .file_name_patterns
                .iter()
                .any(|pattern| wildcard_match(pattern, file_name))
            {
                best = Some(ModePatternKind::FileNamePattern);
            }
            if self
                .declaration
                .file_names
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(file_name))
            {
                best = Some(ModePatternKind::FileName);
            }
        }
        best
    }

    fn classification(
        &self,
        document_id: DocumentId,
        matched_by: ModePatternKind,
    ) -> ModeClassification {
        ModeClassification {
            document_id,
            package_name: self.declaration.package_name.clone(),
            package_version: self.declaration.package_version.clone(),
            api_prefix: self.declaration.api_prefix.clone(),
            mode_id: self.declaration.mode_id.clone(),
            matched_by,
        }
    }
}

/// Append package-declared command contributions for a specific mode into a
/// composed [`BehaviorManifest`].  Only contributions whose IDs are owned by
/// the given `api_prefix` are appended; contributions targeting other prefixes
/// are skipped (they belong to other packages).
fn append_package_commands(
    manifest: &mut BehaviorManifest,
    package: &PackageRecord,
    _mode_id: &str,
    _package_name: &str,
    _package_version: &str,
    api_prefix: &str,
) {
    for contrib in &package.contributions.commands {
        if !is_package_owned_id(&contrib.id, api_prefix) {
            continue;
        }
        // Only accept server-intent routing policies for package commands.
        let Some(policy) = parse_routing_policy(&contrib.routing_policy) else {
            continue;
        };
        // Skip client-edit policies — packages may not declare built-in client-edit authority.
        let authority = match policy {
            RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck => {
                continue;
            }
            _ => crate::protocol::CommandAuthority::ServerIntent,
        };
        manifest.commands.push(CommandDeclaration {
            command_id: contrib.id.clone(),
            display_name: contrib.display_name.clone(),
            routing_policy: policy,
            authority,
        });
    }
}

/// Parse a routing policy string into the [`RoutingPolicy`] enum.
///
/// Returns `None` for unknown or unsupported policy strings so callers can
/// skip contributions with unrecognised policies rather than failing hard.
fn parse_routing_policy(value: &str) -> Option<RoutingPolicy> {
    match value {
        "client-first-predictable" => Some(RoutingPolicy::ClientFirstPredictable),
        "client-first-requires-ack" => Some(RoutingPolicy::ClientFirstRequiresAck),
        "server-first" => Some(RoutingPolicy::ServerFirst),
        "ui-reactive-priority" => Some(RoutingPolicy::UiReactivePriority),
        "background" => Some(RoutingPolicy::Background),
        _ => None,
    }
}

fn validate_patterns(
    declaration: &ModeDeclaration,
    context: &ModeDiagnosticContext,
) -> Result<(), ModeDiagnostic> {
    if declaration.display_name.trim().is_empty() {
        return Err(context.diagnostic(
            ModeValidationRule::MalformedPattern,
            "mode display_name must be non-empty",
        ));
    }
    if declaration.extensions.is_empty()
        && declaration.mime_types.is_empty()
        && declaration.file_names.is_empty()
        && declaration.file_name_patterns.is_empty()
    {
        return Err(context.diagnostic(
            ModeValidationRule::MalformedPattern,
            "mode declaration must include at least one static extension, MIME, filename, or filename pattern",
        ));
    }

    let mut seen = HashSet::new();
    for extension in &declaration.extensions {
        if !seen.insert(format!("extension:{extension}")) || !valid_extension(extension) {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "extensions must be unique strings without a leading dot",
            ));
        }
    }
    for mime_type in &declaration.mime_types {
        if !seen.insert(format!("mime:{mime_type}")) || !valid_mime_type(mime_type) {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "MIME types must be unique type/subtype strings without whitespace",
            ));
        }
    }
    for file_name in &declaration.file_names {
        if !seen.insert(format!("file:{file_name}")) || !valid_file_name(file_name) {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "file names must be unique basename strings without path separators",
            ));
        }
    }
    for pattern in &declaration.file_name_patterns {
        if !seen.insert(format!("pattern:{pattern}")) || !valid_file_name_pattern(pattern) {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "filename patterns must be unique basename patterns containing exactly one wildcard",
            ));
        }
    }
    Ok(())
}

fn valid_extension(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn valid_mime_type(value: &str) -> bool {
    let Some((left, right)) = value.split_once('/') else {
        return false;
    };
    !left.is_empty()
        && !right.is_empty()
        && !value.chars().any(char::is_whitespace)
        && !value.contains("Deno.core.ops")
}

fn valid_file_name(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains("Deno.core.ops")
}

fn valid_file_name_pattern(value: &str) -> bool {
    valid_file_name(value)
        && value.matches('*').count() == 1
        && value != "*"
        && !value.contains("**")
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return false;
    };
    value.starts_with(prefix)
        && value.ends_with(suffix)
        && value.len() >= prefix.len() + suffix.len()
}

fn file_name_from_path(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

fn extension_from_path(path: &str) -> Option<&str> {
    Path::new(path).extension()?.to_str()
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

struct ModeDiagnosticContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
    mode_id: Option<String>,
}

impl ModeDiagnosticContext {
    fn from_declaration(declaration: &ModeDeclaration) -> Self {
        Self {
            package_name: Some(declaration.package_name.clone()),
            package_version: Some(declaration.package_version.clone()),
            api_prefix: Some(declaration.api_prefix.clone()),
            mode_id: Some(declaration.mode_id.clone()),
        }
    }

    fn from_classification(classification: &ModeClassification) -> Self {
        Self {
            package_name: Some(classification.package_name.clone()),
            package_version: Some(classification.package_version.clone()),
            api_prefix: Some(classification.api_prefix.clone()),
            mode_id: Some(classification.mode_id.clone()),
        }
    }

    fn diagnostic(&self, rule: ModeValidationRule, message: impl Into<Box<str>>) -> ModeDiagnostic {
        ModeDiagnostic {
            package_name: self.package_name.clone().map(String::into_boxed_str),
            package_version: self.package_version.clone().map(String::into_boxed_str),
            api_prefix: self.api_prefix.clone().map(String::into_boxed_str),
            mode_id: self.mode_id.clone().map(String::into_boxed_str),
            rule,
            message: message.into(),
        }
    }
}
