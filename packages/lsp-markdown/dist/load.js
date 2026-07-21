import { serverRegisterDocumentAnalyzer } from "clay:language";
import "./server.js";
import { analyzerId, contributionId, lspMarkdownPackageManifest } from "./index.js";

export async function loadLspMarkdownPackage() {
  const packageManifest = lspMarkdownPackageManifest();
  await serverRegisterDocumentAnalyzer({
    analyzer: {
      id: analyzerId,
      contribution: contributionId,
      modes: ["markdown"],
      moduleSpecifier: import.meta.resolve("./server.js"),
      exportName: "handleDocumentAnalysis",
    },
  });
  return packageManifest;
}

export default loadLspMarkdownPackage;
