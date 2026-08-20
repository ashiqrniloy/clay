// Auto-extracted from surface.rs (Plan 090 task 5). Private submodule: decoration.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorDecorationChunk {
    pub(super) key: DecorationChunkKey,
    pub(super) spans: Vec<DecorationSpan>,
    pub(super) byte_size: usize,
    pub(super) last_access: u64,
    pub(super) provisional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecorationResidualSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorDecorationState {
    pub(super) document_id: DocumentId,
    pub(super) document_version: DocumentVersion,
    pub(super) chunks: Vec<EditorDecorationChunk>,
    pub(super) retained_bytes: usize,
    pub(super) access_counter: u64,
}

impl EditorDecorationState {
    pub(super) fn apply_set(&mut self, set: DecorationSet) -> bool {
        if self.document_id != set.document_id || self.document_version != set.document_version {
            *self = Self {
                document_id: set.document_id,
                document_version: set.document_version,
                ..Self::default()
            };
        }

        let Some(package_prefix) = set.package_prefix().map(str::to_string) else {
            return true;
        };
        let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&set).map(|bytes| bytes.len()) else {
            return false;
        };
        if bytes > SYNTAX_CACHE_BUDGET_BYTES {
            return false;
        }

        let key = set.chunk_key(package_prefix);
        let viewport_start = set.viewport_byte_start;
        let viewport_end = set.viewport_byte_end;
        let mut retained = Vec::with_capacity(self.chunks.len().saturating_add(2));
        let mut residuals = Vec::new();
        for chunk in self.chunks.drain(..) {
            if chunk.key == key {
                continue;
            }
            if chunk.provisional
                && chunk.key.package_prefix == key.package_prefix
                && chunk.key.kind == key.kind
                && ranges_intersect(
                    chunk.key.byte_start,
                    chunk.key.byte_end,
                    key.byte_start,
                    key.byte_end,
                )
            {
                residuals.extend(subtract_provisional_chunk(
                    chunk,
                    key.byte_start,
                    key.byte_end,
                ));
            } else {
                retained.push(chunk);
            }
        }
        for (residual, side) in residuals {
            coalesce_local_residual(&mut retained, residual, side);
        }
        self.chunks = retained;
        self.retained_bytes = self.chunks.iter().map(|chunk| chunk.byte_size).sum();

