// @clay/typescript load entry. Grammar-only package: no modes, commands, completions,
// UI, key behavior, or language-specific Rust branches. Syntax highlighting
// metadata lives in package.json under clay.contributions.syntaxGrammars and is
// validated/registered through clay:syntax at package load time.
import { serverRegisterSyntaxGrammar } from "clay:syntax";

export function typescriptGrammarContract() {
  return {
    packageName: "@clay/typescript",
    packageVersion: "0.1.0",
    packagePrefix: "typescript",
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: {
      languageId: "typescript",
      filePatterns: { extensions: ["ts", "tsx"] },
      grammar: { kind: "tree-sitter-wasm", path: "./grammars/typescript.wasm", source: "tree-sitter-typescript" },
      queries: { highlights: "./queries/highlights.scm" },
      styleMap: {
        keyword: "keyword.control",
        string: "string.quoted",
        comment: "comment.line",
        punctuation: "punctuation.definition"
      },
      budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
    }
  };
}

export default async function loadTypescriptGrammar() {
  return serverRegisterSyntaxGrammar(typescriptGrammarContract());
}
