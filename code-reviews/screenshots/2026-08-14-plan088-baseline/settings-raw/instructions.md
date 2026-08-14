# Clay UI review capture

- Fixture: ui-review-completion
- Logical window: 900×600
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 temporary HOME/config/data/socket root and
fixture-only documents. It never reads the ambient Clay configuration.

Interactive step: focus the editor, type `hel` if needed, press `Ctrl+Space`,
then press Enter in the terminal to capture the visible completion menu. The
script records UNRESOLVED instead of passing if completion is not visible.
