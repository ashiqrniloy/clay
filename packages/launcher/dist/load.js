// @clay/launcher package load entry (plan 118 Part D).
//
// Claims the window's empty-tab landing: a fresh window and every new empty
// tab open on the launcher. The rows (recent workspaces, configured agent
// types) are host data — the compiled launcher panel requests them through the
// validated session path, so nothing dynamic is hardcoded here and the
// declared tree below stays inert fallback data for the generic SDUI
// renderer.
//
// The launcher grants no authority: it reads display data (names and paths),
// opens folders through the ordinary dialog capability, and never supplies a
// path back to the server. No raw ops, no client JavaScript.
import { serverRegisterPaneContentContribution } from "clay:ui";

export const packageName = "@clay/launcher";
export const apiPrefix = "launcher";

// Mirrors `clay.contributions.ui.paneContents` in `package.json` so the
// manifest, the runtime load entry and the contribution inventory agree.
const LAUNCHER_SURFACE = Object.freeze({
  id: "launcher.start",
  activation: "empty-tab",
  actionTargets: Object.freeze(["workspace.clientOpenFolderDialog"]),
  component: Object.freeze({
    kind: "panel",
    id: "launcher.root",
    title: "Start",
    children: Object.freeze([
      Object.freeze({
        kind: "label",
        id: "launcher.caption",
        text: "Open a folder, an agent, or both.",
        style: Object.freeze({ typography: "typography.caption" })
      }),
      Object.freeze({
        kind: "button",
        id: "launcher.openFolder",
        label: "Open folder…",
        action: Object.freeze({
          commandId: "workspace.clientOpenFolderDialog",
          source: Object.freeze({
            kind: "node",
            nodeId: "launcher.openFolder"
          }),
          arguments: Object.freeze([])
        })
      })
    ])
  })
});

export function launcherPackageContract() {
  return Object.freeze({
    packageName,
    apiPrefix,
    paneContents: Object.freeze([LAUNCHER_SURFACE])
  });
}

export async function loadLauncherPackage(_options = {}) {
  await serverRegisterPaneContentContribution(LAUNCHER_SURFACE);
  return launcherPackageContract();
}

// Default activation entry for `loadPackage("@clay/launcher")`.
export default loadLauncherPackage;
