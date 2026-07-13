// @clay/rust load entry. Phase 18.18 registers native grammar/vocabulary
// metadata plus a `rust` major mode, behavior
// manifest, package-prefixed command, keyword completion provider, and an
// optional status-item UI contribution through generic Clay primitives.
// No language-specific Rust branches, native widgets, or hot-path JS.
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterCompletionProvider } from "clay:completion";
import { serverRegisterModePattern } from "clay:modes";
import { serverRegisterSyntaxGrammar } from "clay:syntax";
import { serverRegisterComponentContribution } from "clay:ui";

import {
  apiPrefix,
  modeId,
  packageName,
  packageVersion,
  rustCommands,
  rustCompletionProvider,
  rustEditorRules,
  rustPackageManifest,
  rustStatusItem,
  rustSyntaxGrammar,
  supportedExtensions,
  supportedFileNames
} from "./index.js";

export function rustGrammarContract() {
  return {
    packageName,
    packageVersion,
    packagePrefix: apiPrefix,
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: rustSyntaxGrammar
  };
}

export async function loadRustPackage() {
  const manifest = rustPackageManifest();

  // Register inert native grammar/vocabulary metadata first; syntax selection
  // remains independent of active major mode.
  await serverRegisterSyntaxGrammar(rustGrammarContract());

  // Register the Rust major-mode pattern. Documents opened with matching
  // extensions or file names will classify as `rust` and activate with the
  // package-supplied editor rules below.
  await serverRegisterModePattern(manifest, {
    modeId,
    displayName: "Rust",
    defaultFontRole: "monospace",
    extensions: supportedExtensions,
    fileNames: supportedFileNames,
    editorRules: rustEditorRules
  });

  // Register the package-prefixed command metadata so it appears in help,
  // command palettes, and validated action intents.
  for (const command of rustCommands) {
    await serverRegisterCommand(manifest, {
      commandId: command.id,
      displayName: command.userFacingName,
      routingPolicy: command.routingPolicy,
      permissions: command.permissions
    });
  }

  // Register bounded static keyword text replacements. Snippet transforms land in Phase 18.19.
  await serverRegisterCompletionProvider({
    packageManifest: manifest,
    packageName,
    packageVersion,
    packagePrefix: apiPrefix,
    apiPrefix,
    permissions: ["completion-provider"],
    completionProvider: rustCompletionProvider,
    contribution: rustCompletionProvider,
    providerId: rustCompletionProvider.id,
    triggerCharacters: rustCompletionProvider.triggerCharacters,
    wordBoundaryChars: rustCompletionProvider.wordBoundaryChars,
    priority: rustCompletionProvider.priority,
    timeoutMs: rustCompletionProvider.budgets.timeoutMs,
    maxItems: rustCompletionProvider.budgets.maxItems
  });

  // Register an optional status-item contribution. The item is inert metadata
  // validated by Clay before any client publication.
  await serverRegisterComponentContribution(manifest, rustStatusItem);

  return manifest;
}

// Default activation entry invoked by `loadPackage("@clay/rust")`.
export default loadRustPackage;
