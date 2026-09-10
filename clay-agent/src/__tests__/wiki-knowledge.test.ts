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

async function codingHost(workspaceRoot: string, agentConfigRoot?: string): Promise<ClayAgentHost> {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    emit: () => {},
    // Hermetic agent config: the wiki/graft gates must not depend on the
    // developer's real ~/.clay/agents/coding-agent/skills.json.
    agentConfigRoot: agentConfigRoot ?? (await tempDir()),
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

test("wiki-init is the sole initiator: gate on enables + dispatches; gate off stays chat-safe", async () => {
  const root = await tempDir();
  const { writeFile, mkdir } = await import("node:fs/promises");
  await writeFile(
    join(root, "README.md"),
    "# Widget\n\nThe widget module exports a buildWidget factory used by every panel.\n",
  );
  // Raw source present, NO knowledge.setOptions call: /wiki-init itself is
  // the initiator (plan 117).
  const host = await codingHost(root);
  const sessionId = (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;

  const init = (await host.handle("session.prompt", { sessionId, text: "/wiki-init" })) as {
    value?: { status?: string; wikiRoot?: string };
  };
  assert.equal(init.value?.status, "initialized");
  assert.equal(init.value?.wikiRoot, ".wiki");
  // Writes stay inside .wiki/: no skill files deployed into the workspace.
  await access(join(root, ".wiki", "index.md"));
  const workspaceFiles = await readdir(root);
  assert(!workspaceFiles.includes(".agents"), "no skill deployment outside .wiki/");
  // Wiki skills activated with init (registry-at-enable).
  const skills = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(skills.skills.some((skill) => skill.name === "wiki-searcher"), "wiki-searcher registers at init");
  assert(skills.skills.some((skill) => skill.name === "wiki-maintainer"), "wiki-maintainer registers at init");
  // New sessions in the workspace carry the wiki tools.
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of WIKI_TOOLS) assert(created.tools.includes(tool), `${tool} active after init`);
  // Second /wiki-init is idempotent: same binding, no re-bind.
  const bindingBefore = (host as unknown as { wiki?: { workspaceRoot: string } }).wiki;
  const again = (await host.handle("session.prompt", { sessionId, text: "/wiki-init" })) as {
    value?: { status?: string };
  };
  assert.equal(again.value?.status, "initialized");
  const bindingAfter = (host as unknown as { wiki?: { workspaceRoot: string } }).wiki;
  assert.equal(bindingAfter, bindingBefore, "no re-bind on repeated /wiki-init");
  await rm(root, { recursive: true, force: true });
  host.close();

  // Gate off: skills.json agentSkills.wikiSearcher=false keeps /wiki-init a
  // chat-safe prompt with zero residue.
  const gatedRoot = await tempDir();
  const gatedConfig = await tempDir();
  await mkdir(join(gatedConfig, "skills"), { recursive: true });
  await writeFile(join(gatedConfig, "skills.json"), JSON.stringify({ agentSkills: { wikiSearcher: false } }));
  const gatedHost = await codingHost(gatedRoot, gatedConfig);
  const gatedSession = (gatedHost as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;
  const gated = (await gatedHost.handle("session.prompt", {
    sessionId: gatedSession,
    text: "/wiki-init",
  })) as { lastEvent?: string };
  assert.equal(gated.lastEvent, "agent_finished", "gated-off /wiki-init is a chat-safe prompt");
  const gatedSkills = (await gatedHost.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(!gatedSkills.skills.some((skill) => skill.name.startsWith("wiki-")), "no wiki skills when gated off");
  await rm(gatedRoot, { recursive: true, force: true });
  await rm(gatedConfig, { recursive: true, force: true });
  gatedHost.close();
});

test("wiki-maintainer gate filters only its own skill at init", async () => {
  const root = await tempDir();
  const { writeFile } = await import("node:fs/promises");
  await writeFile(join(root, "README.md"), "# Widget\n\nbuildWidget exists.\n");
  const agentConfigRoot = await tempDir();
  await writeFile(
    join(agentConfigRoot, "skills.json"),
    JSON.stringify({ agentSkills: { wikiMaintainer: false } }),
  );
  const host = await codingHost(root, agentConfigRoot);
  const sessionId = (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;
  const init = (await host.handle("session.prompt", { sessionId, text: "/wiki-init" })) as {
    value?: { status?: string };
  };
  assert.equal(init.value?.status, "initialized");
  const skills = (await host.handle("skill.list", {})) as { skills: Array<{ name: string }> };
  assert(skills.skills.some((skill) => skill.name === "wiki-searcher"), "searcher unaffected by maintainer gate");
  assert(!skills.skills.some((skill) => skill.name === "wiki-maintainer"), "maintainer gated off never registers");
  await rm(root, { recursive: true, force: true });
  await rm(agentConfigRoot, { recursive: true, force: true });
  host.close();
});

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
  // Dormant without /wiki-init: nothing registers on its own (the sole
  // initiator is the slash command; gate-off behavior is covered above).
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
  // /wiki-init remains the sole initiator: re-running it after a disable
  // re-initializes the binding (init semantics, not a chat-safe prompt).
  await host.handle("knowledge.setOptions", { workspaceRoot: root, wiki: false });
  assert.throws(() => tools.resolve("wiki_search"));
  const reinit = (await host.handle("session.prompt", { sessionId, text: "/wiki-init" })) as {
    value?: { status?: string };
  };
  assert.equal(reinit.value?.status, "initialized", "/wiki-init re-initializes after a disable");
  await access(join(root, ".wiki", "index.md"));
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
