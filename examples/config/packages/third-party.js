// ============================================================================
// Clay canonical example configuration — examples/config/packages/third-party.js
// ============================================================================
//
// Template for third-party package configuration. Loaded from
// examples/config/init.js (section 11) via:
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
//   1. Install with the host CLI (durable, user-approved):
//        clay install npm:<spec>        e.g. npm:@arnilo/st or npm:@arnilo/st@1.2.3
//
//      `clay install` appends a commented, self-removing block to
//      ~/.config/clay/init.js (idempotent — one block per package):
//
//        // clay install npm:@scope/name — remove with `clay remove npm:@scope/name`
//        await loadPackage("@scope/name");
//
//      `clay remove npm:<spec>` deletes the block it wrote and leaves
//      hand-edited lines untouched. A version in the spec (npm:<name>@1.2.3)
//      is PINNED: `clay update --extensions` skips it; a bare spec
//      (npm:<name>) is FLOATING and updates in place.
//
//      Installing never enables, adopts, or executes the package.
//   2. Approve with the host CLI before the package can run:
//        clay package adopt <name>
//   3. loadPackage here consumes that approved state. It validates,
//      enables, and imports the package's declared load entry with
//      host-stamped provenance — it does NOT bypass approval. A load line
//      for an un-adopted third-party package fails closed with an
//      adoption diagnostic; its JavaScript never runs.
//
// See docs/reference/packages/creating-packages.md, the `clay install`/
// `clay package` CLI help, and docs/development/distribution.md for the
// full install/remove/update/adopt flow.

import { loadPackage } from "clay:packages";

// await loadPackage("@vendor/my-package");
// await loadPackage("@vendor/mode");   — the name `clay install` appended
// — or let the host CLI write the line for you:
//   clay install npm:@vendor/mode
//   (appends the block shown in the header above; adopt before it runs)

// Third-party icon packs follow load-then-select (the icon style itself is
// chosen by setIconPack in init.js section 2; loading alone changes nothing):
//   await loadPackage("@vendor/outline-icons");
//   // then in init.js (or a module imported here):
//   // setIconPack("@vendor/outline-icons");

// Language-server grants for third-party packages follow the same
// grant-before-loadPackage ordering as first-party packages:
//
// import { authorizeLanguageServer } from "clay:language-server";
// await authorizeLanguageServer({
//   package: "@vendor/my-package",
//   contribution: "my-package.server",
//   workspaceRootIds: [1],
// });
