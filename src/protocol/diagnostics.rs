use crate::protocol::{DecorationProvenance, DiagnosticSeverity, DocumentId, DocumentVersion};

/// Source identity for Tree-sitter recovery diagnostics.
///
/// Analyzer/LSP Error and Warning spans suppress overlapping spans from this
/// source during composition. Tree-sitter recovery is never correctness
/// authority on its own.
pub const TREE_SITTER_DIAGNOSTIC_SOURCE: &str = "tree-sitter";

/// One inert source-associated byte-range diagnostic.
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
pub struct DiagnosticChunkKey {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub source: String,
    pub package_prefix: String,
    pub byte_start: u64,
    pub byte_end: u64,
}

/// Bounded source snapshot for one document viewport. Empty spans clear the chunk.
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

fn is_analyzer_error_or_warning(span: &DiagnosticSpan) -> bool {
    span.source != TREE_SITTER_DIAGNOSTIC_SOURCE
        && matches!(
            span.severity,
            DiagnosticSeverity::Error | DiagnosticSeverity::Warning
        )
}

fn merge_half_open_intervals(mut intervals: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals
        .sort_unstable_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let mut merged = Vec::with_capacity(intervals.len());
    let mut current = intervals[0];
    for next in intervals.into_iter().skip(1) {
        if next.0 < current.1 {
            current.1 = current.1.max(next.1);
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);
    merged
}

fn interval_overlaps_merged(start: u64, end: u64, merged: &[(u64, u64)]) -> bool {
    let mut left = 0usize;
    let mut right = merged.len();
    while left < right {
        let mid = left + (right - left) / 2;
        if merged[mid].1 <= start {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    left < merged.len() && merged[left].0 < end
}

/// Compose multi-source diagnostics for one document/version viewport.
///
/// Rules:
/// - Source-keyed replacement already happened before this call.
/// - Non-tree-sitter Error/Warning spans suppress overlapping Tree-sitter
///   recovery spans only.
/// - Tree-sitter spans that do not overlap, analyzer Info spans, and all
///   non-tree-sitter spans remain additive.
/// - Ordering is deterministic over already viewport-bounded inputs; the
///   sweep is linear after one sort and one interval merge.
pub fn compose_diagnostic_spans<'a>(
    spans: impl IntoIterator<Item = &'a DiagnosticSpan>,
) -> Vec<&'a DiagnosticSpan> {
    let mut all: Vec<&DiagnosticSpan> = spans.into_iter().collect();
    all.sort_by(|left, right| {
        left.byte_start
            .cmp(&right.byte_start)
            .then_with(|| left.byte_end.cmp(&right.byte_end))
            .then_with(|| severity_rank(left.severity).cmp(&severity_rank(right.severity)))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
            .then_with(|| {
                left.provenance
                    .package_prefix
                    .cmp(&right.provenance.package_prefix)
            })
    });

    let suppressors = merge_half_open_intervals(
        all.iter()
            .filter(|span| is_analyzer_error_or_warning(span))
            .map(|span| (span.byte_start, span.byte_end))
            .collect(),
    );

    all.into_iter()
        .filter(|span| {
            if span.source != TREE_SITTER_DIAGNOSTIC_SOURCE {
                return true;
            }
            !interval_overlaps_merged(span.byte_start, span.byte_end, &suppressors)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(
        source: &str,
        start: u64,
        end: u64,
        severity: DiagnosticSeverity,
        code: &str,
    ) -> DiagnosticSpan {
        DiagnosticSpan {
            byte_start: start,
            byte_end: end,
            severity,
            code: code.to_string(),
            message: code.to_string(),
            source: source.to_string(),
            provenance: DecorationProvenance {
                package_name: "@clay/test".to_string(),
                package_version: "0.1.0".to_string(),
                package_prefix: "test".to_string(),
            },
        }
    }

    #[test]
    fn overlapping_tree_sitter_recovery_yields_to_analyzer_error_and_warning() {
        let tree = span(
            TREE_SITTER_DIAGNOSTIC_SOURCE,
            4,
            8,
            DiagnosticSeverity::Error,
            "syntax.error",
        );
        let lsp_error = span("rust-analyzer", 5, 7, DiagnosticSeverity::Error, "E0001");
        let lsp_warning = span(
            "rust-analyzer",
            20,
            24,
            DiagnosticSeverity::Warning,
            "W0001",
        );
        let tree_overlap_warning = span(
            TREE_SITTER_DIAGNOSTIC_SOURCE,
            21,
            23,
            DiagnosticSeverity::Error,
            "syntax.error",
        );
        let composed =
            compose_diagnostic_spans([&tree, &lsp_error, &lsp_warning, &tree_overlap_warning]);
        assert_eq!(
            composed
                .iter()
                .map(|span| (span.source.as_str(), span.code.as_str(), span.byte_start))
                .collect::<Vec<_>>(),
            vec![
                ("rust-analyzer", "E0001", 5),
                ("rust-analyzer", "W0001", 20)
            ]
        );
    }

    #[test]
    fn non_overlap_and_analyzer_info_remain_additive() {
        let tree = span(
            TREE_SITTER_DIAGNOSTIC_SOURCE,
            0,
            2,
            DiagnosticSeverity::Error,
            "syntax.error",
        );
        let lsp_info = span("lsp-markdown", 0, 2, DiagnosticSeverity::Info, "hint");
        let lsp_error = span(
            "lsp-markdown",
            10,
            12,
            DiagnosticSeverity::Error,
            "broken-link",
        );
        let composed = compose_diagnostic_spans([&tree, &lsp_info, &lsp_error]);
        assert_eq!(composed.len(), 3);
        assert!(
            composed
                .iter()
                .any(|span| span.source == TREE_SITTER_DIAGNOSTIC_SOURCE)
        );
        assert!(composed.iter().any(|span| span.code == "hint"));
        assert!(composed.iter().any(|span| span.code == "broken-link"));
    }
}
