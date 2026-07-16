import { serverRegisterDocumentAnalyzer } from "clay:language";
import "./server.js";
import { analyzerId, contributionId, lspTypescriptPackageManifest } from "./index.js";

export async function loadLspTypescriptPackage() {
  const packageManifest = lspTypescriptPackageManifest();
  await serverRegisterDocumentAnalyzer({
    packageManifest,
    analyzer: {
      id: analyzerId,
      contribution: contributionId,
      modes: ["typescript"],
      moduleSpecifier: import.meta.resolve("./server.js"),
      exportName: "handleDocumentAnalysis",
    },
  });
  return packageManifest;
}

export default loadLspTypescriptPackage;
