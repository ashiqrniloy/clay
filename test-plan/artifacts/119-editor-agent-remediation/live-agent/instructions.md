# Clay UI review capture

- Fixture: ui-review-coding-agent
- Window size: recorded in metadata.txt (window_requested_size, window_viewport)
- Screenshot: inspected, then removed (see `privacy.txt`)
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

Driven steps (AT-SPI, no input synthesis):

```
OK step 1: click 'Agent'
OK step 2: wait 600ms
```

- Measured viewport: 1280x1104 (window frame 1332x1203, cropped screenshot 1280x1151)

Server diagnostics (bounded, root-redacted): `server.diagnostics.txt`.
