//! Bounded LSP snippet expansion for completion accept.
//!
//! `parse_snippet` expands inert LSP snippet syntax (tabstops `$1`, final
//! `$0`, placeholders `${1:default}`, choices `${1|a,b,c|}`) into plain text
//! plus ordered placeholder ranges for a client-local snippet session. The
//! parser is allocation-bounded, never touches the filesystem/network/server,
//! and produces text + ranges only — no executable transforms, commands,
//! shell, variables with external resolution, or callbacks.
//!
//! Backslash escapes (`\$`, `\\`, `\}`), variables (`$name`, `${name}`), and
//! nested placeholders are not implemented this phase: a bare `$` not forming a
//! recognized tabstop/placeholder/choice is emitted as a literal `$`, and a
//! `${...}` that does not start with a tabstop index is rejected as
//! `UnsupportedVariable`. ponytail: add escape/variable/nesting support when a
//! first-party snippet needs it; none do today.

use crate::perf::budgets::COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS;

/// Maximum number of tabstops/placeholders/choices a single snippet may
/// declare. A snippet exceeding this is rejected so a session cannot be driven
/// by an unbounded tabstop list.
pub(crate) const SNIPPET_MAX_TABSTOPS: usize = 32;

/// One navigable placeholder (tabstop) in an expanded snippet.
/// `byte_start`..`byte_end` is a half-open byte range into
/// [`SnippetExpansion::text`]. A zero-width range (`byte_start == byte_end`) is
/// a bare tabstop cursor position. `index` is the tabstop number; `$0`/`${0}`
/// carry `final_tabstop = true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnippetPlaceholder {
    pub byte_start: usize,
    pub byte_end: usize,
    pub index: u32,
    pub final_tabstop: bool,
}

/// Result of expanding an LSP snippet string: the plain text to insert plus the
/// ordered placeholder ranges a snippet session navigates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnippetExpansion {
    pub text: String,
    pub placeholders: Vec<SnippetPlaceholder>,
}

/// Why a snippet string could not be expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnippetParseError {
    /// A `${...}` placeholder or choice was not terminated by `}` / `|}`.
    Unterminated,
    /// A `${...}` construct did not start with a tabstop index (e.g.
    /// `${name}`), so it is an unsupported variable reference.
    UnsupportedVariable,
    /// A `${<index>...}` body was neither `}`, `:default`, nor `|choice|}`.
    Malformed,
    /// The expanded text exceeds the per-item insert-text character budget.
    ExpandedTextTooLong { length: usize, max_chars: usize },
    /// The snippet declares more tabstops than [`SNIPPET_MAX_TABSTOPS`].
    TooManyTabstops { count: usize, max: usize },
}

/// Expand an LSP snippet string into plain text plus ordered placeholder
/// ranges. See the module docs for the supported syntax and deferred features.
pub(crate) fn parse_snippet(input: &str) -> Result<SnippetExpansion, SnippetParseError> {
    let bytes = input.as_bytes();
    let mut text = String::with_capacity(
        input
            .len()
            .min(COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS * 4),
    );
    let mut text_chars = 0;
    let mut placeholders: Vec<SnippetPlaceholder> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            let ch = input[i..].chars().next().expect("non-empty slice");
            push_text_char(&mut text, &mut text_chars, ch)?;
            i += ch.len_utf8();
            continue;
        }
        match bytes.get(i + 1).copied() {
            // Bare tabstop: `$<digits>`.
            Some(d) if d.is_ascii_digit() => {
                let (index, consumed) = parse_index(&input[i + 1..]);
                let pos = text.len();
                push_placeholder(&mut placeholders, pos, pos, index)?;
                i += 1 + consumed;
            }
            // Braced construct: `${...}`.
            Some(b'{') => {
                let body = &input[i + 2..];
                let first = body.as_bytes().first().copied();
                if first.is_none_or(|c| !c.is_ascii_digit()) {
                    return Err(SnippetParseError::UnsupportedVariable);
                }
                let (index, num_len) = parse_index(body);
                let after_num = i + 2 + num_len;
                match bytes.get(after_num).copied() {
                    // `${index}` — empty tabstop.
                    Some(b'}') => {
                        let pos = text.len();
                        push_placeholder(&mut placeholders, pos, pos, index)?;
                        i = after_num + 1;
                    }
                    // `${index:default}` — placeholder whose default is inserted.
                    Some(b':') => {
                        let start = text.len();
                        let mut j = after_num + 1;
                        while j < bytes.len() && bytes[j] != b'}' {
                            let ch = input[j..].chars().next().unwrap();
                            push_text_char(&mut text, &mut text_chars, ch)?;
                            j += ch.len_utf8();
                        }
                        if j >= bytes.len() {
                            return Err(SnippetParseError::Unterminated);
                        }
                        push_placeholder(&mut placeholders, start, text.len(), index)?;
                        i = j + 1;
                    }
                    // `${index|a,b,c|}` — choice; the first option is inserted.
                    Some(b'|') => {
                        let start = text.len();
                        let mut j = after_num + 1;
                        while j < bytes.len() && bytes[j] != b',' && bytes[j] != b'|' {
                            let ch = input[j..].chars().next().unwrap();
                            push_text_char(&mut text, &mut text_chars, ch)?;
                            j += ch.len_utf8();
                        }
                        if j >= bytes.len() {
                            return Err(SnippetParseError::Unterminated);
                        }
                        // Skip any remaining options up to the closing `|`.
                        let mut k = j;
                        while k < bytes.len() && bytes[k] != b'|' {
                            let ch = input[k..].chars().next().unwrap();
                            k += ch.len_utf8();
                        }
                        if k >= bytes.len() || bytes.get(k + 1) != Some(&b'}') {
                            return Err(SnippetParseError::Unterminated);
                        }
                        push_placeholder(&mut placeholders, start, text.len(), index)?;
                        i = k + 2;
                    }
                    _ => return Err(SnippetParseError::Malformed),
                }
            }
            // Bare `$` (end of string, or not followed by digit/`{`): literal.
            _ => {
                push_text_char(&mut text, &mut text_chars, '$')?;
                i += 1;
            }
        }
    }

    Ok(SnippetExpansion { text, placeholders })
}

