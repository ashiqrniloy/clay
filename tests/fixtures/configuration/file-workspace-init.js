import {
  serverGetDocumentStatus,
  serverListDocuments,
  serverOpenDocument,
} from "clay:documents";
import { serverListWorkspaceRoots } from "clay:workspace";

const roots = await serverListWorkspaceRoots();
if (roots.length > 0) {
  const opened = await serverOpenDocument({
    workspaceRootId: roots[0].workspaceRootId,
    path: "note.txt",
  });
  await serverGetDocumentStatus(opened.metadata.documentId);
  await serverListDocuments();
}
