// @clay/typescript load entry. Phase 18.18 language package:
// registers native grammar/vocabulary metadata plus a TypeScript major mode
// with editor behavior rules, one server-first command, keyword/snippet
// completion providers, and an optional status-item UI contribution.

import { serverRegisterSyntaxGrammar } from "clay:syntax";
import { serverRegisterModePattern } from "clay:modes";
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterCompletionProvider, completionTriggerCharactersFromEditorRules } from "clay:completion";
import { serverRegisterComponentContribution } from "clay:ui";
import { buildCodeEditingManifest } from "clay:behavior";

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

const typescriptEditorRules = buildCodeEditingManifest({
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

const typescriptKeywords = Object.freeze([
  "as", "async", "await", "break", "case", "catch", "class", "const",
  "continue", "default", "do", "else", "enum", "export", "extends", "false",
  "finally", "for", "from", "function", "if", "implements", "import", "in",
  "instanceof", "interface", "let", "new", "return", "true", "type", "while"
]);

const typescriptSnippets = Object.freeze([
  { label: "interface", insertText: "interface ${1:Name} {\n  $0\n}", textFormat: "snippet", detail: "interface snippet" },
  { label: "type", insertText: "type ${1:Name} = ${2:Type};$0", textFormat: "snippet", detail: "type alias snippet" }
]);

const typescriptPackageManifest = () => ({
  name: "@clay/typescript",
  version: "0.1.0",
  type: "module",
  exports: {
    ".": "./dist/index.js",
    "./load": "./dist/load.js"
  },
  clay: {
    apiPrefix: "typescript",
    permissions: [
      "mode-registration",
      "mode-activation",
      "command-registration",
      "completion-provider",
      "parse-document",
      "render-decorations"
    ],
    modes: ["typescript"],
    entry: "./dist/index.js",
    loadEntry: "./dist/load.js",
    docs: "./docs/index.md",
    contributions: {
      syntaxGrammars: [typescriptGrammarContract().syntaxGrammar],
      modePatterns: [
        {
          mode: "typescript",
          displayName: "TypeScript",
          extensions: ["ts", "tsx", "mts", "cts"]
        }
      ],
      commands: [
        {
          id: "typescript.toggleLineComment",
          displayName: "Toggle TypeScript Line Comment",
          routingPolicy: "server-first"
        }
      ],
      completionProviders: [
        {
          id: "typescript.keywords",
          priority: 0,
          triggerCharacters: completionTriggerCharactersFromEditorRules(typescriptEditorRules),
          wordBoundaryChars: [".", ";", ","],
          items: typescriptKeywords,
          budgets: { timeoutMs: 300, maxItems: 32 }
        },
        {
          id: "typescript.snippets",
          priority: 0,
          triggerCharacters: completionTriggerCharactersFromEditorRules(typescriptEditorRules),
          wordBoundaryChars: [".", ";", ","],
          items: typescriptSnippets,
          budgets: { timeoutMs: 300, maxItems: 32 }
        }
      ],
      ui: {
        components: [
          {
            kind: "statusItem",
            id: "typescript.status.mode",
            style: { variant: "muted" }
          }
        ]
      }
    }
  }
});

export default async function loadTypescriptPackage() {
  await serverRegisterSyntaxGrammar(typescriptGrammarContract());
  await serverRegisterModePattern(typescriptPackageManifest(), {
    modeId: "typescript",
    displayName: "TypeScript",
    defaultFontRole: "monospace",
    extensions: ["ts", "tsx", "mts", "cts"],
    editorRules: typescriptEditorRules
  });
  await serverRegisterCommand(typescriptPackageManifest(), {
    commandId: "typescript.toggleLineComment",
    displayName: "Toggle TypeScript Line Comment",
    routingPolicy: "server-first",
    permissions: []
  });
  await serverRegisterCompletionProvider({
    packageManifest: typescriptPackageManifest()
  });
  await serverRegisterComponentContribution(typescriptPackageManifest(), {
    kind: "statusItem",
    id: "typescript.status.mode",
    style: { variant: "muted" },
    children: [{ kind: "label", id: "typescript.status.mode.label" }]
  });
}
