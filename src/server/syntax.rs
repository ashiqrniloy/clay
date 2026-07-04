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
        IncrementalParseUpdate, ParseEditNotification, ParseUnit,
    },
    server::{
        decorations::{DecorationValidationError, SyntaxChunkCache, validate_decoration_set},
        parse_coordinator::{ParseCoordinatorError, ParseHandlerFuture},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxGrammarContribution {
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
}

#[derive(Debug, Clone, Default)]
pub struct SyntaxGrammarRegistry {
    grammars_by_id: BTreeMap<String, SyntaxGrammarContribution>,
    language_to_id: BTreeMap<String, String>,
    extension_to_id: BTreeMap<String, String>,
    file_name_to_id: BTreeMap<String, String>,
    active_selections: BTreeMap<DocumentId, SyntaxGrammarSelection>,
}

impl SyntaxGrammarRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_package(
        &mut self,
        package: &PackageRecord,
    ) -> Result<usize, SyntaxGrammarRegistryError> {
        let mut staged = Vec::with_capacity(package.contributions.syntax_grammars.len());
        for descriptor in &package.contributions.syntax_grammars {
            let contribution = SyntaxGrammarContribution::from_descriptor(package, descriptor);
            self.validate_no_conflict(&contribution)?;
            for staged_contribution in &staged {
                self.validate_pair_no_conflict(staged_contribution, &contribution)?;
            }
            staged.push(contribution);
        }

        let count = staged.len();
        for contribution in staged {
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
        let active_syntax_grammar = candidate.map(|(matched_by, grammar)| ActiveSyntaxGrammar {
            document_id: input.document_id,
            document_version,
            contribution_id: grammar.id.clone(),
            language_id: grammar.language_id.clone(),
            package_name: grammar.package_name.clone(),
            package_version: grammar.package_version.clone(),
            package_prefix: grammar.package_prefix.clone(),
            matched_by,
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

fn selection_rationale(path: &Option<String>, grammar: &Option<ActiveSyntaxGrammar>) -> String {
    match grammar {
        Some(grammar) => format!(
            "active syntax grammar {} from {} selected by {} without changing active major mode",
            grammar.language_id,
            grammar.package_name,
            match grammar.matched_by {
                SyntaxGrammarPatternKind::FileName => "filename",
                SyntaxGrammarPatternKind::Extension => "extension",
            }
        ),
        None if path.is_some() => {
            "no loaded syntax grammar matched; document remains editable with its active major mode"
                .to_string()
        }
        None => "no document path supplied; document remains editable with its active major mode"
            .to_string(),
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

#[derive(Debug, Clone)]
pub struct TreeSitterSyntaxHandler {
    contribution: SyntaxGrammarContribution,
    language: Language,
    highlights_query: Arc<Query>,
    trees: Arc<Mutex<HashMap<DocumentId, CachedSyntaxTree>>>,
    decoration_cache: Arc<Mutex<SyntaxChunkCache>>,
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
        Ok(Self {
            contribution,
            language,
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

        let mut parser = Parser::new();
        parser.set_language(&self.language).map_err(|error| {
            TreeSitterSyntaxError::QueryCompileFailed {
                message: error.to_string(),
            }
        })?;
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
        cursor.set_timeout_micros(self.contribution.timeout_micros());
        cursor.set_byte_range(relative_viewport_start..relative_viewport_end);
        let mut captures = cursor.captures(
            &self.highlights_query,
            tree.root_node(),
            window.text.as_bytes(),
        );
        let capture_names = self.highlights_query.capture_names();
        let provenance = self.contribution.provenance();
        let mut spans = Vec::new();

        loop {
            captures.advance();
            let Some((query_match, capture_index)) = captures.get() else {
                break;
            };
            if spans.len() >= MAX_SYNTAX_HIGHLIGHT_SPANS {
                return Err(TreeSitterSyntaxError::CaptureLimitExceeded {
                    limit: MAX_SYNTAX_HIGHLIGHT_SPANS,
                });
            }
            let capture = query_match.captures[*capture_index];
            let capture_name = capture_names[capture.index as usize];
            let Some(style_token) = self.contribution.style_map.get(capture_name) else {
                continue;
            };
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
            spans.push(DecorationSpan {
                byte_start,
                byte_end,
                kind: DecorationKind::Syntax,
                style_token: style_token.clone(),
                priority: 70,
                provenance: provenance.clone(),
            });
        }

        let set = DecorationSet {
            document_id: notification.document_id,
            document_version: notification.document_version,
            viewport_byte_start: notification.viewport.start,
            viewport_byte_end: notification.viewport.end,
            spans,
        };
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
