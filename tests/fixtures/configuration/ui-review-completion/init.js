import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });
