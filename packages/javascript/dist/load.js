// @clay/javascript load entry. Phase 18.18 language package:
// registers native grammar/vocabulary metadata plus a JavaScript major mode
// with editor behavior rules, one server-first command, a keyword completion
// provider, and an optional status-item UI contribution.

import { serverRegisterSyntaxGrammar } from "clay:syntax";
import { serverRegisterModePattern } from "clay:modes";
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterCompletionProvider, completionTriggerCharactersFromEditorRules } from "clay:completion";
import { serverRegisterComponentContribution } from "clay:ui";
import { buildCodeEditingManifest } from "clay:behavior";

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
      styleMap: {
        keyword: { type: "Keyword" },
        string: { type: "String" },
        comment: { type: "Comment" },
        punctuation: { type: "Operator" },
        text: { type: "Paragraph" },
        function: { type: "Function" },
        "function.declaration": { type: "Function", modifiers: ["Declaration"] },
        type: { type: "Type" },
        number: { type: "Number" }
      },
      budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
    }
  };
}

const javascriptEditorRules = buildCodeEditingManifest({
  indentSize: 2,
  lineComment: "//",
  pairs: [
    { open: "(", close: ")" },
    { open: "[", close: "]" },
    { open: "{", close: "}" },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
    { open: "`", close: "`" }
  ],
  electricOutdentCharacters: ["}", ")", "]"],
  autocompleteTriggers: ["."]
});

const javascriptKeywords = Object.freeze([
  "async", "await", "break", "case", "catch", "class", "const", "continue",
  "debugger", "default", "delete", "do", "else", "export", "extends", "false",
  "finally", "for", "from", "function", "if", "import", "in", "instanceof",
  "let", "new", "return", "switch", "throw", "true", "try", "typeof"
]);

const javascriptPackageManifest = () => ({
  name: "@clay/javascript",
  version: "0.1.0",
  type: "module",
  exports: {
    ".": "./dist/index.js",
    "./load": "./dist/load.js"
  },
  clay: {
    apiPrefix: "javascript",
    permissions: [
      "mode-registration",
      "mode-activation",
      "command-registration",
      "completion-provider",
      "parse-document",
      "render-decorations"
    ],
    modes: ["javascript"],
    entry: "./dist/index.js",
    loadEntry: "./dist/load.js",
    docs: "./docs/index.md",
    contributions: {
      syntaxGrammars: [javascriptGrammarContract().syntaxGrammar],
      modePatterns: [
        {
          mode: "javascript",
          displayName: "JavaScript",
          extensions: ["js", "jsx", "mjs", "cjs"]
        }
      ],
      commands: [
        {
          id: "javascript.toggleLineComment",
          displayName: "Toggle JavaScript Line Comment",
          routingPolicy: "server-first"
        }
      ],
      completionProviders: [
        {
          id: "javascript.keywords",
          priority: 0,
          triggerCharacters: completionTriggerCharactersFromEditorRules(javascriptEditorRules),
          wordBoundaryChars: [".", ";", ","],
          items: javascriptKeywords,
          budgets: { timeoutMs: 300, maxItems: 32 }
        }
      ],
      ui: {
        components: [
          {
            kind: "statusItem",
            id: "javascript.status.mode",
            style: { variant: "muted" }
          }
        ]
      }
    }
  }
});

export default async function loadJavaScriptPackage() {
  await serverRegisterSyntaxGrammar({});
  await serverRegisterModePattern({
    modeId: "javascript",
    displayName: "JavaScript",
    defaultFontRole: "monospace",
    extensions: ["js", "jsx", "mjs", "cjs"],
    editorRules: javascriptEditorRules
  });
  await serverRegisterCommand({
    commandId: "javascript.toggleLineComment",
    displayName: "Toggle JavaScript Line Comment",
    routingPolicy: "server-first",
    permissions: []
  });
  await serverRegisterCompletionProvider({
    providerId: "javascript.keywords",
    priority: 0,
    triggerCharacters: completionTriggerCharactersFromEditorRules(javascriptEditorRules),
    wordBoundaryChars: [".", ";", ","],
    items: javascriptKeywords,
    budgets: { timeoutMs: 300, maxItems: 32 }
  });
  await serverRegisterComponentContribution({
    kind: "statusItem",
    id: "javascript.status.mode",
    style: { variant: "muted" },
    children: [{ kind: "label", id: "javascript.status.mode.label" }]
  });
}
