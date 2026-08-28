use crate::protocol::{DocumentFontRole, DocumentId, DocumentVersion, TextByteRange};

/// Package provenance retained on every decoration publication.
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
pub struct DecorationProvenance {
    pub package_name: String,
    pub package_version: String,
    pub package_prefix: String,
}

/// Known inert decoration kinds. The client maps these to native styles only.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DecorationKind {
    Syntax,
    Semantic,
    Diagnostic,
    SearchMatch,
    Link,
    InlayHint,
}

impl DecorationKind {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "syntax" | "Syntax" => Self::Syntax,
            "semantic" | "Semantic" => Self::Semantic,
            "diagnostic" | "Diagnostic" => Self::Diagnostic,
            "search-match" | "searchMatch" | "SearchMatch" => Self::SearchMatch,
            "link" | "Link" => Self::Link,
            "inlayHint" | "inlay-hint" | "InlayHint" => Self::InlayHint,
            _ => return None,
        })
    }

    pub const fn allows_font_role(self) -> bool {
        matches!(self, Self::Syntax | Self::Semantic)
    }

    pub const fn paints_vocabulary_color(self) -> bool {
        matches!(self, Self::Syntax | Self::Semantic | Self::Link)
    }

    pub const fn layer_rank(self) -> u8 {
        match self {
            Self::SearchMatch => 3,
            Self::Semantic => 2,
            Self::Syntax | Self::Link => 1,
            Self::Diagnostic => 0,
            Self::InlayHint => 4,
        }
    }
}

/// Hover or activate at a byte offset. Hover is client-local (payload already
/// on the span). Activate may open an already-granted workspace path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationIntent {
    Hover,
    Activate,
}

/// Optional activatable payload on a [`DecorationKind::Link`] span.
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
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DecorationTarget {
    WorkspacePath {
        relative_path: String,
        range: Option<TextByteRange>,
    },
    DocumentRange {
        range: TextByteRange,
    },
    DisplayOnly {
        text: String,
    },
}

/// Denied above the decoration payload budget; individual strings stay short.
pub const DECORATION_TARGET_MAX_CHARS: usize = 512;

impl DecorationTarget {
    pub fn hover_text(&self) -> &str {
        match self {
            Self::WorkspacePath { relative_path, .. } => relative_path,
            Self::DocumentRange { .. } => "",
            Self::DisplayOnly { text } => text,
        }
    }

    pub fn sanitized(self) -> Option<Self> {
        match self {
            Self::WorkspacePath {
                relative_path,
                range,
            } => {
                let relative_path = sanitize_target_text(&relative_path)?;
                Some(Self::WorkspacePath {
                    relative_path,
                    range,
                })
            }
            Self::DocumentRange { range } if range.is_ordered() => {
                Some(Self::DocumentRange { range })
            }
            Self::DocumentRange { .. } => None,
            Self::DisplayOnly { text } => Some(Self::DisplayOnly {
                text: sanitize_target_text(&text).unwrap_or_default(),
            }),
        }
    }
}

fn sanitize_target_text(text: &str) -> Option<String> {
    let cleaned: String = text
        .chars()
        .filter(|ch| !ch.is_control() || *ch == '\t')
        .take(DECORATION_TARGET_MAX_CHARS)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned.chars().count() > DECORATION_TARGET_MAX_CHARS {
        None
    } else {
        Some(cleaned)
    }
}

/// Resolve a workspace-relative href against the current document path.
/// Absolute, URL, and escaped (`..`) results are `None`.
pub fn resolve_workspace_href(current_document_path: &str, href: &str) -> Option<String> {
    let href = href.trim().replace('\\', "/");
    if href.is_empty()
        || looks_like_external_or_absolute(&href)
        || href.split('/').any(|part| part == "..")
    {
        return None;
    }
    let joined = if href.starts_with("./") || href.starts_with("../") || href == "." || href == ".."
    {
        let mut parts = current_document_dir(current_document_path);
        for component in href.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                other => parts.push(other.to_string()),
            }
        }
        if parts.is_empty() {
            return None;
        }
        parts.join("/")
    } else {
        href
    };
    is_safe_relative_path(&joined).then_some(joined)
}

