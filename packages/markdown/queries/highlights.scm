; @clay/markdown Tree-sitter block + inline highlight query.
; Captures map to Phase 18.15 vocabulary (TokenType + Modifiers) through the
; native descriptor style_map. Headings resolve to Heading1..6; **strong** and
; *emphasis* carry Bold/Italic modifiers; code spans/blocks use the monospace
; document role. Only captures present in the style_map are emitted; unmatched
; nodes stay unstyled.

; Headings: the inline text resolves to the per-level TokenType, the marker
; stays punctuation.
(atx_heading (atx_h1_marker) (inline) @heading-1)
(atx_heading (atx_h2_marker) (inline) @heading-2)
(atx_heading (atx_h3_marker) (inline) @heading-3)
(atx_heading (atx_h4_marker) (inline) @heading-4)
(atx_heading (atx_h5_marker) (inline) @heading-5)
(atx_heading (atx_h6_marker) (inline) @heading-6)
(setext_heading (paragraph) @heading-1 (setext_h1_underline))
(setext_heading (paragraph) @heading-2 (setext_h2_underline))

[
  (atx_h1_marker)
  (atx_h2_marker)
  (atx_h3_marker)
  (atx_h4_marker)
  (atx_h5_marker)
  (atx_h6_marker)
  (setext_h1_underline)
  (setext_h2_underline)
  (fenced_code_block_delimiter)
] @punctuation

[
  (list_marker_plus)
  (list_marker_minus)
  (list_marker_star)
  (list_marker_dot)
  (list_marker_parenthesis)
] @list-marker

(block_quote_marker) @quote

[
  (indented_code_block)
  (fenced_code_block)
] @code

; ponytail: Tier 1 currently uses tree-sitter-md's block grammar, so predicates
; classify standalone inline forms from the raw `inline` range. Mixed inline
; runs remain paragraph text; add generic block+inline grammar composition when
; partial-run styling is required.
((inline) @strong
  (#match? @strong "^\\*\\*[^\\n]+\\*\\*$"))
((inline) @emphasis
  (#match? @emphasis "^[_*][^\\n]+[_*]$"))
((inline) @code-span
  (#match? @code-span "^`[^\\n]+`$"))
((inline) @link
  (#match? @link "^\\[[^]\\n]+\\]\\([^\\n)]+\\)$"))

(paragraph) @text
