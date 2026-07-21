// Clay decoration primitive facade.
//
// Decoration APIs are server-side, load/background-time surfaces for publishing
// inert, bounded decoration ranges. They do not expose renderer callbacks,
// client JavaScript, or raw Deno ops publicly.
//
// Semantic intelligence (Phase 18.20) publishes DecorationKind::Semantic spans
// with direct closed TokenType + Modifiers vocabulary. Legacy styleToken input
// remains a third-party compatibility escape and is classified into the same
// two-axis model server-side.

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
  /**
   * Closed TokenType variant name (e.g. "Function", "Variable", "Keyword").
   * Preferred for semantic/LSP and first-party two-axis publishers.
   * Provide either `tokenType` or legacy `styleToken`.
   */
  tokenType?: string;
  /**
   * Closed Modifiers names (e.g. ["Declaration", "Readonly", "Bold"]).
   * Only consulted when `tokenType` is set.
   */
  modifiers?: string[];
  /**
   * Legacy free-form style token (e.g. "keyword.control"). Classified into
   * TokenType + Modifiers server-side and retained as the optional scope escape.
   */
  styleToken?: string;
  fontRole?: "monospace" | "proportional";
  priority?: number;
};

export type ServerPublishDecorationsOptions = {
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
