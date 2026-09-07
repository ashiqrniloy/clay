# Clay UI review capture

- Fixture: ui-review-design-system-light
- Logical window: 900×600
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

The fixture selects the built-in `@clay/core` design system explicitly and
publishes a representative panel, action, enabled list row, disabled list row,
and editor view. It is paired with the theme named in the fixture.

Runtime evidence: `runtime-tree.txt` records explicit core activation and host-owned states.
