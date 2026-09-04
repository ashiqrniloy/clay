import assert from "node:assert/strict";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(prefix: string): Promise<string> {
  return mkdtemp(join(tmpdir(), prefix));
}

function textProvider(): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  return {
    id: "mock",
    async *generate() {
      yield providerTextDelta("ok");
      yield providerDone();
    },
  };
}

async function codingHost(workspaceRoot: string): Promise<ClayAgentHost> {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-graft-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    emit: () => {},
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "Code.",
    tools: ["read", "write", "edit", "shell"],
  });
  await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot,
  });
  return host;
}

function liveSessionId(host: ClayAgentHost): string {
  return (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;
}

function kernelTools(host: ClayAgentHost): {
  resolve(name: string): { execute(args: unknown, context: unknown): Promise<unknown> };
  get(name: string): unknown;
} {
  return (
    host as unknown as { kernel: { registries: { tools: { resolve(n: string): never; get(n: string): never } } } }
  ).kernel.registries.tools;
}

const GRAFT_TOOLS = [
  "graft_ask",
  "graft_grep",
  "graft_callers",
  "graft_skeleton",
  "graft_map",
  "graft_blast",
];

test("graft option is opt-in: absent CLI fails closed and leaves the agent unperturbed", async () => {
  const root = await tempDir("clay-agent-graft-absent-");
  const host = await codingHost(root);
  // No graft tools on a default coding session.
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of GRAFT_TOOLS) assert(!created.tools.includes(tool), `no ${tool} when disabled`);

  // Enable with no resolvable CLI (no graftCliPath, no host packageRoot, no
  // @nanonets/graft peer): fail closed — the option stays off, nothing loads.
  const enabled = (await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
  })) as { workspaceRoot: string; graft: boolean };
  assert.equal(enabled.graft, false, "absent CLI resolves fail-closed");

  const tools = kernelTools(host);
  for (const tool of GRAFT_TOOLS) assert.equal(tools.get(tool), undefined, `${tool} hidden without CLI`);
  // Agent unperturbed: no graft skill in the catalog, /graft stays chat-safe.
  const skills = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(!skills.skills.some((skill) => skill.name === "graft"));
  const result = (await host.handle("session.prompt", { sessionId: liveSessionId(host), text: "/graft" })) as {
    lastEvent?: string;
  };
  assert.equal(result.lastEvent, "agent_finished");
  await rm(root, { recursive: true, force: true });
  host.close();
});

test("resolved graft CLI registers pull tools that answer with ranked spans; /graft dispatches", async () => {
  const root = await tempDir("clay-agent-graft-live-");
  const host = await codingHost(root);

  // Stub graft CLI: host-owned path, echoes a ranked-span payload for every
  // subcommand (the tools wrap `graft <cmd> … --json` verbatim).
  const cliPath = join(root, "graft-stub.sh");
  await writeFile(
    cliPath,
    [
      "#!/usr/bin/env bash",
      'printf \'{"nodes":[{"id":"n1","title":"Session search","path":"src/agent.rs","lines":"1008-1040","summary":"workspace-scoped FTS over session entries"}]}\'',
      "",
    ].join("\n"),
  );
  await chmod(cliPath, 0o755);

  const enabled = (await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
    graftCliPath: cliPath,
  })) as { workspaceRoot: string; graft: boolean };
  assert.equal(enabled.graft, true, "explicit cliPath resolves");

  // New coding sessions in the enabled workspace carry the graft tools.
  const withTools = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of GRAFT_TOOLS) assert(withTools.tools.includes(tool), `${tool} active when enabled`);

  // graft_ask returns the ranked spans the graph CLI answered with.
  const tools = kernelTools(host);
  const ask = tools.resolve("graft_ask");
  const asked = (await ask.execute({ query: "session search FTS" }, {})) as {
    value?: { nodes?: Array<{ path?: string; lines?: string }> };
  };
  assert.equal(asked.value?.nodes?.[0]?.path, "src/agent.rs");
  assert.equal(asked.value?.nodes?.[0]?.lines, "1008-1040");

  // graft_callers is the exact-edge lookup: same ranked-span envelope.
  const callers = tools.resolve("graft_callers");
  const edges = (await callers.execute({ symbol: "searchSessions" }, {})) as {
    value?: { nodes?: unknown[] };
  };
  assert.ok(Array.isArray(edges.value?.nodes), "graft_callers answers from the graph");

  // /graft reaches the kernel command via the bare-name slash fallback.
  const status = (await host.handle("session.prompt", {
    sessionId: liveSessionId(host),
    text: "/graft",
  })) as { name?: string };
  assert.equal(status.name, "graft", "/graft dispatches the kernel command");

  // Disable: zero residue — commands gone, tools unresolvable, catalog clean.
  await host.handle("knowledge.setOptions", { workspaceRoot: root, graft: false });
  for (const tool of GRAFT_TOOLS) assert.equal(tools.get(tool), undefined, `${tool} unresolvable after disable`);
  const promptAfter = (await host.handle("session.prompt", { sessionId: liveSessionId(host), text: "/graft" })) as {
    lastEvent?: string;
  };
  assert.equal(promptAfter.lastEvent, "agent_finished", "disabled /graft is a chat-safe prompt");
  const skillsAfter = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(!skillsAfter.skills.some((skill) => skill.name === "graft"));
  await rm(root, { recursive: true, force: true });
  host.close();
});

test("graft re-enable rebinds workspaces and wiki/graft flags compose in one call", async () => {
  const rootA = await tempDir("clay-agent-graft-rebind-a-");
  const rootB = await tempDir("clay-agent-graft-rebind-b-");
  const host = await codingHost(rootA);
  const cliPath = join(rootA, "graft-stub.sh");
  await writeFile(
    cliPath,
    "#!/usr/bin/env bash\nprintf '{\"nodes\":[]}'\n",
  );
  await chmod(cliPath, 0o755);
  await host.handle("knowledge.setOptions", { workspaceRoot: rootA, graft: true, graftCliPath: cliPath });
  // One call, both capabilities: wiki compiles the OKF bundle while graft
  // stays bound (the qmd-less catalog fallback serves wiki_search).
  await host.handle("knowledge.setOptions", { workspaceRoot: rootB, wiki: true, graft: true, graftCliPath: cliPath });
  const state = host as unknown as {
    wiki?: { workspaceRoot: string };
    graft?: { workspaceRoot: string };
  };
  assert.equal(state.graft?.workspaceRoot, rootB, "graft binding follows the newest workspace");
  assert.equal(state.wiki?.workspaceRoot, rootB, "wiki binding follows the newest workspace");
  // Disable graft only; the wiki stays bound.
  await host.handle("knowledge.setOptions", { workspaceRoot: rootB, graft: false });
  assert.equal((host as unknown as { graft?: unknown }).graft, undefined);
  assert.equal((host as unknown as { wiki?: { workspaceRoot: string } }).wiki?.workspaceRoot, rootB);
  await rm(rootA, { recursive: true, force: true });
  await rm(rootB, { recursive: true, force: true });
  host.close();
});
