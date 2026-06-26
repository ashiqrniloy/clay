// Clay mode primitive facade skeleton.
//
// Mode APIs are server-owned registration/classification/activation APIs. They
// run at package load, document open, or explicit activation time and never make
// ordinary typing or paint depend on JavaScript in the Rust client.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.modes.runtime_unavailable: Clay mode APIs require the server runtime");
  }
  return ops;
}

function parse<T>(json: string): T {
  return JSON.parse(json) as T;
}

const activationRegistry = ((globalThis as typeof globalThis & { __clayModeActivations?: Record<string, unknown> }).__clayModeActivations ??= Object.create(null));
const activationKey = (apiPrefix: string, modeId: string): string => `${apiPrefix}:${modeId}`;

export function serverRegisterModePattern(packageManifest: unknown, declaration: unknown): unknown {
  const result = parse(requireOps().op_clay_modes_register_pattern(JSON.stringify(packageManifest ?? null), JSON.stringify(declaration ?? null)));
  const manifest = packageManifest as { clay?: { apiPrefix?: string } } | null;
  const mode = declaration as { modeId?: string; editorRules?: unknown; commands?: unknown; keymaps?: unknown } | null;
  if (manifest?.clay?.apiPrefix && mode?.modeId) {
    activationRegistry[activationKey(manifest.clay.apiPrefix, mode.modeId)] = {
      packageManifest,
      editorRules: mode.editorRules,
      commands: mode.commands,
      keymaps: mode.keymaps,
    };
  }
  return result;
}

export function serverClassifyDocument(input: unknown): unknown {
  return parse(requireOps().op_clay_modes_classify_document(JSON.stringify(input ?? null)));
}

export function serverActivateMajorMode(packageManifest: unknown, input: unknown): unknown {
  return parse(requireOps().op_clay_modes_activate_major_mode(JSON.stringify(packageManifest ?? null), JSON.stringify(input ?? null)));
}

export function serverActivateClassifiedMode(classification: unknown, input: unknown = {}): unknown {
  const classified = classification as { apiPrefix?: string; modeId?: string; documentId?: number } | null;
  const activation = activationRegistry[activationKey(String(classified?.apiPrefix), String(classified?.modeId))] as {
    packageManifest: unknown;
    editorRules?: unknown;
    commands?: unknown;
    keymaps?: unknown;
  } | undefined;
  if (!activation || classified?.documentId === undefined || !classified?.modeId) {
    throw new Error("clay.modes.activation_failed: classified mode has no registered activation metadata");
  }
  return serverActivateMajorMode(activation.packageManifest, {
    ...(input as Record<string, unknown>),
    documentId: classified.documentId,
    modeId: classified.modeId,
    editorRules: activation.editorRules,
    commands: activation.commands,
    keymaps: activation.keymaps,
  });
}

export function serverSelectDocumentManifest(options: unknown): never {
  void options;
  return requireOps().op_clay_runtime_unavailable("clay.modes.serverSelectDocumentManifest") as never;
}

export function serverRegisterDecorationProvider(options: unknown): never {
  void options;
  return requireOps().op_clay_runtime_unavailable("clay.modes.serverRegisterDecorationProvider") as never;
}

export function serverRegisterParseProvider(options: unknown): never {
  void options;
  return requireOps().op_clay_runtime_unavailable("clay.modes.serverRegisterParseProvider") as never;
}

export function serverRegisterFoldingProvider(options: unknown): never {
  void options;
  return requireOps().op_clay_runtime_unavailable("clay.modes.serverRegisterFoldingProvider") as never;
}