        if !set.spans.is_empty() {
            let last_access = self.next_access();
            self.retained_bytes = self.retained_bytes.saturating_add(bytes);
            self.chunks.push(EditorDecorationChunk {
                key,
                spans: set.spans,
                byte_size: bytes,
                last_access,
                provisional: false,
            });
        }
        self.evict_outside_near_viewport(viewport_start, viewport_end);
        self.evict_lru_until_budget();
        true
    }

    pub(super) fn span_count(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.spans.len()).sum()
    }

    pub(super) fn state_version(&self) -> Option<DocumentVersion> {
        (self.span_count() > 0).then_some(self.document_version)
    }

    pub(super) fn apply_edit(&mut self, operation: &EditOperation) -> bool {
        let Some((start, end, inserted_text)) = edit_extent(operation) else {
            return false;
        };
        let Ok(inserted_len) = inserted_text.len().try_into() else {
            return false;
        };
        let mut changed = false;
        for chunk in &mut self.chunks {
            let mut chunk_changed = false;
            let original_key_range = (chunk.key.byte_start, chunk.key.byte_end);
            chunk.spans.retain_mut(|span| {
                let original = (span.byte_start, span.byte_end);
                if !interpolate_decoration_span(span, start, end, inserted_text, inserted_len) {
                    chunk_changed = true;
                    return false;
                }
                chunk_changed |= original != (span.byte_start, span.byte_end);
                true
            });
            if let Some((byte_start, byte_end)) = interpolate_range(
                chunk.key.byte_start,
                chunk.key.byte_end,
                start,
                end,
                inserted_len,
            ) {
                chunk.key.byte_start = byte_start;
                chunk.key.byte_end = byte_end;
            }
            for span in &chunk.spans {
                chunk.key.byte_start = chunk.key.byte_start.min(span.byte_start);
                chunk.key.byte_end = chunk.key.byte_end.max(span.byte_end);
            }
            chunk.provisional |=
                chunk_changed || original_key_range != (chunk.key.byte_start, chunk.key.byte_end);
            changed |= chunk_changed;
        }
        self.retain_chunks(|chunk| !chunk.spans.is_empty());
        changed
    }

    pub(super) fn confirm_version(&mut self, document_id: DocumentId, version: DocumentVersion) {
        if self.document_id == document_id {
            self.document_version = version;
            for chunk in &mut self.chunks {
                chunk.key.document_version = version;
            }
        }
    }

    pub(super) fn visible_spans(
        &self,
        visible_start: u64,
        visible_end: u64,
    ) -> impl Iterator<Item = &DecorationSpan> {
        self.chunks
            .iter()
            .filter(move |chunk| {
                ranges_intersect(
                    chunk.key.byte_start,
                    chunk.key.byte_end,
                    visible_start,
                    visible_end,
                )
            })
            .flat_map(|chunk| chunk.spans.iter())
            .filter(move |span| {
                ranges_intersect(span.byte_start, span.byte_end, visible_start, visible_end)
            })
    }

    fn next_access(&mut self) -> u64 {
        self.access_counter = self.access_counter.saturating_add(1);
        self.access_counter
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
        while self.retained_bytes > SYNTAX_CACHE_BUDGET_BYTES {
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

    fn retain_chunks(&mut self, mut keep: impl FnMut(&EditorDecorationChunk) -> bool) {
        self.chunks.retain(|chunk| keep(chunk));
        self.retained_bytes = self.chunks.iter().map(|chunk| chunk.byte_size).sum();
    }
}

pub(super) fn subtract_half_open_range(
    start: u64,
    end: u64,
    removed_start: u64,
    removed_end: u64,
) -> [Option<(u64, u64)>; 2] {
    if !ranges_intersect(start, end, removed_start, removed_end) {
        return [(start < end).then_some((start, end)), None];
    }
    [
        (start < removed_start).then_some((start, end.min(removed_start))),
        (end > removed_end).then_some((start.max(removed_end), end)),
    ]
}

fn subtract_provisional_chunk(
    chunk: EditorDecorationChunk,
    authority_start: u64,
    authority_end: u64,
) -> Vec<(EditorDecorationChunk, DecorationResidualSide)> {
    let chunk_ranges = subtract_half_open_range(
        chunk.key.byte_start,
        chunk.key.byte_end,
        authority_start,
        authority_end,
    );
    let span_fragments = chunk
        .spans
        .iter()
        .flat_map(|span| {
            subtract_half_open_range(
                span.byte_start,
                span.byte_end,
                authority_start,
                authority_end,
            )
            .into_iter()
            .flatten()
            .map(|(byte_start, byte_end)| {
                let mut fragment = span.clone();
                fragment.byte_start = byte_start;
                fragment.byte_end = byte_end;
                fragment
            })
        })
        .collect::<Vec<_>>();

    chunk_ranges
        .into_iter()
        .enumerate()
        .filter_map(|(index, range)| {
            let (byte_start, byte_end) = range?;
            let spans = span_fragments
                .iter()
                .filter(|span| span.byte_start >= byte_start && span.byte_end <= byte_end)
                .cloned()
                .collect::<Vec<_>>();
            if spans.is_empty() {
                return None;
            }
            let mut key = chunk.key.clone();
            key.byte_start = byte_start;
            key.byte_end = byte_end;
            let byte_size = decoration_chunk_byte_size(&key, &spans).unwrap_or(chunk.byte_size);
            Some((
                EditorDecorationChunk {
                    key,
                    spans,
                    byte_size,
                    last_access: chunk.last_access,
                    provisional: true,
                },
                if index == 0 {
                    DecorationResidualSide::Left
                } else {
                    DecorationResidualSide::Right
                },
            ))
        })
        .collect()
}

fn coalesce_local_residual(
    chunks: &mut Vec<EditorDecorationChunk>,
    mut residual: EditorDecorationChunk,
    side: DecorationResidualSide,
) {
    let candidate = chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| {
            chunk.provisional
                && chunk.key.document_id == residual.key.document_id
                && chunk.key.document_version == residual.key.document_version
                && chunk.key.package_prefix == residual.key.package_prefix
                && chunk.key.kind == residual.key.kind
                && match side {
                    DecorationResidualSide::Left => {
                        chunk.key.byte_start <= residual.key.byte_start
                            && chunk.key.byte_end >= residual.key.byte_start
                    }
                    DecorationResidualSide::Right => {
                        chunk.key.byte_start <= residual.key.byte_end
                            && chunk.key.byte_end >= residual.key.byte_end
                    }
                }
        })
        .min_by_key(|(_, chunk)| match side {
            DecorationResidualSide::Left => residual.key.byte_start - chunk.key.byte_start,
            DecorationResidualSide::Right => chunk.key.byte_end - residual.key.byte_end,
        })
        .map(|(index, _)| index);

    if let Some(index) = candidate {
        let neighbor = chunks.remove(index);
        residual.key.byte_start = residual.key.byte_start.min(neighbor.key.byte_start);
        residual.key.byte_end = residual.key.byte_end.max(neighbor.key.byte_end);
        residual.last_access = residual.last_access.max(neighbor.last_access);
        residual.spans.extend(neighbor.spans);
        coalesce_compatible_spans(&mut residual.spans);
        if let Some(byte_size) = decoration_chunk_byte_size(&residual.key, &residual.spans) {
            residual.byte_size = byte_size;
        } else {
            residual.byte_size = residual.byte_size.saturating_add(neighbor.byte_size);
        }
    }
    chunks.push(residual);
}

