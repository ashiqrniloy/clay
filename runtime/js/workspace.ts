// Clay workspace authority facade skeleton.
//
// Workspace APIs are planned server-authoritative metadata APIs. They expose
// configured workspace root metadata without granting client filesystem access.

export type WorkspaceRootId = string;

export interface WorkspaceRootMetadata {
  workspaceRootId: WorkspaceRootId;
  displayName: string;
  displayPath: string;
}

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export async function serverListWorkspaceRoots(): Promise<WorkspaceRootMetadata[]> {
  plannedApi("clay.workspace.serverListWorkspaceRoots");
}
