use crate::{
    packages::{permissions::PackagePermission, record::PackageRecord},
    perf::budgets::{
        DECORATION_NEAR_VIEWPORT_GUARD_BYTES, DIAGNOSTIC_CACHE_BUDGET_BYTES,
        DIAGNOSTIC_MAX_CODE_BYTES, DIAGNOSTIC_MAX_MESSAGE_BYTES,
        DIAGNOSTIC_MAX_PROVENANCE_FIELD_BYTES, DIAGNOSTIC_MAX_SOURCE_BYTES,
        DIAGNOSTIC_MAX_SPANS_PER_SET, DIAGNOSTIC_PAYLOAD_BUDGET_BYTES,
    },
    protocol::{DiagnosticChunkKey, DiagnosticSet, DocumentId, DocumentVersion},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticValidationError {
    MissingPermission {
        package_prefix: String,
    },
    StaleDocumentVersion {
        diagnostic_version: DocumentVersion,
        current_version: DocumentVersion,
    },
    InvalidViewportRange,
    TooManySpans {
        count: usize,
        limit: usize,
    },
    InvalidSpanRange {
        index: usize,
    },
    SpanOutsideViewport {
        index: usize,
    },
    EmptyField {
        index: Option<usize>,
        field: &'static str,
    },
    FieldTooLong {
        index: Option<usize>,
        field: &'static str,
        bytes: usize,
        limit: usize,
    },
    ControlCharacter {
        index: Option<usize>,
        field: &'static str,
    },
    SourceMismatch {
        index: usize,
    },
    PackageProvenanceMismatch {
        index: Option<usize>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticChunk {
    key: DiagnosticChunkKey,
    byte_size: usize,
    span_count: usize,
    last_access: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DiagnosticCacheSnapshot {
    pub(crate) budget_bytes: usize,
    pub(crate) retained_bytes: usize,
    pub(crate) chunk_count: usize,
    pub(crate) evicted_chunks: u64,
}

/// Retained server diagnostic chunks under `DIAGNOSTIC_CACHE_BUDGET_BYTES`.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticChunkCache {
    budget_bytes: usize,
    retained_bytes: usize,
    evicted_chunks: u64,
    access_counter: u64,
    chunks: Vec<DiagnosticChunk>,
}

impl Default for DiagnosticChunkCache {
    fn default() -> Self {
        Self::with_budget(DIAGNOSTIC_CACHE_BUDGET_BYTES)
    }
}

impl DiagnosticChunkCache {
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
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    #[cfg(test)]
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub(crate) fn insert_validated_set(
        &mut self,
        set: DiagnosticSet,
    ) -> Result<DiagnosticCacheSnapshot, DiagnosticValidationError> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&set)
            .map_err(|_| DiagnosticValidationError::SerializationFailed)?
            .len();
        if bytes > self.budget_bytes {
            return Err(DiagnosticValidationError::CacheBudgetExceeded {
                bytes,
                budget: self.budget_bytes,
            });
        }

        let key = set.chunk_key();
        self.remove_key(&key);
        if !set.spans.is_empty() {
            let last_access = self.next_access();
            self.retained_bytes = self.retained_bytes.saturating_add(bytes);
            self.chunks.push(DiagnosticChunk {
                key,
                byte_size: bytes,
                span_count: set.spans.len(),
                last_access,
            });
        }
        self.evict_outside_near_viewport(
            set.document_id,
            set.document_version,
            set.viewport_byte_start,
            set.viewport_byte_end,
        );
        self.evict_lru_until_budget();
        Ok(DiagnosticCacheSnapshot {
            budget_bytes: self.budget_bytes,
            retained_bytes: self.retained_bytes,
            chunk_count: self.chunks.len(),
            evicted_chunks: self.evicted_chunks,
        })
    }

    fn evict_outside_near_viewport(
        &mut self,
        document_id: DocumentId,
        document_version: DocumentVersion,
        viewport_start: u64,
        viewport_end: u64,
    ) {
        let near_start = viewport_start.saturating_sub(DECORATION_NEAR_VIEWPORT_GUARD_BYTES);
        let near_end = viewport_end.saturating_add(DECORATION_NEAR_VIEWPORT_GUARD_BYTES);
        self.retain_chunks(|chunk| {
            chunk.key.document_id != document_id
                || chunk.key.document_version != document_version
                || (chunk.key.byte_start < near_end && chunk.key.byte_end > near_start)
        });
    }

    fn next_access(&mut self) -> u64 {
        self.access_counter = self.access_counter.saturating_add(1);
        self.access_counter
    }

    fn remove_key(&mut self, key: &DiagnosticChunkKey) {
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

    fn retain_chunks(&mut self, mut keep: impl FnMut(&DiagnosticChunk) -> bool) {
        let before = self.chunks.len();
        self.chunks.retain(|chunk| keep(chunk));
        self.evicted_chunks = self
            .evicted_chunks
            .saturating_add((before - self.chunks.len()) as u64);
        self.retained_bytes = self.chunks.iter().map(|chunk| chunk.byte_size).sum();
    }
}

pub fn validate_diagnostic_publication(
    package: &PackageRecord,
    current_document_version: DocumentVersion,
    set: DiagnosticSet,
) -> Result<DiagnosticSet, DiagnosticValidationError> {
    if !package
        .manifest
        .clay
        .permissions
        .contains(&PackagePermission::RenderDecorations)
    {
        return Err(DiagnosticValidationError::MissingPermission {
            package_prefix: package.manifest.clay.api_prefix.clone(),
        });
    }
    validate_diagnostic_set(current_document_version, set, Some(package))
}

pub fn validate_diagnostic_set(
    current_document_version: DocumentVersion,
    set: DiagnosticSet,
    package: Option<&PackageRecord>,
) -> Result<DiagnosticSet, DiagnosticValidationError> {
    if set.document_version != current_document_version {
        return Err(DiagnosticValidationError::StaleDocumentVersion {
            diagnostic_version: set.document_version,
            current_version: current_document_version,
        });
    }
    if set.viewport_byte_start > set.viewport_byte_end {
        return Err(DiagnosticValidationError::InvalidViewportRange);
    }
    if set.spans.len() > DIAGNOSTIC_MAX_SPANS_PER_SET {
        return Err(DiagnosticValidationError::TooManySpans {
            count: set.spans.len(),
            limit: DIAGNOSTIC_MAX_SPANS_PER_SET,
        });
    }

    validate_field(None, "source", &set.source, DIAGNOSTIC_MAX_SOURCE_BYTES)?;
    validate_provenance(None, &set.provenance)?;
    if let Some(package) = package {
        validate_package_provenance(None, &set.provenance, package)?;
    }

    for (index, span) in set.spans.iter().enumerate() {
        if span.byte_start >= span.byte_end {
            return Err(DiagnosticValidationError::InvalidSpanRange { index });
        }
        if span.byte_start < set.viewport_byte_start || span.byte_end > set.viewport_byte_end {
            return Err(DiagnosticValidationError::SpanOutsideViewport { index });
        }
        validate_field(Some(index), "code", &span.code, DIAGNOSTIC_MAX_CODE_BYTES)?;
        validate_field(
            Some(index),
            "message",
            &span.message,
            DIAGNOSTIC_MAX_MESSAGE_BYTES,
        )?;
        validate_field(
            Some(index),
            "source",
            &span.source,
            DIAGNOSTIC_MAX_SOURCE_BYTES,
        )?;
        if span.source != set.source {
            return Err(DiagnosticValidationError::SourceMismatch { index });
        }
        validate_provenance(Some(index), &span.provenance)?;
        if span.provenance != set.provenance {
            return Err(DiagnosticValidationError::PackageProvenanceMismatch {
                index: Some(index),
            });
        }
        if let Some(package) = package {
            validate_package_provenance(Some(index), &span.provenance, package)?;
        }
    }

    let ordered = set.sorted();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&ordered)
        .map_err(|_| DiagnosticValidationError::SerializationFailed)?
        .len();
    if bytes > DIAGNOSTIC_PAYLOAD_BUDGET_BYTES {
        return Err(DiagnosticValidationError::PayloadBudgetExceeded {
            bytes,
            budget: DIAGNOSTIC_PAYLOAD_BUDGET_BYTES,
        });
    }
    Ok(ordered)
}

fn validate_provenance(
    index: Option<usize>,
    provenance: &crate::protocol::DecorationProvenance,
) -> Result<(), DiagnosticValidationError> {
    validate_field(
        index,
        "package_name",
        &provenance.package_name,
        DIAGNOSTIC_MAX_PROVENANCE_FIELD_BYTES,
    )?;
    validate_field(
        index,
        "package_version",
        &provenance.package_version,
        DIAGNOSTIC_MAX_PROVENANCE_FIELD_BYTES,
    )?;
    validate_field(
        index,
        "package_prefix",
        &provenance.package_prefix,
        DIAGNOSTIC_MAX_PROVENANCE_FIELD_BYTES,
    )
}

fn validate_package_provenance(
    index: Option<usize>,
    provenance: &crate::protocol::DecorationProvenance,
    package: &PackageRecord,
) -> Result<(), DiagnosticValidationError> {
    if provenance.package_name != package.manifest.name
        || provenance.package_version != package.manifest.version
        || provenance.package_prefix != package.manifest.clay.api_prefix
    {
        return Err(DiagnosticValidationError::PackageProvenanceMismatch { index });
    }
    Ok(())
}

fn validate_field(
    index: Option<usize>,
    field: &'static str,
    value: &str,
    limit: usize,
) -> Result<(), DiagnosticValidationError> {
    if value.trim().is_empty() {
        return Err(DiagnosticValidationError::EmptyField { index, field });
    }
    if value.len() > limit {
        return Err(DiagnosticValidationError::FieldTooLong {
            index,
            field,
            bytes: value.len(),
            limit,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(DiagnosticValidationError::ControlCharacter { index, field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DecorationProvenance, DiagnosticSeverity, DiagnosticSpan};

    fn sample_set(viewport_start: u64) -> DiagnosticSet {
        let provenance = DecorationProvenance {
            package_name: "@clay/rust".to_string(),
            package_version: "0.1.0".to_string(),
            package_prefix: "rust".to_string(),
        };
        DiagnosticSet {
            document_id: 7,
            document_version: 3,
            viewport_byte_start: viewport_start,
            viewport_byte_end: viewport_start + 8,
            source: "tree-sitter".to_string(),
            provenance: provenance.clone(),
            spans: vec![DiagnosticSpan {
                byte_start: viewport_start + 1,
                byte_end: viewport_start + 2,
                severity: DiagnosticSeverity::Error,
                code: "syntax.error".to_string(),
                message: "syntax error".to_string(),
                source: "tree-sitter".to_string(),
                provenance,
            }],
        }
    }

    #[test]
    fn diagnostic_chunk_cache_clears_empty_and_evicts_far_chunks() {
        let mut cache = DiagnosticChunkCache::with_budget(DIAGNOSTIC_CACHE_BUDGET_BYTES);
        cache
            .insert_validated_set(sample_set(0))
            .expect("near chunk inserts");
        assert_eq!(cache.chunk_count(), 1);

        let mut empty = sample_set(0);
        empty.spans.clear();
        cache
            .insert_validated_set(empty)
            .expect("empty chunk clears");
        assert_eq!(cache.chunk_count(), 0);

        cache
            .insert_validated_set(sample_set(0))
            .expect("near chunk restores");
        cache
            .insert_validated_set(sample_set(1024 * 1024))
            .expect("far chunk inserts");
        assert_eq!(cache.chunk_count(), 1);
        assert!(cache.retained_bytes() <= DIAGNOSTIC_CACHE_BUDGET_BYTES);
    }
}
