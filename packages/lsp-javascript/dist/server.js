import { createTypescriptLanguageServerBridge } from "lsp-shared/typescript-language-server.js";
import {
  contributionId,
  languageIds,
  lspJavascriptPackageManifest,
  packageName,
} from "./index.js";

export function createJavascriptBridge(options) {
  return createTypescriptLanguageServerBridge({
    ...options,
    packageName,
    contribution: contributionId,
    diagnosticSource: "lsp-javascript",
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
    defaultBridge = createJavascriptBridge({
      startSession: startLanguageServerSession,
      publishDecorations: serverPublishDecorations,
      publishDiagnostics: serverPublishDiagnostics,
      packageManifest: lspJavascriptPackageManifest(),
    });
  }
  return defaultBridge.handle(event);
}
