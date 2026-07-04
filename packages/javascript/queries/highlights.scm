; @clay/javascript Tree-sitter highlight query.
; Captures map to known Clay style tokens via package styleMap.
(comment) @comment
(string) @string
(template_string) @string

["{" "}" "(" ")" "[" "]"] @punctuation

[
  "export" "function" "const" "return" "if" "else" "for" "while" "import" "from"
  "class" "extends" "new" "async" "await" "let" "var"
] @keyword
