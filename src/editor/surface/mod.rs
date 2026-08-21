use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ops::Range;

use masonry::accesskit::{NodeId, TextPosition, TextSelection};
use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, Point, Rect, Stroke};
use masonry::parley::style::StyleProperty;
use masonry::peniko::{Color, Fill};

use crate::client::behavior::{
    ChordRouteOutcome, ClientBehaviorState, ClientLocalEdit, ClientUiCommandRoute,
    CompletionTriggerRoute, LanguageIntelligenceTriggerRoute, RoutedBehavior, ServerIntentRoute,
};
use crate::perf::{
    budgets::{
        DECORATION_NEAR_VIEWPORT_GUARD_BYTES, DIAGNOSTIC_CACHE_BUDGET_BYTES,
        KEY_CHORD_PENDING_TIMEOUT_MS, SYNTAX_CACHE_BUDGET_BYTES,
    },
    metrics::PerfRecorder,
};
use crate::protocol::{
    BehaviorManifest, BehaviorVersion, BlinkStyle, CaretShape, CaretStyle,
    CompletionItemTextFormat, CompletionReplacementRange, CompletionTrigger, DecorationChunkKey,
    DecorationKind, DecorationSet, DecorationSpan, DiagnosticChunkKey, DiagnosticSet,
    DiagnosticSpan, DocumentAccess, DocumentFontRole, DocumentId, DocumentVersion, EditOperation,
    ElectricCharacterRule, ElectricEffect, EnterRule, FoldingRange, FoldingRangeSet, FontRole,
    KeyCode, KeyStroke, MovementRules, PairRule, TokenType, WordSeparatorPolicy, WrapPolicy,
    compose_diagnostic_spans,
};
use crate::shell::CompletionMenuAcceptAction;

use super::buffer::{EditResult, EditorBuffer, VisibleSnapshot};
use super::composition::CompositionState;
use super::cursor::CursorState;
use super::history::{EditHistory, HistoryEntry, HistorySelection, invert_edit_operation};
use super::is_printable_text;
use super::layout::{
    LayoutCacheKey, LayoutState, TextChromeLayers, TextFrame, VisibleTextStyleRun,
};
use super::selection::{Selection, SelectionState};
use super::snippet::{SnippetPlaceholder, parse_snippet};
use super::theme::{StyleRegistry, TextAttributes};
use super::typography::TypographyRegistry;
use super::viewport::{Viewport, visible_line_count_from_height};

// All color now comes from the single source of color, `StyleRegistry`
// (super::theme). The only color literals permitted in the editor/shell paint
// path live in super::theme.rs (the theme-definition module); a source-guard
// test in tests/editor_performance_invariants.rs forbids Color::from_rgb*
// literals anywhere else here.
mod caret;
mod chrome;
mod command;
mod decoration;
mod diagnostic;

#[cfg(test)]
use self::caret::BlinkPhase;
use self::caret::CaretBlink;
pub(crate) use self::command::EditorKeyOutcome;
pub use self::command::{
    CursorSelectDirection, EditorCommand, EditorCommandOutcome, EditorEditEvent,
};
pub(crate) use self::command::{
    EditorCompletionRequestEvent, EditorLanguageIntelligenceRequestEvent,
    EditorSelectionQueryRequestEvent, PendingChord,
};
use self::decoration::normalize_visible_text_style_runs;
#[cfg(test)]
use self::decoration::subtract_half_open_range;
pub use self::decoration::{EditorDecorationState, VisibleTextStyleRunForTest};
pub use self::diagnostic::EditorDiagnosticState;

const CARET_WIDTH: f64 = 1.5;
const SCROLLBAR_WIDTH: f64 = 8.0;
const SCROLLBAR_MARGIN: f64 = 4.0;
const SCROLLBAR_MIN_THUMB: f64 = 24.0;
/// Horizontal inset without a gutter (`spacing.xl`).
pub(super) const TEXT_INSET: f64 = 32.0;
/// Extra left inset when the line-number gutter is on (`spacing.xxl`).
pub(super) const TEXT_INSET_GUTTER: f64 = 48.0;
/// Vertical inset (`spacing.lg` minus a 4px optical tighten).
pub(super) const TEXT_INSET_Y: f64 = 20.0;
const PLACEHOLDER_TEXT: &str = "Start typing in the Clay native text canvas…";

/// Bound for the cursor-undo stack (Plan 071 task 9). One snapshot per
/// caret-moving command; oldest drops first.
const CURSOR_UNDO_MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct EditorDocumentState {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub access: DocumentAccess,
    pub behavior_version: BehaviorVersion,
    pub behavior_manifest: Option<BehaviorManifest>,
}

impl Default for EditorDocumentState {
    fn default() -> Self {
        Self {
            document_id: 0,
            document_version: 0,
            access: DocumentAccess::Editable { lease_id: 1 },
            behavior_version: 0,
            behavior_manifest: None,
        }
    }
}

const fn decoration_layer_rank(kind: DecorationKind) -> u8 {
    kind.layer_rank()
}

const fn font_role_rank(role: Option<FontRole>) -> u8 {
    match role {
        Some(FontRole::Monospace) => 2,
        Some(FontRole::Proportional) => 1,
        Some(FontRole::Ui) | None => 0,
    }
}

fn is_completion_word_character(character: char) -> bool {
    // Unify with movement/selection: one classifier, the `Code` policy with
    // underscore-as-word. Completion deliberately stays on the code default so
    // token detection is stable across prose/custom movement policies.
    WordSeparatorPolicy::Code.is_word_char(character, true)
}

const fn is_broad_token(token_type: TokenType) -> bool {
    matches!(
        token_type,
        TokenType::Comment
            | TokenType::String
            | TokenType::Regexp
            | TokenType::Heading1
            | TokenType::Heading2
            | TokenType::Heading3
            | TokenType::Heading4
            | TokenType::Heading5
            | TokenType::Heading6
            | TokenType::Quote
            | TokenType::CodeBlock
            | TokenType::CodeSpan
            | TokenType::Link
            | TokenType::Paragraph
    )
}

