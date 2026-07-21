use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    sync::{Arc, Mutex},
};

use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Language, Parser, Point, Query, QueryCursor, Tree};

const SYNTAX_DECORATION_CHUNK_BYTES: usize = 128;

use crate::{
    packages::{
        modes::{DocumentClassificationInput, MajorModeActivation},
        record::{PackageRecord, SyntaxGrammarContributionDescriptor},
    },
    perf::{
        budgets::{
            DECORATION_PAYLOAD_BUDGET_BYTES, INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
            MAX_OPENABLE_FILE_BYTES,
        },
        metrics::{
            MetricMetadata, MetricValue, PerfRecorder, SYNTAX_PARSE_FULL, SYNTAX_PARSE_INCREMENTAL,
            SYNTAX_PARSE_INVOCATIONS, SYNTAX_QUERY_BYTES, SYNTAX_QUERY_RANGES, global_recorder,
        },
    },
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentId,
        IncrementalParseUpdate, Modifiers, ParseByteRange, ParseEditNotification, ParseUnit,
        TokenType,
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
    pub style_map: BTreeMap<String, crate::packages::record::SyntaxStyleMapEntry>,
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
            engine_tier: match descriptor.grammar_kind.as_str() {
                "native" => SyntaxEngineTier::Native,
                _ => SyntaxEngineTier::Wasm,
            },
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
    InvalidSnapshotArtifact {
        id: String,
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
    highlights_query: &'static str,
    style_map: &'static [(
        &'static str,
        crate::protocol::TokenType,
        crate::protocol::Modifiers,
        Option<crate::protocol::DocumentFontRole>,
        u16,
    )],
    language: fn() -> Language,
    max_window_bytes: usize,
    /// Optional composite-grammar injection query (`queries.injections`). When
    /// present, the handler re-parses each `@injection.content` range with the
    /// first-party embedded grammar registered under the resolved injection
    /// language name (see `FIRST_PARTY_EMBEDDED_GRAMMARS`).
    injections_query_path: Option<&'static str>,
    injections_query: Option<&'static str>,
}

/// First-party embedded grammar resolvable by injection language name. The
/// generic injection executor refuses any name not registered here, so only
/// Clay-vendored grammar artifacts ever parse an injected range.
#[derive(Clone, Copy)]
struct EmbeddedGrammarDescriptor {
    name: &'static str,
    language: fn() -> Language,
    highlights_query: &'static str,
}

impl fmt::Debug for EmbeddedGrammarDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedGrammarDescriptor")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

