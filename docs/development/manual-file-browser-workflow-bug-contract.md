# Manual File Browser Workflow Bug Contract

This file locks the real `cargo run` regressions reported from Linux/GNOME manual testing before Plan 044 fixes them. It is a repro contract, not implementation guidance.

## Manual product path

1. Put workflow config in `~/.config/clay/init.js` with `loadPackage("@clay/markdown")`, `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, `loadPackage("@clay/javascript")`, and `bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" })`.
2. Run `cargo run` from the repository root.
3. Use the native UI directly; do not use `cargo run -- smoke-gui --config-fixture file-browser-workflow` for this repro.

## Locked failures

| Failure | Manual repro | Observed evidence | Owning layer |
| --- | --- | --- | --- |
| Shifted folder-picker binding does not fire | Press `Ctrl+Shift+O` on GNOME/Linux after binding `clientOpenFolderDialog()` | Native folder picker does not open because the manifest binding stores lowercase `o` while the keyboard event supplies uppercase `O` | keybinding route / behavior manifest lookup |
| Nested file rows fail to open | Open a selected folder, navigate into `src/`, click `src/main.rs` | Server reports `ActionSourceMismatch(SduiNodeId(5))`; row id is `main.rs` but action source item id is `src/main.rs` | SDUI list/action identity |
| File-browser actions break after Markdown activation | Open a `.md` file, then click another file or directory | Server reports `UnknownActionCommand("clay.workspace.openFile")` after Markdown behavior install / parse-time diagnostic | server `StaticSduiState` workspace-browser validation |
| Parse timeout poisons workflow instead of degrading | Open Markdown and wait for open-time follow-up | UI shows `clay.parse.open_activation_timeout` / "Open-time parse did not finish before the decoration freshness deadline." and subsequent navigation stops working | open-document follow-up diagnostics and server SDUI validation |
| Second file does not replace first file | Open one `.md` file, then open a second `.md` file | First file remains visible; second file contents do not replace the editor buffer | document-open event application / editor snapshot replacement |
| Editor overlaps the file browser after opening a file | Open a workspace file while the left browser is visible | File-browser text and editor text visually overlap because the editor region falls back to the full rect when the active document id differs from the SDUI editor binding | editor region computation / shell pane layout |
| Purple decorative circle remains visible | Launch/open any file and inspect editor background | Permanent bottom-right purple circle distracts from the text area | editor paint chrome |
| Visible editor card padding remains | Launch/open any file and inspect text area edges | Editor paints an inset card/padding region instead of using the main working area cleanly | editor paint chrome |
| File browser cannot scroll | Select a folder with more rows than the left pane height and use mouse wheel over the browser | Later entries are not reachable; scroll events go to the editor instead | SDUI scroll state / pointer scroll routing |
| Main text area lacks visible scroller | Open a long file and scroll the editor | Text moves via existing scroll state, but no vertical scrollbar/thumb communicates position | editor scroll chrome |

## Entry-gate rules for fixes

- Repros must remain headless or documented manual notes; tests must not open a real GUI, spawn shell commands from test code, or require xdg-desktop-portal.
- Headless tests may use fixtures, temp workspaces, and in-memory SDUI/editor state only.
- No repro or fix may broaden filesystem authority: workspace files stay root-relative or selected-file validated, selected-folder grants stay server-validated, and raw absolute paths must not be asserted in user-facing diagnostics.
- Hot paths stay client-local: typing, paint, layout, pointer movement, wheel scrolling, and ordinary selection/copy routing must not run package JavaScript, server IPC, filesystem work, shell commands, or full-document serialization.
- Packages still cannot call raw `Deno.core.ops`, access native widget handles, scan arbitrary paths, read the clipboard, paste/cut, or write arbitrary clipboard text.

## Follow-up tests planned

- `keybinding_shifted_character_routes_client_ui_command`
- `file_browser_nested_file_action_source_matches_list_item_id`
- `file_browser_actions_still_validate_after_markdown_open_timeout`
- `opening_second_workspace_file_replaces_editor_snapshot`
- `file_browser_left_slot_still_reserves_editor_region_after_document_open`
