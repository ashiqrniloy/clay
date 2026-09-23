// Plan 118 launcher landing review fixture (isolated capture run only).
// Loads the bundled start surface so the empty tab renders the launcher. No
// workspace or agent package is loaded, so both panes show their first-run
// notes — the honest fresh-install landing state.

import { loadPackage } from "clay:packages";
import { setAppearance, setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
setAppearance("dark");

await loadPackage("@clay/launcher");
