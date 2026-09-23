// Editor performance matrix fixture (Plan 099).
//
// Loaded via `clay server <endpoint> --config-fixture clay-performance-matrix`.
// Preloads every first-party language package so mode activation and native
// syntax sessions are warm, and binds the commands the manual matrix flows
// need (open, fold, save, split panes) so a designated-device operator can
// drive the open/type/scroll/fold/save/reload checklist without rebinding.
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");

await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");

bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
bindKey("Ctrl+Shift+O", "workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+Shift+F", "editor.clientToggleFold", { scope: "editor" });
bindKey("Ctrl+\\", "shell.clientSplitPaneVertical", { scope: "editor" });
bindKey("Ctrl+-", "shell.clientSplitPaneHorizontal", { scope: "editor" });
