use std::ops::Range;

use masonry::core::PaintCtx;
use masonry::kurbo::{Affine, Circle, Point, Rect};
use masonry::peniko::{Color, Fill};

use crate::client::behavior::{
    ClientBehaviorState, ClientLocalEdit, RoutedBehavior, ServerIntentRoute,
};
use crate::protocol::{
    BehaviorManifest, BehaviorVersion, DocumentAccess, DocumentId, DocumentVersion, EditOperation,
    EnterRule, KeyStroke, PairRule,
};

use super::buffer::{EditResult, EditorBuffer, VisibleSnapshot};
use super::cursor::CursorState;
use super::is_printable_text;
use super::layout::{LayoutCacheKey, LayoutState};
use super::selection::SelectionState;
use super::viewport::{Viewport, visible_line_count_from_height};

const PANEL_COLOR: Color = Color::from_rgb8(0x24, 0x24, 0x24);
const ACCENT_COLOR: Color = Color::from_rgb8(0x8a, 0x6f, 0xff);
const TEXT_COLOR: Color = Color::from_rgb8(0xf4, 0xf1, 0xff);
const PLACEHOLDER_COLOR: Color = Color::from_rgb8(0x8d, 0x86, 0xa3);
const SELECTION_COLOR: Color = Color::from_rgba8(0x8a, 0x6f, 0xff, 0x66);
const CARET_COLOR: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const CARET_WIDTH: f64 = 1.5;
pub(super) const TEXT_INSET: f64 = 48.0;
pub(super) const TEXT_FONT_SIZE: f32 = 20.0;
const PLACEHOLDER_TEXT: &str = "Start typing in the Clay native text canvas…";
const LINE_HEIGHT_MULTIPLIER: f64 = 1.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand<'a> {
    Insert(&'a str),
    Newline,
    Backspace,
    DeleteForward,
    MoveLeft,
    MoveRight,
    SelectLeft,
    SelectRight,
    MoveUp,
    MoveDown,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorEditEvent {
    pub document_id: DocumentId,
    pub base_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub operation: EditOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommandOutcome {
    pub changed: bool,
    pub edit_event: Option<EditorEditEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorKeyOutcome {
    pub(crate) command_outcome: EditorCommandOutcome,
    pub(crate) server_intent: Option<ServerIntentRoute>,
}

impl EditorKeyOutcome {
    fn client(command_outcome: EditorCommandOutcome) -> Self {
        Self {
            command_outcome,
            server_intent: None,
        }
    }

    fn server(server_intent: ServerIntentRoute) -> Self {
        Self {
            command_outcome: EditorCommandOutcome::unchanged(),
            server_intent: Some(server_intent),
        }
    }

    fn unhandled() -> Self {
        Self::client(EditorCommandOutcome::unchanged())
    }
}

impl EditorCommandOutcome {
    fn unchanged() -> Self {
        Self {
            changed: false,
            edit_event: None,
        }
    }

    fn changed(edit_event: Option<EditorEditEvent>) -> Self {
        Self {
            changed: true,
            edit_event,
        }
    }

    fn from_changed(changed: bool) -> Self {
        Self {
            changed,
            edit_event: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Default)]
pub struct EditorSurface {
    buffer: EditorBuffer,
    document: EditorDocumentState,
    cursor: CursorState,
    selection: Option<SelectionState>,
    viewport: Viewport,
    layout: LayoutState,
    visual_scroll_y: f64,
    last_visual_max_scroll_y: f64,
    follow_visual_end: bool,
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
        self.cursor.set_caret(0);
        self.selection = None;
        self.viewport = Viewport::default();
        self.layout = LayoutState::default();
        self.visual_scroll_y = 0.0;
        self.last_visual_max_scroll_y = 0.0;
        self.follow_visual_end = false;
    }

    pub fn install_behavior_manifest(&mut self, manifest: BehaviorManifest) {
        if ClientBehaviorState::new(manifest.clone()).is_ok() {
            self.document.behavior_version = manifest.behavior_version;
            self.document.behavior_manifest = Some(manifest);
        }
    }

    pub(crate) fn route_key_with_event(&mut self, key: &KeyStroke) -> EditorKeyOutcome {
        let Some(manifest) = &self.document.behavior_manifest else {
            return EditorKeyOutcome::unhandled();
        };
        let Ok(router) = ClientBehaviorState::new(manifest.clone()) else {
            return EditorKeyOutcome::unhandled();
        };

        match router.route_key(key) {
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText(text)) => {
                let outcome = if let Some(pair) = self.pair_rule_for_inserted_text(&text).cloned() {
                    self.insert_pair_with_event(&pair)
                } else {
                    self.insert_text_with_event(&text)
                };
                EditorKeyOutcome::client(outcome)
            }
            RoutedBehavior::ClientEdit(ClientLocalEdit::Newline) => {
                EditorKeyOutcome::client(self.insert_newline_with_event())
            }
            RoutedBehavior::ServerIntent(intent) => EditorKeyOutcome::server(intent),
            RoutedBehavior::Unhandled => EditorKeyOutcome::unhandled(),
        }
    }

    pub fn document_state(&self) -> &EditorDocumentState {
        &self.document
    }

    pub fn command(&mut self, command: EditorCommand<'_>) -> bool {
        self.command_with_event(command).changed
    }

    pub fn command_with_event(&mut self, command: EditorCommand<'_>) -> EditorCommandOutcome {
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
        }
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        self.insert_text_with_event(text).changed
    }

    pub fn insert_text_with_event(&mut self, text: &str) -> EditorCommandOutcome {
        if !self.is_editable() || !is_printable_text(text) {
            return EditorCommandOutcome::unchanged();
        }

        self.replace_selection_or_insert_with_event(text)
    }

    pub fn insert_newline(&mut self) -> bool {
        self.insert_newline_with_event().changed
    }

    pub fn insert_newline_with_event(&mut self) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        let (result, operation) = if let Some(range) = self.selected_range() {
            let operation = EditOperation::Replace {
                start: range.start as u64,
                end: range.end as u64,
                text: "\n".to_string(),
            };
            (self.buffer.replace_range(range, "\n"), operation)
        } else {
            let byte_offset = self.buffer.clamp_byte_offset(self.cursor.caret());
            let text = self.newline_text_at(byte_offset);
            (
                self.buffer.insert_at(byte_offset, &text),
                EditOperation::Insert {
                    byte_offset: byte_offset as u64,
                    text,
                },
            )
        };
        self.finish_edit_with_operation(result, operation)
    }

    pub fn backspace(&mut self) -> bool {
        self.backspace_with_event().changed
    }

    pub fn backspace_with_event(&mut self) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        if let Some(range) = self.selected_range() {
            let operation = EditOperation::Delete {
                start: range.start as u64,
                end: range.end as u64,
            };
            let result = self.buffer.delete_range(range);
            return self.finish_edit_with_operation(result, operation);
        }

        let caret = self.buffer.clamp_byte_offset(self.cursor.caret());
        let Some(previous) = self.buffer.previous_scalar_boundary(caret) else {
            let result = self.buffer.backspace_at(caret);
            return self.finish_edit(result);
        };
        let result = self.buffer.delete_range(previous..caret);
        self.finish_edit_with_operation(
            result,
            EditOperation::Delete {
                start: previous as u64,
                end: caret as u64,
            },
        )
    }

    pub fn delete_forward(&mut self) -> bool {
        self.delete_forward_with_event().changed
    }

    pub fn delete_forward_with_event(&mut self) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        if let Some(range) = self.selected_range() {
            let operation = EditOperation::Delete {
                start: range.start as u64,
                end: range.end as u64,
            };
            let result = self.buffer.delete_range(range);
            return self.finish_edit_with_operation(result, operation);
        }

        let caret = self.buffer.clamp_byte_offset(self.cursor.caret());
        let Some(next) = self.buffer.next_scalar_boundary(caret) else {
            let result = self.buffer.delete_after(caret);
            return self.finish_edit(result);
        };
        let result = self.buffer.delete_range(caret..next);
        self.finish_edit_with_operation(
            result,
            EditOperation::Delete {
                start: caret as u64,
                end: next as u64,
            },
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
        self.move_cursor(|cursor, buffer| cursor.move_to_previous_line(buffer))
    }

    pub fn move_down(&mut self) -> bool {
        self.move_cursor(|cursor, buffer| cursor.move_to_next_line(buffer))
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

    pub fn visible_text(&self) -> String {
        self.visible_snapshot().text
    }

    pub fn hit_test_document_offset(&self, point: Point) -> Option<usize> {
        let snapshot = self.visible_snapshot();
        if snapshot.text.is_empty() {
            return Some(snapshot.start_byte_offset);
        }

        let layout_x = (point.x - TEXT_INSET) as f32;
        let layout_y = (point.y - TEXT_INSET + self.visual_scroll_y) as f32;
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
        let caret = self.cursor.caret();
        let visible_end = snapshot.start_byte_offset + snapshot.text.len();
        if caret < snapshot.start_byte_offset || caret > visible_end {
            return None;
        }

        let visible_offset = caret - snapshot.start_byte_offset;
        let geometry = self
            .layout
            .caret_geometry_for_visible_byte_offset(visible_offset, width)?;
        Some(Rect::new(
            geometry.rect.x0 + TEXT_INSET,
            geometry.rect.y0 + TEXT_INSET - self.visual_scroll_y,
            geometry.rect.x1 + TEXT_INSET,
            geometry.rect.y1 + TEXT_INSET - self.visual_scroll_y,
        ))
    }

    pub fn place_caret_at_point(&mut self, point: Point) -> bool {
        let Some(caret) = self.hit_test_document_offset(point) else {
            return false;
        };

        let previous = self.cursor.caret();
        let had_selection = self.selection.is_some();
        self.cursor.set_caret(caret);
        self.selection = None;
        self.follow_visual_end = false;
        self.ensure_caret_line_visible();
        had_selection || previous != self.cursor.caret()
    }

    pub fn extend_selection_to_point(&mut self, point: Point) -> bool {
        let Some(focus) = self.hit_test_document_offset(point) else {
            return false;
        };

        let previous_caret = self.cursor.caret();
        let previous_selection = self.selection;
        let anchor = self
            .selection
            .map_or(previous_caret, |selection| selection.anchor());
        self.cursor.set_caret(focus);

        let selection = SelectionState::new(anchor, self.cursor.caret()).clamped(&self.buffer);
        self.selection = (!selection.is_collapsed()).then_some(selection);
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;

        previous_caret != self.cursor.caret() || previous_selection != self.selection
    }

    pub fn scroll_lines(&mut self, delta_lines: isize) -> bool {
        if delta_lines != 0 {
            let line_height = TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER;
            if self.scroll_visual_pixels(delta_lines as f64 * line_height) {
                return true;
            }
        }

        let changed = self
            .viewport
            .scroll_lines(delta_lines, self.buffer.line_len());
        if changed {
            self.visual_scroll_y = 0.0;
            self.follow_visual_end = false;
        }
        changed
    }

    pub fn scroll_vertical_pixels(&mut self, delta_pixels: f64) -> bool {
        let line_height = TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER;
        let magnitude = (delta_pixels.abs() / line_height).ceil().max(1.0) as isize;
        let delta_lines = if delta_pixels.is_sign_negative() {
            -magnitude
        } else {
            magnitude
        };
        if self.scroll_visual_pixels(delta_pixels) {
            true
        } else {
            self.scroll_lines(delta_lines)
        }
    }

    pub fn update_visible_line_count_for_height(&mut self, height: f64) -> bool {
        let available_height = (height - (TEXT_INSET * 2.0)).max(0.0);
        let line_height = TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER;
        let visible_line_count = visible_line_count_from_height(available_height, line_height);
        self.viewport
            .set_visible_line_count(visible_line_count, self.buffer.line_len())
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut masonry::vello::Scene) {
        let size = ctx.size();
        let width = size.width;
        let height = size.height;
        self.update_visible_line_count_for_height(height);

        let canvas = Rect::new(
            24.0,
            24.0,
            (width - 24.0).max(24.0),
            (height - 24.0).max(24.0),
        );
        scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_COLOR, None, &canvas);

        let radius = (width.min(height) * 0.12).clamp(32.0, 96.0);
        let circle = Circle::new((width - 72.0, height - 72.0), radius);
        scene.fill(Fill::NonZero, Affine::IDENTITY, ACCENT_COLOR, None, &circle);

        let max_width = (width - (TEXT_INSET * 2.0)).max(1.0) as f32;
        let available_height = (height - (TEXT_INSET * 2.0)).max(0.0);
        let focused = ctx.is_focus_target();
        self.paint_text(ctx, scene, max_width, available_height, focused);
    }

    fn paint_text(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        max_width: f32,
        available_height: f64,
        focused: bool,
    ) {
        let snapshot = self.visible_snapshot();
        let current_text = snapshot.text.as_str();
        let (display_text, color) = if current_text.is_empty() {
            (PLACEHOLDER_TEXT, PLACEHOLDER_COLOR)
        } else {
            (current_text, TEXT_COLOR)
        };

        let caret_visible_offset = self.visible_caret_offset(&snapshot);
        let selection_visible_range = self.visible_selection_range(&snapshot);
        let key = LayoutCacheKey::new(self.buffer.revision(), self.viewport.revision(), max_width);
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
            selection_visible_range,
            SELECTION_COLOR,
        );
        if current_text.is_empty() {
            self.visual_scroll_y = 0.0;
            self.last_visual_max_scroll_y = 0.0;
        } else {
            self.last_visual_max_scroll_y = metrics.max_scroll_y(available_height);
        }
        if focused {
            self.paint_caret(scene, current_text.is_empty(), max_width, available_height);
        }
        self.follow_visual_end = false;
    }

    fn paint_caret(
        &self,
        scene: &mut masonry::vello::Scene,
        text_is_empty: bool,
        max_width: f32,
        available_height: f64,
    ) {
        let caret = if text_is_empty {
            Rect::new(
                TEXT_INSET,
                TEXT_INSET,
                TEXT_INSET + CARET_WIDTH,
                (TEXT_INSET + TEXT_FONT_SIZE as f64 * LINE_HEIGHT_MULTIPLIER)
                    .min(TEXT_INSET + available_height),
            )
        } else if let Some(rect) = self.caret_geometry(CARET_WIDTH as f32) {
            rect
        } else {
            return;
        };

        let clip = Rect::new(
            TEXT_INSET,
            TEXT_INSET,
            TEXT_INSET + max_width as f64,
            TEXT_INSET + available_height,
        );
        scene.push_clip_layer(Affine::IDENTITY, &clip);
        scene.fill(Fill::NonZero, Affine::IDENTITY, CARET_COLOR, None, &caret);
        scene.pop_layer();
    }

    fn visible_caret_offset(&self, snapshot: &VisibleSnapshot) -> Option<usize> {
        let caret = self.cursor.caret();
        let visible_end = snapshot.start_byte_offset + snapshot.text.len();
        (caret >= snapshot.start_byte_offset && caret <= visible_end)
            .then_some(caret - snapshot.start_byte_offset)
    }

    fn visible_selection_range(&self, snapshot: &VisibleSnapshot) -> Option<Range<usize>> {
        let selection = self.selection?;
        let range = selection.normalized_range();
        let visible_start = snapshot.start_byte_offset;
        let visible_end = snapshot.start_byte_offset + snapshot.text.len();
        let start = range.start.max(visible_start);
        let end = range.end.min(visible_end);
        (start < end).then_some((start - visible_start)..(end - visible_start))
    }

    fn visible_snapshot(&self) -> VisibleSnapshot {
        let range = self.viewport.visible_range(self.buffer.line_len());
        self.buffer.visible_snapshot(range)
    }

    fn selected_range(&self) -> Option<Range<usize>> {
        let selection = self.selection?.clamped(&self.buffer);
        let range = selection.normalized_range();
        (range.start < range.end).then_some(range)
    }

    fn pair_rule_for_inserted_text(&self, text: &str) -> Option<&PairRule> {
        let manifest = self.document.behavior_manifest.as_ref()?;
        manifest
            .editor_rules
            .pairs
            .iter()
            .find(|pair| pair.open == text)
    }

    fn newline_text_at(&self, byte_offset: usize) -> String {
        let Some(manifest) = &self.document.behavior_manifest else {
            return "\n".to_string();
        };

        match manifest.editor_rules.enter {
            EnterRule::InsertNewlineOnly => "\n".to_string(),
            EnterRule::PreserveLeadingWhitespace => {
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

    fn insert_pair_with_event(&mut self, pair: &PairRule) -> EditorCommandOutcome {
        if !self.is_editable() {
            return EditorCommandOutcome::unchanged();
        }

        let (result, operation, final_caret) = if let Some(range) = self.selected_range() {
            let selected_text = self.buffer.text_range(range.clone());
            let replacement = format!("{}{}{}", pair.open, selected_text, pair.close);
            let operation = EditOperation::Replace {
                start: range.start as u64,
                end: range.end as u64,
                text: replacement.clone(),
            };
            let result = self.buffer.replace_range(range, &replacement);
            let final_caret = result.caret;
            (result, operation, final_caret)
        } else {
            let byte_offset = self.buffer.clamp_byte_offset(self.cursor.caret());
            let insertion = format!("{}{}", pair.open, pair.close);
            let operation = EditOperation::Insert {
                byte_offset: byte_offset as u64,
                text: insertion.clone(),
            };
            let result = self.buffer.insert_at(byte_offset, &insertion);
            let final_caret = byte_offset + pair.open.len();
            (result, operation, final_caret)
        };

        self.finish_edit_with_operation_and_caret(result, operation, final_caret)
    }

    fn replace_selection_or_insert_with_event(&mut self, text: &str) -> EditorCommandOutcome {
        let (result, operation) = if let Some(range) = self.selected_range() {
            let operation = EditOperation::Replace {
                start: range.start as u64,
                end: range.end as u64,
                text: text.to_string(),
            };
            (self.buffer.replace_range(range, text), operation)
        } else {
            let byte_offset = self.buffer.clamp_byte_offset(self.cursor.caret());
            (
                self.buffer.insert_at(byte_offset, text),
                EditOperation::Insert {
                    byte_offset: byte_offset as u64,
                    text: text.to_string(),
                },
            )
        };
        self.finish_edit_with_operation(result, operation)
    }

    fn collapse_selection_to(&mut self, caret: usize) -> bool {
        let previous_caret = self.cursor.caret();
        let had_selection = self.selection.is_some();
        self.cursor.set_caret(caret);
        self.selection = None;
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        had_selection || previous_caret != self.cursor.caret()
    }

    fn finish_edit(&mut self, result: EditResult) -> EditorCommandOutcome {
        self.cursor.set_caret(result.caret);
        self.selection = None;
        if !result.changed {
            return EditorCommandOutcome::unchanged();
        }

        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        EditorCommandOutcome::changed(None)
    }

    fn finish_edit_with_operation(
        &mut self,
        result: EditResult,
        operation: EditOperation,
    ) -> EditorCommandOutcome {
        self.finish_edit_with_operation_and_caret(result, operation, result.caret)
    }

    fn finish_edit_with_operation_and_caret(
        &mut self,
        result: EditResult,
        operation: EditOperation,
        caret: usize,
    ) -> EditorCommandOutcome {
        self.cursor.set_caret(self.buffer.clamp_byte_offset(caret));
        self.selection = None;
        if !result.changed {
            return EditorCommandOutcome::unchanged();
        }

        self.ensure_caret_line_visible();
        self.follow_visual_end = true;
        let edit_event = self.client_first_event(operation);
        EditorCommandOutcome::changed(edit_event)
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
        let caret_line = self.buffer.line_of_byte(self.cursor.caret());
        self.viewport
            .ensure_line_visible(caret_line, self.buffer.line_len())
    }

    fn move_cursor(
        &mut self,
        movement: impl FnOnce(&mut CursorState, &EditorBuffer) -> bool,
    ) -> bool {
        let had_selection = self.selection.is_some();
        let changed = movement(&mut self.cursor, &self.buffer);
        self.selection = None;
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
        let anchor = self
            .selection
            .map_or_else(|| self.cursor.caret(), |selection| selection.anchor());
        let changed = movement(&mut self.cursor, &self.buffer);
        if !changed {
            return false;
        }

        let mut selection = SelectionState::new(anchor, self.cursor.caret()).clamped(&self.buffer);
        if selection.is_collapsed() {
            self.selection = None;
        } else {
            selection.set_focus(self.cursor.caret());
            self.selection = Some(selection);
        }
        self.ensure_caret_line_visible();
        self.follow_visual_end = false;
        true
    }

    fn scroll_visual_pixels(&mut self, delta_pixels: f64) -> bool {
        if delta_pixels == 0.0 || self.last_visual_max_scroll_y <= 0.0 {
            return false;
        }

        let previous = self.visual_scroll_y;
        self.visual_scroll_y =
            (self.visual_scroll_y + delta_pixels).clamp(0.0, self.last_visual_max_scroll_y);
        self.follow_visual_end = false;
        self.visual_scroll_y != previous
    }

    #[cfg(test)]
    fn build_visible_layout_for_test(&mut self, max_width: f32) {
        let text = self.visible_text();
        self.layout.set_cached_layout_for_test(&text, max_width);
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
    fn set_caret_for_test(&mut self, caret: usize) {
        let caret = self.buffer.clamp_byte_offset(caret);
        self.cursor.set_caret(caret);
    }

    #[cfg(test)]
    fn caret_for_test(&self) -> usize {
        self.cursor.caret()
    }

    #[cfg(test)]
    fn selection_for_test(&self) -> Option<(usize, usize)> {
        self.selection
            .map(|selection| (selection.anchor(), selection.focus()))
    }

    #[cfg(test)]
    fn set_selection_for_test(&mut self, anchor: usize, focus: usize) {
        let selection = SelectionState::new(anchor, focus).clamped(&self.buffer);
        self.cursor.set_caret(selection.focus());
        self.selection = (!selection.is_collapsed()).then_some(selection);
    }

    #[cfg(test)]
    fn visual_scroll_y(&self) -> f64 {
        self.visual_scroll_y
    }

    #[cfg(test)]
    fn set_visual_scroll_bounds_for_test(&mut self, max_scroll_y: f64) {
        self.last_visual_max_scroll_y = max_scroll_y.max(0.0);
        self.visual_scroll_y = self
            .visual_scroll_y
            .clamp(0.0, self.last_visual_max_scroll_y);
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::{EditorCommand, EditorSurface, TEXT_INSET};
    use crate::editor::layout::LayoutCacheKey;
    use crate::protocol::{
        BehaviorManifest, CommandDeclaration, DocumentAccess, EditOperation, KeyBindingContext,
        KeyBindingRule, KeyCode, KeyStroke, RoutingPolicy, TabMode,
    };

    fn generated_lines(line_count: usize) -> String {
        let mut text = String::new();
        for line in 0..line_count {
            writeln!(text, "line {line:05}").expect("writing to String cannot fail");
        }
        text
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
        editor.update_visible_line_count_for_height(TEXT_INSET * 2.0 + 1.0);

        editor.insert_text("first");
        editor.insert_newline();
        editor.insert_text("second");

        assert_eq!(editor.visible_text(), "second");
    }

    #[test]
    fn editor_backspace_keeps_remaining_end_visible() {
        let mut editor = EditorSurface::default();
        editor.update_visible_line_count_for_height(TEXT_INSET * 2.0 + 1.0);
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
    fn large_buffer_visible_extraction_remains_bounded_after_cursor_changes() {
        let text = generated_lines(10_000);
        let mut editor = EditorSurface::default();
        editor.set_text_for_test(&text);
        editor.update_visible_line_count_for_height(TEXT_INSET * 2.0 + 12.0 * 28.0);
        assert!(editor.scroll_lines(5_000));
        let visible_start = editor.visible_snapshot().start_byte_offset;
        editor.set_caret_for_test(visible_start);
        assert!(editor.move_right());
        assert!(editor.select_right());

        let snapshot = editor.visible_snapshot();

        assert_eq!(snapshot.line_range, 5_000..5_016);
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
}
