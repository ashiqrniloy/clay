export type WorkspaceRootId = string;
export type GitHeadState = {
    kind: "branch";
    name: string;
} | {
    kind: "detached";
    shortSha: string;
} | {
    kind: "unborn";
} | {
    kind: "unknown";
};
export type GitRefreshStatus = {
    kind: "success";
} | {
    kind: "non-repository";
} | {
    kind: "timeout";
} | {
    kind: "boundary-rejected";
} | {
    kind: "command-error";
    command: string;
    message: string;
} | {
    kind: "invalid-output";
    command: string;
    message: string;
};
export type GitRefreshState = {
    kind: "idle";
} | {
    kind: "refreshing";
    startedAtMillis: number;
} | {
    kind: "last-success";
    finishedAtMillis: number;
} | {
    kind: "last-error";
    finishedAtMillis: number;
    status: GitRefreshStatus;
};
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
export declare function serverListGitStatuses(): Promise<GitCachedStatus[]>;
export declare function serverRefreshGitStatus(options: {
    workspaceRootId: WorkspaceRootId;
}): Promise<GitCachedStatus>;