fn is_safe_relative_path(path: &str) -> bool {
    let normalized: String = path.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return false;
    }
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    normalized
        .split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn current_document_dir(path: &str) -> Vec<String> {
    let normalized = path.replace('\\', "/");
    let mut parts: Vec<String> = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(str::to_string)
        .collect();
    parts.pop();
    parts
}

fn looks_like_external_or_absolute(href: &str) -> bool {
    let bytes = href.as_bytes();
    href.starts_with('/')
        || href.starts_with('#')
        || href.contains(':')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

/// Plan an Activate for a decoration target. Never mints a browse grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecorationActivatePlan {
    Jump {
        byte_start: u64,
    },
    Focus {
        document_id: DocumentId,
        byte_start: Option<u64>,
    },
    Open {
        workspace_root_id: crate::protocol::WorkspaceRootId,
        relative_path: String,
        byte_start: Option<u64>,
    },
    Denied,
}

pub fn plan_decoration_activate(
    target: &DecorationTarget,
    current_document_id: DocumentId,
    current_workspace_root_id: crate::protocol::WorkspaceRootId,
    current_path: &str,
    retained_id_for_path: Option<DocumentId>,
) -> DecorationActivatePlan {
    match target {
        DecorationTarget::DocumentRange { range } => DecorationActivatePlan::Jump {
            byte_start: range.byte_start,
        },
        DecorationTarget::DisplayOnly { .. } => DecorationActivatePlan::Denied,
        DecorationTarget::WorkspacePath {
            relative_path,
            range,
        } => {
            // WorkspaceRootId 0 is the client-side "no bound root" sentinel;
            // fail closed before queueing an OpenDocument intent.
            if current_workspace_root_id == 0 {
                return DecorationActivatePlan::Denied;
            }
            let Some(resolved) = resolve_workspace_href(current_path, relative_path) else {
                return DecorationActivatePlan::Denied;
            };
            let byte_start = range.map(|range| range.byte_start);
            if paths_equal(current_path, &resolved) {
                return match byte_start {
                    Some(byte_start) => DecorationActivatePlan::Jump { byte_start },
                    None => DecorationActivatePlan::Focus {
                        document_id: current_document_id,
                        byte_start: None,
                    },
                };
            }
            if let Some(document_id) = retained_id_for_path {
                return DecorationActivatePlan::Focus {
                    document_id,
                    byte_start,
                };
            }
            DecorationActivatePlan::Open {
                workspace_root_id: current_workspace_root_id,
                relative_path: resolved,
                byte_start,
            }
        }
    }
}

fn paths_equal(left: &str, right: &str) -> bool {
    left.replace('\\', "/") == right.replace('\\', "/")
}

/// Closed vocabulary axis 1: the semantic text category of a decoration span.
///
/// The base 23 variants mirror the LSP `SemanticTokenType` closed set so that
/// LSP-backed packages map server semantic tokens with no lossy fallback. The
/// trailing 12 `Heading1`..`Paragraph` variants are the Clay prose extension for
/// the text/markdown domain that the LSP set does not cover. Third-party themes
/// resolve via the optional `scope` escape (see [`DecorationSpan::scope`]);
/// `token_type` is always one of these closed variants.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TokenType {
    // LSP SemanticTokenType base (23).
    Namespace,
    Type,
    Class,
    Enum,
    Interface,
    Struct,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Event,
    Function,
    Method,
    Macro,
    Keyword,
    Modifier,
    Comment,
    String,
    Number,
    Regexp,
    Operator,
    Decorator,
    // Clay prose extension (12): text/markdown categories the LSP set omits.
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    ListItem,
    Quote,
    CodeBlock,
    CodeSpan,
    Link,
    Paragraph,
}

