// Plan 112 task 13 modular configuration helper: same public facade, imported
// by init.js. Keep this module free of Clay imports other than the documented
// facades so it runs in every configuration evaluation (startup and reload).

import { setIconPack } from "clay:theme";

export function activateIconPack(specifier) {
  const summary = setIconPack(specifier);
  Deno.core.ops.op_clay_runtime_record(
    `icons:${summary.pack}:${summary.iconCount}:v${summary.schemaVersion}`,
  );
  return summary;
}
