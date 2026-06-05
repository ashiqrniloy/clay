use crate::protocol::{DocumentId, DocumentVersion};

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

/// One inert byte-range decoration span.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DecorationSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub kind: DecorationKind,
    pub style_token: String,
    pub priority: u16,
    pub provenance: DecorationProvenance,
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
