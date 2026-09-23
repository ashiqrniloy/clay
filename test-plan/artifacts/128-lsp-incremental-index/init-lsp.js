// Plan 128 manual fixture: opt-in rust-analyzer intelligence plus a keyboard
// binding for the completion trigger (the host's GNOME input-source switch eats
// Ctrl+Space; Ctrl+J is the documented alternate in the plan 127 artifacts).
import { bindKey } from "clay:keybindings";
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
bindKey("Ctrl+J", "completion.trigger", { scope: "editor" });
bindKey("Ctrl+U", "editor.toggleInlayHints", { scope: "editor" });