fn coalesce_compatible_spans(spans: &mut Vec<DecorationSpan>) {
    spans.sort_by_key(|span| (span.byte_start, span.byte_end));
    let mut coalesced: Vec<DecorationSpan> = Vec::with_capacity(spans.len());
    for span in spans.drain(..) {
        if let Some(previous) = coalesced.last_mut()
            && previous.byte_end >= span.byte_start
            && previous.kind == span.kind
            && previous.token_type == span.token_type
            && previous.modifiers == span.modifiers
            && previous.scope == span.scope
            && previous.font_role == span.font_role
            && previous.priority == span.priority
            && previous.provenance == span.provenance
        {
            previous.byte_end = previous.byte_end.max(span.byte_end);
        } else {
            coalesced.push(span);
        }
    }
    *spans = coalesced;
}

fn decoration_chunk_byte_size(key: &DecorationChunkKey, spans: &[DecorationSpan]) -> Option<usize> {
    rkyv::to_bytes::<rkyv::rancor::Error>(&DecorationSet {
        document_id: key.document_id,
        document_version: key.document_version,
        package_prefix: key.package_prefix.clone(),
        kind: key.kind,
        viewport_byte_start: key.byte_start,
        viewport_byte_end: key.byte_end,
        spans: spans.to_vec(),
    })
    .ok()
    .map(|bytes| bytes.len())
}

pub(super) fn ranges_intersect(
    left_start: u64,
    left_end: u64,
    right_start: u64,
    right_end: u64,
) -> bool {
    left_start < right_end && left_end > right_start
}

/// Tuple shape returned by [`EditorSurface::visible_text_style_runs_for_test`]:
/// `(range, font_role, [bold, italic, underline, strike], color, background, scale)`.
pub type VisibleTextStyleRunForTest = (
    Range<usize>,
    crate::protocol::FontRole,
    [bool; 4],
    Option<Color>,
    Option<Color>,
    f32,
);

