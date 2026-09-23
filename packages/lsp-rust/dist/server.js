import { createLspBridge } from "lsp-shared/bridge.js";
import {
  contributionId,
  lspRustPackageManifest,
  packageName,
} from "./index.js";

export function createRustAnalyzerBridge(options) {
  return createLspBridge({
    ...options,
    packageName,
    contribution: contributionId,
    diagnosticSource: "rust-analyzer",
    languageId: "rust",
    diagnostics: "pull",
    errorPrefix: "lsp-rust",
    features: ["completion", "hover", "definition", "codeAction", "signatureHelp", "inlayHint"],
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
    defaultBridge = createRustAnalyzerBridge({
      startSession: startLanguageServerSession,
      publishDecorations: serverPublishDecorations,
      publishDiagnostics: serverPublishDiagnostics,
      packageManifest: lspRustPackageManifest(),
    });
  }
  return defaultBridge.handle(event);
}
