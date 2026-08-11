// Clay command primitive facade skeleton.
//
// Command APIs register package-owned inert command metadata through the server
// runtime. Registration does not grant handler execution authority; clients see
// validated command/routing metadata rather than package JavaScript.
//
// Phase 18.12: command execution facade routes open/reveal actions through the
// server-owned CommandExecution boundary so file-browser UI actions are
// validated against workspace roots and selected-file grants.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("commands.runtime_unavailable: Clay command APIs require the server runtime");
    }
    return ops;
}
function parse(json) {
    return JSON.parse(json);
}
// Package provenance is stamped host-side from the executing-package context;
// facades never accept caller manifests.
export function serverRegisterCommand(declaration) {
    return parse(requireOps().op_clay_commands_register_command(JSON.stringify(declaration ?? null)));
}
export async function serverExecuteCommand(commandId, args, target) {
    const request = {
        commandId,
        arguments: args ?? {},
        target: target ?? { global: {} },
        expectedPermissions: [],
    };
    return parse(await requireOps().op_clay_commands_execute_command(JSON.stringify(request)));
}
export async function serverOpenFile(args) {
    const result = await serverExecuteCommand("workspace.openFile", args);
    if (result.status.kind !== "workspace" || result.status.action !== "opened") {
        throw new Error(`commands.open_failed: expected opened status, got ${JSON.stringify(result.status)}`);
    }
    return {
        documentId: String(result.status.documentId),
        version: Number(result.status.version),
        path: String(result.status.path ?? ""),
    };
}
export async function serverOpenDirectory(args) {
    const result = await serverExecuteCommand("workspace.openDirectory", args);
    if (result.status.kind !== "workspace" || result.status.action !== "navigated") {
        throw new Error(`commands.open_directory_failed: expected navigated status, got ${JSON.stringify(result.status)}`);
    }
    return {
        workspaceRootId: String(result.status.workspaceRootId),
        relativePath: String(result.status.relativePath ?? ""),
    };
}
export async function serverRevealInTree(args) {
    const result = await serverExecuteCommand("workspace.revealInTree", args);
    if (result.status.kind !== "workspace" || result.status.action !== "revealed") {
        throw new Error(`commands.reveal_failed: expected revealed status, got ${JSON.stringify(result.status)}`);
    }
}
export function serverListCommands() {
    return parse(requireOps().op_clay_commands_list_commands());
}
