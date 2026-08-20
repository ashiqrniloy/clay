use std::collections::BTreeMap;

use crate::packages::permissions::PackagePermission;
use crate::packages::record::PackageRecord;
use crate::perf::budgets::FOLDING_RANGE_PAYLOAD_BUDGET_BYTES;
use crate::protocol::{
    DocumentId, DocumentVersion, FoldingProvenance, FoldingRange, FoldingRangeSet,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FoldingValidationError {
    MissingPermission { package_prefix: String },
    StaleDocumentVersion,
    InvalidRange { index: usize },
    UnorderedRanges,
    ImproperNesting { index: usize },
    PackageProvenanceMismatch { index: usize },
    SetPackageProvenanceMismatch,
    PayloadBudgetExceeded { bytes: usize, budget: usize },
    SerializationFailed,
}

pub(crate) fn validate_folding_publication(
    package: &PackageRecord,
    current_document_version: DocumentVersion,
    set: FoldingRangeSet,
) -> Result<FoldingRangeSet, FoldingValidationError> {
    if !package
        .manifest
        .clay
        .permissions
        .contains(&PackagePermission::RenderFolding)
    {
        return Err(FoldingValidationError::MissingPermission {
            package_prefix: package.manifest.clay.api_prefix.clone(),
        });
    }
    if set.document_version != current_document_version {
        return Err(FoldingValidationError::StaleDocumentVersion);
    }
    validate_folding_set(set, Some(package))
}

fn validate_folding_set(
    set: FoldingRangeSet,
    package: Option<&PackageRecord>,
) -> Result<FoldingRangeSet, FoldingValidationError> {
    if set.package_prefix.is_empty()
        || package.is_some_and(|package| set.package_prefix != package.manifest.clay.api_prefix)
    {
        return Err(FoldingValidationError::SetPackageProvenanceMismatch);
    }

    let mut previous_start = None;
    let mut open: Vec<(u64, u64)> = Vec::new();
    for (index, range) in set.ranges.iter().enumerate() {
        if range.byte_start >= range.byte_end {
            return Err(FoldingValidationError::InvalidRange { index });
        }
        if previous_start.is_some_and(|start| range.byte_start < start) {
            return Err(FoldingValidationError::UnorderedRanges);
        }
        previous_start = Some(range.byte_start);
        while open.last().is_some_and(|(_, end)| range.byte_start >= *end) {
            open.pop();
        }
        if open.last().is_some_and(|(_, end)| range.byte_end > *end) {
            return Err(FoldingValidationError::ImproperNesting { index });
        }
        open.push((range.byte_start, range.byte_end));
        let provenance = &range.provenance;
        if provenance.package_prefix != set.package_prefix
            || package.is_some_and(|package| {
                provenance.package_name != package.manifest.name
                    || provenance.package_version != package.manifest.version
                    || provenance.package_prefix != package.manifest.clay.api_prefix
            })
        {
            return Err(FoldingValidationError::PackageProvenanceMismatch { index });
        }
    }

    let bytes = set
        .serialized_bytes()
        .ok_or(FoldingValidationError::SerializationFailed)?;
    if bytes > FOLDING_RANGE_PAYLOAD_BUDGET_BYTES {
        return Err(FoldingValidationError::PayloadBudgetExceeded {
            bytes,
            budget: FOLDING_RANGE_PAYLOAD_BUDGET_BYTES,
        });
    }
    Ok(set)
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub(crate) struct FoldingRangeRegistry {
    documents: BTreeMap<DocumentId, DocumentFolds>,
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
struct DocumentFolds {
    version: DocumentVersion,
    by_provenance: BTreeMap<String, Vec<FoldingRange>>,
}

#[allow(dead_code)]
impl FoldingRangeRegistry {
    pub(crate) fn publish_ranges(
        &mut self,
        set: FoldingRangeSet,
    ) -> Result<FoldingRangeSet, FoldingValidationError> {
        let set = validate_folding_set(set, None)?;
        self.store(set)
    }

    pub(crate) fn store(
        &mut self,
        set: FoldingRangeSet,
    ) -> Result<FoldingRangeSet, FoldingValidationError> {
        let entry = self.documents.entry(set.document_id).or_default();
        if set.document_version < entry.version {
            return Err(FoldingValidationError::StaleDocumentVersion);
        }
        if set.document_version > entry.version {
            entry.by_provenance.clear();
            entry.version = set.document_version;
        }
        entry
            .by_provenance
            .insert(set.package_prefix.clone(), set.ranges.clone());
        Ok(self.merged(set.document_id).expect("just stored"))
    }

    pub(crate) fn merged(&self, document_id: DocumentId) -> Option<FoldingRangeSet> {
        let entry = self.documents.get(&document_id)?;
        let mut ranges: Vec<FoldingRange> =
            entry.by_provenance.values().flatten().cloned().collect();
        ranges.sort_by_key(|range| (range.byte_start, std::cmp::Reverse(range.byte_end)));
        Some(FoldingRangeSet {
            document_id,
            document_version: entry.version,
            package_prefix: "merged".to_string(),
            ranges,
        })
    }
}

pub(crate) fn folds_from_syntax_tree(
    tree: &tree_sitter::Tree,
    document_id: DocumentId,
    document_version: DocumentVersion,
) -> FoldingRangeSet {
    const MAX_DEPTH: usize = 32;
    let provenance = FoldingProvenance::core();
    let mut ranges = Vec::new();
    collect_named_multiline(
        tree.root_node(),
        0,
        MAX_DEPTH,
        &provenance,
        &mut ranges,
        document_id,
        document_version,
    );
    FoldingRangeSet {
        document_id,
        document_version,
        package_prefix: provenance.package_prefix.clone(),
        ranges,
    }
}

fn collect_named_multiline(
    node: tree_sitter::Node<'_>,
    depth: usize,
    max_depth: usize,
    provenance: &FoldingProvenance,
    ranges: &mut Vec<FoldingRange>,
    document_id: DocumentId,
    document_version: DocumentVersion,
) {
    if depth > max_depth {
        return;
    }
    if node.is_named()
        && node.parent().is_some()
        && node.end_position().row > node.start_position().row
    {
        let candidate = FoldingRange {
            byte_start: node.start_byte() as u64,
            byte_end: node.end_byte() as u64,
            label: None,
            provenance: provenance.clone(),
        };
        ranges.push(candidate);
        let probe = FoldingRangeSet {
            document_id,
            document_version,
            package_prefix: provenance.package_prefix.clone(),
            ranges: ranges.clone(),
        };
        if probe
            .serialized_bytes()
            .is_some_and(|bytes| bytes > FOLDING_RANGE_PAYLOAD_BUDGET_BYTES)
        {
            ranges.pop();
            return;
        }
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            collect_named_multiline(
                child,
                depth + 1,
                max_depth,
                provenance,
                ranges,
                document_id,
                document_version,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packages::record::assemble_package_record;
    use serde_json::json;

    fn package(permissions: &[&str]) -> PackageRecord {
        assemble_package_record(&json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "permissions": permissions,
                "modes": ["markdown"],
                "docs": "./docs/index.md"
            }
        }))
        .expect("folding package fixture validates")
    }

    fn range(start: u64, end: u64) -> FoldingRange {
        FoldingRange {
            byte_start: start,
            byte_end: end,
            label: None,
            provenance: FoldingProvenance {
                package_name: "@clay/markdown".to_string(),
                package_version: "0.1.0".to_string(),
                package_prefix: "markdown".to_string(),
            },
        }
    }

    fn set(ranges: Vec<FoldingRange>) -> FoldingRangeSet {
        FoldingRangeSet {
            document_id: 1,
            document_version: 3,
            package_prefix: "markdown".to_string(),
            ranges,
        }
    }

    #[test]
    fn folding_publish_round_trip_and_budget_deny() {
        let package = package(&["render-folding", "parse-document"]);
        let accepted = validate_folding_publication(&package, 3, set(vec![range(0, 4)])).unwrap();
        assert_eq!(accepted.ranges.len(), 1);

        let mut huge = Vec::new();
        let mut start = 0u64;
        while huge.len() < 400 {
            huge.push(range(start, start + 2));
            start += 2;
        }
        let error = validate_folding_publication(&package, 3, set(huge)).unwrap_err();
        assert!(matches!(
            error,
            FoldingValidationError::PayloadBudgetExceeded { .. }
        ));
    }

    #[test]
    fn folding_stale_version_dropped() {
        let package = package(&["render-folding", "parse-document"]);
        let error = validate_folding_publication(&package, 4, set(vec![range(0, 2)])).unwrap_err();
        assert_eq!(error, FoldingValidationError::StaleDocumentVersion);
    }

    #[test]
    fn package_publish_without_render_folding_denied() {
        let package = package(&["parse-document"]);
        let error = validate_folding_publication(&package, 3, set(vec![range(0, 2)])).unwrap_err();
        assert!(matches!(
            error,
            FoldingValidationError::MissingPermission { .. }
        ));
    }

    #[test]
    fn render_decorations_does_not_grant_render_folding() {
        let package = package(&["render-decorations"]);
        let error = validate_folding_publication(&package, 3, set(vec![range(0, 2)])).unwrap_err();
        assert!(matches!(
            error,
            FoldingValidationError::MissingPermission { .. }
        ));
    }

    #[test]
    fn tree_walk_emits_only_multiline_named_nodes() {
        let language = tree_sitter_javascript::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser
            .parse("function a() {\n  return 1;\n}\nconst x = 1;\n", None)
            .unwrap();
        let set = folds_from_syntax_tree(&tree, 1, 1);
        assert!(
            set.ranges
                .iter()
                .all(|range| range.byte_end > range.byte_start)
        );
        assert!(
            set.ranges
                .iter()
                .all(|range| range.provenance.package_prefix == "core")
        );
        assert!(
            !set.ranges.is_empty(),
            "multiline function body must emit a fold"
        );
        let source = include_str!("folding.rs");
        let body = source
            .split("fn collect_named_multiline")
            .nth(1)
            .and_then(|rest| rest.split("#[cfg(test)]").next())
            .unwrap_or(source);
        for needle in ["\"rust\"", "\"markdown\"", "language_id", "match lang"] {
            assert!(
                !body.contains(needle),
                "tree walk must stay language-generic, found {needle}"
            );
        }
    }
}
