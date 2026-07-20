; @clay/markdown inline-grammar highlight query (markdown_inline injection
; layer). Capture names intentionally match the block query's styleMap keys so
; inline spans flow through the same MARKDOWN_NATIVE_STYLE_MAP vocabulary
; mapping; node ranges come from the real inline grammar instead of the old
; whole-`(inline)` regex predicates.
(strong_emphasis) @strong

(emphasis) @emphasis

(code_span) @code-span

[
  (inline_link)
  (full_reference_link)
  (collapsed_reference_link)
  (shortcut_link)
  (uri_autolink)
  (email_autolink)
] @link
