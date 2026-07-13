// @clay/javascript runtime entry. Re-exports the package manifest builder and
// load entry so users can inspect the contract or load the package explicitly.

import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

export { javascriptGrammarContract, loadJavaScriptPackage } from "./load.js";

export const javascriptEditorRules = buildCodeEditingManifest({
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

export const javascriptCommands = [
  {
    commandId: "javascript.toggleLineComment",
    packagePrefix: "javascript",
    routingPolicy: "server-first",
    displayName: "Toggle JavaScript Line Comment",
    permissions: []
  }
];

export const javascriptCompletionProvider = {
  providerId: "javascript.keywords",
  packagePrefix: "javascript",
  priority: 0,
  triggerCharacters: completionTriggerCharactersFromEditorRules(javascriptEditorRules),
  wordBoundaryChars: [".", ";", ","],
  items: [
    "async", "await", "break", "case", "catch", "class", "const", "continue",
    "debugger", "default", "delete", "do", "else", "export", "extends", "false",
    "finally", "for", "from", "function", "if", "import", "in", "instanceof",
    "let", "new", "return", "switch", "throw", "true", "try", "typeof"
  ],
  budgets: { timeoutMs: 300, maxItems: 32 }
};

export const javascriptStatusItem = {
  kind: "statusItem",
  id: "javascript.status.mode",
  style: { variant: "muted" },
  children: [{ kind: "label", id: "javascript.status.mode.label" }]
};

export function javascriptPackageManifest() {
  return {
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
            id: javascriptCompletionProvider.providerId,
            priority: javascriptCompletionProvider.priority,
            triggerCharacters: javascriptCompletionProvider.triggerCharacters,
            wordBoundaryChars: javascriptCompletionProvider.wordBoundaryChars,
            items: javascriptCompletionProvider.items,
            budgets: javascriptCompletionProvider.budgets
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
  };
}
