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

(process:2518676): dbind-WARNING **: 21:05:16.267: AT-SPI: Unable to open bus connection: Failed to connect to socket /run/user/1000/at-spi2-M4EPV3/socket: No such file or directory

(process:2518676): dbind-WARNING **: 21:05:16.269: AT-SPI: Unable to open bus connection: Failed to connect to socket /run/user/1000/at-spi2-MHPRV3/socket: No such file or directory

(process:2518676): dbind-WARNING **: 21:05:16.269: AT-SPI: Unable to open bus connection: Failed to connect to socket /run/user/1000/at-spi2-3FSRV3/socket: No such file or directory

(process:2518676): dbind-WARNING **: 21:05:16.271: AT-SPI: Unable to open bus connection: Failed to connect to socket /run/user/1000/at-spi2-1JXRV3/socket: No such file or directory
/tmp/clay-ui-review.THOcwT/atspi_probe.py:201: DeprecationWarning: Atspi.Action.get_action_name is deprecated
  candidate = clean(Atspi.Action.get_action_name(node, index)).lower()
OK step 1: click 'Open' via 'press'
OK step 2: wait 1200ms
```

- Measured viewport: 1024x1107 (window frame 1076x1203, cropped screenshot 1024x1151)
