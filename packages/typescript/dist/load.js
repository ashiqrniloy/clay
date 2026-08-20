// @clay/typescript load entry. Execute-only: apply-record installs
// contributions from package.json.
export function typescriptGrammarContract() {
  return {
    packageName: "@clay/typescript",
    packageVersion: "0.1.0",
    packagePrefix: "typescript",
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: {
      languageId: "typescript",
      filePatterns: { extensions: ["ts", "tsx", "mts", "cts"] },
      grammar: { kind: "native", source: "tree-sitter-typescript" },
      queries: { highlights: "./queries/highlights.scm" },
      budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
    }
  };
}

export async function loadTypescriptPackage() {}

export default loadTypescriptPackage;
