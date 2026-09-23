use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    ops::Range,
    sync::{Arc, Mutex},
};

use crate::lock_util::LockOrRecover;
use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Language, Node, Parser, Point, Query, QueryCursor, Tree};

const SYNTAX_DECORATION_CHUNK_BYTES: usize = 128;

use crate::{
    packages::{
        modes::{DocumentClassificationInput, MajorModeActivation},
        record::{PackageRecord, SyntaxGrammarContributionDescriptor},
    },
    perf::{
        budgets::{
            DECORATION_PAYLOAD_BUDGET_BYTES, INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
            NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
        },
        metrics::{
            MetricMetadata, MetricValue, PerfRecorder, SYNTAX_PARSE_FULL, SYNTAX_PARSE_INCREMENTAL,
            SYNTAX_PARSE_INVOCATIONS, SYNTAX_QUERY_BYTES, SYNTAX_QUERY_RANGES, global_recorder,
        },
    },
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentId,
        IncrementalParseUpdate, Modifiers, ParseByteRange, ParseEditNotification, ParseUnit,
        SelectionQuery, SelectionQueryCursor, SelectionQueryRange, SmartSelectAction,
        TextobjectDirection, TextobjectKind, TokenType,
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
    OwnedByNativeDescriptor {
        language_id: String,
        package_prefix: String,
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
    /// Optional text-object query (`queries/textobjects.scm`, Plan 071 task
    /// 10). Captures follow `textobject.<kind>.<inner|around>` naming and back
    /// read-only `clientSelectTextobject`/`clientSmartSelect` ranges.
    textobjects_query_path: Option<&'static str>,
    textobjects_query: Option<&'static str>,
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
    highlights_query: include_str!("../../../packages/markdown/queries/inline-highlights.scm"),
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
            .field("textobjects_query_path", &self.textobjects_query_path)
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
        highlights_query: include_str!("../../../packages/rust/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_rust::LANGUAGE.into(),
        // Code meaning (strings, expressions, macro bodies) can begin well
        // before the visible viewport; a viewport-sized fragment can parse as
        // one recovery ERROR node (lost highlights) or invent tokens (e.g. a
        // closing quote re-read as a string opener). Parse bounded full-file
        // context while query and decoration output remain viewport-limited,
        // same as the markdown grammar below.
        max_window_bytes: NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
        injections_query_path: None,
        injections_query: None,
        textobjects_query_path: Some("packages/rust/queries/textobjects.scm"),
        textobjects_query: Some(include_str!(
            "../../../packages/rust/queries/textobjects.scm"
        )),
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
        highlights_query: include_str!("../../../packages/typescript/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        // See rust.rust: full-file context, viewport-limited output.
        max_window_bytes: NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
        injections_query_path: None,
        injections_query: None,
        textobjects_query_path: Some("packages/typescript/queries/textobjects.scm"),
        textobjects_query: Some(include_str!(
            "../../../packages/typescript/queries/textobjects.scm"
        )),
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
        highlights_query: include_str!("../../../packages/typescript/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        // See rust.rust: full-file context, viewport-limited output.
        max_window_bytes: NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
        injections_query_path: None,
        injections_query: None,
        textobjects_query_path: Some("packages/typescript/queries/textobjects.scm"),
        textobjects_query: Some(include_str!(
            "../../../packages/typescript/queries/textobjects.scm"
        )),
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
        highlights_query: include_str!("../../../packages/javascript/queries/highlights.scm"),
        style_map: DEFAULT_NATIVE_STYLE_MAP,
        language: || tree_sitter_javascript::LANGUAGE.into(),
        // See rust.rust: full-file context, viewport-limited output.
        max_window_bytes: NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
        injections_query_path: None,
        injections_query: None,
        textobjects_query_path: Some("packages/javascript/queries/textobjects.scm"),
        textobjects_query: Some(include_str!(
            "../../../packages/javascript/queries/textobjects.scm"
        )),
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
        highlights_query: include_str!("../../../packages/markdown/queries/highlights.scm"),
        style_map: MARKDOWN_NATIVE_STYLE_MAP,
        language: || tree_sitter_md_025::LANGUAGE.into(),
        // Markdown block meaning (notably fenced-code state) can begin before
        // the visible viewport. Parse bounded full-file context while query
        // and decoration output remain viewport-limited.
        max_window_bytes: NATIVE_GRAMMAR_MAX_WINDOW_BYTES,
        injections_query_path: Some("packages/markdown/queries/injections.scm"),
        injections_query: Some(include_str!(
            "../../../packages/markdown/queries/injections.scm"
        )),
        // Markdown has no function/class/argument text objects worth shipping;
        // smart select still works off the parsed block tree.
        textobjects_query_path: None,
        textobjects_query: None,
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
    (
        "punctuation.bracket",
        TokenType::Operator,
        Modifiers::NONE,
        None,
        70,
    ),
    (
        "punctuation.delimiter",
        TokenType::Operator,
        Modifiers::NONE,
        None,
        70,
    ),
    (
        "punctuation.special",
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
    (
        "function.macro",
        TokenType::Macro,
        Modifiers::NONE,
        None,
        70,
    ),
    (
        "function.method",
        TokenType::Method,
        Modifiers::NONE,
        None,
        70,
    ),
    ("type", TokenType::Type, Modifiers::NONE, None, 70),
    ("type.builtin", TokenType::Type, Modifiers::NONE, None, 70),
    (
        "type.lifetime",
        TokenType::TypeParameter,
        Modifiers::NONE,
        None,
        70,
    ),
    (
        "type.parameter",
        TokenType::TypeParameter,
        Modifiers::NONE,
        None,
        70,
    ),
    ("number", TokenType::Number, Modifiers::NONE, None, 70),
    ("constant", TokenType::EnumMember, Modifiers::NONE, None, 70),
    (
        "constant.builtin",
        TokenType::EnumMember,
        Modifiers::NONE,
        None,
        70,
    ),
    ("property", TokenType::Property, Modifiers::NONE, None, 70),
    (
        "variable.parameter",
        TokenType::Parameter,
        Modifiers::NONE,
        None,
        70,
    ),
    (
        "variable.builtin",
        TokenType::Variable,
        Modifiers::NONE,
        None,
        70,
    ),
    ("variable", TokenType::Variable, Modifiers::NONE, None, 40),
    ("operator", TokenType::Operator, Modifiers::NONE, None, 70),
    ("attribute", TokenType::Decorator, Modifiers::NONE, None, 70),
    (
        "string.regexp",
        TokenType::Regexp,
        Modifiers::NONE,
        None,
        70,
    ),
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
    ("code-label", TokenType::String, Modifiers::NONE, None, 80),
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
    ("link-url", TokenType::String, Modifiers::NONE, None, 85),
    ("quote", TokenType::Quote, Modifiers::NONE, None, 90),
    ("quote-marker", TokenType::Quote, Modifiers::NONE, None, 90),
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

    pub fn native_owned_syntax_languages(package_prefix: &str) -> Vec<&'static str> {
        FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .filter(|descriptor| descriptor.package_prefix == package_prefix)
            .map(|descriptor| descriptor.language_id)
            .collect()
    }

    pub fn native_owned_grammar_ids(package_prefix: &str) -> Vec<&'static str> {
        FIRST_PARTY_NATIVE_GRAMMARS
            .iter()
            .filter(|descriptor| descriptor.package_prefix == package_prefix)
            .map(|descriptor| descriptor.id)
            .collect()
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
                return Err(SyntaxGrammarRegistryError::OwnedByNativeDescriptor {
                    language_id: contribution.language_id.clone(),
                    package_prefix: contribution.package_prefix.clone(),
                });
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
    if let Some(textobjects_query) = descriptor.textobjects_query {
        handler.enable_textobjects(textobjects_query)?;
    }
    Ok(Some(handler))
}

/// Picks one candidate text-object range for a caret `focus` and `direction`.
///
/// `Current` returns the innermost candidate containing the caret, `Next` the
/// earliest candidate strictly after it, `Previous` the latest candidate ending
/// at or before it. No wrapping: a miss yields `None` so the client leaves the
/// selection unchanged.
fn pick_textobject_range(
    candidates: &[Range<usize>],
    focus: usize,
    direction: TextobjectDirection,
) -> Option<Range<usize>> {
    match direction {
        TextobjectDirection::Current => candidates
            .iter()
            .filter(|range| range.start <= focus && focus <= range.end)
            .min_by_key(|range| range.end - range.start)
            .cloned(),
        TextobjectDirection::Next => candidates
            .iter()
            .filter(|range| range.start > focus)
            .min_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)))
            .cloned(),
        TextobjectDirection::Previous => candidates
            .iter()
            .filter(|range| range.end <= focus)
            .max_by(|left, right| left.end.cmp(&right.end).then(left.start.cmp(&right.start)))
            .cloned(),
    }
}

/// Smart-select ranges for every selection cursor, walking the parsed tree.
///
/// `Expand` grows each selection to the smallest node range strictly larger
/// than it (the parent chain), `Shrink` returns to the largest node range
/// strictly contained in the current selection. Reads only; bounded by tree
/// size; never mutates.
fn smart_select_ranges(
    tree: &Tree,
    text: &str,
    action: SmartSelectAction,
    selections: &[SelectionQueryCursor],
) -> Vec<Option<SelectionQueryRange>> {
    let text_len = text.len();
    selections
        .iter()
        .map(|selection| {
            let anchor = usize::try_from(selection.anchor.min(text_len as u64)).unwrap_or(text_len);
            let focus = usize::try_from(selection.focus.min(text_len as u64)).unwrap_or(text_len);
            let (start, end) = if anchor <= focus {
                (anchor, focus)
            } else {
                (focus, anchor)
            };
            let range = match action {
                SmartSelectAction::Expand => expand_smart_select(tree.root_node(), start, end),
                SmartSelectAction::Shrink => shrink_smart_select(tree.root_node(), start, end),
            };
            range.map(|range| SelectionQueryRange {
                start: range.start as u64,
                end: range.end as u64,
            })
        })
        .collect()
}

/// Walks up the tree from the node covering `[start, end]` to the first
/// ancestor whose range strictly contains the selection.
fn expand_smart_select(root: Node<'_>, start: usize, end: usize) -> Option<Range<usize>> {
    let mut node = root.descendant_for_byte_range(start, end)?;
    loop {
        let range = node.byte_range();
        if range.start <= start && range.end >= end && (range.start < start || range.end > end) {
            return Some(range);
        }
        node = node.parent()?;
    }
}

/// Finds the largest node range strictly contained in `[start, end]`, the
/// inverse of one expand step. Collapsed selections and leaf-only bodies have
/// nothing to shrink to.
fn shrink_smart_select(root: Node<'_>, start: usize, end: usize) -> Option<Range<usize>> {
    if start >= end {
        return None;
    }
    let container = root.descendant_for_byte_range(start, end)?;
    let mut best: Option<Range<usize>> = None;
    let mut stack = vec![container];
    while let Some(node) = stack.pop() {
        let range = node.byte_range();
        let strictly_inside =
            range.start >= start && range.end <= end && (range.start > start || range.end < end);
        if strictly_inside
            && best
                .as_ref()
                .is_none_or(|current| (range.end - range.start) > (current.end - current.start))
        {
            best = Some(range);
        }
        for index in 0..node.child_count() {
            if let Some(child) = node.child(index) {
                stack.push(child);
            }
        }
    }
    best
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
    highlights_query: Arc<Query>,
    /// Optional compiled text-object query (Plan 071 task 10). Absent for
    /// grammars without a `textobjects.scm`; selection queries then return no
    /// ranges and smart select keeps working off the parsed tree.
    textobjects_query: Option<Arc<Query>>,
    injections: Option<Arc<InjectionState>>,
    trees: Arc<Mutex<HashMap<DocumentId, CachedSyntaxState>>>,
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

/// Plan 099: per-document syntax state. Each document owns its parser (so
/// same-language documents parse concurrently without a grammar-global lock)
/// plus its latest tree for incremental reuse. Only the tree is ever cloned
/// out of the map; the parser stays with the document's state.
struct CachedSyntaxState {
    document_version: u64,
    window_id: u64,
    tree: Tree,
    parser: Parser,
}

impl fmt::Debug for CachedSyntaxState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedSyntaxState")
            .field("document_version", &self.document_version)
            .field("window_id", &self.window_id)
            .finish_non_exhaustive()
    }
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
        Ok(Self {
            contribution,
            language,
            highlights_query: Arc::new(query),
            textobjects_query: None,
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

    /// Compiles the grammar's text-object query (Plan 071 task 10). Capture
    /// names must follow `textobject.<kind>.<inner|around>`; anything else is
    /// inert. A compile failure rejects the whole contribution fail-closed,
    /// matching the highlights/injection query contract.
    pub fn enable_textobjects(
        &mut self,
        textobjects_query: &str,
    ) -> Result<(), TreeSitterSyntaxError> {
        let query = Query::new(&self.language, textobjects_query).map_err(|error| {
            TreeSitterSyntaxError::QueryCompileFailed {
                message: error.to_string(),
            }
        })?;
        self.textobjects_query = Some(Arc::new(query));
        Ok(())
    }

    /// Read-only tree-sitter text-object/smart-select ranges for one request.
    /// Reuses the cached full-document tree at the same version when present
    /// (native descriptors parse full-file context); otherwise runs one
    /// bounded fresh parse with the contribution timeout. Smart select walks
    /// the tree even when no text-object query exists.
    pub fn selection_query_ranges(
        &self,
        document_id: DocumentId,
        document_version: u64,
        text: &str,
        query: SelectionQuery,
        selections: &[SelectionQueryCursor],
    ) -> Result<Vec<Option<SelectionQueryRange>>, TreeSitterSyntaxError> {
        let tree = self.tree_for_selection_query(document_id, document_version, text)?;
        Ok(match query {
            SelectionQuery::Textobject {
                kind,
                around,
                direction,
            } => self.textobject_ranges(&tree, text, kind, around, direction, selections),
            SelectionQuery::SmartSelect { action } => {
                smart_select_ranges(&tree, text, action, selections)
            }
        })
    }

    /// Build a fresh parser for this handler's language (per-document parser
    /// construction; Plan 099 removed the grammar-global parser mutex).
    fn build_parser(&self) -> Parser {
        let mut parser = Parser::new();
        // The language was validated at construction; a failure here would
        // have already failed `TreeSitterSyntaxHandler::new`.
        let _ = parser.set_language(&self.language);
        parser
    }

    /// Evict arbitrary entries beyond the bounded per-document cache so cold
    /// grammars from closed documents cannot accumulate unbounded tree state,
    /// while preserving the newly inserted document.
    fn bound_tree_cache(
        &self,
        trees: &mut HashMap<DocumentId, CachedSyntaxState>,
        keep: DocumentId,
    ) {
        while trees.len() > crate::perf::budgets::SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES {
            let victim = trees.keys().find(|&&k| k != keep).copied();
            let Some(victim) = victim else {
                break;
            };
            trees.remove(&victim);
        }
    }

    fn tree_for_selection_query(
        &self,
        document_id: DocumentId,
        document_version: u64,
        text: &str,
    ) -> Result<Tree, TreeSitterSyntaxError> {
        let cached = self
            .trees
            .lock_or_recover()
            .get(&document_id)
            .filter(|cached| {
                cached.document_version == document_version
                    && cached.tree.root_node().end_byte() == text.len()
            })
            .map(|cached| cached.tree.clone());
        if let Some(tree) = cached {
            return Ok(tree);
        }
        // One-shot parse with a parser taken out of the document's state (or
        // a fresh one); same-language documents never contend on a shared
        // parser (Plan 099).
        let mut parser = self
            .trees
            .lock_or_recover()
            .get_mut(&document_id)
            .map(|state| std::mem::replace(&mut state.parser, self.build_parser()))
            .unwrap_or_else(|| self.build_parser());
        #[allow(deprecated)]
        parser.set_timeout_micros(self.contribution.timeout_micros());
        let tree = parser
            .parse(text, None)
            .ok_or(TreeSitterSyntaxError::ParseTimedOut)?;
        if let Some(state) = self.trees.lock_or_recover().get_mut(&document_id) {
            state.parser = parser;
        }
        Ok(tree)
    }

    fn textobject_ranges(
        &self,
        tree: &Tree,
        text: &str,
        kind: TextobjectKind,
        around: bool,
        direction: TextobjectDirection,
        selections: &[SelectionQueryCursor],
    ) -> Vec<Option<SelectionQueryRange>> {
        let Some(query) = self.textobjects_query.as_ref() else {
            return vec![None; selections.len()];
        };
        let around_name = format!("textobject.{kind}.around", kind = kind.as_str());
        let inner_name = format!("textobject.{kind}.inner", kind = kind.as_str());
        let capture_names = query.capture_names();
        let mut around_ranges: Vec<Range<usize>> = Vec::new();
        let mut inner_ranges: Vec<Range<usize>> = Vec::new();
        let mut cursor = QueryCursor::new();
        let mut query_matches = cursor.matches(query, tree.root_node(), text.as_bytes());
        loop {
            query_matches.advance();
            let Some(query_match) = query_matches.get() else {
                break;
            };
            for capture in query_match.captures {
                match capture_names.get(capture.index as usize) {
                    Some(name) if *name == around_name => {
                        around_ranges.push(capture.node.byte_range())
                    }
                    Some(name) if *name == inner_name => {
                        inner_ranges.push(capture.node.byte_range())
                    }
                    _ => {}
                }
            }
        }
        // Inner falls back to around when the grammar defines no inner capture
        // for this kind (comments, argument lists, ...).
        let candidates: &[Range<usize>] = if !around && !inner_ranges.is_empty() {
            &inner_ranges
        } else {
            &around_ranges
        };
        selections
            .iter()
            .map(|selection| {
                let focus =
                    usize::try_from(selection.focus.min(text.len() as u64)).unwrap_or(text.len());
                pick_textobject_range(candidates, focus, direction).map(|range| {
                    SelectionQueryRange {
                        start: range.start as u64,
                        end: range.end as u64,
                    }
                })
            })
            .collect()
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
                .lock_or_recover()
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
            .lock_or_recover()
            .get(&notification.document_id)
            .filter(|cached| {
                cached.document_version == notification.document_version
                    && cached.window_id == window.window_id
                    && cached.tree.root_node().end_byte() == window.text.len()
            })
            .map(|cached| cached.tree.clone());
        // The freshly parsed tree plus the document parser it was produced
        // with (returned to the cache below). A cache hit keeps the existing
        // state untouched.
        let mut reparsed: Option<(Tree, &'static str, Parser)> = None;
        let (tree, parse_kind) = if let Some(tree) = cached_tree {
            (tree, "cached")
        } else {
            let metadata =
                MetricMetadata::document(notification.document_id, notification.document_version)
                    .with_trace_id(notification.trace_id);
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

            // Plan 099: parse with the document's own parser. Only one
            // session job per document runs at a time, so the parser cannot
            // contend with itself, and same-language documents each own a
            // parser (no grammar-global mutex).
            let mut parser = self
                .trees
                .lock_or_recover()
                .get_mut(&notification.document_id)
                .map(|state| std::mem::replace(&mut state.parser, self.build_parser()))
                .unwrap_or_else(|| self.build_parser());
            #[allow(deprecated)]
            parser.set_timeout_micros(self.contribution.timeout_micros());
            let parse_kind = if old_tree.is_some() {
                "incremental"
            } else {
                "full"
            };
            let Some(tree) = parser.parse(&window.text, old_tree.as_ref()) else {
                // Give the parser back before failing so the document keeps
                // its reusable state.
                if let Some(state) = self
                    .trees
                    .lock_or_recover()
                    .get_mut(&notification.document_id)
                {
                    state.parser = parser;
                }
                return Err(TreeSitterSyntaxError::ParseTimedOut);
            };
            reparsed = Some((tree.clone(), parse_kind, parser));
            (tree, parse_kind)
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
        let folding_update = Some(crate::server::folding::folds_from_syntax_tree(
            &tree,
            notification.document_id,
            notification.document_version,
        ));
        if let Some((tree, parse_kind, parser)) = reparsed {
            let _ = parse_kind;
            let mut trees = self.trees.lock_or_recover();
            trees.insert(
                notification.document_id,
                CachedSyntaxState {
                    document_version: notification.document_version,
                    window_id: window.window_id,
                    tree,
                    parser,
                },
            );
            self.bound_tree_cache(&mut trees, notification.document_id);
        }

        Ok(IncrementalParseUpdate {
            document_id: notification.document_id,
            document_version: notification.document_version,
            behavior_version: notification.behavior_version,
            package_prefix: notification.package_prefix,
            mode_id: notification.mode_id,
            parse_unit: ParseUnit::Region,
            viewport: update_viewport,
            invalidated_ranges: vec![update_viewport],
            trace_id: notification.trace_id,
            request_id: notification.request_id,
            client_id: None,
            syntax_tree_delta: Some(format!(
                "tree-sitter:{}:{parse_kind}",
                self.contribution.language_id,
            )),
            decoration_updates,
            // Tree-sitter recovery nodes are unreliable on bounded viewport
            // fragments. Diagnostics remain reserved for explicit analyzers
            // (including future LSP packages), not syntax highlighting.
            diagnostic_update: None,
            folding_update,
        })
    }

    pub fn cached_tree_version(&self, document_id: DocumentId) -> Option<u64> {
        self.trees
            .lock_or_recover()
            .get(&document_id)
            .map(|cached| cached.document_version)
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
            MetricMetadata::document(notification.document_id, notification.document_version)
                .with_trace_id(notification.trace_id);
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
        let mut sets = decoration_sets_for_ranges(notification, window, replacement_ranges, spans)
            .into_iter()
            .flat_map(|set| split_decoration_set_to_update_budget(notification, set))
            .collect::<Vec<_>>();
        sets.sort_by_key(|set| {
            !notification.invalidated_ranges.iter().any(|range| {
                range.intersects(ParseByteRange::new(
                    set.viewport_byte_start,
                    set.viewport_byte_end,
                ))
            })
        });
        let mut cache = self.decoration_cache.lock_or_recover();
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
            // Plan 134 D5 keep-expect: tree-sitter `Parser` owns mutable FFI
            // state; a panic mid-parse leaves it unsafe to reuse, so poison must
            // fail loudly instead of resuming with a torn parser.
            let mut parser = layer.parser.lock().expect("embedded parser lock poisoned");
            #[allow(deprecated)]
            parser.set_timeout_micros(self.contribution.timeout_micros());
            let capture_names = layer.highlights.capture_names();
            // Parse each injection range as its own document. Batching every
            // range into one `set_included_ranges` call feeds the bytes
            // *between* ranges (newlines, list markers, block punctuation) to
            // the embedded grammar as inline content, so e.g. markdown_inline
            // paired backticks across lines and painted following plain text as
            // code. One parse per range keeps each inline block isolated; the
            // captures are window-relative and clipped to the viewport later.
            for range in &ranges {
                if parser.set_included_ranges(&[*range]).is_err() {
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
        if let Some(layer) = injections.layers.lock_or_recover().get(language) {
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
            .lock_or_recover()
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
            target: None,
            inlay: None,
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
                trace_id: notification.trace_id,
            }
        })
        .collect()
}

fn update_with_single_set(
    notification: &ParseEditNotification,
    set: DecorationSet,
) -> IncrementalParseUpdate {
    let mut update = empty_update(notification.clone());
    update.decoration_updates = vec![set];
    update
}

fn decoration_set_fits_update_budget(
    notification: &ParseEditNotification,
    set: &DecorationSet,
) -> bool {
    rkyv::to_bytes::<rkyv::rancor::Error>(&update_with_single_set(notification, set.clone()))
        .map(|bytes| bytes.len() <= INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
        .unwrap_or(false)
}

fn tighten_viewport_to_spans(set: &mut DecorationSet) {
    let Some(first) = set.spans.first() else {
        return;
    };
    let mut start = first.byte_start;
    let mut end = first.byte_end;
    for span in &set.spans {
        start = start.min(span.byte_start);
        end = end.max(span.byte_end);
    }
    set.viewport_byte_start = start;
    set.viewport_byte_end = end;
}

fn split_decoration_set_to_update_budget(
    notification: &ParseEditNotification,
    mut set: DecorationSet,
) -> Vec<DecorationSet> {
    if set.spans.len() <= 1 || decoration_set_fits_update_budget(notification, &set) {
        return vec![set];
    }
    // Halves must claim only their own span range. Cloning the parent
    // viewport made every fragment a full-window replaceCovered, so the
    // last 4 KiB payload wiped syntax at the top of large files.
    set.spans
        .sort_by_key(|span| (span.byte_start, span.byte_end));
    let mid = set.spans.len() / 2;
    let mut left = set.clone();
    let mut right = set;
    left.spans.truncate(mid);
    right.spans.drain(..mid);
    tighten_viewport_to_spans(&mut left);
    tighten_viewport_to_spans(&mut right);
    let mut out = split_decoration_set_to_update_budget(notification, left);
    out.extend(split_decoration_set_to_update_budget(notification, right));
    out
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

    fn parse_blocking(
        &self,
        notification: ParseEditNotification,
    ) -> Option<Result<IncrementalParseUpdate, ParseCoordinatorError>> {
        Some(
            self.parse_sync(notification)
                .map_err(|error| ParseCoordinatorError::HandlerFailed(error.to_string())),
        )
    }

    fn runs_on_blocking_executor(&self) -> bool {
        true
    }

    fn selection_query_ranges(
        &self,
        document_id: crate::protocol::DocumentId,
        document_version: u64,
        text: &str,
        query: crate::protocol::SelectionQuery,
        selections: &[crate::protocol::SelectionQueryCursor],
    ) -> Option<Vec<Option<crate::protocol::SelectionQueryRange>>> {
        // A timed-out/failed parse degrades to "no ranges" instead of an
        // error: selection queries are advisory view state.
        self.selection_query_ranges(document_id, document_version, text, query, selections)
            .ok()
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
        trace_id: notification.trace_id,
        request_id: notification.request_id,
        client_id: None,
        syntax_tree_delta: None,
        decoration_updates: Vec::new(),
        diagnostic_update: None,
        folding_update: None,
    }
}

fn map_decoration_error(error: DecorationValidationError) -> TreeSitterSyntaxError {
    TreeSitterSyntaxError::DecorationInvalid(format!("{error:?}"))
}

#[cfg(test)]
mod tests;
