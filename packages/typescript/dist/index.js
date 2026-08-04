// @clay/typescript runtime entry. Re-exports the package manifest builder and
// load entry so users can inspect the contract or load the package explicitly.

import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

export { typescriptGrammarContract, loadTypescriptPackage } from "./load.js";

export const typescriptEditorRules = buildCodeEditingManifest({
  indentSize: 2,
  lineComment: "//",
  // Plan 071 task 11: explicit code movement (identical to the built-in
  // default; declared for discoverability).
  movement: { wordSeparators: "code" },
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

export const typescriptCommands = [
  {
    commandId: "typescript.toggleLineComment",
    packagePrefix: "typescript",
    routingPolicy: "server-first",
    displayName: "Toggle TypeScript Line Comment",
    permissions: []
  }
];

export const typescriptCompletionProvider = {
  providerId: "typescript.keywords",
  packagePrefix: "typescript",
  priority: 0,
  triggerCharacters: completionTriggerCharactersFromEditorRules(typescriptEditorRules),
  wordBoundaryChars: [".", ";", ","],
  items: [
    "as", "async", "await", "break", "case", "catch", "class", "const",
    "continue", "default", "do", "else", "enum", "export", "extends", "false",
    "finally", "for", "from", "function", "if", "implements", "import", "in",
    "instanceof", "interface", "let", "new", "return", "true", "type", "while"
  ],
  budgets: { timeoutMs: 300, maxItems: 32 }
};

export const typescriptSnippetProvider = {
  providerId: "typescript.snippets",
  packagePrefix: "typescript",
  priority: 0,
  triggerCharacters: completionTriggerCharactersFromEditorRules(typescriptEditorRules),
  wordBoundaryChars: [".", ";", ","],
  items: [
    { label: "interface", insertText: "interface ${1:Name} {\n  $0\n}", textFormat: "snippet", detail: "interface snippet" },
    { label: "type", insertText: "type ${1:Name} = ${2:Type};$0", textFormat: "snippet", detail: "type alias snippet" }
  ],
  budgets: { timeoutMs: 300, maxItems: 32 }
};

export const typescriptStatusItem = {
  kind: "statusItem",
  id: "typescript.status.mode",
  style: { variant: "muted" },
  children: [{ kind: "label", id: "typescript.status.mode.label" }]
};

export function typescriptPackageManifest() {
  return {
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
      extensionPoints: [{"id": "typescript.completionProviders", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["completionProvider"], "scopes": ["typescript.keywords", "typescript.snippets"], "summary": "Add or replace typescript completion and snippet providers."}, {"id": "typescript.languageLayers", "version": 1, "operations": ["append"], "contributionKinds": ["analyzer", "diagnosticSource", "intelligenceProvider", "decorationLayer"], "summary": "Append package-owned analyzers, diagnostics, intelligence, and decoration layers for typescript."}, {"id": "typescript.commands", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["command"], "scopes": ["typescript.toggleLineComment"], "summary": "Add or replace typescript commands."}, {"id": "typescript.ui", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["componentContribution", "statusItem", "panelContribution"], "scopes": ["typescript.status.mode"], "summary": "Add or replace typescript status and panel contributions."}, {"id": "typescript.grammar", "version": 1, "operations": ["replace"], "contributionKinds": ["grammar"], "scopes": ["typescript.typescript"], "summary": "Replace the typescript grammar, highlights query, and style map."}, {"id": "typescript.modePattern", "version": 1, "operations": ["append"], "contributionKinds": ["modePattern"], "summary": "Extend typescript file and mode patterns."}],
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
        completionProviders: [typescriptCompletionProvider, typescriptSnippetProvider].map((provider) => ({
          id: provider.providerId,
          priority: provider.priority,
          triggerCharacters: provider.triggerCharacters,
          wordBoundaryChars: provider.wordBoundaryChars,
          items: provider.items,
          budgets: provider.budgets
        })),
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
  };
}
