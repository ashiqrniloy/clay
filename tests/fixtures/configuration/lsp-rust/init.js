import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/rust");
const bridge = await loadPackage("@clay/lsp-rust");
Deno.core.ops.op_clay_runtime_record(`${bridge.name}:${bridge.contributions.languageServers}`);
