# Clay UI review capture

- Fixture: ui-review-large-document
- Window size: recorded in metadata.txt (window_requested_size, window_viewport)
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

The workspace holds a ≥4 MiB `review.rs` (the completion fixture's init.js is
copied, so the `Ctrl+Space` binding and the `@clay/rust` provider are active).
Plan 126 steps: the document opens through the chunked path, the editor is
read-only until ready, then an insertion at the end of the file drives the live
typing path on a document that stays past 4 MiB.

Driven steps (AT-SPI, no input synthesis):

```
OK step 1: wait 9000ms
```

- Measured viewport: 1500x1104 (window frame 1552x1203, cropped screenshot 1500x1151)

Server diagnostics (bounded, root-redacted): `server.diagnostics.txt`.
