;; Tree-sitter text objects for TypeScript (Plan 071 task 10). Shared by the
;; `typescript` and `tsx` native descriptors. Capture names follow
;; `textobject.<kind>.<scope>`; kinds without an `inner` capture fall back to
;; `around` at query time.

(function_declaration) @textobject.function.around
(function_declaration body: (statement_block) @textobject.function.inner)
(function_signature) @textobject.function.around
(method_definition) @textobject.function.around
(method_definition body: (statement_block) @textobject.function.inner)
(arrow_function) @textobject.function.around
(arrow_function body: (statement_block) @textobject.function.inner)

(class_declaration) @textobject.class.around
(class_declaration body: (class_body) @textobject.class.inner)
(abstract_class_declaration) @textobject.class.around
(abstract_class_declaration body: (class_body) @textobject.class.inner)
(interface_declaration) @textobject.class.around
(enum_declaration) @textobject.class.around

(formal_parameters) @textobject.argument.around
(arguments) @textobject.argument.around

(comment) @textobject.comment.around

(for_statement) @textobject.loop.around
(for_in_statement) @textobject.loop.around
(while_statement) @textobject.loop.around
(do_statement) @textobject.loop.around

(if_statement) @textobject.conditional.around
(switch_statement) @textobject.conditional.around
(ternary_expression) @textobject.conditional.around

(call_expression) @textobject.call.around
(call_expression arguments: (arguments) @textobject.call.inner)

(expression_statement) @textobject.statement.around
(lexical_declaration) @textobject.statement.around
(variable_declaration) @textobject.statement.around
