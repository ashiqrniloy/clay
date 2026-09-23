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
//      ~/.clay/init.js (idempotent — one block per package):
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
//   3. Grant the capabilities its manifest declares, if it declares any
//      (completion-provider, parse-document, …). A declared capability that was
//      never granted fails closed with MissingCapabilityGrant; language-server
//      is the one exception (session start is grant-gated instead):
//        clay package authorize <name> --capability completion-provider
//      The "Capability grants" section below has the full contract.
//   4. loadPackage here consumes that approved, granted state. It validates,
//      enables, and imports the package's declared load entry with
//      host-stamped provenance — it does NOT bypass approval. A load line
//      for an un-adopted third-party package fails closed with an
//      adoption diagnostic; its JavaScript never runs.
//
// See docs/reference/packages/creating-packages.md, the `clay install`/
// `clay package` CLI help, docs/reference/clay-js-api/packages/authorize.md,
// and docs/development/distribution.md for the full
// install/remove/update/adopt/grant flow.

import { loadPackage } from "clay:packages";

// await loadPackage("@vendor/my-package");
// await loadPackage("@vendor/mode");   — the name `clay install` appended
// — or let the host CLI write the line for you:
//   clay install npm:@vendor/mode
//   (appends the block shown in the header above; adopt before it runs)

// ----------------------------------------------------------------------------
// Capability grants — explicit, per capability, revocable (plan 136)
// ----------------------------------------------------------------------------
// A third-party package runs with the capabilities the user granted it and
// nothing else. Grants are the configuration/CLI half of the lifecycle;
// `loadPackage` above only consumes them.
//
// From the host CLI (attributed `cli`; requires a current adoption, so the
// grant is durable rather than process-local — an un-adopted package is refused
// with an `adopt` hint):
//
//   clay package authorize @vendor/my-package --capability completion-provider
//   clay package authorize @vendor/my-package --capability completion-provider \
//     --capability parse-document --runtime-profile native-trust --approved-by user
//
//   --capability <cap>       repeatable and required; the manifest must declare
//                            it (`clay.permissions`, or the legacy
//                            `clay.capabilities` alias). Unknown names and
//                            grant-only authorities are refused.
//   --runtime-profile <p>    native-trust (default) | sandboxed | restricted
//   --approved-by <who>      cli (default) | user | config — the attribution
//                            recorded on the durable grant
//
// From this module or init.js (attributed `config`), before `loadPackage`:
//
// import { authorize } from "clay:packages";
// authorize({
//   package: "@vendor/my-package",          // installed package name (required)
//   capabilities: ["completion-provider"], // declared capabilities (required, 1+)
//   runtimeProfile: "native-trust",        // default "native-trust"
//   approvedBy: "config",                  // "config" | "user" | "cli"
// });
//
// A later grant REPLACES the whole granted set (it does not extend it), so pass
// every capability the package should keep. Inspect and withdraw with the CLI:
//
//   clay package inspect @vendor/my-package
//     → `Adoption: approved`, `Grants: <names> (<profile>)`, `Granted by:`,
//       `Approved by:`, and `Ungranted:` for declared-but-ungranted capabilities
//   clay package revoke @vendor/my-package
//     → withdraws the approval AND the grant (re-adopt to start over)
//
// Only the documented surfaces grant: no JSON/TOML key (`capabilityGrant`,
// `grantedCapabilities`, `packages.authorizedCapabilities`, …) grants a
// capability, and package code cannot grant itself anything — `clay:packages`
// is absent from the third-party runtime and the op refuses while a package is
// loading. A capability the manifest never declares can never be granted, and
// a missing, stale, or revoked grant blocks enable with MissingCapabilityGrant.

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
