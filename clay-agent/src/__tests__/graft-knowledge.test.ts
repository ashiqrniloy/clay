import assert from "node:assert/strict";
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "bun:test";
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

async function codingHost(
  workspaceRoot: string,
  opts: { agentConfigRoot?: string; graftCliPath?: string } = {},
): Promise<ClayAgentHost> {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-graft-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    emit: () => {},
    // Hermetic agent config: knowledge gates must not depend on the
    // developer's real skills.json.
    agentConfigRoot: opts.agentConfigRoot ?? (await tempDir("clay-agent-graft-config-")),
    // Deterministic default-bind CLI (plan 117): tests pass a stub or a
    // nonexistent path so resolution never depends on a @nanonets/graft
    // peer being installed.
    graftCliPath: opts.graftCliPath,
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

async function writeGraftStub(root: string): Promise<string> {
  const { writeFile: writeFileRaw, chmod } = await import("node:fs/promises");
  const cliPath = join(root, "graft-stub.sh");
  await writeFileRaw(cliPath, "#!/usr/bin/env bash\nprintf '{}\n'\n", { mode: 0o755 });
  await chmod(cliPath, 0o755);
  return cliPath;
}

test("graft is available by default: first coding session binds pull mode", async () => {
  const root = await tempDir("clay-agent-graft-default-");
  const cliPath = await writeGraftStub(root);
  const host = await codingHost(root, { graftCliPath: cliPath });
  // The FIRST session.new already carries the graft binding: tools active,
  // skill in the catalog, extension visible in the environment.
  const sessionId = liveSessionId(host);
  const env = (await host.handle("environment.list", {})) as {
    extensions?: string[];
    skills?: Array<{ name: string }>;
  };
  assert(env.extensions?.some((name) => name.endsWith("/graft")), "graft extension bound by default");
  assert(env.skills?.some((skill) => skill.name === "graft"), "graft skill catalogued by default");
  const tools = kernelTools(host);
  for (const tool of GRAFT_TOOLS) assert(tools.get(tool), `${tool} registered by default bind`);
  // Second session for the SAME workspace: no re-bind (one attempt per
  // root, already-bound early return).
  const bindingBefore = (host as unknown as { graft?: { workspaceRoot: string } }).graft;
  await host.handle("session.new", { profile: "coding", provider: "mock", model: "demo", workspaceRoot: root });
  const bindingAfter = (host as unknown as { graft?: { workspaceRoot: string } }).graft;
  assert.equal(bindingAfter, bindingBefore, "no re-bind for the same workspace");
  // Explicit disable still wins over the default-on behavior.
  await host.handle("knowledge.setOptions", { workspaceRoot: root, graft: false });
  assert.equal(((host as unknown as { graft?: unknown }).graft), undefined, "disable unbinds");
  // A failed default attempt is not retried: re-enable explicitly, disable,
  // then a new workspace session — the old root stays attempted-once, but
  // the explicit RPC path can always rebind.
  await host.handle("knowledge.setOptions", { workspaceRoot: root, graft: true, graftCliPath: cliPath });
  assert(((host as unknown as { graft?: { workspaceRoot: string } }).graft), "explicit RPC re-binds");
  void sessionId;
  await rm(root, { recursive: true, force: true });
  host.close();
});

test("graft default bind fails closed without a resolvable CLI", async () => {
  const root = await tempDir("clay-agent-graft-absent-");
  // Deterministic unresolvable CLI: the default attempt fails on every
  // machine (no @nanonets/graft peer dependency in tests).
  const host = await codingHost(root, { graftCliPath: "/nonexistent/graft-cli" });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of GRAFT_TOOLS) assert(!created.tools.includes(tool), `no ${tool} when unresolvable`);

  // Explicit enable without any resolvable CLI: fail closed — the option
  // stays off, nothing loads.
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

test("agentSkills.graft=false blocks every binding path including explicit RPC", async () => {
  const { mkdir, writeFile } = await import("node:fs/promises");
  const root = await tempDir("clay-agent-graft-gated-");
  const cliPath = await writeGraftStub(root);
  const agentConfigRoot = await tempDir("clay-agent-graft-gated-config-");
  await mkdir(agentConfigRoot, { recursive: true });
  await writeFile(join(agentConfigRoot, "skills.json"), JSON.stringify({ agentSkills: { graft: false } }));
  const host = await codingHost(root, { agentConfigRoot, graftCliPath: cliPath });
  // Default bind: gate off ⇒ no attempt, no tools, no skill.
  const env = (await host.handle("environment.list", {})) as {
    extensions?: string[];
    skills?: Array<{ name: string }>;
  };
  assert(!env.extensions?.some((name) => name.endsWith("/graft")), "no default bind when gated off");
  assert(!env.skills?.some((skill) => skill.name === "graft"), "no graft skill when gated off");
  // Explicit RPC: also refused — the gate is authoritative on every path.
  const enabled = (await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
  })) as { graft: boolean };
  assert.equal(enabled.graft, false, "RPC graft:true is refused when gated off");
  assert.equal(((host as unknown as { graft?: unknown }).graft), undefined, "no binding materializes");
  await rm(root, { recursive: true, force: true });
  await rm(agentConfigRoot, { recursive: true, force: true });
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

// --- graftDeepModel (plan 121 follow-up): explicit `/graft-build-deep` model ---

async function recordingCli(root: string): Promise<{ cliPath: string; argv: () => Promise<string[]>; env: () => Promise<string[]> }> {
  const argvLog = join(root, "graft-argv.log");
  const envLog = join(root, "graft-env.log");
  const cliPath = join(root, "graft-stub.sh");
  await writeFile(
    cliPath,
    [
      "#!/usr/bin/env bash",
      `printf '%s ' "$@" >> "${argvLog}"`,
      `printf '\\n' >> "${argvLog}"`,
      `printf 'ENV %s|%s|%s|%s\\n' "$GRAFT_PROVIDER" "$GRAFT_MODEL" "$GRAFT_API_KEY" "$GRAFT_BASE_URL" >> "${envLog}"`,
      "printf '{}\\n'",
      "",
    ].join("\n"),
  );
  await chmod(cliPath, 0o755);
  const lines = async (path: string): Promise<string[]> =>
    (await readFile(path, "utf8").catch(() => ""))
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);
  return { cliPath, argv: () => lines(argvLog), env: () => lines(envLog) };
}

test("graftDeepModel resolves a vault credential and reaches the deep child env, never argv", async () => {
  const root = await tempDir("clay-agent-graft-deep-");
  const cli = await recordingCli(root);
  const host = await codingHost(root, { graftCliPath: cli.cliPath });
  const sessionId = liveSessionId(host);

  // Without a deep model the deep build refuses before spawning (unchanged).
  const refused = (await host.handle("command.dispatch", { name: "graft-build-deep", sessionId })) as {
    error?: { message?: string };
  };
  assert.match(refused.error?.message ?? "", /requires the host to configure deepModel/);
  assert.equal((await cli.argv()).length, 0, "no child spawned without a deep model");

  // The key lives in the vault (the same credential the picker stores); the
  // option carries only the model identity.
  await host.handle("credential.put", { provider: "anthropic", secret: "vault-key-121" });
  const enabled = (await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
    graftCliPath: cli.cliPath,
    graftDeepModel: { provider: "anthropic", model: "claude-deep", baseUrl: "https://api.example/v1" },
  })) as { graft?: boolean; graftDeepModel?: { provider?: string; model?: string } };
  assert.equal(enabled.graft, true);
  assert.equal(enabled.graftDeepModel?.provider, "anthropic");

  // The rebind reached the extension: the deep build now spawns with the
  // GRAFT_* child env (key from the vault) and no key on argv.
  const deep = (await host.handle("command.dispatch", { name: "graft-build-deep", sessionId })) as {
    error?: { message?: string };
    value?: { deep?: boolean };
  };
  assert.equal(deep.error, undefined, "deep build runs with the configured model");
  assert.equal(deep.value?.deep, true);
  assert.match((await cli.argv()).at(-1) ?? "", /^build --deep --provider anthropic --model claude-deep/, "deep build argv");
  assert.doesNotMatch((await cli.argv()).at(-1) ?? "", /vault-key-121/, "key never on argv");
  assert.equal((await cli.env()).at(-1), "ENV anthropic|claude-deep|vault-key-121|https://api.example/v1");

  // A repeat with the same deep model does not rebind (one binding).
  const binding = (host as unknown as { graft?: unknown }).graft;
  await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
    graftCliPath: cli.cliPath,
    graftDeepModel: { provider: "anthropic", model: "claude-deep", baseUrl: "https://api.example/v1" },
  });
  assert.equal((host as unknown as { graft?: unknown }).graft, binding, "identical deep model keeps the binding");

  // A changed model rebinds (the extension resolves its child env at load).
  await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
    graftCliPath: cli.cliPath,
    graftDeepModel: { provider: "anthropic", model: "claude-deep-2" },
  });
  assert.notEqual((host as unknown as { graft?: unknown }).graft, binding, "a changed deep model rebinds");
  await host.handle("command.dispatch", { name: "graft-build-deep", sessionId });
  assert.equal((await cli.env()).at(-1), "ENV anthropic|claude-deep-2|vault-key-121|", "rebind carries the new model");

  await rm(root, { recursive: true, force: true });
  host.close();
});