fn shift_snippet_offset(offset: usize, removed_len: usize, inserted_len: usize) -> Option<usize> {
    if inserted_len >= removed_len {
        offset.checked_add(inserted_len - removed_len)
    } else {
        offset.checked_sub(removed_len - inserted_len)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnippetSession {
    placeholders: Vec<SnippetPlaceholder>,
    active_index: usize,
}

#[derive(Debug, Default, Clone)]
struct EditorFoldState {
    // FOLDING_RANGE_PAYLOAD_BUDGET_BYTES
    by_provenance: BTreeMap<String, Vec<FoldingRange>>,
    collapsed: BTreeSet<u64>,
    revision: u64,
}

#[derive(Debug, Default)]
pub struct EditorSurface {
    buffer: EditorBuffer,
    document: EditorDocumentState,
    selections: SelectionState,
    history: EditHistory,
    composition: CompositionState,
    snippet_session: Option<SnippetSession>,
    viewport: Viewport,
    layout: LayoutState,
    decorations: EditorDecorationState,
    diagnostics: EditorDiagnosticState,
    visual_scroll_y: f64,
    visual_scroll_x: f64,
    last_visual_max_scroll_y: f64,
    last_visual_max_scroll_x: f64,
    /// User wrap override. Wins over the mode manifest; packages cannot clear it.
    layout_override: Option<crate::protocol::WrapPolicy>,
    follow_visual_end: bool,
    /// Phase 24.5: an in-progress multi-stroke chord, owned by this surface
    /// so it survives across keystrokes. Holds only validated `KeyStroke`
    /// values from the incoming event stream; cleared on match, mismatch,
    /// timeout, and key paths that bypass the chord matcher.
    pending_chord: Option<PendingChord>,
    /// One-shot flag: keep the caret sub-line visible on the next paint after a
    /// caret move. Explicit scrolling clears it so the view can move away from
    /// the caret instead of snapping back (the caret-keep-visible logic must
    /// not fight user scrolling).
    pin_caret_visible: bool,
    /// User overlay toggle. `None` uses `EditorChrome.inlay_hints`.
    inlay_hints_override: Option<bool>,

    /// Single source of color for the editor + shell paint path (Plan 046 task
    /// 4). Defaults to the Clay theme; task 5 swaps in the active theme at
    /// load/reload. Immutable during paint.
    theme: StyleRegistry,
    /// Phase 20.2: cached resolved UI design-token registry for shell chrome
    /// (scrollbar, status bar). Installed atomically with `theme` when the
    /// active theme changes. Editor text/caret/selection/diagnostics stay on
    /// `theme` (StyleRegistry); shell chrome routes through primitives.
    ui_theme: crate::shell::theme::ResolvedUiTheme,
    /// Active theme package specifier last installed from an `ActiveTheme`
    /// snapshot (`@clay/default` until the first install). Used only for
    /// status/accessibility theme-label observability — never for paint.
    theme_specifier: String,
    /// Client-owned cached profiles, replaced outside paint by bootstrap/live
    /// typography updates and read by the cached Parley layout builder.
    typography: TypographyRegistry,
    /// Bumped only when validated attributes, document role, or decoration data
    /// that can change shaping changes. Color-only rectangle paint stays out of
    /// the layout cache key.
    layout_style_revision: u64,
    folds: EditorFoldState,
    /// Phase 20.4 task 5: client-local pointer state for the editor scrollbar's
    /// InteractionState (Hover when the pointer is over the track, Active when
    /// pressed over the thumb). Stored in the same absolute widget coordinate
    /// space as the `rect` passed to `paint_in_rect`. Client-side only; carries
    /// no authority.
    pointer_pos: Option<masonry::kurbo::Point>,
    pointer_pressed: bool,
    /// Runtime caret appearance override set by `clientSetCursorStyle`. Takes
    /// precedence over the per-mode manifest `caret_style` and the editor
    /// `StyleRegistry` default. Client-local; carries no authority.
    caret_style_override: Option<CaretStyle>,
    /// Caret blink state machine, driven by widget animation frames.
    caret_blink: CaretBlink,
    /// Cursor-undo stack (Plan 071 task 9, VSCode cursorUndo / Ctrl+U).
    /// Snapshots of the selection set taken before each caret-moving or
    /// selection-reshaping command; `UndoCursorMove` pops the latest. Bounded
    /// by [`CURSOR_UNDO_MAX_DEPTH`].
    cursor_undo_stack: VecDeque<SelectionState>,
    perf: PerfRecorder,
}

impl EditorSurface {
    pub fn load_snapshot(
        &mut self,
        document_id: DocumentId,
        version: DocumentVersion,
        text: String,
        access: DocumentAccess,
    ) {
        self.buffer.replace_text(text);
        self.document.document_id = document_id;
        self.document.document_version = version;
        self.document.access = access;
        self.selections = SelectionState::default();
        self.cursor_undo_stack.clear();
        self.history.clear();
        self.composition.clear();
        self.snippet_session = None;
        self.viewport = Viewport::default();
        self.layout = LayoutState::default();
        self.decorations = EditorDecorationState::default();
        self.diagnostics = EditorDiagnosticState::default();
        self.folds = EditorFoldState::default();
        self.layout_style_revision = self.layout_style_revision.saturating_add(1);
        self.visual_scroll_y = 0.0;
        self.visual_scroll_x = 0.0;
        self.last_visual_max_scroll_y = 0.0;
        self.last_visual_max_scroll_x = 0.0;
        self.follow_visual_end = false;
        self.pin_caret_visible = false;
    }

    pub fn load_resync_snapshot(
        &mut self,
        document_id: DocumentId,
        version: DocumentVersion,
        text: String,
        access: DocumentAccess,
    ) {
        let caret = (self.document.document_id == document_id).then_some(self.caret());
        self.load_snapshot(document_id, version, text, access);
        if let Some(caret) = caret {
            self.navigate_to_byte_offset(caret as u64);
        }
    }

    pub fn install_behavior_manifest(&mut self, manifest: BehaviorManifest) {
        if ClientBehaviorState::new(manifest.clone()).is_ok() {
            let previous_role = self.document_font_role();
            let previous_wrap = self.resolved_wrap();
            self.document.behavior_version = manifest.behavior_version;
            self.document.behavior_manifest = Some(manifest);
            if self.document_font_role() != previous_role || self.resolved_wrap() != previous_wrap {
                self.bump_layout_style_revision();
            }
        }
    }

    /// Phase 22.2: advance the connection-wide behavior version without
    /// swapping this document's mode content. Keeps outbound edit/completion
    /// stamps current when another document's mode activation bumps the global
    /// version, without cross-pane manifest content bleed.
    pub fn update_behavior_version(&mut self, behavior_version: BehaviorVersion) {
        if let Some(manifest) = &mut self.document.behavior_manifest {
            manifest.behavior_version = behavior_version;
        }
        self.document.behavior_version = behavior_version;
    }

    pub fn apply_decoration_set(&mut self, set: DecorationSet) -> bool {
        if set.document_id != self.document.document_id
            || set.document_version != self.document.document_version
        {
            return false;
        }
        let applied = self.decorations.apply_set(set);
        if applied {
            self.bump_layout_style_revision();
        }
        applied
    }

    pub fn decoration_target_at(
        &self,
        offset: usize,
    ) -> Option<&crate::protocol::DecorationTarget> {
        let offset = offset as u64;
        self.decorations
            .visible_spans(offset, offset.saturating_add(1))
            .filter(|span| {
                span.target.is_some() && span.byte_start <= offset && offset < span.byte_end
            })
            .max_by_key(|span| span.priority)
            .and_then(|span| span.target.as_ref())
    }

    pub fn apply_diagnostic_set(&mut self, set: DiagnosticSet) -> bool {
        if set.document_id != self.document.document_id
            || set.document_version != self.document.document_version
        {
            return false;
        }
        self.diagnostics.apply_set(set)
    }

    pub fn apply_folding_set(&mut self, set: FoldingRangeSet) -> bool {
        if set.document_id != self.document.document_id
            || set.document_version != self.document.document_version
        {
            return false;
        }
        self.folds
            .by_provenance
            .insert(set.package_prefix, set.ranges);
        let valid: BTreeSet<u64> = self
            .folds
            .by_provenance
            .values()
            .flatten()
            .map(|range| range.byte_start)
            .collect();
        self.folds.collapsed.retain(|start| valid.contains(start));
        self.folds.revision = self.folds.revision.saturating_add(1);
        true
    }

    pub fn toggle_fold(&mut self) -> bool {
        let caret = self.caret() as u64;
        let Some(start) = self.fold_start_for_offset(caret) else {
            return false;
        };
        if !self.folds.collapsed.insert(start) {
            self.folds.collapsed.remove(&start);
        }
        self.folds.revision = self.folds.revision.saturating_add(1);
        true
    }

    pub fn toggle_inlay_hints(&mut self) -> bool {
        let visible = self.inlay_hints_visible();
        self.inlay_hints_override = Some(!visible);
        true
    }

    pub(crate) fn inlay_hints_visible(&self) -> bool {
        self.inlay_hints_override
            .unwrap_or_else(|| self.resolved_chrome().inlay_hints)
    }

    fn fold_ranges(&self) -> impl Iterator<Item = &FoldingRange> {
        self.folds.by_provenance.values().flatten()
    }

    fn fold_start_for_offset(&self, offset: u64) -> Option<u64> {
        self.fold_ranges()
            .filter(|range| offset >= range.byte_start && offset < range.byte_end)
            .min_by_key(|range| range.byte_end - range.byte_start)
            .map(|range| range.byte_start)
    }

    fn line_is_fold_start(&self, line: usize) -> bool {
        let start = self.buffer.byte_of_line(line) as u64;
        self.fold_ranges().any(|range| range.byte_start == start)
    }

    fn line_fold_is_collapsed(&self, line: usize) -> bool {
        let start = self.buffer.byte_of_line(line) as u64;
        self.folds.collapsed.contains(&start)
    }

    fn line_is_hidden(&self, line: usize) -> bool {
        if self.folds.collapsed.is_empty() {
            return false;
        }
        let start = self.buffer.byte_of_line(line) as u64;
        self.fold_ranges().any(|range| {
            self.folds.collapsed.contains(&range.byte_start)
                && start > range.byte_start
                && start < range.byte_end
        })
    }

    fn hidden_bytes_between(&self, start: usize, end: usize) -> usize {
        if self.folds.collapsed.is_empty() || end <= start {
            return 0;
        }
        let start_line = self.buffer.line_of_byte(start);
        let end_line = self.buffer.line_of_byte(end.saturating_sub(1));
        let mut hidden = 0;
        for line in start_line..=end_line {
            if !self.line_is_hidden(line) {
                continue;
            }
            let (line_start, line_end) = self.buffer.line_range(self.buffer.byte_of_line(line));
            let from = start.max(line_start);
            let to = end.min(line_end.saturating_add(1));
            if to > from {
                hidden += to - from;
            }
        }
        hidden
    }

    /// Clear decoration caches for the open document during runtime-generation install.
    pub(crate) fn clear_decorations(&mut self) -> bool {
        if self.decorations.span_count() == 0 {
            self.decorations = EditorDecorationState::default();
            return false;
        }
        self.decorations = EditorDecorationState::default();
        self.bump_layout_style_revision();
        true
    }

    /// Clear diagnostic caches for the open document during runtime-generation install.
    pub(crate) fn clear_diagnostics(&mut self) -> bool {
        if self.diagnostics.span_count() == 0 {
            self.diagnostics = EditorDiagnosticState::default();
            return false;
        }
        self.diagnostics = EditorDiagnosticState::default();
        true
    }

    /// Install typography from a runtime snapshot while preserving caret,
    /// selection, and viewport scroll when profiles actually change.
    pub(crate) fn install_runtime_typography(
        &mut self,
        typography: crate::protocol::ActiveTypography,
    ) -> bool {
        let Ok(next) =
            crate::editor::typography::TypographyRegistry::from_active_typography(typography)
        else {
            return false;
        };
        if self.typography == next {
            return false;
        }
        let caret = self.caret();
        let selection_anchor = self
            .has_selection()
            .then_some(self.selections.primary_anchor());
        let visual_scroll_y = self.visual_scroll_y;
        let last_visual_max_scroll_y = self.last_visual_max_scroll_y;
        let follow_visual_end = self.follow_visual_end;
        let pin_caret_visible = self.pin_caret_visible;
        self.typography = next;
        self.layout = LayoutState::default();
        self.set_primary_focus(caret);
        if let Some(anchor) = selection_anchor {
            self.selections.primary_mut().set_anchor(anchor);
        } else {
            self.clear_selection();
        }
        self.visual_scroll_y = visual_scroll_y;
        self.last_visual_max_scroll_y = last_visual_max_scroll_y;
        self.follow_visual_end = follow_visual_end;
        self.pin_caret_visible = pin_caret_visible;
        true
    }

    pub fn layout_style_revision_for_test(&self) -> u64 {
        self.layout_style_revision
    }

    pub fn visible_diagnostic_paint_ranges_for_test(&self) -> Vec<(Range<usize>, Color)> {
        self.visible_diagnostic_ranges(&self.visible_snapshot())
    }

    pub fn visible_decoration_paint_ranges_for_test(&self) -> Vec<(Range<usize>, Color)> {
        self.visible_decoration_ranges(&self.visible_snapshot())
    }

    /// Normalized presentation runs for the current visible snapshot:
    /// `(range, font_role, [bold, italic, underline, strike], color, background, scale)`.
    /// Integration tests assert rendered styling through this seam instead of
    /// raw token emission.
    pub fn visible_text_style_runs_for_test(&self) -> Vec<VisibleTextStyleRunForTest> {
        normalize_visible_text_style_runs(
            &self.decorations,
            &self.document,
            self.buffer.document_end_byte(),
            &self.visible_snapshot(),
            self.document_font_role(),
            self.theme,
        )
        .into_iter()
        .map(|run| {
            (
                run.range,
                run.font_role,
                [
                    run.attributes.bold,
                    run.attributes.italic,
                    run.attributes.underline,
                    run.attributes.strike,
                ],
                run.color,
                run.background,
                run.scale,
            )
        })
        .collect()
    }

    pub fn decoration_span_count(&self) -> usize {
        self.decorations.span_count()
    }

    pub fn diagnostic_span_count(&self) -> usize {
        self.diagnostics.span_count()
    }

    pub fn visible_diagnostic_spans(
        &self,
        visible_start: u64,
        visible_end: u64,
    ) -> impl Iterator<Item = &DiagnosticSpan> {
        self.diagnostics.visible_spans(visible_start, visible_end)
    }

    pub fn decoration_state_version(&self) -> Option<DocumentVersion> {
        self.decorations.state_version()
    }

    /// Single source of color for the editor + shell paint path. `StyleRegistry`
    /// is `Copy`, so callers cheaply snapshot the resolved theme for one paint.
    pub(crate) fn theme(&self) -> StyleRegistry {
        self.theme
    }

    pub(crate) fn typography(&self) -> &TypographyRegistry {
        &self.typography
    }

    /// Swap the active theme registry. Called by the task-7 `setTheme` flow at
    /// load/reload (never during paint); the resolved registry is then
    /// immutable until the next swap. This is the only sanctioned way to mutate
    /// `theme` after construction.
    #[allow(dead_code)] // wired by Plan 046 task 7 (setTheme) — keep the hook live.
    pub(crate) fn set_theme(&mut self, theme: StyleRegistry) {
        if self.theme != theme {
            self.theme = theme;
            self.bump_layout_style_revision();
        }
    }

    /// Install resolved UI design-token registry for shell chrome (Phase 20.2).
    pub(crate) fn set_ui_theme(&mut self, ui_theme: crate::shell::theme::ResolvedUiTheme) {
        self.ui_theme = ui_theme;
    }

    /// Read-only access to the resolved UI design-token registry for shell
    /// chrome (spacing/dimension/color tokens). Phase 20.4 task 6: the status
    /// bar reads spacing tokens here.
    pub(crate) fn ui_theme(&self) -> &crate::shell::theme::ResolvedUiTheme {
        &self.ui_theme
    }

    /// Phase 20.4 task 5: feed the editor chrome the pointer position (absolute
    /// widget coords, same space as the `rect` passed to `paint_in_rect`) so the
    /// scrollbar can derive Hover/Active state. `None` clears hover.
    pub(crate) fn set_pointer_pos(&mut self, point: Option<masonry::kurbo::Point>) {
        self.pointer_pos = point;
    }

    /// Phase 20.4 task 5: feed the editor chrome the primary-button press state
    /// so the scrollbar can derive Active state when the press is over the thumb.
    pub(crate) fn set_pointer_pressed(&mut self, pressed: bool) {
        self.pointer_pressed = pressed;
    }

    /// Phase 20.4 task 5: clear pointer-driven chrome state (pointer leave /
    /// cancel).
    pub(crate) fn clear_pointer_chrome_state(&mut self) {
        self.pointer_pos = None;
        self.pointer_pressed = false;
    }

    /// Phase 20.4 task 5: derive the scrollbar InteractionState from the
    /// pointer position and press state. Hover when the pointer is over the
    /// track; Active when pressed over the thumb (hit-test on
    /// `scrollbar_thumb_rect`); otherwise Rest. O(1).
    pub(crate) fn scrollbar_interaction_state(
        &self,
        rect: Rect,
        available_height: f64,
    ) -> crate::shell::primitives::InteractionState {
        use crate::shell::primitives::InteractionState;
        let Some(point) = self.pointer_pos else {
            return InteractionState::Rest;
        };
        let track_y0 = rect.y0 + self.inset_y();
        let track_y1 = rect.y0 + self.inset_y() + available_height;
        let x1 = rect.x1 - SCROLLBAR_MARGIN;
        let x0 = x1 - SCROLLBAR_WIDTH;
        let track = Rect::new(x0, track_y0, x1, track_y1);
        if !track.contains(point) {
            return InteractionState::Rest;
        }
        if self.pointer_pressed
            && self
                .scrollbar_thumb_rect(rect)
                .is_some_and(|thumb| thumb.contains(point))
        {
            return InteractionState::Active;
        }
        InteractionState::Hover
    }

    /// Install an inert `ActiveTheme` snapshot: resolve colors into the
    /// registry and retain the package specifier for theme-label observability.
    pub(crate) fn set_active_theme(&mut self, theme: &crate::protocol::ActiveTheme) {
        self.theme_specifier = theme.specifier.clone();
        let registry = crate::editor::theme::StyleRegistry::from_active_theme(theme);
        let base = registry.base;
        self.set_theme(registry);
        // Phase 20.2: install resolved UI tokens for shell chrome, layered over
        // the editor base palette so the chrome (scrollbar, panels) matches the
        // editor text theme instead of falling through to the dark core catalog.
        if let Ok(ui_theme) =
            crate::shell::theme::ResolvedUiTheme::from_active_theme(&theme.design_tokens)
        {
            self.set_ui_theme(ui_theme.with_base_ui(&base));
        }
    }

    /// Active theme package specifier (`@clay/...`), or `@clay/default` before install.
    pub(crate) fn theme_specifier(&self) -> &str {
        if self.theme_specifier.is_empty() {
            "@clay/default"
        } else {
            self.theme_specifier.as_str()
        }
    }

    /// Compact theme label for status/accessibility (no package path).
    pub(crate) fn set_theme_specifier(&mut self, specifier: impl Into<String>) {
        self.theme_specifier = specifier.into();
    }

    pub(crate) fn theme_label(&self) -> String {
        crate::editor::theme::theme_display_label(self.theme_specifier())
    }

    /// Install newer validated typography and discard geometry derived from the
    /// old profiles. Invalid/stale snapshots leave the current layout intact.
    pub(crate) fn set_typography_registry(&mut self, typography: TypographyRegistry) {
        self.typography = typography;
        self.layout = LayoutState::default();
        self.visual_scroll_y = 0.0;
        self.last_visual_max_scroll_y = 0.0;
        self.pin_caret_visible = false;
        self.bump_layout_style_revision();
    }

    pub(crate) fn set_typography(&mut self, typography: crate::protocol::ActiveTypography) -> bool {
        let Ok(changed) = self.typography.install(typography) else {
            return false;
        };
        if changed {
            self.layout = LayoutState::default();
            self.visual_scroll_y = 0.0;
            self.visual_scroll_x = 0.0;
            self.last_visual_max_scroll_y = 0.0;
            self.last_visual_max_scroll_x = 0.0;
            self.pin_caret_visible = false;
        }
        changed
    }

    fn document_font_role(&self) -> FontRole {
        self.document
            .behavior_manifest
            .as_ref()
            .and_then(|manifest| manifest.document_font_role.font_role())
            .unwrap_or(FontRole::Proportional)
    }

    fn inset_x(&self) -> f64 {
        if self.resolved_chrome().gutter {
            TEXT_INSET_GUTTER
        } else {
            TEXT_INSET
        }
    }

    fn inset_y(&self) -> f64 {
        TEXT_INSET_Y
    }

    pub(crate) fn resolved_wrap(&self) -> WrapPolicy {
        if let Some(wrap) = self.layout_override {
            return wrap;
        }
        let Some(manifest) = self.document.behavior_manifest.as_ref() else {
            return WrapPolicy::Viewport;
        };
        manifest
            .editor_rules
            .layout
            .map(|layout| layout.wrap)
            .unwrap_or_else(|| WrapPolicy::from_font_role(manifest.document_font_role))
    }

    /// User wrap override. `None` restores the mode default. Packages cannot clear this.
    pub fn set_editor_layout(&mut self, wrap: Option<WrapPolicy>) -> bool {
        if self.layout_override == wrap {
            return false;
        }
        self.layout_override = wrap;
        self.visual_scroll_x = 0.0;
        self.last_visual_max_scroll_x = 0.0;
        self.bump_layout_style_revision();
        true
    }

    fn layout_max_width(&self, pane_width: f64) -> f32 {
        let content = (pane_width - self.inset_x() * 2.0).max(1.0) as f32;
        match self.resolved_wrap() {
            WrapPolicy::None => f32::MAX,
            WrapPolicy::Viewport => content,
            WrapPolicy::Column(cols) => {
                let size = self.typography.profile(self.document_font_role()).size();
                // ponytail: 0.6em average advance; measure from layout if column looks off.
                content.min(f32::from(cols) * size * 0.6)
            }
        }
    }

    fn conservative_document_line_height(&self) -> f64 {
        // ponytail: uniform body line height; per-line metrics if mixed-heading
        // scroll drift matters.
        self.typography.document_line_height()
    }

    fn bump_layout_style_revision(&mut self) {
        self.layout_style_revision = self.layout_style_revision.saturating_add(1);
    }

    /// Phase 24.5: does the installed behavior manifest claim `key` as the
    /// first stroke of a bound chord (a single-stroke exact match or the
    /// prefix of a multi-stroke sequence)? Non-mutating and allocation-free;
    /// widget key handling uses this to give manifest-claimed strokes
    /// precedence over hard-coded platform shortcuts (e.g. so `Ctrl+X` starts
    /// the `Ctrl+X Ctrl+P` Command Centre chord instead of being swallowed by
    /// the hard-coded cut shortcut).
    pub(crate) fn manifest_claims_chord(&self, key: &KeyStroke) -> bool {
        self.document
            .behavior_manifest
            .as_ref()
            .is_some_and(|manifest| crate::client::behavior::manifest_claims_chord(manifest, key))
    }

    pub(crate) fn route_key_with_event(&mut self, key: &KeyStroke) -> EditorKeyOutcome {
        if matches!(key.key, KeyCode::Tab) && self.snippet_session.is_some() {
            self.pending_chord = None;
            let changed = if key.modifiers.shift {
                self.select_previous_snippet_placeholder()
            } else {
                self.select_next_snippet_placeholder()
            };
            return EditorKeyOutcome::client(EditorCommandOutcome::from_changed(changed));
        }
        if matches!(key.key, KeyCode::Escape) && self.snippet_session.take().is_some() {
            self.pending_chord = None;
            return EditorKeyOutcome::client(EditorCommandOutcome::from_changed(true));
        }
        // Plan 071 task 9: with no menu (widget-handled) or snippet session
        // active, Escape collapses the selection set to the primary caret.
        if matches!(key.key, KeyCode::Escape)
            && (self.selections.selection_count() > 1 || self.has_selection())
        {
            self.pending_chord = None;
            let changed = self.cancel_multiple_selections();
            return EditorKeyOutcome::client(EditorCommandOutcome::from_changed(changed));
        }

        let Some(manifest) = &self.document.behavior_manifest else {
            return EditorKeyOutcome::unhandled();
        };
        let Ok(router) = ClientBehaviorState::new(manifest.clone()) else {
            return EditorKeyOutcome::unhandled();
        };

        // Phase 24.5: a stale pending chord cancels on the next keystroke and
        // the key is re-evaluated as a fresh stroke.
        if self.pending_chord.as_ref().is_some_and(|pending| {
            pending.started_at.elapsed()
                >= std::time::Duration::from_millis(KEY_CHORD_PENDING_TIMEOUT_MS)
        }) {
            self.pending_chord = None;
        }

        let outcome = match &self.pending_chord {
            Some(pending) => router.route_key_sequence(&pending.strokes, key),
            None => router.route_key_sequence(&[], key),
        };
        match outcome {
            ChordRouteOutcome::Matched(behavior) => {
                self.pending_chord = None;
                self.dispatch_routed(behavior)
            }
            ChordRouteOutcome::Pending => {
                // Keep waiting: extend the buffer and consume the key so it
                // neither inserts text nor bubbles to shell handlers.
                let started_at = self
                    .pending_chord
                    .as_ref()
                    .map(|pending| pending.started_at)
                    .unwrap_or_else(std::time::Instant::now);
                let mut strokes = self
                    .pending_chord
                    .take()
                    .map(|pending| pending.strokes)
                    .unwrap_or_default();
                strokes.push(key.clone());
                self.pending_chord = Some(PendingChord {
                    strokes,
                    started_at,
                });
                EditorKeyOutcome::consumed()
            }
            ChordRouteOutcome::Mismatch => {
                // Abandoned chord: clear it and re-evaluate the key fresh so
                // abandoning a prefix never eats typing (Emacs behavior).
                self.pending_chord = None;
                match router.route_key_sequence(&[], key) {
                    ChordRouteOutcome::Matched(behavior) => self.dispatch_routed(behavior),
                    ChordRouteOutcome::Pending => {
                        self.pending_chord = Some(PendingChord {
                            strokes: vec![key.clone()],
                            started_at: std::time::Instant::now(),
                        });
                        EditorKeyOutcome::consumed()
                    }
                    ChordRouteOutcome::Mismatch => self.dispatch_routed(router.route_key(key)),
                }
            }
        }
    }

    /// Dispatch a routed behavior to its side effect path (Phase 24.5:
    /// shared by the single-stroke fast path and the chord matcher).
    fn dispatch_routed(&mut self, behavior: RoutedBehavior) -> EditorKeyOutcome {
        match behavior {
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText(text), completion_trigger) => {
                let implicit_completion = completion_trigger.or_else(|| {
                    (self
                        .document
                        .behavior_manifest
                        .as_ref()
                        .is_some_and(|manifest| {
                            !manifest.editor_rules.autocomplete_triggers.is_empty()
                        })
                        && text.chars().all(is_completion_word_character))
                    .then_some(CompletionTriggerRoute {
                        trigger: CompletionTrigger::Manual,
                        routing_policy: crate::protocol::RoutingPolicy::UiReactivePriority,
                    })
                });
                let outcome = if let Some(pair) = self.pair_rule_for_inserted_text(&text).cloned() {
                    self.insert_pair_with_event(&pair)
                } else if let Some(electric) = self.electric_rule_for_inserted_text(&text).cloned()
                {
                    self.insert_electric_with_event(&electric)
                } else {
                    self.insert_text_with_event(&text)
                };
                let completion_request = outcome
                    .changed
                    .then_some(implicit_completion)
                    .flatten()
                    .and_then(|route| self.completion_request_event(route));
                EditorKeyOutcome::client(outcome).with_completion(completion_request)
            }
            RoutedBehavior::ClientEdit(ClientLocalEdit::Newline, _) => {
                EditorKeyOutcome::client(self.insert_newline_with_event())
            }
            RoutedBehavior::Completion(completion_trigger) => self
                .completion_request_event(completion_trigger)
                .map(EditorKeyOutcome::completion)
                .unwrap_or_else(EditorKeyOutcome::unhandled),
            RoutedBehavior::LanguageIntelligence(route) => self
                .language_intelligence_request_event(route)
                .map(EditorKeyOutcome::language_intelligence)
                .unwrap_or_else(EditorKeyOutcome::unhandled),
            RoutedBehavior::ServerIntent(intent) => EditorKeyOutcome::server(intent),
            RoutedBehavior::ClientUiCommand(command) => EditorKeyOutcome::client_ui(command),
            RoutedBehavior::Unhandled => EditorKeyOutcome::unhandled(),
        }
    }

    /// Route a key against global bindings without enabling editor text edits.
    ///
    /// The welcome surface has no editable document interaction, but it still
    /// needs global commands such as shell topology changes and command-center
    /// opening. Client-edit behaviors are deliberately ignored here.
    pub(crate) fn route_global_key_with_event(&mut self, key: &KeyStroke) -> EditorKeyOutcome {
        let Some(manifest) = &self.document.behavior_manifest else {
            return EditorKeyOutcome::unhandled();
        };
        let Ok(router) = ClientBehaviorState::new(manifest.clone()) else {
            return EditorKeyOutcome::unhandled();
        };

        if self.pending_chord.as_ref().is_some_and(|pending| {
            pending.started_at.elapsed()
                >= std::time::Duration::from_millis(KEY_CHORD_PENDING_TIMEOUT_MS)
        }) {
            self.pending_chord = None;
        }

        let outcome = match &self.pending_chord {
            Some(pending) => router.route_global_key_sequence(&pending.strokes, key),
            None => router.route_global_key_sequence(&[], key),
        };
        match outcome {
            ChordRouteOutcome::Matched(behavior) => {
                self.pending_chord = None;
                match behavior {
                    RoutedBehavior::ClientEdit(..) | RoutedBehavior::Unhandled => {
                        EditorKeyOutcome::unhandled()
                    }
                    behavior => self.dispatch_routed(behavior),
                }
            }
            ChordRouteOutcome::Pending => {
                let started_at = self
                    .pending_chord
                    .as_ref()
                    .map(|pending| pending.started_at)
                    .unwrap_or_else(std::time::Instant::now);
                let mut strokes = self
                    .pending_chord
                    .take()
                    .map(|pending| pending.strokes)
                    .unwrap_or_default();
                strokes.push(key.clone());
                self.pending_chord = Some(PendingChord {
                    strokes,
                    started_at,
                });
                EditorKeyOutcome::consumed()
            }
            ChordRouteOutcome::Mismatch => {
                self.pending_chord = None;
                match router.route_global_key_sequence(&[], key) {
                    ChordRouteOutcome::Matched(behavior) => match behavior {
                        RoutedBehavior::ClientEdit(..) | RoutedBehavior::Unhandled => {
                            EditorKeyOutcome::unhandled()
                        }
                        behavior => self.dispatch_routed(behavior),
                    },
                    ChordRouteOutcome::Pending => {
                        self.pending_chord = Some(PendingChord {
                            strokes: vec![key.clone()],
                            started_at: std::time::Instant::now(),
                        });
                        EditorKeyOutcome::consumed()
                    }
                    ChordRouteOutcome::Mismatch => EditorKeyOutcome::unhandled(),
                }
            }
        }
    }

    pub fn document_state(&self) -> &EditorDocumentState {
        &self.document
    }

    pub fn note_confirmed_version(
        &mut self,
        document_id: DocumentId,
        version: DocumentVersion,
    ) -> bool {
        if self.document.document_id != document_id || self.document.document_version == version {
            return false;
        }

        self.document.document_version = version;
        self.decorations.confirm_version(document_id, version);
        self.diagnostics = EditorDiagnosticState::default();
        true
    }

    pub fn command(&mut self, command: EditorCommand<'_>) -> bool {
        // Any user command (edit or movement) restarts the blink idle phase so
        // the caret stays solid while typing, when the style asks for it.
        if self.effective_caret_style().stop_blink_on_typing {
            self.caret_blink.reset();
        }
        self.command_with_event(command).changed
    }

    /// Jump the caret to a UTF-8 byte offset for definition navigation.
    /// Clamps to a valid scalar boundary; does not mutate document text.
    pub fn navigate_to_byte_offset(&mut self, byte_offset: u64) -> bool {
        let caret = self
            .buffer
            .clamp_byte_offset(usize::try_from(byte_offset).unwrap_or(usize::MAX));
        self.collapse_selection_to(caret)
    }

    pub fn command_with_event(&mut self, command: EditorCommand<'_>) -> EditorCommandOutcome {
        if command.is_selection_changing() {
            self.snapshot_selection_set();
        }
        match command {
            EditorCommand::Insert(text) => self.insert_text_with_event(text),
            EditorCommand::Newline => self.insert_newline_with_event(),
            EditorCommand::Backspace => self.backspace_with_event(),
            EditorCommand::DeleteForward => self.delete_forward_with_event(),
            EditorCommand::MoveLeft => EditorCommandOutcome::from_changed(self.move_left()),
            EditorCommand::MoveRight => EditorCommandOutcome::from_changed(self.move_right()),
            EditorCommand::SelectLeft => EditorCommandOutcome::from_changed(self.select_left()),
            EditorCommand::SelectRight => EditorCommandOutcome::from_changed(self.select_right()),
            EditorCommand::MoveUp => EditorCommandOutcome::from_changed(self.move_up()),
            EditorCommand::MoveDown => EditorCommandOutcome::from_changed(self.move_down()),
            EditorCommand::LineStart => {
                EditorCommandOutcome::from_changed(self.move_to_line_start())
            }
            EditorCommand::LineEnd => EditorCommandOutcome::from_changed(self.move_to_line_end()),
            EditorCommand::DocumentStart => {
                EditorCommandOutcome::from_changed(self.move_to_document_start())
            }
            EditorCommand::DocumentEnd => {
                EditorCommandOutcome::from_changed(self.move_to_document_end())
            }
            EditorCommand::MoveWordStart {
                forward,
                long,
                extend,
            } => EditorCommandOutcome::from_changed(self.move_word_start(forward, long, extend)),
            EditorCommand::MoveWordEnd {
                forward,
                long,
                extend,
            } => EditorCommandOutcome::from_changed(self.move_word_end(forward, long, extend)),
            EditorCommand::MoveSubWord { forward, extend } => {
                EditorCommandOutcome::from_changed(self.move_sub_word(forward, extend))
            }
            EditorCommand::MoveParagraph {
                forward,
                to_end,
                extend,
            } => EditorCommandOutcome::from_changed(self.move_paragraph(forward, to_end, extend)),
            EditorCommand::MoveFirstNonWhitespace { extend } => {
                EditorCommandOutcome::from_changed(self.move_first_non_blank(extend))
            }
            EditorCommand::MoveLastNonWhitespace { extend } => {
                EditorCommandOutcome::from_changed(self.move_last_non_blank(extend))
            }
            EditorCommand::MoveMatchingPair { extend } => {
                EditorCommandOutcome::from_changed(self.move_matching_pair(extend))
            }
            EditorCommand::SelectWord => EditorCommandOutcome::from_changed(self.select_word()),
            EditorCommand::SelectLine => EditorCommandOutcome::from_changed(self.select_line()),
            EditorCommand::SelectParagraph => {
                EditorCommandOutcome::from_changed(self.select_paragraph())
            }
            EditorCommand::AddCursor { direction } => EditorCommandOutcome::from_changed(
                self.add_cursor_line(matches!(direction, CursorSelectDirection::Down)),
            ),
            EditorCommand::ColumnSelect { direction } => match direction {
                CursorSelectDirection::Down | CursorSelectDirection::Up => {
                    EditorCommandOutcome::from_changed(
                        self.add_cursor_line(matches!(direction, CursorSelectDirection::Down)),
                    )
                }
                CursorSelectDirection::Left => {
                    EditorCommandOutcome::from_changed(self.move_all_carets(false))
                }
                CursorSelectDirection::Right => {
                    EditorCommandOutcome::from_changed(self.move_all_carets(true))
                }
            },
            EditorCommand::SelectNextMatch => {
                EditorCommandOutcome::from_changed(self.select_next_match(true))
            }
            EditorCommand::SelectPrevMatch => {
                EditorCommandOutcome::from_changed(self.select_next_match(false))
            }
            EditorCommand::SelectAllMatches => {
                EditorCommandOutcome::from_changed(self.select_all_matches())
            }
            EditorCommand::CancelMultipleSelections => {
                EditorCommandOutcome::from_changed(self.cancel_multiple_selections())
            }
            EditorCommand::KeepSelection => {
                EditorCommandOutcome::from_changed(self.keep_selection())
            }
            EditorCommand::RemoveSelection => {
                EditorCommandOutcome::from_changed(self.remove_selection())
            }
            EditorCommand::UndoCursorMove => self.undo_cursor_move(),
            EditorCommand::ToggleComment => {
                self.apply_line_prefix_transform(LinePrefixKind::ToggleComment)
            }
            EditorCommand::ToggleListMarker => {
                self.apply_line_prefix_transform(LinePrefixKind::ToggleListMarker)
            }
            EditorCommand::RotateHeading => {
                self.apply_line_prefix_transform(LinePrefixKind::RotateHeading)
            }
            EditorCommand::ToggleFold => EditorCommandOutcome::from_changed(self.toggle_fold()),
            EditorCommand::ToggleInlayHints => {
                EditorCommandOutcome::from_changed(self.toggle_inlay_hints())
            }
        }
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        self.insert_text_with_event(text).changed
    }

    pub(crate) fn accept_completion_with_event(
        &mut self,
        action: &CompletionMenuAcceptAction,
        commit_character: Option<&str>,
    ) -> EditorCommandOutcome {
        if !self.is_editable()
            || self.document.document_id != action.document_id
            || self.document.document_version != action.document_version
            || self.document.behavior_version != action.behavior_version
            || !action.replacement_range.is_ordered()
        {
            return EditorCommandOutcome::unchanged();
        }
        let start = action.replacement_range.byte_start as usize;
        let end = action.replacement_range.byte_end as usize;
        if end > self.buffer.document_end_byte() {
            return EditorCommandOutcome::unchanged();
        }
        let (mut text, snippet_placeholders) = match action.text_format {
            CompletionItemTextFormat::PlainText => (action.insert_text.clone(), None),
            CompletionItemTextFormat::Snippet => {
                let Ok(expansion) = parse_snippet(&action.insert_text) else {
                    return EditorCommandOutcome::unchanged();
                };
                let mut placeholders = expansion.placeholders;
                for placeholder in &mut placeholders {
                    let Some(byte_start) = start.checked_add(placeholder.byte_start) else {
                        return EditorCommandOutcome::unchanged();
                    };
                    let Some(byte_end) = start.checked_add(placeholder.byte_end) else {
                        return EditorCommandOutcome::unchanged();
                    };
                    placeholder.byte_start = byte_start;
                    placeholder.byte_end = byte_end;
                }
                (expansion.text, Some(placeholders))
            }
        };
        if let Some(commit_character) = commit_character {
            text.push_str(commit_character);
        }
        if text.is_empty() && start == end {
            return EditorCommandOutcome::unchanged();
        }
        let operation = EditOperation::Replace {
            start: start as u64,
            end: end as u64,
            text: text.clone(),
        };
        let mut outcome = self.apply_and_record_local_edit(operation, None);
        if let Some(placeholders) = snippet_placeholders
            && self.install_snippet_session(placeholders)
        {
            outcome.changed = true;
        }
        outcome
    }

    #[cfg(test)]
    pub(crate) fn has_active_snippet_session(&self) -> bool {
        self.snippet_session.is_some()
    }

    fn install_snippet_session(&mut self, mut placeholders: Vec<SnippetPlaceholder>) -> bool {
        placeholders.sort_by_key(|placeholder| {
            (
                placeholder.final_tabstop,
                placeholder.index,
                placeholder.byte_start,
            )
        });
        let had_session = self.snippet_session.take().is_some();
        let Some(first) = placeholders.first().copied() else {
            return had_session;
        };
        if first.final_tabstop {
            return self.collapse_selection_to(first.byte_end) || had_session;
        }
        self.snippet_session = Some(SnippetSession {
            placeholders,
            active_index: 0,
        });
        self.select_snippet_placeholder(first);
        true
    }

    fn select_next_snippet_placeholder(&mut self) -> bool {
        let Some(session) = &mut self.snippet_session else {
            return false;
        };
        let Some(next_index) = session.active_index.checked_add(1) else {
            return false;
        };
        let Some(placeholder) = session.placeholders.get(next_index).copied() else {
            let caret = session.placeholders[session.active_index].byte_end;
            self.snippet_session = None;
            self.collapse_selection_to(caret);
            return true;
        };
        if placeholder.final_tabstop {
            self.snippet_session = None;
            self.collapse_selection_to(placeholder.byte_end);
        } else {
            session.active_index = next_index;
            self.select_snippet_placeholder(placeholder);
        }
        true
    }

    fn select_previous_snippet_placeholder(&mut self) -> bool {
        let Some(session) = &mut self.snippet_session else {
            return false;
        };
        if session.active_index == 0 {
            return true;
        }
        session.active_index -= 1;
        let placeholder = session.placeholders[session.active_index];
        self.select_snippet_placeholder(placeholder);
        true
    }

    fn select_snippet_placeholder(&mut self, placeholder: SnippetPlaceholder) {
        self.set_primary_focus(placeholder.byte_end);
        let start = placeholder.byte_start;
        let end = self.caret();
        self.selections.primary_mut().set_anchor(start);
        if start == end {
            self.clear_selection();
        }
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
    }

    fn update_snippet_session_after_edit(&mut self, operation: &EditOperation) {
        let edit = match operation {
            EditOperation::Insert { byte_offset, text } => usize::try_from(*byte_offset)
                .ok()
                .map(|start| (start, start, text.len())),
            EditOperation::Delete { start, end } => usize::try_from(*start)
                .ok()
                .zip(usize::try_from(*end).ok())
                .map(|(start, end)| (start, end, 0)),
            EditOperation::Replace { start, end, text } => usize::try_from(*start)
                .ok()
                .zip(usize::try_from(*end).ok())
                .map(|(start, end)| (start, end, text.len())),
        };
        let Some((start, end, inserted_len)) = edit else {
            self.snippet_session = None;
            return;
        };
        let Some(session) = &mut self.snippet_session else {
            return;
        };
        let active = session.placeholders[session.active_index];
        if start < active.byte_start || end > active.byte_end {
            self.snippet_session = None;
            return;
        }

        let removed_len = end - start;
        for (index, placeholder) in session.placeholders.iter_mut().enumerate() {
            if index == session.active_index {
                let Some(byte_end) =
                    shift_snippet_offset(placeholder.byte_end, removed_len, inserted_len)
                else {
                    self.snippet_session = None;
                    return;
                };
                placeholder.byte_end = byte_end;
            } else if placeholder.byte_start >= end {
                let Some(byte_start) =
                    shift_snippet_offset(placeholder.byte_start, removed_len, inserted_len)
                else {
                    self.snippet_session = None;
                    return;
                };
                let Some(byte_end) =
                    shift_snippet_offset(placeholder.byte_end, removed_len, inserted_len)
                else {
                    self.snippet_session = None;
                    return;
                };
                placeholder.byte_start = byte_start;
                placeholder.byte_end = byte_end;
            } else if placeholder.byte_end > start {
                self.snippet_session = None;
                return;
            }
        }
    }

    pub fn insert_text_with_event(&mut self, text: &str) -> EditorCommandOutcome {
        if !self.is_editable() || !is_printable_text(text) {
            return EditorCommandOutcome::unchanged();
        }

        self.replace_selection_or_insert_with_event(text)
    }

    /// Insert or replace using clipboard paste text.
    ///
    /// Unlike ordinary typed insertion, paste may include newlines and tabs after
    /// line-ending normalization. Empty or control-containing clipboard payloads
    /// are no-ops.
    pub fn paste_text_with_event(&mut self, text: &str) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }
        let Some(normalized) = crate::editor::normalize_clipboard_paste_text(text) else {
            return EditorCommandOutcome::unchanged();
        };
        self.replace_selection_or_insert_with_event(&normalized)
    }

    /// True when a non-empty IME preedit overlay is active.
    pub fn is_composing(&self) -> bool {
        self.composition.is_active()
    }

    /// Paint-only preedit text, if composition is active.
    pub fn preedit_text(&self) -> Option<&str> {
        self.composition
            .is_active()
            .then_some(self.composition.text())
    }

    /// Optional byte-indexed cursor span within the active preedit.
    pub fn preedit_cursor_span(&self) -> Option<(usize, usize)> {
        self.composition.cursor_span()
    }

    /// Update the local preedit overlay. Empty text clears composition.
    ///
    /// This never mutates the canonical rope or enqueues edits.
    pub fn set_preedit(&mut self, text: String, cursor: Option<(usize, usize)>) -> bool {
        self.composition.set_preedit(text, cursor)
    }

    /// Discard unfinished composition without committing text.
    pub fn cancel_composition(&mut self) -> bool {
        self.composition.clear()
    }

    pub fn insert_newline(&mut self) -> bool {
        self.insert_newline_with_event().changed
    }

    pub fn insert_newline_with_event(&mut self) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        if self.selections.selection_count() > 1 {
            return self.multi_caret_edit(|surface, focus, range| {
                if range.start < range.end {
                    Some((
                        EditOperation::Replace {
                            start: range.start as u64,
                            end: range.end as u64,
                            text: "\n".to_string(),
                        },
                        range.start + 1,
                    ))
                } else {
                    let offset = surface.buffer.clamp_byte_offset(focus);
                    let text = surface.newline_text_at(offset);
                    let final_caret = offset + text.len();
                    Some((
                        EditOperation::Insert {
                            byte_offset: offset as u64,
                            text,
                        },
                        final_caret,
                    ))
                }
            });
        }

        let operation = if let Some(range) = self.selected_range() {
            EditOperation::Replace {
                start: range.start as u64,
                end: range.end as u64,
                text: "\n".to_string(),
            }
        } else {
            let byte_offset = self.buffer.clamp_byte_offset(self.caret());
            let text = self.newline_text_at(byte_offset);
            EditOperation::Insert {
                byte_offset: byte_offset as u64,
                text,
            }
        };
        self.apply_and_record_local_edit(operation, None)
    }

    pub fn backspace(&mut self) -> bool {
        self.backspace_with_event().changed
    }

    pub fn backspace_with_event(&mut self) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        if self.selections.selection_count() > 1 {
            return self.multi_caret_edit(|surface, focus, range| {
                if range.start < range.end {
                    Some((
                        EditOperation::Delete {
                            start: range.start as u64,
                            end: range.end as u64,
                        },
                        range.start,
                    ))
                } else {
                    let caret = surface.buffer.clamp_byte_offset(focus);
                    let previous = surface.buffer.previous_scalar_boundary(caret)?;
                    Some((
                        EditOperation::Delete {
                            start: previous as u64,
                            end: caret as u64,
                        },
                        previous,
                    ))
                }
            });
        }

        if let Some(range) = self.selected_range() {
            return self.apply_and_record_local_edit(
                EditOperation::Delete {
                    start: range.start as u64,
                    end: range.end as u64,
                },
                None,
            );
        }

        let caret = self.buffer.clamp_byte_offset(self.caret());
        let Some(previous) = self.buffer.previous_scalar_boundary(caret) else {
            let result = self.buffer.backspace_at(caret);
            return self.finish_edit(result);
        };
        self.apply_and_record_local_edit(
            EditOperation::Delete {
                start: previous as u64,
                end: caret as u64,
            },
            None,
        )
    }

    pub fn delete_forward(&mut self) -> bool {
        self.delete_forward_with_event().changed
    }

    pub fn delete_forward_with_event(&mut self) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        if self.selections.selection_count() > 1 {
            return self.multi_caret_edit(|surface, focus, range| {
                if range.start < range.end {
                    Some((
                        EditOperation::Delete {
                            start: range.start as u64,
                            end: range.end as u64,
                        },
                        range.start,
                    ))
                } else {
                    let caret = surface.buffer.clamp_byte_offset(focus);
                    let next = surface.buffer.next_scalar_boundary(caret)?;
                    Some((
                        EditOperation::Delete {
                            start: caret as u64,
                            end: next as u64,
                        },
                        caret,
                    ))
                }
            });
        }

        if let Some(range) = self.selected_range() {
            return self.apply_and_record_local_edit(
                EditOperation::Delete {
                    start: range.start as u64,
                    end: range.end as u64,
                },
                None,
            );
        }

        let caret = self.buffer.clamp_byte_offset(self.caret());
        let Some(next) = self.buffer.next_scalar_boundary(caret) else {
            let result = self.buffer.delete_after(caret);
            return self.finish_edit(result);
        };
        self.apply_and_record_local_edit(
            EditOperation::Delete {
                start: caret as u64,
                end: next as u64,
            },
            None,
        )
    }

    pub fn move_left(&mut self) -> bool {
        if let Some(range) = self.selected_range() {
            return self.collapse_selection_to(range.start);
        }

        self.move_cursor(|cursor, buffer| cursor.move_to_previous_scalar(buffer))
    }

    pub fn move_right(&mut self) -> bool {
        if let Some(range) = self.selected_range() {
            return self.collapse_selection_to(range.end);
        }

        self.move_cursor(|cursor, buffer| cursor.move_to_next_scalar(buffer))
    }

    pub fn select_left(&mut self) -> bool {
        self.extend_selection(|cursor, buffer| cursor.move_to_previous_scalar(buffer))
    }

    pub fn select_right(&mut self) -> bool {
        self.extend_selection(|cursor, buffer| cursor.move_to_next_scalar(buffer))
    }

    pub fn move_up(&mut self) -> bool {
        let sticky = self.sticky_column_enabled();
        let changed = self.move_cursor(|cursor, buffer| {
            if sticky {
                cursor.move_to_previous_line(buffer)
            } else if cursor.move_to_previous_line(buffer) {
                cursor.move_to_line_start(buffer);
                true
            } else {
                false
            }
        });
        if changed {
            self.skip_hidden_lines(false);
        }
        changed
    }

    pub fn move_down(&mut self) -> bool {
        let sticky = self.sticky_column_enabled();
        let changed = self.move_cursor(|cursor, buffer| {
            if sticky {
                cursor.move_to_next_line(buffer)
            } else if cursor.move_to_next_line(buffer) {
                cursor.move_to_line_start(buffer);
                true
            } else {
                false
            }
        });
        if changed {
            self.skip_hidden_lines(true);
        }
        changed
    }

    fn skip_hidden_lines(&mut self, forward: bool) -> bool {
        if self.folds.collapsed.is_empty() {
            return false;
        }
        let mut skipped = false;
        loop {
            let line = self.buffer.line_of_byte(self.caret());
            if !self.line_is_hidden(line) {
                break;
            }
            let moved = self.move_cursor(|cursor, buffer| {
                if forward {
                    cursor.move_to_next_line(buffer)
                } else {
                    cursor.move_to_previous_line(buffer)
                }
            });
            if !moved {
                break;
            }
            skipped = true;
        }
        skipped
    }

    pub fn move_to_line_start(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_line_start(buffer))
    }

    pub fn move_to_line_end(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_line_end(buffer))
    }

    pub fn move_to_document_start(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_document_start(buffer))
    }

    pub fn move_to_document_end(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_document_end(buffer))
    }

    /// Active movement policy (cloned per command; never on the typing hot path).
    fn movement_rules(&self) -> MovementRules {
        self.document
            .behavior_manifest
            .as_ref()
            .map(|manifest| manifest.editor_rules.movement.clone())
            .unwrap_or_default()
    }

    fn sticky_column_enabled(&self) -> bool {
        self.document
            .behavior_manifest
            .as_ref()
            .is_none_or(|manifest| manifest.editor_rules.movement.sticky_column)
    }

    /// Resolve the effective caret style: runtime `clientSetCursorStyle`
    /// override -> per-mode manifest `caret_style` -> editor `StyleRegistry`
    /// default.
    pub fn effective_caret_style(&self) -> CaretStyle {
        if let Some(style) = self.caret_style_override {
            return style;
        }
        self.document
            .behavior_manifest
            .as_ref()
            .and_then(|manifest| manifest.editor_rules.caret_style)
            .unwrap_or(self.theme.caret_style)
    }

    /// Runtime caret style override set by `clientSetCursorStyle`.
    pub fn caret_style_override(&self) -> Option<CaretStyle> {
        self.caret_style_override
    }

    /// Set (or clear, with `None`) the runtime caret style override. Returns
    /// `true` when the style actually changed (caller should repaint); the
    /// blink clock resets either way so a fresh cycle starts visible.
    pub fn set_caret_style_override(&mut self, style: Option<CaretStyle>) -> bool {
        let changed = self.caret_style_override != style;
        self.caret_style_override = style;
        self.caret_blink.reset();
        changed
    }

    /// Advance the blink clock by `delta_ms` under the effective blink style.
    /// Returns true when the caret visibility changed (so the widget knows to
    /// repaint).
    pub fn advance_blink(&mut self, delta_ms: u64) -> bool {
        let style = self.effective_caret_style().blink;
        let before = self.caret_blink.is_visible();
        self.caret_blink.advance(&style, delta_ms);
        before != self.caret_blink.is_visible()
    }

    /// True when the caret should animate (effective blink style is not Solid).
    pub fn caret_animates(&self) -> bool {
        self.effective_caret_style().blink.animates()
    }

    /// Whether the caret is currently visible in its blink cycle.
    pub fn caret_blink_visible(&self) -> bool {
        self.caret_blink.is_visible()
    }

    /// Whether the caret at `selection_index` should paint this frame. The
    /// primary caret honours the blink cycle; secondary carets stay solid so
    /// every cursor remains visible while typing with multiple selections
    /// (Plan 071 task 8).
    fn caret_should_paint(&self, selection_index: usize) -> bool {
        if selection_index == self.selections.primary_index() {
            self.caret_blink.is_visible()
        } else {
            true
        }
    }

    fn move_or_extend(
        &mut self,
        extend: bool,
        movement: impl FnOnce(&mut CursorState, &EditorBuffer) -> bool,
    ) -> bool {
        if extend {
            self.extend_selection(movement)
        } else {
            self.move_cursor(movement)
        }
    }

    pub fn move_word_start(&mut self, forward: bool, long: bool, extend: bool) -> bool {
        let policy = self.movement_rules().word_separators.clone();
        let underscore = self.movement_rules().treat_underscore_as_word;
        self.move_or_extend(extend, |cursor, buffer| {
            if forward {
                cursor.move_to_next_word_start(buffer, &policy, underscore, long)
            } else {
                cursor.move_to_prev_word_start(buffer, &policy, underscore, long)
            }
        })
    }

    pub fn move_word_end(&mut self, forward: bool, long: bool, extend: bool) -> bool {
        let rules = self.movement_rules();
        let policy = rules.word_separators.clone();
        let underscore = rules.treat_underscore_as_word;
        let stop_at_eol = rules.stop_at_eol_word_end;
        self.move_or_extend(extend, |cursor, buffer| {
            if forward {
                cursor.move_to_next_word_end(buffer, &policy, underscore, long, stop_at_eol)
            } else {
                cursor.move_to_prev_word_end(buffer, &policy, underscore, long)
            }
        })
    }

    pub fn move_sub_word(&mut self, forward: bool, extend: bool) -> bool {
        let camel = self.movement_rules().camel_case_sub_word;
        self.move_or_extend(extend, |cursor, buffer| {
            if forward {
                cursor.move_to_next_sub_word_start(buffer, camel)
            } else {
                cursor.move_to_prev_sub_word_start(buffer, camel)
            }
        })
    }

    pub fn move_paragraph(&mut self, forward: bool, to_end: bool, extend: bool) -> bool {
        let style = self.movement_rules().paragraph_style;
        self.move_or_extend(extend, |cursor, buffer| {
            if to_end {
                cursor.move_to_paragraph_end(buffer, style)
            } else if forward {
                cursor.move_to_next_paragraph(buffer, style)
            } else {
                cursor.move_to_prev_paragraph(buffer, style)
            }
        })
    }

    pub fn move_first_non_blank(&mut self, extend: bool) -> bool {
        self.move_or_extend(extend, |cursor, buffer| {
            cursor.move_to_first_non_blank(buffer)
        })
    }

    pub fn move_last_non_blank(&mut self, extend: bool) -> bool {
        self.move_or_extend(extend, |cursor, buffer| {
            cursor.move_to_last_non_blank(buffer)
        })
    }

    /// Matching-pair motion: resolves the single-char open/close pair around the
    /// caret from the inert manifest `pairs`, then asks the buffer to jump to
    /// its match. `ponytail:` single-char distinct pairs only (quotes/multi-char
    /// pairs are skipped); bracket matching is the common case.
    pub fn move_matching_pair(&mut self, extend: bool) -> bool {
        let pairs: Vec<(char, char)> = self
            .document
            .behavior_manifest
            .as_ref()
            .into_iter()
            .flat_map(|manifest| manifest.editor_rules.pairs.iter())
            .filter_map(|pair| {
                let mut open_chars = pair.open.chars();
                let mut close_chars = pair.close.chars();
                let open = open_chars.next()?;
                let close = close_chars.next()?;
                if open_chars.next().is_some() || close_chars.next().is_some() || open == close {
                    return None;
                }
                Some((open, close))
            })
            .collect();
        let caret = self.caret();
        let candidate = self
            .buffer
            .char_at(caret)
            .or_else(|| self.buffer.char_before(caret));
        let Some(target) = candidate else {
            return false;
        };
        let Some((open, close)) = pairs
            .into_iter()
            .find(|(o, c)| *o == target || *c == target)
        else {
            return false;
        };
        self.move_or_extend(extend, move |cursor, buffer| {
            cursor.move_to_matching_pair(buffer, open, close)
        })
    }

    /// Select the word run containing the caret (code policy + underscore).
    /// No-op when the caret is on a separator/whitespace.
    /// `ponytail:` between-words caret no-ops (VSCode selects the next word);
    /// add when a `count`-aware select op needs it.
    pub fn select_word(&mut self) -> bool {
        let rules = self.movement_rules();
        let policy = rules.word_separators.clone();
        let underscore = rules.treat_underscore_as_word;
        let caret = self.caret();
        let Some((start, end)) = self.buffer.word_range_at(caret, &policy, underscore, false)
        else {
            return false;
        };
        self.set_selection_range(start, end)
    }

    /// Select the caret's line content (excludes the line terminator).
    pub fn select_line(&mut self) -> bool {
        let (start, end) = self.buffer.line_range(self.caret());
        self.set_selection_range(start, end)
    }

    /// Select the paragraph (maximal non-blank line run) containing the caret.
    pub fn select_paragraph(&mut self) -> bool {
        let style = self.movement_rules().paragraph_style;
        let (start, end) = self.buffer.paragraph_range(self.caret(), style);
        self.set_selection_range(start, end)
    }

    /// Establish a collapsed-or-expanded selection over `[start, end]` and move
    /// the caret to `end`. A degenerate range clears the selection.
    fn set_selection_range(&mut self, start: usize, end: usize) -> bool {
        let start = self.buffer.clamp_byte_offset(start);
        let end = self.buffer.clamp_byte_offset(end);
        let caret_was = self.caret();
        self.set_primary_focus(end);
        self.selections.primary_mut().set_anchor(start);
        let has_range = start != end;
        if !has_range {
            self.clear_selection();
        }
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        end != caret_was || has_range
    }

    // ------------------------------------------------------------------
    // Plan 071 task 9: multi-cursor commands. Every command operates on the
    // selection set generically; paint already iterates the set (task 8), so
    // no command carries its own paint path.
    // ------------------------------------------------------------------

    /// Snapshot the selection set for cursor-undo. Skips a snapshot identical
    /// to the newest one (e.g. a move that hit the document edge).
    fn snapshot_selection_set(&mut self) {
        if self.cursor_undo_stack.back() == Some(&self.selections) {
            return;
        }
        self.cursor_undo_stack.push_back(self.selections.clone());
        while self.cursor_undo_stack.len() > CURSOR_UNDO_MAX_DEPTH {
            self.cursor_undo_stack.pop_front();
        }
    }

    /// Restore the previous selection set (Ctrl+U / cursorUndo). Cursor
    /// movements only; edits have their own undo history.
    fn undo_cursor_move(&mut self) -> EditorCommandOutcome {
        let Some(mut snapshot) = self.cursor_undo_stack.pop_back() else {
            return EditorCommandOutcome::unchanged();
        };
        // Snapshots may predate edits; clamp every caret/anchor back in range.
        snapshot.clamp_to(&self.buffer);
        self.selections = snapshot;
        self.snippet_session = None;
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        EditorCommandOutcome::from_changed(true)
    }

    /// Add a collapsed caret one line below/above the primary caret at the
    /// same scalar column (add-cursor-below/above and column-select-down/up
    /// share this primitive). Refuses to stack two carets on one line.
    fn add_cursor_line(&mut self, below: bool) -> bool {
        let focus = self.selections.primary_focus();
        let line = self.buffer.line_of_byte(focus);
        let target_line = if below {
            line.checked_add(1)
        } else {
            line.checked_sub(1)
        };
        let Some(target_line) = target_line else {
            return false;
        };
        if target_line >= self.buffer.line_len() {
            return false;
        }
        if self
            .selections
            .selections()
            .iter()
            .any(|selection| self.buffer.line_of_byte(selection.focus()) == target_line)
        {
            return false;
        }
        let column = self.buffer.scalar_column_of_byte(focus);
        let offset = self.buffer.byte_for_line_scalar_column(target_line, column);
        self.selections
            .push_and_make_primary(Selection::collapsed(offset));
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        true
    }

    /// Move every caret one scalar left/right, collapsing each selection
    /// (column-select-left/right: "left/right moves all carets").
    fn move_all_carets(&mut self, right: bool) -> bool {
        let mut changed = false;
        for index in 0..self.selections.selection_count() {
            let focus = self.selections.selection_mut(index).focus();
            let target = if right {
                self.buffer.next_scalar_boundary(focus)
            } else {
                self.buffer.previous_scalar_boundary(focus)
            };
            if let Some(target) = target {
                let selection = self.selections.selection_mut(index);
                selection.set_focus(target);
                selection.set_anchor(target);
                changed = true;
            }
        }
        if changed {
            self.ensure_caret_line_visible();
            self.follow_visual_end = false;
        }
        changed
    }

    /// The current match needle: the primary selection's text when expanded,
    /// otherwise the word at the primary caret (returned range is `Some` only
    /// in the word case, marking "first press: select the word").
    fn match_needle(&self) -> Option<(String, Option<Range<usize>>)> {
        if let Some(range) = self.selections.primary_range() {
            let text = self.buffer.text_range(range);
            return (!text.is_empty()).then_some((text, None));
        }
        let rules = self.movement_rules();
        let (start, end) = self.buffer.word_range_at(
            self.selections.primary_focus(),
            &rules.word_separators,
            rules.treat_underscore_as_word,
            false,
        )?;
        Some((self.buffer.text_range(start..end), Some(start..end)))
    }

    /// Add the next (forward) or previous (backward) occurrence of the needle
    /// as a new primary selection. First press on a collapsed caret selects
    /// the word itself. Searches wrap once around the document; returns false
    /// when every occurrence is already selected (bounded: one scan).
    fn select_next_match(&mut self, forward: bool) -> bool {
        let Some((needle, word_range)) = self.match_needle() else {
            return false;
        };
        if let Some(range) = word_range {
            return self.set_selection_range(range.start, range.end);
        }
        let doc = self.buffer.text_range(0..self.buffer.document_end_byte());
        let occurrences: Vec<Range<usize>> = doc
            .match_indices(needle.as_str())
            .map(|(start, matched)| start..start + matched.len())
            .collect();
        if occurrences.is_empty() {
            return false;
        }
        let already: Vec<Range<usize>> = self
            .selections
            .selections()
            .iter()
            .map(|selection| selection.normalized_range())
            .filter(|range| range.start < range.end)
            .collect();
        let unselected = |range: &&Range<usize>| !already.contains(range);
        let focus = self.selections.primary_focus();
        let next = if forward {
            occurrences
                .iter()
                .find(|range| range.start >= focus && unselected(range))
                .or_else(|| occurrences.iter().find(unselected))
        } else {
            occurrences
                .iter()
                .rev()
                .find(|range| range.end <= focus && unselected(range))
                .or_else(|| occurrences.iter().rev().find(unselected))
        };
        let Some(range) = next else {
            return false;
        };
        self.selections
            .push_and_make_primary(Selection::new(range.start, range.end));
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        true
    }

    /// Replace the selection set with every occurrence of the needle; the
    /// occurrence containing the original caret becomes primary.
    fn select_all_matches(&mut self) -> bool {
        let Some((needle, word_range)) = self.match_needle() else {
            return false;
        };
        let doc = self.buffer.text_range(0..self.buffer.document_end_byte());
        let occurrences: Vec<Range<usize>> = doc
            .match_indices(needle.as_str())
            .map(|(start, matched)| start..start + matched.len())
            .collect();
        if occurrences.is_empty() {
            return false;
        }
        let original_focus = self.selections.primary_focus();
        let primary = match word_range {
            Some(range) => occurrences
                .iter()
                .position(|occurrence| *occurrence == range),
            None => occurrences
                .iter()
                .position(|occurrence| occurrence.contains(&original_focus)),
        }
        .unwrap_or(0);
        let selections: Vec<Selection> = occurrences
            .iter()
            .map(|range| Selection::new(range.start, range.end))
            .collect();
        if !self.selections.set_selections(selections, primary) {
            return false;
        }
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        true
    }

    /// Escape: collapse the set to the primary caret.
    fn cancel_multiple_selections(&mut self) -> bool {
        let changed = self.selections.selection_count() > 1 || self.has_selection();
        self.selections.keep_only_primary();
        self.selections.collapse_primary();
        self.snippet_session = None;
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        changed
    }

    /// Helix keep_primary_selection: keep the primary (range intact), drop rest.
    fn keep_selection(&mut self) -> bool {
        if self.selections.selection_count() <= 1 {
            return false;
        }
        self.selections.keep_only_primary();
        self.ensure_caret_line_visible();
        true
    }

    /// Helix remove_primary_selection: drop the primary, keep the rest.
    fn remove_selection(&mut self) -> bool {
        if self.selections.selection_count() <= 1 {
            return false;
        }
        self.selections.remove_primary();
        self.ensure_caret_line_visible();
        true
    }

    /// Multi-cursor edit: one operation per caret, applied right-to-left so
    /// byte offsets stay valid, recorded as ONE undo step and emitted as one
    /// edit event per caret (the connection layer stamps ascending optimistic
    /// base versions, so the server applies them in order).
    ///
    /// `per_caret` maps (surface, caret focus, normalized range) to the
    /// operation + final caret for that selection. Returning `None` skips the
    /// caret (e.g. backspace at document start).
    ///
    /// `ponytail:` overlapping selections are applied as-is (callers build
    /// non-overlapping sets); snippet sessions are single-caret and are
    /// dropped by a multi edit.
    fn multi_caret_edit<F>(&mut self, per_caret: F) -> EditorCommandOutcome
    where
        F: Fn(&EditorSurface, usize, Range<usize>) -> Option<(EditOperation, usize)>,
    {
        let plans: Vec<(usize, Range<usize>)> = self
            .selections
            .selections()
            .iter()
            .map(|selection| (selection.focus(), selection.normalized_range()))
            .collect();
        let mut ops: Vec<(EditOperation, usize)> = Vec::with_capacity(plans.len());
        for (focus, range) in plans {
            if let Some(op) = per_caret(self, focus, range) {
                ops.push(op);
            }
        }
        if ops.is_empty() {
            return EditorCommandOutcome::unchanged();
        }

        let primary_index = self.selections.primary_index();
        let set_before: Vec<HistorySelection> = self
            .selections
            .selections()
            .iter()
            .map(|selection| HistorySelection {
                caret: selection.focus(),
                anchor: (!selection.is_collapsed()).then(|| selection.anchor()),
            })
            .collect();

        // Apply right-to-left.
        let mut order: Vec<usize> = (0..ops.len()).collect();
        order.sort_by(|&a, &b| op_start_offset(&ops[b].0).cmp(&op_start_offset(&ops[a].0)));

        let mut forward_ops: Vec<EditOperation> = Vec::with_capacity(ops.len());
        let mut inverse_ops: Vec<EditOperation> = Vec::with_capacity(ops.len());
        let mut final_carets = vec![0usize; ops.len()];
        let mut decorations_changed = false;
        for index in order {
            let (operation, final_caret) = &ops[index];
            let prior_text = self.prior_text_for_operation(operation);
            inverse_ops.push(invert_edit_operation(operation, &prior_text));
            forward_ops.push(operation.clone());
            self.apply_buffer_operation(operation);
            if self.decorations.apply_edit(operation) {
                decorations_changed = true;
            }
            final_carets[index] = *final_caret;
        }

        let new_selections: Vec<Selection> = final_carets
            .iter()
            .map(|caret| Selection::collapsed(self.buffer.clamp_byte_offset(*caret)))
            .collect();
        let set_after: Vec<HistorySelection> = new_selections
            .iter()
            .map(|selection| HistorySelection {
                caret: selection.focus(),
                anchor: None,
            })
            .collect();
        let _ = self
            .selections
            .set_selections(new_selections, primary_index);

        self.snippet_session = None;
        if decorations_changed {
            self.bump_layout_style_revision();
        }
        self.history.record(HistoryEntry {
            forward: forward_ops[0].clone(),
            inverse: inverse_ops[0].clone(),
            selection_before: set_before[primary_index.min(set_before.len() - 1)],
            selection_after: HistorySelection::collapsed(
                final_carets[primary_index.min(final_carets.len() - 1)],
            ),
            forward_ops: forward_ops.clone(),
            inverse_ops,
            selection_set_before: set_before,
            selection_set_after: set_after,
            primary_index,
        });
        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        self.perf.record_counter("editor.input.local_edit", 1);
        let events: Vec<EditorEditEvent> = forward_ops
            .into_iter()
            .filter_map(|operation| self.client_first_event(operation))
            .collect();
        EditorCommandOutcome::changed_multi(events)
    }

    pub fn visible_text(&self) -> String {
        self.visible_snapshot().text
    }

    /// Bounded text window exposed to native accessibility consumers.
    ///
    /// Accessibility follows the same viewport/fold window as the editor's
    /// visible text instead of forcing a full-document layout or IPC round
    /// trip from the accessibility pass.
    pub(crate) fn accessibility_text(&self) -> String {
        self.visible_snapshot().text
    }

    pub(crate) fn accessibility_selection(&self, text_run_id: NodeId) -> Option<TextSelection> {
        if !self.folds.collapsed.is_empty() {
            return None;
        }
        let snapshot = self.visible_snapshot();
        let start = snapshot.start_byte_offset;
        let to_character_index = |offset: usize| {
            let relative = offset.checked_sub(start)?;
            (relative <= snapshot.text.len() && snapshot.text.is_char_boundary(relative))
                .then(|| snapshot.text[..relative].chars().count())
        };
        let anchor = to_character_index(self.selections.primary_anchor())?;
        let focus = to_character_index(self.selections.primary_focus())?;
        Some(TextSelection {
            anchor: TextPosition {
                node: text_run_id,
                character_index: anchor,
            },
            focus: TextPosition {
                node: text_run_id,
                character_index: focus,
            },
        })
    }

    pub(crate) fn set_accessibility_selection(
        &mut self,
        text_run_id: NodeId,
        selection: &TextSelection,
    ) -> bool {
        if !self.folds.collapsed.is_empty()
            || selection.anchor.node != text_run_id
            || selection.focus.node != text_run_id
        {
            return false;
        }
        let snapshot = self.visible_snapshot();
        let start = snapshot.start_byte_offset;
        let byte_offset = |character_index: usize| {
            start.saturating_add(
                snapshot
                    .text
                    .char_indices()
                    .nth(character_index)
                    .map_or(snapshot.text.len(), |(offset, _)| offset),
            )
        };
        let anchor = byte_offset(selection.anchor.character_index);
        let focus = byte_offset(selection.focus.character_index);
        let changed =
            self.selections.primary_anchor() != anchor || self.selections.primary_focus() != focus;
        self.selections.primary_mut().set_anchor(anchor);
        self.selections.primary_mut().set_focus(focus);
        self.ensure_caret_line_visible();
        changed
    }

    pub(crate) fn replace_accessibility_text(&mut self, value: &str) -> EditorCommandOutcome {
        let _ = self.cancel_composition();
        self.paste_text_with_event(value)
    }

    pub(crate) fn replace_accessibility_value(&mut self, value: &str) -> EditorCommandOutcome {
        let end = self.buffer.document_end_byte();
        self.selections.primary_mut().set_anchor(0);
        self.selections.primary_mut().set_focus(end);
        self.replace_accessibility_text(value)
    }

    pub(crate) fn visible_byte_range(&self) -> std::ops::Range<u64> {
        let snapshot = self.visible_snapshot();
        let start = snapshot.start_byte_offset as u64;
        start..start.saturating_add(snapshot.text.len() as u64)
    }

    pub fn with_perf_recorder(mut self, perf: PerfRecorder) -> Self {
        self.perf = perf;
        self
    }

    pub fn perf_snapshots(&self) -> Vec<crate::perf::metrics::MetricSnapshot> {
        self.perf.snapshots()
    }

    pub fn hit_test_document_offset(&self, point: Point) -> Option<usize> {
        let snapshot = self.visible_snapshot();
        if snapshot.text.is_empty() {
            return Some(snapshot.start_byte_offset);
        }

        let layout_x = (point.x - self.inset_x() + self.visual_scroll_x) as f32;
        let layout_y = (point.y - self.inset_y() + self.visual_scroll_y) as f32;
        let visible_offset = self
            .layout
            .hit_test_visible_byte_offset(layout_x, layout_y)?
            .min(snapshot.text.len());
        Some(
            self.buffer
                .clamp_byte_offset(snapshot.start_byte_offset + visible_offset),
        )
    }

    pub fn caret_geometry(&self, width: f32) -> Option<Rect> {
        let snapshot = self.visible_snapshot();
        self.caret_geometry_from_visible_snapshot(&snapshot, width)
    }

    /// Candidate-window IME area in editor-local coordinates.
    ///
    /// Uses caret geometry and widens by an estimate of active preedit width so
    /// platform candidate UI stays near the composition.
    pub fn ime_cursor_area(&self, editor_width: f64, editor_height: f64) -> Rect {
        if let Some(caret) = self.caret_geometry(CARET_WIDTH as f32) {
            let mut area = caret;
            if let Some(preedit) = self.preedit_text() {
                let estimated = (preedit.chars().count() as f64) * (caret.height().max(8.0) * 0.55);
                area.x1 = (area.x0 + estimated.max(area.width())).min(editor_width);
            }
            // Keep the area inside the editor panel.
            area.x0 = area.x0.clamp(0.0, editor_width);
            area.x1 = area.x1.clamp(area.x0, editor_width);
            area.y0 = area.y0.clamp(0.0, editor_height);
            area.y1 = area.y1.clamp(area.y0, editor_height);
            if area.width() < 1.0 || area.height() < 1.0 {
                return Rect::new(0.0, 0.0, editor_width.max(1.0), editor_height.max(1.0));
            }
            return area;
        }
        Rect::new(0.0, 0.0, editor_width.max(1.0), editor_height.max(1.0))
    }

    pub fn place_caret_at_point(&mut self, point: Point) -> bool {
        let Some(caret) = self.hit_test_document_offset(point) else {
            return false;
        };

        let previous = self.caret();
        let had_selection = self.has_selection();
        self.snippet_session = None;
        self.set_primary_focus(caret);
        self.clear_selection();
        self.follow_visual_end = false;
        self.ensure_caret_line_visible();
        had_selection || previous != self.caret()
    }

    pub fn extend_selection_to_point(&mut self, point: Point) -> bool {
        let Some(focus) = self.hit_test_document_offset(point) else {
            return false;
        };

        let previous_caret = self.caret();
        let previous_anchor = self.selections.primary_anchor();
        self.snippet_session = None;
        self.set_primary_focus(focus);
        self.selections.clamp_primary_anchor(&self.buffer);
        let now_anchor = self.selections.primary_anchor();
        let now_caret = self.caret();
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;

        previous_caret != now_caret || previous_anchor != now_anchor
    }

    pub fn scroll_lines(&mut self, delta_lines: isize) -> bool {
        self.pin_caret_visible = false;
        let changed = self
            .viewport
            .scroll_lines(delta_lines, self.buffer.line_len());
        if changed {
            // Line/page deltas snap to whole lines; drop the sub-line visual
            // offset so the next paint aligns to the new first visible line.
            self.visual_scroll_y = 0.0;
            self.follow_visual_end = false;
        }
        changed
    }

    pub fn scroll_vertical_pixels(&mut self, delta_pixels: f64) -> bool {
        self.pin_caret_visible = false;
        let line_height = self.conservative_document_line_height();
        let document_lines = self.buffer.line_len();
        let previous_line = self.viewport.first_visible_line();
        let previous_visual = self.visual_scroll_y;

        // Accumulate into the sub-line visual offset and advance the logical
        // first visible line by one each time a full line_height is crossed.
        // Advancing and subtracting (rather than exhausting an overscan budget
        // and resetting to zero) keeps pixel scrolling continuous with no
        // backward jump when the visual budget is exhausted.
        self.visual_scroll_y += delta_pixels;
        while self.visual_scroll_y >= line_height {
            if !self.viewport.scroll_lines(1, document_lines) {
                let budget = self.last_visual_max_scroll_y.max(0.0);
                self.visual_scroll_y = self.visual_scroll_y.min(budget);
                break;
            }
            self.visual_scroll_y -= line_height;
        }
        while self.visual_scroll_y < 0.0 {
            if !self.viewport.scroll_lines(-1, document_lines) {
                self.visual_scroll_y = 0.0;
                break;
            }
            self.visual_scroll_y += line_height;
        }
        // Keep the visual offset within one line while logical lines can
        // still advance; once the first visible line reaches its maximum (or
        // the document fits on one page), wrapped lines can make the visible
        // window taller than the viewport, so allow the full visual budget.
        let max_first = document_lines.saturating_sub(self.viewport.visible_line_count());
        let viewport_at_end = self.viewport.first_visible_line() >= max_first;
        let visual_cap = if max_first > 0 && !viewport_at_end {
            line_height.min(self.last_visual_max_scroll_y.max(0.0))
        } else {
            self.last_visual_max_scroll_y.max(0.0)
        };
        self.visual_scroll_y = self.visual_scroll_y.clamp(0.0, visual_cap);
        self.follow_visual_end = false;
        previous_line != self.viewport.first_visible_line()
            || previous_visual != self.visual_scroll_y
    }

    pub fn scroll_horizontal_pixels(&mut self, delta_pixels: f64) -> bool {
        if matches!(
            self.resolved_wrap(),
            WrapPolicy::Viewport | WrapPolicy::Column(_)
        ) {
            return false;
        }
        self.pin_caret_visible = false;
        let previous = self.visual_scroll_x;
        let cap = self.last_visual_max_scroll_x.max(0.0);
        self.visual_scroll_x = (self.visual_scroll_x + delta_pixels).clamp(0.0, cap);
        previous != self.visual_scroll_x
    }

    pub fn update_visible_line_count_for_height(&mut self, height: f64) -> bool {
        let available_height = (height - (self.inset_y() * 2.0)).max(0.0);
        let line_height = self.conservative_document_line_height();
        let visible_line_count = visible_line_count_from_height(available_height, line_height);
        let overscan = match self.resolved_wrap() {
            WrapPolicy::None => 4,
            WrapPolicy::Viewport | WrapPolicy::Column(_) => 12,
        };
        let overscan_changed = self.viewport.set_overscan_lines(overscan);
        let count_changed = self
            .viewport
            .set_visible_line_count(visible_line_count, self.buffer.line_len());
        overscan_changed || count_changed
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut masonry::vello::Scene) {
        self.paint_in_rect(ctx, scene, ctx.size().to_rect());
    }

    pub(crate) fn paint_in_rect(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        rect: Rect,
    ) {
        let width = rect.width();
        let height = rect.height();
        self.update_visible_line_count_for_height(height);

        scene.push_clip_layer(Affine::IDENTITY, &rect);
        // Paint the editor background across the full editor rect. There is no
        // visible inset card or decorative accent circle; a small text inset
        // keeps text from hugging the edges. Color comes from the single source
        // of color (super::theme), never an inline literal.
        let panel_bg = self.theme.base.panel_bg;
        scene.fill(Fill::NonZero, Affine::IDENTITY, panel_bg, None, &rect);

        let max_width = self.layout_max_width(width);
        let available_width = (width - (self.inset_x() * 2.0)).max(1.0);
        let available_height = (height - (self.inset_y() * 2.0)).max(0.0);
        let focused = ctx.is_focus_target();
        self.paint_text(
            ctx,
            scene,
            max_width,
            available_width,
            available_height,
            focused,
            (rect.x0, rect.y0),
        );
        self.paint_vertical_scrollbar(scene, rect, available_height);
        scene.pop_layer();
    }

    /// Compute the vertical scrollbar thumb rect for the editor `rect`, or
    /// `None` when the content fits (no scrollable overflow). Shared between
    /// paint and tests so the thumb position is deterministic and never depends
    /// on rendered pixels.
    pub(crate) fn scrollbar_thumb_rect(&self, rect: Rect) -> Option<Rect> {
        let line_height = self.conservative_document_line_height();
        let document_lines = self.buffer.line_len();
        let visible = self.viewport.visible_line_count();
        let max_first = document_lines.saturating_sub(visible);
        let available_height = (rect.height() - (self.inset_y() * 2.0)).max(0.0);
        let track_y0 = rect.y0 + self.inset_y();
        let track_y1 = rect.y0 + self.inset_y() + available_height;
        let track_height = (track_y1 - track_y0).max(0.0);

        // The thumb tracks total document progress: logical lines plus the
        // sub-line visual offset. For single-page content taller than the
        // viewport (e.g. one heavily wrapped line, or a test-faked budget),
        // fall back to the visual-only progress against `last_visual_max_scroll_y`.
        let (total_scrollable, scrolled, content) = if max_first > 0 {
            let total = max_first as f64 * line_height;
            let s = self.viewport.first_visible_line() as f64 * line_height
                + self.visual_scroll_y.clamp(0.0, line_height);
            (total, s, total + available_height)
        } else if self.last_visual_max_scroll_y > 0.0 {
            (
                self.last_visual_max_scroll_y,
                self.visual_scroll_y,
                self.last_visual_max_scroll_y + available_height,
            )
        } else {
            return None;
        };
        let frac = if total_scrollable > 0.0 {
            (scrolled / total_scrollable).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let ratio = if content > 0.0 {
            (available_height / content).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_height = (ratio * track_height).max(SCROLLBAR_MIN_THUMB.min(track_height));
        let scrollable_track = (track_height - thumb_height).max(0.0);
        let thumb_y0 = track_y0 + frac * scrollable_track;
        let x1 = rect.x1 - SCROLLBAR_MARGIN;
        let x0 = x1 - SCROLLBAR_WIDTH;
        Some(Rect::new(x0, thumb_y0, x1, thumb_y0 + thumb_height))
    }

    fn paint_vertical_scrollbar(
        &mut self,
        scene: &mut masonry::vello::Scene,
        rect: Rect,
        available_height: f64,
    ) {
        let track_y0 = rect.y0 + self.inset_y();
        let track_y1 = rect.y0 + self.inset_y() + available_height;
        let x1 = rect.x1 - SCROLLBAR_MARGIN;
        let x0 = x1 - SCROLLBAR_WIDTH;
        let track = Rect::new(x0, track_y0, x1, track_y1);

        // Route scrollbar chrome through primitive (Phase 20.2); thread the
        // real InteractionState from pointer/press state (Phase 20.4 task 5)
        // so the thumb is dim at rest and full on hover/active.
        let thumb = self.scrollbar_thumb_rect(rect).unwrap_or(Rect::ZERO);
        let state = self.scrollbar_interaction_state(rect, available_height);
        crate::shell::primitives::paint_scroll_chrome(scene, track, thumb, state, &self.ui_theme);
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "paint pass keeps geometry explicit instead of a per-frame heap context"
    )]
    fn paint_text(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        max_width: f32,
        available_width: f64,
        available_height: f64,
        focused: bool,
        origin: (f64, f64),
    ) {
        let snapshot = self.visible_snapshot();
        let current_text = snapshot.text.as_str();
        let (display_text, color) = if current_text.is_empty() {
            (PLACEHOLDER_TEXT, self.theme.base.placeholder)
        } else {
            (current_text, self.theme.base.text)
        };

        let caret_visible_offset = self.visible_caret_offset(&snapshot);
        let selection_visible_ranges = self.visible_selection_ranges(&snapshot);
        let diagnostic_visible_ranges = self.visible_diagnostic_ranges(&snapshot);
        let document_font_role = self.document_font_role();
        let key = LayoutCacheKey::new(self.buffer.revision(), self.viewport.revision(), max_width)
            .with_presentation(
                self.typography.revision(),
                self.layout_style_revision,
                document_font_role,
            )
            .with_ligatures(self.typography.profile(document_font_role).feature_hash())
            .with_fold_revision(self.folds.revision);
        let decorations = &self.decorations;
        let document = &self.document;
        let document_end = self.buffer.document_end_byte();
        let theme = self.theme;
        let pin_caret_visible = std::mem::take(&mut self.pin_caret_visible);
        let chrome = self.resolved_chrome();
        let active_offsets = if chrome.active_line {
            self.visible_caret_offsets(&snapshot)
        } else {
            Vec::new()
        };
        let bracket_ranges = self.visible_bracket_ranges(&snapshot);
        let indent_tab = self.indent_tab_width();
        let frame = TextFrame {
            inset_x: self.inset_x(),
            inset_y: self.inset_y(),
            scroll_x: self.visual_scroll_x,
            clip_width: available_width,
        };
        let metrics = self.layout.paint_text(
            ctx,
            scene,
            display_text,
            color,
            max_width,
            &mut self.visual_scroll_y,
            self.follow_visual_end && !current_text.is_empty(),
            available_height,
            key,
            caret_visible_offset,
            &selection_visible_ranges,
            self.theme.base.selection,
            &diagnostic_visible_ranges,
            origin,
            pin_caret_visible,
            &self.typography,
            document_font_role,
            frame,
            TextChromeLayers {
                active_line_offsets: &active_offsets,
                active_line_color: self.theme.line_highlight,
                bracket_ranges: &bracket_ranges,
                bracket_color: self.theme.bracket_match,
                indent_tab,
                indent_color: self.theme.indent_guide,
            },
            || {
                normalize_visible_text_style_runs(
                    decorations,
                    document,
                    document_end,
                    &snapshot,
                    document_font_role,
                    theme,
                )
            },
        );
        if current_text.is_empty() {
            self.visual_scroll_y = 0.0;
            self.visual_scroll_x = 0.0;
            self.last_visual_max_scroll_y = 0.0;
            self.last_visual_max_scroll_x = 0.0;
        } else {
            self.last_visual_max_scroll_y = metrics.max_scroll_y(available_height);
            self.last_visual_max_scroll_x = metrics.max_scroll_x(available_width);
            self.visual_scroll_x = self
                .visual_scroll_x
                .clamp(0.0, self.last_visual_max_scroll_x);
        }
        self.paint_gutter(ctx, scene, &snapshot, available_height, origin);
        self.paint_inlay_overlays(ctx, scene, &snapshot, available_height, origin);
        if focused && !self.composition.is_active() {
            self.paint_caret(
                scene,
                available_width as f32,
                available_height,
                &snapshot,
                origin,
            );
        }
        if focused && self.composition.is_active() {
            self.paint_preedit_overlay(
                ctx,
                scene,
                available_width as f32,
                available_height,
                caret_visible_offset,
                origin,
            );
        }
        self.follow_visual_end = false;
    }

    fn paint_inlay_overlays(
        &self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        snapshot: &VisibleSnapshot,
        available_height: f64,
        origin: (f64, f64),
    ) {
        if !self.inlay_hints_visible() || snapshot.text.is_empty() {
            return;
        }
        let visible_start = snapshot.start_byte_offset as u64;
        let visible_end = visible_start + snapshot.text.len() as u64;
        let color = self.theme.base.placeholder;
        let document_font_role = self.document_font_role();
        let profile = self.typography.profile(document_font_role);
        let font_size = (profile.size() * 0.85).max(8.0);
        for span in self.decorations.visible_spans(visible_start, visible_end) {
            let Some(inlay) = span.inlay.as_ref() else {
                continue;
            };
            let Ok(byte) = usize::try_from(span.byte_start) else {
                continue;
            };
            let Some(visible_offset) = self.visible_byte_offset(byte, snapshot) else {
                continue;
            };
            let Some(geometry) = self
                .layout
                .caret_geometry_for_visible_byte_offset(visible_offset, CARET_WIDTH as f32)
            else {
                continue;
            };
            let (font_context, layout_context) = ctx.text_contexts();
            let mut builder = layout_context.ranged_builder(font_context, &inlay.label, 1.0, true);
            builder.push_default(StyleProperty::FontStack(profile.font_stack()));
            builder.push_default(StyleProperty::FontSize(font_size));
            builder.push_default(StyleProperty::Brush(BrushIndex(0)));
            let mut layout = builder.build(&inlay.label);
            layout.break_all_lines(None);
            let width = layout.full_width() as f64;
            let mut x = origin.0 + geometry.rect.x0 + self.inset_x() - self.visual_scroll_x;
            if matches!(inlay.placement, crate::protocol::InlayPlacement::Before) {
                x -= width;
            }
            let y = origin.1 + geometry.rect.y0 + self.inset_y() - self.visual_scroll_y;
            let clip = Rect::new(
                origin.0 + self.inset_x(),
                origin.1 + self.inset_y(),
                origin.0 + self.inset_x() + 4096.0,
                origin.1 + self.inset_y() + available_height,
            );
            scene.push_clip_layer(Affine::IDENTITY, &clip);
            render_text(
                scene,
                Affine::translate((x, y)),
                &layout,
                &[color.into()],
                true,
            );
            scene.pop_layer();
        }
    }

    fn paint_caret(
        &self,
        scene: &mut masonry::vello::Scene,
        max_width: f32,
        available_height: f64,
        snapshot: &VisibleSnapshot,
        origin: (f64, f64),
    ) {
        let style = self.effective_caret_style();
        let color = self.theme.base.caret;
        let scroll = self.visual_scroll_y;

        let clip = Rect::new(
            origin.0 + self.inset_x() - self.visual_scroll_x,
            origin.1 + self.inset_y(),
            origin.0 + self.inset_x() - self.visual_scroll_x + max_width as f64,
            origin.1 + self.inset_y() + available_height,
        );
        scene.push_clip_layer(Affine::IDENTITY, &clip);
        for (index, selection) in self.selections.selections().iter().enumerate() {
            if !self.caret_should_paint(index) {
                continue;
            }
            let Some(visible_offset) = self.visible_byte_offset(selection.focus(), snapshot) else {
                continue;
            };
            let Some(cell) = self
                .layout
                .caret_cell_for_visible_byte_offset(visible_offset)
            else {
                continue;
            };

            let line_height = (cell.line_bottom - cell.line_top).max(1.0);
            let line_bottom_abs = origin.1 + cell.line_bottom + self.inset_y() - scroll;
            let drawn_height = line_height * f64::from(style.height_pct.clamp(0.1, 1.0));
            let centre_y =
                origin.1 + (cell.line_top + cell.line_bottom) / 2.0 + self.inset_y() - scroll;
            let top = centre_y - drawn_height / 2.0;
            let bottom = centre_y + drawn_height / 2.0;
            let left = origin.0 + cell.x + self.inset_x() - self.visual_scroll_x;
            let stroke_width = f64::from(style.width_px).max(0.5);
            let cell_width = cell.advance.max(stroke_width);

            match style.shape {
                CaretShape::Bar | CaretShape::Line => {
                    let caret = Rect::new(left, top, left + stroke_width, bottom);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &caret);
                }
                CaretShape::Block => {
                    let block = Rect::new(left, top, left + cell_width, bottom);
                    if style.hollow {
                        scene.stroke(
                            &Stroke::new(stroke_width),
                            Affine::IDENTITY,
                            color,
                            None,
                            &block,
                        );
                    } else {
                        scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &block);
                    }
                }
                CaretShape::Underline => {
                    let underline = Rect::new(
                        left,
                        line_bottom_abs - stroke_width,
                        left + cell_width,
                        line_bottom_abs,
                    );
                    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &underline);
                }
            }
        }
        scene.pop_layer();
    }

    fn paint_preedit_overlay(
        &self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        max_width: f32,
        available_height: f64,
        caret_visible_offset: Option<usize>,
        origin: (f64, f64),
    ) {
        let Some(preedit) = self.preedit_text() else {
            return;
        };
        let Some(visible_offset) = caret_visible_offset else {
            return;
        };
        let Some(geometry) = self
            .layout
            .caret_geometry_for_visible_byte_offset(visible_offset, CARET_WIDTH as f32)
        else {
            return;
        };

        let insert_x = origin.0 + geometry.rect.x0 + self.inset_x() - self.visual_scroll_x;
        let insert_y = origin.1 + geometry.rect.y0 + self.inset_y() - self.visual_scroll_y;
        let line_height = (geometry.rect.y1 - geometry.rect.y0).max(1.0);

        let document_font_role = self.document_font_role();
        let profile = self.typography.profile(document_font_role);
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, preedit, 1.0, true);
        builder.push_default(StyleProperty::FontStack(profile.font_stack()));
        builder.push_default(StyleProperty::FontSize(profile.size()));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        builder.push_default(StyleProperty::Underline(true));
        let mut layout = builder.build(preedit);
        layout.break_all_lines(Some(max_width));

        let clip = Rect::new(
            origin.0 + self.inset_x() - self.visual_scroll_x,
            origin.1 + self.inset_y(),
            origin.0 + self.inset_x() - self.visual_scroll_x + max_width as f64,
            origin.1 + self.inset_y() + available_height,
        );
        scene.push_clip_layer(Affine::IDENTITY, &clip);

        let preedit_width = layout.full_width() as f64;
        if let Some((begin, end)) = self.preedit_cursor_span()
            && begin < end
            && !preedit.is_empty()
            && end <= preedit.len()
        {
            let frac_start = begin as f64 / preedit.len() as f64;
            let frac_end = end as f64 / preedit.len() as f64;
            let span = Rect::new(
                insert_x + preedit_width * frac_start,
                insert_y,
                insert_x + preedit_width * frac_end.max(frac_start),
                insert_y + line_height,
            );
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                self.theme.base.selection,
                None,
                &span,
            );
        }

        render_text(
            scene,
            Affine::translate((insert_x, insert_y)),
            &layout,
            &[self.theme.base.text.into()],
            true,
        );

        let caret_frac = match self.preedit_cursor_span() {
            Some((begin, _)) if !preedit.is_empty() => begin as f64 / preedit.len() as f64,
            _ => 1.0,
        };
        let caret_x = insert_x + preedit_width * caret_frac;
        // Shape-consistent IME caret: honour the active caret shape/width. The
        // preedit caret is always visible (composition is active) and uses a
        // line-height-derived cell width because the preedit glyphs are laid
        // out separately from the document layout.
        let preedit_style = self.effective_caret_style();
        let preedit_stroke = f64::from(preedit_style.width_px).max(0.5);
        let preedit_cell = (line_height * 0.6).max(preedit_stroke);
        let caret_color = self.theme.base.caret;
        match preedit_style.shape {
            CaretShape::Underline => {
                let underline = Rect::new(
                    caret_x,
                    insert_y + line_height - preedit_stroke,
                    caret_x + preedit_cell,
                    insert_y + line_height,
                );
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    caret_color,
                    None,
                    &underline,
                );
            }
            CaretShape::Block => {
                let block = Rect::new(
                    caret_x,
                    insert_y,
                    caret_x + preedit_cell,
                    insert_y + line_height,
                );
                if preedit_style.hollow {
                    scene.stroke(
                        &Stroke::new(preedit_stroke),
                        Affine::IDENTITY,
                        caret_color,
                        None,
                        &block,
                    );
                } else {
                    scene.fill(Fill::NonZero, Affine::IDENTITY, caret_color, None, &block);
                }
            }
            CaretShape::Bar | CaretShape::Line => {
                let caret = Rect::new(
                    caret_x,
                    insert_y,
                    caret_x + preedit_stroke,
                    insert_y + line_height,
                );
                scene.fill(Fill::NonZero, Affine::IDENTITY, caret_color, None, &caret);
            }
        }
        scene.pop_layer();
    }

    fn caret_geometry_from_visible_snapshot(
        &self,
        snapshot: &VisibleSnapshot,
        width: f32,
    ) -> Option<Rect> {
        let caret = self.caret();
        let visible_end = snapshot.start_byte_offset + snapshot.text.len();
        if caret < snapshot.start_byte_offset || caret > visible_end {
            return None;
        }

        let visible_offset = caret - snapshot.start_byte_offset;
        let geometry = self
            .layout
            .caret_geometry_for_visible_byte_offset(visible_offset, width)?;
        Some(Rect::new(
            geometry.rect.x0 + self.inset_x() - self.visual_scroll_x,
            geometry.rect.y0 + self.inset_y() - self.visual_scroll_y,
            geometry.rect.x1 + self.inset_x() - self.visual_scroll_x,
            geometry.rect.y1 + self.inset_y() - self.visual_scroll_y,
        ))
    }

    fn visible_caret_offset(&self, snapshot: &VisibleSnapshot) -> Option<usize> {
        self.visible_byte_offset(self.caret(), snapshot)
    }

    fn visible_byte_offset(&self, byte: usize, snapshot: &VisibleSnapshot) -> Option<usize> {
        if byte < snapshot.start_byte_offset {
            return None;
        }
        let hidden = self.hidden_bytes_between(snapshot.start_byte_offset, byte);
        let visible = byte - snapshot.start_byte_offset - hidden;
        (visible <= snapshot.text.len()).then_some(visible)
    }

    fn visible_selection_ranges(&self, snapshot: &VisibleSnapshot) -> Vec<Range<usize>> {
        let visible_start = snapshot.start_byte_offset;
        let visible_end = snapshot.start_byte_offset + snapshot.text.len();
        self.selections
            .selections()
            .iter()
            .filter_map(|selection| {
                if selection.is_collapsed() {
                    return None;
                }
                let range = selection.normalized_range();
                let start = range.start.max(visible_start);
                let end = range.end.min(visible_end);
                (start < end).then(|| (start - visible_start)..(end - visible_start))
            })
            .collect()
    }

    fn visible_decoration_ranges(&self, snapshot: &VisibleSnapshot) -> Vec<(Range<usize>, Color)> {
        if self.decorations.document_id != self.document.document_id
            || self.decorations.document_version != self.document.document_version
        {
            return Vec::new();
        }
        let visible_start = snapshot.start_byte_offset;
        let visible_end = visible_start + snapshot.text.len();
        let document_end = self.buffer.document_end_byte();
        self.decorations
            .visible_spans(visible_start as u64, visible_end as u64)
            .filter_map(|span| {
                let start = usize::try_from(span.byte_start).ok()?;
                let end = usize::try_from(span.byte_end).ok()?;
                if start >= end || end > document_end {
                    return None;
                }
                let start = start.max(visible_start);
                let end = end.min(visible_end);
                let range = (start - visible_start)..(end - visible_start);
                (range.start < range.end
                    && snapshot.text.is_char_boundary(range.start)
                    && snapshot.text.is_char_boundary(range.end))
                .then(|| {
                    (
                        range,
                        self.theme
                            .style_for(span.kind, span.token_type, span.modifiers)
                            .color,
                    )
                })
            })
            .collect()
    }

    fn visible_diagnostic_ranges(&self, snapshot: &VisibleSnapshot) -> Vec<(Range<usize>, Color)> {
        if self.diagnostics.document_id != self.document.document_id
            || self.diagnostics.document_version != self.document.document_version
        {
            return Vec::new();
        }
        let visible_start = snapshot.start_byte_offset;
        let visible_end = visible_start + snapshot.text.len();
        let document_end = self.buffer.document_end_byte();
        self.diagnostics
            .visible_spans(visible_start as u64, visible_end as u64)
            .filter_map(|span| {
                let start = usize::try_from(span.byte_start).ok()?;
                let end = usize::try_from(span.byte_end).ok()?;
                if start > end || end > document_end {
                    return None;
                }
                let start = start.max(visible_start);
                let end = end.min(visible_end);
                let range = (start - visible_start)..(end - visible_start);
                (range.start <= range.end
                    && snapshot.text.is_char_boundary(range.start)
                    && snapshot.text.is_char_boundary(range.end))
                .then(|| (range, self.theme.diagnostic_style(span.severity).color))
            })
            .collect()
    }

    fn visible_snapshot(&self) -> VisibleSnapshot {
        // Logical-line window plus wrap-mode overscan. Unwrapped megabyte
        // lines still extract whole; ponytail: column-window if that shows up.
        let _scope = self.perf.scope("editor.visible_extraction");
        let range = self.viewport.visible_range(self.buffer.line_len());
        let snapshot = self.fold_visible_snapshot(self.buffer.visible_snapshot(range));
        self.perf.record_bytes(
            "editor.visible_extraction.bytes",
            snapshot.text.len() as u64,
        );
        snapshot
    }

    fn fold_visible_snapshot(&self, snapshot: VisibleSnapshot) -> VisibleSnapshot {
        if self.folds.collapsed.is_empty() {
            return snapshot;
        }
        let mut text = String::new();
        let mut first = true;
        for (index, line) in snapshot.text.split('\n').enumerate() {
            if self.line_is_hidden(snapshot.line_range.start + index) {
                continue;
            }
            if !first {
                text.push('\n');
            }
            text.push_str(line);
            first = false;
        }
        VisibleSnapshot {
            text,
            line_range: snapshot.line_range,
            start_byte_offset: snapshot.start_byte_offset,
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let mut ranges: Vec<Range<usize>> = self
            .selections
            .selections()
            .iter()
            .map(|selection| selection.normalized_range())
            .filter(|range| range.start < range.end)
            .collect();
        if ranges.is_empty() {
            return None;
        }
        // Multi-cursor copy joins every range in document order (Plan 071
        // task 9); a single selection yields exactly its text (parity).
        ranges.sort_by_key(|range| range.start);
        let mut text = String::new();
        for (index, range) in ranges.iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            text.push_str(&self.buffer.text_range(range.clone()));
        }
        (!text.is_empty()).then_some(text)
    }

    fn selected_range(&self) -> Option<Range<usize>> {
        self.selections.primary_range()
    }

    fn completion_request_event(
        &self,
        route: CompletionTriggerRoute,
    ) -> Option<EditorCompletionRequestEvent> {
        if !matches!(
            route.routing_policy,
            crate::protocol::RoutingPolicy::UiReactivePriority
        ) {
            return None;
        }
        let cursor = self.buffer.clamp_byte_offset(self.caret());
        let start = self.word_prefix_start(cursor);
        Some(EditorCompletionRequestEvent {
            document_id: self.document.document_id,
            document_version: self.document.document_version,
            behavior_version: self.document.behavior_version,
            cursor_byte_offset: cursor as u64,
            replacement_range: CompletionReplacementRange::new(start as u64, cursor as u64),
            trigger: route.trigger,
        })
    }

    fn language_intelligence_request_event(
        &self,
        route: LanguageIntelligenceTriggerRoute,
    ) -> Option<EditorLanguageIntelligenceRequestEvent> {
        if !matches!(
            route.routing_policy,
            crate::protocol::RoutingPolicy::UiReactivePriority
        ) {
            return None;
        }
        self.language_intelligence_request_for_feature(route.feature)
    }

    /// Captures the current document/version/cursor for a language-intelligence
    /// feature request. Used by keybindings and Control Center interception.
    pub(crate) fn language_intelligence_request_for_feature(
        &self,
        feature: crate::protocol::LanguageIntelligenceFeature,
    ) -> Option<EditorLanguageIntelligenceRequestEvent> {
        let cursor = self.buffer.clamp_byte_offset(self.caret());
        Some(EditorLanguageIntelligenceRequestEvent {
            document_id: self.document.document_id,
            document_version: self.document.document_version,
            behavior_version: self.document.behavior_version,
            cursor_byte_offset: cursor as u64,
            feature,
        })
    }

    /// Captures the current document/version and the whole selection set for
    /// a text-object/smart-select request (Plan 071 task 10). Bounded by
    /// `MAX_SELECTION_QUERY_CURSORS` so the server work stays finite.
    pub(crate) fn selection_query_request_for(
        &self,
        query: crate::protocol::SelectionQuery,
    ) -> Option<EditorSelectionQueryRequestEvent> {
        let selections: Vec<crate::protocol::SelectionQueryCursor> = self
            .selections
            .selections()
            .iter()
            .take(crate::protocol::MAX_SELECTION_QUERY_CURSORS)
            .map(|selection| crate::protocol::SelectionQueryCursor {
                anchor: self.buffer.clamp_byte_offset(selection.anchor()) as u64,
                focus: self.buffer.clamp_byte_offset(selection.focus()) as u64,
            })
            .collect();
        Some(EditorSelectionQueryRequestEvent {
            document_id: self.document.document_id,
            document_version: self.document.document_version,
            behavior_version: self.document.behavior_version,
            query,
            selections,
        })
    }

    /// Installs selection-query ranges as the new selection set (multi-cursor
    /// aware: one resulting selection per requested caret). Snapshotting the
    /// previous set first keeps cursor-undo working across queries.
    pub(crate) fn apply_selection_query_result(&mut self, selections: Vec<Selection>) {
        if selections.is_empty() {
            return;
        }
        let primary = self.selections.primary_index().min(selections.len() - 1);
        self.snapshot_selection_set();
        self.selections.set_selections(selections, primary);
        self.selections.clamp_to(&self.buffer);
    }

    fn word_prefix_start(&self, cursor: usize) -> usize {
        let cursor = self.buffer.clamp_byte_offset(cursor);
        let line_start = self.buffer.line_start_byte(cursor);
        let before = self.buffer.text_range(line_start..cursor);
        let start_in_line = before
            .char_indices()
            .rev()
            .find(|(_, character)| !is_completion_word_character(*character))
            .map(|(index, character)| index + character.len_utf8())
            .unwrap_or(0);
        line_start + start_in_line
    }

    fn pair_rule_for_inserted_text(&self, text: &str) -> Option<&PairRule> {
        let manifest = self.document.behavior_manifest.as_ref()?;
        manifest
            .editor_rules
            .pairs
            .iter()
            .find(|pair| pair.open == text)
    }

    fn electric_rule_for_inserted_text(&self, text: &str) -> Option<&ElectricCharacterRule> {
        let manifest = self.document.behavior_manifest.as_ref()?;
        manifest
            .editor_rules
            .electric_characters
            .iter()
            .find(|rule| rule.trigger == text)
    }

    fn newline_text_at(&self, byte_offset: usize) -> String {
        let Some(manifest) = &self.document.behavior_manifest else {
            return "\n".to_string();
        };

        match manifest.editor_rules.enter {
            EnterRule::InsertNewlineOnly => "\n".to_string(),
            // The generic list-continuation and fence-indent variants are
            // executed by Rust-known transform engines when the client applies
            // them locally.  For now fall through to whitespace preservation;
            // the full engine implementation is tracked in a later task.
            EnterRule::ContinueLineMarkers { .. }
            | EnterRule::PreserveFenceBodyIndent { .. }
            | EnterRule::PreserveLeadingWhitespace => {
                let line_before = self.buffer.line_text_before_byte(byte_offset);
                let indent: String = line_before
                    .chars()
                    .take_while(|character| *character == ' ' || *character == '\t')
                    .collect();
                let trimmed = line_before.trim_start_matches([' ', '\t']);
                let continuation = manifest
                    .editor_rules
                    .comments
                    .iter()
                    .find(|rule| trimmed.starts_with(&rule.line_prefix))
                    .map(|rule| rule.continue_prefix.as_str())
                    .unwrap_or("");

                format!("\n{indent}{continuation}")
            }
        }
    }

    fn apply_line_prefix_transform(&mut self, kind: LinePrefixKind) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }
        let Some(manifest) = self.document.behavior_manifest.as_ref() else {
            return EditorCommandOutcome::unchanged_with(kind.missing_rule_diagnostic());
        };
        let spec = match kind.spec(&manifest.editor_rules) {
            Ok(spec) => spec,
            Err(diagnostic) => return EditorCommandOutcome::unchanged_with(diagnostic),
        };

        let line_indices = unique_line_indices(&self.buffer, self.selections.selections());
        let mut plans: Vec<(usize, usize, String)> = Vec::new();
        match &spec {
            LinePrefixSpec::Toggle {
                prefix,
                ordered_dot,
            } => {
                let mut non_empty = 0usize;
                let mut with_prefix = 0usize;
                let mut lines = Vec::new();
                for &line in &line_indices {
                    let (start, end) = self.buffer.line_range(self.buffer.byte_of_line(line));
                    let text = self.buffer.text_range(start..end);
                    let empty = text.trim().is_empty();
                    if !empty {
                        non_empty += 1;
                        if line_has_toggle_prefix(&text, prefix, *ordered_dot) {
                            with_prefix += 1;
                        }
                    }
                    lines.push((start, end, text, empty));
                }
                if non_empty == 0 {
                    return EditorCommandOutcome::unchanged();
                }
                let strip = with_prefix == non_empty;
                for (start, end, text, empty) in lines {
                    if empty {
                        continue;
                    }
                    let next = if strip {
                        strip_toggle_prefix(&text, prefix, *ordered_dot)
                    } else if line_has_toggle_prefix(&text, prefix, *ordered_dot) {
                        continue;
                    } else {
                        add_line_prefix(&text, prefix)
                    };
                    if next != text {
                        plans.push((start, end, next));
                    }
                }
            }
            LinePrefixSpec::Rotate { prefixes } => {
                for &line in &line_indices {
                    let (start, end) = self.buffer.line_range(self.buffer.byte_of_line(line));
                    let text = self.buffer.text_range(start..end);
                    let next = rotate_line_prefix(&text, prefixes);
                    if next != text {
                        plans.push((start, end, next));
                    }
                }
            }
        }
        if plans.is_empty() {
            return EditorCommandOutcome::unchanged();
        }
        self.apply_line_range_replacements(plans)
    }

    fn apply_line_range_replacements(
        &mut self,
        mut plans: Vec<(usize, usize, String)>,
    ) -> EditorCommandOutcome {
        plans.sort_by_key(|plan| std::cmp::Reverse(plan.0));
        let primary_index = self.selections.primary_index();
        let set_before: Vec<HistorySelection> = self
            .selections
            .selections()
            .iter()
            .map(|selection| HistorySelection {
                caret: selection.focus(),
                anchor: (!selection.is_collapsed()).then(|| selection.anchor()),
            })
            .collect();
        let mut mapped: Vec<Selection> = self.selections.selections().to_vec();
        let mut forward_ops = Vec::with_capacity(plans.len());
        let mut inverse_ops = Vec::with_capacity(plans.len());
        let mut decorations_changed = false;
        for (start, end, text) in plans {
            let operation = EditOperation::Replace {
                start: start as u64,
                end: end as u64,
                text: text.clone(),
            };
            let prior_text = self.prior_text_for_operation(&operation);
            inverse_ops.push(invert_edit_operation(&operation, &prior_text));
            forward_ops.push(operation.clone());
            self.apply_buffer_operation(&operation);
            if self.decorations.apply_edit(&operation) {
                decorations_changed = true;
            }
            let new_len = text.len();
            for selection in &mut mapped {
                selection.set_focus(map_offset_after_replace(
                    selection.focus(),
                    start,
                    end,
                    new_len,
                ));
                selection.set_anchor(map_offset_after_replace(
                    selection.anchor(),
                    start,
                    end,
                    new_len,
                ));
            }
        }
        inverse_ops.reverse();
        let set_after: Vec<HistorySelection> = mapped
            .iter()
            .map(|selection| HistorySelection {
                caret: selection.focus(),
                anchor: (!selection.is_collapsed()).then(|| selection.anchor()),
            })
            .collect();
        let _ = self.selections.set_selections(mapped, primary_index);
        self.snippet_session = None;
        if decorations_changed {
            self.bump_layout_style_revision();
        }
        self.history.record(HistoryEntry {
            forward: forward_ops[0].clone(),
            inverse: inverse_ops[0].clone(),
            selection_before: set_before[primary_index.min(set_before.len() - 1)],
            selection_after: set_after[primary_index.min(set_after.len() - 1)],
            forward_ops: forward_ops.clone(),
            inverse_ops,
            selection_set_before: set_before,
            selection_set_after: set_after,
            primary_index,
        });
        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        self.perf.record_counter("editor.input.local_edit", 1);
        let events: Vec<EditorEditEvent> = forward_ops
            .into_iter()
            .filter_map(|operation| self.client_first_event(operation))
            .collect();
        EditorCommandOutcome::changed_multi(events)
    }

    fn insert_pair_with_event(&mut self, pair: &PairRule) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        let (operation, final_caret) = if let Some(range) = self.selected_range() {
            let selected_text = self.buffer.text_range(range.clone());
            let replacement = format!("{}{}{}", pair.open, selected_text, pair.close);
            let operation = EditOperation::Replace {
                start: range.start as u64,
                end: range.end as u64,
                text: replacement,
            };
            let final_caret =
                range.start + pair.open.len() + selected_text.len() + pair.close.len();
            (operation, final_caret)
        } else {
            let byte_offset = self.buffer.clamp_byte_offset(self.caret());
            let insertion = format!("{}{}", pair.open, pair.close);
            let operation = EditOperation::Insert {
                byte_offset: byte_offset as u64,
                text: insertion,
            };
            let final_caret = byte_offset + pair.open.len();
            (operation, final_caret)
        };

        self.apply_and_record_local_edit(operation, Some(final_caret))
    }

    /// Apply an electric-character rule locally from manifest data. For
    /// [`ElectricEffect::OutdentOneLevel`], when the trigger is typed as the
    /// first non-whitespace character on an over-indented line, the leading
    /// whitespace is shed by one indentation unit before the trigger is
    /// inserted, so a closing bracket aligns with its opener. Otherwise the
    /// trigger is inserted as ordinary text. No IPC, JavaScript, or server
    /// round trip is involved; the effect is entirely Rust-known and driven by
    /// declarative manifest parameters.
    fn insert_electric_with_event(&mut self, rule: &ElectricCharacterRule) -> EditorCommandOutcome {
        if !self.is_editable() || !matches!(rule.effect, ElectricEffect::OutdentOneLevel) {
            return EditorCommandOutcome::unchanged();
        }

        let byte_offset = self.buffer.clamp_byte_offset(self.caret());
        let line_start = self.buffer.line_start_byte(byte_offset);
        let leading = self.buffer.text_range(line_start..byte_offset);

        // Only reflow when the typed trigger is the first non-whitespace on the
        // line and the line has at least one indentation unit to shed.
        let Some(dedented) = dedent_leading_one_level(
            &leading,
            self.electric_tab_kind(),
            self.electric_indent_width(),
        ) else {
            return self.insert_text_with_event(&rule.trigger);
        };

        let replacement = format!("{dedented}{}", rule.trigger);
        let operation = EditOperation::Replace {
            start: line_start as u64,
            end: byte_offset as u64,
            text: replacement,
        };
        let final_caret = line_start + dedented.len() + rule.trigger.len();
        self.apply_and_record_local_edit(operation, Some(final_caret))
    }

    fn electric_tab_kind(&self) -> crate::protocol::TabMode {
        self.document
            .behavior_manifest
            .as_ref()
            .map(|m| m.editor_rules.tab.mode.clone())
            .unwrap_or(crate::protocol::TabMode::InsertSpaces)
    }

    fn electric_indent_width(&self) -> usize {
        self.document
            .behavior_manifest
            .as_ref()
            .map(|m| m.editor_rules.tab.spaces_per_tab as usize)
            .unwrap_or(4)
    }

    fn replace_selection_or_insert_with_event(&mut self, text: &str) -> EditorCommandOutcome {
        if self.selections.selection_count() > 1 {
            let inserted = text.to_string();
            return self.multi_caret_edit(|surface, focus, range| {
                if range.start < range.end {
                    Some((
                        EditOperation::Replace {
                            start: range.start as u64,
                            end: range.end as u64,
                            text: inserted.clone(),
                        },
                        range.start + inserted.len(),
                    ))
                } else {
                    let offset = surface.buffer.clamp_byte_offset(focus);
                    Some((
                        EditOperation::Insert {
                            byte_offset: offset as u64,
                            text: inserted.clone(),
                        },
                        offset + inserted.len(),
                    ))
                }
            });
        }
        let operation = if let Some(range) = self.selected_range() {
            EditOperation::Replace {
                start: range.start as u64,
                end: range.end as u64,
                text: text.to_string(),
            }
        } else {
            let byte_offset = self.buffer.clamp_byte_offset(self.caret());
            EditOperation::Insert {
                byte_offset: byte_offset as u64,
                text: text.to_string(),
            }
        };
        self.apply_and_record_local_edit(operation, None)
    }

    fn collapse_selection_to(&mut self, caret: usize) -> bool {
        let previous_caret = self.caret();
        let had_selection = self.has_selection();
        self.snippet_session = None;
        self.set_primary_focus(caret);
        self.clear_selection();
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        had_selection || previous_caret != self.caret()
    }

    fn finish_edit(&mut self, result: EditResult) -> EditorCommandOutcome {
        self.set_primary_focus(result.caret);
        self.clear_selection();
        if !result.changed {
            return EditorCommandOutcome::unchanged();
        }

        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        EditorCommandOutcome::changed(None)
    }

    /// Apply a local edit operation, record its inverse for undo, and emit an
    /// ordinary client-first edit event when the lease/manifest allow it.
    fn apply_and_record_local_edit(
        &mut self,
        operation: EditOperation,
        final_caret: Option<usize>,
    ) -> EditorCommandOutcome {
        let selection_before = self.capture_history_selection();
        let prior_text = self.prior_text_for_operation(&operation);
        let result = self.apply_buffer_operation(&operation);
        let caret = final_caret.unwrap_or(result.caret);
        self.finish_edit_with_operation_and_caret(
            result,
            operation,
            caret,
            selection_before,
            prior_text,
        )
    }

    /// Undo the latest local edit by applying its inverse as an ordinary edit.
    pub fn undo_with_event(&mut self) -> EditorCommandOutcome {
        let _ = self.composition.clear();
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }
        let Some(entry) = self.history.undo() else {
            return EditorCommandOutcome::unchanged();
        };
        if entry.is_multi() {
            // Inverse ops were recorded in forward application order; undoing
            // replays them in reverse (Plan 071 task 9).
            let mut inverse_ops = entry.inverse_ops;
            inverse_ops.reverse();
            return self.apply_multi_history_restore(
                inverse_ops,
                entry.selection_set_before,
                entry.primary_index,
            );
        }
        self.apply_history_restore(&entry.inverse, entry.selection_before)
    }

    /// Redo the latest undone local edit as an ordinary edit.
    pub fn redo_with_event(&mut self) -> EditorCommandOutcome {
        let _ = self.composition.clear();
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }
        let Some(entry) = self.history.redo() else {
            return EditorCommandOutcome::unchanged();
        };
        if entry.is_multi() {
            return self.apply_multi_history_restore(
                entry.forward_ops,
                entry.selection_set_after,
                entry.primary_index,
            );
        }
        self.apply_history_restore(&entry.forward, entry.selection_after)
    }

    #[cfg(test)]
    pub(crate) fn history_for_test(&self) -> &EditHistory {
        &self.history
    }

    fn finish_edit_with_operation_and_caret(
        &mut self,
        result: EditResult,
        operation: EditOperation,
        caret: usize,
        selection_before: HistorySelection,
        prior_text: String,
    ) -> EditorCommandOutcome {
        if result.changed {
            self.update_snippet_session_after_edit(&operation);
            if self.decorations.apply_edit(&operation) {
                self.bump_layout_style_revision();
            }
        }
        self.set_primary_focus(self.buffer.clamp_byte_offset(caret));
        self.clear_selection();
        if !result.changed {
            return EditorCommandOutcome::unchanged();
        }

        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        self.perf.record_counter("editor.input.local_edit", 1);
        let selection_after = self.capture_history_selection();
        let inverse = invert_edit_operation(&operation, &prior_text);
        self.history.record(HistoryEntry {
            forward: operation.clone(),
            inverse,
            selection_before,
            selection_after,
            forward_ops: Vec::new(),
            inverse_ops: Vec::new(),
            selection_set_before: Vec::new(),
            selection_set_after: Vec::new(),
            primary_index: 0,
        });
        let edit_event = self.client_first_event(operation);
        EditorCommandOutcome::changed(edit_event)
    }

    fn apply_history_restore(
        &mut self,
        operation: &EditOperation,
        selection: HistorySelection,
    ) -> EditorCommandOutcome {
        let result = self.apply_buffer_operation(operation);
        if !result.changed {
            return EditorCommandOutcome::unchanged();
        }
        self.snippet_session = None;
        if self.decorations.apply_edit(operation) {
            self.bump_layout_style_revision();
        }
        self.restore_history_selection(selection);
        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        self.perf.record_counter("editor.input.local_edit", 1);
        let edit_event = self.client_first_event(operation.clone());
        EditorCommandOutcome::changed(edit_event)
    }

    /// Restore a combined multi-cursor history step (Plan 071 task 9): apply
    /// every operation (stored right-to-left, so order keeps offsets valid)
    /// and reinstall the whole selection set snapshot.
    fn apply_multi_history_restore(
        &mut self,
        operations: Vec<EditOperation>,
        set: Vec<HistorySelection>,
        primary_index: usize,
    ) -> EditorCommandOutcome {
        let mut decorations_changed = false;
        for operation in &operations {
            self.apply_buffer_operation(operation);
            if self.decorations.apply_edit(operation) {
                decorations_changed = true;
            }
        }
        if decorations_changed {
            self.bump_layout_style_revision();
        }
        let selections: Vec<Selection> = set
            .iter()
            .map(|snapshot| {
                Selection::new(snapshot.anchor.unwrap_or(snapshot.caret), snapshot.caret)
            })
            .collect();
        let mut restored = SelectionState::default();
        let _ = restored.set_selections(selections, primary_index);
        restored.clamp_to(&self.buffer);
        self.selections = restored;
        self.snippet_session = None;
        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        self.perf.record_counter("editor.input.local_edit", 1);
        let events: Vec<EditorEditEvent> = operations
            .into_iter()
            .filter_map(|operation| self.client_first_event(operation))
            .collect();
        EditorCommandOutcome::changed_multi(events)
    }

    fn apply_buffer_operation(&mut self, operation: &EditOperation) -> EditResult {
        match operation {
            EditOperation::Insert { byte_offset, text } => {
                self.buffer.insert_at(*byte_offset as usize, text)
            }
            EditOperation::Delete { start, end } => {
                self.buffer.delete_range(*start as usize..*end as usize)
            }
            EditOperation::Replace { start, end, text } => self
                .buffer
                .replace_range(*start as usize..*end as usize, text),
        }
    }

    fn prior_text_for_operation(&self, operation: &EditOperation) -> String {
        match operation {
            EditOperation::Insert { .. } => String::new(),
            EditOperation::Delete { start, end } | EditOperation::Replace { start, end, .. } => {
                self.buffer.text_range(*start as usize..*end as usize)
            }
        }
    }

    fn capture_history_selection(&self) -> HistorySelection {
        HistorySelection {
            caret: self.caret(),
            anchor: self
                .has_selection()
                .then(|| self.selections.primary_anchor()),
        }
    }

    fn restore_history_selection(&mut self, selection: HistorySelection) {
        let caret = self.buffer.clamp_byte_offset(selection.caret);
        self.set_primary_focus(caret);
        let clamped_anchor = selection
            .anchor
            .map(|anchor| self.buffer.clamp_byte_offset(anchor))
            .unwrap_or(caret);
        self.selections.primary_mut().set_anchor(clamped_anchor);
    }

    fn client_first_event(&self, operation: EditOperation) -> Option<EditorEditEvent> {
        if !self.is_editable() || !self.manifest_allows(&operation) {
            return None;
        }

        Some(EditorEditEvent {
            document_id: self.document.document_id,
            base_version: self.document.document_version,
            behavior_version: self.document.behavior_version,
            operation,
        })
    }

    fn is_editable(&self) -> bool {
        matches!(self.document.access, DocumentAccess::Editable { .. })
    }

    fn manifest_allows(&self, operation: &EditOperation) -> bool {
        let Some(manifest) = &self.document.behavior_manifest else {
            return false;
        };

        manifest.allows_client_first_edit(operation)
    }

    fn ensure_caret_line_visible(&mut self) -> bool {
        let caret_line = self.buffer.line_of_byte(self.caret());
        let changed = self
            .viewport
            .ensure_line_visible(caret_line, self.buffer.line_len());
        // A caret move always wants the caret sub-line visible on the next
        // paint; explicit scrolling clears this flag so the view can move away.
        self.pin_caret_visible = true;
        changed
    }

    /// Active caret byte offset = the primary selection focus. Replaces the
    /// legacy self.caret() reads.
    pub(crate) fn caret(&self) -> usize {
        self.selections.primary_focus()
    }

    /// Move the primary focus, clearing preferred_x (mirrors CursorState::set_caret).
    /// Does not touch the anchor; pair with clear_selection when clearing.
    fn set_primary_focus(&mut self, focus: usize) {
        self.selections.set_primary_focus(focus);
    }

    /// Collapse the primary selection (anchor := focus), i.e. no active range.
    /// Replaces the legacy self.selection = None.
    fn clear_selection(&mut self) {
        self.selections.collapse_primary();
    }

    /// True when the primary selection is an active range. Replaces
    /// self.selection.is_some().
    fn has_selection(&self) -> bool {
        self.selections.has_selection()
    }

    fn move_cursor(
        &mut self,
        movement: impl FnOnce(&mut CursorState, &EditorBuffer) -> bool,
    ) -> bool {
        self.snippet_session = None;
        let had_selection = self.has_selection();
        let changed = movement(self.selections.primary_mut().cursor_mut(), &self.buffer);
        self.clear_selection();
        if changed || had_selection {
            self.ensure_caret_line_visible();
            self.follow_visual_end = false;
        }
        changed || had_selection
    }

    fn extend_selection(
        &mut self,
        movement: impl FnOnce(&mut CursorState, &EditorBuffer) -> bool,
    ) -> bool {
        self.snippet_session = None;
        let changed = movement(self.selections.primary_mut().cursor_mut(), &self.buffer);
        if !changed {
            return false;
        }
        self.selections.clamp_primary_anchor(&self.buffer);
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        true
    }

    #[cfg(test)]
    fn build_visible_layout_for_test(&mut self, max_width: f32) {
        let text = self.visible_text();
        let display_text = if text.is_empty() {
            PLACEHOLDER_TEXT
        } else {
            text.as_str()
        };
        self.layout.set_cached_layout_with_typography_for_test(
            display_text,
            max_width,
            &self.typography,
            self.document_font_role(),
        );
    }

    #[cfg(test)]
    fn set_text_for_test(&mut self, text: &str) {
        self.load_snapshot(
            0,
            0,
            text.to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
    }

    #[cfg(test)]
    pub(crate) fn set_caret_for_test(&mut self, caret: usize) {
        let caret = self.buffer.clamp_byte_offset(caret);
        self.set_primary_focus(caret);
        self.clear_selection();
    }

    #[cfg(test)]
    pub(crate) fn caret_for_test(&self) -> usize {
        self.caret()
    }

    #[cfg(test)]
    pub(crate) fn selection_for_test(&self) -> Option<(usize, usize)> {
        let selection = self.selections.primary();
        (selection.anchor() != selection.focus()).then(|| (selection.anchor(), selection.focus()))
    }

    #[cfg(test)]
    pub(crate) fn set_selection_for_test(&mut self, anchor: usize, focus: usize) {
        let anchor = self.buffer.clamp_byte_offset(anchor);
        let focus = self.buffer.clamp_byte_offset(focus);
        self.set_primary_focus(focus);
        self.selections.primary_mut().set_anchor(anchor);
    }

    #[cfg(test)]
    pub(crate) fn add_selection_for_test(&mut self, anchor: usize, focus: usize) {
        let selection = Selection::new(
            self.buffer.clamp_byte_offset(anchor),
            self.buffer.clamp_byte_offset(focus),
        );
        self.selections.push_selection(selection);
    }

    #[cfg(test)]
    pub(crate) fn selection_count_for_test(&self) -> usize {
        self.selections.selection_count()
    }

    #[cfg(test)]
    pub(crate) fn buffer_text_for_test(&self) -> String {
        self.buffer.text_range(0..self.buffer.document_end_byte())
    }

    #[cfg(test)]
    pub(crate) fn selection_focus_positions_for_test(&self) -> Vec<usize> {
        self.selections
            .selections()
            .iter()
            .map(|selection| selection.focus())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn visual_scroll_y(&self) -> f64 {
        self.visual_scroll_y
    }

    #[cfg(test)]
    pub(crate) fn set_visual_scroll_bounds_for_test(&mut self, max_scroll_y: f64) {
        self.last_visual_max_scroll_y = max_scroll_y.max(0.0);
        self.visual_scroll_y = self
            .visual_scroll_y
            .clamp(0.0, self.last_visual_max_scroll_y);
    }

    #[cfg(test)]
    pub(crate) fn set_visual_scroll_x_bounds_for_test(&mut self, max_scroll_x: f64) {
        self.last_visual_max_scroll_x = max_scroll_x.max(0.0);
        self.visual_scroll_x = self
            .visual_scroll_x
            .clamp(0.0, self.last_visual_max_scroll_x);
    }
}

/// Shed one indentation unit from a line of all-whitespace `leading` text.
/// Returns the dedented leading string when the line is over-indented enough
/// to lose a full unit, otherwise `None` (no electric reflow applies).
///
/// This is the generic Rust-known transform engine behind
/// [`ElectricEffect::OutdentOneLevel`]; it consults only the declarative tab
/// kind/width from the manifest and contains no language-specific branch.
/// Start byte offset of an edit operation, for right-to-left ordering of
/// multi-cursor edits (Plan 071 task 9).
enum LinePrefixKind {
    ToggleComment,
    ToggleListMarker,
    RotateHeading,
}

enum LinePrefixSpec {
    Toggle { prefix: String, ordered_dot: bool },
    Rotate { prefixes: Vec<String> },
}

impl LinePrefixKind {
    fn missing_rule_diagnostic(self) -> &'static str {
        match self {
            Self::ToggleComment => "no comments rule",
            Self::ToggleListMarker => "no list markers",
            Self::RotateHeading => "no heading prefixes",
        }
    }

    fn spec(
        self,
        rules: &crate::protocol::EditorBehaviorRules,
    ) -> Result<LinePrefixSpec, &'static str> {
        match self {
            Self::ToggleComment => {
                let prefix = rules
                    .comments
                    .first()
                    .map(|rule| rule.line_prefix.as_str())
                    .filter(|prefix| !prefix.is_empty())
                    .ok_or("no comments rule")?;
                Ok(LinePrefixSpec::Toggle {
                    prefix: prefix.to_string(),
                    ordered_dot: false,
                })
            }
            Self::ToggleListMarker => {
                let crate::protocol::EnterRule::ContinueLineMarkers { markers, .. } = &rules.enter
                else {
                    return Err("no list markers");
                };
                let marker = markers
                    .first()
                    .map(String::as_str)
                    .ok_or("no list markers")?;
                if marker == "ordered-dot" {
                    Ok(LinePrefixSpec::Toggle {
                        prefix: "1. ".to_string(),
                        ordered_dot: true,
                    })
                } else {
                    let prefix = if marker.ends_with(' ') {
                        marker.to_string()
                    } else {
                        format!("{marker} ")
                    };
                    Ok(LinePrefixSpec::Toggle {
                        prefix,
                        ordered_dot: false,
                    })
                }
            }
            Self::RotateHeading => {
                if rules.heading_prefixes.is_empty() {
                    return Err("no heading prefixes");
                }
                Ok(LinePrefixSpec::Rotate {
                    prefixes: rules.heading_prefixes.clone(),
                })
            }
        }
    }
}

