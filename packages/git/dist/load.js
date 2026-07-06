// @clay/git load entry.
//
// `loadPackage("@clay/git")` imports this module and invokes its default
// export. The package owns the read-only Git status UI composition only: it
// loads its manifest and publishes a sanitized status tree backed by the
// server-owned `clay:git` facade. It declares no permissions and receives no
// shell, network, filesystem, or mutating Git authority.

import { serverLoadPackage } from "clay:packages";
import { defineLabel, definePanel, defineStack, publishTree } from "clay:sdui";
import { gitPackageManifest } from "./index.js";
import { publishGitStatus } from "./status.js";

const SDUI = Object.freeze({
  defineLabel,
  definePanel,
  defineStack,
  publishTree
});

export function gitPackageContract() {
  return {
    packageName: "@clay/git",
    packageVersion: "0.1.0",
    packagePrefix: "git",
    permissions: [],
    sdui: {
      regionId: "git.status",
      displayName: "Git Status",
      adapter: "./dist/status.js"
    }
  };
}

// Advanced/per-load entry point. Validates the package manifest and publishes
// the read-only status tree once. The default user path is
// `loadPackage("@clay/git")`, which invokes this through the first-party
// resolver; this helper remains for explicit/per-load use.
export async function loadGitPackage(clay, options = {}) {
  const packageManifest = options.packageManifest ?? gitPackageManifest();
  await clay.packages?.serverLoadPackage?.(packageManifest);
  return publishGitStatus(clay);
}

// Default activation entry invoked by `loadPackage("@clay/git")`.
export default async function loadGit() {
  return loadGitPackage({ sdui: SDUI, packages: { serverLoadPackage } });
}
