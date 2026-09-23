import { createTypescriptLanguageServerBridge } from "lsp-shared/typescript-language-server.js";
import {
  contributionId,
  languageIds,
  lspTypescriptPackageManifest,
  packageName,
} from "./index.js";

export function createTypescriptBridge(options) {
  return createTypescriptLanguageServerBridge({
    ...options,
    packageName,
    contribution: contributionId,
    diagnosticSource: "lsp-typescript",
    languageIds: [...languageIds],
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
    defaultBridge = createTypescriptBridge({
      startSession: startLanguageServerSession,
      publishDecorations: serverPublishDecorations,
      publishDiagnostics: serverPublishDiagnostics,
      packageManifest: lspTypescriptPackageManifest(),
    });
  }
  return defaultBridge.handle(event);
}
