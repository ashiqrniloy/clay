// Plan 129 module 10 fixture: a two-stroke sequence chord (module 10 K60).
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Q Ctrl+W", "editor.clientMoveCursor.nextWordStart", { scope: "editor" });
