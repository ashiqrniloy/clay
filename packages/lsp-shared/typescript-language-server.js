import {
  createLspBridge,
  languageIdForRelativePath as languageIdFromMap,
  lspClientCapabilities,
} from "./bridge.js";

const EXTENSION_LANGUAGE_IDS = Object.freeze({
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "typescriptreact",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascriptreact",
});

export const typescriptLanguageServerClientCapabilities = lspClientCapabilities({
  diagnostics: "push",
});

export function defaultTypescriptLanguageServerInitializationOptions({ tsserverPath } = {}) {
  const options = {
    hostInfo: "Clay",
    disableAutomaticTypingAcquisition: true,
  };
  if (typeof tsserverPath === "string" && tsserverPath.length > 0) {
    options.tsserver = { path: tsserverPath };
  }
  return options;
}

export function languageIdForRelativePath(relativePath, allowedLanguageIds) {
  return languageIdFromMap(relativePath, allowedLanguageIds, EXTENSION_LANGUAGE_IDS, "lsp-typescript");
}

export function createTypescriptLanguageServerBridge(options) {
  return createLspBridge({
    ...options,
    languageIdsByExtension: EXTENSION_LANGUAGE_IDS,
    diagnostics: "push",
    errorPrefix: "lsp-typescript",
    initializationOptions: {
      ...defaultTypescriptLanguageServerInitializationOptions({ tsserverPath: options.tsserverPath }),
      ...(options.initializationOptions ?? {}),
    },
  });
}
