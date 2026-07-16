// Clay language-intelligence provider primitive facade.
//
// Registration is configuration/package-load time only. Short providers receive
// bounded windows; approved document analyzers receive bounded canonical events
// through package-owned module specifiers. No callback arguments, raw ops,
// client JS, filesystem, network, shell, or implicit process authority cross
// this facade. Process use separately requires `clay:language-server` grant.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.language.runtime_unavailable: Clay language APIs require the server runtime");
  }
  return ops;
}

function parseResult(json: string): unknown {
  return JSON.parse(json);
}

export type LanguageIntelligenceFeature =
  | "hover"
  | "definition"
  | "goToDefinition"
  | "codeAction"
  | "signatureHelp";

export type LanguageIntelligenceProviderDeclaration = {
  id: string;
  modes?: string[];
  features: LanguageIntelligenceFeature[];
  priority?: number;
  module?: string;
  exportName?: string;
  timeoutMs?: number;
  budgets?: { timeoutMs?: number };
  handler?: never;
  callback?: never;
  function?: never;
  clientJavaScript?: never;
  nativeHandle?: never;
  rawOps?: never;
  executable?: never;
  process?: never;
  languageServer?: never;
};

export type ServerRegisterLanguageIntelligenceProviderOptions = {
  packageManifest?: unknown;
  packageName?: string;
  packageVersion?: string;
  packagePrefix?: string;
  apiPrefix?: string;
  permissions?: string[];
  provider?: LanguageIntelligenceProviderDeclaration;
  id?: string;
  modes?: string[];
  features?: LanguageIntelligenceFeature[];
  priority?: number;
  exportName?: string;
  timeoutMs?: number;
  module?: Record<string, unknown>;
  handler?: never;
  callback?: never;
  function?: never;
  clientJavaScript?: never;
  nativeHandle?: never;
  rawOps?: never;
  executable?: never;
  process?: never;
  languageServer?: never;
};

export type ServerRegisterDocumentAnalyzerOptions = {
  packageManifest: unknown;
  analyzer: {
    id: string;
    contribution: string;
    modes?: string[];
    moduleSpecifier: string;
    exportName?: string;
  };
};

export function serverRegisterDocumentAnalyzer(
  options: ServerRegisterDocumentAnalyzerOptions,
): unknown {
  return parseResult(
    requireOps()["op_clay_language_register_document_analyzer"](
      JSON.stringify(options ?? null),
    ),
  );
}

export function serverRegisterLanguageIntelligenceProvider(
  options: ServerRegisterLanguageIntelligenceProviderOptions,
): unknown {
  for (const key of [
    "handler",
    "callback",
    "function",
    "clientJavaScript",
    "nativeHandle",
    "rawOps",
    "executable",
    "process",
    "languageServer",
  ]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(
        `clay.language.invalid_provider: executable or process authority field ${key} is not accepted by the public registration contract`,
      );
    }
  }
  const { module, exportName = "provideLanguageIntelligence", ...opOptions } = options ?? {};
  const registration = parseResult(
    requireOps()["op_clay_language_register_intelligence_provider"](
      JSON.stringify({
        ...(opOptions ?? {}),
        exportName,
        runtimeBridge: module !== undefined,
      }),
    ),
  ) as { token?: string };
  if (module !== undefined) {
    const handler = module[exportName];
    if (typeof handler !== "function") {
      throw new Error(
        `clay.language.invalid_provider: module export ${exportName} must be a function`,
      );
    }
    (globalThis as typeof globalThis & {
      __clayLanguageIntelligenceHandlers?: Record<string, unknown>;
    }).__clayLanguageIntelligenceHandlers ??= Object.create(null);
    (globalThis as typeof globalThis & {
      __clayLanguageIntelligenceHandlers: Record<string, unknown>;
    }).__clayLanguageIntelligenceHandlers[registration.token ?? ""] = handler;
  }
  return registration;
}
