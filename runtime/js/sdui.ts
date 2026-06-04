// Clay server-driven UI facade.
//
// These helpers run inside Clay's constrained server-side JavaScript runtime and
// delegate SDUI node validation/construction and explicit publication to
// Clay-owned ops. Published trees are validated by the server before they are
// sent through the existing Phase 12 SDUI snapshot/update protocol path.

export type SduiNodeId = string | number;

export interface SduiActionIntent {
  commandId: string;
  arguments?: Record<string, string | number | boolean>;
}

export interface SduiNodeDefinition {
  kind: "panel" | "label" | "button" | "list" | "editorView" | "flex" | "stack";
  id?: SduiNodeId;
  [property: string]: unknown;
}

export interface SduiListItemDefinition {
  id: string;
  label: string;
  detail?: string;
  action?: SduiActionIntent;
}

type ClaySduiOps = {
  op_clay_sdui_define_node?: (kind: string, optionsJson: string) => string;
  op_clay_sdui_publish_tree?: (treeJson: string) => void;
};

declare const Deno: undefined | { core?: { ops?: ClaySduiOps } };

function sduiOps(): Required<ClaySduiOps> {
  const ops = Deno?.core?.ops;
  if (
    typeof ops?.op_clay_sdui_define_node !== "function" ||
    typeof ops?.op_clay_sdui_publish_tree !== "function"
  ) {
    throw new Error("clay.sdui runtime ops are unavailable in this context");
  }
  return ops as Required<ClaySduiOps>;
}

function defineNode(kind: SduiNodeDefinition["kind"], options: Record<string, unknown>): SduiNodeDefinition {
  return JSON.parse(sduiOps().op_clay_sdui_define_node(kind, JSON.stringify(options ?? {}))) as SduiNodeDefinition;
}

export function definePanel(options: {
  id?: SduiNodeId;
  title: string;
  children?: SduiNodeDefinition[];
}): SduiNodeDefinition {
  return defineNode("panel", options);
}

export function defineLabel(options: { id?: SduiNodeId; text: string }): SduiNodeDefinition {
  return defineNode("label", options);
}

export function defineButton(options: {
  id?: SduiNodeId;
  label: string;
  action: SduiActionIntent;
}): SduiNodeDefinition {
  return defineNode("button", options);
}

export function defineList(options: {
  id?: SduiNodeId;
  items: SduiListItemDefinition[];
}): SduiNodeDefinition {
  return defineNode("list", options);
}

export function defineEditorView(options: {
  id?: SduiNodeId;
  documentId: string | number;
  expectedVersion?: number;
}): SduiNodeDefinition {
  return defineNode("editorView", options);
}

export function defineFlex(options: {
  id?: SduiNodeId;
  direction: "row" | "column";
  children?: SduiNodeDefinition[];
}): SduiNodeDefinition {
  return defineNode("flex", options);
}

export function defineStack(options: {
  id?: SduiNodeId;
  children?: SduiNodeDefinition[];
}): SduiNodeDefinition {
  return defineNode("stack", options);
}

export async function publishTree(tree: SduiNodeDefinition): Promise<void> {
  sduiOps().op_clay_sdui_publish_tree(JSON.stringify(tree));
}