const FIRST_PARTY_EMBEDDED_GRAMMARS: &[EmbeddedGrammarDescriptor] = &[EmbeddedGrammarDescriptor {
    name: "markdown_inline",
    language: || tree_sitter_md_025::INLINE_LANGUAGE.into(),
    highlights_query: include_str!("../../packages/markdown/queries/inline-highlights.scm"),
}];

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
            .field("max_window_bytes", &self.max_window_bytes)
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
        highlights_query: include_str!("../../packages/rust/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_rust::LANGUAGE.into(),
        // Code meaning (strings, expressions, macro bodies) can begin well
        // before the visible viewport; a viewport-sized fragment can parse as
        // one recovery ERROR node (lost highlights) or invent tokens (e.g. a
        // closing quote re-read as a string opener). Parse bounded full-file
        // context while query and decoration output remain viewport-limited,
        // same as the markdown grammar below.
        max_window_bytes: MAX_OPENABLE_FILE_BYTES,
        injections_query_path: None,
        injections_query: None,
    },
    NativeGrammarDescriptor {
        package_name: "@clay/typescript",
        package_version: "builtin",
        package_prefix: "typescript",
        id: "typescript.typescript",
        language_id: "typescript",
        extensions: &["ts", "mts", "cts"],
        file_names: &[],
        grammar_source: "tree-sitter-typescript",
        highlights_query_path: "packages/typescript/queries/highlights.scm",
        highlights_query: include_str!("../../packages/typescript/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        // See rust.rust: full-file context, viewport-limited output.
        max_window_bytes: MAX_OPENABLE_FILE_BYTES,
        injections_query_path: None,
        injections_query: None,
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
        highlights_query: include_str!("../../packages/typescript/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        // See rust.rust: full-file context, viewport-limited output.
        max_window_bytes: MAX_OPENABLE_FILE_BYTES,
        injections_query_path: None,
        injections_query: None,
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
        highlights_query: include_str!("../../packages/javascript/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_javascript::LANGUAGE.into(),
        // See rust.rust: full-file context, viewport-limited output.
        max_window_bytes: MAX_OPENABLE_FILE_BYTES,
        injections_query_path: None,
        injections_query: None,
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
        highlights_query: include_str!("../../packages/markdown/queries/highlights.scm"),
        style_map: MARKDOWN_NATIVE_STYLE_MAP,
        language: || tree_sitter_md_025::LANGUAGE.into(),
        // Markdown block meaning (notably fenced-code state) can begin before
        // the visible viewport. Parse bounded full-file context while query
        // and decoration output remain viewport-limited.
        max_window_bytes: MAX_OPENABLE_FILE_BYTES,
        injections_query_path: Some("packages/markdown/queries/injections.scm"),
        injections_query: Some(include_str!(
            "../../packages/markdown/queries/injections.scm"
        )),
    },
];

const DEFAULT_NATIVE_STYLE_MAP: &[(
    &str,
    TokenType,
    Modifiers,
    Option<crate::protocol::DocumentFontRole>,
    u16,
)] = &[
    ("keyword", TokenType::Keyword, Modifiers::NONE, None, 70),
    ("string", TokenType::String, Modifiers::NONE, None, 70),
    ("comment", TokenType::Comment, Modifiers::NONE, None, 70),
    (
        "punctuation",
        TokenType::Operator,
        Modifiers::NONE,
        None,
        70,
    ),
    ("text", TokenType::Paragraph, Modifiers::NONE, None, 70),
    ("function", TokenType::Function, Modifiers::NONE, None, 70),
    (
        "function.declaration",
        TokenType::Function,
        Modifiers::DECLARATION,
        None,
        70,
    ),
    ("type", TokenType::Type, Modifiers::NONE, None, 70),
    ("number", TokenType::Number, Modifiers::NONE, None, 70),
];

// Narrow inline captures (code-span/strong/emphasis/link) outrank broad
// prose captures (text/heading) at 80 so overlapping ranges resolve to the
// inline token instead of the paragraph base color.
const MARKDOWN_NATIVE_STYLE_MAP: &[(
    &str,
    TokenType,
    Modifiers,
    Option<crate::protocol::DocumentFontRole>,
    u16,
)] = &[
    (
        "punctuation",
        TokenType::Operator,
        Modifiers::NONE,
        None,
        70,
    ),
    ("text", TokenType::Paragraph, Modifiers::NONE, None, 70),
    (
        "code",
        TokenType::CodeBlock,
        Modifiers::NONE,
        Some(crate::protocol::DocumentFontRole::Monospace),
        70,
    ),
    (
        "code-span",
        TokenType::CodeSpan,
        Modifiers::NONE,
        Some(crate::protocol::DocumentFontRole::Monospace),
        80,
    ),
    ("heading-1", TokenType::Heading1, Modifiers::NONE, None, 70),
    ("heading-2", TokenType::Heading2, Modifiers::NONE, None, 70),
    ("heading-3", TokenType::Heading3, Modifiers::NONE, None, 70),
    ("heading-4", TokenType::Heading4, Modifiers::NONE, None, 70),
    ("heading-5", TokenType::Heading5, Modifiers::NONE, None, 70),
    ("heading-6", TokenType::Heading6, Modifiers::NONE, None, 70),
    ("strong", TokenType::Paragraph, Modifiers::BOLD, None, 80),
    (
        "emphasis",
        TokenType::Paragraph,
        Modifiers::ITALIC,
        None,
        80,
    ),
    (
        "list-marker",
        TokenType::ListItem,
        Modifiers::NONE,
        None,
        70,
    ),
    ("link", TokenType::Link, Modifiers::NONE, None, 80),
    ("quote", TokenType::Quote, Modifiers::NONE, None, 70),
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

    pub(crate) fn validate_snapshot(
        grammars: &[SyntaxGrammarContribution],
        engine_preferences: &BTreeMap<String, SyntaxEngineTier>,
    ) -> Result<(), SyntaxGrammarRegistryError> {
        let mut registry = Self::new();
        for (target, tier) in engine_preferences {
            registry.set_engine_preference(target, *tier)?;
        }
        for grammar in grammars {
            if grammar.engine_tier == SyntaxEngineTier::Wasm
                && grammar.web_tree_sitter_artifact_contract().is_err()
            {
                return Err(SyntaxGrammarRegistryError::InvalidSnapshotArtifact {
                    id: grammar.id.clone(),
                });
            }
            registry.validate_no_conflict(grammar)?;
            registry.insert_contribution(grammar.clone());
        }
        Ok(())
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
            let tier2_override = contribution.engine_tier == SyntaxEngineTier::Wasm
                && (explicit_tier2_override || preference == Some(SyntaxEngineTier::Wasm));
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
            if contribution.engine_tier == SyntaxEngineTier::Wasm
                && (explicit_tier2_override
                    || self.preference_for_contribution(&contribution)
                        == Some(SyntaxEngineTier::Wasm))
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

    pub(crate) fn engine_preferences(&self) -> BTreeMap<String, SyntaxEngineTier> {
        self.engine_preferences.clone()
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

    pub(crate) fn find_candidate_for_path(
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
        self.native_descriptors_by_id
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

pub(crate) fn select_grammar_for_path<'a>(
    grammars: &'a [SyntaxGrammarContribution],
    engine_preferences: &BTreeMap<String, SyntaxEngineTier>,
    path: &str,
) -> Option<&'a SyntaxGrammarContribution> {
    let file_name = file_name(path);
    let extension = extension(path);
    grammars
        .iter()
        .find(|grammar| {
            file_name
                .is_some_and(|name| grammar.file_names.iter().any(|candidate| candidate == name))
        })
        .or_else(|| {
            grammars.iter().find(|grammar| {
                extension.is_some_and(|extension| {
                    grammar
                        .extensions
                        .iter()
                        .any(|candidate| candidate == extension)
                })
            })
        })
        .filter(|grammar| {
            engine_preferences
                .get(&grammar.language_id)
                .or_else(|| engine_preferences.get(&grammar.package_prefix))
                .or_else(|| engine_preferences.get(&grammar.package_name))
                .is_none_or(|tier| *tier == grammar.engine_tier)
        })
}

pub(crate) fn native_handler(
    contribution: &SyntaxGrammarContribution,
) -> Result<Option<TreeSitterSyntaxHandler>, TreeSitterSyntaxError> {
    let Some(descriptor) = FIRST_PARTY_NATIVE_GRAMMARS
        .iter()
        .find(|descriptor| descriptor.id == contribution.id)
    else {
        return Ok(None);
    };
    let mut handler = TreeSitterSyntaxHandler::new(
        contribution.clone(),
        (descriptor.language)(),
        descriptor.highlights_query,
    )?;
    if let Some(injections_query) = descriptor.injections_query {
        handler.enable_injections(injections_query)?;
    }
    Ok(Some(handler))
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
        grammar_kind: "native".to_string(),
        grammar_path: String::new(),
        grammar_source: Some(descriptor.grammar_source.to_string()),
        highlights_query_path: descriptor.highlights_query_path.to_string(),
        locals_query_path: None,
        injections_query_path: descriptor.injections_query_path.map(str::to_string),
        style_map: descriptor
            .style_map
            .iter()
            .map(|(capture, token_type, modifiers, font_role, priority)| {
                (
                    (*capture).to_string(),
                    crate::packages::record::SyntaxStyleMapEntry {
                        token_type: *token_type,
                        modifiers: *modifiers,
                        scope: None,
                        font_role: *font_role,
                        priority: *priority,
                    },
                )
            })
            .collect(),
        timeout_ms: Some(5_000),
        max_window_bytes: Some(descriptor.max_window_bytes),
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
    pub token_type: TokenType,
    pub modifiers: Modifiers,
    pub scope: Option<String>,
    pub font_role: Option<crate::protocol::DocumentFontRole>,
    pub priority: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSitterSyntaxError {
    QueryCompileFailed { message: String },
    QueryCaptureNotMapped { capture: String },
    WindowTooLarge { bytes: usize, budget: usize },
    ParseTimedOut,
    DecorationInvalid(String),
    PayloadBudgetExceeded { bytes: usize, budget: usize },
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
                "syntax highlight capture @{capture} has no vocabulary styleMap entry"
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
        }
    }
}

#[derive(Clone)]
pub struct TreeSitterSyntaxHandler {
    contribution: SyntaxGrammarContribution,
    language: Language,
    parser: Arc<Mutex<Parser>>,
    highlights_query: Arc<Query>,
    injections: Option<Arc<InjectionState>>,
    trees: Arc<Mutex<HashMap<DocumentId, CachedSyntaxTree>>>,
    decoration_cache: Arc<Mutex<SyntaxChunkCache>>,
    perf: PerfRecorder,
}

/// Generic composite-grammar state: a host-language injection query plus the
/// lazily built embedded layers it references. Layer parsers are cached per
/// injection language name so repeated parses of the same embedded grammar
/// reuse one parser.
#[derive(Debug)]
struct InjectionState {
    query: Query,
    content_capture: u32,
    language_capture: Option<u32>,
    layers: Mutex<BTreeMap<String, Arc<EmbeddedLayer>>>,
}

struct EmbeddedLayer {
    parser: Mutex<Parser>,
    highlights: Query,
}

impl fmt::Debug for EmbeddedLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddedLayer")
            .field("highlights", &self.highlights)
            .finish_non_exhaustive()
    }
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
    window_id: u64,
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
            injections: None,
            trees: Arc::new(Mutex::new(HashMap::new())),
            decoration_cache: Arc::new(Mutex::new(SyntaxChunkCache::default())),
            perf: global_recorder(),
        })
    }

    /// Enable generic composite-grammar parsing: after the host parse, every
    /// `@injection.content` range is re-parsed with the first-party embedded
    /// grammar resolved from the pattern's `#set! injection.language "..."`
    /// property or `@injection.language` capture text, and the embedded
    /// grammar's highlight captures are emitted through this contribution's
    /// style map with this package's provenance.
    pub fn enable_injections(
        &mut self,
        injections_query: &str,
    ) -> Result<(), TreeSitterSyntaxError> {
        let query = Query::new(&self.language, injections_query).map_err(|error| {
            TreeSitterSyntaxError::QueryCompileFailed {
                message: error.to_string(),
            }
        })?;
        let capture_names = query.capture_names();
        let content_capture = capture_names
            .iter()
            .position(|name| *name == "injection.content")
            .map(|index| index as u32)
            .ok_or_else(|| TreeSitterSyntaxError::QueryCompileFailed {
                message: "injection query declares no @injection.content capture".to_string(),
            })?;
        let language_capture = capture_names
            .iter()
            .position(|name| *name == "injection.language")
            .map(|index| index as u32);
        self.injections = Some(Arc::new(InjectionState {
            query,
            content_capture,
            language_capture,
            layers: Mutex::new(BTreeMap::new()),
        }));
        Ok(())
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

        let relative_edit = window
            .incremental_edit
            .then_some(notification.accepted_edit)
            .flatten()
            .and_then(|edit| edit.relative_to_window(window));
        let expected_old_window_bytes = relative_edit.and_then(|edit| {
            usize::try_from(
                window.text.len() as i128 - (edit.new_end_byte as i128 - edit.old_end_byte as i128),
            )
            .ok()
        });
        let old_tree = relative_edit.and_then(|edit| {
            let input_edit = input_edit(edit)?;
            let mut tree = self
                .trees
                .lock()
                .expect("syntax tree cache lock poisoned")
                .get(&notification.document_id)
                .filter(|cached| {
                    cached.document_version.checked_add(1) == Some(notification.document_version)
                        && cached.window_id == window.window_id
                        && expected_old_window_bytes == Some(cached.tree.root_node().end_byte())
                })?
                .tree
                .clone();
            tree.edit(&input_edit);
            Some(tree)
        });

        let cached_tree = self
            .trees
            .lock()
            .expect("syntax tree cache lock poisoned")
            .get(&notification.document_id)
            .filter(|cached| {
                cached.document_version == notification.document_version
                    && cached.window_id == window.window_id
                    && cached.tree.root_node().end_byte() == window.text.len()
            })
            .map(|cached| cached.tree.clone());
        let (tree, parse_kind) = if let Some(tree) = cached_tree {
            (tree, "cached")
        } else {
            let metadata =
                MetricMetadata::document(notification.document_id, notification.document_version);
            self.perf.record_with_metadata(
                SYNTAX_PARSE_INVOCATIONS,
                MetricValue::Counter { amount: 1 },
                metadata.clone(),
            );
            self.perf.record_with_metadata(
                if old_tree.is_some() {
                    SYNTAX_PARSE_INCREMENTAL
                } else {
                    SYNTAX_PARSE_FULL
                },
                MetricValue::Counter { amount: 1 },
                metadata,
            );

            let mut parser = self.parser.lock().expect("syntax parser lock poisoned");
            #[allow(deprecated)]
            parser.set_timeout_micros(self.contribution.timeout_micros());
            let Some(tree) = parser.parse(&window.text, old_tree.as_ref()) else {
                return Err(TreeSitterSyntaxError::ParseTimedOut);
            };
            (
                tree,
                if old_tree.is_some() {
                    "incremental"
                } else {
                    "full"
                },
            )
        };

        let affected_ranges = query_ranges(&notification, window, old_tree.as_ref(), &tree);
        let replacement_ranges = replacement_ranges(&affected_ranges, &window.text);
        let decoration_updates =
            self.decorations_for_window(&notification, window, &tree, &replacement_ranges)?;
        let update_viewport = ParseByteRange::new(
            window
                .byte_start
                .saturating_add(replacement_ranges.first().map_or(0, |range| range.start) as u64),
            window
                .byte_start
                .saturating_add(replacement_ranges.last().map_or(0, |range| range.end) as u64),
        );
        self.trees
            .lock()
            .expect("syntax tree cache lock poisoned")
            .insert(
                notification.document_id,
                CachedSyntaxTree {
                    document_version: notification.document_version,
                    window_id: window.window_id,
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
            viewport: update_viewport,
            invalidated_ranges: vec![update_viewport],
            syntax_tree_delta: Some(format!(
                "tree-sitter:{}:{parse_kind}",
                self.contribution.language_id,
            )),
            decoration_updates,
            // Tree-sitter recovery nodes are unreliable on bounded viewport
            // fragments. Diagnostics remain reserved for explicit analyzers
            // (including future LSP packages), not syntax highlighting.
            diagnostic_update: None,
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
        replacement_ranges: &[std::ops::Range<usize>],
    ) -> Result<Vec<DecorationSet>, TreeSitterSyntaxError> {
        let query_range = replacement_ranges
            .first()
            .map(|first| first.start..replacement_ranges.last().map_or(first.end, |last| last.end))
            .unwrap_or(0..0);
        let metadata =
            MetricMetadata::document(notification.document_id, notification.document_version);
        self.perf.record_with_metadata(
            SYNTAX_QUERY_RANGES,
            MetricValue::Counter {
                amount: u64::from(!query_range.is_empty()),
            },
            metadata.clone(),
        );
        self.perf.record_with_metadata(
            SYNTAX_QUERY_BYTES,
            MetricValue::Bytes {
                bytes: query_range.len() as u64,
            },
            metadata,
        );

        let mut syntax_captures = Vec::new();
        if !query_range.is_empty() {
            let mut cursor = QueryCursor::new();
            #[allow(deprecated)]
            cursor.set_timeout_micros(self.contribution.timeout_micros());
            cursor.set_byte_range(query_range.clone());
            let mut captures = cursor.captures(
                &self.highlights_query,
                tree.root_node(),
                window.text.as_bytes(),
            );
            let capture_names = self.highlights_query.capture_names();

            loop {
                captures.advance();
                let Some((query_match, capture_index)) = captures.get() else {
                    break;
                };
                let capture = query_match.captures[*capture_index];
                let capture_name = capture_names[capture.index as usize];
                if !self.contribution.style_map.contains_key(capture_name) {
                    continue;
                }
                let absolute_start = window
                    .byte_start
                    .saturating_add(capture.node.start_byte() as u64);
                let absolute_end = window
                    .byte_start
                    .saturating_add(capture.node.end_byte() as u64);
                if absolute_start >= absolute_end {
                    continue;
                }
                syntax_captures.push(SyntaxCapture {
                    byte_start: absolute_start,
                    byte_end: absolute_end,
                    capture_name: capture_name.to_string(),
                });
            }
        }

        if let Some(injections) = &self.injections {
            self.injection_captures_for_window(
                injections,
                window,
                tree,
                &query_range,
                &mut syntax_captures,
            )?;
        }

        let spans = captures_to_decoration_spans(&self.contribution, syntax_captures)?;
        let mut sets = decoration_sets_for_ranges(notification, window, replacement_ranges, spans);
        sets.sort_by_key(|set| {
            !notification.invalidated_ranges.iter().any(|range| {
                range.intersects(ParseByteRange::new(
                    set.viewport_byte_start,
                    set.viewport_byte_end,
                ))
            })
        });
        let mut cache = self
            .decoration_cache
            .lock()
            .expect("syntax decoration cache lock poisoned");
        for set in &mut sets {
            *set = validate_decoration_set(notification.document_version, set.clone(), None)
                .map_err(map_decoration_error)?;
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&*set)
                .map_err(|error| TreeSitterSyntaxError::DecorationInvalid(error.to_string()))?
                .len();
            if bytes > DECORATION_PAYLOAD_BUDGET_BYTES {
                return Err(TreeSitterSyntaxError::PayloadBudgetExceeded {
                    bytes,
                    budget: DECORATION_PAYLOAD_BUDGET_BYTES,
                });
            }
            cache
                .insert_validated_set(&self.contribution.package_prefix, set.clone())
                .map_err(map_decoration_error)?;
        }
        Ok(sets)
    }
}

