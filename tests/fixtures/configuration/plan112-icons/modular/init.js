// Plan 112 task 13 configuration fixture: modular configuration. The icon
// selection lives in a local module imported by init.js, proving the same
// facade behavior applies from modular imports, not only inline init.js calls.

import { activateIconPack } from "./icons-config.js";

activateIconPack("@clay/icons-phosphor-duotone");
