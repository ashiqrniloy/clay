import { authorizeLanguageServer } from "clay:language-server";
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
bindKey({
  scope: "editor",
  bindings: { "Ctrl+Alt+I": "editor.toggleInlayHints" },
});
await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