impl TreeSitterSyntaxHandler {
    /// Run the generic injection executor: collect `@injection.content` ranges
    /// per injection language from the host tree, re-parse each range set with
    /// the registered embedded grammar (`set_included_ranges`), and append the
    /// embedded highlight captures (same style map, same provenance) to the
    /// host captures. Unregistered language names (e.g. fenced-code info
    /// strings with no first-party grammar) and timed-out embedded parses are
    /// skipped so host decorations still ship.
    fn injection_captures_for_window(
        &self,
        injections: &InjectionState,
        window: &crate::protocol::ParseWindowSnapshot,
        tree: &Tree,
        query_range: &std::ops::Range<usize>,
        syntax_captures: &mut Vec<SyntaxCapture>,
    ) -> Result<(), TreeSitterSyntaxError> {
        if query_range.is_empty() {
            return Ok(());
        }
        let mut groups: BTreeMap<String, Vec<tree_sitter::Range>> = BTreeMap::new();
        let mut cursor = QueryCursor::new();
        #[allow(deprecated)]
        cursor.set_timeout_micros(self.contribution.timeout_micros());
        cursor.set_byte_range(query_range.clone());
        let mut matches =
            cursor.matches(&injections.query, tree.root_node(), window.text.as_bytes());
        loop {
            matches.advance();
            let Some(query_match) = matches.get() else {
                break;
            };
            let language = injections
                .query
                .property_settings(query_match.pattern_index)
                .iter()
                .find(|property| &*property.key == "injection.language")
                .and_then(|property| property.value.as_deref())
                .map(str::to_string)
                .or_else(|| {
                    let index = injections.language_capture?;
                    query_match
                        .captures
                        .iter()
                        .find(|capture| capture.index == index)
                        .and_then(|capture| {
                            capture
                                .node
                                .utf8_text(window.text.as_bytes())
                                .ok()
                                .map(str::to_string)
                        })
                });
            let Some(language) = language.filter(|language| !language.is_empty()) else {
                continue;
            };
            for capture in query_match
                .captures
                .iter()
                .filter(|capture| capture.index == injections.content_capture)
            {
                groups
                    .entry(language.clone())
                    .or_default()
                    .push(capture.node.range());
            }
        }

        for (language, mut ranges) in groups {
            let Some(layer) = self.embedded_layer(injections, &language)? else {
                continue;
            };
            ranges.sort_by_key(|range| (range.start_byte, range.end_byte));
            ranges.dedup_by_key(|range| (range.start_byte, range.end_byte));
            let mut parser = layer.parser.lock().expect("embedded parser lock poisoned");
            #[allow(deprecated)]
            parser.set_timeout_micros(self.contribution.timeout_micros());
            if parser.set_included_ranges(&ranges).is_err() {
                continue;
            }
            let Some(embedded_tree) = parser.parse(&window.text, None) else {
                continue;
            };
            let mut cursor = QueryCursor::new();
            #[allow(deprecated)]
            cursor.set_timeout_micros(self.contribution.timeout_micros());
            let mut captures = cursor.captures(
                &layer.highlights,
                embedded_tree.root_node(),
                window.text.as_bytes(),
            );
            let capture_names = layer.highlights.capture_names();
            loop {
                captures.advance();
                let Some((query_match, capture_index)) = captures.get() else {
                    break;
                };
                let capture = query_match.captures[*capture_index];
                let capture_name = capture_names[capture.index as usize];
                if !self.contribution.style_map.contains_key(capture_name) {
                    continue;
                }
                let absolute_start = window
                    .byte_start
                    .saturating_add(capture.node.start_byte() as u64);
                let absolute_end = window
                    .byte_start
                    .saturating_add(capture.node.end_byte() as u64);
                if absolute_start >= absolute_end {
                    continue;
                }
                syntax_captures.push(SyntaxCapture {
                    byte_start: absolute_start,
                    byte_end: absolute_end,
                    capture_name: capture_name.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Resolve an injection language name to a cached embedded layer, building
    /// it from the first-party registry on first use. Returns `None` for names
    /// with no registered first-party grammar.
    fn embedded_layer(
        &self,
        injections: &InjectionState,
        language: &str,
    ) -> Result<Option<Arc<EmbeddedLayer>>, TreeSitterSyntaxError> {
        if let Some(layer) = injections
            .layers
            .lock()
            .expect("embedded layer cache lock poisoned")
            .get(language)
        {
            return Ok(Some(Arc::clone(layer)));
        }
        let Some(descriptor) = FIRST_PARTY_EMBEDDED_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.name == language)
        else {
            return Ok(None);
        };
        let embedded_language = (descriptor.language)();
        let highlights =
            Query::new(&embedded_language, descriptor.highlights_query).map_err(|error| {
                TreeSitterSyntaxError::QueryCompileFailed {
                    message: error.to_string(),
                }
            })?;
        let mut parser = Parser::new();
        parser.set_language(&embedded_language).map_err(|error| {
            TreeSitterSyntaxError::QueryCompileFailed {
                message: error.to_string(),
            }
        })?;
        let layer = Arc::new(EmbeddedLayer {
            parser: Mutex::new(parser),
            highlights,
        });
        injections
            .layers
            .lock()
            .expect("embedded layer cache lock poisoned")
            .insert(language.to_string(), Arc::clone(&layer));
        Ok(Some(layer))
    }
}

fn input_edit(edit: crate::protocol::ParseInputEdit) -> Option<InputEdit> {
    Some(InputEdit {
        start_byte: usize::try_from(edit.start_byte).ok()?,
        old_end_byte: usize::try_from(edit.old_end_byte).ok()?,
        new_end_byte: usize::try_from(edit.new_end_byte).ok()?,
        start_position: Point::new(
            usize::try_from(edit.start_position.row).ok()?,
            usize::try_from(edit.start_position.column).ok()?,
        ),
        old_end_position: Point::new(
            usize::try_from(edit.old_end_position.row).ok()?,
            usize::try_from(edit.old_end_position.column).ok()?,
        ),
        new_end_position: Point::new(
            usize::try_from(edit.new_end_position.row).ok()?,
            usize::try_from(edit.new_end_position.column).ok()?,
        ),
    })
}

fn query_ranges(
    notification: &ParseEditNotification,
    window: &crate::protocol::ParseWindowSnapshot,
    old_tree: Option<&Tree>,
    new_tree: &Tree,
) -> Vec<std::ops::Range<usize>> {
    let visible_start = notification
        .viewport
        .start
        .saturating_sub(window.byte_start)
        .min(window.text.len() as u64) as usize;
    let visible_end = notification
        .viewport
        .end
        .saturating_sub(window.byte_start)
        .min(window.text.len() as u64) as usize;
    let visible = visible_start..visible_end;
    let Some(old_tree) = old_tree else {
        return normalize_query_ranges([visible.clone()], &window.text, visible);
    };

    let changed = old_tree
        .changed_ranges(new_tree)
        .map(|range| range.start_byte..range.end_byte);
    let explicit = notification.invalidated_ranges.iter().map(|range| {
        range
            .start
            .saturating_sub(window.byte_start)
            .min(window.text.len() as u64) as usize
            ..range
                .end
                .saturating_sub(window.byte_start)
                .min(window.text.len() as u64) as usize
    });
    normalize_query_ranges(changed.chain(explicit), &window.text, visible)
}

fn normalize_query_ranges(
    ranges: impl IntoIterator<Item = std::ops::Range<usize>>,
    text: &str,
    visible: std::ops::Range<usize>,
) -> Vec<std::ops::Range<usize>> {
    let mut ranges = ranges
        .into_iter()
        .filter_map(|range| {
            let mut start = range.start.min(text.len()).max(visible.start);
            let mut end = range.end.min(text.len()).min(visible.end);
            if start > end {
                return None;
            }
            while start > visible.start && !text.is_char_boundary(start) {
                start -= 1;
            }
            while end < visible.end && !text.is_char_boundary(end) {
                end += 1;
            }
            if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                return None;
            }
            if start == end {
                if end < visible.end {
                    end += text[end..].chars().next()?.len_utf8();
                } else if start > visible.start {
                    start -= text[..start].chars().next_back()?.len_utf8();
                }
            } else {
                if start > visible.start {
                    start -= text[..start].chars().next_back()?.len_utf8();
                }
                if end < visible.end {
                    end += text[end..].chars().next()?.len_utf8();
                }
            }
            (start < end).then_some(start..end)
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged: Vec<std::ops::Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(previous) = merged
            .last_mut()
            .filter(|previous| range.start <= previous.end)
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

fn replacement_ranges(
    affected_ranges: &[std::ops::Range<usize>],
    text: &str,
) -> Vec<std::ops::Range<usize>> {
    if affected_ranges.is_empty() || text.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut chunk_start = 0;
    for nominal_end in (SYNTAX_DECORATION_CHUNK_BYTES..text.len())
        .step_by(SYNTAX_DECORATION_CHUNK_BYTES)
        .chain(std::iter::once(text.len()))
    {
        let mut chunk_end = nominal_end;
        while chunk_end < text.len() && !text.is_char_boundary(chunk_end) {
            chunk_end += 1;
        }
        let chunk = chunk_start..chunk_end;
        if affected_ranges
            .iter()
            .any(|affected| affected.start < chunk.end && chunk.start < affected.end)
        {
            ranges.push(chunk.clone());
        }
        chunk_start = chunk_end;
    }
    ranges
}

pub fn map_capture_to_vocabulary(
    contribution: &SyntaxGrammarContribution,
    capture: &SyntaxCapture,
) -> Result<SyntaxVocabularySpan, TreeSitterSyntaxError> {
    let entry = contribution
        .style_map
        .get(&capture.capture_name)
        .ok_or_else(|| TreeSitterSyntaxError::QueryCaptureNotMapped {
            capture: capture.capture_name.clone(),
        })?;
    Ok(SyntaxVocabularySpan {
        byte_start: capture.byte_start,
        byte_end: capture.byte_end,
        token_type: entry.token_type,
        modifiers: entry.modifiers,
        scope: entry.scope.clone(),
        font_role: entry.font_role,
        priority: entry.priority,
    })
}

fn captures_to_decoration_spans(
    contribution: &SyntaxGrammarContribution,
    captures: Vec<SyntaxCapture>,
) -> Result<Vec<DecorationSpan>, TreeSitterSyntaxError> {
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
            // First-party vocabulary maps leave scope empty. Legacy grammar
            // contributions preserve their validated style token here.
            scope: vocabulary.scope,
            font_role: vocabulary.font_role,
            priority: vocabulary.priority,
            provenance: provenance.clone(),
        });
    }
    Ok(spans)
}

fn decoration_sets_for_ranges(
    notification: &ParseEditNotification,
    window: &crate::protocol::ParseWindowSnapshot,
    replacement_ranges: &[std::ops::Range<usize>],
    spans: Vec<DecorationSpan>,
) -> Vec<DecorationSet> {
    replacement_ranges
        .iter()
        .map(|range| {
            let chunk_start = window.byte_start.saturating_add(range.start as u64);
            let chunk_end = window.byte_start.saturating_add(range.end as u64);
            let chunk_spans = spans
                .iter()
                .filter_map(|span| {
                    let mut span = span.clone();
                    span.byte_start = span.byte_start.max(chunk_start);
                    span.byte_end = span.byte_end.min(chunk_end);
                    (span.byte_start < span.byte_end).then_some(span)
                })
                .collect();
            DecorationSet {
                document_id: notification.document_id,
                document_version: notification.document_version,
                package_prefix: notification.package_prefix.clone(),
                kind: DecorationKind::Syntax,
                viewport_byte_start: chunk_start,
                viewport_byte_end: chunk_end,
                spans: chunk_spans,
            }
        })
        .collect()
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
        decoration_updates: Vec::new(),
        diagnostic_update: None,
    }
}

fn map_decoration_error(error: DecorationValidationError) -> TreeSitterSyntaxError {
    TreeSitterSyntaxError::DecorationInvalid(format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        perf::metrics::PerfRecorder,
        protocol::{ParseByteRange, ParseWindowSnapshot},
    };

    fn notification(version: u64, text: &str) -> ParseEditNotification {
        ParseEditNotification {
            document_id: 7,
            document_version: version,
            behavior_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            viewport: ParseByteRange::new(0, text.len() as u64),
            invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
            accepted_edit: (version == 2).then_some(crate::protocol::ParseInputEdit {
                base_document_version: 1,
                document_version: 2,
                start_byte: 0,
                old_end_byte: 13,
                new_end_byte: text.len() as u64,
                start_position: crate::protocol::ParsePoint::new(0, 0),
                old_end_position: crate::protocol::ParsePoint::new(1, 0),
                new_end_position: crate::protocol::ParsePoint::new(1, 0),
            }),
            parse_windows: vec![ParseWindowSnapshot {
                document_id: 7,
                document_version: version,
                package_prefix: "rust".to_string(),
                mode_id: "rust.rust".to_string(),
                window_id: 0,
                byte_start: 0,
                byte_end: text.len() as u64,
                base_line: 0,
                base_column: 0,
                incremental_edit: version == 2,
                text: text.to_string(),
            }],
            memory_budget: None,
        }
    }

    // Regression: code grammars parse full-file context so a viewport
    // landing mid-expression no longer collapses into a recovery ERROR node
    // (white text) or invents tokens (e.g. bogus string spans).
    #[test]
    fn code_grammars_parse_full_file_context_for_viewport_output() {
        for descriptor in FIRST_PARTY_NATIVE_GRAMMARS {
            assert_eq!(
                descriptor.max_window_bytes, MAX_OPENABLE_FILE_BYTES,
                "{}",
                descriptor.id
            );
        }

        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "rust.rust")
            .unwrap();
        let handler = native_handler(&contribution_from_native_descriptor(descriptor))
            .unwrap()
            .unwrap();
        // Large source with a long expression; viewport lands mid-expression.
        let text = format!(
            "fn head() {{}}\nlet value = RuntimeOptions {{\n{}\n    ..Default::default()\n}};\nfn tail() -> usize {{ 42 }}\n",
            (0..120)
                .map(|index| format!("    field_{index}: Some(\"value_{index}\".to_string()),"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(text.len() > 4 * 1024);
        let viewport = ParseByteRange::new(1_024, 3_072);
        let scroll_notification = |viewport: ParseByteRange| ParseEditNotification {
            document_id: 7,
            document_version: 1,
            behavior_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            viewport,
            invalidated_ranges: vec![viewport],
            accepted_edit: None,
            parse_windows: vec![ParseWindowSnapshot {
                document_id: 7,
                document_version: 1,
                package_prefix: "rust".to_string(),
                mode_id: "rust.rust".to_string(),
                window_id: 0,
                byte_start: 0,
                byte_end: text.len() as u64,
                base_line: 0,
                base_column: 0,
                incremental_edit: false,
                text: text.clone(),
            }],
            memory_budget: None,
        };

        let update = handler
            .parse_sync(scroll_notification(viewport))
            .expect("full-file window parses");
        let string_spans = update
            .decoration_updates
            .iter()
            .flat_map(|set| set.spans.iter())
            .filter(|span| span.token_type == TokenType::String)
            .count();
        assert!(
            string_spans > 8,
            "mid-expression viewport keeps string highlights: {string_spans}"
        );
        assert!(
            update
                .decoration_updates
                .iter()
                .flat_map(|set| set.spans.iter())
                .all(|span| span.byte_start >= viewport.start && span.byte_end <= viewport.end),
            "decoration output stays viewport-limited"
        );

        // Scrolling the same version reuses the cached full-file tree.
        let scrolled = handler
            .parse_sync(scroll_notification(ParseByteRange::new(2_048, 4_096)))
            .expect("scroll reuses cached tree");
        assert_eq!(
            scrolled.syntax_tree_delta.as_deref(),
            Some("tree-sitter:rust:cached")
        );
        assert!(!scrolled.decoration_updates.is_empty());
    }

    #[test]
    fn markdown_native_descriptor_enables_inline_injection() {
        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "markdown.markdown")
            .expect("Markdown descriptor");
        assert!(descriptor.injections_query.is_some());

        let contribution = contribution_from_native_descriptor(descriptor);
        assert_eq!(
            contribution.injections_query_path.as_deref(),
            Some("packages/markdown/queries/injections.scm")
        );
        let handler = native_handler(&contribution)
            .expect("native handler builds")
            .expect("markdown descriptor resolves");
        assert!(handler.injections.is_some());

        // Non-composite grammars stay single-language.
        let rust = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "rust.rust")
            .expect("Rust descriptor");
        let handler = native_handler(&contribution_from_native_descriptor(rust))
            .expect("native handler builds")
            .expect("rust descriptor resolves");
        assert!(handler.injections.is_none());
    }

    #[test]
    fn same_version_markdown_scroll_reuses_full_document_tree_context() {
        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "markdown.markdown")
            .unwrap();
        let mut handler = native_handler(&contribution_from_native_descriptor(descriptor))
            .unwrap()
            .unwrap();
        let perf = PerfRecorder::for_test(true);
        handler.perf = perf.clone();
        let text = format!(
            "```text\n{}LAST CODE LINE\n```\n\nPlain prose after fence.\n",
            "code inside fence\n".repeat(300)
        );
        let scroll_start = text.find("LAST CODE LINE").unwrap() as u64;
        let prose = text.find("Plain prose after fence.").unwrap() as u64;

        let mut initial = notification(1, &text);
        initial.package_prefix = "markdown".to_string();
        initial.mode_id = "markdown.markdown".to_string();
        initial.viewport = ParseByteRange::new(0, 4_096);
        initial.invalidated_ranges = vec![initial.viewport];
        initial.parse_windows[0].package_prefix = initial.package_prefix.clone();
        initial.parse_windows[0].mode_id = initial.mode_id.clone();
        handler.parse_sync(initial).unwrap();

        let mut scrolled = notification(1, &text);
        scrolled.package_prefix = "markdown".to_string();
        scrolled.mode_id = "markdown.markdown".to_string();
        scrolled.viewport = ParseByteRange::new(scroll_start, text.len() as u64);
        scrolled.invalidated_ranges = vec![scrolled.viewport];
        scrolled.parse_windows[0].package_prefix = scrolled.package_prefix.clone();
        scrolled.parse_windows[0].mode_id = scrolled.mode_id.clone();
        let update = handler.parse_sync(scrolled).unwrap();

        assert_eq!(
            update.syntax_tree_delta.as_deref(),
            Some("tree-sitter:markdown:cached")
        );
        assert_eq!(
            perf.snapshots()
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INVOCATIONS)
                .count(),
            1
        );
        assert!(
            update
                .decoration_updates
                .iter()
                .any(|set| set
                    .spans
                    .iter()
                    .any(|span| span.token_type == TokenType::Paragraph
                        && span.byte_start <= prose
                        && span.byte_end > prose))
        );
        assert!(
            !update
                .decoration_updates
                .iter()
                .any(|set| set
                    .spans
                    .iter()
                    .any(|span| span.token_type == TokenType::CodeBlock
                        && span.byte_start <= prose
                        && span.byte_end > prose))
        );
    }

