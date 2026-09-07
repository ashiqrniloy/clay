// Plan 112 task 13 configuration fixture: the documented minimal load+select
// path. `loadPackage` registers only (load ≠ select); `setIconPack` selects.
// Zero icon lines would keep the bundled Regular subset active instead.

import { loadPackage } from "clay:packages";
import { setIconPack } from "clay:theme";

await loadPackage("@clay/icons-phosphor-regular");

const summary = setIconPack("@clay/icons-phosphor-regular");
Deno.core.ops.op_clay_runtime_record(
  `icons:${summary.pack}:${summary.iconCount}:v${summary.schemaVersion}`,
);
