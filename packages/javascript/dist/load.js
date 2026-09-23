// @clay/javascript load entry. Execute-only: apply-record installs
// contributions from package.json.
export function javascriptGrammarContract() {
  return {
    packageName: "@clay/javascript",
    packageVersion: "0.1.0",
    packagePrefix: "javascript",
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: {
      languageId: "javascript",
      filePatterns: { extensions: ["js", "jsx", "mjs", "cjs"] },
      grammar: { kind: "native", source: "tree-sitter-javascript" },
      queries: { highlights: "./queries/highlights.scm" },
      budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
    }
  };
}

export async function loadJavaScriptPackage() {}

export default loadJavaScriptPackage;