fn unique_line_indices(buffer: &EditorBuffer, selections: &[Selection]) -> Vec<usize> {
    let mut lines = BTreeSet::new();
    for selection in selections {
        let range = selection.normalized_range();
        let start_line = buffer.line_of_byte(range.start);
        let end_byte = if range.end > range.start {
            range.end.saturating_sub(1)
        } else {
            range.start
        };
        let end_line = buffer.line_of_byte(end_byte);
        for line in start_line..=end_line {
            lines.insert(line);
        }
    }
    lines.into_iter().collect()
}

fn indent_split(line: &str) -> (&str, &str) {
    let indent_len = line
        .find(|character: char| character != ' ' && character != '\t')
        .unwrap_or(line.len());
    line.split_at(indent_len)
}

fn line_has_toggle_prefix(line: &str, prefix: &str, ordered_dot: bool) -> bool {
    let rest = indent_split(line).1;
    if rest.starts_with(prefix) {
        return true;
    }
    ordered_dot && ordered_dot_body(rest).is_some()
}

fn ordered_dot_body(rest: &str) -> Option<&str> {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    rest[digits..]
        .strip_prefix(". ")
        .or_else(|| rest[digits..].strip_prefix('.'))
}

fn add_line_prefix(line: &str, prefix: &str) -> String {
    let (indent, rest) = indent_split(line);
    format!("{indent}{prefix}{rest}")
}

