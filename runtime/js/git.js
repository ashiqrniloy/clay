// Clay Git discovery facade.
//
// Read-only APIs backed by server-owned workspace roots and the narrow Git
// discovery/cache service. No shell, network, or mutating Git authority leaks
// through this module.
function gitOps() {
    const ops = globalThis.Deno?.core?.ops;
    if (typeof ops?.op_clay_git_list_statuses !== "function" ||
        typeof ops?.op_clay_git_refresh_status !== "function") {
        throw new Error("clay:git runtime ops are unavailable in this environment");
    }
    return ops;
}
export async function serverListGitStatuses() {
    return JSON.parse(await gitOps().op_clay_git_list_statuses());
}
export async function serverRefreshGitStatus(options) {
    return JSON.parse(await gitOps().op_clay_git_refresh_status(JSON.stringify(options ?? null)));
}
