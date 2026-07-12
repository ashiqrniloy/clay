; @clay/markdown Tree-sitter block highlight query.
; Captures map to Phase 18.15 vocabulary through the native descriptor styleMap.
(atx_heading (inline) @text)
(setext_heading (paragraph) @text)

[
  (atx_h1_marker)
  (atx_h2_marker)
  (atx_h3_marker)
  (atx_h4_marker)
  (atx_h5_marker)
  (atx_h6_marker)
  (setext_h1_underline)
  (setext_h2_underline)
  (list_marker_plus)
  (list_marker_minus)
  (list_marker_star)
  (list_marker_dot)
  (list_marker_parenthesis)
  (fenced_code_block_delimiter)
  (block_quote_marker)
] @punctuation

[
  (indented_code_block)
  (fenced_code_block)
] @code

(paragraph) @text