fn push_text_char(
    text: &mut String,
    length: &mut usize,
    ch: char,
) -> Result<(), SnippetParseError> {
    if *length >= COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS {
        return Err(SnippetParseError::ExpandedTextTooLong {
            length: *length + 1,
            max_chars: COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS,
        });
    }
    text.push(ch);
    *length += 1;
    Ok(())
}

/// Parse a leading run of ASCII digits as a tabstop index, returning the index
/// and the byte length consumed. The caller guarantees a leading digit.
fn parse_index(rest: &str) -> (u32, usize) {
    let digits: &str = rest
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("");
    let value = digits.parse::<u32>().unwrap_or(0);
    (value, digits.len())
}

fn push_placeholder(
    placeholders: &mut Vec<SnippetPlaceholder>,
    byte_start: usize,
    byte_end: usize,
    index: u32,
) -> Result<(), SnippetParseError> {
    if placeholders.len() >= SNIPPET_MAX_TABSTOPS {
        return Err(SnippetParseError::TooManyTabstops {
            count: placeholders.len() + 1,
            max: SNIPPET_MAX_TABSTOPS,
        });
    }
    placeholders.push(SnippetPlaceholder {
        byte_start,
        byte_end,
        index,
        final_tabstop: index == 0,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_round_trips_verbatim() {
        let exp = parse_snippet("hello world").unwrap();
        assert_eq!(exp.text, "hello world");
        assert!(exp.placeholders.is_empty());
    }

    #[test]
    fn bare_dollar_is_literal() {
        // `$` not followed by a digit or `{` is a literal dollar.
        let exp = parse_snippet("cost: $name and $").unwrap();
        assert_eq!(exp.text, "cost: $name and $");
        assert!(exp.placeholders.is_empty());
    }

    #[test]
    fn tabstops_and_placeholders_expand() {
        let exp = parse_snippet("fn ${1:name}(${2:args}) {\n\t$0\n}").unwrap();
        assert_eq!(exp.text, "fn name(args) {\n\t\n}");
        assert_eq!(
            exp.placeholders,
            vec![
                SnippetPlaceholder {
                    byte_start: 3,
                    byte_end: 7,
                    index: 1,
                    final_tabstop: false
                },
                SnippetPlaceholder {
                    byte_start: 8,
                    byte_end: 12,
                    index: 2,
                    final_tabstop: false
                },
                SnippetPlaceholder {
                    byte_start: 17,
                    byte_end: 17,
                    index: 0,
                    final_tabstop: true
                },
            ]
        );
    }

    #[test]
    fn choice_uses_first_option() {
        let exp = parse_snippet("${1|pub,priv|} fn").unwrap();
        assert_eq!(exp.text, "pub fn");
        assert_eq!(
            exp.placeholders,
            vec![SnippetPlaceholder {
                byte_start: 0,
                byte_end: 3,
                index: 1,
                final_tabstop: false
            }]
        );
    }

    #[test]
    fn empty_placeholder_and_braced_tabstop() {
        let exp = parse_snippet("x${1}y${2:}z$0").unwrap();
        assert_eq!(exp.text, "xyz");
        assert_eq!(
            exp.placeholders,
            vec![
                SnippetPlaceholder {
                    byte_start: 1,
                    byte_end: 1,
                    index: 1,
                    final_tabstop: false
                },
                SnippetPlaceholder {
                    byte_start: 2,
                    byte_end: 2,
                    index: 2,
                    final_tabstop: false
                },
                SnippetPlaceholder {
                    byte_start: 3,
                    byte_end: 3,
                    index: 0,
                    final_tabstop: true
                },
            ]
        );
    }

    #[test]
    fn unterminated_constructs_are_rejected() {
        assert_eq!(
            parse_snippet("${1:unfinished").unwrap_err(),
            SnippetParseError::Unterminated
        );
        assert_eq!(
            parse_snippet("${1|a,b").unwrap_err(),
            SnippetParseError::Unterminated
        );
    }

    #[test]
    fn unsupported_variable_is_rejected() {
        assert_eq!(
            parse_snippet("${name}").unwrap_err(),
            SnippetParseError::UnsupportedVariable
        );
    }

    #[test]
    fn malformed_braced_is_rejected() {
        // `${1x}` has a non-digit after the index that is not `}`/`:`/`|`.
        assert_eq!(
            parse_snippet("${1x}").unwrap_err(),
            SnippetParseError::Malformed
        );
    }

    #[test]
    fn too_many_tabstops_are_rejected() {
        let mut snippet = String::new();
        for index in 1..=(SNIPPET_MAX_TABSTOPS + 1) {
            snippet.push_str(&format!("${{{index}}}"));
        }
        assert!(matches!(
            parse_snippet(&snippet).unwrap_err(),
            SnippetParseError::TooManyTabstops {
                max: SNIPPET_MAX_TABSTOPS,
                ..
            }
        ));
    }

    #[test]
    fn oversize_expanded_text_is_rejected() {
        // Literal text (no tabstops) isolates the expanded-text cap.
        let snippet = "x".repeat(COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS + 1);
        assert!(matches!(
            parse_snippet(&snippet).unwrap_err(),
            SnippetParseError::ExpandedTextTooLong {
                max_chars: COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS,
                ..
            }
        ));
    }
}
