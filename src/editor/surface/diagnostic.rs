// Auto-extracted from surface.rs (Plan 090 task 5). Private submodule: diagnostic.
use super::decoration::ranges_intersect;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorDiagnosticChunk {
    key: DiagnosticChunkKey,
    spans: Vec<DiagnosticSpan>,
    byte_size: usize,
    last_access: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorDiagnosticState {
    pub(super) document_id: DocumentId,
    pub(super) document_version: DocumentVersion,
    pub(super) chunks: Vec<EditorDiagnosticChunk>,
    /// Viewport-bounded multi-source composition rebuilt when chunks change.
    /// Paint reads this so composition never runs on the paint hot path.
    pub(super) composed_spans: Vec<DiagnosticSpan>,
    pub(super) retained_bytes: usize,
    pub(super) access_counter: u64,
}

impl EditorDiagnosticState {
    pub(super) fn apply_set(&mut self, set: DiagnosticSet) -> bool {
        if self.document_id != set.document_id || self.document_version != set.document_version {
            *self = Self {
                document_id: set.document_id,
                document_version: set.document_version,
                ..Self::default()
            };
        }

        let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&set).map(|bytes| bytes.len()) else {
            return false;
        };
        if bytes > DIAGNOSTIC_CACHE_BUDGET_BYTES {
            return false;
        }

        let key = set.chunk_key();
        let viewport_start = set.viewport_byte_start;
        let viewport_end = set.viewport_byte_end;
        self.remove_key(&key);
        if set.spans.is_empty() {
            self.rebuild_composed();
            return true;
        }
        let last_access = self.next_access();
        self.retained_bytes = self.retained_bytes.saturating_add(bytes);
        self.chunks.push(EditorDiagnosticChunk {
            key,
            spans: set.spans,
            byte_size: bytes,
            last_access,
        });
        self.evict_outside_near_viewport(viewport_start, viewport_end);
        self.evict_lru_until_budget();
        self.rebuild_composed();
        true
    }

    pub(super) fn span_count(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.spans.len()).sum()
    }

    pub(super) fn visible_spans(
        &self,
        visible_start: u64,
        visible_end: u64,
    ) -> impl Iterator<Item = &DiagnosticSpan> {
        self.composed_spans.iter().filter(move |span| {
            ranges_intersect(span.byte_start, span.byte_end, visible_start, visible_end)
        })
    }

    fn rebuild_composed(&mut self) {
        let composed =
            compose_diagnostic_spans(self.chunks.iter().flat_map(|chunk| chunk.spans.iter()));
        self.composed_spans = composed.into_iter().cloned().collect();
    }

    fn next_access(&mut self) -> u64 {
        self.access_counter = self.access_counter.saturating_add(1);
        self.access_counter
    }

    fn remove_key(&mut self, key: &DiagnosticChunkKey) {
        self.retain_chunks(|chunk| &chunk.key != key);
    }

    fn evict_outside_near_viewport(&mut self, viewport_start: u64, viewport_end: u64) {
        let near_start = viewport_start.saturating_sub(DECORATION_NEAR_VIEWPORT_GUARD_BYTES);
        let near_end = viewport_end.saturating_add(DECORATION_NEAR_VIEWPORT_GUARD_BYTES);
        self.retain_chunks(|chunk| {
            ranges_intersect(
                chunk.key.byte_start,
                chunk.key.byte_end,
                near_start,
                near_end,
            )
        });
    }

    fn evict_lru_until_budget(&mut self) {
        while self.retained_bytes > DIAGNOSTIC_CACHE_BUDGET_BYTES {
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
        }
    }

    fn retain_chunks(&mut self, mut keep: impl FnMut(&EditorDiagnosticChunk) -> bool) {
        self.chunks.retain(|chunk| keep(chunk));
        self.retained_bytes = self.chunks.iter().map(|chunk| chunk.byte_size).sum();
    }
}
