use super::buffer::EditorBuffer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorState {
    caret: usize,
    preferred_x: Option<f32>,
}

impl Default for CursorState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl CursorState {
    pub fn new(caret: usize) -> Self {
        Self {
            caret,
            preferred_x: None,
        }
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn set_caret(&mut self, caret: usize) {
        self.caret = caret;
        self.clear_preferred_x();
    }

    #[cfg(test)]
    pub fn preferred_x(&self) -> Option<f32> {
        self.preferred_x
    }

    #[cfg(test)]
    pub fn set_preferred_x(&mut self, preferred_x: f32) {
        self.preferred_x = Some(preferred_x);
    }

    pub fn clear_preferred_x(&mut self) {
        self.preferred_x = None;
    }

    pub fn move_to_previous_scalar(&mut self, buffer: &EditorBuffer) -> bool {
        let Some(caret) = buffer.previous_scalar_boundary(self.caret) else {
            self.set_caret(buffer.document_start_byte());
            return false;
        };
        self.move_to(caret, buffer)
    }

    pub fn move_to_next_scalar(&mut self, buffer: &EditorBuffer) -> bool {
        let Some(caret) = buffer.next_scalar_boundary(self.caret) else {
            self.set_caret(buffer.document_end_byte());
            return false;
        };
        self.move_to(caret, buffer)
    }

    pub fn move_to_document_start(&mut self, buffer: &EditorBuffer) -> bool {
        self.move_to(buffer.document_start_byte(), buffer)
    }

    pub fn move_to_document_end(&mut self, buffer: &EditorBuffer) -> bool {
        self.move_to(buffer.document_end_byte(), buffer)
    }

    pub fn move_to_line_start(&mut self, buffer: &EditorBuffer) -> bool {
        self.move_to(buffer.line_start_byte(self.caret), buffer)
    }

    pub fn move_to_line_end(&mut self, buffer: &EditorBuffer) -> bool {
        self.move_to(buffer.line_end_byte(self.caret), buffer)
    }

    fn move_to(&mut self, caret: usize, buffer: &EditorBuffer) -> bool {
        let previous = self.caret;
        self.set_caret(buffer.clamp_byte_offset(caret));
        self.caret != previous
    }
}

#[cfg(test)]
mod tests {
    use super::CursorState;
    use crate::editor::buffer::EditorBuffer;

    #[test]
    fn cursor_defaults_to_document_start() {
        let cursor = CursorState::default();

        assert_eq!(cursor.caret(), 0);
        assert_eq!(cursor.preferred_x(), None);
    }

    #[test]
    fn setting_caret_clears_preferred_x() {
        let mut cursor = CursorState::new(3);
        cursor.set_preferred_x(42.0);

        cursor.set_caret(7);

        assert_eq!(cursor.caret(), 7);
        assert_eq!(cursor.preferred_x(), None);
    }

    #[test]
    fn cursor_moves_left_and_right_over_ascii() {
        let buffer = EditorBuffer::from_text("abc");
        let mut cursor = CursorState::new(1);

        assert!(cursor.move_to_next_scalar(&buffer));
        assert_eq!(cursor.caret(), 2);
        assert!(cursor.move_to_previous_scalar(&buffer));
        assert_eq!(cursor.caret(), 1);
    }

    #[test]
    fn cursor_moves_over_multibyte_scalars_without_invalid_offsets() {
        let buffer = EditorBuffer::from_text("a🦀三é");
        let mut cursor = CursorState::default();
        let mut offsets = Vec::new();

        while cursor.move_to_next_scalar(&buffer) {
            offsets.push(cursor.caret());
        }

        assert_eq!(offsets, vec![1, 5, 8, 10]);
        assert_eq!(cursor.caret(), buffer.document_end_byte());
    }

    #[test]
    fn cursor_boundary_policy_for_combining_marks_is_documented() {
        let buffer = EditorBuffer::from_text("e\u{301}");
        let mut cursor = CursorState::default();

        cursor.move_to_next_scalar(&buffer);
        assert_eq!(cursor.caret(), "e".len());
        cursor.move_to_next_scalar(&buffer);
        assert_eq!(cursor.caret(), "e\u{301}".len());
    }

    #[test]
    fn line_start_and_line_end_handle_lf_and_final_line() {
        let buffer = EditorBuffer::from_text("zero\none\ntwo");
        let mut cursor = CursorState::new("zero\no".len());

        assert!(cursor.move_to_line_start(&buffer));
        assert_eq!(cursor.caret(), "zero\n".len());
        assert!(cursor.move_to_line_end(&buffer));
        assert_eq!(cursor.caret(), "zero\none".len());

        cursor.set_caret(buffer.document_end_byte());
        assert!(!cursor.move_to_line_end(&buffer));
        assert_eq!(cursor.caret(), "zero\none\ntwo".len());
    }

    #[test]
    fn document_start_and_end_movement_clamps_to_buffer_bounds() {
        let buffer = EditorBuffer::from_text("abc");
        let mut cursor = CursorState::new(999);

        assert!(cursor.move_to_document_end(&buffer));
        assert_eq!(cursor.caret(), 3);
        assert!(cursor.move_to_document_start(&buffer));
        assert_eq!(cursor.caret(), 0);
    }
}
