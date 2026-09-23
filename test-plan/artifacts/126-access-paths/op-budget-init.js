// Plan 126 D2 live check: a package-facing `documents.open` of the workspace's
// ≥4 MiB file must fail with the typed `documents.document_too_large` error and
// must not carry document text into the diagnostic. The call is deliberately
// uncaught: the error surfaces through the shipped configuration-diagnostic
// path, which is what the manual step inspects.
import { serverOpenDocument } from "clay:documents";

await serverOpenDocument({ workspaceRootId: "1", path: "review.rs" });
