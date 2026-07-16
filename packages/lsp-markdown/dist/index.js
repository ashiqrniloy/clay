export const packageName = "@clay/lsp-markdown";
export const packageVersion = "0.1.0";
export const apiPrefix = "lsp-markdown";
export const contributionId = "lsp-markdown.server";
export const analyzerId = "lsp-markdown.bridge";
export const languageIds = Object.freeze(["markdown"]);

export function lspMarkdownPackageManifest() {
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
        estimatedManifestBytes: 1500,
        hotPathPolicy: "authorized analyzer worker only; no keypress, paint, or client JavaScript",
      },
      contributions: {
        languageServers: [{
          id: contributionId,
          executable: "marksman",
          args: ["server"],
          inheritEnvironment: [],
        }],
        completionProviders: [{
          id: "lsp-markdown.completion",
          priority: 100,
          triggerCharacters: ["[", "#", "("],
          wordBoundaryChars: [".", ",", ";", "]"],
          items: [],
          budgets: { timeoutMs: 5000, maxItems: 256 },
        }],
        languageIntelligenceProviders: [{
          id: "lsp-markdown.intelligence",
          modes: ["markdown"],
          features: ["hover", "definition", "codeAction"],
          priority: 100,
          module: "./dist/server.js",
          exportName: "handleDocumentAnalysis",
          timeoutMs: 5000,
        }],
      },
    },
  };
}
