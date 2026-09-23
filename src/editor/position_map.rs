//! UTF-16 (CodeMirror / JS string) ↔ UTF-8 (Clay rope / protocol) offsets.
//!
//! One conversion boundary for the Tauri/React client. Offsets that land
//! inside a multi-unit scalar snap to that scalar's start so the result is
//! always a valid UTF-8 boundary.

/// Convert a UTF-16 code-unit offset into a UTF-8 byte offset.
pub fn utf16_to_utf8(text: &str, utf16: usize) -> usize {
    let mut seen_utf16 = 0usize;
    let mut seen_utf8 = 0usize;
    for character in text.chars() {
        let width = character.len_utf16();
        if utf16 < seen_utf16.saturating_add(width) {
            return seen_utf8;
        }
        seen_utf16 = seen_utf16.saturating_add(width);
        seen_utf8 = seen_utf8.saturating_add(character.len_utf8());
        if utf16 == seen_utf16 {
            return seen_utf8;
        }
    }
    text.len()
}

/// Convert a UTF-8 byte offset into a UTF-16 code-unit offset.
pub fn utf8_to_utf16(text: &str, utf8: usize) -> usize {
    let mut seen_utf8 = 0usize;
    let mut seen_utf16 = 0usize;
    for character in text.chars() {
        let width = character.len_utf8();
        if utf8 < seen_utf8.saturating_add(width) {
            return seen_utf16;
        }
        seen_utf8 = seen_utf8.saturating_add(width);
        seen_utf16 = seen_utf16.saturating_add(character.len_utf16());
        if utf8 == seen_utf8 {
            return seen_utf16;
        }
    }
    seen_utf16
}

#[cfg(test)]
mod tests {
    use super::{utf8_to_utf16, utf16_to_utf8};

    /// Shared with `frontend/src/editor/position-map.ts`.
    const VECTORS: &[(&str, usize, usize)] = &[
        ("", 0, 0),
        ("abc", 1, 1),
        ("abc", 3, 3),
        ("héllo", 2, 3),
        ("a😀b", 1, 1),
        ("a😀b", 3, 5),
        ("a😀b", 4, 6),
        ("e\u{0301}", 1, 1),
        ("e\u{0301}", 2, 3),
        ("a\r\nb", 2, 2),
        ("a\r\nb", 3, 3),
        ("𐍈", 0, 0),
        ("𐍈", 2, 4),
    ];

    #[test]
    fn golden_vectors_round_trip() {
        for &(text, utf16, utf8) in VECTORS {
            assert_eq!(
                utf16_to_utf8(text, utf16),
                utf8,
                "utf16→utf8 {text:?} @{utf16}"
            );
            assert_eq!(
                utf8_to_utf16(text, utf8),
                utf16,
                "utf8→utf16 {text:?} @{utf8}"
            );
        }
    }

    #[test]
    fn mid_surrogate_snaps_to_scalar_start() {
        // 😀 occupies UTF-16 [1, 3) in "a😀b". Offset 2 is the trailing
        // surrogate; the map must not split the scalar.
        assert_eq!(utf16_to_utf8("a😀b", 2), 1);
    }

    #[test]
    fn mid_utf8_snaps_to_scalar_start() {
        // é is two UTF-8 bytes starting at offset 1 in "héllo".
        assert_eq!(utf8_to_utf16("héllo", 2), 1);
    }

    #[test]
    fn out_of_range_clamps_to_end() {
        assert_eq!(utf16_to_utf8("ab", 99), 2);
        assert_eq!(utf8_to_utf16("ab", 99), 2);
    }
}
