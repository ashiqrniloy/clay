# @clay/design-instrument

**Quiet Instrument** — Clay's shipped UI design system: one plane, hairline zones,
two elevations, accent reserved for state.

## Structure

- `package.json`: declarative manifest contributing `clay.contributions.uiDesignSystem`
  (schema version 1, 165 recipes, 14 design-system-local values). Inert data only —
  zero permissions, zero modes, no `entry`, no JavaScript.
- `docs/index.md`: the profile, the recipe reference, and the invariants.

## Status

Authored by plan 118 task 8 from `DESIGN.md` §4–§11 and the approved artifact set
(`design-artifacts/approved/quiet-instrument-migration/`). It is **not yet the
runtime default**: the legacy Neobrutal package held that until task 9 removed it,
and task 16 (core fallbacks + host CSS adoption) is what makes this package the
implicit default; the theme `designTokens` overrides land in task 14. Until then
this package validates and is inert, but no host surface consumes its values.

## Development & Verification

```bash
cargo test --test presentation package_ui_conformance::
cargo test --test presentation theme_packages::
```

The package is asserted to:

- assemble as inert data (no permissions, no modes, no entry, no executable surface);
- carry exactly the reference key set (142) minus the 12 `chat.default.*` keys
  whose surface the migration deletes, plus the families the target IA and the
  language need (165 total) — the delta is a test, not an assumption;
- reference only semantic theme colour roles (`ThemeColorRef`), never a literal;
- stay inside the design-system payload budget (64 KiB);
- reject mutated probes (literal colours, out-of-bounds radius/border/duration/
  opacity, too many shadow layers).