/// Closed vocabulary axis 2: orthogonal modifiers on a decoration span.
///
/// Bitfield newtype over `u16`. The low 10 bits mirror the LSP
/// `SemanticTokenModifiers` closed set; bits 10..13 carry the Clay text-attribute
/// modifiers (`Bold`/`Italic`/`Underline`/`Strikethrough`) used by the prose
/// domain. Kept as a plain `u16` newtype (no bitflags dependency) so it derives
/// cheaply through rkyv and stays a fixed 2 bytes on the wire.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct Modifiers(pub u16);

impl Modifiers {
    pub const NONE: Self = Self(0);
    // LSP SemanticTokenModifiers base (10).
    pub const DECLARATION: Self = Self(1 << 0);
    pub const DEFINITION: Self = Self(1 << 1);
    pub const READONLY: Self = Self(1 << 2);
    pub const STATIC: Self = Self(1 << 3);
    pub const DEPRECATED: Self = Self(1 << 4);
    pub const ABSTRACT: Self = Self(1 << 5);
    pub const ASYNC: Self = Self(1 << 6);
    pub const MODIFICATION: Self = Self(1 << 7);
    pub const DOCUMENTATION: Self = Self(1 << 8);
    pub const DEFAULT_LIBRARY: Self = Self(1 << 9);
    // Clay text-attribute modifiers (4).
    pub const BOLD: Self = Self(1 << 10);
    pub const ITALIC: Self = Self(1 << 11);
    pub const UNDERLINE: Self = Self(1 << 12);
    pub const STRIKETHROUGH: Self = Self(1 << 13);

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }
    /// True when every bit set in `other` is also set in `self`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
    #[inline]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Parse a `Modifiers` bitfield from Rust variant names (e.g.
    /// `["Declaration", "Bold"]`). Returns `None` if any name is unknown so
    /// styleMap/theme validation rejects it up front. Empty input yields
    /// `Modifiers::NONE`.
    pub(crate) fn from_names(names: &[&str]) -> Option<Modifiers> {
        let mut mods = Modifiers::NONE;
        for name in names {
            let bit = match *name {
                "Declaration" => Modifiers::DECLARATION,
                "Definition" => Modifiers::DEFINITION,
                "Readonly" => Modifiers::READONLY,
                "Static" => Modifiers::STATIC,
                "Deprecated" => Modifiers::DEPRECATED,
                "Abstract" => Modifiers::ABSTRACT,
                "Async" => Modifiers::ASYNC,
                "Modification" => Modifiers::MODIFICATION,
                "Documentation" => Modifiers::DOCUMENTATION,
                "DefaultLibrary" => Modifiers::DEFAULT_LIBRARY,
                "Bold" => Modifiers::BOLD,
                "Italic" => Modifiers::ITALIC,
                "Underline" => Modifiers::UNDERLINE,
                "Strikethrough" => Modifiers::STRIKETHROUGH,
                _ => return None,
            };
            mods.insert(bit);
        }
        Some(mods)
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl TokenType {
    /// Frozen compatibility mapper for old packages that still emit free-form
    /// `style_token` strings. New first-party producers must emit closed
    /// [`TokenType`] + [`Modifiers`] instead.
    ///
    /// The original string is preserved as [`DecorationSpan::scope`] by the
    /// `from_style_token` constructor so existing packages render unchanged (the
    /// paint path keys off `token_type`, which this maps deterministically).
    /// Strings outside the known families fall back to `(Variable, NONE)`; the
    /// publication-boundary validation rejects unknown first-party tokens before
    /// a span reaches the wire, so the fallback is inert for trusted input.
    pub fn classify_style_token(style_token: &str) -> (TokenType, Modifiers) {
        match style_token {
            "markup.heading.1" => (TokenType::Heading1, Modifiers::NONE),
            "markup.heading.2" => (TokenType::Heading2, Modifiers::NONE),
            "markup.heading.3" => (TokenType::Heading3, Modifiers::NONE),
            "markup.heading.4" => (TokenType::Heading4, Modifiers::NONE),
            "markup.heading.5" => (TokenType::Heading5, Modifiers::NONE),
            "markup.heading.6" => (TokenType::Heading6, Modifiers::NONE),
            "markup.strong" => (TokenType::Paragraph, Modifiers::BOLD),
            "markup.emphasis" => (TokenType::Paragraph, Modifiers::ITALIC),
            "markup.inline-code" => (TokenType::CodeSpan, Modifiers::NONE),
            "markup.code-block" => (TokenType::CodeBlock, Modifiers::NONE),
            "markup.list-marker" => (TokenType::ListItem, Modifiers::NONE),
            "keyword.control" => (TokenType::Keyword, Modifiers::NONE),
            "string.quoted" => (TokenType::String, Modifiers::NONE),
            "comment.line" => (TokenType::Comment, Modifiers::NONE),
            "punctuation.definition" => (TokenType::Operator, Modifiers::NONE),
            "text" => (TokenType::Paragraph, Modifiers::NONE),
            // diagnostic.*/search.match drive color via `kind`, not token_type;
            // map to a neutral Variable so `kind`-first coloring stays authoritative.
            "diagnostic.error" | "diagnostic.warning" | "diagnostic.info" | "search.match" => {
                (TokenType::Variable, Modifiers::NONE)
            }
            _ => (TokenType::Variable, Modifiers::NONE),
        }
    }

    /// Parse a `TokenType` by its Rust variant name (e.g. `"Keyword"`,
    /// `"Heading1"`, `"Function"`). Used by the Plan 046 task-5 theme-override
    /// contract so theme packages target closed token families by name without
    /// reaching the free-form `style_token` string. Returns `None` for unknown
    /// names so callers (theme contribution validation) reject them up front.
    pub fn from_name(name: &str) -> Option<TokenType> {
        Some(match name {
            "Namespace" => TokenType::Namespace,
            "Type" => TokenType::Type,
            "Class" => TokenType::Class,
            "Enum" => TokenType::Enum,
            "Interface" => TokenType::Interface,
            "Struct" => TokenType::Struct,
            "TypeParameter" => TokenType::TypeParameter,
            "Parameter" => TokenType::Parameter,
            "Variable" => TokenType::Variable,
            "Property" => TokenType::Property,
            "EnumMember" => TokenType::EnumMember,
            "Event" => TokenType::Event,
            "Function" => TokenType::Function,
            "Method" => TokenType::Method,
            "Macro" => TokenType::Macro,
            "Keyword" => TokenType::Keyword,
            "Modifier" => TokenType::Modifier,
            "Comment" => TokenType::Comment,
            "String" => TokenType::String,
            "Number" => TokenType::Number,
            "Regexp" => TokenType::Regexp,
            "Operator" => TokenType::Operator,
            "Decorator" => TokenType::Decorator,
            "Heading1" => TokenType::Heading1,
            "Heading2" => TokenType::Heading2,
            "Heading3" => TokenType::Heading3,
            "Heading4" => TokenType::Heading4,
            "Heading5" => TokenType::Heading5,
            "Heading6" => TokenType::Heading6,
            "ListItem" => TokenType::ListItem,
            "Quote" => TokenType::Quote,
            "CodeBlock" => TokenType::CodeBlock,
            "CodeSpan" => TokenType::CodeSpan,
            "Link" => TokenType::Link,
            "Paragraph" => TokenType::Paragraph,
            _ => return None,
        })
    }

    /// Stable dense index (0..34) into a per-`TokenType` override table.
    /// Used by the Plan 046 theme registry to store text-attribute defaults
    /// (`bold`/`italic`/`underline`/`strike`) per token family without a
    /// `HashMap`. Order is the declaration order of the variants above.
    pub fn index(&self) -> usize {
        match self {
            TokenType::Namespace => 0,
            TokenType::Type => 1,
            TokenType::Class => 2,
            TokenType::Enum => 3,
            TokenType::Interface => 4,
            TokenType::Struct => 5,
            TokenType::TypeParameter => 6,
            TokenType::Parameter => 7,
            TokenType::Variable => 8,
            TokenType::Property => 9,
            TokenType::EnumMember => 10,
            TokenType::Event => 11,
            TokenType::Function => 12,
            TokenType::Method => 13,
            TokenType::Macro => 14,
            TokenType::Keyword => 15,
            TokenType::Modifier => 16,
            TokenType::Comment => 17,
            TokenType::String => 18,
            TokenType::Number => 19,
            TokenType::Regexp => 20,
            TokenType::Operator => 21,
            TokenType::Decorator => 22,
            TokenType::Heading1 => 23,
            TokenType::Heading2 => 24,
            TokenType::Heading3 => 25,
            TokenType::Heading4 => 26,
            TokenType::Heading5 => 27,
            TokenType::Heading6 => 28,
            TokenType::ListItem => 29,
            TokenType::Quote => 30,
            TokenType::CodeBlock => 31,
            TokenType::CodeSpan => 32,
            TokenType::Link => 33,
            TokenType::Paragraph => 34,
        }
    }
}

/// One inert byte-range decoration span.
///
/// Plan 046 two-axis model: axis 1 is the closed [`TokenType`] enum (drives the
/// default paint color), axis 2 is [`Modifiers`] (drives bold/italic/underline
/// styling). `scope` is the optional open escape preserving the original
/// free-form style-token string (e.g. `"keyword.control"`) for third-party theme
/// longest-prefix resolution; first-party production always sets it via
/// [`DecorationSpan::from_style_token`].
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
pub struct DecorationSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub kind: DecorationKind,
    pub token_type: TokenType,
    pub modifiers: Modifiers,
    pub scope: Option<String>,
    /// Optional syntax/semantic document-role override. `None` inherits the
    /// document default; diagnostics and search are rejected at validation.
    pub font_role: Option<DocumentFontRole>,
    pub priority: u16,
    pub provenance: DecorationProvenance,
    pub target: Option<DecorationTarget>,
    pub inlay: Option<InlayHintPayload>,
}

