// Clay workspace authority facade.
//
// Workspace APIs expose server-authoritative metadata without granting client
// filesystem access.
function workspaceOps() {
    const ops = globalThis.Deno?.core?.ops;
    if (typeof ops?.op_clay_workspace_list_roots !== "function" ||
        typeof ops?.op_clay_workspace_add_root !== "function" ||
        typeof ops?.op_clay_workspace_discover_root_for_path !== "function" ||
        typeof ops?.op_clay_workspace_list_directory !== "function" ||
        typeof ops?.op_clay_workspace_create_listing_cancel_token !== "function" ||
        typeof ops?.op_clay_workspace_cancel_listing !== "function") {
        throw new Error("clay:workspace runtime ops are unavailable in this environment");
    }
    return ops;
}
export async function serverListWorkspaceRoots() {
    return JSON.parse(await workspaceOps().op_clay_workspace_list_roots());
}
export async function serverAddWorkspaceRoot(path) {
    const result = JSON.parse(await workspaceOps().op_clay_workspace_add_root(path));
    return result.workspaceRootId;
}
export async function serverDiscoverWorkspaceRootForPath(path) {
    return JSON.parse(await workspaceOps().op_clay_workspace_discover_root_for_path(path));
}
export async function serverListDirectory(options) {
    const request = {
        rootId: options.rootId,
        relativePath: options.relativePath ?? "",
        maxDepth: options.maxDepth,
        maxEntries: options.maxEntries,
    };
    return JSON.parse(await workspaceOps().op_clay_workspace_list_directory(JSON.stringify(request), options.cancelTokenId));
}
export async function serverCreateListingCancelToken() {
    return workspaceOps().op_clay_workspace_create_listing_cancel_token();
}
export async function serverCancelListing(tokenId) {
    return workspaceOps().op_clay_workspace_cancel_listing(tokenId);
}
export function clientOpenFolderDialog() {
    return "clay.workspace.clientOpenFolderDialog";
}
