// @clay/settings — first-party settings UI surface (Phase 20.6).
//
// Inert catalog-composed SDUI surface for theme, appearance, and typography
// configuration. Re-exports the load entry so `loadPackage("@clay/settings")`
// invokes `loadSettingsPackage` through the first-party resolver.
export { settingsPackageContract, loadSettingsPackage } from "./load.js";