// Clay decoration primitive facade.
//
// Decoration APIs are server-side, load/background-time surfaces for publishing
// inert, bounded decoration ranges. They do not expose renderer callbacks,
// client JavaScript, or raw Deno ops publicly.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.decorations.runtime_unavailable: Clay decoration APIs require the server runtime");
  }
  return ops;
}

function parseResult(json: string): unknown {
  return JSON.parse(json);
}

export type DecorationSpanInput = {
  byteStart: number;
  byteEnd: number;
  kind: "syntax" | "semantic" | "diagnostic" | "search-match";
  styleToken: string;
  priority?: number;
};

export type ServerPublishDecorationsOptions = {
  packageManifest?: unknown;
  packageName?: string;
  packageVersion?: string;
  packagePrefix?: string;
  apiPrefix?: string;
  permissions?: string[];
  documentId: number;
  documentVersion: number;
  currentDocumentVersion?: number;
  behaviorVersion?: number;
  viewport: { byteStart: number; byteEnd: number };
  spans: DecorationSpanInput[];
};

export function serverPublishDecorations(options: ServerPublishDecorationsOptions): unknown {
  return parseResult(requireOps()["op_clay_decorations_publish_decorations"](JSON.stringify(options ?? null)));
}
