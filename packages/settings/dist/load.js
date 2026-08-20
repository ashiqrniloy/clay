// @clay/settings package load entry (Phase 20.6).
//
// The host applies settings commands from package.json; this entry registers
// the catalog-composed settings overlay. The surface is composed entirely of implemented ComponentKind kinds
// (modal/collapse/dropdown/textInput/label/button/flex) declared in
// `clay.contributions.ui.panels`; no executable render surface runs on the
// hot path. Control actions emit inert `settings.*` command intents
// validated by the server-side settings command executor.
import { serverRegisterPanelContribution } from "clay:ui";

export const packageName = "@clay/settings";
export const apiPrefix = "settings";

const SETTINGS_COMMANDS = Object.freeze([
  { id: "settings.open", displayName: "Open Settings", routingPolicy: "server-first" },
  { id: "settings.close", displayName: "Close Settings", routingPolicy: "server-first" },
  { id: "settings.setTheme", displayName: "Set Theme", routingPolicy: "server-first" },
  { id: "settings.setAppearance", displayName: "Set Appearance", routingPolicy: "server-first" },
  { id: "settings.setTypography", displayName: "Set Typography", routingPolicy: "server-first" },
  { id: "settings.reset", displayName: "Reset Settings", routingPolicy: "server-first" }
]);

// The settings panel declaration mirrors `clay.contributions.ui.panels` in
// `package.json` so the registry, the runtime load entry, and the manifest
// contribution inventory agree.
const SETTINGS_PANEL = Object.freeze({
  id: "settings.surface",
  slot: "right",
  kind: "fixed",
  defaultVisibility: "hidden",
  actionTargets: [
    "settings.close",
    "settings.setTheme",
    "settings.setAppearance",
    "settings.setTypography",
    "settings.reset"
  ],
  component: {
    kind: "panel",
    id: "settings.root",
    title: "Settings",
    children: [{
      kind: "scroll",
      id: "settings.scroll",
      children: [
      {
        kind: "collapse",
        id: "settings.section.theme",
        title: "Theme",
        children: [
          { kind: "label", id: "settings.label.theme", text: "Theme", style: { typography: "typography.title" } },
          {
            kind: "dropdown",
            id: "settings.dropdown.theme",
            title: "Theme",
            items: [
              { id: "@clay/theme-modus-operandi", label: "Modus Operandi", action: { commandId: "settings.setTheme" } },
              { id: "@clay/theme-modus-vivendi", label: "Modus Vivendi", action: { commandId: "settings.setTheme" } },
              { id: "@clay/theme-gruvbox-material-light", label: "Gruvbox Material Light", action: { commandId: "settings.setTheme" } },
              { id: "@clay/theme-gruvbox-material-dark", label: "Gruvbox Material Dark", action: { commandId: "settings.setTheme" } }
            ]
          },
          { kind: "label", id: "settings.label.appearance", text: "Appearance", style: { typography: "typography.title" } },
          {
            kind: "dropdown",
            id: "settings.dropdown.appearance",
            title: "Appearance",
            items: [
              { id: "light", label: "Light", action: { commandId: "settings.setAppearance" } },
              { id: "dark", label: "Dark", action: { commandId: "settings.setAppearance" } },
              { id: "system", label: "System", action: { commandId: "settings.setAppearance" } }
            ]
          }
        ]
      },
      {
        kind: "collapse",
        id: "settings.section.typography",
        title: "Typography",
        children: [
          { kind: "label", id: "settings.label.fonts", text: "Font families (comma-separated, per role)", style: { typography: "typography.title" } },
          { kind: "textInput", id: "settings.input.font.monospace", title: "Monospace families", style: { validationState: "none", placeholderColor: "text.muted" } },
          { kind: "textInput", id: "settings.input.font.proportional", title: "Proportional families", style: { validationState: "none", placeholderColor: "text.muted" } },
          { kind: "textInput", id: "settings.input.font.ui", title: "UI families", style: { validationState: "none", placeholderColor: "text.muted" } },
          { kind: "label", id: "settings.label.sizes", text: "Base sizes and hierarchy ratios", style: { typography: "typography.title" } },
          { kind: "textInput", id: "settings.input.size.monospace", title: "Monospace base size (6–96)", style: { validationState: "none", placeholderColor: "text.muted" } },
          { kind: "textInput", id: "settings.input.size.proportional", title: "Proportional base size (6–96)", style: { validationState: "none", placeholderColor: "text.muted" } },
          { kind: "textInput", id: "settings.input.size.ui", title: "UI base size (6–96)", style: { validationState: "none", placeholderColor: "text.muted" } },
          { kind: "textInput", id: "settings.input.hierarchy", title: "Hierarchy ratios (display, title, section, body, status, detail, caption)", style: { validationState: "none", placeholderColor: "text.muted" } }
        ]
      },
      {
        kind: "flex",
        id: "settings.actions",
        style: { gap: "spacing.sm" },
        children: [
          { kind: "button", id: "settings.button.apply", label: "Apply", action: { commandId: "settings.setTypography" }, style: { variant: "primary" } },
          { kind: "button", id: "settings.button.reset", label: "Reset", action: { commandId: "settings.reset" }, style: { variant: "muted" } },
          { kind: "button", id: "settings.button.close", label: "Close", action: { commandId: "settings.close" }, style: { variant: "default" } }
        ]
      }
      ]
    }]
  }
});

export function settingsPackageContract() {
  return { packageName, apiPrefix, commands: SETTINGS_COMMANDS, overlay: SETTINGS_PANEL };
}

export async function loadSettingsPackage(_options = {}) {
  // Commands come from the host-applied package.json record. The execute-only
  // entry owns the panel contribution, avoiding duplicate command registration.
  await serverRegisterPanelContribution(SETTINGS_PANEL);
  return settingsPackageContract();
}

// Default activation entry for `loadPackage("@clay/settings")`.
export default loadSettingsPackage;