/// Overlay label. Inert: no command, no URL.
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
pub struct InlayHintPayload {
    pub label: String,
    pub placement: InlayPlacement,
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
pub enum InlayPlacement {
    Before,
    After,
}

/// Cap on a single inlay label. Set payload still uses DECORATION_PAYLOAD_BUDGET_BYTES.
pub const INLAY_LABEL_MAX_CHARS: usize = 64;

impl InlayHintPayload {
    pub fn sanitized(self) -> Option<Self> {
        let label: String = self
            .label
            .chars()
            .filter(|ch| !ch.is_control())
            .take(INLAY_LABEL_MAX_CHARS)
            .collect();
        let label = label.trim().to_string();
        if label.is_empty() || label.chars().count() > INLAY_LABEL_MAX_CHARS {
            None
        } else {
            Some(Self {
                label,
                placement: self.placement,
            })
        }
    }

    pub fn from_name(placement: &str, label: String) -> Option<Self> {
        let placement = match placement {
            "before" | "Before" => InlayPlacement::Before,
            "after" | "After" => InlayPlacement::After,
            _ => return None,
        };
        Self { label, placement }.sanitized()
    }
}

impl DecorationSpan {
    /// Direct two-axis constructor for syntax/semantic publishers that already
    /// own closed `TokenType` + `Modifiers` values (native grammars and future
    /// LSP bridge packages). `scope` stays `None` so validation treats the span
    /// as vocabulary-native rather than a free-form style-token escape.
    pub fn from_vocabulary(
        byte_start: u64,
        byte_end: u64,
        kind: DecorationKind,
        token_type: TokenType,
        modifiers: Modifiers,
        priority: u16,
        provenance: DecorationProvenance,
    ) -> Self {
        Self {
            byte_start,
            byte_end,
            kind,
            token_type,
            modifiers,
            scope: None,
            font_role: None,
            priority,
            provenance,
            target: None,
            inlay: None,
        }
    }

