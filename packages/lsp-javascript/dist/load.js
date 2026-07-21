import { serverRegisterDocumentAnalyzer } from "clay:language";
import "./server.js";
import { analyzerId, contributionId, lspJavascriptPackageManifest } from "./index.js";

export async function loadLspJavascriptPackage() {
  const packageManifest = lspJavascriptPackageManifest();
  await serverRegisterDocumentAnalyzer({
    analyzer: {
      id: analyzerId,
      contribution: contributionId,
      modes: ["javascript"],
      moduleSpecifier: import.meta.resolve("./server.js"),
      exportName: "handleDocumentAnalysis",
    },
  });
  return packageManifest;
}

export default loadLspJavascriptPackage;