    #[test]
    fn query_ranges_merge_and_expand_utf8_safe_empty_invalidations() {
        let text = "aéz";

        let ranges = normalize_query_ranges([0..1, 1..1, 1..3, 3..3], text, 0..text.len());

        assert_eq!(ranges, vec![0..4]);
        assert!(ranges.iter().all(|range| {
            text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
        }));
    }

    #[test]
    fn replacement_ranges_move_shared_chunk_boundaries_past_utf8_scalars() {
        let text = format!("{}é{}", "a".repeat(127), "b".repeat(130));

        let affected = 127..130;
        let ranges = replacement_ranges(std::slice::from_ref(&affected), &text);

        assert_eq!(ranges, vec![0..129, 129..256]);
        assert!(ranges.iter().all(|range| {
            text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
        }));
    }

    #[test]
    fn incremental_parse_queries_less_than_unchanged_window() {
        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "rust.rust")
            .expect("Rust descriptor");
        let perf = PerfRecorder::for_test(true);
        let mut handler = TreeSitterSyntaxHandler::new(
            contribution_from_native_descriptor(descriptor),
            (descriptor.language)(),
            descriptor.highlights_query,
        )
        .expect("Rust handler");
        handler.perf = perf.clone();
        let suffix = " let distant = 2;".repeat(20);
        let old_text = format!("fn main() {{ le value = 1;{suffix} }}\n");
        let new_text = format!("fn main() {{ let value = 1;{suffix} }}\n");
        let insertion = old_text.find("le value").expect("partial keyword") + 2;

