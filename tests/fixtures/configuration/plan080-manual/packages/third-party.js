// ============================================================================
// Clay canonical example configuration — examples/packages/third-party.js
// ============================================================================
//
// Template for third-party package configuration. Loaded from
// examples/init.js (section 11) via:
//
//   await loadConfigurationModule({ path: "./packages/third-party.js", optional: true });
//
// No third-party packages ship with Clay; this file is a commented template.
// A broken or missing module records configuration.module_failed and never
// blocks the base configuration or app launch.
//
// Third-party packages are NOT raw-loaded from this file. Adoption follows
// the Clay package authority model:
//
//   1. Install + approve with the host CLI (durable, user-approved):
//        clay package add <spec>
//        clay package adopt <name>
//   2. loadPackage here consumes that approved state. It validates,
//      enables, and imports the package's declared load entry with
//      host-stamped provenance — it does NOT bypass approval.
//
// See docs/reference/packages/creating-packages.md and the `clay package`
// CLI help for the full install/enable/adopt/revoke/rollback flow.

import { loadPackage } from "clay:packages";

// await loadPackage("@vendor/my-package");
// await loadPackage("github:user/repo");

// Language-server grants for third-party packages follow the same
// grant-before-loadPackage ordering as first-party packages:
//
// import { authorizeLanguageServer } from "clay:language-server";
// await authorizeLanguageServer({
//   package: "@vendor/my-package",
//   contribution: "my-package.server",
//   workspaceRootIds: [1],
// });
