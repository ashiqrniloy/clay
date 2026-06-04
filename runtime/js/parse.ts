// Clay parse primitive facade.
//
// Parse APIs are server-side registration-time surfaces for package parse
// handlers. Parser execution stays on the server background lane and never
// grants filesystem, network, shell, AI, raw-op, or client-JavaScript authority.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.parse.runtime_unavailable: Clay parse APIs require the server runtime");
  }
  return ops;
}

function parseResult(json: string): unknown {
  return JSON.parse(json);
}

export type ServerRegisterParseHandlerOptions = {
  packageManifest?: unknown;
  packageName?: string;
  packageVersion?: string;
  packagePrefix?: string;
  apiPrefix?: string;
  permissions?: string[];
  mode: string;
  parseUnit?: "file" | "region" | "line-group";
  viewportPriority?: boolean;
  timeoutMs?: number;
  maxWindowBytes?: number;
  parseWindowBytes?: number;
  guardBytes?: number;
  memoryBudgetBytes?: number;
};

export function serverRegisterParseHandler(options: ServerRegisterParseHandlerOptions): unknown {
  return parseResult(requireOps()["op_clay_parse_register_parse_handler"](JSON.stringify(options ?? null)));
}
