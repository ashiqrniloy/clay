export const packageName = "@clay/lsp-rust";
export const packageVersion = "0.1.0";
export const apiPrefix = "lsp-rust";
export const contributionId = "lsp-rust.server";
export const analyzerId = "lsp-rust.bridge";

export function lspRustPackageManifest() {
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
      extensionPoints: [{"id": "lsp-rust.providers", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["completionProvider", "intelligenceProvider", "analyzer"], "scopes": ["lsp-rust.completion", "lsp-rust.intelligence"], "summary": "Add or replace language-server completion, intelligence, and analyzer providers. The language-server descriptor and grant are not mutable."}],
      contributions: {
        languageServers: [{
          id: contributionId,
          executable: "rustup",
          args: ["run", "stable", "rust-analyzer"],
          inheritEnvironment: ["HOME", "PATH"],
        }],
        completionProviders: [{
          id: "lsp-rust.completion",
          priority: 100,
          triggerCharacters: [":", ".", "'", "("],
          wordBoundaryChars: [".", "::", ";", ","],
          items: [],
          budgets: { timeoutMs: 5000, maxItems: 256 },
        }],
        languageIntelligenceProviders: [{
          id: "lsp-rust.intelligence",
          modes: ["rust"],
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