test("graftDeepModel accepts an inline key for keyless providers and fails closed otherwise", async () => {
  const root = await tempDir("clay-agent-graft-deep-inline-");
  const cli = await recordingCli(root);
  const host = await codingHost(root, { graftCliPath: cli.cliPath });
  const sessionId = liveSessionId(host);

  // litellm/orcarouter have no stored credential: an inline key is the only
  // way in, and it is remembered for redaction.
  await host.handle("knowledge.setOptions", {
    workspaceRoot: root,
    graft: true,
    graftCliPath: cli.cliPath,
    graftDeepModel: { provider: "litellm", model: "router/gpt", apiKey: "inline-key-121" },
  });
  await host.handle("command.dispatch", { name: "graft-build-deep", sessionId });
  assert.equal((await cli.env()).at(-1), "ENV litellm|router/gpt|inline-key-121|");

  // A model with no key anywhere fails closed: no half-configured deep build.
  await assert.rejects(
    () =>
      host.handle("knowledge.setOptions", {
        workspaceRoot: root,
        graft: true,
        graftCliPath: cli.cliPath,
        graftDeepModel: { provider: "orcarouter", model: "some-model" },
      }),
    /requires an apiKey or a stored `orcarouter` credential/,
  );
  // Contradictions and malformed shapes fail closed before anything binds.
  await assert.rejects(
    () =>
      host.handle("knowledge.setOptions", {
        workspaceRoot: root,
        graft: false,
        graftDeepModel: { provider: "openai", model: "gpt-x" },
      }),
    /requires `graft: true`/,
  );
  await assert.rejects(
    () =>
      host.handle("knowledge.setOptions", {
        workspaceRoot: root,
        graft: true,
        graftDeepModel: { provider: "not-a-provider", model: "m" },
      }),
    /provider must be one of openai\|anthropic\|litellm\|orcarouter/,
  );
  await assert.rejects(
    () =>
      host.handle("knowledge.setOptions", {
        workspaceRoot: root,
        graft: true,
        graftDeepModel: { provider: "openai", model: "" },
      }),
    /params\.model is required/,
  );

  await rm(root, { recursive: true, force: true });
  host.close();
});

