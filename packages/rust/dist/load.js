// @clay/rust load entry. Execute-only: apply-record installs modes, commands,
// completions, syntax, and UI from package.json.
export function rustGrammarContract() {
  return {
    packageName: "@clay/rust",
    packageVersion: "0.1.0",
    packagePrefix: "rust",
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: {
      languageId: "rust",
      filePatterns: { extensions: ["rs"] },
      grammar: { kind: "native", source: "tree-sitter-rust" },
      queries: { highlights: "./queries/highlights.scm" },
      budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
    }
  };
}

export async function loadRustPackage() {}

export default loadRustPackage;
