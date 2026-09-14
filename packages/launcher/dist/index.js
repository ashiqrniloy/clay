// @clay/launcher — first-party start surface (plan 118 Part D).
//
// Re-exports the load entry so `loadPackage("@clay/launcher")` invokes
// `loadLauncherPackage`. The empty-tab landing election, the recents store and
// the agent enumeration are server-owned; this package only claims the pane
// content and its inert fallback tree.
export { launcherPackageContract, loadLauncherPackage } from "./load.js";
