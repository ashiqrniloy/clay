import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

export const packageName = "@clay/rust";
export const packageVersion = "0.1.0";
export const apiPrefix = "rust";
export const modeId = "rust";

export const supportedExtensions = ["rs"];
export const supportedFileNames = ["Cargo.toml"];

export const rustSyntaxGrammar = Object.freeze({
  languageId: "rust",
  filePatterns: { extensions: ["rs"] },
  grammar: {
    kind: "tree-sitter-wasm",
    path: "./grammars/rust.wasm",
    source: "tree-sitter-rust"
  },
  queries: { highlights: "./queries/highlights.scm" },
  styleMap: {
    keyword: "keyword.control",
    string: "string.quoted",
    comment: "comment.line",
    punctuation: "punctuation.definition"
  },
  budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
});

// Rust-appropriate behavior rules expressed through the generic Clay
// `buildCodeEditingManifest` helper. The editor core deserializes the result
// into language-agnostic EnterRule, PairRule, CommentContinuationRule, TabRule,
// and ElectricCharacterRule types; no Rust-specific behavior logic lives in the
// Rust client or server.
export const rustEditorRules = Object.freeze(
  buildCodeEditingManifest({
    indentSize: 4,
    lineComment: "//",
    electricOutdentCharacters: ["}"],
    autocompleteTriggers: [".", "::"]
  })
);

export const rustCommands = Object.freeze([
  {
    id: "rust.toggleLineComment",
    userFacingName: "Toggle Rust Line Comment",
    routingPolicy: "ServerFirst",
    permissions: []
  }
]);

export const rustCompletionProvider = Object.freeze({
  id: "rust.keywords",
  priority: 20,
  triggerCharacters: completionTriggerCharactersFromEditorRules(rustEditorRules),
  wordBoundaryChars: [".", "::", ";", ","],
  budgets: { timeoutMs: 300, maxItems: 32 }
});

export const rustStatusItem = Object.freeze({
  kind: "statusItem",
  id: "rust.status.mode",
  style: { variant: "muted" }
});

export function rustPackageManifest() {
  return {
    name: packageName,
    version: packageVersion,
    type: "module",
    exports: {
      ".": "./dist/index.js",
      "./load": "./dist/load.js"
    },
    clay: {
      apiPrefix,
      entry: "./dist/index.js",
      loadEntry: "./dist/load.js",
      permissions: [
        "mode-registration",
        "mode-activation",
        "command-registration",
        "completion-provider",
        "parse-document",
        "render-decorations"
      ],
      modes: [modeId],
      docs: "./docs/index.md",
      apiDependencies: [
        "clay.syntax.serverRegisterSyntaxGrammar",
        "clay.modes.serverRegisterModePattern",
        "clay.behavior.buildCodeEditingManifest",
        "clay.commands.serverRegisterCommand",
        "clay.completion.serverRegisterCompletionProvider",
        "clay.completion.completionTriggerCharactersFromEditorRules",
        "clay.ui.serverRegisterComponentContribution"
      ],
      performance: {
        estimatedManifestBytes: 1700,
        hotPathPolicy: "grammar metadata validated at load; no hot-path JS"
      },
      contributions: {
        syntaxGrammars: [rustSyntaxGrammar],
        modePatterns: [
          {
            mode: modeId,
            displayName: "Rust",
            extensions: supportedExtensions,
            fileNames: supportedFileNames
          }
        ],
        commands: rustCommands.map((command) => ({
          id: command.id,
          displayName: command.userFacingName,
          routingPolicy: "server-first"
        })),
        completionProviders: [rustCompletionProvider],
        ui: {
          components: [rustStatusItem]
        }
      }
    }
  };
}

export { rustGrammarContract, loadRustPackage, default } from "./load.js";
