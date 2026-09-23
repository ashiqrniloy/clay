// SDUI wire shapes are generated from the Rust DTO layer
// (`src-tauri/src/bridge/dto.rs`) into `../bridge/generated/bridge.ts`
// (plan 119 SC-1) — never restate them.
//
// What stays hand-written in this module:
//  * `PackageComponentNode` and friends: the *authoring* component JSON inside
//    a package surface, validated at render time (the wire carries it as a
//    generated `JsonValue`).
//  * `SduiTreeUpdate`/`SduiTreeOperation`: event-only payloads of the shell's
//    narrowing of `ClientConnectionEvent`, which generation excludes.
export type {
  PackageInputRouteContent as PackageInputRoute,
  PackageUiProvenance,
  RuntimeSnapshotDto as RuntimeSnapshot,
  SduiActionArgument,
  SduiActionIntent,
  SduiActionSource,
  SduiActionValue,
  SduiEditorBinding,
  SduiFlexDirection,
  SduiListFilter,
  SduiListItem,
  SduiNode,
  SduiNodeId,
  SduiNodeKind,
  SduiTree,
  UiChoiceOption,
  UiChoicesSnapshot,
} from "../bridge/types";

import type {
  PackageInputRouteContent as PackageInputRoute,
  SduiNode,
  SduiNodeId,
  PackageOverlayDto,
  PackagePanelDto,
  PackageSurfaceDto,
  PackageUiSnapshotDto,
} from "../bridge/types";

/** Event-only SDUI tree update (arrives inside `ClientConnectionEvent`). */
export type SduiTreeOperation =
  | { replaceRoot: { rootId: SduiNodeId } }
  | { replaceNode: { node: SduiNode } }
  | { removeNode: { nodeId: SduiNodeId } };

export interface SduiTreeUpdate {
  baseUiVersion: number;
  newUiVersion: number;
  operations: SduiTreeOperation[];
}

export type FontRole = "ui" | "monospace" | "proportional";
export type TextVariant =
  "display" | "title" | "section" | "body" | "status" | "detail" | "caption";

export interface PackageAction {
  commandId: string;
  arguments?: Record<string, string | number | boolean>;
}

export interface PackageListItem {
  id: string;
  label: string;
  detail?: string;
  /** Semantic icon reference resolved against the active icon pack. */
  icon?: string;
  action?: PackageAction;
  selected?: boolean;
  disabled?: boolean;
}

export interface PackageComponentNode {
  id: string;
  kind:
    | "editorView"
    | "panel"
    | "label"
    | "button"
    | "list"
    | "flex"
    | "stack"
    | "overlay"
    | "scroll"
    | "portal"
    | "statusItem"
    | "dropdown"
    | "collapse"
    | "modal"
    | "textInput"
    | "tabList";
  title?: string;
  text?: string;
  label?: string;
  /** Semantic icon reference (button/label/list/statusItem kinds only). */
  icon?: string;
  direction?: "row" | "column";
  disabled?: boolean;
  /** `textInput` only: multiline growing composer variant (plan 108 G3). */
  multiline?: boolean;
  action?: PackageAction;
  items?: PackageListItem[];
  children?: PackageComponentNode[];
  style?: {
    background?: string;
    contentColor?: string;
    borderColor?: string;
    accentColor?: string;
    padding?: string;
    gap?: string;
    rowHeight?: string;
    inset?: string;
    radius?: string;
    typography?: `typography.${TextVariant}`;
    opacity?: string;
    fontRole?: FontRole;
    variant?: "default" | "muted" | "primary" | "danger";
    placeholderColor?: string;
    validationState?: "none" | "error" | "warning" | "success";
  };
}

/**
 * A resolved package surface: the generated DTO with its `component` JSON
 * narrowed to the parsed authoring node the SDUI renderer consumes.
 */
export type PackageSurface = Omit<PackageSurfaceDto, "component"> & {
  component: PackageComponentNode;
};

export type PackagePanel = Omit<
  PackagePanelDto,
  "component" | "slot" | "visibility"
> & {
  component: PackageComponentNode;
  slot: "left" | "right" | "top" | "bottom";
  visibility: "visible" | "hidden" | "collapsed";
};

export type PackageOverlay = Omit<
  PackageOverlayDto,
  "component" | "anchor" | "focusPolicy" | "dismissalPolicy"
> & {
  component: PackageComponentNode;
  anchor: "working-area" | "active-pane" | "main" | "pointer";
  focusPolicy: "none" | "restore" | "trap";
  dismissalPolicy: "manual" | "escape" | "outside" | "escape-or-outside";
};

export type PackageUiSnapshot = Omit<
  PackageUiSnapshotDto,
  "emptyTab" | "surfaces" | "panels" | "overlays" | "components" | "inputRoutes"
> & {
  emptyTab: PackageSurface | null;
  /** Named pane surfaces (`activation: "pane"`), e.g. the Coding Agent split. */
  surfaces: PackageSurface[];
  panels: PackagePanel[];
  overlays: PackageOverlay[];
  components: PackageSurface[];
  inputRoutes: PackageInputRoute[];
};