fn strip_toggle_prefix(line: &str, prefix: &str, ordered_dot: bool) -> String {
    let (indent, rest) = indent_split(line);
    if let Some(body) = rest.strip_prefix(prefix) {
        return format!("{indent}{body}");
    }
    if ordered_dot && let Some(body) = ordered_dot_body(rest) {
        return format!("{indent}{body}");
    }
    line.to_string()
}

fn rotate_line_prefix(line: &str, prefixes: &[String]) -> String {
    let (indent, rest) = indent_split(line);
    let current = prefixes
        .iter()
        .enumerate()
        .filter(|(_, prefix)| rest.starts_with(prefix.as_str()))
        .max_by_key(|(_, prefix)| prefix.len());
    match current {
        None => format!("{indent}{}{rest}", prefixes[0]),
        Some((index, prefix)) if index + 1 < prefixes.len() => {
            format!("{indent}{}{}", prefixes[index + 1], &rest[prefix.len()..])
        }
        Some((_, prefix)) => format!("{indent}{}", &rest[prefix.len()..]),
    }
}

fn map_offset_after_replace(offset: usize, start: usize, old_end: usize, new_len: usize) -> usize {
    if offset <= start {
        offset
    } else if offset >= old_end {
        offset - (old_end - start) + new_len
    } else {
        start + (offset - start).min(new_len)
    }
}

