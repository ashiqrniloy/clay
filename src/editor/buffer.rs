use crop::Rope;

#[derive(Debug, Default)]
pub struct EditorBuffer {
    rope: Rope,
}

impl EditorBuffer {
    pub fn insert_str(&mut self, text: &str) {
        self.rope.insert(self.rope.byte_len(), text);
    }

    pub fn backspace(&mut self) {
        let Some(last_char) = self.rope.chars().next_back() else {
            return;
        };

        let end = self.rope.byte_len();
        self.rope.delete(end - last_char.len_utf8()..end);
    }

    pub fn visible_text(&self) -> String {
        self.rope.to_string()
    }
}
