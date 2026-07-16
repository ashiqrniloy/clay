// Clay completion provider primitive facade.
//
// Completion APIs register inert metadata and optional imported module exports
// at load/reload time. Provider execution stays server-side, cancellable, and
// bounded; no raw ops, callback arguments, client JS, filesystem, network,
// shell, AI, WASM, native-widget, or package-manager authority is exposed.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.completion.runtime_unavailable: Clay completion APIs require the server runtime");
  }
  return ops;
}

function parseResult(json: string): unknown {
  return JSON.parse(json);
}

export type CompletionProviderItem =
  | string
  | {
      label: string;
      insertText: string;
      detail?: string;
      textFormat?: "plainText" | "snippet";
    };

export type ServerRegisterCompletionProviderOptions = {
  packageManifest?: unknown;
  packageName?: string;
  packageVersion?: string;
  packagePrefix?: string;
  apiPrefix?: string;
  permissions?: string[];
  completionProvider?: unknown;
  contribution?: unknown;
  providerId?: string;
  triggerCharacters?: string[];
  triggers?: { characters?: string[]; wordBoundary?: boolean };
  wordBoundaryChars?: string[];
  items?: CompletionProviderItem[];
  priority?: number;
  exclusive?: boolean;
  timeoutMs?: number;
  maxItems?: number;
  handler?: never;
  callback?: never;
  complete?: never;
  function?: never;
  clientJavaScript?: never;
  nativeHandle?: never;
  rawOps?: never;
  module?: Record<string, unknown>;
  exportName?: string;
};

export function serverRegisterCompletionProvider(options: ServerRegisterCompletionProviderOptions): unknown {
  for (const key of ["handler", "callback", "complete", "function", "clientJavaScript", "nativeHandle", "rawOps"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.completion.invalid_provider: executable or raw authority field ${key} is not accepted by the public registration contract`);
    }
  }
  const { module, exportName = "provideCompletion", ...opOptions } = options ?? {};
  const registration = parseResult(
    requireOps()["op_clay_completion_register_completion_provider"](
      JSON.stringify({ ...opOptions, exportName, runtimeBridge: module !== undefined }),
    ),
  ) as { tokens?: string[] };
  if (module !== undefined) {
    const handler = module[exportName];
    if (typeof handler !== "function") {
      throw new Error(`clay.completion.invalid_provider: module export ${exportName} must be a function`);
    }
    (globalThis as typeof globalThis & { __clayCompletionHandlers?: Record<string, unknown> })
      .__clayCompletionHandlers ??= Object.create(null);
    for (const token of registration.tokens ?? []) {
      (globalThis as typeof globalThis & { __clayCompletionHandlers: Record<string, unknown> })
        .__clayCompletionHandlers[token] = handler;
    }
  }
  return registration;
}

export type ServerDisableCompletionOptions =
  | { provider: string; packagePrefix?: never }
  | { provider?: never; packagePrefix: string };

export function serverDisableCompletion(options: ServerDisableCompletionOptions): unknown {
  for (const key of Object.keys(options ?? {})) {
    if (key !== "provider" && key !== "packagePrefix") {
      throw new Error("clay.completion.invalid_disable: only provider or packagePrefix is accepted");
    }
  }
  const provider = (options ?? {}).provider;
  const packagePrefix = (options ?? {}).packagePrefix;
  const targets = [provider, packagePrefix].filter((value) => typeof value === "string" && value.trim().length > 0);
  if (targets.length !== 1) {
    throw new Error("clay.completion.invalid_disable: provide exactly one non-empty provider or packagePrefix");
  }
  return parseResult(requireOps()["op_clay_completion_disable"](JSON.stringify(options)));
}

export type ServerListCompletionProvidersForTriggerOptions = {
  trigger: string;
};

export function serverListCompletionProvidersForTrigger(options: ServerListCompletionProvidersForTriggerOptions): unknown {
  const trigger = (options ?? {}).trigger;
  if (typeof trigger !== "string" || trigger.length === 0) {
    throw new Error("clay.completion.invalid_trigger: trigger must be a non-empty string");
  }
  return parseResult(requireOps()["op_clay_completion_providers_for_trigger"](trigger));
}

export type EditorRulesLike = {
  autocompleteTriggers?: Array<{ trigger?: string }>;
};

export function completionTriggerCharactersFromEditorRules(editorRules: EditorRulesLike): string[] {
  const triggers = editorRules?.autocompleteTriggers ?? [];
  const characters: string[] = [];
  for (const trigger of triggers) {
    const value = trigger?.trigger;
    if (typeof value === "string" && value.length > 0) {
      characters.push(value);
    }
  }
  return characters;
}
