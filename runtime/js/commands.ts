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

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.commands.runtime_unavailable: Clay command APIs require the server runtime");
  }
  return ops;
}

function parse<T>(json: string): T {
  return JSON.parse(json) as T;
}

export function serverRegisterCommand(packageManifest: unknown, declaration: unknown): unknown {
  return parse(requireOps().op_clay_commands_register_command(JSON.stringify(packageManifest ?? null), JSON.stringify(declaration ?? null)));
}

export interface CommandExecutionResult {
  commandId: string;
  routingPolicy: string;
  target: unknown;
  status: { kind: string; [key: string]: unknown };
}

export interface DocumentHandle {
  documentId: string;
  version: number;
  path: string;
}

export async function serverExecuteCommand(
  commandId: string,
  args?: Record<string, unknown>,
  target?: { activeDocument?: { documentId: string } } | { workspace: unknown } | { global: unknown },
): Promise<CommandExecutionResult> {
  const request = {
    commandId,
    arguments: args ?? {},
    target: target ?? { global: {} },
    expectedPermissions: [],
  };
  return parse(await requireOps().op_clay_commands_execute_command(JSON.stringify(request)));
}

export async function serverOpenFile(args: {
  workspaceRootId?: string;
  relativePath?: string;
  absolutePath?: string;
}): Promise<DocumentHandle> {
  const result = await serverExecuteCommand("clay.workspace.openFile", args as Record<string, unknown>);
  if (result.status.kind !== "workspace" || result.status.action !== "opened") {
    throw new Error(`clay.commands.open_failed: expected opened status, got ${JSON.stringify(result.status)}`);
  }
  return {
    documentId: String(result.status.documentId),
    version: Number(result.status.version),
    path: String(result.status.path ?? ""),
  };
}

export async function serverRevealInTree(args: { documentId: string }): Promise<void> {
  const result = await serverExecuteCommand("clay.workspace.revealInTree", args as Record<string, unknown>);
  if (result.status.kind !== "workspace" || result.status.action !== "revealed") {
    throw new Error(`clay.commands.reveal_failed: expected revealed status, got ${JSON.stringify(result.status)}`);
  }
}

export function serverListCommands(): unknown[] {
  return parse(requireOps().op_clay_commands_list_commands());
}
