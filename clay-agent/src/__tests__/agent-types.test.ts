import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "bun:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(prefix = "clay-agent-types-"): Promise<string> {
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

/**
 * One daemon whose default agent lives in `<data>/agents/coding-agent`, with
 * a second configured agent beside it — the layout the server's per-agent
 * roots resolve against (`~/.clay/agents/<type>`).
 */
async function hostWithAgents(): Promise<{
  host: ClayAgentHost;
  dataDir: string;
  root: string;
  reviewerRoot: string;
  workspace: string;
}> {
  const dataDir = await tempDir();
  const agents = join(dataDir, "agents");
  const root = join(agents, "coding-agent");
  const reviewerRoot = join(agents, "reviewer");
  await mkdir(reviewerRoot, { recursive: true });
  await writeFile(join(reviewerRoot, "SYSTEM.md"), "You are the reviewer.\n");
  await writeFile(
    join(reviewerRoot, "skills.json"),
    JSON.stringify({ roots: { workspace: { enabled: false } } }),
  );
  // A user-added skill only the reviewer root carries.
  await mkdir(join(reviewerRoot, "skills", "review-only"), { recursive: true });
  await writeFile(
    join(reviewerRoot, "skills", "review-only", "SKILL.md"),
    "---\nname: review-only\ndescription: Review helper\n---\n\nReview things.\n",
  );
  const workspace = await tempDir("clay-agent-types-ws-");
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    emit: () => {},
    agentConfigRoot: root,
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "Code.",
    tools: ["read", "write", "edit", "shell"],
  });
  return { host, dataDir, root, reviewerRoot, workspace };
}

test("session.new runs the named agent's config root", async () => {
  const { host, workspace, reviewerRoot } = await hostWithAgents();
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: workspace,
    agent: "reviewer",
  })) as { agent?: string; agentRoot?: string; sessionId: string };

  assert.equal(created.agent, "reviewer");
  assert.equal(created.agentRoot, reviewerRoot);
  // The reviewer's own skills.json disabled the workspace root, and its
  // config root contributes the skill the default agent does not have.
  const loaded = host as unknown as {
    live: Map<string, { agentRoot: string; agentType?: string }>;
  };
  assert.equal(loaded.live.get(created.sessionId)?.agentRoot, reviewerRoot);
  assert.equal(loaded.live.get(created.sessionId)?.agentType, "reviewer");
  host.close();
});

test("an unconfigured or hostile agent name is rejected, never the default", async () => {
  const { host, workspace } = await hostWithAgents();
  // An empty/absent name means the daemon's default agent (the server uses
  // `""` to switch a tab back), so only names that cannot resolve fail.
  for (const agent of ["missing", "../coding-agent", "a/b", "."]) {
    await assert.rejects(
      () =>
        host.handle("session.new", {
          profile: "coding",
          provider: "mock",
          model: "demo",
          workspaceRoot: workspace,
          agent,
        }),
      /not configured|invalid agent type/,
      `agent ${JSON.stringify(agent)} must fail closed`,
    );
  }
  await assert.rejects(
    () =>
      host.handle("session.new", {
        profile: "coding",
        provider: "mock",
        model: "demo",
        workspaceRoot: workspace,
        agent: 7,
      }),
    /agent must be a string/,
  );
  host.close();
});

test("session.setAgent switches in place and keeps the session", async () => {
  const { host, workspace, reviewerRoot } = await hostWithAgents();
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: workspace,
  })) as { sessionId: string };
  const sessionId = created.sessionId;

  const switched = (await host.handle("session.setAgent", {
    sessionId,
    agent: "reviewer",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string; agent?: string; agentRoot?: string; tools?: string[] };

  assert.equal(switched.sessionId, sessionId, "the session id survives the switch");
  assert.equal(switched.agent, "reviewer");
  assert.equal(switched.agentRoot, reviewerRoot);
  // The switch is recorded in the session record, so a resume keeps it.
  const loaded = (await host.handle("session.load", { sessionId })) as {
    metadata?: { agentType?: string };
  };
  assert.equal(loaded.metadata?.agentType, "reviewer");

  // The next run is the reviewer's: its SYSTEM.md layer is in the prompt.
  const live = host as unknown as {
    live: Map<string, { systemPrompt?: unknown; agentRoot: string }>;
  };
  assert.equal(live.live.get(sessionId)?.agentRoot, reviewerRoot);
  host.close();
});

test("switching back to the default agent clears the record's agent", async () => {
  const { host, workspace } = await hostWithAgents();
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: workspace,
    agent: "reviewer",
  })) as { sessionId: string };

  const back = (await host.handle("session.setAgent", {
    sessionId: created.sessionId,
    agent: "",
    provider: "mock",
    model: "demo",
  })) as { agent?: string };
  assert.equal(back.agent, undefined, "the default agent has no type name");
  const loaded = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    metadata?: { agentType?: string };
  };
  assert.equal(loaded.metadata?.agentType, undefined);
  host.close();
});

test("transcript entries carry the agent that produced them", async () => {
  const { host, workspace } = await hostWithAgents();
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: workspace,
  })) as { sessionId: string };
  const sessionId = created.sessionId;
  await host.handle("session.setAgent", {
    sessionId,
    agent: "reviewer",
    provider: "mock",
    model: "demo",
  });
  await host.handle("session.prompt", { sessionId, text: "review this" });

  const entries = (await(
    host as unknown as { persistence: { list: (id: string) => Promise<unknown[]> } }
  ).persistence.list(sessionId)) as Array<{ metadata?: { agentType?: string } }>;
  const stamped = entries.filter((entry) => entry.metadata?.agentType === "reviewer");
  assert(stamped.length > 0, "entries written after the switch are stamped `reviewer`");
  host.close();
});

test("a switch to an agent with a different config cannot inherit the old one", async () => {
  const { host, workspace, root } = await hostWithAgents();
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: workspace,
  })) as { sessionId: string; tools?: string[] };
  const before = new Set(created.tools ?? []);
  const switched = (await host.handle("session.setAgent", {
    sessionId: created.sessionId,
    agent: "reviewer",
    provider: "mock",
    model: "demo",
  })) as { tools?: string[] };
  const after = switched.tools ?? [];
  // The reviewer declares no MCP servers and inherits no bridge: its tools
  // are the profile's, never the previous agent's connected set.
  assert(after.every((tool) => before.has(tool) || !tool.startsWith("mcp")), String(after));

  // And back: the default agent root is restored, not left at the reviewer's.
  const live = host as unknown as {
    live: Map<string, { agentRoot: string }>;
    agentConfigRoot: string;
  };
  await host.handle("session.setAgent", {
    sessionId: created.sessionId,
    agent: "",
    provider: "mock",
    model: "demo",
  });
  assert.equal(live.live.get(created.sessionId)?.agentRoot, root);
  host.close();
});
