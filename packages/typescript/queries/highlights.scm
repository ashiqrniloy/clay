; @clay/typescript Tree-sitter highlight query.
; Shared by the typescript and tsx native descriptors. JSX node types are not
; in the TS grammar, so JSX tags stay on the JavaScript query only.
; Captures map through the native descriptor style_map.

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
  "=>" "<<" ">>" ">>>" "|" "^" "&" "~"
] @operator

[
  "export" "import" "from" "function" "const" "let" "var" "return"
  "if" "else" "for" "while" "class" "interface" "type" "extends"
  "implements" "new" "async" "await" "static" "public" "private"
  "protected" "readonly" "abstract" "override" "declare" "namespace"
  "enum" "keyof" "typeof" "satisfies" "is" "in" "of" "yield" "throw"
  "try" "catch" "finally" "switch" "case" "default" "break" "continue"
  "debugger" "do" "void" "with" "as"
] @keyword

[
  (true) (false) (null) (undefined)
] @constant.builtin

(predefined_type) @type.builtin
(type_identifier) @type
(type_parameters (type_parameter (type_identifier) @type.parameter))
(type_arguments (type_identifier) @type)

(function_declaration name: (identifier) @function.declaration)
(method_definition name: (property_identifier) @function.declaration)
(arrow_function parameter: (identifier) @variable.parameter)
(required_parameter (identifier) @variable.parameter)
(optional_parameter (identifier) @variable.parameter)

(call_expression function: (identifier) @function)
(call_expression
  function: (member_expression property: (property_identifier) @function.method))

(property_identifier) @property
(property_signature name: (property_identifier) @property)
(public_field_definition name: (property_identifier) @property)
(pair key: (property_identifier) @property)
(member_expression property: (property_identifier) @property)
