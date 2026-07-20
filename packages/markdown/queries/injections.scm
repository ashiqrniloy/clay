; @clay/markdown injection query. Declares composite-grammar layers for the
; generic injection executor: the host block grammar parses first, then each
; @injection.content range is re-parsed with the grammar registered under the
; resolved injection language name (`#set! injection.language "..."` or the
; text of an @injection.language capture). Trimmed from upstream
; tree-sitter-md 0.5.6 to the layers Clay can resolve; unregistered language
; names (e.g. fenced-code info strings) are skipped by the executor.
((inline) @injection.content
  (#set! injection.language "markdown_inline"))
((pipe_table_cell) @injection.content
  (#set! injection.language "markdown_inline"))

(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content)
