use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, BezPath, Rect};
use masonry::parley::style::StyleProperty;
use masonry::peniko::Fill;

use super::EditorSurface;
use crate::editor::buffer::{EditorBuffer, VisibleSnapshot};
use crate::protocol::{EditorChrome, FontRole, PairRule};

/// ponytail: bracket scan stops here; raise if nested files need farther matches.
pub(super) const BRACKET_MATCH_SCAN_BYTES: usize = 64 * 1024;
const GUTTER_PAD: f64 = 8.0;

impl EditorSurface {
    pub(super) fn resolved_chrome(&self) -> EditorChrome {
        let Some(manifest) = self.document.behavior_manifest.as_ref() else {
            return EditorChrome::prose();
        };
        manifest
            .editor_rules
            .chrome
            .unwrap_or_else(|| EditorChrome::from_font_role(manifest.document_font_role))
    }

    pub(super) fn visible_caret_offsets(&self, snapshot: &VisibleSnapshot) -> Vec<usize> {
        self.selections
            .selections()
            .iter()
            .filter_map(|selection| self.visible_byte_offset(selection.focus(), snapshot))
            .collect()
    }

    pub(super) fn visible_bracket_ranges(
        &self,
        snapshot: &VisibleSnapshot,
    ) -> Vec<std::ops::Range<usize>> {
        if !self.resolved_chrome().bracket_match {
            return Vec::new();
        }
        let Some(manifest) = self.document.behavior_manifest.as_ref() else {
            return Vec::new();
        };
        let mut ranges = Vec::new();
        for selection in self.selections.selections() {
            for (open, close) in single_char_pairs(&manifest.editor_rules.pairs) {
                if let Some([left, right]) = bracket_pair_ranges(
                    &self.buffer,
                    selection.focus(),
                    open,
                    close,
                    BRACKET_MATCH_SCAN_BYTES,
                ) {
                    if let Some(range) = visible_range(snapshot, left) {
                        ranges.push(range);
                    }
                    if let Some(range) = visible_range(snapshot, right) {
                        ranges.push(range);
                    }
                }
            }
        }
        ranges
    }

    pub(super) fn indent_tab_width(&self) -> Option<u8> {
        if !self.resolved_chrome().indent_guides {
            return None;
        }
        Some(
            self.document
                .behavior_manifest
                .as_ref()
                .map(|manifest| manifest.editor_rules.tab.spaces_per_tab.max(1))
                .unwrap_or(4),
        )
    }

    pub(super) fn paint_gutter(
        &self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut masonry::vello::Scene,
        snapshot: &VisibleSnapshot,
        available_height: f64,
        origin: (f64, f64),
    ) {
        if !self.resolved_chrome().gutter || snapshot.text.is_empty() {
            return;
        }
        let inset_x = self.inset_x();
        let inset_y = self.inset_y();
        let clip = Rect::new(
            origin.0,
            origin.1 + inset_y,
            origin.0 + inset_x - 4.0,
            origin.1 + inset_y + available_height,
        );
        if clip.width() <= 0.0 || clip.height() <= 0.0 {
            return;
        }
        scene.push_clip_layer(Affine::IDENTITY, &clip);
        let active_lines = active_logical_lines(self, snapshot);
        let profile = self.typography.profile(FontRole::Monospace);
        let (font_context, layout_context) = ctx.text_contexts();
        let mut byte = 0;
        let mut visual_lines = snapshot.text.split('\n');
        for line_idx in snapshot.line_range.clone() {
            if self.line_is_hidden(line_idx) {
                continue;
            }
            let Some(line) = visual_lines.next() else {
                break;
            };
            let number = line_idx + 1;
            if let Some((y0, y1)) = self.layout.line_vertical_span(byte) {
                let label = number.to_string();
                let mut builder = layout_context.ranged_builder(font_context, &label, 1.0, true);
                builder.push_default(StyleProperty::FontStack(profile.font_stack()));
                builder.push_default(StyleProperty::FontSize(profile.size()));
                builder.push_default(StyleProperty::Brush(BrushIndex(0)));
                let mut layout = builder.build(&label);
                layout.break_all_lines(Some((inset_x - GUTTER_PAD * 2.0).max(1.0) as f32));
                let width = f64::from(layout.full_width());
                let x = gutter_number_origin_x(
                    origin.0 + inset_x - GUTTER_PAD,
                    width,
                    origin.0 + GUTTER_PAD,
                );
                let y = origin.1 + y0 + inset_y - self.visual_scroll_y;
                let color = if active_lines.contains(&line_idx) {
                    self.theme.gutter_foreground_active
                } else {
                    self.theme.gutter_foreground
                };
                let _ = y1;
                render_text(
                    scene,
                    Affine::translate((x, y)),
                    &layout,
                    &[color.into()],
                    true,
                );
                if self.line_is_fold_start(line_idx) {
                    let collapsed = self.line_fold_is_collapsed(line_idx);
                    paint_fold_chevron(scene, origin.0 + 4.0, y + 4.0, color, collapsed);
                }
            }
            byte = byte.saturating_add(line.len()).saturating_add(1);
        }
        scene.pop_layer();
    }
}