    pub fn from_inlay(
        byte_start: u64,
        byte_end: u64,
        payload: InlayHintPayload,
        priority: u16,
        provenance: DecorationProvenance,
    ) -> Self {
        Self {
            byte_start,
            byte_end,
            kind: DecorationKind::InlayHint,
            token_type: TokenType::Type,
            modifiers: Modifiers::NONE,
            scope: None,
            font_role: None,
            priority,
            provenance,
            target: None,
            inlay: Some(payload),
        }
    }

    /// Frozen compatibility constructor for old packages. Classifies a
    /// free-form `style_token` into [`TokenType`] + [`Modifiers`] and keeps the
    /// original string as the open-escape `scope`. New first-party producers
    /// must use [`DecorationSpan::from_vocabulary`].
    pub fn from_style_token(
        byte_start: u64,
        byte_end: u64,
        kind: DecorationKind,
        style_token: &str,
        priority: u16,
        provenance: DecorationProvenance,
    ) -> Self {
        let (token_type, modifiers) = TokenType::classify_style_token(style_token);
        Self {
            byte_start,
            byte_end,
            kind,
            token_type,
            modifiers,
            scope: Some(style_token.to_string()),
            font_role: None,
            priority,
            provenance,
            target: None,
            inlay: None,
        }
    }
}

/// Cache key for one versioned decoration chunk.
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
    Hash,
)]
#[serde(rename_all = "camelCase")]
pub struct DecorationChunkKey {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub package_prefix: String,
    pub kind: DecorationKind,
    pub byte_start: u64,
    pub byte_end: u64,
}

