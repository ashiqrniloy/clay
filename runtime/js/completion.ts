// Clay completion provider primitive facade.
//
// Completion APIs register inert package provider metadata at load/reload time.
// Provider execution stays server-side UI-reactive/cancellable and this facade
// does not expose raw ops, callbacks, client JS, filesystem, network, shell, AI,
// WASM, native-widget, or package-manager authority.

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
  items?: string[];
  priority?: number;
  timeoutMs?: number;
  maxItems?: number;
  handler?: never;
  callback?: never;
  complete?: never;
  function?: never;
  clientJavaScript?: never;
  nativeHandle?: never;
  rawOps?: never;
  module?: never;
};

export function serverRegisterCompletionProvider(options: ServerRegisterCompletionProviderOptions): unknown {
  for (const key of ["handler", "callback", "complete", "function", "clientJavaScript", "nativeHandle", "rawOps", "module"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.completion.invalid_provider: executable or raw authority field ${key} is not accepted by the public registration contract`);
    }
  }
  return parseResult(requireOps()["op_clay_completion_register_completion_provider"](JSON.stringify(options ?? null)));
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
