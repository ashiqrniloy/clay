# Clay UI review capture

- Fixture: ui-review-launcher
- Window size: recorded in metadata.txt (window_requested_size, window_viewport)
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

Runtime evidence: `runtime-tree.txt` records the launcher landing.

- Measured viewport: 1024x1107 (window frame 1076x1203, cropped screenshot 1024x1151)
