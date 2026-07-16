import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-typescript",
  contribution: "lsp-typescript.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/typescript");
const bridge = await loadPackage("@clay/lsp-typescript");
Deno.core.ops.op_clay_runtime_record(`${bridge.name}:${bridge.contributions.languageServers}`);
