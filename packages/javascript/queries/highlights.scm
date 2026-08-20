; @clay/javascript Tree-sitter highlight query.
; Captures map to Phase 18.15 vocabulary (TokenType + Modifiers) through the
; package styleMap / native descriptor style_map. Only captures present in the
; styleMap are emitted; unmatched nodes stay unstyled (no default color leak).

(comment) @comment
(string) @string
(template_string) @string
(regex) @string.regexp
(number) @number

[
  "{" "}" "(" ")" "[" "]"
] @punctuation.bracket

[
  ":" "." "," ";"
] @punctuation.delimiter

(optional_chain) @punctuation.delimiter
(template_substitution "${" @punctuation.special "}" @punctuation.special)

[
  "===" "!==" "==" "!=" "<" ">" "<=" ">=" "+" "-" "*" "/" "%"
  "++" "--" "+=" "-=" "*=" "/=" "%=" "&&" "||" "!" "=" "??" "??="
  "=>" "<<" ">>" ">>>" "|" "^" "&" "~" "instanceof" "typeof" "in" "of"
] @operator

[
  "export" "import" "from" "function" "const" "let" "var" "return"
  "if" "else" "for" "while" "class" "extends" "new" "async" "await"
  "static" "get" "set" "yield" "throw" "try" "catch" "finally"
  "switch" "case" "default" "break" "continue" "debugger" "do"
  "void" "with" "typeof" "delete" "in" "of"
] @keyword

[
  (true) (false) (null) (undefined)
] @constant.builtin

((identifier) @variable.builtin
 (#match? @variable.builtin "^(arguments|module|console|window|document|globalThis|self)$"))

(function_declaration name: (identifier) @function.declaration)
(function_expression name: (identifier) @function.declaration)
(method_definition name: (property_identifier) @function.declaration)
(generator_function_declaration name: (identifier) @function.declaration)
(generator_function name: (identifier) @function.declaration)
(arrow_function parameter: (identifier) @variable.parameter)
(formal_parameters (identifier) @variable.parameter)

(call_expression function: (identifier) @function)
(call_expression
  function: (member_expression property: (property_identifier) @function.method))

(property_identifier) @property
(pair key: (property_identifier) @property)
(member_expression property: (property_identifier) @property)

(jsx_opening_element name: (identifier) @type)
(jsx_closing_element name: (identifier) @type)
(jsx_self_closing_element name: (identifier) @type)
