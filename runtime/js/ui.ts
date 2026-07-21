// Clay package UI contribution facade.
//
// These helpers run inside Clay's constrained server-side JavaScript runtime and
// delegate inert slot-aware UI contribution validation to Clay-owned ops.  The
// public API accepts declarative contribution data only; package provenance is
// stamped host-side from the executing-package context. Raw op names, Masonry
// widgets, native handles, CSS strings, renderer callbacks, and client-side
// JavaScript hooks are not package-facing authorities.

export type PanelSlot = "left" | "right" | "top" | "bottom";
export type PanelVisibility = "visible" | "hidden" | "collapsed";
export type OverlayAnchor = "working-area" | "active-pane" | "main" | "pointer";
export type OverlayFocusPolicy = "none" | "restore" | "trap";
export type OverlayDismissalPolicy = "manual" | "escape" | "outside" | "escape-or-outside";
export type InputScope = "component" | "panel" | "overlay";
export type PointerClickPolicy = "none" | "focus" | "action" | "select";
export type PointerDragPolicy = "none" | "select" | "pan";
export type ComponentFocusPolicy = "none" | "restore-editor" | "focus-component" | "trap";
export type SelectionPolicy = "preserve-editor" | "component-local" | "disabled";
export type UiStateScope = "package-global" | "user-config" | "workspace" | "document" | "pane" | "component" | "transient-overlay";
export type UiStateOwner = "package" | "shell" | "server";
export type UiStateLifetime = "session" | "workspace" | "document" | "transient";
export type UiStatePersistence = "none" | "client-local" | "server-canonical" | "deferred";
export type UiStateImplementationStatus = "implemented" | "deferred";
export type UiStateSchemaKind = "boolean" | "number" | "string" | "enum" | "object";
export type ThemeTokenType = "color-role" | "spacing" | "radius" | "typography" | "opacity";
export type ComponentFontRole = "ui" | "monospace" | "proportional";
export type LayoutOverrideProperty = "slot" | "visibility" | "splitRatio" | "themeToken" | "inputDefault" | "actionDefault" | "fallback";
export type LayoutOverrideSource = "user-config" | "active-major-mode" | "compatible-minor-mode" | "global-package" | "package-default";

export interface UiActionIntent {
  commandId: string;
  arguments?: Record<string, string | number | boolean>;
}

export interface ComponentStyleVariables {
  background?: string;
  contentColor?: string;
  borderColor?: string;
  accentColor?: string;
  padding?: string;
  gap?: string;
  rowHeight?: string;
  inset?: string;
  radius?: string;
  typography?: string;
  fontRole?: ComponentFontRole;
  opacity?: string;
  variant?: "default" | "muted" | "primary" | "danger";
}

export interface ComponentContributionDefinition {
  kind: "editorView" | "panel" | "label" | "button" | "list" | "flex" | "stack" | "overlay" | "scroll" | "portal" | "statusItem";
  id: string;
  style?: ComponentStyleVariables;
  children?: ComponentContributionDefinition[];
  action?: UiActionIntent;
  [property: string]: unknown;
}

export interface PanelContributionDefinition {
  id: string;
  slot: PanelSlot;
  kind?: "fixed";
  defaultVisibility?: PanelVisibility;
  component: ComponentContributionDefinition;
  actionTargets?: string[];
}

export interface TransientOverlayContributionDefinition {
  id: string;
  anchor?: OverlayAnchor;
  focusPolicy?: OverlayFocusPolicy;
  dismissalPolicy?: OverlayDismissalPolicy;
  component: ComponentContributionDefinition;
  actionTargets?: string[];
}

export interface PackageInputContributionDefinition {
  id: string;
  scope: InputScope;
  componentId: string;
  pointer?: {
    click?: PointerClickPolicy;
    action?: string;
    drag?: PointerDragPolicy;
  };
  focus?: {
    policy?: ComponentFocusPolicy;
    trap?: boolean;
  };
  selectionPolicy?: SelectionPolicy;
  context?: {
    modes?: string[];
  };
  actionTargets?: string[];
}