// --- /graft-init + /graft-build-deep (plan 121): non-interactive init, deep fail-closed ---

test("graft init is non-interactive and deep build fails closed before spawn", async () => {
  const root = await tempDir("clay-agent-graft-init-");
  const argvLog = join(root, "graft-argv.log");
  const cliPath = join(root, "graft-stub.sh");
  // Recording stub: every spawn appends its argv as one line, then answers {}
  // (the pull tools' JSON envelope). The log is the spawn witness for the
  // fail-closed deep-build case below.
  await writeFile(
    cliPath,
    [
      "#!/usr/bin/env bash",
      `printf '%s ' "$@" >> "${argvLog}"`,
      `printf '\\n' >> "${argvLog}"`,
      "printf '{}\\n'",
      "",
    ].join("\n"),
  );
  await chmod(cliPath, 0o755);
  const host = await codingHost(root, { graftCliPath: cliPath });
  const sessionId = liveSessionId(host);
  const spawns = async (): Promise<string[]> => {
    const text = await readFile(argvLog, "utf8").catch(() => "");
    return text.split("\n").map((line) => line.trim()).filter(Boolean);
  };

  // /graft-build-deep with no host deepModel: Prism refuses before spawning
  // (no hidden paid LLM call), and the command result carries the error.
  const beforeDeep = (await spawns()).length;
  const deep = (await host.handle("command.dispatch", {
    name: "graft-build-deep",
    sessionId,
  })) as { error?: { message?: string }; value?: { deep?: boolean } };
  assert.match(deep.error?.message ?? "", /requires the host to configure deepModel/);
  assert.equal(deep.value, undefined, "no deep build ran");
  assert.equal((await spawns()).length, beforeDeep, "no child spawned for an unconfigured deep build");

  // /graft-init: host fixed argv — non-interactive, no user-level state, and
  // MCP/hook/statusline wiring stays off (Prism provides its own surfaces).
  const init = (await host.handle("command.dispatch", {
    name: "graft-init",
    sessionId,
  })) as { value?: { yes?: boolean; agents?: string[] } };
  assert.equal(init.value?.yes, true, "initYes: true reaches the extension");
  const initArgv = (await spawns()).at(-1) ?? "";
  assert.match(initArgv, /^init /);
  assert.match(initArgv, /--yes/);
  assert.match(initArgv, /--no-global/);
  assert.match(initArgv, /--no-mcp/);
  assert.match(initArgv, /--no-hooks/);
  assert.match(initArgv, /--no-statusline/);
  assert.doesNotMatch(initArgv, /GRAFT_API_KEY|--api-key/, "no secret or key flag on argv");

  // The plain /graft-build still spawns (user-triggered, non-deep).
  await host.handle("command.dispatch", { name: "graft-build", sessionId });
  assert.equal((await spawns()).at(-1), "build", "non-deep build spawns `build`");

  // The delivered skill body names both commands (the seeded SKILL.md is
  // rendered from this same in-code content on a fresh agent config).
  const skill = (
    host as unknown as { skills: { get(name: string): { instructions?: string } | undefined } }
  ).skills.get("graft");
  assert.match(skill?.instructions ?? "", /\/graft-init/);
  assert.match(skill?.instructions ?? "", /\/graft-build-deep/);

  await rm(root, { recursive: true, force: true });
  host.close();
});
