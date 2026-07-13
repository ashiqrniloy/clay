; @clay/typescript Tree-sitter highlight query.
; Captures map to Phase 18.15 vocabulary (TokenType + Modifiers) through the
; package styleMap / native descriptor style_map. Only captures present in the
; styleMap are emitted; unmatched nodes stay unstyled (no default color leak).

(comment) @comment
(string) @string
(template_string) @string

["{" "}" "(" ")" "[" "]"] @punctuation

[
  "export" "function" "const" "return" "if" "else" "for" "while" "import" "from"
  "class" "interface" "type" "extends" "new" "async" "await"
] @keyword

(function_declaration name: (identifier) @function.declaration)
(type_identifier) @type
(predefined_type) @type
(number) @number
