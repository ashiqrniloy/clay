use crate::protocol::{DecorationProvenance, DiagnosticSeverity, DocumentId, DocumentVersion};

/// One inert source-associated byte-range diagnostic.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub source: String,
    pub provenance: DecorationProvenance,
}

/// Replacement key for one source's versioned viewport diagnostic chunk.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticChunkKey {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub source: String,
    pub package_prefix: String,
    pub byte_start: u64,
    pub byte_end: u64,
}

/// Bounded source snapshot for one document viewport. Empty spans clear the chunk.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSet {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub viewport_byte_start: u64,
    pub viewport_byte_end: u64,
    /// Set-level identity keeps empty replacement sets source-addressable.
    pub source: String,
    pub provenance: DecorationProvenance,
    pub spans: Vec<DiagnosticSpan>,
}

impl DiagnosticSet {
    pub fn chunk_key(&self) -> DiagnosticChunkKey {
        DiagnosticChunkKey {
            document_id: self.document_id,
            document_version: self.document_version,
            source: self.source.clone(),
            package_prefix: self.provenance.package_prefix.clone(),
            byte_start: self.viewport_byte_start,
            byte_end: self.viewport_byte_end,
        }
    }

    pub fn sorted(mut self) -> Self {
        self.spans.sort_by(|left, right| {
            left.byte_start
                .cmp(&right.byte_start)
                .then_with(|| left.byte_end.cmp(&right.byte_end))
                .then_with(|| severity_rank(left.severity).cmp(&severity_rank(right.severity)))
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });
        self
    }
}

const fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Info => 2,
    }
}
