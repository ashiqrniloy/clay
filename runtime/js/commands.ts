// Clay command primitive facade skeleton.
//
// Command APIs register package-owned inert command metadata through the server
// runtime. Registration does not grant handler execution authority; clients see
// validated command/routing metadata rather than package JavaScript.

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

export function serverListCommands(): unknown[] {
  return parse(requireOps().op_clay_commands_list_commands());
}
