; @clay/rust Tree-sitter highlight query.
; Captures map to Phase 18.15 vocabulary (TokenType + Modifiers) through the
; package styleMap / native descriptor style_map. Only captures present in the
; styleMap are emitted; unmatched nodes stay unstyled (no default color leak).

; Comments
(line_comment) @comment
(block_comment) @comment

; Strings + chars
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

; Brackets
[
  "{" "}" "(" ")" "[" "]"
] @punctuation.bracket

; Delimiters
[
  "::" ":" "." "," ";"
] @punctuation.delimiter

; Operators
[
  "*" "&" "'" "+" "-" "/" "%" "==" "!=" "=="
  "!" "<" ">" "<=" ">=" "=" "<<" ">>" "|"
  "^"  "||" "&&" "+=" "-=" "*=" "/=" "%="
] @operator

; Keywords
[
  "fn" "let" "const" "static" "return" "if" "else" "match"
  "while" "for" "loop" "use" "mod" "struct" "enum" "impl" "trait"
  "type" "where" "as" "in" "unsafe" "extern" "pub"
  "async" "await" "move" "ref" "break" "continue" "default"
] @keyword

; `crate`, `self`, `super`, and `mut` are grammar nodes, not bare keyword tokens.
(crate) @keyword
(self) @keyword
(super) @keyword
(mutable_specifier) @keyword

; Primitive / built-in types
(primitive_type) @type.builtin

; User-defined types
(type_identifier) @type

; Lifetime as a type-parameter marker
(lifetime (identifier) @type.lifetime)

; Generic type parameters
(type_parameters (type_parameter (type_identifier) @type.parameter))
(type_arguments (type_identifier) @type)

; Function definitions
(function_item (identifier) @function.declaration)
(function_signature_item (identifier) @function.declaration)

; Function / method calls
(call_expression function: (identifier) @function)
(call_expression function: (scoped_identifier name: (identifier) @function))
(call_expression
  function: (field_expression field: (field_identifier) @function.method))
(generic_function
  function: (field_expression field: (field_identifier) @function.method))

; Field access
(field_expression field: (field_identifier) @property)
(field_declaration name: (field_identifier) @property)
(shorthand_field_identifier) @property

; Macro invocations
(macro_invocation macro: (identifier) @function.macro "!" @punctuation.delimiter)

; Attributes / decorators
(attribute_item) @attribute
(inner_attribute_item) @attribute

; Function parameters and self
(parameters (parameter (identifier) @variable.parameter))
(parameters (self_parameter) @variable.parameter)
((self) @variable.parameter)

; Constants by convention (ALL_CAPS)
((identifier) @constant
 (#match? @constant "^[A-Z][A-Z\\d_]+$"))

; Numeric literals
(integer_literal) @number
(float_literal) @number

; Boolean literals
(boolean_literal) @constant.builtin
