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

