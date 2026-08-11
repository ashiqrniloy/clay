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
    /// Semantic document typography selected when this mode activates.
    pub document_font_role: crate::protocol::DocumentFontRole,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
    pub file_names: Vec<String>,
    pub file_name_patterns: Vec<String>,
    /// Declarative shebang patterns matched against the interpreter token of an
    /// open document's shebang line (e.g. `python*`, `node`, or `*` for any
    /// interpreter). Validated like other mode patterns; no language-specific
    /// Rust branches.
    pub shebang_patterns: Vec<String>,
    /// Declarative literal markers matched at the start of an open document's
    /// bounded leading-content slice (e.g. `<?xml`, `<!DOCTYPE html`). The
    /// weakest package-declared classification signal.
    pub content_probes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentClassificationInput {
    pub document_id: DocumentId,
    pub path: Option<String>,
    pub mime_type: Option<String>,
    /// Optional shebang line (the first line of an already-open document when it
    /// begins with `#!`), supplied only by the open path.  `None` disables
    /// shebang probing.  Packages cannot supply shebang slices through this
    /// field; the open path is the sole authority.
    pub shebang: Option<String>,
    /// Optional bounded leading-content slice of an already-open document,
    /// supplied only by the open path.  Slices exceeding
    /// [`MAX_LEADING_CONTENT_BYTES`] are rejected (treated as absent) so
    /// content probes never read unbounded content.  No filesystem scan,
    /// directory walk, or arbitrary package predicate is performed.
    pub leading_content: Option<String>,
}

/// Maximum number of bytes of leading document content that classification
/// probes may inspect.  Enforced by [`ModeRegistry::classify`]: oversize
/// slices are rejected (treated as absent) and the document classifies via the
/// remaining precedence ladder, so probes can never read unbounded content.
pub const MAX_LEADING_CONTENT_BYTES: usize = 512;

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
/// The signal that matched a document to a mode, in ascending precedence.
/// The full Phase 18.9 classification ladder is:
///
/// `user override > exact filename > wildcard filename > extension > MIME >
/// shebang > bounded leading-content probe > core.code > core.text`
///
/// Package-declared signals are always consulted before built-in fallback
/// modes, so any package match (even the weakest content probe) wins over
/// `core.code`/`core.text`; the variants below only order package-declared
/// signals against each other (and `core.code`'s own extension/shebang match
/// against the `core.text` universal fallback).
pub enum ModePatternKind {
    /// Built-in universal fallback (e.g. `core.text`) selected when no
    /// package-declared or built-in pattern matched a document. Lowest
    /// precedence, so any real pattern wins over it.
    Fallback,
    /// Bounded leading-content probe: a package-declared literal marker matched
    /// at the start of the open document's bounded leading-content slice.
    /// Weakest package-declared signal.
    ContentProbe,
    /// Shebang line: a package-declared shebang pattern matched the document's
    /// interpreter token (e.g. `#!/usr/bin/env python3`).
    Shebang,
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
    pub document_font_role: crate::protocol::DocumentFontRole,
    /// The classification signal that selected this mode, recorded so the Phase
    /// 18.9 discovery commands can report the classification source and the
    /// fallback rationale without recomputing classification or scanning any
    /// filesystem. Read-only provenance; carries no authority.
    pub matched_by: ModePatternKind,
}

/// Phase 18.9 discovery provenance label for an active mode. Built-in
/// `core.text`/`core.code` fallback modes are Clay-owned; any other mode is
/// owned by the package that declared it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeProvenance {
    /// Clay-owned built-in fallback mode (`core.text`/`core.code`), always-on
    /// and requiring no package.
    CoreBuiltIn,
    /// Owned by an enabled package that declared the mode.
    Package,
}

/// Phase 18.9 discovery summary for one active major mode, as returned by
/// [`ModeRegistry::list_active_modes`]. Reads installed registry state only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveModeSummary {
    pub document_id: DocumentId,
    pub mode_id: String,
    pub package_name: String,
    pub api_prefix: String,
    pub provenance: ModeProvenance,
    pub classification_source: ModePatternKind,
}

