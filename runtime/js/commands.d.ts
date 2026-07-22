export declare function serverRegisterCommand(declaration: unknown): unknown;
export interface CommandExecutionResult {
    commandId: string;
    routingPolicy: string;
    target: unknown;
    status: {
        kind: string;
        [key: string]: unknown;
    };
}
export interface DocumentHandle {
    documentId: string;
    version: number;
    path: string;
}
export declare function serverExecuteCommand(commandId: string, args?: Record<string, unknown>, target?: {
    activeDocument?: {
        documentId: string;
    };
} | {
    workspace: unknown;
} | {
    global: unknown;
}): Promise<CommandExecutionResult>;
export declare function serverOpenFile(args: {
    workspaceRootId?: string;
    relativePath?: string;
    absolutePath?: string;
}): Promise<DocumentHandle>;
export declare function serverOpenDirectory(args: {
    workspaceRootId: string;
    relativePath?: string;
}): Promise<{
    workspaceRootId: string;
    relativePath: string;
}>;
export declare function serverRevealInTree(args: {
    documentId: string;
}): Promise<void>;
export declare function serverListCommands(): unknown[];