/// Bounded, versioned server-to-client decoration payload for one document viewport or chunk.
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
pub struct DecorationSet {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    /// Set-level ownership keeps empty authoritative replacement chunks keyed.
    pub package_prefix: String,
    /// Replacement/cache layer, retained even when `spans` is empty.
    pub kind: DecorationKind,
    pub viewport_byte_start: u64,
    pub viewport_byte_end: u64,
    pub spans: Vec<DecorationSpan>,
    /// Optional content-free trace linking a viewport/edit to its patch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<crate::protocol::PerformanceTraceId>,
}

impl DecorationSet {
    pub fn chunk_key(&self, package_prefix: impl Into<String>) -> DecorationChunkKey {
        DecorationChunkKey {
            document_id: self.document_id,
            document_version: self.document_version,
            package_prefix: package_prefix.into(),
            kind: self.kind,
            byte_start: self.viewport_byte_start,
            byte_end: self.viewport_byte_end,
        }
    }

    pub fn package_prefix(&self) -> Option<&str> {
        (!self.package_prefix.is_empty()).then_some(self.package_prefix.as_str())
    }

    pub fn sorted_viewport_first(mut self) -> Self {
        self.spans.sort_by(|left, right| {
            let left_visible =
                span_intersects_viewport(left, self.viewport_byte_start, self.viewport_byte_end);
            let right_visible =
                span_intersects_viewport(right, self.viewport_byte_start, self.viewport_byte_end);
            right_visible
                .cmp(&left_visible)
                .then_with(|| right.priority.cmp(&left.priority))
                .then_with(|| left.byte_start.cmp(&right.byte_start))
                .then_with(|| left.byte_end.cmp(&right.byte_end))
        });
        self
    }
}

