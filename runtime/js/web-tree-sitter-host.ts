// Internal Tier 2 web-tree-sitter host adapter contract.
//
// Packages do not import this module directly. Clay host code owns the adapter,
// validates package-root-confined grammar/query assets, initializes
// web-tree-sitter once, and returns inert capture records to the shared syntax
// decoration pipeline. No network, shell, package-manager, client JS, native
// handles, or raw ops are part of this contract.

export type WebTreeSitterArtifactContract = {
  contributionId: string;
  packageName: string;
  packagePrefix: string;
  grammarPath: `./grammars/${string}.wasm`;
  highlightsQueryPath: `./queries/${string}.scm`;
  localsQueryPath?: `./queries/${string}.scm`;
  injectionsQueryPath?: `./queries/${string}.scm`;
};

export type WebTreeSitterCapture = {
  startByte: number;
  endByte: number;
  captureName: string;
};

export type WebTreeSitterDiagnosticCapture = {
  startByte: number;
  endByte: number;
  kind: "error" | "missing";
};

type WebTreeSitterNode = {
  startIndex: number;
  endIndex: number;
  hasError: boolean;
  isError: boolean;
  isMissing: boolean;
  childCount: number;
  child(index: number): WebTreeSitterNode | null;
};

export function collectWebTreeSitterDiagnostics(
  root: WebTreeSitterNode,
  viewportStart: number,
  viewportEnd: number,
  limit = 128,
): WebTreeSitterDiagnosticCapture[] {
  if (!root.hasError) return [];
  const captures: WebTreeSitterDiagnosticCapture[] = [];
  const stack = [root];
  while (stack.length && captures.length < limit) {
    const node = stack.pop()!;
    if (node.isError || node.isMissing) {
      const startByte = Math.max(node.startIndex, viewportStart);
      const endByte = Math.min(node.endIndex, viewportEnd);
      if (
        startByte < endByte ||
        (node.isMissing && node.startIndex >= viewportStart && node.startIndex <= viewportEnd)
      ) {
        captures.push({
          startByte,
          endByte,
          kind: node.isMissing ? "missing" : "error",
        });
      }
    }
    for (let index = node.childCount - 1; index >= 0; index--) {
      const child = node.child(index);
      if (child?.hasError || child?.isError || child?.isMissing) stack.push(child);
    }
  }
  return captures;
}

let initPromise: Promise<void> | undefined;
const languageCache = new Map<string, unknown>();
const queryCache = new Map<string, unknown>();

export function validateWebTreeSitterArtifactContract(
  contract: WebTreeSitterArtifactContract,
): WebTreeSitterArtifactContract {
  assertPackageAsset(contract.grammarPath, "./grammars/", ".wasm");
  assertPackageAsset(contract.highlightsQueryPath, "./queries/", ".scm");
  if (contract.localsQueryPath) assertPackageAsset(contract.localsQueryPath, "./queries/", ".scm");
  if (contract.injectionsQueryPath) assertPackageAsset(contract.injectionsQueryPath, "./queries/", ".scm");
  return contract;
}

export async function initializeWebTreeSitter(Parser: { init(options?: unknown): Promise<void> }): Promise<void> {
  initPromise ??= Parser.init({
    locateFile(file: string) {
      if (file !== "tree-sitter.wasm") {
        throw new Error("syntax.web_tree_sitter.invalid_runtime_asset");
      }
      return "clay://runtime/tree-sitter.wasm";
    },
  });
  await initPromise;
}

export async function loadWebTreeSitterLanguage(
  Language: { load(path: string): Promise<unknown> },
  contract: WebTreeSitterArtifactContract,
): Promise<unknown> {
  const validated = validateWebTreeSitterArtifactContract(contract);
  const cached = languageCache.get(validated.grammarPath);
  if (cached) return cached;
  const language = await Language.load(validated.grammarPath);
  languageCache.set(validated.grammarPath, language);
  return language;
}

export function cachedWebTreeSitterQuery<T>(key: string, compile: () => T): T {
  const cached = queryCache.get(key) as T | undefined;
  if (cached) return cached;
  const query = compile();
  queryCache.set(key, query);
  return query;
}

function assertPackageAsset(path: string, prefix: string, suffix: string): void {
  if (
    !path.startsWith(prefix) ||
    !path.endsWith(suffix) ||
    path.includes("..") ||
    path.includes("\\") ||
    path.includes("://") ||
    path.startsWith("/")
  ) {
    throw new Error(`syntax.web_tree_sitter.invalid_artifact_path: ${path}`);
  }
}
