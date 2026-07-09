use crate::packages::permissions::PackagePermission;
use crate::packages::record::PackageRecord;
use crate::perf::budgets::{
    DECORATION_NEAR_VIEWPORT_GUARD_BYTES, DECORATION_PAYLOAD_BUDGET_BYTES,
    SYNTAX_CACHE_BUDGET_BYTES,
};
use crate::protocol::{DecorationChunkKey, DecorationSet, DocumentId, DocumentVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecorationValidationError {
    MissingPermission {
        package_prefix: String,
    },
    StaleDocumentVersion {
        decoration_version: DocumentVersion,
        current_version: DocumentVersion,
    },
    InvalidViewportRange,
    InvalidSpanRange {
        index: usize,
    },
    SpanOutsideViewport {
        index: usize,
    },
    EmptyStyleToken {
        index: usize,
    },
    UnknownStyleToken {
        index: usize,
        style_token: String,
    },
    PackageProvenanceMismatch {
        index: usize,
    },
    PayloadBudgetExceeded {
        bytes: usize,
        budget: usize,
    },
    CacheBudgetExceeded {
        bytes: usize,
        budget: usize,
    },
    SerializationFailed,
}

pub fn validate_decoration_publication(
    package: &PackageRecord,
    current_document_version: DocumentVersion,
    set: DecorationSet,
) -> Result<DecorationSet, DecorationValidationError> {
    if !package
        .manifest
        .clay
        .permissions
        .contains(&PackagePermission::RenderDecorations)
    {
        return Err(DecorationValidationError::MissingPermission {
            package_prefix: package.manifest.clay.api_prefix.clone(),
        });
    }

    if set.document_version != current_document_version {
        return Err(DecorationValidationError::StaleDocumentVersion {
            decoration_version: set.document_version,
            current_version: current_document_version,
        });
    }

    validate_decoration_set(current_document_version, set, Some(package))
}

pub fn validate_decoration_set(
    current_document_version: DocumentVersion,
    set: DecorationSet,
    package: Option<&PackageRecord>,
) -> Result<DecorationSet, DecorationValidationError> {
    if set.document_version != current_document_version {
        return Err(DecorationValidationError::StaleDocumentVersion {
            decoration_version: set.document_version,
            current_version: current_document_version,
        });
    }
    if set.viewport_byte_start > set.viewport_byte_end {
        return Err(DecorationValidationError::InvalidViewportRange);
    }

    for (index, span) in set.spans.iter().enumerate() {
        if span.byte_start >= span.byte_end {
            return Err(DecorationValidationError::InvalidSpanRange { index });
        }
        if span.byte_start < set.viewport_byte_start || span.byte_end > set.viewport_byte_end {
            return Err(DecorationValidationError::SpanOutsideViewport { index });
        }
        // Plan 046 two-axis model: the free-form style-token string now lives in
        // the optional `scope` escape. Production preserves the original string
        // there via `DecorationSpan::from_style_token`, so the closed allowlist +
        // injection guards apply to `scope` exactly as they did to `style_token`.
        // A `scope`-less span is direct two-axis construction with a closed
        // `token_type` (always valid), so nothing to validate for it here.
        if let Some(scope) = span.scope.as_deref() {
            if scope.trim().is_empty() || scope.contains('{') || scope.contains('}') {
                return Err(DecorationValidationError::EmptyStyleToken { index });
            }
            if !is_known_style_token(scope) {
                return Err(DecorationValidationError::UnknownStyleToken {
                    index,
                    style_token: scope.to_string(),
                });
            }
        }
        if let Some(package) = package {
            let provenance = &span.provenance;
            if provenance.package_name != package.manifest.name
                || provenance.package_version != package.manifest.version
                || provenance.package_prefix != package.manifest.clay.api_prefix
            {
                return Err(DecorationValidationError::PackageProvenanceMismatch { index });
            }
        }
    }

    let ordered = set.sorted_viewport_first();
    let bytes = serialized_decoration_bytes(&ordered)?;
    if bytes > DECORATION_PAYLOAD_BUDGET_BYTES {
        return Err(DecorationValidationError::PayloadBudgetExceeded {
            bytes,
            budget: DECORATION_PAYLOAD_BUDGET_BYTES,
        });
    }

    Ok(ordered)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecorationChunk {
    key: DecorationChunkKey,
    byte_size: usize,
    span_count: usize,
    last_access: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecorationCacheSnapshot {
    budget_bytes: usize,
    retained_bytes: usize,
    chunk_count: usize,
    evicted_chunks: u64,
}

/// Generic retained syntax/decor chunk cache for server-side validated data.
#[derive(Debug, Clone)]
pub(crate) struct SyntaxChunkCache {
    budget_bytes: usize,
    retained_bytes: usize,
    evicted_chunks: u64,
    access_counter: u64,
    chunks: Vec<DecorationChunk>,
}

impl Default for SyntaxChunkCache {
    fn default() -> Self {
        Self::with_budget(SYNTAX_CACHE_BUDGET_BYTES)
    }
}

impl SyntaxChunkCache {
    pub(crate) fn with_budget(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            retained_bytes: 0,
            evicted_chunks: 0,
            access_counter: 0,
            chunks: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    #[cfg(test)]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    #[cfg(test)]
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    #[cfg(test)]
    pub(crate) fn evicted_chunks(&self) -> u64 {
        self.evicted_chunks
    }

    pub(crate) fn snapshot(&self) -> DecorationCacheSnapshot {
        DecorationCacheSnapshot {
            budget_bytes: self.budget_bytes,
            retained_bytes: self.retained_bytes,
            chunk_count: self.chunks.len(),
            evicted_chunks: self.evicted_chunks,
        }
    }

    pub(crate) fn insert_validated_set(
        &mut self,
        package_prefix: &str,
        set: DecorationSet,
    ) -> Result<DecorationCacheSnapshot, DecorationValidationError> {
        let bytes = serialized_decoration_bytes(&set)?;
        if bytes > self.budget_bytes {
            return Err(DecorationValidationError::CacheBudgetExceeded {
                bytes,
                budget: self.budget_bytes,
            });
        }

        let key = set.chunk_key(package_prefix.to_string());
        let chunk = DecorationChunk {
            key: key.clone(),
            byte_size: bytes,
            span_count: set.spans.len(),
            last_access: self.next_access(),
        };
        self.remove_key(&key);
        if chunk.span_count > 0 {
            self.retained_bytes += chunk.byte_size;
            self.chunks.push(chunk);
        }
        self.evict_outside_near_viewport(
            set.document_id,
            set.document_version,
            set.viewport_byte_start,
            set.viewport_byte_end,
            DECORATION_NEAR_VIEWPORT_GUARD_BYTES,
        );
        self.evict_lru_until_budget();
        Ok(self.snapshot())
    }

    pub(crate) fn evict_outside_near_viewport(
        &mut self,
        document_id: DocumentId,
        document_version: DocumentVersion,
        viewport_start: u64,
        viewport_end: u64,
        guard_bytes: u64,
    ) {
        let near_start = viewport_start.saturating_sub(guard_bytes);
        let near_end = viewport_end.saturating_add(guard_bytes);
        self.retain_chunks(|chunk| {
            chunk.key.document_id != document_id
                || chunk.key.document_version != document_version
                || ranges_intersect(
                    chunk.key.byte_start,
                    chunk.key.byte_end,
                    near_start,
                    near_end,
                )
        });
    }

    fn next_access(&mut self) -> u64 {
        self.access_counter = self.access_counter.saturating_add(1);
        self.access_counter
    }

    fn remove_key(&mut self, key: &DecorationChunkKey) {
        self.retain_chunks(|chunk| &chunk.key != key);
    }

    fn evict_lru_until_budget(&mut self) {
        while self.retained_bytes > self.budget_bytes {
            let Some((oldest_index, _)) = self
                .chunks
                .iter()
                .enumerate()
                .min_by_key(|(_, chunk)| chunk.last_access)
            else {
                break;
            };
            let removed = self.chunks.remove(oldest_index);
            self.retained_bytes = self.retained_bytes.saturating_sub(removed.byte_size);
            self.evicted_chunks = self.evicted_chunks.saturating_add(1);
        }
    }

    fn retain_chunks(&mut self, mut keep: impl FnMut(&DecorationChunk) -> bool) {
        let before = self.chunks.len();
        self.chunks.retain(|chunk| keep(chunk));
        self.evicted_chunks = self
            .evicted_chunks
            .saturating_add(before.saturating_sub(self.chunks.len()) as u64);
        self.retained_bytes = self.chunks.iter().map(|chunk| chunk.byte_size).sum();
    }
}

fn serialized_decoration_bytes(set: &DecorationSet) -> Result<usize, DecorationValidationError> {
    rkyv::to_bytes::<rkyv::rancor::Error>(set)
        .map_err(|_| DecorationValidationError::SerializationFailed)
        .map(|bytes| bytes.len())
}

fn ranges_intersect(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && left_end > right_start
}

fn is_known_style_token(style_token: &str) -> bool {
    matches!(
        style_token,
        "markup.heading.1"
            | "markup.heading.2"
            | "markup.heading.3"
            | "markup.heading.4"
            | "markup.heading.5"
            | "markup.heading.6"
            | "markup.strong"
            | "markup.emphasis"
            | "markup.inline-code"
            | "markup.code-block"
            | "markup.list-marker"
            | "keyword.control"
            | "string.quoted"
            | "comment.line"
            | "punctuation.definition"
            | "diagnostic.error"
            | "diagnostic.warning"
            | "diagnostic.info"
            | "search.match"
            | "text"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{SyntaxChunkCache, validate_decoration_publication};
    use crate::packages::record::assemble_package_record;
    use crate::perf::budgets::SYNTAX_CACHE_BUDGET_BYTES;
    use crate::protocol::{DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan};

    fn decoration_package() -> crate::packages::record::PackageRecord {
        assemble_package_record(&json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "permissions": ["render-decorations"],
                "modes": ["markdown"],
                "docs": "./docs/index.md"
            }
        }))
        .expect("decoration package fixture validates")
    }

    fn decoration_set_for_range(document_version: u64, byte_start: u64) -> DecorationSet {
        DecorationSet {
            document_id: 7,
            document_version,
            viewport_byte_start: byte_start,
            viewport_byte_end: byte_start + 64,
            spans: vec![DecorationSpan::from_style_token(
                byte_start,
                byte_start + 5,
                DecorationKind::Syntax,
                "markup.heading.1",
                10,
                DecorationProvenance {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    package_prefix: "markdown".to_string(),
                },
            )],
        }
    }

    #[test]
    fn large_file_decoration_cache_respects_30_mib_budget() {
        let package = decoration_package();
        let first =
            validate_decoration_publication(&package, 3, decoration_set_for_range(3, 0)).unwrap();
        let chunk_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&first)
            .expect("decoration chunk serializes")
            .len();
        let mut cache = SyntaxChunkCache::with_budget((chunk_bytes * 2) + 8);

        for index in 0..3 {
            let start = (index * 128) as u64;
            let set =
                validate_decoration_publication(&package, 3, decoration_set_for_range(3, start))
                    .unwrap();
            cache.insert_validated_set("markdown", set).unwrap();
        }

        assert_eq!(
            SyntaxChunkCache::default().budget_bytes(),
            SYNTAX_CACHE_BUDGET_BYTES
        );
        assert!(cache.retained_bytes() <= cache.budget_bytes());
        assert!(cache.chunk_count() <= 2);
        assert!(cache.evicted_chunks() >= 1);
    }
}