pub(super) fn normalize_visible_text_style_runs(
    decorations: &EditorDecorationState,
    document: &EditorDocumentState,
    document_end: usize,
    snapshot: &VisibleSnapshot,
    default_font_role: FontRole,
    theme: StyleRegistry,
) -> Vec<VisibleTextStyleRun> {
    if snapshot.text.is_empty()
        || decorations.document_id != document.document_id
        || decorations.document_version != document.document_version
    {
        return Vec::new();
    }

    let visible_start = snapshot.start_byte_offset;
    let visible_end = visible_start + snapshot.text.len();
    let mut spans = Vec::new();
    for span in decorations.visible_spans(visible_start as u64, visible_end as u64) {
        if span.inlay.is_some() {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            usize::try_from(span.byte_start),
            usize::try_from(span.byte_end),
        ) else {
            continue;
        };
        if start >= end || end > document_end {
            continue;
        }
        let start = start.max(visible_start) - visible_start;
        let end = end.min(visible_end) - visible_start;
        if start >= end
            || !snapshot.text.is_char_boundary(start)
            || !snapshot.text.is_char_boundary(end)
        {
            continue;
        }
        let style = theme.style_for(span.kind, span.token_type, span.modifiers);
        spans.push(VisibleDecorationStyle {
            range: start..end,
            span,
            attributes: style.attributes(),
            color: span.kind.paints_vocabulary_color().then_some(style.color),
            background: style.background,
            scale: style.scale,
        });
    }

    let mut boundaries = Vec::with_capacity(spans.len().saturating_mul(2).saturating_add(2));
    boundaries.extend([0, snapshot.text.len()]);
    for style in &spans {
        boundaries.extend([style.range.start, style.range.end]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut runs: Vec<VisibleTextStyleRun> = Vec::new();
    for boundary in boundaries.windows(2) {
        let range = boundary[0]..boundary[1];
        if range.start == range.end {
            continue;
        }
        let active = spans
            .iter()
            .filter(|style| style.range.start <= range.start && style.range.end >= range.end);
        let mut attributes = TextAttributes::default();
        let mut font_span = None;
        let mut color_style: Option<&VisibleDecorationStyle<'_>> = None;
        let mut background_style: Option<&VisibleDecorationStyle<'_>> = None;
        let mut scale_style: Option<&VisibleDecorationStyle<'_>> = None;
        for style in active {
            attributes.bold |= style.attributes.bold;
            attributes.italic |= style.attributes.italic;
            attributes.underline |= style.attributes.underline;
            attributes.strike |= style.attributes.strike;
            if span_font_role(style.span).is_some()
                && font_span.is_none_or(|current| font_role_precedes(style.span, current))
            {
                font_span = Some(style.span);
            }
            if style.color.is_some()
                && color_style.is_none_or(|current| font_role_precedes(style.span, current.span))
            {
                color_style = Some(style);
            }
            if style.background.is_some()
                && background_style
                    .is_none_or(|current| font_role_precedes(style.span, current.span))
            {
                background_style = Some(style);
            }
            if (style.scale - 1.0).abs() > f32::EPSILON
                && scale_style.is_none_or(|current| font_role_precedes(style.span, current.span))
            {
                scale_style = Some(style);
            }
        }
        let font_role = font_span
            .and_then(span_font_role)
            .unwrap_or(default_font_role);
        let color = color_style.and_then(|style| style.color);
        let background = background_style.and_then(|style| style.background);
        let scale = scale_style.map(|style| style.scale).unwrap_or(1.0);
        if let Some(previous) = runs.last_mut()
            && previous.range.end == range.start
            && previous.font_role == font_role
            && previous.attributes == attributes
            && previous.color == color
            && previous.background == background
            && previous.scale == scale
        {
            previous.range.end = range.end;
        } else {
            runs.push(VisibleTextStyleRun {
                range,
                font_role,
                attributes,
                color,
                background,
                scale,
            });
        }
    }
    runs
}

struct VisibleDecorationStyle<'a> {
    range: Range<usize>,
    span: &'a DecorationSpan,
    attributes: TextAttributes,
    color: Option<Color>,
    background: Option<Color>,
    scale: f32,
}

fn span_font_role(span: &DecorationSpan) -> Option<FontRole> {
    span.kind
        .allows_font_role()
        .then_some(span.font_role)
        .flatten()
        .and_then(DocumentFontRole::font_role)
}

/// Higher priority wins; a semantic layer wins an equal-priority syntax layer;
/// then package provenance and role make equal input deterministic without
/// trusting source arrival order.
fn font_role_precedes(candidate: &DecorationSpan, current: &DecorationSpan) -> bool {
    candidate
        .priority
        .cmp(&current.priority)
        .then_with(|| {
            decoration_layer_rank(candidate.kind).cmp(&decoration_layer_rank(current.kind))
        })
        .then_with(|| {
            current
                .provenance
                .package_prefix
                .cmp(&candidate.provenance.package_prefix)
        })
        .then_with(|| {
            current
                .provenance
                .package_name
                .cmp(&candidate.provenance.package_name)
        })
        .then_with(|| {
            current
                .provenance
                .package_version
                .cmp(&candidate.provenance.package_version)
        })
        .then_with(|| {
            font_role_rank(span_font_role(candidate)).cmp(&font_role_rank(span_font_role(current)))
        })
        .is_gt()
}

fn edit_extent(operation: &EditOperation) -> Option<(u64, u64, &str)> {
    let extent = match operation {
        EditOperation::Insert { byte_offset, text } => (*byte_offset, *byte_offset, text.as_str()),
        EditOperation::Delete { start, end } => (*start, *end, ""),
        EditOperation::Replace { start, end, text } => (*start, *end, text.as_str()),
    };
    (extent.0 <= extent.1).then_some(extent)
}

fn interpolate_decoration_span(
    span: &mut DecorationSpan,
    edit_start: u64,
    edit_end: u64,
    inserted_text: &str,
    inserted_len: u64,
) -> bool {
    let broad_syntax = span.kind == DecorationKind::Syntax && is_broad_token(span.token_type);
    let same_word_suffix = span.kind == DecorationKind::Syntax
        && !broad_syntax
        && !inserted_text.is_empty()
        && inserted_text.chars().all(is_completion_word_character);
    if edit_start == edit_end {
        if edit_start < span.byte_start {
            let Some((start, end)) = shift_range(span.byte_start, span.byte_end, inserted_len, 0)
            else {
                return false;
            };
            span.byte_start = start;
            span.byte_end = end;
        } else if edit_start == span.byte_start {
            // Text inserted exactly at the span's first byte lands *before* the
            // span, so the span shifts right unchanged. Extending here (the old
            // broad-token behavior) absorbed the typed char into the span and
            // bled its color onto preceding text — e.g. typing before `one`
            // painted the new char as code until the next re-parse.
            let Some((start, end)) = shift_range(span.byte_start, span.byte_end, inserted_len, 0)
            else {
                return false;
            };
            span.byte_start = start;
            span.byte_end = end;
        } else if edit_start < span.byte_end
            || (edit_start == span.byte_end && (broad_syntax || same_word_suffix))
        {
            if span.kind != DecorationKind::Syntax {
                return false;
            }
            let Some(end) = span.byte_end.checked_add(inserted_len) else {
                return false;
            };
            span.byte_end = end;
        }
        return true;
    }

    if span.byte_end <= edit_start {
        return true;
    }
    if span.byte_start >= edit_end {
        let Some((start, end)) = shift_range(
            span.byte_start,
            span.byte_end,
            inserted_len,
            edit_end - edit_start,
        ) else {
            return false;
        };
        span.byte_start = start;
        span.byte_end = end;
        return true;
    }
    if span.kind != DecorationKind::Syntax {
        return false;
    }

    let survives_left = span.byte_start < edit_start;
    let survives_right = span.byte_end > edit_end;
    match (survives_left, survives_right) {
        (true, true) => {
            let Some(end) = shift_offset(
                span.byte_end,
                inserted_len,
                edit_end.saturating_sub(edit_start),
            ) else {
                return false;
            };
            span.byte_end = end;
        }
        (true, false) => {
            span.byte_end = if broad_syntax {
                let Some(end) = edit_start.checked_add(inserted_len) else {
                    return false;
                };
                end
            } else {
                edit_start
            };
        }
        (false, true) => {
            let Some(end) = shift_offset(
                span.byte_end,
                inserted_len,
                edit_end.saturating_sub(edit_start),
            ) else {
                return false;
            };
            span.byte_start = if broad_syntax {
                edit_start
            } else {
                let Some(start) = edit_start.checked_add(inserted_len) else {
                    return false;
                };
                start
            };
            span.byte_end = end;
        }
        (false, false) => return false,
    }
    span.byte_start < span.byte_end
}

fn interpolate_range(
    start: u64,
    end: u64,
    edit_start: u64,
    edit_end: u64,
    inserted_len: u64,
) -> Option<(u64, u64)> {
    if end <= edit_start {
        return Some((start, end));
    }
    if start >= edit_end {
        return shift_range(start, end, inserted_len, edit_end - edit_start);
    }
    let start = if start < edit_start {
        start
    } else {
        edit_start
    };
    let end = if end > edit_end {
        shift_offset(end, inserted_len, edit_end - edit_start)?
    } else {
        edit_start.checked_add(inserted_len)?
    };
    (start < end).then_some((start, end))
}

fn shift_range(start: u64, end: u64, added: u64, removed: u64) -> Option<(u64, u64)> {
    Some((
        shift_offset(start, added, removed)?,
        shift_offset(end, added, removed)?,
    ))
}

fn shift_offset(offset: u64, added: u64, removed: u64) -> Option<u64> {
    offset.checked_sub(removed)?.checked_add(added)
}