fn span_intersects_viewport(span: &DecorationSpan, viewport_start: u64, viewport_end: u64) -> bool {
    span.byte_start < viewport_end && span.byte_end > viewport_start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoration_span_wire_shape_has_no_background_field() {
        let span = DecorationSpan::from_vocabulary(
            0,
            4,
            DecorationKind::Syntax,
            TokenType::Quote,
            Modifiers::NONE,
            10,
            DecorationProvenance {
                package_name: "test".into(),
                package_version: "1.0.0".into(),
                package_prefix: "test".into(),
            },
        );
        let encoded = format!("{span:?}");
        assert!(
            !encoded.contains("background"),
            "DecorationSpan must stay vocabulary-only: {encoded}"
        );
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&span).expect("span encodes");
        assert!(
            bytes.len() <= crate::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES,
            "single span stays inside the decoration budget"
        );
    }

    #[test]
    fn link_span_round_trip_with_workspace_target() {
        let span = DecorationSpan {
            byte_start: 1,
            byte_end: 8,
            kind: DecorationKind::Link,
            token_type: TokenType::Link,
            modifiers: Modifiers::NONE,
            scope: None,
            font_role: None,
            priority: 80,
            provenance: DecorationProvenance {
                package_name: "@clay/markdown".into(),
                package_version: "0.1.0".into(),
                package_prefix: "markdown".into(),
            },
            target: Some(DecorationTarget::WorkspacePath {
                relative_path: "docs/note.md".into(),
                range: None,
            }),
            inlay: None,
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&span).expect("encodes");
        let decoded =
            rkyv::from_bytes::<DecorationSpan, rkyv::rancor::Error>(&bytes).expect("decode");
        assert_eq!(decoded, span);
        assert_eq!(
            plan_decoration_activate(
                decoded.target.as_ref().expect("target"),
                1,
                7,
                "readme.md",
                None,
            ),
            DecorationActivatePlan::Open {
                workspace_root_id: 7,
                relative_path: "docs/note.md".into(),
                byte_start: None,
            }
        );
    }

    #[test]
    fn activate_http_or_escape_path_denied_no_grant() {
        assert_eq!(
            plan_decoration_activate(
                &DecorationTarget::DisplayOnly {
                    text: "https://example.com".into(),
                },
                1,
                7,
                "readme.md",
                None,
            ),
            DecorationActivatePlan::Denied
        );
        assert_eq!(
            plan_decoration_activate(
                &DecorationTarget::WorkspacePath {
                    relative_path: "../secret".into(),
                    range: None,
                },
                1,
                7,
                "docs/readme.md",
                None,
            ),
            DecorationActivatePlan::Denied
        );
        assert_eq!(
            plan_decoration_activate(
                &DecorationTarget::WorkspacePath {
                    relative_path: "notes.md".into(),
                    range: None,
                },
                1,
                0,
                "readme.md",
                None,
            ),
            DecorationActivatePlan::Denied
        );
        assert!(resolve_workspace_href("docs/a.md", "https://x").is_none());
        assert!(resolve_workspace_href("docs/a.md", "/etc/passwd").is_none());
    }

    #[test]
    fn activate_workspace_path_opens_or_focuses() {
        let target = DecorationTarget::WorkspacePath {
            relative_path: "note.md".into(),
            range: None,
        };
        assert_eq!(
            plan_decoration_activate(&target, 1, 7, "readme.md", Some(9)),
            DecorationActivatePlan::Focus {
                document_id: 9,
                byte_start: None,
            }
        );
        assert_eq!(
            plan_decoration_activate(&target, 1, 7, "note.md", None),
            DecorationActivatePlan::Focus {
                document_id: 1,
                byte_start: None,
            }
        );
        assert_eq!(
            plan_decoration_activate(
                &DecorationTarget::DocumentRange {
                    range: TextByteRange::new(4, 8),
                },
                1,
                7,
                "readme.md",
                None,
            ),
            DecorationActivatePlan::Jump { byte_start: 4 }
        );
    }

    #[test]
    fn inlay_span_round_trip_and_budget() {
        let span = DecorationSpan::from_inlay(
            2,
            3,
            InlayHintPayload {
                label: ": i32".into(),
                placement: InlayPlacement::After,
            },
            10,
            DecorationProvenance {
                package_name: "@clay/lsp-rust".into(),
                package_version: "0.1.0".into(),
                package_prefix: "lsp-rust".into(),
            },
        );
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&span).expect("encodes");
        let decoded =
            rkyv::from_bytes::<DecorationSpan, rkyv::rancor::Error>(&bytes).expect("decode");
        assert_eq!(decoded, span);
        assert!(bytes.len() <= crate::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES);
        assert!(span.inlay.is_some());
        assert_eq!(span.kind, DecorationKind::InlayHint);
    }
}