        handler
            .parse_sync(notification(1, &old_text))
            .expect("full parse");
        let mut incremental = notification(2, &new_text);
        incremental.invalidated_ranges =
            vec![ParseByteRange::new(insertion as u64, insertion as u64 + 1)];
        incremental.accepted_edit = Some(crate::protocol::ParseInputEdit {
            base_document_version: 1,
            document_version: 2,
            start_byte: insertion as u64,
            old_end_byte: insertion as u64,
            new_end_byte: insertion as u64 + 1,
            start_position: crate::protocol::ParsePoint::new(0, insertion as u64),
            old_end_position: crate::protocol::ParsePoint::new(0, insertion as u64),
            new_end_position: crate::protocol::ParsePoint::new(0, insertion as u64 + 1),
        });
        handler.parse_sync(incremental).expect("incremental parse");

        let queried_bytes = perf.snapshots().into_iter().find_map(|snapshot| {
            (snapshot.name == SYNTAX_QUERY_BYTES && snapshot.metadata.version == Some(2))
                .then_some(snapshot.value)
        });
        assert!(matches!(
            queried_bytes,
            Some(MetricValue::Bytes { bytes }) if bytes <= SYNTAX_DECORATION_CHUNK_BYTES as u64
                && bytes < new_text.len() as u64
        ));
    }

    #[test]
    fn decoration_member_count_does_not_multiply_parse_or_query_invocations() {
        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "rust.rust")
            .expect("Rust descriptor");
        let perf = PerfRecorder::for_test(true);
        let mut handler = TreeSitterSyntaxHandler::new(
            contribution_from_native_descriptor(descriptor),
            (descriptor.language)(),
            "(identifier) @keyword",
        )
        .expect("Rust handler");
        handler.perf = perf.clone();
        let text = (0..80)
            .map(|index| format!("let value_{index} = {index};"))
            .collect::<Vec<_>>()
            .join("\n");

        let update = handler
            .parse_sync(notification(1, &text))
            .expect("full parse");
        let snapshots = perf.snapshots();

        assert!(update.decoration_updates.len() > 1);
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INVOCATIONS)
                .count(),
            1
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_QUERY_RANGES)
                .count(),
            1
        );
    }

    #[test]
    fn first_party_continuity_edits_keep_one_bounded_parse_and_query() {
        for (id, old_text, new_text) in [
            ("rust.rust", "fn app() {}\n", "fn application() {}\n"),
            (
                "typescript.typescript",
                "function app() {}\n",
                "function application() {}\n",
            ),
            (
                "typescript.tsx",
                "function app() {}\n",
                "function application() {}\n",
            ),
            (
                "javascript.javascript",
                "function app() {}\n",
                "function application() {}\n",
            ),
            (
                "markdown.markdown",
                "Paragraph text.\n",
                "Paragraph texts.\n",
            ),
        ] {
            let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
                .iter()
                .find(|descriptor| descriptor.id == id)
                .unwrap_or_else(|| panic!("{id} descriptor"));
            let perf = PerfRecorder::for_test(true);
            let mut handler = TreeSitterSyntaxHandler::new(
                contribution_from_native_descriptor(descriptor),
                (descriptor.language)(),
                descriptor.highlights_query,
            )
            .unwrap_or_else(|error| panic!("{id} handler: {error}"));
            handler.perf = perf.clone();

            let prepare = |mut notification: ParseEditNotification| {
                notification.package_prefix = descriptor.package_prefix.to_string();
                notification.mode_id = descriptor.id.to_string();
                notification.parse_windows[0].package_prefix =
                    descriptor.package_prefix.to_string();
                notification.parse_windows[0].mode_id = descriptor.id.to_string();
                notification
            };
            handler
                .parse_sync(prepare(notification(1, old_text)))
                .unwrap_or_else(|error| panic!("{id} initial parse: {error}"));

            let start = old_text
                .bytes()
                .zip(new_text.bytes())
                .position(|(old, new)| old != new)
                .unwrap_or_else(|| old_text.len().min(new_text.len()));
            let mut old_end = old_text.len();
            let mut new_end = new_text.len();
            while old_end > start
                && new_end > start
                && old_text.as_bytes()[old_end - 1] == new_text.as_bytes()[new_end - 1]
            {
                old_end -= 1;
                new_end -= 1;
            }
            let point = |text: &str, offset: usize| {
                let prefix = &text[..offset];
                crate::protocol::ParsePoint::new(
                    prefix.bytes().filter(|byte| *byte == b'\n').count() as u64,
                    prefix
                        .rsplit_once('\n')
                        .map_or(prefix.len(), |(_, tail)| tail.len()) as u64,
                )
            };
            let mut incremental = prepare(notification(2, new_text));
            incremental.invalidated_ranges =
                vec![ParseByteRange::new(start as u64, new_end as u64)];
            incremental.accepted_edit = Some(crate::protocol::ParseInputEdit {
                base_document_version: 1,
                document_version: 2,
                start_byte: start as u64,
                old_end_byte: old_end as u64,
                new_end_byte: new_end as u64,
                start_position: point(old_text, start),
                old_end_position: point(old_text, old_end),
                new_end_position: point(new_text, new_end),
            });
            incremental.parse_windows[0].incremental_edit = true;
            let update = handler
                .parse_sync(incremental)
                .unwrap_or_else(|error| panic!("{id} incremental parse: {error}"));
            let snapshots = perf.snapshots();
            let parse_count = snapshots
                .iter()
                .filter(|snapshot| {
                    snapshot.name == SYNTAX_PARSE_INVOCATIONS
                        && snapshot.metadata.version == Some(2)
                })
                .count();
            let query_bytes = snapshots
                .iter()
                .find_map(|snapshot| {
                    (snapshot.name == SYNTAX_QUERY_BYTES && snapshot.metadata.version == Some(2))
                        .then_some(&snapshot.value)
                })
                .and_then(|value| match value {
                    MetricValue::Bytes { bytes } => Some(*bytes),
                    _ => None,
                })
                .expect("incremental query bytes");
            let query_count = snapshots
                .iter()
                .filter(|snapshot| {
                    snapshot.name == SYNTAX_QUERY_RANGES && snapshot.metadata.version == Some(2)
                })
                .count();
            let members = update.decoration_updates.len();

            eprintln!(
                "{id}: parses={parse_count}, query_ranges={query_count}, query_bytes={query_bytes}, members={members}"
            );
            assert_eq!(parse_count, 1, "{id}: parser calls");
            assert_eq!(query_count, 1, "{id}: query calls");
            assert!(
                query_bytes <= SYNTAX_DECORATION_CHUNK_BYTES as u64,
                "{id}: query bytes {query_bytes}"
            );
            assert_eq!(members, 1, "{id}: touched replacement members");
        }
    }

    #[test]
    fn native_parse_records_source_safe_work_classification_and_query_counts() {
        let descriptor = FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .find(|descriptor| descriptor.id == "rust.rust")
            .expect("Rust descriptor");
        let perf = PerfRecorder::for_test(true);
        let mut handler = TreeSitterSyntaxHandler::new(
            contribution_from_native_descriptor(descriptor),
            (descriptor.language)(),
            descriptor.highlights_query,
        )
        .expect("Rust handler");
        handler.perf = perf.clone();

        handler
            .parse_sync(notification(1, "fn main() {}\n"))
            .expect("full parse");
        handler
            .parse_sync(notification(2, "fn main() { let value = 1; }\n"))
            .expect("incremental parse");

        let snapshots = perf.snapshots();
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INVOCATIONS)
                .count(),
            2
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_PARSE_FULL)
                .count(),
            1
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_PARSE_INCREMENTAL)
                .count(),
            1
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_QUERY_RANGES)
                .count(),
            2
        );
        assert!(snapshots.iter().all(|snapshot| {
            snapshot.metadata.document_id == Some(7)
                && snapshot.metadata.version.is_some()
                && snapshot.metadata.sanitized_path.is_none()
        }));
    }
}
