; @clay/rust Tree-sitter highlight query.
; Captures map to Phase 18.15 vocabulary (TokenType + Modifiers) through the
; package styleMap / native descriptor style_map. Only captures present in the
; styleMap are emitted; unmatched nodes stay unstyled (no default color leak).

(line_comment) @comment
(string_literal) @string
(raw_string_literal) @string

["{" "}" "(" ")" "[" "]"] @punctuation

[
  "fn" "let" "return" "if" "else" "match" "while" "for" "loop" "use" "mod"
  "struct" "enum" "impl" "trait" "type" "where" "as" "in" "unsafe" "extern"
] @keyword

(function_item name: (identifier) @function.declaration)
(type_identifier) @type
(primitive_type) @type
(integer_literal) @number
(float_literal) @number
