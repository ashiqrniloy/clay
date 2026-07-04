; @clay/rust Tree-sitter highlight query.
; Captures map to known Clay style tokens via package styleMap.
; Only captures present in styleMap are emitted; unmatched nodes stay unstyled.
(line_comment) @comment
(string_literal) @string
(raw_string_literal) @string

["{" "}" "(" ")" "[" "]"] @punctuation

[
  "fn" "let" "return" "if" "else" "match" "while" "for" "loop" "use" "mod"
  "struct" "enum" "impl" "trait" "type" "where" "as" "in" "unsafe" "extern"
] @keyword
