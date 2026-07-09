// End-to-end file browser workflow smoke fixture.
// ~/.config/clay/init.js equivalent: load first-party language packages and
// bind only documented Clay command IDs needed for the six-step workflow.
import { bindKey } from "clay:keybindings";
import { clientCopySelection } from "clay:editor";
import { loadPackage } from "clay:packages";
import { setTheme } from "clay:theme";
import { clientOpenFolderDialog } from "clay:workspace";

setTheme("@clay/theme-gruvbox-material-dark");

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
bindKey("Ctrl+P", "clay.workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "clay.workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
