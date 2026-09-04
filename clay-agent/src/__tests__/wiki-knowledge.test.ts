import assert from "node:assert/strict";
import { access, mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-wiki-"));
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
    dataDir: await tempDir(),
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

const WIKI_TOOLS = ["wiki_search", "wiki_read_page", "wiki_record_insight"];

test("wiki option is opt-in: disabled by default leaves no residue", async () => {
  const root = await tempDir();
  const host = await codingHost(root);
  // No wiki tools on a default coding session.
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of WIKI_TOOLS) assert(!created.tools.includes(tool), `no ${tool} when disabled`);
  // No wiki skills in the catalog, no wiki commands registered.
  const skills = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(!skills.skills.some((skill) => skill.name.startsWith("wiki-")));
  // /wiki-init with the option disabled stays a chat-safe prompt (unknown
  // command), not a dispatch and not an error.
  const result = (await host.handle("session.prompt", {
    sessionId: (host as unknown as { live: Map<string, unknown> }).live.keys().next().value,
    text: "/wiki-init",
  })) as { lastEvent?: string };
  assert.equal(result.lastEvent, "agent_finished");
  const skillsAfter = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert.equal(skillsAfter.skills.length, skills.skills.length, "no residue after disabled prompt");
  await rm(root, { recursive: true, force: true });
  host.close();
});

test("wiki init/refresh/lint commands build an OKF bundle the search tool answers from", async () => {
  const root = await tempDir();
  // Raw source for the wiki to compile (codebase profile input).
  const { writeFile } = await import("node:fs/promises");
  await writeFile(
    join(root, "README.md"),
    "# Widget\n\nThe widget module exports a buildWidget factory used by every panel.\n",
  );
  const host = await codingHost(root);
  const sessionId = (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;

  // qmdPath points at a nonexistent binary: the exit-gate drill for "qmd
  // absent ⇒ catalog fallback works" (host-owned binary, deny by default).
  const enabled = (await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    wiki: true,
    qmdPath: "/nonexistent/qmd-for-drill",
  })) as {
    workspaceRoot: string;
    wiki: boolean;
  };
  assert.equal(enabled.wiki, true);

  // New coding sessions in the enabled workspace carry the wiki tools.
  const withTools = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of WIKI_TOOLS) assert(withTools.tools.includes(tool), `${tool} active when enabled`);

  // /wiki-init via the slash-command intercept (kernel-registered command);
  // dispatch results ride the { name, value } command envelope.
  const init = (await host.handle("session.prompt", { sessionId, text: "/wiki-init" })) as {
    value?: { status?: string; wikiRoot?: string };
  };
  assert.equal(init.value?.status, "initialized");
  assert.equal(init.value?.wikiRoot, ".wiki");
  // Writes stay inside .wiki/: no skill files deployed into the workspace.
  await access(join(root, ".wiki", "index.md"));
  const workspaceFiles = await readdir(root);
  assert(!workspaceFiles.includes(".agents"), "no skill deployment outside .wiki/");

  // /wiki-refresh is incremental (Merkle-diff) and idempotent on no changes.
  const refresh = (await host.handle("session.prompt", { sessionId, text: "/wiki-refresh" })) as {
    value?: { delta?: { unchanged?: unknown[] } };
  };
  assert.ok(refresh.value?.delta, "refresh reports a source delta");

  // /wiki-lint reports health.
  const lint = (await host.handle("session.prompt", { sessionId, text: "/wiki-lint" })) as {
    value?: { ok?: boolean };
  };
  assert.equal(typeof lint.value?.ok, "boolean");

  // The search tool answers from the compiled bundle: run the tool directly
  // through a second prompt asking the model (mock echoes) — instead assert
  // the tool resolves and executes via the kernel registry.
  const tools = (host as unknown as {
    kernel: { registries: { tools: { resolve(name: string): { execute(args: unknown, context: unknown): Promise<unknown> } } } };
  }).kernel.registries.tools;
  const search = tools.resolve("wiki_search");
  const searchResult = (await search.execute({ query: "widget module" }, {})) as {
    value?: { hits?: unknown[]; hitCount?: number };
  };
  assert.ok(
    (searchResult.value?.hitCount ?? 0) > 0 && Array.isArray(searchResult.value?.hits),
    "wiki_search answers from the OKF bundle via the catalog fallback",
  );

  // Disable: zero residue — commands gone, tools unresolvable, catalog clean.
  await host.handle("knowledge.setOptions", { workspaceRoot: root, wiki: false });
  assert.throws(() => tools.resolve("wiki_search"));
  const promptAfter = (await host.handle("session.prompt", { sessionId, text: "/wiki-init" })) as {
    lastEvent?: string;
  };
  assert.equal(promptAfter.lastEvent, "agent_finished", "disabled /wiki-init is a chat-safe prompt");
  const skillsAfter = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(!skillsAfter.skills.some((skill) => skill.name.startsWith("wiki-")));
  await rm(root, { recursive: true, force: true });
  host.close();
});

test("wiki re-enable is idempotent and rebinding switches workspaces", async () => {
  const rootA = await tempDir();
  const host = await codingHost(rootA);
  await host.handle("knowledge.setOptions", { workspaceRoot: rootA, wiki: true });
  await host.handle("knowledge.setOptions", { workspaceRoot: rootA, wiki: true });
  const rootB = await tempDir();
  await host.handle("knowledge.setOptions", { workspaceRoot: rootB, wiki: true });
  // The binding follows the newest workspace: session A's workspace no
  // longer gets wiki tools; B's does.
  const tools = (host as unknown as {
    wiki?: { workspaceRoot: string };
  }).wiki;
  assert.equal(tools?.workspaceRoot, rootB);
  await host.handle("knowledge.setOptions", { workspaceRoot: rootB, wiki: false });
  const skills = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(!skills.skills.some((skill) => skill.name.startsWith("wiki-")));
  await rm(rootA, { recursive: true, force: true });
  await rm(rootB, { recursive: true, force: true });
  host.close();
});
