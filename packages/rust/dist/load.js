// @clay/rust load entry. Grammar-only package: no modes, commands, completions,
// UI, key behavior, or language-specific Rust branches. Syntax highlighting
// metadata lives in package.json under clay.contributions.syntaxGrammars and is
// validated/registered through clay:syntax at package load time.
import { serverRegisterSyntaxGrammar } from "clay:syntax";

export function rustGrammarContract() {
  return {
    packageName: "@clay/rust",
    packageVersion: "0.1.0",
    packagePrefix: "rust",
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: {
      languageId: "rust",
      filePatterns: { extensions: ["rs"] },
      grammar: { kind: "tree-sitter-wasm", path: "./grammars/rust.wasm", source: "tree-sitter-rust" },
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

export default async function loadRustGrammar() {
  return serverRegisterSyntaxGrammar(rustGrammarContract());
}
