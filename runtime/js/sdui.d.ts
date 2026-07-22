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
export declare function definePanel(options: {
    id?: SduiNodeId;
    title: string;
    children?: SduiNodeDefinition[];
}): SduiNodeDefinition;
export declare function defineLabel(options: {
    id?: SduiNodeId;
    text: string;
}): SduiNodeDefinition;
export declare function defineButton(options: {
    id?: SduiNodeId;
    label: string;
    action: SduiActionIntent;
}): SduiNodeDefinition;
export declare function defineList(options: {
    id?: SduiNodeId;
    items: SduiListItemDefinition[];
}): SduiNodeDefinition;
export declare function defineEditorView(options: {
    id?: SduiNodeId;
    documentId: string | number;
    expectedVersion?: number;
}): SduiNodeDefinition;
export declare function defineFlex(options: {
    id?: SduiNodeId;
    direction: "row" | "column";
    children?: SduiNodeDefinition[];
}): SduiNodeDefinition;
export declare function defineStack(options: {
    id?: SduiNodeId;
    children?: SduiNodeDefinition[];
}): SduiNodeDefinition;
export declare function publishTree(tree: SduiNodeDefinition): Promise<void>;
