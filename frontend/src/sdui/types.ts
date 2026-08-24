import type { ThemeSnapshot, TypographySnapshot } from "../theme/types";

export type SduiNodeId = number;

export interface SduiActionIntent {
  commandId: string;
  source:
    | { button: { nodeId: SduiNodeId } }
    | { listItem: { nodeId: SduiNodeId; itemId: string } };
  arguments: Array<{
    name: string;
    value:
      | { string: string }
      | { bool: boolean }
      | { i64: number }
      | { u64: number };
  }>;
}

export interface SduiListItem {
  id: string;
  label: string;
  detail: string | null;
  action: SduiActionIntent | null;
}

export type SduiNodeKind =
  | { panel: { title: string; children: SduiNodeId[] } }
  | { label: { text: string } }
  | { button: { label: string; action: SduiActionIntent } }
  | { list: { items: SduiListItem[] } }
  | {
      editorView: {
        binding: { documentId: number; expectedVersion: number | null };
      };
    }
  | { flex: { direction: "row" | "column"; children: SduiNodeId[] } }
  | { stack: { children: SduiNodeId[] } };

export interface SduiNode {
  id: SduiNodeId;
  kind: SduiNodeKind;
}

export interface SduiTree {
  uiVersion: number;
  rootId: SduiNodeId;
  nodes: SduiNode[];
}

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
    | "textInput";
  title?: string;
  text?: string;
  label?: string;
  direction?: "row" | "column";
  disabled?: boolean;
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

export interface PackageUiProvenance {
  packageName: string;
  packageVersion: string;
  apiPrefix: string;
  trustDomain: "trusted" | "thirdParty";
}

export interface PackageSurface {
  id: string;
  component: PackageComponentNode;
  actionTargets: string[];
  provenance: PackageUiProvenance;
}

export interface PackagePanel extends PackageSurface {
  slot: "left" | "right" | "top" | "bottom";
  visibility: "visible" | "hidden" | "collapsed";
  actionTargets: string[];
}

export interface PackageOverlay extends PackageSurface {
  anchor: "working-area" | "active-pane" | "main" | "pointer";
  focusPolicy: "none" | "restore" | "trap";
  dismissalPolicy: "manual" | "escape" | "outside" | "escape-or-outside";
  actionTargets: string[];
}

export interface PackageInputRoute {
  id: string;
  scope: string;
  componentId: string;
  pointerClick: string;
  pointerAction: string | null;
  pointerDrag: string;
  focusPolicy: string;
  selectionPolicy: string;
  contextModes: string[];
  actionTargets: string[];
  provenance: PackageUiProvenance;
}

export interface PackageUiSnapshot {
  version: number;
  emptyTab: PackageSurface | null;
  panels: PackagePanel[];
  overlays: PackageOverlay[];
  components: PackageSurface[];
  inputRoutes: PackageInputRoute[];
}

export interface RuntimeSnapshot {
  runtimeGenerationId: number;
  behaviorManifest: Record<string, unknown>;
  activeTheme: ThemeSnapshot;
  activeTypography: TypographySnapshot;
  sduiTree: SduiTree;
  packageUi: PackageUiSnapshot;
  documents: Array<Record<string, unknown>>;
  diagnostics: Array<{ severity: string; code: string; message: string }>;
}
