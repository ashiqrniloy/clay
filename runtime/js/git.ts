// Clay Git discovery facade.
//
// Read-only APIs backed by server-owned workspace roots and the narrow Git
// discovery/cache service. No shell, network, or mutating Git authority leaks
// through this module.

export type WorkspaceRootId = string;

export type GitHeadState =
  | { kind: "branch"; name: string }
  | { kind: "detached"; shortSha: string }
  | { kind: "unborn" }
  | { kind: "unknown" };

export type GitRefreshStatus =
  | { kind: "success" }
  | { kind: "non-repository" }
  | { kind: "timeout" }
  | { kind: "boundary-rejected" }
  | { kind: "command-error"; command: string; message: string }
  | { kind: "invalid-output"; command: string; message: string };

export type GitRefreshState =
  | { kind: "idle" }
  | { kind: "refreshing"; startedAtMillis: number }
  | { kind: "last-success"; finishedAtMillis: number }
  | { kind: "last-error"; finishedAtMillis: number; status: GitRefreshStatus };

export interface GitStatusSnapshot {
  workspaceRootId: WorkspaceRootId;
  workspaceRoot: string;
  repositoryRoot: string | null;
  head: GitHeadState;
  dirty: boolean;
  changedFileCount: number;
  lastRefresh: GitRefreshStatus;
}

export interface GitCachedStatus {
  workspaceRootId: WorkspaceRootId;
  workspaceRoot: string;
  snapshot: GitStatusSnapshot | null;
  refreshState: GitRefreshState;
}

interface GitOps {
  op_clay_git_list_statuses?: () => Promise<string>;
  op_clay_git_refresh_status?: (requestJson: string) => Promise<string>;
}

declare const globalThis: { Deno?: { core?: { ops?: GitOps } } };

function gitOps(): Required<GitOps> {
  const ops = globalThis.Deno?.core?.ops;
  if (
    typeof ops?.op_clay_git_list_statuses !== "function" ||
    typeof ops?.op_clay_git_refresh_status !== "function"
  ) {
    throw new Error("clay:git runtime ops are unavailable in this environment");
  }
  return ops as Required<GitOps>;
}

export async function serverListGitStatuses(): Promise<GitCachedStatus[]> {
  return JSON.parse(await gitOps().op_clay_git_list_statuses()) as GitCachedStatus[];
}

export async function serverRefreshGitStatus(options: {
  workspaceRootId: WorkspaceRootId;
}): Promise<GitCachedStatus> {
  return JSON.parse(
    await gitOps().op_clay_git_refresh_status(JSON.stringify(options ?? null)),
  ) as GitCachedStatus;
}
