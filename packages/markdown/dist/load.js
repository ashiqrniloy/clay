// @clay/markdown load entry. Execute-only except the parse-handler module import.
import { serverRegisterParseHandler } from "clay:parse";
import { serverRegisterPanelContribution } from "clay:ui";

export async function loadMarkdownPackage() {
  let parserModule;
  try {
    parserModule = await import("./parser.js");
  } catch {
    // ponytail: copied fixture load roots may omit parser.js
  }
  await serverRegisterParseHandler({
    mode: "markdown",
    parseUnit: "line-group",
    viewportPriority: true,
    adapter: "./dist/parser.js",
    ...(parserModule ? { module: parserModule, exportName: "parseMarkdownDecorationUpdate" } : {}),
    maxWindowBytes: 64 * 1024,
    guardBytes: 4 * 1024,
    memoryBudgetBytes: 30 * 1024 * 1024,
    timeoutMs: 5000
  });
}

export async function markdownLoadMode() {
  return loadMarkdownPackage();
}

export function registerMarkdownPreview() {
  return serverRegisterPanelContribution({
    id: "markdown.preview",
    slot: "right",
    kind: "fixed",
    defaultVisibility: "hidden",
    actionTargets: ["markdown.togglePreview"],
    component: {
      kind: "panel",
      id: "markdown.preview.root",
      title: "Markdown Preview",
      children: []
    }
  });
}

export default loadMarkdownPackage;
