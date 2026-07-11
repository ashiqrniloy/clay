use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    sync::{Arc, Mutex},
};

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

const MAX_SYNTAX_HIGHLIGHT_SPANS: usize = 128;

use crate::{
    packages::{
        modes::{DocumentClassificationInput, MajorModeActivation},
        record::{PackageRecord, SyntaxGrammarContributionDescriptor},
    },
    perf::budgets::{DECORATION_PAYLOAD_BUDGET_BYTES, INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES},
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentId,
        IncrementalParseUpdate, Modifiers, ParseEditNotification, ParseUnit, TokenType,
    },
    server::{
        decorations::{DecorationValidationError, SyntaxChunkCache, validate_decoration_set},
        parse_coordinator::{ParseCoordinatorError, ParseHandlerFuture},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxEngineTier {
    Native,
    Wasm,
    JavaScriptFallback,
}

impl SyntaxEngineTier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Wasm => "wasm",
            Self::JavaScriptFallback => "javascript",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxGrammarContribution {
    pub engine_tier: SyntaxEngineTier,
    pub package_name: String,
    pub package_version: String,
    pub package_prefix: String,
    pub id: String,
    pub language_id: String,
    pub extensions: Vec<String>,
    pub file_names: Vec<String>,
    pub grammar_kind: String,
    pub grammar_path: String,
    pub grammar_source: Option<String>,
    pub highlights_query_path: String,
    pub locals_query_path: Option<String>,
    pub injections_query_path: Option<String>,
    pub style_map: BTreeMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub max_window_bytes: Option<usize>,
    pub estimated_payload_bytes: usize,
}

impl SyntaxGrammarContribution {
    fn from_descriptor(
        package: &PackageRecord,
        descriptor: &SyntaxGrammarContributionDescriptor,
    ) -> Self {
        Self {
            engine_tier: SyntaxEngineTier::Wasm,
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
            id: descriptor.id.clone(),
            language_id: descriptor.language_id.clone(),
            extensions: descriptor.extensions.clone(),
            file_names: descriptor.file_names.clone(),
            grammar_kind: descriptor.grammar_kind.clone(),
            grammar_path: descriptor.grammar_path.clone(),
            grammar_source: descriptor.grammar_source.clone(),
            highlights_query_path: descriptor.highlights_query_path.clone(),
            locals_query_path: descriptor.locals_query_path.clone(),
            injections_query_path: descriptor.injections_query_path.clone(),
            style_map: descriptor.style_map.clone(),
            timeout_ms: descriptor.timeout_ms,
            max_window_bytes: descriptor.max_window_bytes,
            estimated_payload_bytes: descriptor.estimated_payload_bytes,
        }
    }

    pub fn provenance(&self) -> DecorationProvenance {
        DecorationProvenance {
            package_name: self.package_name.clone(),
            package_version: self.package_version.clone(),
            package_prefix: self.package_prefix.clone(),
        }
    }

    fn timeout_micros(&self) -> u64 {
        self.timeout_ms.unwrap_or(5_000).saturating_mul(1_000)
    }

    fn max_window_bytes(&self) -> usize {
        self.max_window_bytes
            .unwrap_or(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
    }

    pub fn web_tree_sitter_artifact_contract(
        &self,
    ) -> Result<WebTreeSitterArtifactContract, WebTreeSitterArtifactError> {
        if self.engine_tier != SyntaxEngineTier::Wasm || self.grammar_kind != "tree-sitter-wasm" {
            return Err(WebTreeSitterArtifactError::NotWasmTier {
                kind: self.grammar_kind.clone(),
            });
        }
        validate_wasm_path(&self.grammar_path)
            .map_err(|path| WebTreeSitterArtifactError::GrammarPathNotConfined { path })?;
        validate_query_path(&self.highlights_query_path)
            .map_err(|path| WebTreeSitterArtifactError::QueryPathNotConfined { path })?;
        for path in [&self.locals_query_path, &self.injections_query_path]
            .into_iter()
            .flatten()
        {
            validate_query_path(path)
                .map_err(|path| WebTreeSitterArtifactError::QueryPathNotConfined { path })?;
        }
        Ok(WebTreeSitterArtifactContract {
            contribution_id: self.id.clone(),
            package_name: self.package_name.clone(),
            package_prefix: self.package_prefix.clone(),
            grammar_path: self.grammar_path.clone(),
            highlights_query_path: self.highlights_query_path.clone(),
            locals_query_path: self.locals_query_path.clone(),
            injections_query_path: self.injections_query_path.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebTreeSitterArtifactError {
    NotWasmTier { kind: String },
    GrammarPathNotConfined { path: String },
    QueryPathNotConfined { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebTreeSitterArtifactContract {
    pub contribution_id: String,
    pub package_name: String,
    pub package_prefix: String,
    pub grammar_path: String,
    pub highlights_query_path: String,
    pub locals_query_path: Option<String>,
    pub injections_query_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxGrammarRegistryError {
    DuplicateContributionId {
        id: String,
    },
    DuplicateLanguage {
        language_id: String,
        existing_package_prefix: String,
        new_package_prefix: String,
    },
    DuplicateExtension {
        extension: String,
        existing_language_id: String,
        new_language_id: String,
    },
    DuplicateFileName {
        file_name: String,
        existing_language_id: String,
        new_language_id: String,
    },
    InvalidEnginePreference {
        target: String,
        tier: String,
    },
}

#[derive(Clone, Copy)]
pub struct NativeGrammarDescriptor {
    package_name: &'static str,
    package_version: &'static str,
    package_prefix: &'static str,
    id: &'static str,
    language_id: &'static str,
    extensions: &'static [&'static str],
    file_names: &'static [&'static str],
    grammar_source: &'static str,
    highlights_query_path: &'static str,
    style_map: &'static [(&'static str, &'static str)],
    language: fn() -> Language,
}

impl fmt::Debug for NativeGrammarDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeGrammarDescriptor")
            .field("package_name", &self.package_name)
            .field("package_version", &self.package_version)
            .field("package_prefix", &self.package_prefix)
            .field("id", &self.id)
            .field("language_id", &self.language_id)
            .field("extensions", &self.extensions)
            .field("file_names", &self.file_names)
            .field("grammar_source", &self.grammar_source)
            .field("highlights_query_path", &self.highlights_query_path)
            .finish_non_exhaustive()
    }
}

const FIRST_PARTY_NATIVE_GRAMMARS: &[NativeGrammarDescriptor] = &[
    NativeGrammarDescriptor {
        package_name: "@clay/rust",
        package_version: "builtin",
        package_prefix: "rust",
        id: "rust.rust",
        language_id: "rust",
        extensions: &["rs"],
        file_names: &[],
        grammar_source: "tree-sitter-rust",
        highlights_query_path: "packages/rust/queries/highlights.scm",
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_rust::LANGUAGE.into(),
    },
    NativeGrammarDescriptor {
        package_name: "@clay/typescript",
        package_version: "builtin",
        package_prefix: "typescript",
        id: "typescript.typescript",
        language_id: "typescript",
        extensions: &["ts"],
        file_names: &[],
        grammar_source: "tree-sitter-typescript",
        highlights_query_path: "packages/typescript/queries/highlights.scm",
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    },
    NativeGrammarDescriptor {
        package_name: "@clay/typescript",
        package_version: "builtin",
        package_prefix: "typescript",
        id: "typescript.tsx",
        language_id: "tsx",
        extensions: &["tsx"],
        file_names: &[],
        grammar_source: "tree-sitter-typescript",
        highlights_query_path: "packages/typescript/queries/highlights.scm",
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    },
    NativeGrammarDescriptor {
        package_name: "@clay/javascript",
        package_version: "builtin",
        package_prefix: "javascript",
        id: "javascript.javascript",
        language_id: "javascript",
        extensions: &["js", "jsx", "mjs", "cjs"],
        file_names: &[],
        grammar_source: "tree-sitter-javascript",
        highlights_query_path: "packages/javascript/queries/highlights.scm",
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_javascript::LANGUAGE.into(),
    },
    NativeGrammarDescriptor {
        package_name: "@clay/markdown",
        package_version: "builtin",
        package_prefix: "markdown",
        id: "markdown.markdown",
        language_id: "markdown",
        extensions: &["md", "markdown", "mdown"],
        file_names: &[],
        grammar_source: "tree-sitter-md-025",
        highlights_query_path: "packages/markdown/queries/highlights.scm",
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_md_025::LANGUAGE.into(),
    },
];

const DEFAULT_NATIVE_STYLE_MAP: &[(&str, &str)] = &[
    ("keyword", "keyword.control"),
    ("string", "string.quoted"),
    ("comment", "comment.line"),
    ("punctuation", "punctuation.definition"),
    ("text", "text"),
];

#[derive(Debug, Clone, Default)]
pub struct SyntaxGrammarRegistry {
    grammars_by_id: BTreeMap<String, SyntaxGrammarContribution>,
    native_descriptors_by_id: BTreeMap<String, NativeGrammarDescriptor>,
    language_to_id: BTreeMap<String, String>,
    extension_to_id: BTreeMap<String, String>,
    file_name_to_id: BTreeMap<String, String>,
    active_selections: BTreeMap<DocumentId, SyntaxGrammarSelection>,
    engine_preferences: BTreeMap<String, SyntaxEngineTier>,
}

impl SyntaxGrammarRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_first_party_native() -> Self {
        let mut registry = Self::new();
        registry
            .register_first_party_native_grammars()
            .expect("first-party native syntax grammar descriptors must not conflict");
        registry
    }

    pub fn first_party_native_descriptors() -> &'static [NativeGrammarDescriptor] {
        FIRST_PARTY_NATIVE_GRAMMARS
    }

    pub fn register_first_party_native_grammars(
        &mut self,
    ) -> Result<usize, SyntaxGrammarRegistryError> {
        let mut staged: Vec<(SyntaxGrammarContribution, NativeGrammarDescriptor)> =
            Vec::with_capacity(FIRST_PARTY_NATIVE_GRAMMARS.len());
        for descriptor in FIRST_PARTY_NATIVE_GRAMMARS {
            let contribution = contribution_from_native_descriptor(descriptor);
            self.validate_no_conflict(&contribution)?;
            for (staged_contribution, _) in &staged {
                self.validate_pair_no_conflict(staged_contribution, &contribution)?;
            }
            staged.push((contribution, *descriptor));
        }

        let count = staged.len();
        for (contribution, descriptor) in staged {
            self.insert_contribution(contribution);
            self.native_descriptors_by_id
                .insert(descriptor.id.to_string(), descriptor);
        }
        Ok(count)
    }

    pub fn native_language(&self, contribution_id: &str) -> Option<Language> {
        self.native_descriptors_by_id
            .get(contribution_id)
            .map(|descriptor| (descriptor.language)())
    }

    pub fn register_package(
        &mut self,
        package: &PackageRecord,
    ) -> Result<usize, SyntaxGrammarRegistryError> {
        self.register_package_inner(package, false)
    }

    pub fn set_engine_preference(
        &mut self,
        target: &str,
        tier: SyntaxEngineTier,
    ) -> Result<(), SyntaxGrammarRegistryError> {
        let normalized = normalize_engine_preference_target(target).ok_or_else(|| {
            SyntaxGrammarRegistryError::InvalidEnginePreference {
                target: target.to_string(),
                tier: tier.as_str().to_string(),
            }
        })?;
        self.engine_preferences.insert(normalized, tier);
        Ok(())
    }

    pub fn register_package_with_explicit_tier2_override(
        &mut self,
        package: &PackageRecord,
    ) -> Result<usize, SyntaxGrammarRegistryError> {
        self.register_package_inner(package, true)
    }

    fn register_package_inner(
        &mut self,
        package: &PackageRecord,
        explicit_tier2_override: bool,
    ) -> Result<usize, SyntaxGrammarRegistryError> {
        let mut staged = Vec::with_capacity(package.contributions.syntax_grammars.len());
        for descriptor in &package.contributions.syntax_grammars {
            let contribution = SyntaxGrammarContribution::from_descriptor(package, descriptor);
            let preference = self.preference_for_contribution(&contribution);
            if preference == Some(SyntaxEngineTier::JavaScriptFallback) {
                continue;
            }
            let tier2_override =
                explicit_tier2_override || preference == Some(SyntaxEngineTier::Wasm);
            if self.is_shadowed_by_native_first_party(&contribution) && !tier2_override {
                continue;
            }
            self.validate_no_conflict_ignoring_overridden_native(&contribution, tier2_override)?;
            for staged_contribution in &staged {
                self.validate_pair_no_conflict(staged_contribution, &contribution)?;
            }
            staged.push(contribution);
        }

        let count = staged.len();
        for contribution in staged {
            if explicit_tier2_override
                || self.preference_for_contribution(&contribution) == Some(SyntaxEngineTier::Wasm)
            {
                self.remove_overridden_native_first_party(&contribution);
            }
            self.insert_contribution(contribution);
        }
        Ok(count)
    }

    pub fn list(&self) -> impl Iterator<Item = &SyntaxGrammarContribution> {
        self.grammars_by_id.values()
    }

    pub fn get(&self, id: &str) -> Option<&SyntaxGrammarContribution> {
        self.grammars_by_id.get(id)
    }

    pub fn find_for_extension(&self, extension: &str) -> Option<&SyntaxGrammarContribution> {
        self.extension_to_id
            .get(extension)
            .and_then(|id| self.grammars_by_id.get(id))
    }

    pub fn find_for_file_name(&self, file_name: &str) -> Option<&SyntaxGrammarContribution> {
        self.file_name_to_id
            .get(file_name)
            .and_then(|id| self.grammars_by_id.get(id))
    }

    pub fn select_for_document(
        &mut self,
        input: &DocumentClassificationInput,
        active_major_mode: &MajorModeActivation,
        document_version: u64,
    ) -> SyntaxGrammarSelection {
        let candidate = input
            .path
            .as_deref()
            .and_then(|path| self.find_candidate_for_path(path));
        let active_syntax_grammar = candidate.and_then(|(matched_by, grammar)| {
            let preference = self.preference_for_contribution(grammar);
            if preference.is_some_and(|tier| tier != grammar.engine_tier) {
                return None;
            }
            Some(ActiveSyntaxGrammar {
                document_id: input.document_id,
                document_version,
                contribution_id: grammar.id.clone(),
                language_id: grammar.language_id.clone(),
                package_name: grammar.package_name.clone(),
                package_version: grammar.package_version.clone(),
                package_prefix: grammar.package_prefix.clone(),
                engine_tier: grammar.engine_tier,
                matched_by,
            })
        });
        let why = selection_rationale(&input.path, &active_syntax_grammar);
        let selection = SyntaxGrammarSelection {
            document_id: input.document_id,
            document_version,
            active_major_mode: active_major_mode.mode_id.clone(),
            behavior_version: active_major_mode.behavior_version,
            active_syntax_grammar,
            why,
        };
        self.active_selections
            .insert(input.document_id, selection.clone());
        selection
    }

    pub fn active_selection(&self, document_id: DocumentId) -> Option<&SyntaxGrammarSelection> {
        self.active_selections.get(&document_id)
    }

    fn preference_for_contribution(
        &self,
        contribution: &SyntaxGrammarContribution,
    ) -> Option<SyntaxEngineTier> {
        self.engine_preferences
            .get(&contribution.language_id)
            .or_else(|| self.engine_preferences.get(&contribution.package_prefix))
            .or_else(|| self.engine_preferences.get(&contribution.package_name))
            .copied()
    }

    fn find_candidate_for_path(
        &self,
        path: &str,
    ) -> Option<(SyntaxGrammarPatternKind, &SyntaxGrammarContribution)> {
        let file_name = file_name(path);
        if let Some(grammar) = file_name.and_then(|name| self.find_for_file_name(name)) {
            return Some((SyntaxGrammarPatternKind::FileName, grammar));
        }
        extension(path)
            .and_then(|extension| self.find_for_extension(extension))
            .map(|grammar| (SyntaxGrammarPatternKind::Extension, grammar))
    }

    fn insert_contribution(&mut self, contribution: SyntaxGrammarContribution) {
        let id = contribution.id.clone();
        self.language_to_id
            .insert(contribution.language_id.clone(), id.clone());
        for extension in &contribution.extensions {
            self.extension_to_id.insert(extension.clone(), id.clone());
        }
        for file_name in &contribution.file_names {
            self.file_name_to_id.insert(file_name.clone(), id.clone());
        }
        self.grammars_by_id.insert(id, contribution);
    }

    fn is_shadowed_by_native_first_party(&self, contribution: &SyntaxGrammarContribution) -> bool {
        contribution.engine_tier == SyntaxEngineTier::Wasm
            && self
                .native_descriptors_by_id
                .values()
                .filter(|descriptor| descriptor.package_prefix == contribution.package_prefix)
                .flat_map(|descriptor| descriptor.extensions.iter().copied())
                .collect::<std::collections::BTreeSet<_>>()
                .is_superset(&contribution.extensions.iter().map(String::as_str).collect())
            && contribution.file_names.iter().all(|file_name| {
                self.find_for_file_name(file_name).is_some_and(|grammar| {
                    grammar.engine_tier == SyntaxEngineTier::Native
                        && grammar.package_prefix == contribution.package_prefix
                })
            })
    }

    fn validate_no_conflict_ignoring_overridden_native(
        &self,
        contribution: &SyntaxGrammarContribution,
        explicit_tier2_override: bool,
    ) -> Result<(), SyntaxGrammarRegistryError> {
        if !explicit_tier2_override || contribution.engine_tier != SyntaxEngineTier::Wasm {
            return self.validate_no_conflict(contribution);
        }
        let mut clone = self.clone();
        clone.remove_overridden_native_first_party(contribution);
        clone.validate_no_conflict(contribution)
    }

    fn remove_overridden_native_first_party(&mut self, contribution: &SyntaxGrammarContribution) {
        if contribution.engine_tier != SyntaxEngineTier::Wasm {
            return;
        }
        let ids: Vec<String> = self
            .grammars_by_id
            .values()
            .filter(|grammar| {
                grammar.engine_tier == SyntaxEngineTier::Native
                    && grammar.package_prefix == contribution.package_prefix
                    && (grammar.language_id == contribution.language_id
                        || grammar
                            .extensions
                            .iter()
                            .any(|extension| contribution.extensions.contains(extension)))
            })
            .map(|grammar| grammar.id.clone())
            .collect();
        for id in ids {
            self.remove_contribution(&id);
            self.native_descriptors_by_id.remove(&id);
        }
    }

    fn remove_contribution(&mut self, id: &str) {
        let Some(contribution) = self.grammars_by_id.remove(id) else {
            return;
        };
        self.language_to_id.remove(&contribution.language_id);
        for extension in contribution.extensions {
            self.extension_to_id.remove(&extension);
        }
        for file_name in contribution.file_names {
            self.file_name_to_id.remove(&file_name);
        }
    }

    fn validate_no_conflict(
        &self,
        contribution: &SyntaxGrammarContribution,
    ) -> Result<(), SyntaxGrammarRegistryError> {
        if self.grammars_by_id.contains_key(&contribution.id) {
            return Err(SyntaxGrammarRegistryError::DuplicateContributionId {
                id: contribution.id.clone(),
            });
        }
        if let Some(existing_id) = self.language_to_id.get(&contribution.language_id) {
            let existing = &self.grammars_by_id[existing_id];
            return Err(SyntaxGrammarRegistryError::DuplicateLanguage {
                language_id: contribution.language_id.clone(),
                existing_package_prefix: existing.package_prefix.clone(),
                new_package_prefix: contribution.package_prefix.clone(),
            });
        }
        for extension in &contribution.extensions {
            if let Some(existing_id) = self.extension_to_id.get(extension) {
                let existing = &self.grammars_by_id[existing_id];
                return Err(SyntaxGrammarRegistryError::DuplicateExtension {
                    extension: extension.clone(),
                    existing_language_id: existing.language_id.clone(),
                    new_language_id: contribution.language_id.clone(),
                });
            }
        }
        for file_name in &contribution.file_names {
            if let Some(existing_id) = self.file_name_to_id.get(file_name) {
                let existing = &self.grammars_by_id[existing_id];
                return Err(SyntaxGrammarRegistryError::DuplicateFileName {
                    file_name: file_name.clone(),
                    existing_language_id: existing.language_id.clone(),
                    new_language_id: contribution.language_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_pair_no_conflict(
        &self,
        existing: &SyntaxGrammarContribution,
        next: &SyntaxGrammarContribution,
    ) -> Result<(), SyntaxGrammarRegistryError> {
        if existing.id == next.id {
            return Err(SyntaxGrammarRegistryError::DuplicateContributionId {
                id: next.id.clone(),
            });
        }
        if existing.language_id == next.language_id {
            return Err(SyntaxGrammarRegistryError::DuplicateLanguage {
                language_id: next.language_id.clone(),
                existing_package_prefix: existing.package_prefix.clone(),
                new_package_prefix: next.package_prefix.clone(),
            });
        }
        if let Some(extension) = existing
            .extensions
            .iter()
            .find(|extension| next.extensions.contains(*extension))
        {
            return Err(SyntaxGrammarRegistryError::DuplicateExtension {
                extension: extension.clone(),
                existing_language_id: existing.language_id.clone(),
                new_language_id: next.language_id.clone(),
            });
        }
        if let Some(file_name) = existing
            .file_names
            .iter()
            .find(|file_name| next.file_names.contains(*file_name))
        {
            return Err(SyntaxGrammarRegistryError::DuplicateFileName {
                file_name: file_name.clone(),
                existing_language_id: existing.language_id.clone(),
                new_language_id: next.language_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxGrammarPatternKind {
    FileName,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSyntaxGrammar {
    pub document_id: DocumentId,
    pub document_version: u64,
    pub contribution_id: String,
    pub language_id: String,
    pub package_name: String,
    pub package_version: String,
    pub package_prefix: String,
    pub engine_tier: SyntaxEngineTier,
    pub matched_by: SyntaxGrammarPatternKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxGrammarSelection {
    pub document_id: DocumentId,
    pub document_version: u64,
    pub active_major_mode: String,
    pub behavior_version: u64,
    pub active_syntax_grammar: Option<ActiveSyntaxGrammar>,
    pub why: String,
}

fn contribution_from_native_descriptor(
    descriptor: &NativeGrammarDescriptor,
) -> SyntaxGrammarContribution {
    SyntaxGrammarContribution {
        engine_tier: SyntaxEngineTier::Native,
        package_name: descriptor.package_name.to_string(),
        package_version: descriptor.package_version.to_string(),
        package_prefix: descriptor.package_prefix.to_string(),
        id: descriptor.id.to_string(),
        language_id: descriptor.language_id.to_string(),
        extensions: descriptor
            .extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
        file_names: descriptor
            .file_names
            .iter()
            .map(|file_name| (*file_name).to_string())
            .collect(),
        grammar_kind: "tree-sitter-native".to_string(),
        grammar_path: "builtin".to_string(),
        grammar_source: Some(descriptor.grammar_source.to_string()),
        highlights_query_path: descriptor.highlights_query_path.to_string(),
        locals_query_path: None,
        injections_query_path: None,
        style_map: descriptor
            .style_map
            .iter()
            .map(|(capture, token)| ((*capture).to_string(), (*token).to_string()))
            .collect(),
        timeout_ms: Some(5_000),
        max_window_bytes: Some(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES),
        estimated_payload_bytes: 512,
    }
}

fn selection_rationale(path: &Option<String>, grammar: &Option<ActiveSyntaxGrammar>) -> String {
    match grammar {
        Some(grammar) => format!(
            "active syntax grammar {} from {} selected by {} using {} tier without changing active major mode",
            grammar.language_id,
            grammar.package_name,
            match grammar.matched_by {
                SyntaxGrammarPatternKind::FileName => "filename",
                SyntaxGrammarPatternKind::Extension => "extension",
            },
            grammar.engine_tier.as_str()
        ),
        None if path.is_some() => {
            "no loaded syntax grammar matched; document remains editable with its active major mode"
                .to_string()
        }
        None => "no document path supplied; document remains editable with its active major mode"
            .to_string(),
    }
}

fn normalize_engine_preference_target(target: &str) -> Option<String> {
    let normalized = target.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.contains(char::is_whitespace)
        || normalized.contains('/')
        || normalized.contains('\\')
        || normalized.contains("..")
    {
        None
    } else {
        Some(normalized)
    }
}

fn file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

fn extension(path: &str) -> Option<&str> {
    let name = file_name(path)?;
    let (_, extension) = name.rsplit_once('.')?;
    if extension.is_empty() {
        None
    } else {
        Some(extension)
    }
}

fn validate_wasm_path(path: &str) -> Result<(), String> {
    if path.starts_with("./grammars/")
        && path.ends_with(".wasm")
        && !path.contains("..")
        && !path.contains('\\')
    {
        Ok(())
    } else {
        Err(path.to_string())
    }
}

fn validate_query_path(path: &str) -> Result<(), String> {
    if path.starts_with("./queries/")
        && path.ends_with(".scm")
        && !path.contains("..")
        && !path.contains('\\')
    {
        Ok(())
    } else {
        Err(path.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxCapture {
    pub byte_start: u64,
    pub byte_end: u64,
    pub capture_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxVocabularySpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub style_token: String,
    pub token_type: TokenType,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSitterSyntaxError {
    QueryCompileFailed { message: String },
    QueryCaptureNotMapped { capture: String },
    WindowTooLarge { bytes: usize, budget: usize },
    ParseTimedOut,
    DecorationInvalid(String),
    PayloadBudgetExceeded { bytes: usize, budget: usize },
    CaptureLimitExceeded { limit: usize },
}

impl fmt::Display for TreeSitterSyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryCompileFailed { message } => {
                write!(
                    formatter,
                    "syntax highlight query failed to compile: {message}"
                )
            }
            Self::QueryCaptureNotMapped { capture } => write!(
                formatter,
                "syntax highlight capture @{capture} is not mapped to a known Clay style token"
            ),
            Self::WindowTooLarge { bytes, budget } => write!(
                formatter,
                "syntax parse window is {bytes} bytes, above the {budget} byte budget"
            ),
            Self::ParseTimedOut => write!(formatter, "tree-sitter syntax parse timed out"),
            Self::DecorationInvalid(message) => {
                write!(formatter, "syntax decoration validation failed: {message}")
            }
            Self::PayloadBudgetExceeded { bytes, budget } => write!(
                formatter,
                "syntax decoration payload is {bytes} bytes, above the {budget} byte budget"
            ),
            Self::CaptureLimitExceeded { limit } => write!(
                formatter,
                "syntax highlight query produced more than {limit} captures for one viewport"
            ),
        }
    }
}

#[derive(Clone)]
pub struct TreeSitterSyntaxHandler {
    contribution: SyntaxGrammarContribution,
    language: Language,
    parser: Arc<Mutex<Parser>>,
    highlights_query: Arc<Query>,
    trees: Arc<Mutex<HashMap<DocumentId, CachedSyntaxTree>>>,
    decoration_cache: Arc<Mutex<SyntaxChunkCache>>,
}

impl fmt::Debug for TreeSitterSyntaxHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TreeSitterSyntaxHandler")
            .field("contribution", &self.contribution)
            .field("language", &self.language)
            .field("highlights_query", &self.highlights_query)
            .field("trees", &self.trees)
            .field("decoration_cache", &self.decoration_cache)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
struct CachedSyntaxTree {
    document_version: u64,
    window_start: u64,
    tree: Tree,
}

impl TreeSitterSyntaxHandler {
    pub fn new(
        contribution: SyntaxGrammarContribution,
        language: Language,
        highlights_query: &str,
    ) -> Result<Self, TreeSitterSyntaxError> {
        let query = Query::new(&language, highlights_query).map_err(|error| {
            TreeSitterSyntaxError::QueryCompileFailed {
                message: error.to_string(),
            }
        })?;
        for capture in query.capture_names() {
            if !contribution.style_map.contains_key(*capture) {
                return Err(TreeSitterSyntaxError::QueryCaptureNotMapped {
                    capture: (*capture).to_string(),
                });
            }
        }
        let mut parser = Parser::new();
        parser.set_language(&language).map_err(|error| {
            TreeSitterSyntaxError::QueryCompileFailed {
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            contribution,
            language,
            parser: Arc::new(Mutex::new(parser)),
            highlights_query: Arc::new(query),
            trees: Arc::new(Mutex::new(HashMap::new())),
            decoration_cache: Arc::new(Mutex::new(SyntaxChunkCache::default())),
        })
    }

    pub fn parse_sync(
        &self,
        notification: ParseEditNotification,
    ) -> Result<IncrementalParseUpdate, TreeSitterSyntaxError> {
        let Some(window) = notification
            .parse_windows
            .iter()
            .find(|window| window.byte_range().intersects(notification.viewport))
            .or_else(|| notification.parse_windows.first())
        else {
            return Ok(empty_update(notification));
        };

        let window_bytes = window.text.len();
        let window_budget = self.contribution.max_window_bytes();
        if window_bytes > window_budget {
            return Err(TreeSitterSyntaxError::WindowTooLarge {
                bytes: window_bytes,
                budget: window_budget,
            });
        }

        let old_tree = self
            .trees
            .lock()
            .expect("syntax tree cache lock poisoned")
            .get(&notification.document_id)
            .filter(|cached| {
                cached.document_version < notification.document_version
                    && cached.window_start == window.byte_start
            })
            .map(|cached| cached.tree.clone());

        let mut parser = self.parser.lock().expect("syntax parser lock poisoned");
        #[allow(deprecated)]
        parser.set_timeout_micros(self.contribution.timeout_micros());
        let Some(tree) = parser.parse(&window.text, old_tree.as_ref()) else {
            return Err(TreeSitterSyntaxError::ParseTimedOut);
        };

        let decoration_update = self.decorations_for_window(&notification, window, &tree)?;
        self.trees
            .lock()
            .expect("syntax tree cache lock poisoned")
            .insert(
                notification.document_id,
                CachedSyntaxTree {
                    document_version: notification.document_version,
                    window_start: window.byte_start,
                    tree,
                },
            );

        Ok(IncrementalParseUpdate {
            document_id: notification.document_id,
            document_version: notification.document_version,
            behavior_version: notification.behavior_version,
            package_prefix: notification.package_prefix,
            mode_id: notification.mode_id,
            parse_unit: ParseUnit::Region,
            viewport: notification.viewport,
            invalidated_ranges: notification.invalidated_ranges,
            syntax_tree_delta: Some(format!(
                "tree-sitter:{}:{}",
                self.contribution.language_id,
                if old_tree.is_some() {
                    "incremental"
                } else {
                    "full"
                }
            )),
            decoration_update: Some(decoration_update),
        })
    }

    pub fn cached_tree_version(&self, document_id: DocumentId) -> Option<u64> {
        self.trees
            .lock()
            .expect("syntax tree cache lock poisoned")
            .get(&document_id)
            .map(|cached| cached.document_version)
    }

    pub fn parser_cache_id(&self) -> usize {
        Arc::as_ptr(&self.parser) as usize
    }

    fn decorations_for_window(
        &self,
        notification: &ParseEditNotification,
        window: &crate::protocol::ParseWindowSnapshot,
        tree: &Tree,
    ) -> Result<DecorationSet, TreeSitterSyntaxError> {
        let relative_viewport_start = notification
            .viewport
            .start
            .saturating_sub(window.byte_start)
            .min(window.text.len() as u64) as usize;
        let relative_viewport_end = notification
            .viewport
            .end
            .saturating_sub(window.byte_start)
            .min(window.text.len() as u64) as usize;

        let mut cursor = QueryCursor::new();
        #[allow(deprecated)]
        cursor.set_timeout_micros(self.contribution.timeout_micros());
        cursor.set_byte_range(relative_viewport_start..relative_viewport_end);
        let mut captures = cursor.captures(
            &self.highlights_query,
            tree.root_node(),
            window.text.as_bytes(),
        );
        let capture_names = self.highlights_query.capture_names();
        let mut syntax_captures = Vec::new();

        loop {
            captures.advance();
            let Some((query_match, capture_index)) = captures.get() else {
                break;
            };
            if syntax_captures.len() >= MAX_SYNTAX_HIGHLIGHT_SPANS {
                return Err(TreeSitterSyntaxError::CaptureLimitExceeded {
                    limit: MAX_SYNTAX_HIGHLIGHT_SPANS,
                });
            }
            let capture = query_match.captures[*capture_index];
            let capture_name = capture_names[capture.index as usize];
            if !self.contribution.style_map.contains_key(capture_name) {
                return Err(TreeSitterSyntaxError::QueryCaptureNotMapped {
                    capture: capture_name.to_string(),
                });
            }
            let absolute_start = window
                .byte_start
                .saturating_add(capture.node.start_byte() as u64);
            let absolute_end = window
                .byte_start
                .saturating_add(capture.node.end_byte() as u64);
            let byte_start = absolute_start.max(notification.viewport.start);
            let byte_end = absolute_end.min(notification.viewport.end);
            if byte_start >= byte_end {
                continue;
            }
            syntax_captures.push(SyntaxCapture {
                byte_start,
                byte_end,
                capture_name: capture_name.to_string(),
            });
        }

        let set = captures_to_decoration_set(&self.contribution, notification, syntax_captures)?;
        let set = validate_decoration_set(notification.document_version, set, None)
            .map_err(map_decoration_error)?;
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&set)
            .map_err(|error| TreeSitterSyntaxError::DecorationInvalid(error.to_string()))?
            .len();
        if bytes > DECORATION_PAYLOAD_BUDGET_BYTES {
            return Err(TreeSitterSyntaxError::PayloadBudgetExceeded {
                bytes,
                budget: DECORATION_PAYLOAD_BUDGET_BYTES,
            });
        }
        self.decoration_cache
            .lock()
            .expect("syntax decoration cache lock poisoned")
            .insert_validated_set(&self.contribution.package_prefix, set.clone())
            .map_err(map_decoration_error)?;
        Ok(set)
    }
}

pub fn map_capture_to_vocabulary(
    contribution: &SyntaxGrammarContribution,
    capture: &SyntaxCapture,
) -> Result<SyntaxVocabularySpan, TreeSitterSyntaxError> {
    let style_token = contribution
        .style_map
        .get(&capture.capture_name)
        .ok_or_else(|| TreeSitterSyntaxError::QueryCaptureNotMapped {
            capture: capture.capture_name.clone(),
        })?;
    let (token_type, modifiers) = TokenType::classify_style_token(style_token);
    Ok(SyntaxVocabularySpan {
        byte_start: capture.byte_start,
        byte_end: capture.byte_end,
        style_token: style_token.clone(),
        token_type,
        modifiers,
    })
}

fn captures_to_decoration_set(
    contribution: &SyntaxGrammarContribution,
    notification: &ParseEditNotification,
    captures: Vec<SyntaxCapture>,
) -> Result<DecorationSet, TreeSitterSyntaxError> {
    let provenance = contribution.provenance();
    let mut spans = Vec::with_capacity(captures.len());
    for capture in captures {
        let vocabulary = map_capture_to_vocabulary(contribution, &capture)?;
        spans.push(DecorationSpan {
            byte_start: vocabulary.byte_start,
            byte_end: vocabulary.byte_end,
            kind: DecorationKind::Syntax,
            token_type: vocabulary.token_type,
            modifiers: vocabulary.modifiers,
            scope: Some(vocabulary.style_token),
            priority: 70,
            provenance: provenance.clone(),
        });
    }
    Ok(DecorationSet {
        document_id: notification.document_id,
        document_version: notification.document_version,
        viewport_byte_start: notification.viewport.start,
        viewport_byte_end: notification.viewport.end,
        spans,
    })
}

impl crate::server::parse_coordinator::ParseHandler for TreeSitterSyntaxHandler {
    fn parse(&self, notification: ParseEditNotification) -> ParseHandlerFuture {
        let handler = self.clone();
        Box::pin(async move {
            handler
                .parse_sync(notification)
                .map_err(|error| ParseCoordinatorError::HandlerFailed(error.to_string()))
        })
    }
}

fn empty_update(notification: ParseEditNotification) -> IncrementalParseUpdate {
    IncrementalParseUpdate {
        document_id: notification.document_id,
        document_version: notification.document_version,
        behavior_version: notification.behavior_version,
        package_prefix: notification.package_prefix,
        mode_id: notification.mode_id,
        parse_unit: ParseUnit::Region,
        viewport: notification.viewport,
        invalidated_ranges: notification.invalidated_ranges,
        syntax_tree_delta: None,
        decoration_update: None,
    }
}

fn map_decoration_error(error: DecorationValidationError) -> TreeSitterSyntaxError {
    TreeSitterSyntaxError::DecorationInvalid(format!("{error:?}"))
}
