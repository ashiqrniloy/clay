#!/usr/bin/env node
import { stderr, stdin, stdout } from "node:process";
import { ClayAgentHost } from "./host.js";
import { redactText } from "./redact.js";
import { encodeFrame, FrameTooLargeError, parseFrame, readNdjsonConcurrent } from "./rpc.js";

const MIN_NODE = 20;

function nodeMajor(): number {
  const major = Number(process.versions.node.split(".")[0]);
  return Number.isFinite(major) ? major : 0;
}

function fail(message: string, code = 1): never {
  stderr.write(`${message}\n`);
  process.exit(code);
}

function parseArgs(argv: string[]): {
  dataDir: string;
  mock: boolean;
  agentConfigRoot?: string;
} {
  let dataDir: string | undefined;
  let agentConfigRoot: string | undefined;
  let mock = false;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--data-dir") {
      dataDir = argv[i + 1];
      i += 1;
    } else if (arg.startsWith("--data-dir=")) {
      dataDir = arg.slice("--data-dir=".length);
    } else if (arg === "--agent-config-root") {
      agentConfigRoot = argv[i + 1];
      i += 1;
    } else if (arg.startsWith("--agent-config-root=")) {
      agentConfigRoot = arg.slice("--agent-config-root=".length);
    } else if (arg === "--mock") {
      mock = true;
    } else if (arg === "--help" || arg === "-h") {
      stdout.write(
        "Usage: clay-agent --data-dir DIR [--agent-config-root DIR] [--mock]\n",
      );
      process.exit(0);
    }
  }
  if (!dataDir) fail("clay-agent requires --data-dir");
  return { dataDir, mock, agentConfigRoot };
}

function write(value: unknown): void {
  stdout.write(encodeFrame(value));
}

async function main(): Promise<void> {
  if (nodeMajor() < MIN_NODE) fail(`clay-agent requires Node >= ${MIN_NODE} (found ${process.versions.node})`);
  const args = parseArgs(process.argv.slice(2));
  let host: ClayAgentHost | undefined;
  const secrets = new Set<string>();

  const emit = (method: string, params: unknown): void => {
    if (method === "reverse") {
      // params is already a complete JSON-RPC frame (daemon-initiated request).
      write(params);
      return;
    }
    try {
      write({ jsonrpc: "2.0", method, params });
    } catch (error) {
      if (!(error instanceof FrameTooLargeError)) throw error;
      // One oversized event (e.g. a multi-megabyte tool result) must never
      // kill the run: degrade to an error event the server can surface.
      const sessionId = (params as { sessionId?: string } | undefined)?.sessionId ?? "";
      write({
        jsonrpc: "2.0",
        method,
        params: {
          sessionId,
          event: {
            type: "error",
            message:
              "Clay dropped an oversized agent event (JSON-RPC frame cap); the run continued without it. Narrow the tool output (e.g. repo_search path/outputMode) and retry.",
          },
        },
      });
    }
  };

  const handleLine = async (line: string): Promise<void> => {
    let parsed;
    try {
      parsed = parseFrame(line);
    } catch (error) {
      if (error instanceof FrameTooLargeError) {
        write({ jsonrpc: "2.0", id: null, error: { code: -32600, message: error.message } });
        return;
      }
      write({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "parse error" } });
      return;
    }
    const id = parsed.id ?? null;
    const method = typeof parsed.method === "string" ? parsed.method : "";
    try {
      if (!method && id !== null && id !== undefined) {
        // Response to a daemon-initiated reverse request.
        host?.resolveReverse(id, parsed as { result?: unknown; error?: { code?: number; message?: string } });
        return;
      }
      if (method === "initialize") {
        if (host) throw Object.assign(new Error("already initialized"), { rpcCode: -32000 });
        const params = parsed.params && typeof parsed.params === "object" ? (parsed.params as Record<string, unknown>) : {};
        const passphrase = typeof params.passphrase === "string" ? params.passphrase : "";
        if (!passphrase) throw Object.assign(new Error("initialize.passphrase is required"), { rpcCode: -32602 });
        secrets.add(passphrase);
        // Server-built MCP allow-list (validated fail-closed in host.ts).
        const mcpAllowList = Array.isArray(params.mcpAllowList) ? params.mcpAllowList : [];
        try {
          host = await ClayAgentHost.create({
            dataDir: args.dataDir,
            agentConfigRoot: args.agentConfigRoot,
            passphrase,
            mock: args.mock,
            emit,
            mcpAllowList,
          });
        } catch (error) {
          stderr.write(`${redactText(error instanceof Error ? error.message : String(error), secrets)}\n`);
          write({
            jsonrpc: "2.0",
            id,
            error: { code: -32000, message: "unreadable credential vault" },
          });
          process.exit(1);
        }
        write({ jsonrpc: "2.0", id, result: { ok: true, mock: args.mock, prism: "0.5.5", mcpServers: mcpAllowList.length } });
        return;
      }
      if (method === "shutdown") {
        host?.close(); // kills owned MCP/Obscura children before exit
        write({ jsonrpc: "2.0", id, result: { ok: true } });
        process.exit(0);
      }
      if (!host) throw Object.assign(new Error("initialize required"), { rpcCode: -32000 });
      const result = await host.handle(method, parsed.params ?? {});
      write({ jsonrpc: "2.0", id, result });
    } catch (error) {
      const mapped = host?.redactError(error) ?? {
        code: -32000,
        message: redactText(error instanceof Error ? error.message : String(error), secrets),
      };
      write({ jsonrpc: "2.0", id, error: mapped });
    }
  };

  process.on("SIGTERM", () => {
    host?.close();
    process.exit(0);
  });
  process.on("SIGINT", () => {
    host?.close();
    process.exit(0);
  });

  try {
    await readNdjsonConcurrent(stdin, handleLine);
  } catch (error) {
    if (error instanceof FrameTooLargeError) {
      write({ jsonrpc: "2.0", id: null, error: { code: -32600, message: error.message } });
      process.exit(1);
    }
    throw error;
  }
}

main().catch((error: unknown) => {
  fail(redactText(error instanceof Error ? error.message : String(error)));
});
