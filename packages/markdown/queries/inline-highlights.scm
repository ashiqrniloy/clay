; @clay/markdown inline-grammar highlight query (markdown_inline injection
; layer). Capture names match the block query's styleMap keys so inline spans
; flow through MARKDOWN_NATIVE_STYLE_MAP.
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

(link_text) @link
(link_destination) @link-url
