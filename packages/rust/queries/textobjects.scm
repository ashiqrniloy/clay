;; Tree-sitter text objects for Rust (Plan 071 task 10).
;; Capture names follow `textobject.<kind>.<scope>`: `around` covers the whole
;; node, `inner` the meaningful interior. Kinds without an `inner` capture fall
;; back to `around` at query time.

(function_item) @textobject.function.around
(function_item body: (block) @textobject.function.inner)

;; Rust has no classes; struct/enum/trait/impl bodies are the closest analog.
(struct_item) @textobject.class.around
(enum_item) @textobject.class.around
(trait_item) @textobject.class.around
(impl_item) @textobject.class.around
(struct_item body: (field_declaration_list) @textobject.class.inner)
(enum_item body: (enum_variant_list) @textobject.class.inner)
(trait_item body: (declaration_list) @textobject.class.inner)
(impl_item body: (declaration_list) @textobject.class.inner)

(parameters) @textobject.argument.around
(arguments) @textobject.argument.around

(line_comment) @textobject.comment.around
(block_comment) @textobject.comment.around

(for_expression) @textobject.loop.around
(while_expression) @textobject.loop.around
(loop_expression) @textobject.loop.around

(if_expression) @textobject.conditional.around
(match_expression) @textobject.conditional.around

(call_expression) @textobject.call.around
(call_expression arguments: (arguments) @textobject.call.inner)

(expression_statement) @textobject.statement.around
(let_declaration) @textobject.statement.around
