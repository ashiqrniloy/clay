// Plan 129 module 04 fixture: bundled @clay/rust plus an explicit completion
// trigger on Ctrl+J (Ctrl+Space is consumed by this host's GNOME input-source
// switch, recorded as a host ceiling in the module 04/10 records).
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });
bindKey("Ctrl+J", "completion.trigger", { scope: "editor" });
