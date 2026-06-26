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
  module?: Record<string, unknown>;
  handler?: never;
  callback?: never;
  onParse?: never;
  function?: never;
  exportName?: string;
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
  for (const key of ["handler", "callback", "onParse", "function"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.parse.invalid_handler: executable ${key} callbacks are not accepted by the public registration contract`);
    }
  }
  const { module, exportName = "default", ...opOptions } = options ?? {};
  const registration = parseResult(requireOps()["op_clay_parse_register_parse_handler"](JSON.stringify({ ...(opOptions ?? {}), runtimeBridge: module !== undefined }))) as { token?: string };
  if (module !== undefined) {
    const handler = module[exportName];
    if (typeof handler !== "function") {
      throw new Error(`clay.parse.invalid_handler: module export ${exportName} must be a function`);
    }
    (globalThis as typeof globalThis & { __clayParseHandlers?: Record<string, unknown> }).__clayParseHandlers ??= Object.create(null);
    (globalThis as typeof globalThis & { __clayParseHandlers: Record<string, unknown> }).__clayParseHandlers[registration.token ?? ""] = handler;
  }
  return registration;
}
