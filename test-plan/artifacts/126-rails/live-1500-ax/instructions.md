# Clay UI review capture

- Fixture: ui-review-workspace
- Window size: recorded in metadata.txt (window_requested_size, window_viewport)
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

- Measured viewport: 1500x1104 (window frame 1552x1203, cropped screenshot 1500x1151)

Server diagnostics (bounded, root-redacted): `server.diagnostics.txt`.

Driven steps (AT-SPI, no input synthesis):

```
OK step 1: wait 50000ms
```

- Measured viewport: 900x1104 (window frame 952x1203, cropped screenshot 944x1151)

Server diagnostics (bounded, root-redacted): `server.diagnostics.txt`.
