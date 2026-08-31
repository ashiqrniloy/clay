# Clay UI review capture

- Fixture: ui-review-recovery
- Logical window: 900×600
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

The script stops the isolated server after the connected tree appears and
captures the resulting disconnected/recovery state.
