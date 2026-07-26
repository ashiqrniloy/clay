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
export type ThemeTokenType =
    | "color-role"
    | "spacing"
    | "radius"
    | "typography"
    | "opacity"
    | "dimension"
    | "elevation"
    | "motion-duration"
    | "z-level"
    | "density";
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
        properties?: Record<string, {
            kind: UiStateSchemaKind;
        }>;
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
export interface PackageLayoutIntentDefinition {
    id: string;
    targetPane: string;
    orientation: "horizontal" | "vertical";
    ratio: number;
    position?: "first" | "second";
}
export declare function serverRegisterPanelContribution(declaration: PanelContributionDefinition): Record<string, unknown>;
export declare function serverRegisterComponentContribution(declaration: ComponentContributionDefinition): Record<string, unknown>;
export declare function serverRegisterTransientOverlayContribution(declaration: TransientOverlayContributionDefinition): Record<string, unknown>;
export declare function serverRegisterInputContribution(declaration: PackageInputContributionDefinition): Record<string, unknown>;
export declare function serverRegisterUiStateScope(declaration: PackageUiStateScopeDefinition): Record<string, unknown>;
export declare function serverSetLayoutOverride(declaration: PackageLayoutOverrideDefinition): Record<string, unknown>;
export declare function serverRequestLayoutIntent(declaration: PackageLayoutIntentDefinition): Record<string, unknown>;
export declare function serverRegisterThemeToken(declaration: PackageThemeTokenDeclaration): Record<string, unknown>;