fn op_start_offset(operation: &EditOperation) -> usize {
    match operation {
        EditOperation::Insert { byte_offset, .. } => *byte_offset as usize,
        EditOperation::Delete { start, .. } | EditOperation::Replace { start, .. } => {
            *start as usize
        }
    }
}

fn dedent_leading_one_level(
    leading: &str,
    tab_kind: crate::protocol::TabMode,
    width: usize,
) -> Option<String> {
    if leading.is_empty() || !leading.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }
    match tab_kind {
        crate::protocol::TabMode::InsertSpaces => {
            if width == 0 {
                return None;
            }
            let spaces = leading.chars().take_while(|&c| c == ' ').count();
            if spaces >= width {
                Some(leading[width..].to_string())
            } else {
                None
            }
        }
        crate::protocol::TabMode::InsertTabCharacter => {
            if leading.as_bytes().first() == Some(&b'\t') {
                Some(leading[1..].to_string())
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::{
        BlinkPhase, CARET_WIDTH, CaretBlink, Color, CursorSelectDirection, EditorCommand,
        EditorSurface, SCROLLBAR_MARGIN, SCROLLBAR_WIDTH, Selection, TEXT_INSET, TEXT_INSET_GUTTER,
        TEXT_INSET_Y, normalize_visible_text_style_runs, subtract_half_open_range,
    };
    use crate::editor::layout::LayoutCacheKey;
    use crate::perf::{
        budgets::{KEY_CHORD_PENDING_TIMEOUT_MS, SYNTAX_CACHE_BUDGET_BYTES},
        metrics::PerfRecorder,
    };
    use crate::protocol::{
        ActiveTypography, BehaviorManifest, BehaviorScope, BlinkStyle, CaretShape, CaretStyle,
        CommandAuthority, CommandDeclaration, CompletionItemTextFormat, CompletionTrigger,
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentAccess,
        DocumentFontRole, EditOperation, EnterRule, FoldingProvenance, FoldingRange,
        FoldingRangeSet, FontRole, KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers,
        KeyStroke, Modifiers, RoutingPolicy, TabMode, TokenType, WrapPolicy,
    };
    use crate::shell::CompletionMenuAcceptAction;
    use masonry::kurbo::{Point, Rect};

    fn span(
        byte_start: u64,
        byte_end: u64,
        kind: DecorationKind,
        font_role: Option<DocumentFontRole>,
        priority: u16,
        modifiers: Modifiers,
    ) -> DecorationSpan {
        DecorationSpan {
            byte_start,
            byte_end,
            kind,
            token_type: TokenType::CodeSpan,
            modifiers,
            scope: None,
            font_role,
            priority,
            provenance: DecorationProvenance {
                package_name: "test".to_string(),
                package_version: "1.0.0".to_string(),
                package_prefix: "test".to_string(),
            },
            target: None,
            inlay: None,
        }
    }

    fn decoration_set(
        version: u64,
        viewport_start: u64,
        viewport_end: u64,
        spans: Vec<DecorationSpan>,
    ) -> DecorationSet {
        DecorationSet {
            document_id: 1,
            document_version: version,
            package_prefix: "test".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: viewport_start,
            viewport_byte_end: viewport_end,
            spans,
        }
    }

    fn syntax_span(byte_start: u64, byte_end: u64, token_type: TokenType) -> DecorationSpan {
        let mut span = span(
            byte_start,
            byte_end,
            DecorationKind::Syntax,
            None,
            70,
            Modifiers::NONE,
        );
        span.token_type = token_type;
        span
    }

    fn snippet_completion_action(insert_text: &str) -> CompletionMenuAcceptAction {
        CompletionMenuAcceptAction {
            request_id: 1,
            document_id: 7,
            document_version: 12,
            behavior_version: 3,
            replacement_range: crate::protocol::CompletionReplacementRange::new(0, 3),
            insert_text: insert_text.to_string(),
            text_format: CompletionItemTextFormat::Snippet,
            commit_characters: String::new(),
        }
    }

    fn normalized_runs(editor: &EditorSurface) -> Vec<super::VisibleTextStyleRun> {
        let snapshot = editor.visible_snapshot();
        normalize_visible_text_style_runs(
            &editor.decorations,
            &editor.document,
            editor.buffer.document_end_byte(),
            &snapshot,
            editor.document_font_role(),
            editor.theme,
        )
    }

    #[test]
    fn markdown_code_range_uses_monospace_inside_proportional_layout() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "text code".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        assert!(editor.apply_decoration_set(DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 9,
            spans: vec![span(
                5,
                9,
                DecorationKind::Syntax,
                Some(DocumentFontRole::Monospace),
                70,
                Modifiers::NONE,
            )],
        }));

        let runs = normalized_runs(&editor);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].font_role, crate::protocol::FontRole::Proportional);
        assert_eq!(runs[1].range, 5..9);
        assert_eq!(runs[1].font_role, crate::protocol::FontRole::Monospace);
    }

    #[test]
    fn narrow_capture_priority_outranks_broad_prose_at_overlap() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "see link now".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        let broad = syntax_span(0, 12, TokenType::Paragraph);
        assert_eq!(broad.priority, 70);
        let mut narrow = syntax_span(4, 8, TokenType::Link);
        narrow.priority = 80;
        // Broad span first: priority, not emission order, must decide.
        assert!(editor.apply_decoration_set(decoration_set(1, 0, 12, vec![broad, narrow])));

        let runs = normalized_runs(&editor);
        let link_style =
            editor
                .theme
                .style_for(DecorationKind::Syntax, TokenType::Link, Modifiers::NONE);
        let link_run = runs
            .iter()
            .find(|run| run.range == (4..8))
            .expect("overlap resolves into its own run");
        assert_eq!(link_run.color, Some(link_style.color));
        let prose_style = editor.theme.style_for(
            DecorationKind::Syntax,
            TokenType::Paragraph,
            Modifiers::NONE,
        );
        let prose_run = runs
            .iter()
            .find(|run| run.range == (0..4))
            .expect("broad prose run remains outside the overlap");
        assert_eq!(prose_run.color, Some(prose_style.color));
    }

    #[test]
    fn local_edit_keeps_unaffected_decorations_stable_through_ack() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "text code".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        assert!(editor.apply_decoration_set(DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 9,
            spans: vec![span(
                5,
                9,
                DecorationKind::Syntax,
                Some(DocumentFontRole::Monospace),
                70,
                Modifiers::NONE,
            )],
        }));

        assert!(editor.insert_text_with_event("!").changed);
        assert!(editor.note_confirmed_version(1, 2));

        let runs = normalized_runs(&editor);
        assert_eq!(editor.decoration_state_version(), Some(2));
        assert_eq!(runs.last().unwrap().range, 6..10);
        assert_eq!(runs.last().unwrap().font_role, FontRole::Monospace);
    }

    #[test]
    fn optimistic_interior_utf8_insert_extends_syntax_span() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            12,
            vec![syntax_span(2, 5, TokenType::Keyword)],
        )));

        assert!(editor.decorations.apply_edit(&EditOperation::Insert {
            byte_offset: 3,
            text: "🦀".to_string(),
        }));

        let span = &editor.decorations.chunks[0].spans[0];
        assert_eq!((span.byte_start, span.byte_end), (2, 9));
        assert!(editor.decorations.chunks[0].provisional);
    }

    #[test]
    fn optimistic_broad_token_families_shift_at_start_extend_at_end() {
        for token_type in [
            TokenType::Comment,
            TokenType::String,
            TokenType::Paragraph,
            TokenType::CodeBlock,
            TokenType::CodeSpan,
        ] {
            let mut editor = EditorSurface::default();
            assert!(editor.decorations.apply_set(decoration_set(
                1,
                0,
                12,
                vec![syntax_span(2, 6, token_type)],
            )));

            // Inserting at the span's first byte lands before the span: it
            // shifts right unchanged (no leftward color bleed).
            assert!(editor.decorations.apply_edit(&EditOperation::Insert {
                byte_offset: 2,
                text: "/".to_string(),
            }));
            // Inserting at the (shifted) end extends the broad span.
            assert!(editor.decorations.apply_edit(&EditOperation::Insert {
                byte_offset: 7,
                text: "\n".to_string(),
            }));

            let span = &editor.decorations.chunks[0].spans[0];
            assert_eq!((span.byte_start, span.byte_end), (3, 8));
        }
    }

    #[test]
    fn optimistic_narrow_token_families_inherit_same_word_suffixes() {
        for token_type in [
            TokenType::Function,
            TokenType::Type,
            TokenType::Variable,
            TokenType::Keyword,
            TokenType::Number,
        ] {
            let mut editor = EditorSurface::default();
            assert!(editor.decorations.apply_set(decoration_set(
                1,
                0,
                12,
                vec![syntax_span(2, 5, token_type)],
            )));

            assert!(editor.decorations.apply_edit(&EditOperation::Insert {
                byte_offset: 5,
                text: "x2_".to_string(),
            }));

            let span = &editor.decorations.chunks[0].spans[0];
            assert_eq!((span.byte_start, span.byte_end), (2, 8));
        }
    }

    #[test]
    fn optimistic_narrow_span_stops_at_non_word_boundaries() {
        for text in [" ", "\t", "\n", "\"", "]", "/", "+"] {
            let mut editor = EditorSurface::default();
            assert!(editor.decorations.apply_set(decoration_set(
                1,
                0,
                12,
                vec![syntax_span(2, 5, TokenType::Keyword)],
            )));

            assert!(!editor.decorations.apply_edit(&EditOperation::Insert {
                byte_offset: 5,
                text: text.to_string(),
            }));

            let span = &editor.decorations.chunks[0].spans[0];
            assert_eq!((span.byte_start, span.byte_end), (2, 5));
        }
    }

    #[test]
    fn optimistic_narrow_span_inherits_unicode_word_suffix() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            12,
            vec![syntax_span(2, 5, TokenType::Variable)],
        )));

        assert!(editor.decorations.apply_edit(&EditOperation::Insert {
            byte_offset: 5,
            text: "é".to_string(),
        }));

        let span = &editor.decorations.chunks[0].spans[0];
        assert_eq!((span.byte_start, span.byte_end), (2, 7));
    }

    #[test]
    fn optimistic_non_syntax_layers_do_not_inherit_same_word_suffixes() {
        for kind in [
            DecorationKind::Semantic,
            DecorationKind::Diagnostic,
            DecorationKind::SearchMatch,
        ] {
            let mut span = syntax_span(2, 5, TokenType::Variable);
            span.kind = kind;
            let mut set = decoration_set(1, 0, 12, vec![span]);
            set.kind = kind;
            let mut editor = EditorSurface::default();
            assert!(editor.decorations.apply_set(set));

            assert!(!editor.decorations.apply_edit(&EditOperation::Insert {
                byte_offset: 5,
                text: "x".to_string(),
            }));

            let span = &editor.decorations.chunks[0].spans[0];
            assert_eq!((span.byte_start, span.byte_end), (2, 5));
        }
    }

    #[test]
    fn optimistic_replace_resizes_syntax_but_invalidates_semantic_overlap() {
        let mut syntax = syntax_span(2, 8, TokenType::Variable);
        let mut semantic = syntax.clone();
        semantic.kind = DecorationKind::Semantic;
        let mut editor = EditorSurface::default();
        assert!(
            editor
                .decorations
                .apply_set(decoration_set(1, 0, 12, vec![syntax, semantic],))
        );

        assert!(editor.decorations.apply_edit(&EditOperation::Replace {
            start: 4,
            end: 6,
            text: "x".to_string(),
        }));

        let spans = &editor.decorations.chunks[0].spans;
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].byte_start, spans[0].byte_end), (2, 7));
        syntax = spans[0].clone();
        assert_eq!(syntax.kind, DecorationKind::Syntax);
    }

    #[test]
    fn optimistic_delete_shrinks_only_surviving_syntax_geometry() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            12,
            vec![syntax_span(2, 10, TokenType::String)],
        )));

        assert!(
            editor
                .decorations
                .apply_edit(&EditOperation::Delete { start: 4, end: 7 })
        );

        let span = &editor.decorations.chunks[0].spans[0];
        assert_eq!((span.byte_start, span.byte_end), (2, 7));
    }

    #[test]
    fn optimistic_edit_shifts_unaffected_non_syntax_layers() {
        let mut editor = EditorSurface::default();
        for (index, kind) in [
            DecorationKind::Semantic,
            DecorationKind::Diagnostic,
            DecorationKind::SearchMatch,
        ]
        .into_iter()
        .enumerate()
        {
            let start = 10 + index as u64 * 3;
            let mut span = syntax_span(start, start + 2, TokenType::Variable);
            span.kind = kind;
            let mut set = decoration_set(1, start, start + 2, vec![span]);
            set.kind = kind;
            assert!(editor.decorations.apply_set(set));
        }

        assert!(
            editor
                .decorations
                .apply_edit(&EditOperation::Delete { start: 2, end: 4 })
        );

        let ranges = editor
            .decorations
            .chunks
            .iter()
            .map(|chunk| {
                let span = &chunk.spans[0];
                (span.kind, span.byte_start, span.byte_end)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ranges,
            vec![
                (DecorationKind::Semantic, 8, 10),
                (DecorationKind::Diagnostic, 11, 13),
                (DecorationKind::SearchMatch, 14, 16),
            ]
        );
    }

    #[test]
    fn half_open_subtraction_returns_zero_one_or_two_fragments() {
        assert_eq!(subtract_half_open_range(0, 8, 0, 8), [None, None]);
        assert_eq!(subtract_half_open_range(0, 9, 0, 8), [None, Some((8, 9))]);
        assert_eq!(
            subtract_half_open_range(0, 20, 8, 12),
            [Some((0, 8)), Some((12, 20))]
        );
        assert_eq!(
            subtract_half_open_range(2, 4, 0, 2),
            [Some((2, 4)), None],
            "validated UTF-8 boundaries are preserved rather than shifted"
        );
    }

    #[test]
    fn current_authority_replaces_only_its_viewport_and_coalesces_right_residual() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            8,
            vec![syntax_span(0, 8, TokenType::Comment)],
        )));
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            8,
            16,
            vec![syntax_span(8, 16, TokenType::Comment)],
        )));
        assert!(editor.decorations.apply_edit(&EditOperation::Insert {
            byte_offset: 8,
            text: "x".to_string(),
        }));
        editor.decorations.confirm_version(1, 2);

        assert!(
            editor
                .decorations
                .apply_set(decoration_set(2, 0, 8, Vec::new()))
        );
        assert_eq!(editor.decoration_span_count(), 1);
        assert_eq!(editor.decorations.chunks.len(), 1);
        assert_eq!(
            (
                editor.decorations.chunks[0].key.byte_start,
                editor.decorations.chunks[0].key.byte_end,
            ),
            (8, 17)
        );
        assert_eq!(
            (
                editor.decorations.chunks[0].spans[0].byte_start,
                editor.decorations.chunks[0].spans[0].byte_end,
            ),
            (8, 17)
        );
    }

    #[test]
    fn authoritative_viewport_splits_crossing_provisional_span() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            20,
            vec![syntax_span(0, 20, TokenType::Comment)],
        )));
        assert!(editor.decorations.apply_edit(&EditOperation::Insert {
            byte_offset: 10,
            text: "x".to_string(),
        }));
        editor.decorations.confirm_version(1, 2);

        assert!(editor.decorations.apply_set(decoration_set(
            2,
            8,
            12,
            vec![syntax_span(8, 12, TokenType::Keyword)],
        )));

        let mut ranges = editor
            .decorations
            .chunks
            .iter()
            .flat_map(|chunk| chunk.spans.iter())
            .map(|span| (span.byte_start, span.byte_end, span.token_type))
            .collect::<Vec<_>>();
        ranges.sort_by_key(|range| range.0);
        assert_eq!(
            ranges,
            vec![
                (0, 8, TokenType::Comment),
                (8, 12, TokenType::Keyword),
                (12, 21, TokenType::Comment),
            ]
        );
    }

    #[test]
    fn authoritative_syntax_preserves_other_package_and_semantic_provisional_chunks() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            8,
            vec![syntax_span(0, 8, TokenType::Comment)],
        )));
        let mut other_package =
            decoration_set(1, 0, 8, vec![syntax_span(0, 8, TokenType::Variable)]);
        other_package.package_prefix = "other".to_string();
        other_package.spans[0].provenance.package_prefix = "other".to_string();
        assert!(editor.decorations.apply_set(other_package));
        let mut semantic = decoration_set(1, 0, 8, vec![syntax_span(0, 8, TokenType::Variable)]);
        semantic.kind = DecorationKind::Semantic;
        semantic.spans[0].kind = DecorationKind::Semantic;
        assert!(editor.decorations.apply_set(semantic));
        assert!(editor.decorations.apply_edit(&EditOperation::Insert {
            byte_offset: 0,
            text: "x".to_string(),
        }));
        editor.decorations.confirm_version(1, 2);

        assert!(
            editor
                .decorations
                .apply_set(decoration_set(2, 0, 8, Vec::new()))
        );

        assert!(editor.decorations.chunks.iter().any(|chunk| {
            chunk.key.package_prefix == "other" && chunk.key.kind == DecorationKind::Syntax
        }));
        assert!(
            editor
                .decorations
                .chunks
                .iter()
                .any(|chunk| chunk.key.kind == DecorationKind::Semantic)
        );
    }

    #[test]
    fn repeated_authority_keeps_local_residual_cache_bounded() {
        let mut editor = EditorSurface::default();
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            8,
            vec![syntax_span(0, 8, TokenType::Comment)],
        )));
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            8,
            16,
            vec![syntax_span(8, 16, TokenType::Comment)],
        )));

        for version in 2..=513 {
            assert!(editor.decorations.apply_edit(&EditOperation::Insert {
                byte_offset: 4,
                text: "x".to_string(),
            }));
            editor.decorations.confirm_version(1, version);
            assert!(editor.decorations.apply_set(decoration_set(
                version,
                0,
                8,
                vec![syntax_span(0, 8, TokenType::Comment)],
            )));
            assert_eq!(editor.decorations.chunks.len(), 2, "version {version}");
            assert_eq!(editor.decorations.span_count(), 2, "version {version}");
            assert_eq!(
                editor
                    .decorations
                    .chunks
                    .iter()
                    .filter(|chunk| chunk.provisional)
                    .count(),
                1,
                "version {version}"
            );
            assert_eq!(
                editor.decorations.retained_bytes,
                editor
                    .decorations
                    .chunks
                    .iter()
                    .map(|chunk| chunk.byte_size)
                    .sum::<usize>(),
                "version {version}: cache accounting"
            );
            assert!(
                editor.decorations.retained_bytes <= SYNTAX_CACHE_BUDGET_BYTES,
                "version {version}: cache budget"
            );
        }
    }

    #[test]
    fn reversed_edit_is_ignored_and_snapshot_replacement_clears_provisional_state() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "comment".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.decorations.apply_set(decoration_set(
            1,
            0,
            7,
            vec![syntax_span(0, 7, TokenType::Comment)],
        )));
        assert!(
            !editor
                .decorations
                .apply_edit(&EditOperation::Delete { start: 5, end: 2 })
        );
        assert!(editor.decorations.apply_edit(&EditOperation::Insert {
            byte_offset: 3,
            text: "x".to_string(),
        }));

        editor.load_snapshot(
            2,
            1,
            "other".to_string(),
            DocumentAccess::Editable { lease_id: 2 },
        );

        assert_eq!(editor.decoration_span_count(), 0);
    }

    #[test]
    fn overlapping_style_runs_resolve_deterministically_and_merge_adjacent_runs() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "abcde".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.apply_decoration_set(DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 5,
            spans: vec![
                span(
                    1,
                    4,
                    DecorationKind::Syntax,
                    Some(DocumentFontRole::Monospace),
                    10,
                    Modifiers::BOLD,
                ),
                span(
                    2,
                    5,
                    DecorationKind::Semantic,
                    Some(DocumentFontRole::Proportional),
                    10,
                    Modifiers::ITALIC,
                ),
            ],
        }));

        let runs = normalized_runs(&editor);
        assert_eq!(runs.len(), 4);
        assert_eq!(runs[1].range, 1..2);
        assert_eq!(runs[1].font_role, crate::protocol::FontRole::Monospace);
        assert!(runs[1].attributes.bold);
        assert_eq!(runs[2].range, 2..4);
        assert_eq!(runs[2].font_role, crate::protocol::FontRole::Proportional);
        assert!(runs[2].attributes.bold && runs[2].attributes.italic);
        assert_eq!(runs[3].range, 4..5);
    }

    #[test]
    fn search_match_and_quote_backgrounds_join_style_runs() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "abcde".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut quote = syntax_span(0, 5, TokenType::Quote);
        quote.kind = DecorationKind::Syntax;
        let mut search = syntax_span(1, 3, TokenType::Variable);
        search.kind = DecorationKind::SearchMatch;
        let mut search_set = decoration_set(1, 0, 5, vec![search]);
        search_set.kind = DecorationKind::SearchMatch;
        assert!(editor.apply_decoration_set(decoration_set(1, 0, 5, vec![quote])));
        assert!(editor.apply_decoration_set(search_set));

        let runs = normalized_runs(&editor);
        let quote_bg = crate::editor::theme::StyleRegistry::default()
            .style_for(DecorationKind::Syntax, TokenType::Quote, Modifiers::NONE)
            .background;
        let search_bg = crate::editor::theme::StyleRegistry::default()
            .style_for(
                DecorationKind::SearchMatch,
                TokenType::Variable,
                Modifiers::NONE,
            )
            .background;
        let mid = runs
            .iter()
            .find(|run| run.range == (1..3))
            .expect("search overlap run");
        assert_eq!(mid.background, search_bg);
        let edge = runs
            .iter()
            .find(|run| run.range == (0..1))
            .expect("quote-only run");
        assert_eq!(edge.background, quote_bg);
    }

    #[test]
    fn mixed_role_normalization_stays_bounded_by_visible_span_boundaries() {
        let text = "x".repeat(1_000);
        let spans = (0..500)
            .map(|index| {
                span(
                    index * 2,
                    index * 2 + 1,
                    DecorationKind::Syntax,
                    Some(DocumentFontRole::Monospace),
                    10,
                    Modifiers::NONE,
                )
            })
            .collect();
        let mut editor = EditorSurface::default();
        editor.load_snapshot(1, 1, text, DocumentAccess::Editable { lease_id: 1 });
        assert!(editor.apply_decoration_set(DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 1_000,
            spans,
        }));

        let runs = normalized_runs(&editor);
        assert_eq!(runs.len(), 1_000);
        assert_eq!(runs.first().unwrap().range, 0..1);
        assert_eq!(runs.last().unwrap().range, 999..1_000);
    }

    #[test]
    fn scrolling_past_earlier_spans_does_not_underflow_visible_ranges() {
        let text = (0..200)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        let later_start = text.find("line 150").unwrap() as u64;
        let mut editor = EditorSurface::default();
        editor.load_snapshot(1, 1, text.clone(), DocumentAccess::Editable { lease_id: 1 });
        assert!(editor.apply_decoration_set(DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: text.len() as u64,
            spans: vec![
                span(0, 4, DecorationKind::Syntax, None, 10, Modifiers::NONE,),
                span(
                    later_start,
                    later_start + 8,
                    DecorationKind::Syntax,
                    None,
                    10,
                    Modifiers::NONE,
                ),
            ],
        }));

        assert!(editor.scroll_lines(120));
        let snapshot = editor.visible_snapshot();
        let runs = normalized_runs(&editor);

        assert!(snapshot.start_byte_offset > 4);
        assert!(runs.iter().all(|run| run.range.end <= snapshot.text.len()));
    }

    #[test]
    fn diagnostic_and_invalid_utf8_spans_cannot_change_font_role() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "éx".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.apply_decoration_set(DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Diagnostic,
            viewport_byte_start: 0,
            viewport_byte_end: 3,
            spans: vec![
                span(
                    0,
                    2,
                    DecorationKind::Diagnostic,
                    Some(DocumentFontRole::Monospace),
                    99,
                    Modifiers::NONE,
                ),
                span(
                    1,
                    2,
                    DecorationKind::Syntax,
                    Some(DocumentFontRole::Monospace),
                    99,
                    Modifiers::NONE,
                ),
            ],
        }));

        let runs = normalized_runs(&editor);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].range, 0..3);
        assert_eq!(runs[0].font_role, crate::protocol::FontRole::Proportional);
    }

    #[test]
    fn editor_surface_paint_has_no_decorative_accent_circle() {
        // Source guard: the permanent purple bottom-right accent circle was
        // removed. If it returns, this test fails fast. The guard targets the
        // Circle shape primitive used only for that decoration.
        let source = include_str!("mod.rs");
        let paint = source
            .split("mod tests")
            .next()
            .expect("non-test editor surface source");
        assert!(
            !paint.contains("Circle::new"),
            "editor surface must not paint a decorative circle"
        );
    }

    #[test]
    fn editor_surface_uses_full_rect_background_without_visible_card_inset() {
        // Source guard: the 24px inset card/canvas rect has been removed; the
        // background is filled across the full editor rect.
        let source = include_str!("mod.rs");
        let paint = source
            .split("fn paint_in_rect")
            .nth(1)
            .expect("paint_in_rect body");
        let body = paint.split("fn paint_text").next().expect("paint body");
        assert!(
            body.contains("let panel_bg = self.theme.base.panel_bg;"),
            "editor background color must come from the StyleRegistry single source of color"
        );
        assert!(
            body.contains("scene.fill(Fill::NonZero, Affine::IDENTITY, panel_bg, None, &rect)"),
            "editor background must fill the full editor rect"
        );
        assert!(
            !body.contains("+ 24.0"),
            "editor paint must not reintroduce a visible 24px inset card"
        );
    }

    fn editor_with_scroll_bounds(max_scroll_y: f64) -> EditorSurface {
        let mut editor = EditorSurface::default();
        editor.set_visual_scroll_bounds_for_test(max_scroll_y);
        editor
    }

    #[test]
    fn editor_scrollbar_thumb_reflects_visual_scroll_position() {
        let rect = Rect::new(240.0, 0.0, 900.0, 600.0);
        let mut editor = editor_with_scroll_bounds(2000.0);

        let top = editor.scrollbar_thumb_rect(rect).expect("scrollable thumb");
        // Scrolling down moves the thumb down without changing its height.
        editor.scroll_vertical_pixels(700.0);
        let scrolled = editor.scrollbar_thumb_rect(rect).expect("scrollable thumb");
        assert!(
            scrolled.y0 > top.y0,
            "thumb moves down as visual_scroll_y grows"
        );
        assert!((scrolled.height() - top.height()).abs() < 1e-6);
        // Reaching the bottom pins the thumb to the bottom of the track.
        editor.scroll_vertical_pixels(2000.0);
        let bottom = editor.scrollbar_thumb_rect(rect).expect("scrollable thumb");
        assert!(bottom.y1 <= rect.y1 - TEXT_INSET_Y + 0.5);
        assert!(bottom.y1 > scrolled.y1);
    }

    #[test]
    fn editor_scrollbar_hidden_when_content_fits() {
        let rect = Rect::new(240.0, 0.0, 900.0, 600.0);
        let editor = editor_with_scroll_bounds(0.0);
        assert!(editor.scrollbar_thumb_rect(rect).is_none());
    }

    #[test]
    fn editor_scrollbar_stays_inside_main_editor_region_with_left_browser() {
        // The left file browser occupies [0, 240]; the editor main region is
        // [240, 900]. The scrollbar thumb must stay inside the editor rect and
        // never cross into the file browser or past the editor's right edge.
        let rect = Rect::new(240.0, 0.0, 900.0, 600.0);
        let mut editor = editor_with_scroll_bounds(1500.0);
        editor.scroll_vertical_pixels(400.0);
        let thumb = editor.scrollbar_thumb_rect(rect).expect("scrollable thumb");
        assert!(
            thumb.x0 >= rect.x0,
            "scrollbar must not overlap the file browser"
        );
        assert!(
            thumb.x1 <= rect.x1,
            "scrollbar must stay left of the editor right edge"
        );
        assert!(
            thumb.y0 >= rect.y0,
            "scrollbar must stay below the editor top"
        );
        assert!(
            thumb.y1 <= rect.y1,
            "scrollbar must stay above the editor bottom"
        );
    }

    #[test]
    fn editor_scrollbar_reflects_hover_and_active_state() {
        // Plan 065 task 5: the editor scrollbar derives InteractionState from
        // the pointer position + press state (Hover over the track, Active when
        // pressed over the thumb, Rest elsewhere) instead of hardcoded Rest.
        use crate::shell::primitives::InteractionState;
        let rect = Rect::new(240.0, 0.0, 900.0, 600.0);
        let available_height = (rect.height() - (TEXT_INSET_Y * 2.0)).max(0.0);
        let mut editor = editor_with_scroll_bounds(2000.0);

        // No pointer -> Rest.
        assert_eq!(
            editor.scrollbar_interaction_state(rect, available_height),
            InteractionState::Rest
        );

        // At scroll-top the thumb sits at the top of the track.
        let thumb = editor.scrollbar_thumb_rect(rect).expect("scrollable thumb");
        let track_x = rect.x1 - SCROLLBAR_MARGIN - SCROLLBAR_WIDTH / 2.0;

        // Pointer over the thumb, not pressed -> Hover.
        editor.set_pointer_pos(Some(Point::new(thumb.center().x, thumb.center().y)));
        assert_eq!(
            editor.scrollbar_interaction_state(rect, available_height),
            InteractionState::Hover
        );

        // Pressed over the thumb -> Active.
        editor.set_pointer_pressed(true);
        assert_eq!(
            editor.scrollbar_interaction_state(rect, available_height),
            InteractionState::Active
        );

        // Pressed but pointer moved off the thumb (still over the track, near
        // the track bottom) -> Hover.
        let track_bottom = rect.y0 + TEXT_INSET_Y + available_height - 1.0;
        editor.set_pointer_pos(Some(Point::new(track_x, track_bottom)));
        assert_eq!(
            editor.scrollbar_interaction_state(rect, available_height),
            InteractionState::Hover
        );

        // Pointer off the track -> Rest (even while pressed).
        editor.set_pointer_pos(Some(Point::new(rect.x0, rect.y0)));
        assert_eq!(
            editor.scrollbar_interaction_state(rect, available_height),
            InteractionState::Rest
        );

        // Clearing chrome state -> Rest.
        editor.clear_pointer_chrome_state();
        assert_eq!(
            editor.scrollbar_interaction_state(rect, available_height),
            InteractionState::Rest
        );
    }

    #[test]
    fn editor_caret_selection_diagnostics_use_base_ui_colors() {
        // Plan 065 task 5: caret/selection/diagnostics stay on the editor
        // StyleRegistry (base.caret / base.selection / diagnostic_style), the
        // single source of color for editor paint — separate from SDUI typed
        // tokens. Source guard complementing the conformance color-literal scan.
        let source = include_str!("mod.rs");
        let non_test = source.split("mod tests").next().expect("non-test source");
        let paint_text = non_test
            .split("fn paint_text")
            .nth(1)
            .expect("paint_text body");
        assert!(
            paint_text.contains("base.caret"),
            "caret color must come from self.theme.base.caret"
        );
        assert!(
            paint_text.contains("base.selection"),
            "selection color must come from self.theme.base.selection"
        );
        assert!(
            paint_text.contains("diagnostic_style"),
            "diagnostic squiggle colors must come from diagnostic_style(severity)"
        );
    }

    fn generated_lines(line_count: usize) -> String {
        let mut text = String::new();
        for line in 0..line_count {
            writeln!(text, "line {line:05}").expect("writing to String cannot fail");
        }
        text
    }

    #[test]
    fn editor_visible_extraction_records_metric_when_enabled() {
        let perf = PerfRecorder::for_test(true);
        let mut editor = EditorSurface::default().with_perf_recorder(perf);
        editor.load_snapshot(
            1,
            2,
            "alpha\nbeta".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        assert_eq!(editor.visible_text(), "alpha\nbeta");

        let snapshots = editor.perf_snapshots();
        assert!(
            snapshots
                .iter()
                .any(|snapshot| snapshot.name == "editor.visible_extraction")
        );
        assert!(
            snapshots
                .iter()
                .any(|snapshot| snapshot.name == "editor.visible_extraction.bytes")
        );
    }

    #[test]
    fn editor_load_snapshot_replaces_text_and_resets_caret() {
        let mut editor = EditorSurface::default();
        editor.insert_text("local");
        editor.set_caret_for_test("local".len());
        editor.set_visual_scroll_bounds_for_test(100.0);
        assert!(editor.scroll_vertical_pixels(10.0));

        editor.load_snapshot(
            42,
            7,
            "server 🦀\ntext".to_string(),
            DocumentAccess::ReadOnly,
        );

        assert_eq!(editor.visible_text(), "server 🦀\ntext");
        assert_eq!(editor.caret_for_test(), 0);
        assert_eq!(editor.selection_for_test(), None);
        assert_eq!(editor.visual_scroll_y(), 0.0);
        assert_eq!(editor.document_state().document_id, 42);
        assert_eq!(editor.document_state().document_version, 7);
        assert_eq!(editor.document_state().access, DocumentAccess::ReadOnly);
    }

    #[test]
    fn move_word_start_command_advances_caret_via_manifest_policy() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "foo.bar baz".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        editor.set_caret_for_test(0);

        // Code policy (underscore joins): `foo` then `.` separator then `bar`.
        assert!(editor.command(EditorCommand::MoveWordStart {
            forward: true,
            long: false,
            extend: false,
        }));
        assert_eq!(editor.caret_for_test(), 4);

        // Extend to the next word start (`b` of `baz` at offset 8) selects 4..8.
        assert!(editor.command(EditorCommand::MoveWordStart {
            forward: true,
            long: false,
            extend: true,
        }));
        assert_eq!(editor.caret_for_test(), 8);
        assert_eq!(editor.selection_for_test(), Some((4, 8)));
    }

    #[test]
    fn move_matching_pair_command_toggles_brackets_via_manifest_pairs() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "({[]})".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        editor.set_caret_for_test(0);

        // `(` at 0 → matching `)` at 5.
        assert!(editor.command(EditorCommand::MoveMatchingPair { extend: false }));
        assert_eq!(editor.caret_for_test(), 5);
        // `)` at 5 → matching `(` at 0.
        assert!(editor.command(EditorCommand::MoveMatchingPair { extend: false }));
        assert_eq!(editor.caret_for_test(), 0);
    }

    #[test]
    fn select_word_selects_word_run_at_caret() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "foo.bar baz".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        // Caret inside "bar" (offset 4): selects "bar" (4..7).
        editor.set_caret_for_test(4);
        assert!(editor.command(EditorCommand::SelectWord));
        assert_eq!(editor.selection_for_test(), Some((4, 7)));
        assert_eq!(editor.caret_for_test(), 7);
    }

    #[test]
    fn select_word_noop_on_separator() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "foo.bar".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        // Caret on "." (offset 3): no word run → no-op.
        editor.set_caret_for_test(3);
        assert!(!editor.command(EditorCommand::SelectWord));
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn select_line_selects_line_content() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "hello\nworld".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        // Caret in "world" (offset 8): selects "world" (6..11).
        editor.set_caret_for_test(8);
        assert!(editor.command(EditorCommand::SelectLine));
        assert_eq!(editor.selection_for_test(), Some((6, 11)));
        assert_eq!(editor.caret_for_test(), 11);
    }

    #[test]
    fn select_paragraph_selects_non_blank_run() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "para1\nline2\n\npara2".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        // Caret in "line2" (offset 8): selects "para1\nline2" (0..11).
        editor.set_caret_for_test(8);
        assert!(editor.command(EditorCommand::SelectParagraph));
        assert_eq!(editor.selection_for_test(), Some((0, 11)));
        assert_eq!(editor.caret_for_test(), 11);
    }

    #[test]
    fn caret_blink_solid_is_always_visible() {
        let mut blink = CaretBlink::default();
        for delta in [0, 100, 1000, 5000] {
            blink.advance(&BlinkStyle::Solid, delta);
            assert!(blink.is_visible(), "Solid caret must never hide");
        }
    }

    #[test]
    fn caret_blink_cycles_wait_on_off() {
        let style = BlinkStyle::Blink {
            on_ms: 100,
            off_ms: 100,
            wait_ms: 50,
        };
        let mut blink = CaretBlink::default();
        assert!(blink.is_visible(), "caret starts visible");
        // After the 50ms wait the caret is On (visible).
        blink.advance(&style, 50);
        assert_eq!(blink.phase, BlinkPhase::On);
        assert!(blink.is_visible());
        // After the 100ms on-phase the caret is Off (hidden).
        blink.advance(&style, 100);
        assert_eq!(blink.phase, BlinkPhase::Off);
        assert!(!blink.is_visible());
        // After the 100ms off-phase the caret is On again.
        blink.advance(&style, 100);
        assert_eq!(blink.phase, BlinkPhase::On);
        assert!(blink.is_visible());
    }

    #[test]
    fn caret_blink_reset_returns_to_visible_wait() {
        let style = BlinkStyle::Blink {
            on_ms: 100,
            off_ms: 100,
            wait_ms: 50,
        };
        let mut blink = CaretBlink::default();
        blink.advance(&style, 150); // wait(50) + on(100) -> Off
        assert!(!blink.is_visible());
        blink.reset();
        assert_eq!(blink.phase, BlinkPhase::Wait);
        assert!(blink.is_visible(), "reset restarts visible + wait");
    }

    #[test]
    fn effective_caret_style_resolves_override_manifest_theme() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "text".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        // No manifest, no override: the editor StyleRegistry default (Bar).
        assert_eq!(editor.effective_caret_style().shape, CaretShape::Bar);

        // Per-mode manifest override wins over the theme default.
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.caret_style = Some(CaretStyle {
            shape: CaretShape::Block,
            ..CaretStyle::default()
        });
        editor.install_behavior_manifest(manifest);
        assert_eq!(editor.effective_caret_style().shape, CaretShape::Block);

        // Runtime clientSetCursorStyle override wins over the manifest.
        editor.set_caret_style_override(Some(CaretStyle {
            shape: CaretShape::Underline,
            ..CaretStyle::default()
        }));
        assert_eq!(editor.effective_caret_style().shape, CaretShape::Underline);

        // Clearing the override falls back to the manifest.
        editor.set_caret_style_override(None);
        assert_eq!(editor.effective_caret_style().shape, CaretShape::Block);
    }

    #[test]
    fn command_resets_blink_when_stop_blink_on_typing() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "text".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        // An animating style with stop_blink_on_typing (the default).
        editor.set_caret_style_override(Some(CaretStyle {
            blink: BlinkStyle::Blink {
                on_ms: 100,
                off_ms: 100,
                wait_ms: 50,
            },
            ..CaretStyle::default()
        }));
        // Drive the blink into its Off phase.
        editor.advance_blink(50); // Wait -> On (still visible, no change)
        assert!(editor.advance_blink(100)); // On -> Off (visibility flips)
        assert!(!editor.caret_blink_visible());
        // A user command resets the blink to visible.
        editor.command(EditorCommand::MoveRight);
        assert!(editor.caret_blink_visible(), "typing restarts the blink");
    }

    #[test]
    fn multi_selection_paint_data_renders_both_ranges() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "one two three".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        // Primary selection over "one", secondary over "three".
        editor.set_selection_for_test(0, 3);
        editor.add_selection_for_test(8, 13);
        assert_eq!(editor.selection_count_for_test(), 2);

        let snapshot = editor.visible_snapshot();
        let ranges = editor.visible_selection_ranges(&snapshot);
        assert_eq!(ranges.len(), 2, "both selections feed the paint path");
        // Ranges are visible-relative byte offsets.
        assert_eq!(ranges[0], 0..3);
        assert_eq!(ranges[1], 8..13);
    }

    #[test]
    fn multi_caret_paint_gates_primary_on_blink_secondary_solid() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "one two three".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.set_selection_for_test(0, 3);
        editor.add_selection_for_test(8, 13);
        editor.set_caret_style_override(Some(CaretStyle {
            blink: BlinkStyle::Blink {
                on_ms: 100,
                off_ms: 100,
                wait_ms: 50,
            },
            ..CaretStyle::default()
        }));

        // Drive the primary blink into its Off phase.
        editor.advance_blink(50); // Wait -> On
        editor.advance_blink(100); // On -> Off
        assert!(!editor.caret_should_paint(0), "primary hides on blink off");
        assert!(editor.caret_should_paint(1), "secondary stays solid");

        // Reset the blink: primary paints again, secondary unaffected.
        editor.command(EditorCommand::MoveRight);
        assert!(editor.caret_should_paint(0));
        assert!(editor.caret_should_paint(1));
    }

    #[test]
    fn ime_preedit_attaches_to_primary_caret_only() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "one two three".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        // Primary caret at byte 4 (inside "two"); a secondary selection exists.
        editor.set_caret_for_test(4);
        editor.add_selection_for_test(8, 13);
        assert_eq!(editor.selection_count_for_test(), 2);

        let snapshot = editor.visible_snapshot();
        // The IME caret offset follows the PRIMARY caret, not the secondary.
        let caret_offset = editor.visible_caret_offset(&snapshot);
        assert_eq!(caret_offset, Some(4));
    }

    // ------------------------------------------------------------------
    // Plan 071 task 9: multi-cursor commands.
    // ------------------------------------------------------------------

    fn multi_cursor_editor(text: &str) -> EditorSurface {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            text.to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        editor
    }

    #[test]
    fn add_cursor_below_creates_two_carets_and_typing_inserts_at_both() {
        let mut editor = multi_cursor_editor("aa\nbb\n");
        editor.set_caret_for_test(1);

        assert!(editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Down,
        }));
        assert_eq!(editor.selection_count_for_test(), 2);
        // Primary is the newly added caret on line 2, same column.
        assert_eq!(editor.caret_for_test(), 4);

        // Typing inserts at both carets.
        let outcome = editor.insert_text_with_event("X");
        assert!(outcome.changed);
        assert_eq!(editor.buffer_text_for_test(), "aXa\nbXb\n");
        assert_eq!(outcome.edit_events.len(), 2, "one edit event per caret");
        // Both carets survive the edit, advanced past the inserted char.
        assert_eq!(editor.selection_count_for_test(), 2);
    }

    #[test]
    fn add_cursor_refuses_to_stack_on_same_line_or_past_edges() {
        let mut editor = multi_cursor_editor("aa\nbb\n");
        editor.set_caret_for_test(1);
        assert!(!editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Up,
        }));
        assert!(editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Down,
        }));
        // A second press targets line 2, which already has a caret -> no-op.
        assert!(!editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Down,
        }));
        assert_eq!(editor.selection_count_for_test(), 2);
    }

    #[test]
    fn select_next_match_selects_word_then_next_occurrences_and_wraps() {
        let mut editor = multi_cursor_editor("foo bar foo baz foo");
        editor.set_caret_for_test(1);

        // First press selects the word under the caret.
        assert!(editor.command(EditorCommand::SelectNextMatch));
        assert_eq!(editor.selection_for_test(), Some((0, 3)));
        assert_eq!(editor.selection_count_for_test(), 1);

        // Second press adds the next occurrence as a new primary selection.
        assert!(editor.command(EditorCommand::SelectNextMatch));
        assert_eq!(editor.selection_count_for_test(), 2);
        assert_eq!(editor.selection_for_test(), Some((8, 11)));

        // Third press adds the last occurrence.
        assert!(editor.command(EditorCommand::SelectNextMatch));
        assert_eq!(editor.selection_count_for_test(), 3);
        assert_eq!(editor.selection_for_test(), Some((16, 19)));

        // All occurrences selected -> next press wraps and finds nothing new.
        assert!(!editor.command(EditorCommand::SelectNextMatch));
        assert_eq!(editor.selection_count_for_test(), 3);
    }

    #[test]
    fn select_prev_match_walks_backwards_and_wraps() {
        let mut editor = multi_cursor_editor("foo bar foo baz foo");
        editor.set_selection_for_test(8, 11);

        assert!(editor.command(EditorCommand::SelectPrevMatch));
        assert_eq!(editor.selection_for_test(), Some((0, 3)));
        assert_eq!(editor.selection_count_for_test(), 2);

        // Wraps to the last occurrence.
        assert!(editor.command(EditorCommand::SelectPrevMatch));
        assert_eq!(editor.selection_for_test(), Some((16, 19)));
        assert_eq!(editor.selection_count_for_test(), 3);
    }

    #[test]
    fn select_all_matches_selects_every_occurrence_and_copy_unions_them() {
        let mut editor = multi_cursor_editor("foo bar foo baz foo");
        editor.set_caret_for_test(1);

        assert!(editor.command(EditorCommand::SelectAllMatches));
        assert_eq!(editor.selection_count_for_test(), 3);
        // The occurrence containing the original caret is primary.
        assert_eq!(editor.selection_for_test(), Some((0, 3)));
        // Copy unions every range in document order.
        assert_eq!(editor.selected_text().as_deref(), Some("foo\nfoo\nfoo"));
    }

    #[test]
    fn column_select_down_grows_box_and_left_right_moves_all_carets() {
        let mut editor = multi_cursor_editor("abcd\nefgh\n");
        editor.set_caret_for_test(1);

        assert!(editor.command(EditorCommand::ColumnSelect {
            direction: CursorSelectDirection::Down,
        }));
        assert_eq!(editor.selection_count_for_test(), 2);
        assert_eq!(editor.caret_for_test(), 6);

        // Right moves every caret one scalar.
        assert!(editor.command(EditorCommand::ColumnSelect {
            direction: CursorSelectDirection::Right,
        }));
        assert_eq!(editor.selection_count_for_test(), 2);
        assert_eq!(editor.caret_for_test(), 7);
        let focus_positions: Vec<usize> = editor.selection_focus_positions_for_test();
        assert_eq!(focus_positions, vec![2, 7]);

        // Left moves them back.
        assert!(editor.command(EditorCommand::ColumnSelect {
            direction: CursorSelectDirection::Left,
        }));
        assert_eq!(editor.selection_focus_positions_for_test(), vec![1, 6]);
    }

    #[test]
    fn cancel_multiple_selections_collapses_to_primary_caret() {
        let mut editor = multi_cursor_editor("foo bar foo");
        editor.set_selection_for_test(0, 3);
        editor.add_selection_for_test(8, 11);
        assert_eq!(editor.selection_count_for_test(), 2);

        assert!(editor.command(EditorCommand::CancelMultipleSelections));
        assert_eq!(editor.selection_count_for_test(), 1);
        assert!(!editor.has_selection());
    }

    #[test]
    fn keep_and_remove_primary_follow_helix_semantics() {
        let mut editor = multi_cursor_editor("foo bar foo");
        editor.set_selection_for_test(0, 3);
        editor.add_selection_for_test(8, 11);

        // Keep: only the primary (with its range) survives.
        assert!(editor.command(EditorCommand::KeepSelection));
        assert_eq!(editor.selection_count_for_test(), 1);
        assert_eq!(editor.selection_for_test(), Some((0, 3)));

        // Rebuild two selections; remove-primary drops the primary.
        editor.add_selection_for_test(8, 11);
        assert!(editor.command(EditorCommand::RemoveSelection));
        assert_eq!(editor.selection_count_for_test(), 1);
        assert_eq!(editor.selection_for_test(), Some((8, 11)));

        // Removing the last selection is a no-op.
        assert!(!editor.command(EditorCommand::RemoveSelection));
        assert_eq!(editor.selection_count_for_test(), 1);
    }

    #[test]
    fn cursor_undo_restores_previous_selection_set() {
        let mut editor = multi_cursor_editor("foo bar foo");
        editor.set_caret_for_test(0);

        assert!(editor.command(EditorCommand::MoveRight));
        // single-line doc: no-op, still snapshots dedup-safe
        assert!(!editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Down,
        }));
        assert!(editor.command(EditorCommand::SelectAllMatches));
        assert_eq!(editor.selection_count_for_test(), 2);

        // Ctrl+U walks the set back through the snapshots.
        assert!(editor.command(EditorCommand::UndoCursorMove));
        assert_eq!(editor.selection_count_for_test(), 1);
        assert!(editor.command(EditorCommand::UndoCursorMove));
        assert_eq!(editor.caret_for_test(), 0);
        // Stack exhausted -> unchanged.
        assert!(!editor.command(EditorCommand::UndoCursorMove));
    }

    #[test]
    fn multi_caret_typing_undoes_as_one_step() {
        let mut editor = multi_cursor_editor("aa\nbb\n");
        editor.set_caret_for_test(0);
        assert!(editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Down,
        }));

        assert!(editor.insert_text_with_event("X").changed);
        assert_eq!(editor.buffer_text_for_test(), "Xaa\nXbb\n");

        // One undo reverses every caret's insert and restores both carets.
        assert!(editor.undo_with_event().changed);
        assert_eq!(editor.buffer_text_for_test(), "aa\nbb\n");
        assert_eq!(editor.selection_count_for_test(), 2);

        // Redo re-applies the combined step.
        assert!(editor.redo_with_event().changed);
        assert_eq!(editor.buffer_text_for_test(), "Xaa\nXbb\n");
        assert_eq!(editor.selection_count_for_test(), 2);
    }

    #[test]
    fn multi_caret_backspace_deletes_at_every_caret() {
        let mut editor = multi_cursor_editor("Xa\nXb\n");
        editor.set_caret_for_test(1);
        assert!(editor.command(EditorCommand::AddCursor {
            direction: CursorSelectDirection::Down,
        }));
        assert_eq!(editor.caret_for_test(), 4);

        assert!(editor.backspace_with_event().changed);
        assert_eq!(editor.buffer_text_for_test(), "a\nb\n");
        assert_eq!(editor.selection_count_for_test(), 2);
    }

    #[test]
    fn editor_installs_minimal_behavior_manifest() {
        let mut editor = EditorSurface::default();
        let manifest = BehaviorManifest::minimal_text_editing(5);

        editor.install_behavior_manifest(manifest.clone());

        assert_eq!(editor.document_state().behavior_version, 5);
        assert_eq!(editor.document_state().behavior_manifest, Some(manifest));
    }

    #[test]
    fn insert_command_emits_insert_operation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            42,
            7,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(9));
        editor.set_caret_for_test(1);

        let outcome = editor.command_with_event(EditorCommand::Insert("X"));

        assert!(outcome.changed);
        let event = outcome.edit_event.expect("editable manifest emits edits");
        assert_eq!(event.document_id, 42);
        assert_eq!(event.base_version, 7);
        assert_eq!(event.behavior_version, 9);
        assert_eq!(
            event.operation,
            EditOperation::Insert {
                byte_offset: 1,
                text: "X".to_string()
            }
        );
        assert_eq!(editor.visible_text(), "aXb");
    }

    #[test]
    fn undo_insert_delete_replace_restores_text_and_caret() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            3,
            "abc".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        editor.set_caret_for_test(1);

        assert!(editor.insert_text_with_event("X").changed);
        assert_eq!(editor.visible_text(), "aXbc");
        assert_eq!(editor.caret_for_test(), 2);

        let undone = editor.undo_with_event();
        assert!(undone.changed);
        assert_eq!(
            undone
                .edit_event
                .expect("undo emits inverse edit")
                .operation,
            EditOperation::Delete { start: 1, end: 2 }
        );
        assert_eq!(editor.visible_text(), "abc");
        assert_eq!(editor.caret_for_test(), 1);

        editor.set_selection_for_test(1, 2);
        assert!(editor.insert_text_with_event("YZ").changed);
        assert_eq!(editor.visible_text(), "aYZc");
        let undone_replace = editor.undo_with_event();
        assert!(undone_replace.changed);
        assert_eq!(editor.visible_text(), "abc");
        assert_eq!(editor.selection_for_test(), Some((1, 2)));

        editor.set_selection_for_test(3, 3);
        assert!(editor.backspace_with_event().changed);
        assert_eq!(editor.visible_text(), "ab");
        assert!(editor.undo_with_event().changed);
        assert_eq!(editor.visible_text(), "abc");
        assert_eq!(editor.caret_for_test(), 3);
    }

    #[test]
    fn redo_restores_undone_edit_and_new_edit_clears_redo() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            3,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        editor.set_caret_for_test(2);
        assert!(editor.insert_text_with_event("!").changed);
        assert!(editor.undo_with_event().changed);
        assert!(editor.history_for_test().can_redo());

        let redone = editor.redo_with_event();
        assert!(redone.changed);
        assert_eq!(editor.visible_text(), "ab!");
        assert_eq!(editor.caret_for_test(), 3);
        assert!(!editor.history_for_test().can_redo());

        assert!(editor.undo_with_event().changed);
        assert!(editor.insert_text_with_event("?").changed);
        assert!(!editor.history_for_test().can_redo());
        assert!(!editor.redo_with_event().changed);
        assert_eq!(editor.visible_text(), "ab?");
    }

    #[test]
    fn read_only_observer_undo_is_noop_and_resync_clears_history() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            3,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        editor.set_caret_for_test(2);
        assert!(editor.insert_text_with_event("x").changed);
        assert!(editor.history_for_test().can_undo());

        editor.load_resync_snapshot(7, 4, "abx".to_string(), DocumentAccess::ReadOnly);
        assert!(!editor.history_for_test().can_undo());
        assert!(!editor.undo_with_event().changed);
        assert_eq!(editor.visible_text(), "abx");
    }

    #[test]
    fn edit_event_carries_behavior_version() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(99));

        let outcome = editor.insert_text_with_event("x");

        assert_eq!(outcome.edit_event.unwrap().behavior_version, 99);
    }

    #[test]
    fn editor_routes_client_first_key_through_manifest() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));

        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("x".to_string())));

        assert!(outcome.command_outcome.changed);
        assert_eq!(outcome.server_intent, None);
        assert_eq!(editor.visible_text(), "x");
        assert_eq!(
            outcome.command_outcome.edit_event.unwrap().behavior_version,
            3
        );
    }

    fn editor_installs_two_stroke_chord_manifest(behavior_version: u32) -> EditorSurface {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(behavior_version.into());
        // `controlCenter.open` is already declared in the default manifest.
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "controlCenter.open".to_string(),
            sequence: vec![g.clone(), g.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        editor.install_behavior_manifest(manifest);
        editor
    }

    fn editor_installs_generated_chord_manifest(behavior_version: u32) -> EditorSurface {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(behavior_version.into());
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh",
            "Refresh Workspace",
        ));
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        let a = KeyStroke::new(KeyCode::Character("a".to_string()));
        let b = KeyStroke::new(KeyCode::Character("b".to_string()));
        let c = KeyStroke::new(KeyCode::Character("c".to_string()));
        manifest.keymaps.extend([
            KeyBindingRule {
                command_id: "controlCenter.open".to_string(),
                sequence: vec![g.clone(), g],
                context: KeyBindingContext::Global,
                routing_policy: RoutingPolicy::ServerFirst,
            },
            KeyBindingRule {
                command_id: "workspace.refresh".to_string(),
                sequence: vec![a, b, c],
                context: KeyBindingContext::Global,
                routing_policy: RoutingPolicy::ServerFirst,
            },
        ]);
        editor.install_behavior_manifest(manifest);
        editor
    }

    #[test]
    fn editor_pending_chord_consumes_strokes_and_dispatches_on_completion() {
        let mut editor = editor_installs_two_stroke_chord_manifest(3);
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));

        // First stroke: pending — consumed, no text inserted, no dispatch.
        let first = editor.route_key_with_event(&g);
        assert!(first.consumed);
        assert!(!first.command_outcome.changed);
        assert_eq!(editor.visible_text(), "");
        assert_eq!(editor.pending_chord.as_ref().unwrap().strokes.len(), 1);

        // Second stroke: exact match dispatches through the server-intent
        // lane and clears the pending chord.
        let second = editor.route_key_with_event(&g);
        assert!(!second.consumed);
        assert!(matches!(
            second.server_intent,
            Some(crate::client::behavior::ServerIntentRoute {
                ref command_id,
                routing_policy: RoutingPolicy::ServerFirst,
            }) if command_id == "controlCenter.open"
        ));
        assert!(editor.pending_chord.is_none());
        assert_eq!(editor.visible_text(), "");
    }

    #[test]
    fn editor_abandoned_chord_does_not_eat_the_next_key() {
        let mut editor = editor_installs_two_stroke_chord_manifest(3);
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));

        assert!(editor.route_key_with_event(&g).consumed);
        // Abandon the chord: the unrelated key is re-evaluated fresh and
        // inserts its text; the prefix "g" is never inserted.
        let next =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("z".to_string())));
        assert!(!next.consumed);
        assert!(next.command_outcome.changed);
        assert_eq!(editor.visible_text(), "z");
        assert!(editor.pending_chord.is_none());
    }

    #[test]
    fn editor_stale_pending_chord_cancels_on_the_next_key() {
        let mut editor = editor_installs_two_stroke_chord_manifest(3);
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));

        assert!(editor.route_key_with_event(&g).consumed);
        // Backdate the pending chord past the timeout.
        editor.pending_chord.as_mut().unwrap().started_at = std::time::Instant::now()
            - std::time::Duration::from_millis(KEY_CHORD_PENDING_TIMEOUT_MS + 1);
        // The stale chord cancels; the incoming key is re-evaluated fresh
        // (unbound "x" inserts its text) and the chord state is cleared.
        let next =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("x".to_string())));
        assert!(!next.consumed);
        assert!(next.command_outcome.changed);
        assert_eq!(editor.visible_text(), "x");
        assert!(editor.pending_chord.is_none());
    }

    #[test]
    fn editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions() {
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        let a = KeyStroke::new(KeyCode::Character("a".to_string()));
        let b = KeyStroke::new(KeyCode::Character("b".to_string()));
        let c = KeyStroke::new(KeyCode::Character("c".to_string()));

        // Fixed seeds deliberately cover complete, mismatch, timeout, and
        // three-stroke paths without introducing a property-testing crate.
        for seed in 0..128_u32 {
            let mut editor = editor_installs_generated_chord_manifest(seed + 1);
            match seed % 4 {
                0 => {
                    assert!(editor.route_key_with_event(&g).consumed);
                    let outcome = editor.route_key_with_event(&g);
                    assert!(matches!(
                        outcome.server_intent,
                        Some(crate::client::behavior::ServerIntentRoute {
                            ref command_id,
                            routing_policy: RoutingPolicy::ServerFirst,
                        }) if command_id == "controlCenter.open"
                    ));
                    assert!(!outcome.consumed);
                }
                1 => {
                    assert!(editor.route_key_with_event(&g).consumed);
                    let outcome = editor
                        .route_key_with_event(&KeyStroke::new(KeyCode::Character("x".to_string())));
                    assert!(outcome.command_outcome.changed);
                    assert_eq!(editor.visible_text(), "x");
                }
                2 => {
                    assert!(editor.route_key_with_event(&g).consumed);
                    editor.pending_chord.as_mut().unwrap().started_at = std::time::Instant::now()
                        - std::time::Duration::from_millis(KEY_CHORD_PENDING_TIMEOUT_MS + 1);
                    let outcome = editor
                        .route_key_with_event(&KeyStroke::new(KeyCode::Character("z".to_string())));
                    assert!(outcome.command_outcome.changed);
                    assert_eq!(editor.visible_text(), "z");
                }
                _ => {
                    assert!(editor.route_key_with_event(&a).consumed);
                    assert!(editor.route_key_with_event(&b).consumed);
                    let outcome = editor.route_key_with_event(&c);
                    assert!(matches!(
                        outcome.server_intent,
                        Some(crate::client::behavior::ServerIntentRoute {
                            ref command_id,
                            routing_policy: RoutingPolicy::ServerFirst,
                        }) if command_id == "workspace.refresh"
                    ));
                    assert!(!outcome.consumed);
                }
            }
            assert!(
                editor.pending_chord.is_none(),
                "seed {seed} left a chord pending"
            );
            assert!(
                editor.visible_text().len() <= 1,
                "seed {seed} inserted too much text"
            );
        }
    }

    #[test]
    fn editor_pending_chord_buffer_never_exceeds_longest_bound_sequence() {
        // Phase 24.5: the pending buffer grows by one validated stroke per
        // Pending outcome, and the matcher reports Pending only while the
        // candidate is a strict prefix of some rule, so the buffer can never
        // exceed the longest bound sequence.
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh",
            "Refresh Workspace",
        ));
        let a = KeyStroke::new(KeyCode::Character("a".to_string()));
        let b = KeyStroke::new(KeyCode::Character("b".to_string()));
        let c = KeyStroke::new(KeyCode::Character("c".to_string()));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.refresh".to_string(),
            sequence: vec![a.clone(), b.clone(), c.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        editor.install_behavior_manifest(manifest);
        let longest = 3usize;

        assert!(editor.route_key_with_event(&a).consumed);
        assert!(editor.route_key_with_event(&b).consumed);
        assert_eq!(editor.pending_chord.as_ref().unwrap().strokes.len(), 2);
        assert!(
            editor.pending_chord.as_ref().unwrap().strokes.len() <= longest,
            "pending buffer must never exceed the longest bound sequence"
        );

        // The completing stroke dispatches and clears the buffer.
        let done = editor.route_key_with_event(&c);
        assert!(!done.consumed);
        assert!(matches!(
            done.server_intent,
            Some(crate::client::behavior::ServerIntentRoute {
                ref command_id,
                routing_policy: RoutingPolicy::ServerFirst,
            }) if command_id == "workspace.refresh"
        ));
        assert!(editor.pending_chord.is_none());
    }

    #[test]
    fn editor_requests_completion_while_typing_a_word() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));

        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("p".to_string())));
        let completion = outcome
            .completion_request
            .expect("word typing requests completion");

        assert_eq!(completion.cursor_byte_offset, 1);
        assert_eq!(completion.replacement_range.byte_start, 0);
        assert_eq!(completion.replacement_range.byte_end, 1);
        assert_eq!(completion.trigger, CompletionTrigger::Manual);
    }

    #[test]
    fn editor_routes_autocomplete_trigger_after_local_edit() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "value".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_caret_for_test("value".len());

        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character(".".to_string())));

        assert!(outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "value.");
        let completion = outcome
            .completion_request
            .expect("trigger requests completion");
        assert_eq!(completion.document_id, 7);
        assert_eq!(completion.document_version, 12);
        assert_eq!(completion.behavior_version, 3);
        assert_eq!(completion.cursor_byte_offset, 6);
        assert_eq!(completion.replacement_range.byte_start, 6);
        assert_eq!(completion.replacement_range.byte_end, 6);
        assert_eq!(
            completion.trigger,
            CompletionTrigger::Character(".".to_string())
        );
    }

    #[test]
    fn editor_accepts_completion_as_local_replacement() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "pri".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        let action = CompletionMenuAcceptAction {
            request_id: 1,
            document_id: 7,
            document_version: 12,
            behavior_version: 3,
            replacement_range: crate::protocol::CompletionReplacementRange::new(0, 3),
            insert_text: "println!".to_string(),
            text_format: CompletionItemTextFormat::PlainText,
            commit_characters: ";".to_string(),
        };

        let outcome = editor.accept_completion_with_event(&action, Some(";"));

        assert!(outcome.changed);
        assert_eq!(editor.visible_text(), "println!;");
        assert_eq!(
            outcome.edit_event.unwrap().operation,
            EditOperation::Replace {
                start: 0,
                end: 3,
                text: "println!;".to_string()
            }
        );
        assert!(!editor.has_active_snippet_session());
    }

    #[test]
    fn editor_accepts_snippet_as_local_expansion_and_selects_first_placeholder() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "pri".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));

        let outcome = editor.accept_completion_with_event(
            &snippet_completion_action("fn ${1:name}(${2:args}) {\n\t$0\n}"),
            None,
        );

        assert_eq!(editor.visible_text(), "fn name(args) {\n\t\n}");
        assert_eq!(editor.selection_for_test(), Some((3, 7)));
        assert!(editor.has_active_snippet_session());
        assert_eq!(
            outcome.edit_event.unwrap().operation,
            EditOperation::Replace {
                start: 0,
                end: 3,
                text: "fn name(args) {\n\t\n}".to_string(),
            }
        );
    }

    #[test]
    fn snippet_tab_navigation_moves_forward_backward_and_ends_at_final_tabstop() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "pri".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.accept_completion_with_event(
            &snippet_completion_action("fn ${1:name}(${2:args}) {\n\t$0\n}"),
            None,
        );

        editor.route_key_with_event(&KeyStroke::new(KeyCode::Tab));
        assert_eq!(editor.selection_for_test(), Some((8, 12)));

        editor.route_key_with_event(&KeyStroke {
            key: KeyCode::Tab,
            modifiers: KeyModifiers {
                shift: true,
                ..KeyModifiers::NONE
            },
        });
        assert_eq!(editor.selection_for_test(), Some((3, 7)));

        editor.route_key_with_event(&KeyStroke::new(KeyCode::Tab));
        editor.route_key_with_event(&KeyStroke::new(KeyCode::Tab));
        assert!(!editor.has_active_snippet_session());
        assert_eq!(editor.caret_for_test(), 17);
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn snippet_escape_exits_session_without_an_edit() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "pri".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor
            .accept_completion_with_event(&snippet_completion_action("fn ${1:name}() {$0}"), None);

        let outcome = editor.route_key_with_event(&KeyStroke::new(KeyCode::Escape));

        assert!(outcome.command_outcome.changed);
        assert_eq!(outcome.command_outcome.edit_event, None);
        assert!(!editor.has_active_snippet_session());
        assert_eq!(editor.visible_text(), "fn name() {}");
    }

    #[test]
    fn editing_active_placeholder_shifts_later_snippet_ranges() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "pri".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.accept_completion_with_event(
            &snippet_completion_action("fn ${1:name}(${2:args}) {\n\t$0\n}"),
            None,
        );

        editor.insert_text_with_event("x");
        editor.route_key_with_event(&KeyStroke::new(KeyCode::Tab));

        assert_eq!(editor.visible_text(), "fn x(args) {\n\t\n}");
        assert_eq!(editor.selection_for_test(), Some((5, 9)));
        assert_eq!(editor.selected_text().as_deref(), Some("args"));
    }

    #[test]
    fn editor_routes_manual_completion_without_text_mutation() {
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.keymaps.push(KeyBindingRule {
            command_id: "completion.trigger".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character(" ".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::UiReactivePriority,
        });
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            7,
            12,
            "hello value".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(manifest);
        editor.set_caret_for_test("hello value".len());

        let outcome = editor.route_key_with_event(&KeyStroke {
            key: KeyCode::Character(" ".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert!(!outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "hello value");
        let completion = outcome.completion_request.expect("manual request");
        assert_eq!(completion.trigger, CompletionTrigger::Manual);
        assert_eq!(completion.cursor_byte_offset, "hello value".len() as u64);
        assert_eq!(
            completion.replacement_range.byte_start,
            "hello ".len() as u64
        );
        assert_eq!(
            completion.replacement_range.byte_end,
            "hello value".len() as u64
        );
    }

    #[test]
    fn editor_routes_server_first_key_without_local_mutation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.save",
            "Save Workspace File",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.save".to_string(),
            sequence: vec![KeyStroke::new(KeyCode::Character("s".to_string()))],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        editor.install_behavior_manifest(manifest);

        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("s".to_string())));

        assert!(!outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "");
        assert_eq!(outcome.server_intent.unwrap().command_id, "workspace.save");
    }

    #[test]
    fn editor_routes_client_ui_command_without_local_mutation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.commands.push(CommandDeclaration::client_ui(
            "documents.clientOpenFileDialog",
            "Open File Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "documents.clientOpenFileDialog".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("o".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        editor.install_behavior_manifest(manifest);

        let outcome = editor.route_key_with_event(&KeyStroke {
            key: KeyCode::Character("o".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert!(!outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "");
        assert_eq!(outcome.server_intent, None);
        assert_eq!(
            outcome.client_ui_command.unwrap().command_id,
            "documents.clientOpenFileDialog"
        );
    }

    #[test]
    fn editor_routes_rebound_move_cursor_command_to_client_ui() {
        // Plan 071 task 5: rebinding Ctrl+Right to a direction-specific
        // `clientMoveCursor.*` command ID routes it as a ClientUiCommand
        // (client-local, no document mutation).
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "para one\n\npara two".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.commands.push(CommandDeclaration::client_ui(
            "editor.clientMoveCursor.nextParagraph",
            "Next Paragraph",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "editor.clientMoveCursor.nextParagraph".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::ArrowRight,
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        editor.install_behavior_manifest(manifest);
        editor.set_caret_for_test(0);

        let outcome = editor.route_key_with_event(&KeyStroke {
            key: KeyCode::ArrowRight,
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        // The chord is routed to the client UI layer, not mutated locally here;
        // main.rs dispatches it to EditorWidget::apply_editor_client_command.
        assert!(!outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "para one\n\npara two");
        assert_eq!(
            outcome.client_ui_command.unwrap().command_id,
            "editor.clientMoveCursor.nextParagraph"
        );
    }

    #[test]
    fn editor_routes_rebound_multi_cursor_command_to_client_ui() {
        // Plan 071 task 9: rebinding a chord to an allowlisted multi-cursor
        // command ID routes it as a ClientUiCommand (client-local view state).
        let mut editor = multi_cursor_editor("foo bar foo");
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.commands.push(CommandDeclaration::client_ui(
            "editor.clientSelectAllMatches",
            "Select All Matches",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "editor.clientSelectAllMatches".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("m".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        editor.install_behavior_manifest(manifest);
        editor.set_caret_for_test(1);

        let outcome = editor.route_key_with_event(&KeyStroke {
            key: KeyCode::Character("m".to_string()),
            modifiers: KeyModifiers {
                control: true,
                shift: true,
                ..KeyModifiers::NONE
            },
        });

        assert!(!outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "foo bar foo");
        assert_eq!(
            outcome.client_ui_command.unwrap().command_id,
            "editor.clientSelectAllMatches"
        );
    }

    #[test]
    fn editor_routes_textobject_command_as_ui_reactive_server_intent() {
        // Plan 071 task 10: textobject/smart-select command IDs bind with
        // UiReactivePriority + ServerIntent authority (bindKey's auto-
        // declaration); routing hands the widget a server intent so it can
        // capture the selection set locally and query the server read-only.
        let mut editor = multi_cursor_editor("foo bar foo");
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.commands.push(CommandDeclaration::ui_reactive(
            "editor.clientSmartSelect.expand",
            "Expand Selection",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "editor.clientSmartSelect.expand".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("\\".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::UiReactivePriority,
        });
        editor.install_behavior_manifest(manifest);
        editor.set_caret_for_test(1);

        let outcome = editor.route_key_with_event(&KeyStroke {
            key: KeyCode::Character("\\".to_string()),
            modifiers: KeyModifiers {
                control: true,
                shift: true,
                ..KeyModifiers::NONE
            },
        });

        // Read-only query: no local mutation; the intent reaches the widget.
        assert!(!outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "foo bar foo");
        assert_eq!(
            outcome.server_intent.unwrap().command_id,
            "editor.clientSmartSelect.expand"
        );
    }

    #[test]
    fn selection_query_request_captures_every_caret_and_apply_installs_ranges() {
        // Plan 071 task 10: the request carries the whole selection set and
        // applying a result installs one selection per requested caret.
        let mut editor = multi_cursor_editor("foo bar foo\nfoo");
        editor.set_selection_for_test(0, 3);
        editor.selections.push_selection(Selection::collapsed(9));

        let event = editor
            .selection_query_request_for(crate::protocol::SelectionQuery::SmartSelect {
                action: crate::protocol::SmartSelectAction::Expand,
            })
            .expect("request captured");
        assert_eq!(
            event.selections,
            vec![
                crate::protocol::SelectionQueryCursor {
                    anchor: 0,
                    focus: 3
                },
                crate::protocol::SelectionQueryCursor {
                    anchor: 9,
                    focus: 9
                },
            ]
        );

        // Applying ranges keeps the primary index and replaces each selection.
        editor.apply_selection_query_result(vec![Selection::new(0, 7), Selection::new(8, 11)]);
        assert_eq!(editor.selection_count_for_test(), 2);
        assert_eq!(editor.selection_for_test(), Some((0, 7)));
        // Cursor-undo restores the pre-query set.
        assert!(editor.command(EditorCommand::UndoCursorMove));
        assert_eq!(editor.selection_for_test(), Some((0, 3)));
    }

    #[test]
    fn enter_rule_preserves_leading_indentation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "    child".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_caret_for_test("    child".len());

        let outcome = editor.route_key_with_event(&KeyStroke::new(KeyCode::Enter));

        assert!(outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "    child\n    ");
        assert_eq!(editor.caret_for_test(), "    child\n    ".len());
        assert_eq!(
            outcome.command_outcome.edit_event.unwrap().operation,
            EditOperation::Insert {
                byte_offset: "    child".len() as u64,
                text: "\n    ".to_string(),
            }
        );
    }

    #[test]
    fn tab_rule_inserts_configured_spaces() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let mut manifest = BehaviorManifest::minimal_text_editing(3);
        manifest.editor_rules.tab.mode = TabMode::InsertSpaces;
        manifest.editor_rules.tab.spaces_per_tab = 2;
        editor.install_behavior_manifest(manifest);

        let outcome = editor.route_key_with_event(&KeyStroke::new(KeyCode::Tab));

        assert!(outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "  ");
        assert_eq!(
            outcome.command_outcome.edit_event.unwrap().operation,
            EditOperation::Insert {
                byte_offset: 0,
                text: "  ".to_string(),
            }
        );
    }

    #[test]
    fn pair_rule_wraps_selection_or_inserts_pair() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_caret_for_test(1);

        let caret_outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("(".to_string())));

        assert!(caret_outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "a()b");
        assert_eq!(editor.caret_for_test(), 2);
        assert_eq!(
            caret_outcome.command_outcome.edit_event.unwrap().operation,
            EditOperation::Insert {
                byte_offset: 1,
                text: "()".to_string(),
            }
        );

        editor.set_selection_for_test(1, 3);
        let selection_outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("[".to_string())));

        assert!(selection_outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "a[()]b");
        assert_eq!(
            selection_outcome
                .command_outcome
                .edit_event
                .unwrap()
                .operation,
            EditOperation::Replace {
                start: 1,
                end: 3,
                text: "[()]".to_string(),
            }
        );
    }

    #[test]
    fn comment_continuation_rule_continues_simple_comment_prefix() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "  // note".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_caret_for_test("  // note".len());

        let outcome = editor.route_key_with_event(&KeyStroke::new(KeyCode::Enter));

        assert!(outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "  // note\n  // ");
        assert_eq!(
            outcome.command_outcome.edit_event.unwrap().operation,
            EditOperation::Insert {
                byte_offset: "  // note".len() as u64,
                text: "\n  // ".to_string(),
            }
        );
    }

    #[test]
    fn selection_replacement_emits_replace_operation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "abcdef".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_selection_for_test(2, 5);

        let outcome = editor.insert_text_with_event("XY");

        assert!(outcome.changed);
        assert_eq!(editor.visible_text(), "abXYf");
        assert_eq!(
            outcome.edit_event.unwrap().operation,
            EditOperation::Replace {
                start: 2,
                end: 5,
                text: "XY".to_string()
            }
        );
    }

    #[test]
    fn backspace_emits_delete_operation_at_unicode_boundary() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "a🦀b".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_caret_for_test("a🦀".len());

        let outcome = editor.backspace_with_event();

        assert!(outcome.changed);
        assert_eq!(editor.visible_text(), "ab");
        assert_eq!(
            outcome.edit_event.unwrap().operation,
            EditOperation::Delete {
                start: 1,
                end: "a🦀".len() as u64
            }
        );
    }

    #[test]
    fn selected_text_returns_forward_backward_unicode_ranges() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "alpha 🦀 beta".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        editor.set_selection_for_test(0, "alpha".len());
        assert_eq!(editor.selected_text().as_deref(), Some("alpha"));

        let start = "alpha ".len();
        let end = "alpha 🦀".len();
        editor.set_selection_for_test(end, start);
        assert_eq!(editor.selected_text().as_deref(), Some("🦀"));
    }

    #[test]
    fn selected_text_returns_none_for_collapsed_selection() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "abc".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        editor.set_selection_for_test(1, 1);

        assert_eq!(editor.selected_text(), None);
    }

    #[test]
    fn paste_text_inserts_at_caret_and_replaces_selection() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "abc".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        let inserted = editor.paste_text_with_event("X\nY");
        assert!(inserted.changed);
        assert_eq!(editor.visible_text(), "X\nYabc");

        editor.set_selection_for_test(0, 3);
        let replaced = editor.paste_text_with_event("Z");
        assert!(replaced.changed);
        assert_eq!(editor.visible_text(), "Zabc");
    }

    #[test]
    fn paste_text_normalizes_crlf_and_rejects_controls() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "abc".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        assert!(editor.paste_text_with_event("a\r\nb").changed);
        assert_eq!(editor.visible_text(), "a\nbabc");
        assert!(!editor.paste_text_with_event("").changed);
        assert!(!editor.paste_text_with_event("a\0b").changed);
        assert_eq!(editor.visible_text(), "a\nbabc");
    }

    #[test]
    fn preedit_does_not_change_canonical_text_or_enqueue_edits() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "hello".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.set_preedit("あ".into(), Some((0, 3))));
        assert!(editor.is_composing());
        assert_eq!(editor.preedit_text(), Some("あ"));
        assert_eq!(editor.visible_text(), "hello");
        assert_eq!(editor.history_for_test().undo_len(), 0);
        assert!(editor.set_preedit(String::new(), None));
        assert!(!editor.is_composing());
        assert_eq!(editor.visible_text(), "hello");
    }

    #[test]
    fn empty_preedit_clears_overlay_without_edit() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "x".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.set_preedit("pre".into(), Some((0, 3))));
        assert!(editor.set_preedit(String::new(), None));
        assert!(!editor.is_composing());
        assert_eq!(editor.visible_text(), "x");
        assert!(!editor.cancel_composition());
    }

    #[test]
    fn load_snapshot_cancels_unfinished_composition() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "abc".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.set_preedit("compose".into(), None));
        editor.load_snapshot(
            2,
            1,
            "other".to_string(),
            DocumentAccess::Editable { lease_id: 2 },
        );
        assert!(!editor.is_composing());
        assert_eq!(editor.visible_text(), "other");
    }

    #[test]
    fn undo_cancels_preedit_before_inverse_edit() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        assert!(editor.command(EditorCommand::Insert("ab")));
        assert!(editor.set_preedit("未".into(), Some((0, 3))));
        assert!(editor.is_composing());
        let outcome = editor.undo_with_event();
        assert!(outcome.changed);
        assert!(!editor.is_composing());
        assert_eq!(editor.visible_text(), "");
    }

    #[test]
    fn delete_forward_selected_range_emits_delete_operation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "abcdef".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_selection_for_test(5, 2);

        let outcome = editor.delete_forward_with_event();

        assert!(outcome.changed);
        assert_eq!(editor.visible_text(), "abf");
        assert_eq!(
            outcome.edit_event.unwrap().operation,
            EditOperation::Delete { start: 2, end: 5 }
        );
    }

    #[test]
    fn read_only_editor_allows_navigation_but_not_mutation() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(1, 2, "abc".to_string(), DocumentAccess::ReadOnly);
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
        editor.set_caret_for_test(1);

        let move_outcome = editor.command_with_event(EditorCommand::MoveRight);
        assert!(move_outcome.changed);
        assert_eq!(editor.caret_for_test(), 2);

        let edit_outcome = editor.command_with_event(EditorCommand::Insert("X"));
        assert!(!edit_outcome.changed);
        assert_eq!(edit_outcome.edit_event, None);
        assert_eq!(editor.visible_text(), "abc");
    }

    #[test]
    fn editor_events_do_not_block_without_ipc_consumer() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        let outcome = editor.command_with_event(EditorCommand::Insert("a"));

        assert!(outcome.changed);
        assert_eq!(outcome.edit_event, None);
        assert_eq!(editor.visible_text(), "a");
    }

    #[test]
    fn ordinary_typing_does_not_wait_for_server_or_javascript() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            String::new(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));

        let started = std::time::Instant::now();
        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("x".to_string())));

        assert!(outcome.command_outcome.changed);
        assert_eq!(outcome.server_intent, None);
        assert_eq!(editor.visible_text(), "x");
        assert!(outcome.command_outcome.edit_event.is_some());
        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "manifest-declared typing should complete locally before any async IPC/JS work"
        );
    }

    #[test]
    fn editor_enter_inserts_newline() {
        let mut editor = EditorSurface::default();

        editor.insert_text("first");
        let changed = editor.insert_newline();
        editor.insert_text("second");

        assert!(changed);
        assert_eq!(editor.visible_text(), "first\nsecond");
    }

    #[test]
    fn editor_insert_text_uses_caret_instead_of_appending() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_caret_for_test(1);

        let changed = editor.insert_text("X");

        assert!(changed);
        assert_eq!(editor.visible_text(), "aXbc");
        assert_eq!(editor.caret_for_test(), 2);
    }

    #[test]
    fn editor_insert_newline_auto_scrolls_to_new_line() {
        let mut editor = EditorSurface::default();
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 1.0);

        editor.insert_text("first");
        editor.insert_newline();
        editor.insert_text("second");

        assert_eq!(editor.visible_text(), "second");
    }

    #[test]
    fn editor_backspace_keeps_remaining_end_visible() {
        let mut editor = EditorSurface::default();
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 1.0);
        editor.insert_text("first");
        editor.insert_newline();
        editor.insert_text("second");

        let changed = editor.backspace();

        assert!(changed);
        assert_eq!(editor.visible_text(), "secon");
    }

    #[test]
    fn editor_delete_forward_removes_text_after_caret() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_caret_for_test(1);

        let changed = editor.delete_forward();

        assert!(changed);
        assert_eq!(editor.visible_text(), "ac");
        assert_eq!(editor.caret_for_test(), 1);
    }

    #[test]
    fn editor_cursor_navigation_moves_over_unicode_boundaries() {
        let mut editor = EditorSurface::default();
        editor.insert_text("a🦀b");
        editor.set_caret_for_test("a🦀".len());

        assert!(editor.move_left());
        assert_eq!(editor.caret_for_test(), 1);
        assert!(editor.move_right());
        assert_eq!(editor.caret_for_test(), "a🦀".len());
    }

    #[test]
    fn editor_home_end_navigation_uses_current_line() {
        let mut editor = EditorSurface::default();
        editor.insert_text("zero");
        editor.insert_newline();
        editor.insert_text("one");
        editor.set_caret_for_test("zero\no".len());

        assert!(editor.move_to_line_end());
        assert_eq!(editor.caret_for_test(), "zero\none".len());
        assert!(editor.move_to_line_start());
        assert_eq!(editor.caret_for_test(), "zero\n".len());
    }

    #[test]
    fn editor_up_down_navigation_preserves_scalar_column() {
        let mut editor = EditorSurface::default();
        editor.insert_text("a🦀c");
        editor.insert_newline();
        editor.insert_text("xy");
        editor.insert_newline();
        editor.insert_text("三四五");
        editor.set_caret_for_test("a🦀".len());

        assert!(editor.move_down());
        assert_eq!(editor.caret_for_test(), "a🦀c\nxy".len());
        assert!(editor.move_down());
        assert_eq!(editor.caret_for_test(), "a🦀c\nxy\n三四".len());
        assert!(editor.move_up());
        assert_eq!(editor.caret_for_test(), "a🦀c\nxy".len());
    }

    #[test]
    fn place_caret_at_point_before_text_moves_to_visible_start() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.build_visible_layout_for_test(300.0);

        let changed =
            editor.place_caret_at_point(masonry::kurbo::Point::new(TEXT_INSET - 100.0, TEXT_INSET));

        assert!(changed);
        assert_eq!(editor.caret_for_test(), 0);
    }

    #[test]
    fn place_caret_at_point_after_text_moves_to_visible_end() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_caret_for_test(0);
        editor.build_visible_layout_for_test(300.0);

        let changed = editor.place_caret_at_point(masonry::kurbo::Point::new(
            TEXT_INSET + 10_000.0,
            TEXT_INSET,
        ));

        assert!(changed);
        assert_eq!(editor.caret_for_test(), "abc".len());
    }

    #[test]
    fn place_caret_at_point_clears_selection_even_when_caret_stays_put() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_selection_for_test(1, 3);
        editor.build_visible_layout_for_test(300.0);

        let changed = editor.place_caret_at_point(masonry::kurbo::Point::new(
            TEXT_INSET + 10_000.0,
            TEXT_INSET,
        ));

        assert!(changed);
        assert_eq!(editor.caret_for_test(), "abc".len());
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn pointer_drag_extends_selection_from_click_anchor() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.build_visible_layout_for_test(300.0);

        assert!(
            editor
                .place_caret_at_point(masonry::kurbo::Point::new(TEXT_INSET - 100.0, TEXT_INSET,))
        );
        assert!(editor.extend_selection_to_point(masonry::kurbo::Point::new(
            TEXT_INSET + 10_000.0,
            TEXT_INSET,
        )));

        assert_eq!(editor.caret_for_test(), "abc".len());
        assert_eq!(editor.selection_for_test(), Some((0, "abc".len())));
    }

    #[test]
    fn pointer_drag_can_select_backwards() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_caret_for_test(0);
        editor.build_visible_layout_for_test(300.0);

        assert!(editor.place_caret_at_point(masonry::kurbo::Point::new(
            TEXT_INSET + 10_000.0,
            TEXT_INSET,
        )));
        assert!(
            editor.extend_selection_to_point(masonry::kurbo::Point::new(
                TEXT_INSET - 100.0,
                TEXT_INSET,
            ))
        );

        assert_eq!(editor.caret_for_test(), 0);
        assert_eq!(editor.selection_for_test(), Some(("abc".len(), 0)));
    }

    #[test]
    fn editor_command_layer_routes_navigation_and_editing() {
        let mut editor = EditorSurface::default();

        assert!(editor.command(EditorCommand::Insert("abc")));
        assert!(editor.command(EditorCommand::MoveLeft));
        assert!(editor.command(EditorCommand::Insert("X")));
        assert!(editor.command(EditorCommand::LineStart));
        assert!(editor.command(EditorCommand::DeleteForward));

        assert_eq!(editor.visible_text(), "bXc");
    }

    #[test]
    fn typing_replaces_selected_range() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abcdef");
        editor.set_selection_for_test(2, 5);

        let changed = editor.insert_text("X");

        assert!(changed);
        assert_eq!(editor.visible_text(), "abXf");
        assert_eq!(editor.caret_for_test(), 3);
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn enter_replaces_selected_range() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abcd");
        editor.set_selection_for_test(1, 3);

        let changed = editor.insert_newline();

        assert!(changed);
        assert_eq!(editor.visible_text(), "a\nd");
        assert_eq!(editor.caret_for_test(), 2);
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn backspace_deletes_selected_range() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abcdef");
        editor.set_selection_for_test(5, 2);

        let changed = editor.backspace();

        assert!(changed);
        assert_eq!(editor.visible_text(), "abf");
        assert_eq!(editor.caret_for_test(), 2);
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn delete_forward_deletes_selected_range() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abcdef");
        editor.set_selection_for_test(1, 4);

        let changed = editor.delete_forward();

        assert!(changed);
        assert_eq!(editor.visible_text(), "aef");
        assert_eq!(editor.caret_for_test(), 1);
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn shift_left_and_right_extend_selection() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");

        assert!(editor.select_left());
        assert_eq!(editor.selection_for_test(), Some((3, 2)));
        assert!(editor.select_left());
        assert_eq!(editor.selection_for_test(), Some((3, 1)));
        assert!(editor.select_right());
        assert_eq!(editor.selection_for_test(), Some((3, 2)));
    }

    #[test]
    fn non_shift_movement_clears_selection() {
        let mut editor = EditorSurface::default();
        editor.insert_text("abc");
        editor.set_selection_for_test(1, 3);

        assert!(editor.move_left());
        assert_eq!(editor.caret_for_test(), 1);
        assert_eq!(editor.selection_for_test(), None);
    }

    #[test]
    fn empty_document_caret_uses_default_document_profile() {
        let mut editor = EditorSurface::default();
        editor.build_visible_layout_for_test(300.0);
        let default_height = editor
            .caret_geometry(CARET_WIDTH as f32)
            .expect("placeholder layout should provide a caret")
            .height();

        let mut typography = ActiveTypography {
            revision: 1,
            ..ActiveTypography::default()
        };
        typography.proportional.size = 40.0;
        assert!(editor.set_typography(typography));
        editor.build_visible_layout_for_test(300.0);
        let configured_height = editor
            .caret_geometry(CARET_WIDTH as f32)
            .expect("configured placeholder layout should provide a caret")
            .height();

        assert!(configured_height > default_height);
    }

    #[test]
    fn custom_typography_keeps_scrollbar_and_viewport_geometry_bounded() {
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&"line\n".repeat(10_000));
        let mut typography = ActiveTypography {
            revision: 1,
            ..ActiveTypography::default()
        };
        typography.monospace.size = 40.0;
        typography.proportional.size = 10.0;
        assert!(editor.set_typography(typography));
        assert!(editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 112.0));
        assert_eq!(editor.viewport.visible_line_count(), 2);
        editor.set_visual_scroll_bounds_for_test(2_000.0);
        assert!(editor.scroll_vertical_pixels(400.0));

        let rect = Rect::new(0.0, 0.0, 900.0, 600.0);
        let thumb = editor.scrollbar_thumb_rect(rect).expect("scrollable thumb");
        assert!(thumb.y0 >= rect.y0 + TEXT_INSET_Y);
        assert!(thumb.y1 <= rect.y1 - TEXT_INSET_Y);
        assert!(editor.visible_text().len() < 10_000);

        let typography = ActiveTypography {
            revision: 2,
            ..ActiveTypography::default()
        };
        assert!(editor.set_typography(typography));
        assert_eq!(editor.visual_scroll_y(), 0.0);
    }

    #[test]
    fn editor_scroll_vertical_pixels_uses_visual_overflow_before_logical_lines() {
        let mut editor = EditorSurface::default();
        editor.set_visual_scroll_bounds_for_test(80.0);

        let changed = editor.scroll_vertical_pixels(20.0);

        assert!(changed);
        assert_eq!(editor.visual_scroll_y(), 20.0);
        assert_eq!(editor.visible_text(), "");
    }

    #[test]
    fn editor_visual_scroll_clamps_to_known_overflow() {
        let mut editor = EditorSurface::default();
        editor.set_visual_scroll_bounds_for_test(80.0);

        let changed = editor.scroll_vertical_pixels(200.0);

        assert!(changed);
        assert_eq!(editor.visual_scroll_y(), 80.0);
    }

    #[test]
    fn scroll_vertical_pixels_reaches_wrapped_overflow_at_document_end() {
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&"line\n".repeat(100));
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 4.0 * 28.0);
        // Wrapped lines make the final window taller than the viewport: the
        // real overflow budget exceeds one line height.
        editor.set_visual_scroll_bounds_for_test(200.0);
        editor.scroll_lines(10_000);
        assert_eq!(editor.viewport.first_visible_line(), 96);

        let changed = editor.scroll_vertical_pixels(500.0);

        assert!(changed);
        assert_eq!(
            editor.visual_scroll_y(),
            200.0,
            "scroll must reach the full wrapped-line overflow budget at document end"
        );
    }

    #[test]
    fn scroll_after_caret_move_clears_caret_pin() {
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&"line\n".repeat(20));
        editor.set_caret_for_test(0);

        editor.ensure_caret_line_visible();
        assert!(editor.pin_caret_visible, "caret move sets pin flag");

        editor.scroll_vertical_pixels(20.0);
        assert!(
            !editor.pin_caret_visible,
            "explicit scroll clears caret pin so paint cannot snap back to caret"
        );
    }

    #[test]
    fn scroll_vertical_pixels_advances_viewport_after_visual_budget() {
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&"line\n".repeat(100));
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 4.0 * 28.0);
        editor.set_visual_scroll_bounds_for_test(80.0);

        for _ in 0..10 {
            editor.scroll_vertical_pixels(20.0);
        }

        assert!(
            editor.viewport.first_visible_line() > 0,
            "viewport must advance after visual overflow budget is consumed"
        );
    }

    #[test]
    fn pixel_scroll_never_jumps_backward_across_line_boundaries() {
        // Continuity guard: as the user scrolls down, the visible snapshot's
        // start byte offset must be non-decreasing. The old two-tier model
        // exhausted an overscan visual budget then advanced one line and reset
        // visual to zero, jumping content backward. The continuous model
        // advances first_visible_line as each line_height is crossed.
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(
            &"line
"
            .repeat(200),
        );
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 4.0 * 28.0);
        editor.set_caret_for_test(0);
        editor.ensure_caret_line_visible();
        editor.set_visual_scroll_bounds_for_test(120.0);

        let mut previous_start = editor.visible_snapshot().start_byte_offset;
        for _ in 0..120 {
            let advanced = editor.scroll_vertical_pixels(20.0);
            let start = editor.visible_snapshot().start_byte_offset;
            assert!(
                start >= previous_start,
                "scrolling down must never move the visible start backward: {previous_start} -> {start}"
            );
            // Scrolling down must always make progress until the document end.
            assert!(advanced, "scroll-down must keep advancing before the end");
            previous_start = start;
        }
        assert!(
            editor.viewport.first_visible_line() > 0,
            "viewport must have advanced well into the document"
        );
    }

    #[test]
    fn visible_caret_offset_returns_none_when_caret_above_viewport() {
        // Regression guard: when the user scrolls down and the caret stays
        // above the visible snapshot, the offset subtraction must not overflow.
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&"line\n".repeat(100));
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 4.0 * 28.0);
        editor.set_caret_for_test(0);
        editor.ensure_caret_line_visible();
        // Simulate the visual budget the next paint would compute so scrolling
        // can advance the viewport without a real paint in this unit test.
        editor.set_visual_scroll_bounds_for_test(120.0);

        // Scroll the viewport down far enough that byte offset 0 is no longer
        // in the visible snapshot.
        for _ in 0..20 {
            editor.scroll_vertical_pixels(20.0);
        }
        assert!(editor.viewport.first_visible_line() > 0);

        let snapshot = editor.visible_snapshot();
        assert!(
            snapshot.start_byte_offset > 0,
            "visible snapshot must start after the caret for this regression"
        );
        assert_eq!(
            editor.visible_caret_offset(&snapshot),
            None,
            "caret above visible snapshot must return None, not panic"
        );
    }

    #[test]
    fn large_buffer_visible_extraction_remains_bounded_after_cursor_changes() {
        let text = generated_lines(10_000);
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&text);
        editor.update_visible_line_count_for_height(TEXT_INSET_Y * 2.0 + 12.0 * 28.0);
        assert!(editor.scroll_lines(5_000));
        let visible_start = editor.visible_snapshot().start_byte_offset;
        editor.set_caret_for_test(visible_start);
        assert!(editor.move_right());
        assert!(editor.select_right());

        let snapshot = editor.visible_snapshot();

        assert_eq!(snapshot.line_range, 5_000..5_024);
        assert!(snapshot.text.len() < text.len() / 100);
        assert!(snapshot.text.starts_with("line 05000\n"));
    }

    #[test]
    fn layout_cache_invalidates_on_caret_relevant_viewport_change_only_when_needed() {
        let mut editor = EditorSurface::default();
        assert!(editor.insert_text("abcdef"));
        let key_before =
            LayoutCacheKey::new(editor.buffer.revision(), editor.viewport.revision(), 300.0);

        assert!(editor.move_left());
        assert!(editor.select_left());
        let key_after =
            LayoutCacheKey::new(editor.buffer.revision(), editor.viewport.revision(), 300.0);

        assert_eq!(key_after, key_before);
    }

    #[test]
    fn client_first_typing_updates_local_text_without_waiting_for_server_command() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.scope = BehaviorScope::GlobalDefault;
        manifest.manifest_id = "test".to_string();
        manifest.keymaps.push(KeyBindingRule {
            command_id: "markdown.togglePreview".to_string(),
            sequence: vec![KeyStroke::new(KeyCode::Character("p".to_string()))],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        manifest.commands.push(CommandDeclaration {
            command_id: "markdown.togglePreview".to_string(),
            display_name: "Toggle Preview".to_string(),
            routing_policy: RoutingPolicy::ServerFirst,
            authority: CommandAuthority::ServerIntent,
        });

        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            "hello".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(manifest);

        let before = editor.visible_text();
        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("x".to_string())));
        let after_insert = editor.visible_text();

        assert_eq!(before, "hello");
        assert_eq!(after_insert, "xhello");
        assert!(outcome.command_outcome.changed);

        // A server-first keybinding produces a server intent but the editor
        // does not apply a local text edit for that key, keeping typing hot
        // paths independent of command execution.
        let server_outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("p".to_string())));
        assert!(
            server_outcome.server_intent.is_some(),
            "server-first keybinding produces an intent"
        );
        assert!(
            server_outcome.command_outcome.edit_event.is_none(),
            "server-first keybinding does not apply a local text edit"
        );
        assert_eq!(editor.visible_text(), "xhello");
    }

    // ── Phase 18.9 Task 5: generic key behavior through behavior manifests ──

    #[test]
    fn core_code_enter_auto_indents_and_electric_outdent_without_ipc() {
        let mut editor = EditorSurface::default();
        // Caret rests on an over-indented line inside a block.
        editor.load_snapshot(
            1,
            2,
            "fn a() {\n        ".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(3));
        editor.set_caret_for_test("fn a() {\n        ".len());

        // Enter first: auto-indents by preserving leading whitespace. This is
        // a ClientFirstPredictable local edit — the edit event is emitted with
        // no server/JavaScript round trip.
        let enter_outcome = editor.route_key_with_event(&KeyStroke::new(KeyCode::Enter));
        assert!(enter_outcome.command_outcome.changed);
        assert!(enter_outcome.command_outcome.edit_event.is_some());
        assert_eq!(editor.visible_text(), "fn a() {\n        \n        ");

        // Reset to the over-indented line and type `}`: the electric-character
        // rule sheds one indentation unit before inserting the trigger, so the
        // closing brace aligns with the block opener. Still fully local.
        editor.load_snapshot(
            1,
            2,
            "fn a() {\n        ".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(3));
        editor.set_caret_for_test("fn a() {\n        ".len());
        let electric_outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("}".to_string())));
        assert!(electric_outcome.command_outcome.changed);
        assert!(electric_outcome.command_outcome.edit_event.is_some());
        assert_eq!(editor.visible_text(), "fn a() {\n    }");

        // The electric edit is a Replace (shed indent + insert trigger), not a
        // bare Insert, proving the Rust-known engine reflowed from manifest data.
        assert!(matches!(
            electric_outcome
                .command_outcome
                .edit_event
                .unwrap()
                .operation,
            EditOperation::Replace { .. }
        ));
    }

    #[test]
    fn core_code_comment_continuation_hook_applies_on_enter_inside_comment() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "  // note".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(3));
        editor.set_caret_for_test("  // note".len());

        let outcome = editor.route_key_with_event(&KeyStroke::new(KeyCode::Enter));

        assert!(outcome.command_outcome.changed);
        assert_eq!(editor.visible_text(), "  // note\n  // ");
        // Local ClientFirstPredictable edit, no IPC wait.
        assert_eq!(
            outcome.command_outcome.edit_event.unwrap().operation,
            EditOperation::Insert {
                byte_offset: "  // note".len() as u64,
                text: "\n  // ".to_string(),
            }
        );
    }

    #[test]
    fn core_code_pair_insertion_runs_client_side_from_manifest() {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            2,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(3));
        editor.set_caret_for_test(1);

        // Every opener declared by the core.code manifest inserts both sides
        // and leaves the caret between them, locally from manifest data.
        for opener in ["(", "[", "{", "\"", "'"] {
            editor.load_snapshot(
                1,
                2,
                "ab".to_string(),
                DocumentAccess::Editable { lease_id: 1 },
            );
            editor.install_behavior_manifest(BehaviorManifest::core_code_editing(3));
            editor.set_caret_for_test(1);
            let outcome = editor
                .route_key_with_event(&KeyStroke::new(KeyCode::Character(opener.to_string())));
            assert!(
                outcome.command_outcome.changed,
                "opener {opener:?} should insert a pair"
            );
            assert!(outcome.command_outcome.edit_event.is_some());
            assert_eq!(
                editor.caret_for_test(),
                2,
                "caret sits between pair sides for {opener:?}"
            );
        }

        // Spot-check `(` explicitly: full text and emitted operation.
        editor.load_snapshot(
            1,
            2,
            "ab".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(3));
        editor.set_caret_for_test(1);
        let outcome =
            editor.route_key_with_event(&KeyStroke::new(KeyCode::Character("(".to_string())));
        assert_eq!(editor.visible_text(), "a()b");
        assert_eq!(
            outcome.command_outcome.edit_event.unwrap().operation,
            EditOperation::Insert {
                byte_offset: 1,
                text: "()".to_string(),
            }
        );
    }

    #[test]
    fn syntax_decoration_colors_are_distinct_by_token_family() {
        let registry = super::StyleRegistry::default();
        let kw = TokenType::classify_style_token("keyword.control").0;
        let str = TokenType::classify_style_token("string.quoted").0;
        let com = TokenType::classify_style_token("comment.line").0;
        let pun = TokenType::classify_style_token("punctuation.definition").0;
        let h1 = TokenType::classify_style_token("markup.heading.1").0;
        assert_ne!(
            registry
                .style_for(DecorationKind::Syntax, kw, Modifiers::NONE)
                .color,
            registry
                .style_for(DecorationKind::Syntax, str, Modifiers::NONE)
                .color
        );
        assert_ne!(
            registry
                .style_for(DecorationKind::Syntax, com, Modifiers::NONE)
                .color,
            registry
                .style_for(DecorationKind::Syntax, pun, Modifiers::NONE)
                .color
        );
        assert_eq!(
            registry
                .style_for(DecorationKind::Syntax, h1, Modifiers::NONE)
                .color,
            registry
                .style_for(DecorationKind::Semantic, h1, Modifiers::NONE)
                .color
        );
        assert_eq!(
            registry
                .style_for(
                    DecorationKind::Syntax,
                    TokenType::Paragraph,
                    Modifiers::NONE
                )
                .color,
            registry.base.text
        );
    }

    #[test]
    fn free_form_style_token_decoration_colors_baseline_locked() {
        // Phase 18.15 (Plan 046) baseline lock: pins the EXACT rendered color
        // for every two-axis token family and decoration layer. The compat
        // mapper (`TokenType::classify_style_token`) feeds the original
        // free-form style_token families into the StyleRegistry single source
        // of color; the task-5 active-theme overrides MUST reproduce these
        // colors unchanged. Edit only if a theme change intentionally revises
        // the default palette.
        let registry = super::StyleRegistry::default();
        let assert_family_color = |style_token: &str, expected: Color, family: &str| {
            let (token_type, _mods) = TokenType::classify_style_token(style_token);
            assert_eq!(
                registry
                    .style_for(DecorationKind::Syntax, token_type, Modifiers::NONE)
                    .color,
                expected,
                "{family} family baseline"
            );
        };
        assert_family_color(
            "keyword.control",
            Color::from_rgb8(0xc7, 0x92, 0xea),
            "keyword.*",
        );
        assert_family_color(
            "string.quoted",
            Color::from_rgb8(0xc3, 0xe8, 0x8d),
            "string.*",
        );
        assert_family_color(
            "comment.line",
            Color::from_rgb8(0x7f, 0x84, 0x8e),
            "comment.*",
        );
        assert_family_color(
            "punctuation.definition",
            Color::from_rgb8(0xd4, 0xd4, 0xd4),
            "punctuation.*",
        );
        // Unknown prefixes fall back to Variable -> default Syntax color.
        let (variable_tt, _) = TokenType::classify_style_token("variable.other");
        assert_eq!(
            registry
                .style_for(DecorationKind::Syntax, variable_tt, Modifiers::NONE)
                .color,
            Color::from_rgb8(0xe0, 0x6c, 0x75),
            "default Syntax family baseline"
        );
        // Plan 059 task 3 revised the prose palette from uniform muted green
        // to differentiated heading hues (Heading1 is red, bold by default).
        assert_family_color(
            "markup.heading.1",
            Color::from_rgb8(0xff, 0x4d, 0x6d),
            "markup.* Syntax",
        );
        // Semantic uses the same TokenType family table as Syntax so LSP
        // semantic tokens refine vocabulary without a second theme path.
        // Diagnostic/SearchMatch remain kind-first layer colors.
        let (function_tt, _) = TokenType::classify_style_token("function");
        assert_eq!(
            registry
                .style_for(DecorationKind::Semantic, function_tt, Modifiers::NONE)
                .color,
            registry
                .style_for(DecorationKind::Syntax, function_tt, Modifiers::NONE)
                .color,
            "Semantic layer shares Syntax TokenType colors"
        );
        let (any_tt, _) = TokenType::classify_style_token("function");
        assert_eq!(
            registry
                .style_for(DecorationKind::Diagnostic, any_tt, Modifiers::NONE)
                .color,
            Color::from_rgba8(0xff, 0x4d, 0x6d, 0x3f),
            "Diagnostic layer baseline"
        );
        assert_eq!(
            registry
                .style_for(DecorationKind::SearchMatch, any_tt, Modifiers::NONE)
                .color,
            Color::from_rgba8(0xff, 0xd1, 0x66, 0x45),
            "SearchMatch layer baseline"
        );
    }

    #[test]
    fn wrap_defaults_follow_document_font_role() {
        let mut editor = EditorSurface::default();
        assert_eq!(editor.resolved_wrap(), WrapPolicy::Viewport);

        let mut code = BehaviorManifest::core_code_editing(1);
        code.document_font_role = DocumentFontRole::Monospace;
        editor.install_behavior_manifest(code);
        assert_eq!(editor.resolved_wrap(), WrapPolicy::None);

        let mut prose = BehaviorManifest::minimal_text_editing(2);
        prose.document_font_role = DocumentFontRole::Proportional;
        editor.install_behavior_manifest(prose);
        assert_eq!(
            editor.resolved_wrap(),
            WrapPolicy::Column(WrapPolicy::DEFAULT_COLUMN)
        );
    }

    #[test]
    fn user_wrap_override_beats_manifest() {
        let mut editor = EditorSurface::default();
        let mut manifest = BehaviorManifest::core_code_editing(1);
        manifest.document_font_role = DocumentFontRole::Monospace;
        editor.install_behavior_manifest(manifest);
        assert!(editor.set_editor_layout(Some(WrapPolicy::Viewport)));
        assert_eq!(editor.resolved_wrap(), WrapPolicy::Viewport);
        let mut again = BehaviorManifest::core_code_editing(2);
        again.document_font_role = DocumentFontRole::Monospace;
        editor.install_behavior_manifest(again);
        assert_eq!(editor.resolved_wrap(), WrapPolicy::Viewport);
    }

    #[test]
    fn column_wrap_is_narrower_than_viewport() {
        let mut editor = EditorSurface::default();
        editor.set_editor_layout(Some(WrapPolicy::Viewport));
        let viewport = editor.layout_max_width(900.0);
        editor.set_editor_layout(Some(WrapPolicy::Column(40)));
        let column = editor.layout_max_width(900.0);
        assert!(column < viewport);
        editor.set_editor_layout(Some(WrapPolicy::None));
        assert_eq!(editor.layout_max_width(900.0), f32::MAX);
    }

    #[test]
    fn horizontal_scroll_only_applies_when_unwrapped() {
        let mut editor = EditorSurface::default();
        editor.set_visual_scroll_x_bounds_for_test(200.0);
        assert!(!editor.scroll_horizontal_pixels(40.0));
        assert_eq!(editor.visual_scroll_x, 0.0);

        editor.set_editor_layout(Some(WrapPolicy::None));
        editor.set_visual_scroll_x_bounds_for_test(200.0);
        assert!(editor.scroll_horizontal_pixels(40.0));
        assert_eq!(editor.visual_scroll_x, 40.0);
        assert!(editor.scroll_horizontal_pixels(400.0));
        assert_eq!(editor.visual_scroll_x, 200.0);
    }

    #[test]
    fn horizontal_scroll_does_not_change_layout_cache_key() {
        let mut editor = EditorSurface::default();
        editor.set_editor_layout(Some(WrapPolicy::None));
        let before = LayoutCacheKey::new(
            editor.buffer.revision(),
            editor.viewport.revision(),
            f32::MAX,
        );
        editor.set_visual_scroll_x_bounds_for_test(100.0);
        assert!(editor.scroll_horizontal_pixels(10.0));
        let after = LayoutCacheKey::new(
            editor.buffer.revision(),
            editor.viewport.revision(),
            f32::MAX,
        );
        assert_eq!(before, after);
    }

    #[test]
    fn insets_are_asymmetric_and_gutter_widens_left() {
        let mut editor = EditorSurface::default();
        assert_eq!(editor.inset_x(), TEXT_INSET);
        assert_eq!(editor.inset_y(), TEXT_INSET_Y);
        assert!(editor.inset_y() < editor.inset_x());

        let mut code = BehaviorManifest::core_code_editing(1);
        code.document_font_role = DocumentFontRole::Monospace;
        editor.install_behavior_manifest(code);
        assert_eq!(editor.inset_x(), TEXT_INSET_GUTTER);
    }

    fn load_editable(text: &str) -> EditorSurface {
        let mut editor = EditorSurface::default();
        editor.load_snapshot(
            1,
            1,
            text.to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor
    }

    #[test]
    fn toggle_comment_adds_and_strips_prefix_after_indent() {
        let mut editor = load_editable("    code");
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(1));
        editor.set_caret_for_test(4);
        assert!(editor.command(EditorCommand::ToggleComment));
        assert_eq!(editor.visible_text(), "    //code");
        assert!(editor.command(EditorCommand::ToggleComment));
        assert_eq!(editor.visible_text(), "    code");
    }

    #[test]
    fn toggle_comment_multi_caret_and_selection_lines() {
        let mut editor = load_editable("a\nb\nc");
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(1));
        editor.set_selection_for_test(0, 3);
        assert!(editor.command(EditorCommand::ToggleComment));
        assert_eq!(editor.visible_text(), "//a\n//b\nc");
        editor.load_snapshot(
            1,
            1,
            "a\nb\nc".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::core_code_editing(1));
        editor.set_caret_for_test(0);
        editor.add_selection_for_test(4, 4);
        assert!(editor.command(EditorCommand::ToggleComment));
        assert_eq!(editor.visible_text(), "//a\nb\n//c");
    }

    #[test]
    fn toggle_comment_no_rule_is_noop() {
        let mut editor = load_editable("code");
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.comments.clear();
        editor.install_behavior_manifest(manifest);
        let outcome = editor.command_with_event(EditorCommand::ToggleComment);
        assert!(!outcome.changed);
        assert_eq!(outcome.diagnostic, Some("no comments rule"));
        assert_eq!(editor.visible_text(), "code");
    }

    #[test]
    fn toggle_list_marker_toggles_dash_and_ordered_dot() {
        let mut editor = load_editable("item");
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.enter = EnterRule::ContinueLineMarkers {
            markers: vec!["-".to_string(), "ordered-dot".to_string()],
            exit_on_empty_item: true,
        };
        editor.install_behavior_manifest(manifest.clone());
        assert!(editor.command(EditorCommand::ToggleListMarker));
        assert_eq!(editor.visible_text(), "- item");
        assert!(editor.command(EditorCommand::ToggleListMarker));
        assert_eq!(editor.visible_text(), "item");

        manifest.editor_rules.enter = EnterRule::ContinueLineMarkers {
            markers: vec!["ordered-dot".to_string()],
            exit_on_empty_item: true,
        };
        editor.load_snapshot(
            1,
            1,
            "item".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(manifest);
        assert!(editor.command(EditorCommand::ToggleListMarker));
        assert_eq!(editor.visible_text(), "1. item");
        editor.load_snapshot(
            1,
            1,
            "2. item".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        editor.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.enter = EnterRule::ContinueLineMarkers {
            markers: vec!["ordered-dot".to_string()],
            exit_on_empty_item: true,
        };
        editor.install_behavior_manifest(manifest);
        assert!(editor.command(EditorCommand::ToggleListMarker));
        assert_eq!(editor.visible_text(), "item");
    }

    #[test]
    fn rotate_heading_cycles_atx_levels() {
        let mut editor = load_editable("title");
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.heading_prefixes = vec![
            "# ".to_string(),
            "## ".to_string(),
            "### ".to_string(),
            "#### ".to_string(),
            "##### ".to_string(),
            "###### ".to_string(),
        ];
        editor.install_behavior_manifest(manifest);
        assert!(editor.command(EditorCommand::RotateHeading));
        assert_eq!(editor.visible_text(), "# title");
        assert!(editor.command(EditorCommand::RotateHeading));
        assert_eq!(editor.visible_text(), "## title");
        for _ in 0..4 {
            assert!(editor.command(EditorCommand::RotateHeading));
        }
        assert_eq!(editor.visible_text(), "###### title");
        assert!(editor.command(EditorCommand::RotateHeading));
        assert_eq!(editor.visible_text(), "title");
    }

    fn apply_core_folds(editor: &mut EditorSurface, ranges: &[(u64, u64)]) {
        assert!(
            editor.apply_folding_set(FoldingRangeSet {
                document_id: 1,
                document_version: 1,
                package_prefix: "core".to_string(),
                ranges: ranges
                    .iter()
                    .map(|(start, end)| FoldingRange {
                        byte_start: *start,
                        byte_end: *end,
                        label: None,
                        provenance: FoldingProvenance::core(),
                    })
                    .collect(),
            })
        );
    }

    #[test]
    fn toggle_fold_hides_and_restores_interior_lines() {
        let mut editor = load_editable("fn a() {\n    1\n}\n");
        apply_core_folds(&mut editor, &[(0, 16)]);
        editor.set_caret_for_test(0);
        assert!(!editor.line_is_hidden(1));
        assert!(editor.command(EditorCommand::ToggleFold));
        assert!(editor.line_is_hidden(1));
        assert!(!editor.line_is_hidden(0));
        assert!(editor.command(EditorCommand::ToggleFold));
        assert!(!editor.line_is_hidden(1));
    }

    #[test]
    fn nested_parent_collapse_hides_child() {
        let mut editor = load_editable("outer {\n  inner {\n    x\n  }\n}\n");
        apply_core_folds(&mut editor, &[(0, 30), (10, 26)]);
        editor.set_caret_for_test(0);
        assert!(editor.command(EditorCommand::ToggleFold));
        assert!(editor.line_is_hidden(1));
        assert!(editor.line_is_hidden(2));
        assert!(editor.line_is_hidden(3));
    }

    #[test]
    fn toggle_inlay_hides_overlay() {
        let mut editor = load_editable("let x = 1;");
        let mut manifest = BehaviorManifest::core_code_editing(1);
        manifest.document_font_role = DocumentFontRole::Monospace;
        editor.install_behavior_manifest(manifest);
        assert!(editor.inlay_hints_visible());
        let mut set = decoration_set(
            1,
            0,
            10,
            vec![DecorationSpan::from_inlay(
                4,
                5,
                crate::protocol::InlayHintPayload {
                    label: ": i32".into(),
                    placement: crate::protocol::InlayPlacement::After,
                },
                10,
                crate::protocol::DecorationProvenance {
                    package_name: "test".into(),
                    package_version: "1.0.0".into(),
                    package_prefix: "test".into(),
                },
            )],
        );
        set.kind = DecorationKind::InlayHint;
        assert!(editor.apply_decoration_set(set));
        assert_eq!(
            editor
                .decorations
                .visible_spans(0, 10)
                .filter(|span| span.inlay.is_some())
                .count(),
            1
        );
        assert!(editor.command(EditorCommand::ToggleInlayHints));
        assert!(!editor.inlay_hints_visible());
        assert!(editor.command(EditorCommand::ToggleInlayHints));
        assert!(editor.inlay_hints_visible());
    }
}
