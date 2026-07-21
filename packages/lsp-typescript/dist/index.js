export const packageName = "@clay/lsp-typescript";
export const packageVersion = "0.1.0";
export const apiPrefix = "lsp-typescript";
export const contributionId = "lsp-typescript.server";
export const analyzerId = "lsp-typescript.bridge";
export const languageIds = Object.freeze(["typescript", "typescriptreact"]);

export function lspTypescriptPackageManifest() {
  return {
    name: packageName,
    version: packageVersion,
    type: "module",
    exports: {
      ".": "./dist/index.js",
      "./load": "./dist/load.js",
      "./server": "./dist/server.js",
    },
    clay: {
      apiPrefix,
      entry: "./dist/index.js",
      loadEntry: "./dist/load.js",
      permissions: ["parse-document", "completion-provider", "render-decorations"],
      capabilities: ["language-server"],
      modes: [],
      docs: "./docs/index.md",
      apiDependencies: [
        "clay.language.serverRegisterDocumentAnalyzer",
        "clay.language-server.startLanguageServerSession",
        "clay.decorations.serverPublishDecorations",
        "clay.diagnostics.serverPublishDiagnostics",
      ],
      performance: {
        estimatedManifestBytes: 1600,
        hotPathPolicy: "authorized analyzer worker only; no keypress, paint, or client JavaScript",
      },
      extensionPoints: [{"id": "lsp-typescript.providers", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["completionProvider", "intelligenceProvider", "analyzer"], "scopes": ["lsp-typescript.completion", "lsp-typescript.intelligence"], "summary": "Add or replace language-server completion, intelligence, and analyzer providers. The language-server descriptor and grant are not mutable."}],
      contributions: {
        languageServers: [{
          id: contributionId,
          executable: "typescript-language-server",
          args: ["--stdio"],
          inheritEnvironment: ["HOME", "PATH"],
        }],
        completionProviders: [{
          id: "lsp-typescript.completion",
          priority: 100,
          triggerCharacters: [".", "\"", "'", "/", "@", "<"],
          wordBoundaryChars: [".", ";", ","],
          items: [],
          budgets: { timeoutMs: 5000, maxItems: 256 },
        }],
        languageIntelligenceProviders: [{
          id: "lsp-typescript.intelligence",
          modes: ["typescript"],
          features: ["hover", "definition", "codeAction", "signatureHelp"],
          priority: 100,
          module: "./dist/server.js",
          exportName: "handleDocumentAnalysis",
          timeoutMs: 5000,
        }],
      },
    },
  };
}
