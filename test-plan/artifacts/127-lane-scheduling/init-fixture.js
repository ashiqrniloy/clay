// Plan 127 lane-scheduling fixture config: load the third-party fixture package
// (whose capability grants have no user-facing surface on this build) and bind
// the completion trigger. Expected live: the load fails closed with a sanitized
// diagnostic and no package code runs.
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@fixture/lane");
bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });

// Read by fixture-package/parser.js (the deliberate general-lane hold).
globalThis.clayLaneBusyMs = 1000;
