use crate::protocol::{DocumentId, DocumentVersion};

/// Package or core provenance retained on every folding publication.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FoldingProvenance {
    pub package_name: String,
    pub package_version: String,
    pub package_prefix: String,
}

impl FoldingProvenance {
    pub fn core() -> Self {
        Self {
            package_name: "core".to_string(),
            package_version: "0".to_string(),
            package_prefix: "core".to_string(),
        }
    }
}

/// One collapsible byte range. Collapsed state is client-local.
/// Sets are denied above `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FoldingRange {
    pub byte_start: u64,
    pub byte_end: u64,
    pub label: Option<String>,
    pub provenance: FoldingProvenance,
}

/// Validated folding ranges for one document version and one provenance.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FoldingRangeSet {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub package_prefix: String,
    pub ranges: Vec<FoldingRange>,
}

impl FoldingRangeSet {
    pub fn serialized_bytes(&self) -> Option<usize> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .ok()
            .map(|bytes| bytes.len())
    }
}
