// Clay workspace authority facade.
//
// Workspace APIs expose server-authoritative metadata without granting client
// filesystem access.

export type WorkspaceRootId = string;

export interface WorkspaceRootMetadata {
  workspaceRootId: WorkspaceRootId;
  displayName: string;
  displayPath: string;
}

interface WorkspaceOps {
  op_clay_workspace_list_roots?: () => Promise<string>;
}

declare const globalThis: { Deno?: { core?: { ops?: WorkspaceOps } } };

function workspaceOps(): Required<WorkspaceOps> {
  const ops = globalThis.Deno?.core?.ops;
  if (typeof ops?.op_clay_workspace_list_roots !== "function") {
    throw new Error("clay:workspace runtime ops are unavailable in this environment");
  }
  return ops as Required<WorkspaceOps>;
}

export async function serverListWorkspaceRoots(): Promise<WorkspaceRootMetadata[]> {
  return JSON.parse(await workspaceOps().op_clay_workspace_list_roots()) as WorkspaceRootMetadata[];
}
