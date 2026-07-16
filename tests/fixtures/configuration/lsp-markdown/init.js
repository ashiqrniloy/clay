import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-markdown",
  contribution: "lsp-markdown.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/markdown");
const bridge = await loadPackage("@clay/lsp-markdown");
Deno.core.ops.op_clay_runtime_record(`${bridge.name}:${bridge.contributions.languageServers}`);