fn paint_fold_chevron(
    scene: &mut masonry::vello::Scene,
    x: f64,
    y: f64,
    color: masonry::peniko::Color,
    collapsed: bool,
) {
    let mut path = BezPath::new();
    if collapsed {
        path.move_to((x, y));
        path.line_to((x + 6.0, y + 4.0));
        path.line_to((x, y + 8.0));
    } else {
        path.move_to((x, y));
        path.line_to((x + 8.0, y));
        path.line_to((x + 4.0, y + 6.0));
    }
    path.close_path();
    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &path);
}

pub(super) fn gutter_number_origin_x(gutter_right: f64, number_width: f64, min_x: f64) -> f64 {
    (gutter_right - number_width).max(min_x)
}

#[cfg(test)]
fn indent_guide_columns(line: &str, spaces_per_tab: u8) -> Vec<usize> {
    let tab = usize::from(spaces_per_tab.max(1));
    let mut cols = 0;
    for character in line.chars() {
        match character {
            ' ' => cols += 1,
            '\t' => cols += tab - (cols % tab),
            _ => break,
        }
    }
    (tab..=cols).step_by(tab).collect()
}

fn single_char_pairs(pairs: &[PairRule]) -> impl Iterator<Item = (char, char)> + '_ {
    pairs.iter().filter_map(|pair| {
        let mut open = pair.open.chars();
        let mut close = pair.close.chars();
        match (open.next(), open.next(), close.next(), close.next()) {
            (Some(left), None, Some(right), None) if left != right => Some((left, right)),
            _ => None,
        }
    })
}

fn bracket_at_or_before(
    buffer: &EditorBuffer,
    caret: usize,
    open: char,
    close: char,
) -> Option<(usize, bool)> {
    let caret = buffer.clamp_byte_offset(caret);
    match buffer.char_at(caret) {
        Some(character) if character == open => Some((caret, true)),
        Some(character) if character == close => Some((caret, false)),
        _ => {
            let previous = buffer.char_before(caret)?;
            if previous == open {
                Some((caret - previous.len_utf8(), true))
            } else if previous == close {
                Some((caret - previous.len_utf8(), false))
            } else {
                None
            }
        }
    }
}

fn bracket_pair_ranges(
    buffer: &EditorBuffer,
    caret: usize,
    open: char,
    close: char,
    max_bytes: usize,
) -> Option<[(usize, usize); 2]> {
    let (anchor, is_open) = bracket_at_or_before(buffer, caret, open, close)?;
    let matched = buffer.matching_pair_byte_within(caret, open, close, max_bytes)?;
    let (anchor_end, match_end) = if is_open {
        (anchor + open.len_utf8(), matched + close.len_utf8())
    } else {
        (anchor + close.len_utf8(), matched + open.len_utf8())
    };
    Some([(anchor, anchor_end), (matched, match_end)])
}

