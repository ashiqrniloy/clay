# UI Visual and Accessibility Review

- Before reviewing UI or launching a visual/accessibility audit, run `npx ui-skills start`, inspect the relevant category, and load the smallest useful skill set (prefer 1, max 3); record selected slugs with the review evidence.
- Every UI-changing Clay plan includes one post-implementation visual and accessibility review task before final docs/wiki work.
- Launch representative states, capture and inspect screenshots, retain their paths and findings in completion evidence.
- When `computer-use-linux` is available, call `get_app_state` first and verify accessibility tree semantics, keyboard flow, visible focus, modal containment, and announcements for changed controls.
- If live review tooling is unavailable, record the exact blocker and leave manual acceptance unresolved; structural tests are not visual proof.
- Decision source: `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
