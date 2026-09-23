// Plan 112 task 13 configuration fixture: deterministic startup with no
// selection. No load lines, no setIconPack call: the bundled Regular safety
// subset stays active and no icon-pack snapshot is emitted.

import { setAppearance } from "clay:theme";

setAppearance("dark");
