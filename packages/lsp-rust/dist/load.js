import { serverRegisterDocumentAnalyzer } from "clay:language";
import "./server.js";
import { analyzerId, contributionId, lspRustPackageManifest } from "./index.js";

export async function loadLspRustPackage() {
  const packageManifest = lspRustPackageManifest();
  await serverRegisterDocumentAnalyzer({
    packageManifest,
    analyzer: {
      id: analyzerId,
      contribution: contributionId,
      modes: ["rust"],
      moduleSpecifier: import.meta.resolve("./server.js"),
      exportName: "handleDocumentAnalysis",
    },
  });
  return packageManifest;
}

export default loadLspRustPackage;
