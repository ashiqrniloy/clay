// Clay syntax grammar primitive facade.
//
// Syntax APIs register inert, first-party grammar metadata for server-side
// background highlighting. They do not expose raw Deno ops, executable parser
// callbacks, native grammar handles, arbitrary artifact loading, or client JS.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.syntax.runtime_unavailable: Clay syntax APIs require the server runtime");
  }
  return ops;
}

function parseResult(json: string): unknown {
  return JSON.parse(json);
}

export type ServerRegisterSyntaxGrammarOptions = {
  packageManifest?: unknown;
  packageName?: string;
  packageVersion?: string;
  packagePrefix?: string;
  apiPrefix?: string;
  permissions?: string[];
  syntaxGrammar?: unknown;
  contribution?: unknown;
  languageId?: string;
  filePatterns?: { extensions?: string[]; fileNames?: string[] };
  grammar?: { kind: "tree-sitter-wasm"; path: string; source?: string };
  queries?: { highlights: string; locals?: string; injections?: string };
  styleMap?: Record<string, string>;
  budgets?: { timeoutMs?: number; maxWindowBytes?: number };
  handler?: never;
  callback?: never;
  onParse?: never;
  function?: never;
  clientJavaScript?: never;
  nativeHandle?: never;
  rawOps?: never;
};

export function serverRegisterSyntaxGrammar(options: ServerRegisterSyntaxGrammarOptions): unknown {
  for (const key of ["handler", "callback", "onParse", "function", "clientJavaScript", "nativeHandle", "rawOps"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.syntax.invalid_grammar: executable or raw authority field ${key} is not accepted by the public registration contract`);
    }
  }
  return parseResult(requireOps()["op_clay_syntax_register_syntax_grammar"](JSON.stringify(options ?? null)));
}
