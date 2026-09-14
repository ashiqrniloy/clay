# Clay UI review capture

- Fixture: ui-review-workspace
- Window size: recorded in metadata.txt (window_requested_size, window_viewport)
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.

Driven steps (AT-SPI, no input synthesis):

```

(process:2313230): dbind-WARNING **: 19:43:29.315: AT-SPI: Unable to open bus connection: Failed to connect to socket /run/user/1000/at-spi2-VASRV3/socket: No such file or directory
OK step 1: focus 'Palette'
OK step 2: wait 600ms
```

- Measured viewport: 1500x1107 (window frame 1552x1203, cropped screenshot 1500x1151)
