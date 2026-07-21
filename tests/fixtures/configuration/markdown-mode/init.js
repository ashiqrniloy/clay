// Markdown smoke fixture: load the bundled package through the real
// loadPackage path so all contributions register with host-stamped
// provenance, then open sample.md and activate the classified mode.
// Inline package manifests and config-side contribution registration are
// not package authority (Plan 061 task 5).
import { serverOpenDocument } from "clay:documents";
import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
import { loadPackage } from "clay:packages";
import { serverListWorkspaceRoots } from "clay:workspace";

await loadPackage("@clay/markdown");

const roots = await serverListWorkspaceRoots();
if (roots.length > 0) {
  const opened = await serverOpenDocument({ workspaceRootId: roots[0].workspaceRootId, path: "sample.md" });
  const documentId = Number(opened?.metadata?.documentId ?? 1);
  const documentPath = String(opened?.metadata?.path ?? "sample.md");
  const classification = serverClassifyDocument({ documentId, path: documentPath });
  serverActivateClassifiedMode(classification, { path: documentPath });
}
