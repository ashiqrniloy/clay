// Plan 112 task 13 configuration fixture: deny case. Selecting a third-party
// pack that was never loaded must fail closed with `theme.load_failed`
// (load ≠ select), leave no active icon-pack snapshot, and keep the bundled
// fallback subset active. The rejection is caught and recorded so the
// configuration evaluation itself succeeds with a sanitized diagnostic trail.

import { setIconPack } from "clay:theme";

try {
  setIconPack("@vendor/unloaded-icons");
  Deno.core.ops.op_clay_runtime_record("unexpected-success");
} catch (error) {
  Deno.core.ops.op_clay_runtime_record(`denied:${String(error).slice(0, 64)}`);
}