fn visible_range(
    snapshot: &VisibleSnapshot,
    range: (usize, usize),
) -> Option<std::ops::Range<usize>> {
    let visible_start = snapshot.start_byte_offset;
    let visible_end = snapshot.start_byte_offset + snapshot.text.len();
    let start = range.0.max(visible_start);
    let end = range.1.min(visible_end);
    (start < end).then(|| (start - visible_start)..(end - visible_start))
}

fn active_logical_lines(editor: &EditorSurface, snapshot: &VisibleSnapshot) -> Vec<usize> {
    editor
        .selections
        .selections()
        .iter()
        .filter_map(|selection| {
            let byte = selection.focus();
            if byte < snapshot.start_byte_offset {
                return None;
            }
            let relative = byte - snapshot.start_byte_offset;
            if relative > snapshot.text.len() {
                return None;
            }
            let line = snapshot.text[..relative]
                .bytes()
                .filter(|b| *b == b'\n')
                .count();
            Some(snapshot.line_range.start + line)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{BehaviorManifest, DocumentFontRole};

    #[test]
    fn chrome_defaults_follow_document_font_role() {
        let mut editor = EditorSurface::default();
        assert_eq!(editor.resolved_chrome(), EditorChrome::prose());

        let mut code = BehaviorManifest::core_code_editing(1);
        code.document_font_role = DocumentFontRole::Monospace;
        editor.install_behavior_manifest(code);
        assert_eq!(editor.resolved_chrome(), EditorChrome::code());

        let mut prose = BehaviorManifest::minimal_text_editing(2);
        prose.document_font_role = DocumentFontRole::Proportional;
        editor.install_behavior_manifest(prose);
        assert_eq!(editor.resolved_chrome(), EditorChrome::prose());
    }

    #[test]
    fn explicit_chrome_overrides_role_default() {
        let mut editor = EditorSurface::default();
        let mut manifest = BehaviorManifest::core_code_editing(1);
        manifest.document_font_role = DocumentFontRole::Monospace;
        manifest.editor_rules.chrome = Some(EditorChrome {
            gutter: false,
            active_line: true,
            indent_guides: false,
            bracket_match: false,
            inlay_hints: false,
        });
        editor.install_behavior_manifest(manifest);
        let chrome = editor.resolved_chrome();
        assert!(!chrome.gutter);
        assert!(chrome.active_line);
        assert!(!chrome.indent_guides);
        assert!(!chrome.bracket_match);
        assert!(!chrome.inlay_hints);
    }

    #[test]
    fn prose_chrome_defaults_inlays_off() {
        assert!(!EditorChrome::prose().inlay_hints);
        assert!(EditorChrome::code().inlay_hints);
        let mut editor = EditorSurface::default();
        let mut prose = BehaviorManifest::minimal_text_editing(1);
        prose.document_font_role = DocumentFontRole::Proportional;
        editor.install_behavior_manifest(prose);
        assert!(!editor.resolved_chrome().inlay_hints);
    }

    #[test]
    fn indent_guide_columns_count_spaces_and_tabs() {
        assert_eq!(indent_guide_columns("fn main", 4), Vec::<usize>::new());
        assert_eq!(indent_guide_columns("    x", 4), vec![4]);
        assert_eq!(indent_guide_columns("        x", 4), vec![4, 8]);
        assert_eq!(indent_guide_columns("\t\tx", 4), vec![4, 8]);
    }

    #[test]
    fn gutter_digits_right_align_in_inset() {
        assert_eq!(gutter_number_origin_x(40.0, 12.0, 8.0), 28.0);
        assert_eq!(gutter_number_origin_x(40.0, 40.0, 8.0), 8.0);
    }
}
