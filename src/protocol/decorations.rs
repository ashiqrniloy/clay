use crate::protocol::{DocumentFontRole, DocumentId, DocumentVersion};

/// Package provenance retained on every decoration publication.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DecorationProvenance {
    pub package_name: String,
    pub package_version: String,
    pub package_prefix: String,
}

/// Known inert decoration kinds. The client maps these to native styles only.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    Syntax,
    Semantic,
    Diagnostic,
    SearchMatch,
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
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
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
)]
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
    /// Compat mapper: classify a free-form `style_token` string from the Plan
    /// 046 baseline families into the two-axis `(token_type, modifiers)` model.
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
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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
}

impl DecorationSpan {
    /// Compat constructor: classify a free-form `style_token` into the two-axis
    /// `(token_type, modifiers)` model and preserve the original string as the
    /// open-escape `scope`. Existing package style_token families render
    /// unchanged because `decoration_color` keys off `token_type`.
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
        }
    }
}

/// Cache key for one versioned decoration chunk.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecorationChunkKey {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub package_prefix: String,
    pub byte_start: u64,
    pub byte_end: u64,
}

/// Bounded, versioned server-to-client decoration payload for one document viewport or chunk.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DecorationSet {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub viewport_byte_start: u64,
    pub viewport_byte_end: u64,
    pub spans: Vec<DecorationSpan>,
}

impl DecorationSet {
    pub fn chunk_key(&self, package_prefix: impl Into<String>) -> DecorationChunkKey {
        DecorationChunkKey {
            document_id: self.document_id,
            document_version: self.document_version,
            package_prefix: package_prefix.into(),
            byte_start: self.viewport_byte_start,
            byte_end: self.viewport_byte_end,
        }
    }

    pub fn package_prefix(&self) -> Option<&str> {
        self.spans
            .first()
            .map(|span| span.provenance.package_prefix.as_str())
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
