// @clay/git runtime entry. Owns the package manifest factory consumed by the
// load entry and re-exports the package contract. The authoritative manifest
// is `packages/git/package.json`; this factory mirrors it for explicit/per-load
// validation through `packages.serverLoadPackage`.

export const packageName = "@clay/git";
export const apiPrefix = "git";

export function gitPackageManifest() {
  return {
    name: packageName,
    version: "0.1.0",
    type: "module",
    exports: {
      ".": "./dist/index.js",
      "./load": "./dist/load.js",
      "./status": "./dist/status.js"
    },
    clay: {
      apiPrefix,
      entry: "./dist/index.js",
      loadEntry: "./dist/load.js",
      permissions: [],
      modes: [],
      docs: "./docs/index.md",
      apiDependencies: [
        "git.serverListGitStatuses",
        "sdui.publishTree"
      ],
      performance: {
        estimatedManifestBytes: 1500,
        hotPathPolicy: "no hot-path JS; status reads cached Git state at load/update time only"
      },
      extensionPoints: [{"id": "git.statusRegion", "version": 1, "operations": ["replace"], "contributionKinds": ["sduiRegion"], "scopes": ["git.status"], "summary": "Replace the Git status SDUI region."}, {"id": "git.contributions", "version": 1, "operations": ["append"], "contributionKinds": ["command", "decorationLayer", "statusItem", "panelContribution", "componentContribution", "sduiRegion"], "summary": "Add package-owned commands, decorations, status items, and panels built on cached Git status."}],
      contributions: {
        commands: [],
        configuration: [],
        keyRouting: [],
        textTransforms: [],
        sdui: [
          {
            regionId: "git.status",
            displayName: "Git Status",
            adapter: "./dist/status.js",
            estimatedSnapshotBytes: 1024,
            estimatedUpdateBytes: 256
          }
        ],
        decorations: []
      }
    }
  };
}

export { gitPackageContract, loadGitPackage } from "./load.js";