export interface PackageUiStateScopeDefinition {
  id: string;
  scope: UiStateScope;
  owner: UiStateOwner;
  lifetime: UiStateLifetime;
  persistence: UiStatePersistence;
  implementationStatus?: UiStateImplementationStatus;
  targetId?: string;
  valueSchema: {
    kind: UiStateSchemaKind;
    values?: string[];
    properties?: Record<string, { kind: UiStateSchemaKind }>;
  };
}

export interface PackageThemeTokenDeclaration {
  token: string;
  type: ThemeTokenType;
  fallback: string;
  description: string;
}

export interface PackageLayoutOverrideDefinition {
  targetId: string;
  property: LayoutOverrideProperty;
  value: unknown;
  source?: LayoutOverrideSource;
}

type ClayUiOps = {
  op_clay_ui_register_panel_contribution?: (manifestJson: string, declarationJson: string) => string;
  op_clay_ui_register_component_contribution?: (manifestJson: string, declarationJson: string) => string;
  op_clay_ui_register_transient_overlay_contribution?: (manifestJson: string, declarationJson: string) => string;
  op_clay_ui_register_theme_token?: (manifestJson: string, declarationJson: string) => string;
  op_clay_ui_register_input_contribution?: (manifestJson: string, declarationJson: string) => string;
  op_clay_ui_register_ui_state_scope?: (manifestJson: string, declarationJson: string) => string;
  op_clay_ui_set_layout_override?: (declarationJson: string) => string;
};

declare const Deno: undefined | { core?: { ops?: ClayUiOps } };

function uiOps(): Required<ClayUiOps> {
  const ops = Deno?.core?.ops;
  if (
    typeof ops?.op_clay_ui_register_panel_contribution !== "function" ||
    typeof ops?.op_clay_ui_register_component_contribution !== "function" ||
    typeof ops?.op_clay_ui_register_transient_overlay_contribution !== "function" ||
    typeof ops?.op_clay_ui_register_theme_token !== "function" ||
    typeof ops?.op_clay_ui_register_input_contribution !== "function" ||
    typeof ops?.op_clay_ui_register_ui_state_scope !== "function" ||
    typeof ops?.op_clay_ui_set_layout_override !== "function"
  ) {
    throw new Error("clay.ui runtime ops are unavailable in this context");
  }
  return ops as Required<ClayUiOps>;
}

function encode(value: unknown): string {
  return JSON.stringify(value ?? null);
}

export function serverRegisterPanelContribution(
  declaration: PanelContributionDefinition,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_register_panel_contribution(encode(declaration))) as Record<string, unknown>;
}

export function serverRegisterComponentContribution(
  declaration: ComponentContributionDefinition,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_register_component_contribution(encode(declaration))) as Record<string, unknown>;
}

export function serverRegisterTransientOverlayContribution(
  declaration: TransientOverlayContributionDefinition,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_register_transient_overlay_contribution(encode(declaration))) as Record<string, unknown>;
}

export function serverRegisterInputContribution(
  declaration: PackageInputContributionDefinition,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_register_input_contribution(encode(declaration))) as Record<string, unknown>;
}

export function serverRegisterUiStateScope(
  declaration: PackageUiStateScopeDefinition,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_register_ui_state_scope(encode(declaration))) as Record<string, unknown>;
}

export function serverSetLayoutOverride(
  declaration: PackageLayoutOverrideDefinition,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_set_layout_override(encode(declaration))) as Record<string, unknown>;
}

export function serverRegisterThemeToken(
  declaration: PackageThemeTokenDeclaration,
): Record<string, unknown> {
  return JSON.parse(uiOps().op_clay_ui_register_theme_token(encode(declaration))) as Record<string, unknown>;
}
