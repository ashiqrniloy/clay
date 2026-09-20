#!/usr/bin/env node
// Plan 130 agent-host decomposition — live daemon protocol drive (2026-09-20).
//
// Manual step A21 (test-plan/16-agent-host.md) / C43 (module 17) on a real
// build: two `clay-agent` processes over one data dir with different cwds,
// driven through the daemon's own ndjson JSON-RPC (no test harness, no mock
// host object), asserting the workspace binding of the second process:
//
//   * `session.new` records the workspace root it was given; a session created
//     WITHOUT a root keeps the documented `process.cwd()` fallback, so the
//     resume leg's root can only come from the record;
//   * a fresh daemon started in an unrelated cwd resumes that session with the
//     RECORDED root (never its own cwd);
//   * the resumed session re-activates the workspace's coding surfaces — graft
//     binding + its skill (the plan 130 A2 fix) — while MCP stays absent for an
//     empty allow-list and Obscura stays hidden without a binary;
//   * prompt / search / resumable scoping / fork / clone / autonomy / compact
//     all work on the real daemon after the decomposition.
//
// Usage: node live-daemon.mjs            (clay-agent/dist must be built)
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { createInterface } from "node:readline";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..", "..");
const root = "/tmp/clay-plan130-live";
const dataDir = join(root, "daemon-data");
const cwdA = join(root, "cwd-a");
const cwdB = join(root, "cwd-b");
const otherRoot = join(root, "other-workspace");
const passphrase = "plan130-live-passphrase";
const globalPrefix = join(process.execPath, "..", "..", "lib", "node_modules");
const mcpFixture = join(repo, "clay-agent", "src", "__tests__", "mcp-fixture-server.mjs");
const mcpAllowList = [{ serverId: "live", command: process.execPath, args: [mcpFixture] }];

const results = [];

function record(name, detail) {
  results.push({ name, result: "PASS", detail });
  console.log(`PASS  ${name}${detail ? ` — ${detail}` : ""}`);
}

function fail(name, detail) {
  results.push({ name, result: "FAIL", detail });
  console.log(`FAIL  ${name} — ${detail}`);
  process.exitCode = 1;
}

function check(name, body, detail) {
  try {
    body();
  } catch (error) {
    fail(name, error instanceof Error ? error.message : String(error));
    return false;
  }
  record(name, typeof detail === "function" ? detail() : detail);
  return true;
}

class Daemon {
  constructor(cwd) {
    this.cwd = cwd;
    this.nextId = 1;
    this.pending = new Map();
    this.events = [];
    this.reverse = [];
  }

  async start() {
    this.child = spawn(process.execPath, [join(repo, "clay-agent", "dist", "main.js"), "--data-dir", dataDir, "--mock"], {
      cwd: this.cwd,
      stdio: ["pipe", "pipe", "pipe"],
      env: {
        ...process.env,
        // Graft's optional peer lives in the host's global prefix here; the
        // documented resolution chain (cliPath → package root → peer) only
        // reaches it through the module graph, which NODE_PATH feeds.
        NODE_PATH: globalPrefix,
        CLAY_OBSCURA_BIN: "",
      },
    });
    this.child.stderr.on("data", (chunk) => process.stderr.write(`[daemon${this.cwd.endsWith("b") ? "-b" : "-a"}] ${chunk}`));
    createInterface({ input: this.child.stdout }).on("line", (line) => this.#line(line));
    this.ready = new Promise((resolve) => {
      this.child.once("spawn", resolve);
    });
    await this.ready;
    const init = await this.call("initialize", { passphrase, mcpAllowList });
    assert.equal(init.error, undefined, `initialize failed: ${JSON.stringify(init.error)}`);
    return init.result;
  }

  #line(line) {
    if (!line.trim()) return;
    const frame = JSON.parse(line);
    if (frame.method === "reverse") {
      // No document backend in this probe: answer with a typed error so the
      // daemon never waits on the 30 s reverse timeout (no leg here needs one).
      this.reverse.push(frame);
      this.write({ jsonrpc: "2.0", id: frame.id, error: { code: -32601, message: "no reverse backend in the live probe" } });
      return;
    }
    if (frame.method) {
      this.events.push(frame);
      return;
    }
    const pending = this.pending.get(frame.id);
    if (!pending) return;
    this.pending.delete(frame.id);
    pending(frame);
  }

  write(value) {
    this.child.stdin.write(`${JSON.stringify(value)}\n`);
  }

  call(method, params = {}) {
    const id = this.nextId++;
    const answer = new Promise((resolve) => this.pending.set(id, resolve));
    this.write({ jsonrpc: "2.0", id, method, params });
    return answer;
  }

  async ok(method, params = {}) {
    const frame = await this.call(method, params);
    assert.equal(frame.error, undefined, `${method} failed: ${JSON.stringify(frame.error)}`);
    return frame.result;
  }

  async stop() {
    try {
      await this.ok("shutdown");
    } catch {
      // already gone
    }
    this.child.stdin.end();
    await new Promise((resolve) => {
      this.child.once("exit", resolve);
      setTimeout(() => {
        this.child.kill("SIGKILL");
        resolve();
      }, 2000);
    });
  }
}

