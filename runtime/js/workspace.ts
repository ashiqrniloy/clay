// Clay workspace authority facade.
//
// Workspace APIs expose server-authoritative metadata without granting client
// filesystem access.

export type WorkspaceRootId = string;
export type ClientOpenFolderDialogCommandId = "clay.workspace.clientOpenFolderDialog";

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

interface WorkspaceOps {
  op_clay_workspace_list_roots?: () => Promise<string>;
  op_clay_workspace_add_root?: (path: string) => Promise<string>;
  op_clay_workspace_discover_root_for_path?: (path: string) => Promise<string>;
  op_clay_workspace_list_directory?: (requestJson: string, cancelTokenId?: string) => Promise<string>;
  op_clay_workspace_create_listing_cancel_token?: () => Promise<string>;
  op_clay_workspace_cancel_listing?: (tokenId: string) => Promise<boolean>;
}

declare const globalThis: { Deno?: { core?: { ops?: WorkspaceOps } } };

function workspaceOps(): Required<WorkspaceOps> {
  const ops = globalThis.Deno?.core?.ops;
  if (
    typeof ops?.op_clay_workspace_list_roots !== "function" ||
    typeof ops?.op_clay_workspace_add_root !== "function" ||
    typeof ops?.op_clay_workspace_discover_root_for_path !== "function" ||
    typeof ops?.op_clay_workspace_list_directory !== "function" ||
    typeof ops?.op_clay_workspace_create_listing_cancel_token !== "function" ||
    typeof ops?.op_clay_workspace_cancel_listing !== "function"
  ) {
    throw new Error("clay:workspace runtime ops are unavailable in this environment");
  }
  return ops as Required<WorkspaceOps>;
}

export async function serverListWorkspaceRoots(): Promise<WorkspaceRootMetadata[]> {
  return JSON.parse(await workspaceOps().op_clay_workspace_list_roots()) as WorkspaceRootMetadata[];
}

export async function serverAddWorkspaceRoot(path: string): Promise<WorkspaceRootId> {
  const result = JSON.parse(await workspaceOps().op_clay_workspace_add_root(path)) as {
    workspaceRootId: WorkspaceRootId;
  };
  return result.workspaceRootId;
}

export async function serverDiscoverWorkspaceRootForPath(
  path: string,
): Promise<WorkspaceRootDiscoveryResult> {
  return JSON.parse(
    await workspaceOps().op_clay_workspace_discover_root_for_path(path),
  ) as WorkspaceRootDiscoveryResult;
}

export async function serverListDirectory(options: FileListOptions): Promise<FileListPage> {
  const request = {
    rootId: options.rootId,
    relativePath: options.relativePath ?? "",
    maxDepth: options.maxDepth,
    maxEntries: options.maxEntries,
  };
  return JSON.parse(
    await workspaceOps().op_clay_workspace_list_directory(
      JSON.stringify(request),
      options.cancelTokenId,
    ),
  ) as FileListPage;
}

export async function serverCreateListingCancelToken(): Promise<string> {
  return workspaceOps().op_clay_workspace_create_listing_cancel_token();
}

export async function serverCancelListing(tokenId: string): Promise<boolean> {
  return workspaceOps().op_clay_workspace_cancel_listing(tokenId);
}

export function clientOpenFolderDialog(): ClientOpenFolderDialogCommandId {
  return "clay.workspace.clientOpenFolderDialog";
}
