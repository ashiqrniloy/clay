// Plan 127 lane-scheduling manual step: bundled packages active, completion
// trigger bound. @clay/markdown's parse handler is the real package parse
// handler that runs for .md edits.
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
// Ctrl+Space is the host's GNOME input-source switch, so bind a chord the
// compositor does not consume. Ctrl+Space stays bound for parity with the other
// fixtures.
bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });
bindKey("Ctrl+J", "completion.trigger", { scope: "editor" });
