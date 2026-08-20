import {
  createLspBridge,
  languageIdForRelativePath as languageIdFromMap,
} from "lsp-shared/bridge.js";
import {
  contributionId,
  languageIds,
  lspMarkdownPackageManifest,
  packageName,
} from "./index.js";

const EXTENSION_LANGUAGE_IDS = Object.freeze({
  md: "markdown",
  markdown: "markdown",
  mdown: "markdown",
});

export function languageIdForRelativePath(relativePath, allowedLanguageIds = languageIds) {
  return languageIdFromMap(relativePath, allowedLanguageIds, EXTENSION_LANGUAGE_IDS, "lsp-markdown");
}

export function createMarksmanBridge(options) {
  return createLspBridge({
    ...options,
    packageName,
    contribution: contributionId,
    diagnosticSource: "lsp-markdown",
    languageIds: [...languageIds],
    languageIdsByExtension: EXTENSION_LANGUAGE_IDS,
    diagnostics: "push",
    features: ["completion", "hover", "definition", "codeAction"],
    errorPrefix: "lsp-markdown",
  });
}

let defaultBridge;

export async function handleDocumentAnalysis(event) {
  if (!defaultBridge) {
    const [
      { startLanguageServerSession },
      { serverPublishDecorations },
      { serverPublishDiagnostics },
    ] = await Promise.all([
      import("clay:language-server"),
      import("clay:decorations"),
      import("clay:diagnostics"),
    ]);
    defaultBridge = createMarksmanBridge({
      startSession: startLanguageServerSession,
      publishDecorations: serverPublishDecorations,
      publishDiagnostics: serverPublishDiagnostics,
      packageManifest: lspMarkdownPackageManifest(),
    });
  }
  return defaultBridge.handle(event);
}
