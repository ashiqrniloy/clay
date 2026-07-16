import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-javascript",
  contribution: "lsp-javascript.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/javascript");
const bridge = await loadPackage("@clay/lsp-javascript");
Deno.core.ops.op_clay_runtime_record(`${bridge.name}:${bridge.contributions.languageServers}`);
