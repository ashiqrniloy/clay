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

export function serverRegisterModePattern(packageManifest: unknown, declaration: unknown): unknown {
  return parse(requireOps().op_clay_modes_register_pattern(JSON.stringify(packageManifest ?? null), JSON.stringify(declaration ?? null)));
}

export function serverClassifyDocument(input: unknown): unknown {
  return parse(requireOps().op_clay_modes_classify_document(JSON.stringify(input ?? null)));
}

export function serverActivateMajorMode(packageManifest: unknown, input: unknown): unknown {
  return parse(requireOps().op_clay_modes_activate_major_mode(JSON.stringify(packageManifest ?? null), JSON.stringify(input ?? null)));
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
