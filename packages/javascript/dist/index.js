// @clay/javascript runtime entry. Re-exports the package manifest builder and
// load entry so users can inspect the contract or load the package explicitly.

import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

export { javascriptGrammarContract, loadJavaScriptPackage } from "./load.js";

export const javascriptEditorRules = buildCodeEditingManifest({
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

