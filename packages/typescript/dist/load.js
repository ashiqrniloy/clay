// @clay/typescript load entry. Phase 18.14 full language package:
// keeps the Phase 18.10 grammar contribution, adds a TypeScript major mode
// with editor behavior rules, one server-first command, a keyword completion
// provider, and an optional status-item UI contribution.

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

const typescriptEditorRules = buildCodeEditingManifest({
  indentSize: 2,
  lineComment: "//",
  electricOutdentCharacters: ["}"],
  autocompleteTriggers: ["."]
});

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
          priority: 20,
          triggerCharacters: completionTriggerCharactersFromEditorRules(typescriptEditorRules),
          wordBoundaryChars: [".", ";", ","],
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
    packageManifest: typescriptPackageManifest(),
    providerId: "typescript.keywords",
    priority: 20,
    triggerCharacters: completionTriggerCharactersFromEditorRules(typescriptEditorRules),
    wordBoundaryChars: [".", ";", ","],
    budgets: { timeoutMs: 300, maxItems: 32 }
  });
  await serverRegisterComponentContribution(typescriptPackageManifest(), {
    kind: "statusItem",
    id: "typescript.status.mode",
    style: { variant: "muted" },
    children: [{ kind: "label", id: "typescript.status.mode.label" }]
  });
}
