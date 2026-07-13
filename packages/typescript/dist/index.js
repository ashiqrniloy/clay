// @clay/typescript runtime entry. Re-exports the package manifest builder and
// load entry so users can inspect the contract or load the package explicitly.

import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

export { typescriptGrammarContract, loadTypescriptPackage } from "./load.js";

export const typescriptEditorRules = buildCodeEditingManifest({
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
            id: typescriptCompletionProvider.providerId,
            priority: typescriptCompletionProvider.priority,
            triggerCharacters: typescriptCompletionProvider.triggerCharacters,
            wordBoundaryChars: typescriptCompletionProvider.wordBoundaryChars,
            items: typescriptCompletionProvider.items,
            budgets: typescriptCompletionProvider.budgets
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
  };
}
