import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { access, chmod, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
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

async function codingHost(
  workspaceRoot: string,
  agentConfigRoot?: string,
  resolveObscura?: () => string | undefined,
): Promise<ClayAgentHost> {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    emit: () => {},
    // Hermetic agent config: the wiki/graft gates must not depend on the
    // developer's real ~/.clay/agents/coding-agent/skills.json.
    agentConfigRoot: agentConfigRoot ?? (await tempDir()),
    ...(resolveObscura ? { resolveObscuraBinary: resolveObscura } : {}),
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

const WIKI_TOOLS = ["wiki_search", "wiki_read_page", "wiki_record_insight", "wiki_ingest"];

interface WikiToolResult {
  value?: {
    id?: string;
    rawDir?: string;
    sourcePath?: string;
    extractPath?: string;
    url?: string;
    runStarted?: boolean;
  };
  metadata?: Record<string, unknown>;
}

function wikiTool(host: ClayAgentHost, name: string): { execute(args: unknown, context: unknown): Promise<WikiToolResult> } {
  return (
    host as unknown as {
      kernel: { registries: { tools: { resolve(name: string): { execute(args: unknown, context: unknown): Promise<WikiToolResult> } } } };
    }
  ).kernel.registries.tools.resolve(name);
}

/** Fake Obscura CLI: marks only `fetch` runs, then dumps markdown. */
async function fakeObscura(dir: string): Promise<{ path: string; marker: string }> {
  const path = join(dir, "obscura");
  const marker = join(dir, "obscura-ran");
  await writeFile(
    path,
    `#!/bin/sh\nif [ "$1" = "fetch" ]; then : > "${marker}"; printf '# Fetched RFC body\\n\\nExternal widgets claim.\\n'; exit 0; fi\nexit 1\n`,
    "utf8",
  );
  await chmod(path, 0o755);
  return { path, marker };
}

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
  assert.equal(init.value?.wikiRoot, join(root, ".wiki"));
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
  assert.equal(init.value?.wikiRoot, join(root, ".wiki"));
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

// --- Ingest (plan 121): staged raw layer, untrusted labeling, no fetch of its own ---

test("wiki ingest stages text under the raw layer and /wiki-ingest files it via drivers", async () => {
  const root = await tempDir();
  await writeFile(join(root, "README.md"), "# Widget\n\nbuildWidget exists.\n");
  // No Obscura: text/path ingest must work regardless (url is covered below).
  const host = await codingHost(root, undefined, () => undefined);
  const sessionId = (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;
  await host.handle("knowledge.setOptions", { workspaceRoot: root, wiki: true });
  const ingest = wikiTool(host, "wiki_ingest");

  // Staging works before the wiki is scaffolded; log.md gets an entry only
  // when the wiki root exists, so no .wiki/ is created as a side effect.
  const before = await ingest.execute({ text: "External: widgets are assembled from panels.", title: "Widget Notes" }, {});
  assert.equal(before.metadata?.trust, "untrusted_external", "staged sources are labeled untrusted");
  // Prism 0.7 reports workspace-relative posix paths for the raw layer.
  assert.ok(
    before.value?.rawDir?.startsWith("raw/ingest/"),
    `raw layer under the workspace (${before.value?.rawDir})`,
  );
  assert.match(await readFile(resolve(root, before.value!.extractPath!), "utf8"), /widgets are assembled from panels/);
  assert.ok((await readdir(resolve(root, before.value!.rawDir!))).includes("extract.md"), "extract staged next to the immutable original");
  assert.equal(existsSync(join(root, ".wiki")), false, "ingest alone never scaffolds the wiki");

  // With the wiki root present, the Ingested log entry lands.
  await host.handle("session.prompt", { sessionId, text: "/wiki-init" });
  await ingest.execute({ text: "External: panels render widget state.", title: "Panel Notes" }, {});
  assert.match(await readFile(join(root, ".wiki", "log.md"), "utf8"), /Ingested/);

  // /wiki-ingest is an extension-registered command; dispatching it through
  // a live session's drivers hands the brief to the wiki-maintainer skill.
  const dispatched = (await host.handle("command.dispatch", {
    name: "wiki-ingest",
    sessionId,
    args: { text: "External: the panel registry is keyed by widget id.", title: "Registry Notes" },
  })) as WikiToolResult;
  assert.equal(dispatched.value?.runStarted, true, "filing run started via drivers");
  await access(resolve(root, dispatched.value!.rawDir!, "extract.md"));

  // No live session ⇒ no drivers ⇒ the command stays stage-only.
  const stagedOnly = (await host.handle("command.dispatch", {
    name: "wiki-ingest",
    args: { text: "External: stage-only source." },
  })) as WikiToolResult;
  assert.equal(stagedOnly.value?.runStarted, false);

  await rm(root, { recursive: true, force: true });
  host.close();
});

test("wiki ingest fails closed: path escapes and private hosts never spawn the CLI", async () => {
  const root = await tempDir();
  const outside = await tempDir();
  await writeFile(join(outside, "secret.md"), "private\n");
  const obscura = await fakeObscura(await tempDir());
  const host = await codingHost(root, undefined, () => obscura.path);
  const sessionId = (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;
  await host.handle("knowledge.setOptions", { workspaceRoot: root, wiki: true });
  const ingest = wikiTool(host, "wiki_ingest");

  // `path` containment: realpath must stay inside the workspace root.
  await assert.rejects(
    () => ingest.execute({ path: join("..", basename(outside), "secret.md") }, {}),
    /escapes the workspace/,
  );

  // Private host: Prism's SSRF gate rejects before the host hook, so the
  // Obscura CLI never spawns (no marker).
  await assert.rejects(() => ingest.execute({ url: "http://127.0.0.1/private.md" }, {}), /ingest url rejected/);
  assert.equal(existsSync(obscura.marker), false, "no CLI spawn for a blocked host");

  // Allowed URL: the hook runs Obscura and stages its markdown dump as source.md.
  const fetched = await ingest.execute({ url: "https://example.com/rfc", title: "RFC" }, {});
  assert.equal(fetched.value?.url, "https://example.com/rfc");
  assert.equal(basename(fetched.value?.sourcePath ?? ""), "source.md", "markdown dump, not a binary source");
  assert.match(await readFile(resolve(root, fetched.value!.extractPath!), "utf8"), /Fetched RFC body/);
  assert.equal(existsSync(obscura.marker), true, "CLI spawned exactly on the allowed URL");

  await rm(root, { recursive: true, force: true });
  await rm(outside, { recursive: true, force: true });
  host.close();
});

test("wiki ingest url without Obscura fails closed while text still stages", async () => {
  const root = await tempDir();
  const host = await codingHost(root, undefined, () => undefined);
  await host.handle("knowledge.setOptions", { workspaceRoot: root, wiki: true });
  const ingest = wikiTool(host, "wiki_ingest");
  await assert.rejects(
    () => ingest.execute({ url: "https://example.com/nohook" }, {}),
    /requires a fetchUrl host hook/,
  );
  const staged = await ingest.execute({ text: "No hook needed for inline text." }, {});
  assert.ok(staged.value?.extractPath, "text ingest is unaffected by an absent Obscura");
  await rm(root, { recursive: true, force: true });
  host.close();
});
