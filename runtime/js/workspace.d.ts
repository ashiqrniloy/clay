export type WorkspaceRootId = string;
export type ClientOpenFolderDialogCommandId = "workspace.clientOpenFolderDialog";
export interface WorkspaceRootMetadata {
    workspaceRootId: WorkspaceRootId;
    displayName: string;
    displayPath: string;
}
export interface WorkspaceRootDiscoveryResult {
    workspaceRootId: WorkspaceRootId | null;
    discovered: boolean;
}
export type FileListEntryKind = "directory" | "file" | "symlink" | "other";
export interface FileListEntryDiagnostic {
    code: string;
    message: string;
}
export interface FileListEntry {
    name: string;
    kind: FileListEntryKind;
    relativePath: string;
    sizeHint: number | null;
    childCount: number | null;
    diagnostic: FileListEntryDiagnostic | null;
}
export interface FileListPage {
    rootId: WorkspaceRootId;
    entries: FileListEntry[];
    truncated: boolean;
    cancelled: boolean;
    diagnostics: Array<{
        code: string;
        message: string;
        hint: string | null;
    }>;
}
export interface FileListOptions {
    rootId: WorkspaceRootId;
    relativePath?: string;
    maxDepth?: number;
    maxEntries?: number;
    cancelTokenId?: string;
}
export declare function serverListWorkspaceRoots(): Promise<WorkspaceRootMetadata[]>;
export declare function serverAddWorkspaceRoot(path: string): Promise<WorkspaceRootId>;
export declare function serverDiscoverWorkspaceRootForPath(path: string): Promise<WorkspaceRootDiscoveryResult>;
export declare function serverListDirectory(options: FileListOptions): Promise<FileListPage>;
export declare function serverCreateListingCancelToken(): Promise<string>;
export declare function serverCancelListing(tokenId: string): Promise<boolean>;
export declare function clientOpenFolderDialog(): ClientOpenFolderDialogCommandId;
