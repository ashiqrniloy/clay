// Plan 118 review fixture: the Settings panel open over a document workspace.
// `settings.open` is a built-in server command routed to the client, so it is
// dispatchable from init.js without a package load.

import { commandDispatch } from "clay:agent";

try {
  await commandDispatch({ name: "settings.open" });
} catch {
  // The panel also opens from the shell entry point; the capture records FAIL
  // rather than a settings state when neither path lands.
}