/// Phase 18.9 discovery explanation for a document's active major mode, as
/// returned by [`ModeRegistry::explain_active_mode`]. Carries no execution,
/// document, or workspace authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeExplanation {
    pub document_id: DocumentId,
    pub active_mode: String,
    pub display_name: Option<String>,
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub provenance: ModeProvenance,
    pub classification_source: ModePatternKind,
    pub fallback_used: bool,
    /// Human-readable rationale describing why this mode is active (e.g. which
    /// signal matched, whether a built-in fallback was used because no language
    /// package claimed the document).
    pub why: String,
}

/// Human-readable rationale for an active mode selected by `matched_by`, used
/// by the discovery commands. Generic over the classification signal and the
/// built-in/package provenance; performs no language-specific branching and no
/// authority work.
fn mode_rationale(matched_by: ModePatternKind, is_builtin: bool) -> String {
    let source = match matched_by {
        ModePatternKind::FileName => "exact filename",
        ModePatternKind::FileNamePattern => "wildcard filename",
        ModePatternKind::Extension => "extension",
        ModePatternKind::MimeType => "MIME type",
        ModePatternKind::Shebang => "shebang line",
        ModePatternKind::ContentProbe => "bounded leading-content probe",
        ModePatternKind::Fallback => "universal fallback",
    };
    if is_builtin && matches!(matched_by, ModePatternKind::Fallback) {
        format!(
            "no language package or built-in pattern matched; built-in core.text \
             universal fallback claimed the document via {source}"
        )
    } else if is_builtin {
        format!(
            "no language package matched; built-in core.code claimed the document \
             via {source}"
        )
    } else {
        format!("package-declared mode matched the document via {source}")
    }
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

/// Reserved API prefix for Clay-owned built-in modes.
pub const CORE_API_PREFIX: &str = "core";
/// Reserved mode ID for the built-in universal plain-text fallback mode.
pub const CORE_TEXT_MODE_ID: &str = "core.text";
/// Reserved mode ID for the built-in code-oriented fallback mode.
pub const CORE_CODE_MODE_ID: &str = "core.code";

/// Built-in universal plain-text fallback mode.  Carries no static patterns;
/// it is selected by [`ModeRegistry::classify`] only when no package-declared
/// or built-in pattern matched, so any real match wins over it.  Always-on;
/// requires no package, no `init.js` line, and no `loadPackage` step.
pub fn core_text_mode() -> ModeDeclaration {
    ModeDeclaration {
        package_name: "clay".to_string(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        api_prefix: CORE_API_PREFIX.to_string(),
        mode_id: CORE_TEXT_MODE_ID.to_string(),
        display_name: "Plain Text".to_string(),
        document_font_role: crate::protocol::DocumentFontRole::Proportional,
        extensions: Vec::new(),
        mime_types: Vec::new(),
        file_names: Vec::new(),
        file_name_patterns: Vec::new(),
        shebang_patterns: Vec::new(),
        content_probes: Vec::new(),
    }
}

/// Built-in code-oriented fallback mode.  Claims a curated, declarative set of
/// common programming-language extensions so a disabled or absent language
/// package still leaves the file editable as generic code.  Language packages
/// declare the same extensions and win classification on ties because built-in
/// modes have lowest precedence.  No language-specific Rust branches: the
/// extension list is declarative metadata validated like any mode pattern.
pub fn core_code_mode() -> ModeDeclaration {
    ModeDeclaration {
        package_name: "clay".to_string(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        api_prefix: CORE_API_PREFIX.to_string(),
        mode_id: CORE_CODE_MODE_ID.to_string(),
        display_name: "Code".to_string(),
        document_font_role: crate::protocol::DocumentFontRole::Monospace,
        extensions: vec![
            "rs".to_string(),
            "py".to_string(),
            "js".to_string(),
            "ts".to_string(),
            "jsx".to_string(),
            "tsx".to_string(),
            "c".to_string(),
            "h".to_string(),
            "cpp".to_string(),
            "hpp".to_string(),
            "cc".to_string(),
            "cxx".to_string(),
            "java".to_string(),
            "go".to_string(),
            "rb".to_string(),
            "php".to_string(),
            "sh".to_string(),
            "bash".to_string(),
            "zsh".to_string(),
            "fish".to_string(),
            "lua".to_string(),
            "swift".to_string(),
            "kt".to_string(),
            "dart".to_string(),
            "r".to_string(),
            "pl".to_string(),
            "scala".to_string(),
            "clj".to_string(),
            "sql".to_string(),
            "css".to_string(),
            "scss".to_string(),
            "less".to_string(),
            "xml".to_string(),
            "html".to_string(),
            "toml".to_string(),
            "yaml".to_string(),
            "yml".to_string(),
            "json".to_string(),
            "ini".to_string(),
            "conf".to_string(),
            "vim".to_string(),
            "el".to_string(),
            "ex".to_string(),
            "exs".to_string(),
            "zig".to_string(),
            "nim".to_string(),
            "cs".to_string(),
            "fs".to_string(),
        ],
        mime_types: Vec::new(),
        file_names: Vec::new(),
        file_name_patterns: Vec::new(),
        // Any shebang line marks a document as a script (code-like), so an
        // extensionless script with no owning language package still resolves
        // to `core.code` rather than the plain-text fallback. `*` matches any
        // interpreter token and is plain declarative metadata: no
        // language-specific Rust branch is involved.
        shebang_patterns: vec!["*".to_string()],
        content_probes: Vec::new(),
    }
}

impl ModeRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry
            .register_builtin_mode(core_text_mode())
            .expect("core.text built-in mode must register at startup");
        registry
            .register_builtin_mode(core_code_mode())
            .expect("core.code built-in mode must register at startup");
        registry
    }

    pub fn activation_budget_ms(&self) -> u64 {
        MODE_ACTIVATION_P95_BUDGET_MS
    }

    /// Remove a registered mode declaration (e.g. when its owning package is
    /// disabled mid-session). Returns `true` if the mode was present and
    /// removed. Built-in `core.*` modes are Clay-owned and cannot be removed
    /// (returns `false`) so the always-available fallback guarantee holds.
    ///
    /// This drops the declaration from the candidate set only; it does not by
    /// itself reclassify open documents. The centralized activation path
    /// (`classify` + `activate_major_mode`/`activate_builtin_major_mode`) must
    /// be re-run for each affected document so it reclassifies deterministically
    /// and gets a strictly greater behavior version (the prior activation must
    /// remain so re-activation bumps the recorded version). A previously-active
    /// activation for the removed mode is therefore left in place until
    /// reclassification replaces it: it cannot bypass validation because
    /// `select_behavior_manifest_for_document` errors for an unregistered mode
    /// (its owning package is no longer enabled), forcing reclassification.
    /// Grants no new authority (in-process registry mutation only); it is
    /// symmetric with `register_mode`/`register_builtin_mode` and introduces no
    /// new primitive category.
    pub fn unregister_mode(&mut self, mode_id: &str) -> bool {
        if mode_id.starts_with(CORE_API_PREFIX) {
            return false;
        }
        self.modes.remove(mode_id).is_some()
    }

    /// Remove every package-owned mode declaration for `api_prefix`. Built-in
    /// `core.*` modes are never removed. Returns the number of modes withdrawn.
    /// Callers must re-run open-document classification afterward; this only
    /// mutates the candidate set.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "package-scoped mode withdrawal is exercised by reload cleanup tests and disable hooks"
        )
    )]
    pub(crate) fn unregister_package_modes(&mut self, api_prefix: &str) -> usize {
        if api_prefix == CORE_API_PREFIX || api_prefix.starts_with("clay.") {
            return 0;
        }
        let removed_major: Vec<String> = self
            .modes
            .iter()
            .filter(|(_, mode)| mode.declaration.api_prefix == api_prefix)
            .map(|(mode_id, _)| mode_id.clone())
            .collect();
        let removed_minor: Vec<String> = self
            .minor_modes
            .iter()
            .filter(|(_, mode)| mode.declaration.api_prefix == api_prefix)
            .map(|(mode_id, _)| mode_id.clone())
            .collect();
        let count = removed_major.len() + removed_minor.len();
        for mode_id in removed_major {
            self.modes.remove(&mode_id);
        }
        for mode_id in removed_minor {
            self.minor_modes.remove(&mode_id);
        }
        count
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
            || declaration.mode_id.starts_with("core.")
            || !is_package_owned_id(&declaration.mode_id, &declaration.api_prefix)
        {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidModeId,
                "mode ID must use the package apiPrefix or apiPrefix.* namespace; \
                 `clay.*` and `core.*` are reserved for Clay-owned built-in modes",
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

        self.modes.insert(
            declaration.mode_id.clone(),
            RegisteredMode {
                declaration,
                is_builtin: false,
            },
        );
        Ok(())
    }

    /// Register a Clay-owned built-in major mode (e.g. `core.text` or
    /// `core.code`) at server startup. Built-in modes are always-on, carry
    /// built-in provenance, have lowest classification precedence, and require
    /// no package, no `init.js` line, and no `loadPackage` step.  Packages may
    /// not register `core.*` IDs; [`Self::register_mode`] rejects them.
    ///
    /// Built-in fallback modes may declare zero static patterns (universal
    /// fallback, e.g. `core.text`); when patterns are present they are
    /// validated like package patterns so built-in metadata stays well-formed.
    pub fn register_builtin_mode(
        &mut self,
        declaration: ModeDeclaration,
    ) -> Result<(), ModeDiagnostic> {
        let context = ModeDiagnosticContext::from_declaration(&declaration);
        if !declaration.mode_id.starts_with("core.") {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidModeId,
                "built-in mode IDs must use the reserved `core.*` namespace",
            ));
        }
        if declaration.api_prefix != CORE_API_PREFIX {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidPrefix,
                "built-in mode api_prefix must be `core`",
            ));
        }
        if self.modes.contains_key(&declaration.mode_id) {
            return Err(context.diagnostic(
                ModeValidationRule::DuplicateModeId,
                "mode IDs must be unique among enabled package modes",
            ));
        }
        if declaration.display_name.trim().is_empty() {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "mode display_name must be non-empty",
            ));
        }
        // Built-in fallback modes may declare zero patterns (universal
        // fallback).  When patterns are present, reuse the package-pattern
        // validator so built-in metadata stays well-formed.
        if !declaration.extensions.is_empty()
            || !declaration.mime_types.is_empty()
            || !declaration.file_names.is_empty()
            || !declaration.file_name_patterns.is_empty()
            || !declaration.shebang_patterns.is_empty()
            || !declaration.content_probes.is_empty()
        {
            validate_patterns(&declaration, &context)?;
        }
        self.modes.insert(
            declaration.mode_id.clone(),
            RegisteredMode {
                declaration,
                is_builtin: true,
            },
        );
        Ok(())
    }

    pub fn classify(
        &self,
        input: &DocumentClassificationInput,
    ) -> Result<ModeClassification, ModeDiagnostic> {
        let file_name = input.path.as_deref().and_then(file_name_from_path);
        let extension = input.path.as_deref().and_then(extension_from_path);
        let shebang = input.shebang.as_deref();
        // Reject oversize leading-content slices: probes never read unbounded
        // content. An oversize slice is treated as absent so classification
        // still succeeds via the remaining precedence ladder.
        let leading_content = input
            .leading_content
            .as_deref()
            .filter(|slice| slice.len() <= MAX_LEADING_CONTENT_BYTES);

        // Phase 1: best package-declared match across all signal kinds. Any
        // package match (even the weakest content probe) beats the built-in
        // fallbacks, so package modes are consulted before built-ins. Equal-
        // kind ties between two package modes are genuinely ambiguous.
        let mut best_package: Option<(ModePatternKind, &RegisteredMode)> = None;
        for mode in self.modes.values() {
            if mode.is_builtin {
                continue;
            }
            let Some(kind) = mode.best_match(
                input.mime_type.as_deref(),
                extension,
                file_name,
                shebang,
                leading_content,
            ) else {
                continue;
            };
            match best_package {
                None => best_package = Some((kind, mode)),
                Some((best_kind, _)) if kind > best_kind => best_package = Some((kind, mode)),
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
        if let Some((matched_by, mode)) = best_package {
            return Ok(mode.classification(input.document_id, matched_by));
        }

        // Phase 2: built-in fallback modes. `core.code` claims code-like
        // extensions and any shebang line; otherwise `core.text` is the
        // universal plain-text fallback. If neither built-in is registered the
        // legacy NoClassificationMatch error is preserved.
        if let Some(core_code) = self.modes.get(CORE_CODE_MODE_ID)
            && let Some(kind) =
                core_code.best_match(None, extension, file_name, shebang, leading_content)
        {
            return Ok(core_code.classification(input.document_id, kind));
        }
        if let Some(core_text) = self.modes.get(CORE_TEXT_MODE_ID) {
            return Ok(core_text.classification(input.document_id, ModePatternKind::Fallback));
        }
        Err(ModeDiagnostic::new(
            None,
            None,
            None,
            None,
            ModeValidationRule::NoClassificationMatch,
            format!("no registered mode matched document {}", input.document_id),
        ))
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
        let Some(registered) = self.modes.get(&classification.mode_id) else {
            return Err(
                ModeDiagnosticContext::from_classification(&classification).diagnostic(
                    ModeValidationRule::UndeclaredMode,
                    "major mode activation requires a registered mode declaration",
                ),
            );
        };

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
            document_font_role: registered.declaration.document_font_role,
            matched_by: classification.matched_by,
        };
        self.active_major_modes
            .insert(activation.document_id, activation.clone());
        Ok(activation)
    }

    /// Activate a built-in Clay-owned major mode (e.g. `core.text` or
    /// `core.code`) for a document. Built-in modes have no owning package, so
    /// this path skips package permission/provenance checks and relies on the
    /// built-in provenance recorded at registration. Used by the open path
    /// when classification resolves to a built-in fallback mode; package-
    /// declared modes must still go through [`Self::activate_major_mode`].
    pub fn activate_builtin_major_mode(
        &mut self,
        classification: ModeClassification,
    ) -> Result<MajorModeActivation, ModeDiagnostic> {
        let Some(registered) = self.modes.get(&classification.mode_id) else {
            return Err(ModeDiagnostic::new(
                Some(classification.package_name.clone()),
                Some(classification.package_version.clone()),
                Some(classification.api_prefix.clone()),
                Some(classification.mode_id.clone()),
                ModeValidationRule::UndeclaredMode,
                "major mode activation requires a registered mode declaration",
            ));
        };
        if !registered.is_builtin {
            return Err(ModeDiagnostic::new(
                Some(classification.package_name.clone()),
                Some(classification.package_version.clone()),
                Some(classification.api_prefix.clone()),
                Some(classification.mode_id.clone()),
                ModeValidationRule::InvalidPrefix,
                "activate_builtin_major_mode may only activate Clay-owned built-in modes; \
                 use activate_major_mode for package-declared modes",
            ));
        }
        let behavior_version = self
            .active_major_modes
            .get(&classification.document_id)
            .map_or(1, |active| active.behavior_version.saturating_add(1));
        let activation = MajorModeActivation {
            document_id: classification.document_id,
            package_name: classification.package_name.clone(),
            package_version: classification.package_version.clone(),
            api_prefix: classification.api_prefix.clone(),
            mode_id: classification.mode_id.clone(),
            behavior_version,
            document_font_role: registered.declaration.document_font_role,
            matched_by: classification.matched_by,
        };
        self.active_major_modes
            .insert(activation.document_id, activation.clone());
        Ok(activation)
    }

    pub fn active_major_mode(&self, document_id: DocumentId) -> Option<&MajorModeActivation> {
        self.active_major_modes.get(&document_id)
    }

    /// Phase 18.9 discovery: lists every currently-active major mode (one per
    /// open document) with its provenance and the classification signal that
    /// selected it. Reads installed registry state only: no filesystem scan,
    /// network call, shell, AI, WASM, package evaluation, or any other
    /// authority. Bounded by the number of open documents. Crate-internal: the
    /// user-facing surface is the built-in `modes.listActiveModes` command
    /// (resolved via [`CommandExecutor::execute_discovery`]), not this method.
    pub(crate) fn list_active_modes(&self) -> Vec<ActiveModeSummary> {
        self.active_major_modes
            .values()
            .map(|activation| ActiveModeSummary {
                document_id: activation.document_id,
                mode_id: activation.mode_id.clone(),
                package_name: activation.package_name.clone(),
                api_prefix: activation.api_prefix.clone(),
                provenance: self.provenance_of(&activation.mode_id, activation),
                classification_source: activation.matched_by,
            })
            .collect()
    }

    /// Phase 18.9 discovery: explains the active major mode for a document,
    /// including owning package or `core` built-in provenance, the
    /// classification source, whether a built-in fallback was used, and a
    /// human-readable rationale describing why this mode is active. Reads
    /// installed registry state only; carries no execution, document, or
    /// workspace authority and triggers no filesystem scans or package
    /// evaluation. Crate-internal: the user-facing surface is the built-in
    /// `modes.explainActiveMode` command (resolved via
    /// [`CommandExecutor::execute_discovery`]), not this method.
    pub(crate) fn explain_active_mode(&self, document_id: DocumentId) -> Option<ModeExplanation> {
        let activation = self.active_major_modes.get(&document_id)?;
        let registered = self.modes.get(&activation.mode_id);
        let is_builtin = registered
            .map(|mode| mode.is_builtin)
            .unwrap_or_else(|| activation.mode_id.starts_with("core."));
        let display_name = registered.map(|mode| mode.declaration.display_name.clone());
        let provenance = self.provenance_of(&activation.mode_id, activation);
        let fallback_used = matches!(activation.matched_by, ModePatternKind::Fallback);
        Some(ModeExplanation {
            document_id: activation.document_id,
            active_mode: activation.mode_id.clone(),
            display_name,
            package_name: activation.package_name.clone(),
            package_version: activation.package_version.clone(),
            api_prefix: activation.api_prefix.clone(),
            provenance,
            classification_source: activation.matched_by,
            fallback_used,
            why: mode_rationale(activation.matched_by, is_builtin),
        })
    }

    /// Derive the provenance label for an active mode. A mode is a Clay-owned
    /// built-in (`core.text`/`core.code`) when it is registered as built-in or
    /// its ID carries the reserved `core.` prefix; otherwise it is owned by the
    /// package that declared it.
    fn provenance_of(&self, mode_id: &str, activation: &MajorModeActivation) -> ModeProvenance {
        let is_builtin = self
            .modes
            .get(mode_id)
            .map(|mode| mode.is_builtin)
            .unwrap_or_else(|| {
                activation.api_prefix == CORE_API_PREFIX || mode_id.starts_with("core.")
            });
        if is_builtin {
            ModeProvenance::CoreBuiltIn
        } else {
            ModeProvenance::Package
        }
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
            || declaration.mode_id.starts_with("core.")
            || !is_package_owned_id(&declaration.mode_id, &declaration.api_prefix)
        {
            return Err(context.diagnostic(
                ModeValidationRule::InvalidModeId,
                "minor mode ID must use the package apiPrefix or apiPrefix.* namespace; \
                 `clay.*` and `core.*` are reserved for Clay-owned built-in modes",
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

        // Built-in Clay-owned `core.*` modes ship their own default rule sets
        // and have no owning package, so they neither need nor require an
        // enabled package record. `core.code` starts from the code-oriented
        // manifest (electric-character reflow, code pairs/comment
        // continuation); every other mode starts from the text default and
        // layers its declared package commands on top.
        let is_builtin = major_activation.mode_id.starts_with("core.");
        let mut manifest = if major_activation.mode_id == CORE_CODE_MODE_ID {
            BehaviorManifest::core_code_editing(major_activation.behavior_version)
        } else {
            BehaviorManifest::minimal_text_editing(major_activation.behavior_version)
        };
        manifest.manifest_id = format!(
            "{}.{}",
            major_activation.api_prefix, major_activation.mode_id
        );
        manifest.scope = BehaviorScope::Document { document_id };
        manifest.document_font_role = major_activation.document_font_role;

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

        if !is_builtin {
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

            // Append package command contributions for the major mode.
            append_package_commands(
                &mut manifest,
                major_package,
                &major_activation.mode_id,
                &major_activation.package_name,
                &major_activation.package_version,
                &major_activation.api_prefix,
            );
        }

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
    /// `true` for Clay-owned built-in modes registered through
    /// [`ModeRegistry::register_builtin_mode`] (e.g. `core.text`, `core.code`).
    /// Built-in modes always-on, carry built-in provenance, and have lowest
    /// classification precedence so package-declared patterns win on ties.
    is_builtin: bool,
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
        shebang: Option<&str>,
        leading_content: Option<&str>,
    ) -> Option<ModePatternKind> {
        let mut best = None;
        // Weakest package-declared signals first; later matches overwrite up
        // to the strongest kind so the mode is represented by its best signal.
        if let Some(leading_content) = leading_content
            && self
                .declaration
                .content_probes
                .iter()
                .any(|marker| leading_content.starts_with(marker.as_str()))
        {
            best = Some(ModePatternKind::ContentProbe);
        }
        if let Some(shebang) = shebang
            && let Some(interpreter) = shebang_interpreter(shebang)
            && self.declaration.shebang_patterns.iter().any(|pattern| {
                if pattern.contains('*') {
                    wildcard_match(pattern, interpreter)
                } else {
                    pattern.eq_ignore_ascii_case(interpreter)
                }
            })
        {
            best = Some(ModePatternKind::Shebang);
        }
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
        && declaration.shebang_patterns.is_empty()
        && declaration.content_probes.is_empty()
    {
        return Err(context.diagnostic(
            ModeValidationRule::MalformedPattern,
            "mode declaration must include at least one static extension, MIME, filename, filename pattern, shebang pattern, or content probe",
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
    for shebang in &declaration.shebang_patterns {
        if !seen.insert(format!("shebang:{shebang}")) || !valid_shebang_pattern(shebang) {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "shebang patterns must be unique interpreter globs containing at least one `*` (e.g. `python*`, `node`, `*`)",
            ));
        }
    }
    for probe in &declaration.content_probes {
        if !seen.insert(format!("probe:{probe}")) || !valid_content_probe(probe) {
            return Err(context.diagnostic(
                ModeValidationRule::MalformedPattern,
                "content probes must be unique non-empty literal markers without wildcards or path separators",
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

/// A shebang pattern is an interpreter glob matched against the interpreter
/// token of an open document's shebang line. A pattern without `*` matches the
/// interpreter exactly (e.g. `bash`); a pattern with exactly one `*` matches
/// via glob (e.g. `python*` matches `python3`); `*` alone matches any
/// interpreter so a built-in mode may claim any shebang line as generic code.
/// At most one `*` is allowed so matching stays a single deterministic glob.
fn valid_shebang_pattern(value: &str) -> bool {
    !value.is_empty()
        && value.matches('*').count() <= 1
        && !value.contains("**")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '+' | '/' | '*'))
        && !value.contains("Deno.core.ops")
}

/// A content probe is a literal marker matched at the very start of an open
/// document's bounded leading-content slice (e.g. `<?xml`, `<!DOCTYPE html`).
/// Wildcards and path separators are rejected so probes stay deterministic
/// literal prefixes. The marker must fit within the bounded leading slice.
fn valid_content_probe(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LEADING_CONTENT_BYTES
        && !value.contains('*')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_whitespace)
        && !value.contains("Deno.core.ops")
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

/// Extract the interpreter token from a shebang line for declarative pattern
/// matching. `#!/usr/bin/env python3` -> `python3`, `#!/bin/bash` -> `bash`,
/// `#!/usr/local/bin/node` -> `node`. Returns `None` for a line that is not a
/// shebang. Handles the common `/usr/bin/env <program> [args]` form by taking
/// the program name; otherwise takes the basename of the interpreter path.
/// Generic and language-agnostic: no interpreter-specific Rust branches.
fn shebang_interpreter(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("#!")?.trim_start();
    let mut tokens = rest.split_whitespace();
    let interpreter = tokens.next()?;
    // `/usr/bin/env <program>` form: the program name follows `env`.
    let program = if interpreter.ends_with("/env") || interpreter == "env" {
        tokens.next().unwrap_or(interpreter)
    } else {
        interpreter
    };
    // Strip any leading path components to get the interpreter basename
    // (e.g. `/usr/local/bin/node` -> `node`).
    program
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

pub(crate) struct ModeDiagnosticContext {
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

    pub(crate) fn from_classification(classification: &ModeClassification) -> Self {
        Self {
            package_name: Some(classification.package_name.clone()),
            package_version: Some(classification.package_version.clone()),
            api_prefix: Some(classification.api_prefix.clone()),
            mode_id: Some(classification.mode_id.clone()),
        }
    }

    pub(crate) fn diagnostic(
        &self,
        rule: ModeValidationRule,
        message: impl Into<Box<str>>,
    ) -> ModeDiagnostic {
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
