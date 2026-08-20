import path from "node:path";
import { pathToFileURL } from "node:url";

const packagesRoot = path.resolve(import.meta.dirname, "../../../packages");

export async function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith("lsp-shared/")) {
    return {
      shortCircuit: true,
      url: pathToFileURL(path.join(packagesRoot, specifier)).href,
    };
  }
  return nextResolve(specifier, context);
}