async function main() {
  await rm(root, { recursive: true, force: true });
  for (const dir of [root, dataDir, cwdA, cwdB, otherRoot]) await mkdir(dir, { recursive: true, mode: 0o700 });

  const first = new Daemon(cwdA);
  const init = await first.start();
  check("daemon initialize (real build, isolated data dir)", () => {
    assert.equal(init.mock, true);
    assert.equal(init.prism, "0.7.0");
    assert.equal(init.mcpServers, 1, "one allow-listed server");
  }, `mock=${init.mock} prism=${init.prism} mcpServers=${init.mcpServers}`);

  await first.ok("agentProfile.register", { name: "coding", instructions: "Live plan-130 probe.", tools: ["read", "write"] });

  const skillsBefore = await first.ok("skill.list", {});
  const namesBefore = JSON.stringify(skillsBefore);
  check("graft skill is not registered before any coding session", () => {
    assert.ok(!namesBefore.includes("graft"), `graft already listed: ${namesBefore.slice(0, 200)}`);
  }, "fresh daemon: no graft binding");

  // Session #1: explicit recorded root (the repo — a real graft workspace).
  const created = await first.ok("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: repo,
  });
  check("session.new records the given workspace root and activates coding surfaces", () => {
    assert.equal(created.workspaceRoot, repo);
    assert.ok(created.tools.includes("write"), `no write tool: ${created.tools}`);
    assert.ok(created.tools.includes("graft_ask"), `no graft tool: ${created.tools}`);
    assert.ok(created.tools.some((tool) => tool.startsWith("mcp:live:")), `no MCP tool: ${created.tools}`);
  }, `sessionId=${created.sessionId} tools=${created.tools.length}`);

  const resumedSkillsLive = await first.ok("skill.list", {});
  check("binding a coding session registers the graft skill", () => {
    assert.ok(JSON.stringify(resumedSkillsLive).includes("graft"), "graft skill missing after session.new");
  });

  const prompt = await first.ok("session.prompt", { sessionId: created.sessionId, text: "plan130 live probe alpha" });
  check("session.prompt streams to completion on the real daemon", () => {
    assert.equal(prompt.lastEvent, "agent_finished");
  }, `lastEvent=${prompt.lastEvent} events=${first.events.length}`);

  const hits = await first.ok("session.search", { sessionId: created.sessionId, query: "probe alpha" });
  const hitIds = (hits.hits ?? hits.results ?? []).map((hit) => hit.sessionId);
  check("session.search finds this workspace's session", () => {
    assert.ok(hitIds.includes(created.sessionId), `no hit for own session: ${JSON.stringify(hits).slice(0, 200)}`);
  }, `hits=${hitIds.length}`);

  const none = await first.ok("session.search", { sessionId: created.sessionId, query: "zzz-not-in-any-workspace" });
  check("session.search is a search, not a dump (no match ⇒ no hit)", () => {
    assert.equal((none.hits ?? none.results ?? []).length, 0);
  });

  const resumable = await first.ok("session.resumable", { workspaceRoot: repo });
  check("session.resumable lists the session for its recorded root", () => {
    assert.ok(resumable.sessions.some((row) => row.sessionId === created.sessionId));
    assert.ok(resumable.sessions.every((row) => typeof row.label === "string" || row.label === undefined));
  }, `rows=${resumable.sessions.length}`);

  const elsewhere = await first.ok("session.resumable", { workspaceRoot: otherRoot });
  check("session.resumable is workspace-scoped (other root sees nothing)", () => {
    assert.ok(!elsewhere.sessions.some((row) => row.sessionId === created.sessionId));
  }, `rows=${elsewhere.sessions.length}`);

  const cloned = await first.ok("session.clone", { sessionId: created.sessionId });
  check("session.clone produces an independent persisted session", () => {
    assert.notEqual(cloned.sessionId, created.sessionId);
  }, `clone=${cloned.sessionId}`);

  const forked = await first.ok("session.fork", { sessionId: created.sessionId });
  check("session.fork keeps the session id at a different leaf", () => {
    assert.equal(forked.sessionId, created.sessionId);
  }, `leaf=${forked.leafId.slice(0, 12)}…`);

  const autonomyOff = await first.ok("session.setAutonomy", { sessionId: created.sessionId, enabled: false });
  const autonomyOn = await first.ok("session.setAutonomy", { sessionId: created.sessionId, enabled: true });
  check("session.setAutonomy toggles both ways", () => {
    assert.equal(autonomyOff.fullAutonomy ?? autonomyOff.enabled, false);
    assert.equal(autonomyOn.fullAutonomy ?? autonomyOn.enabled, true);
  });

  const compacted = await first.ok("session.compact", { sessionId: created.sessionId });
  const loaded = await first.ok("session.load", { sessionId: created.sessionId });
  const kinds = (loaded.entries ?? loaded.entry ?? []).map((entry) => entry.kind);
  check("session.compact appends a compaction entry to the persisted session", () => {
    assert.equal(compacted.sessionId, created.sessionId);
    assert.ok(kinds.includes("compaction"), `no compaction entry: ${kinds}`);
  }, `entries=${kinds.length}`);

  // Session #2: no workspaceRoot ⇒ the documented cwd fallback. Its existence
  // is what makes the phase-2 root assertion meaningful.
  const cwdFallback = await first.ok("session.new", { profile: "coding", provider: "mock", model: "demo" });
  check("session.new without a root keeps the documented process.cwd() fallback", () => {
    assert.equal(cwdFallback.workspaceRoot, cwdA, `expected ${cwdA}, got ${cwdFallback.workspaceRoot}`);
  }, `root=${cwdFallback.workspaceRoot}`);

  await first.stop();

  const second = new Daemon(cwdB);
  const init2 = await second.start();
  await second.ok("agentProfile.register", { name: "coding", instructions: "Live plan-130 probe.", tools: ["read", "write"] });
  const restartSkills = await second.ok("skill.list", {});
  check("fresh daemon starts with no graft binding (restart really clears it)", () => {
    assert.ok(!JSON.stringify(restartSkills).includes("graft"));
  }, `mock=${init2.mock}`);

  const inventoryBefore = await second.ok("environment.list", {});
  check("fresh daemon starts with lazy capabilities (nothing connected yet)", () => {
    assert.deepEqual(inventoryBefore.mcpServers, []);
    assert.deepEqual(inventoryBefore.extensions, []);
  }, `mcpServers=${inventoryBefore.mcpServers.length} extensions=${inventoryBefore.extensions.length}`);

  const resumed = await second.ok("session.resume", { sessionId: created.sessionId });
  check("resumed session binds its RECORDED workspace root, not the daemon cwd", () => {
    assert.equal(resumed.workspaceRoot, repo, `recorded ${repo}, got ${resumed.workspaceRoot}`);
    assert.notEqual(resumed.workspaceRoot, cwdB);
  }, `root=${resumed.workspaceRoot} (daemon cwd ${cwdB})`);

  const afterSkills = await second.ok("skill.list", {});
  check("resume re-activates the workspace's graft binding (plan 130 A2)", () => {
    assert.ok(JSON.stringify(afterSkills).includes("graft"), "graft skill missing after resume");
  }, "graft skill registered by ensureLive");

  const inventory = await second.ok("environment.list", { sessionId: created.sessionId });
  check("resume re-connects the workspace's MCP allow-list (plan 130 A2)", () => {
    const server = (inventory.mcpServers ?? [])[0];
    assert.ok(server, `no MCP outcomes: ${JSON.stringify(inventory.mcpServers)}`);
    assert.equal(server.serverId, "live");
    assert.equal(server.connected, true, `not connected: ${JSON.stringify(server)}`);
    assert.ok(server.tools > 0);
  }, JSON.stringify(inventory.mcpServers));

  check("resume re-loads the graft extension for the recorded root", () => {
    assert.ok(
      (inventory.extensions ?? []).some((name) => String(name).includes("graft")),
      `extensions=${JSON.stringify(inventory.extensions)}`,
    );
  }, JSON.stringify(inventory.extensions));

  const context = await second.ok("session.context", { sessionId: created.sessionId });
  const systemItems = (context.categories ?? []).find((category) => category.kind === "systemPrompt");
  check("resumed session composes its system-prompt layers", () => {
    assert.ok(systemItems && systemItems.count > 0, JSON.stringify(context).slice(0, 200));
  }, `systemPrompt items=${systemItems?.count ?? 0}`);

  // Autonomy is restored from the session record on resume (decision
  // 2026-09-20-2049); the RPC reply does not carry it, so this leg proves the
  // toggle still works on a resumed session rather than the restored value.
  const resumedAutonomy = await second.ok("session.setAutonomy", { sessionId: created.sessionId, enabled: false });
  check("session.setAutonomy works on the resumed session", () => {
    assert.equal(resumedAutonomy.fullAutonomy ?? resumedAutonomy.enabled, false);
  }, "off; a resumed session keeps the autonomy recorded with it (see the automated resume case)");

  const continueRun = await second.ok("session.prompt", { sessionId: created.sessionId, text: "plan130 live probe resumed" });
  check("resumed session accepts a follow-up prompt on the restarted daemon", () => {
    assert.equal(continueRun.lastEvent, "agent_finished");
  }, `lastEvent=${continueRun.lastEvent}`);

  check("no reverse RPC was needed by the text-only mock provider", () => {
    assert.equal(first.reverse.length + second.reverse.length, 0);
  });

  await second.stop();

  await writeFile(
    join(here, "live-daemon.json"),
    `${JSON.stringify({ root, dataDir, cwdA, cwdB, workspaceRoot: repo, sessionId: created.sessionId, cloned, results }, null, 2)}\n`,
  );
  console.log(`\n${results.filter((entry) => entry.result === "PASS").length}/${results.length} live daemon legs passed`);
}

main().catch((error) => {
  fail("live daemon drive crashed", error instanceof Error ? error.stack ?? error.message : String(error));
  process.exit(1);
});
