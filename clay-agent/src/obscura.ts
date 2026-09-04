/**
 * Host-owned Obscura capability (decision 2159). The daemon owns the Obscura
 * process lifecycle: spawn, readiness, tool registration, and close. Everything
 * is hidden when the binary is absent — missing Obscura is a capability
 * reduction, never an error. Nothing here is "sandboxed": Obscura children are
 * same-user processes under the host's authority.
 */
import { createObscuraMcpTools, connectObscuraCdp, type ObscuraCdpSession } from "@arnilo/prism-web-tools/obscura";
import { createObscuraWebTools, type ObscuraSearchProfile } from "@arnilo/prism-web-tools/obscura";
import { createBrowserTools } from "@arnilo/prism-web-tools/browser";
import type { ToolDefinition } from "@arnilo/prism";
import { resolveObscuraBinary } from "./resolve-obscura.js";

export { resolveObscuraBinary, resetObscuraBinaryCache } from "./resolve-obscura.js";

export interface ObscuraHarness {
  /** All registered tools: `obscura_*` MCP tools plus browser tools. */
  readonly tools: ToolDefinition[];
  close(): Promise<void>;
}

function assertNoInsecureFlags(args: readonly string[]): void {
  // Defense in depth: Prism rejects these too; we never pass allowInsecureFlags.
  for (const arg of args) {
    if (arg === "--allow-private-network" || arg === "--allow-file-access") {
      throw Object.assign(new Error(`insecure Obscura flag rejected: ${arg}`), { rpcCode: -32602 });
    }
  }
}

/**
 * Spawn the Obscura harness. Managed mode only: `connectObscuraCdp` spawns
 * `obscura serve` on loopback (daemon-owned, SIGTERM→SIGKILL on close) and
 * `createObscuraMcpTools` owns the `obscura mcp` stdio child. CDP binds
 * 127.0.0.1 only — never a remote endpoint, no allowRemoteEndpoint.
 */
export async function spawnObscuraHarness(options: {
  readonly command: string;
  readonly cdpPort?: number;
  /** Replaceable HTML search profile for `web_search` (default profile otherwise). */
  readonly searchProfile?: ObscuraSearchProfile;
  /** Test seam: replace the CDP attach (default `connectObscuraCdp`). */
  readonly connectCdp?: typeof connectObscuraCdp;
}): Promise<ObscuraHarness> {
  const command = options.command;
  if (!command.startsWith("/")) {
    throw Object.assign(new Error("Obscura command must be an absolute path"), { rpcCode: -32602 });
  }
  const connectCdp = options.connectCdp ?? connectObscuraCdp;
  const cdpPort = options.cdpPort ?? 9222;
  const serveArgs = ["serve", "--host", "127.0.0.1", "--port", String(cdpPort)];
  assertNoInsecureFlags(serveArgs);

  // 1) Serve process + Playwright-over-CDP attach (loopback only).
  const session: ObscuraCdpSession = await connectCdp({
    command,
    args: serveArgs,
    stderr: "pipe",
  });
  let mcp: Awaited<ReturnType<typeof createObscuraMcpTools>>;
  try {
    mcp = await createObscuraMcpTools({
      transport: { type: "stdio", command, args: ["mcp"] },
    });
  } catch (error) {
    // Never leak a half-started serve process.
    await session.close().catch(() => {});
    throw error;
  }
  try {
    // 3) Web tools: `web_search`/`web_fetch` (replaceable HTML search
    //    profile) plus native `obscura_fetch`/`obscura_scrape`. The CLI runs
    //    per call under the same owned binary; `allowEval` stays off —
    //    custom scrape expressions are deny-by-default.
    const web = createObscuraWebTools({
      command,
      ...(options.searchProfile ? { searchProfile: options.searchProfile } : {}),
    });
    // 4) Prism-owned browser tools on the attached browser.
    const browserTools = createBrowserTools({ browser: session.browser });

    let closed = false;
    return {
      tools: [...mcp.tools, ...web.tools, ...browserTools],
      close: async () => {
        if (closed) return;
        closed = true;
        await mcp.close().catch(() => {});
        await session.close().catch(() => {}); // browser first, then owned serve process
      },
    };
  } catch (error) {
    // Never leak a half-started harness: MCP child + serve process.
    await mcp.close().catch(() => {});
    await session.close().catch(() => {});
    throw error;
  }
}
