# Third-party icon-pack fixtures (Plan 112 task 5)

- `valid-partial/`: adopted third-party pack with own-prefixed keys only
  (`vendor.*`). Partial coverage is legitimate: consumers fall back per key
  (state table row 8/9) without any implicit core-key grant.
- `hostile/`: third-party pack impersonating the core namespace (`action.close`)
  and smuggling a raw `svg` field. Must fail at record assembly (structural deny
  list + first-party core-key gate) through the ordinary adoption path.
