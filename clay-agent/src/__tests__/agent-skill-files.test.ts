import assert from "node:assert/strict";
import { access, chmod, mkdir, mkdtemp, readFile, rm, stat, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(prefix = "clay-agent-skillfiles-"): Promise<string> {
  return mkdtemp(join(tmpdir(), prefix));
}

/** Graft skill seed body first line (asserts built-in fallback content). */
const GRAFT_SEED_DESCRIPTION =
  "Use the graft context graph to locate code by architecture, callers, and coupling before spelunking.";

function textProvider(): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  return {
    id: "mock",
    async *generate() {
      yield providerTextDelta("ok");
      yield providerDone();
    },
  };
}

async function codingHost(agentConfigRoot: string): Promise<ClayAgentHost> {
  // Graft default bind requires a resolvable CLI; the stub (prints an empty
  // graph) keeps the graft skill registration reachable without
  // @nanonets/graft.
  const stubDir = await tempDir("clay-agent-skillfiles-bin-");
  const graftCliPath = join(stubDir, "graft-stub.sh");
  await writeFile(graftCliPath, "#!/usr/bin/env bash\nprintf '{}\\n'\n", { mode: 0o755 });
  await chmod(graftCliPath, 0o755);
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-skillfiles-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    emit: () => {},
    agentConfigRoot,
    homeSkillsRoot: await tempDir("clay-agent-skillfiles-home-"),
    graftCliPath,
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
    workspaceRoot: await tempDir("clay-agent-skillfiles-ws-"),
  });
  return host;
}

test("host creation seeds all three agent-delivered SKILL.md files", async () => {
  const configRoot = await tempDir();
  const host = await codingHost(configRoot);
  for (const dir of ["graft", "wiki-searcher", "wiki-maintainer"]) {
    const file = join(configRoot, "skills", dir, "SKILL.md");
    await access(file);
    const text = await readFile(file, "utf8");
    assert.match(text, /^---\nname: /, `${dir} seed has frontmatter`);
    assert.ok(text.includes("description: "), `${dir} seed has a description`);
  }
  await rm(configRoot, { recursive: true, force: true });
  host.close();
});

test("seeding records size+mtime stamps in .seed-manifest.json", async () => {
  const configRoot = await tempDir();
  const host = await codingHost(configRoot);
  const manifest = JSON.parse(
    await readFile(join(configRoot, ".seed-manifest.json"), "utf8"),
  ) as Record<string, { sizeBytes: number; mtimeMs: number }>;
  for (const name of [
    "SYSTEM.md",
    "skills/graft/SKILL.md",
    "skills/wiki-searcher/SKILL.md",
    "skills/wiki-maintainer/SKILL.md",
  ]) {
    const stamp = manifest[name];
    assert.ok(stamp, `${name} is stamped`);
    assert.ok(Number.isInteger(stamp.sizeBytes) && stamp.sizeBytes >= 0);
    assert.ok(Number.isInteger(stamp.mtimeMs) && stamp.mtimeMs > 0);
    // The Rust listing derives the file's mtime by truncation
    // (`as_millis()`), so the stamp must be the truncated value: rounding
    // would sit up to +1 ms high and badge every untouched seed "edited".
    const stats = await stat(join(configRoot, name));
    assert.equal(stamp.sizeBytes, stats.size, `${name} size matches`);
    assert.equal(
      stamp.mtimeMs,
      Math.trunc(stats.mtimeMs),
      `${name} mtime is truncated, not rounded`,
    );
  }
  await rm(configRoot, { recursive: true, force: true });
  host.close();
});

test("edited skill file content is what the registry delivers", async () => {
  const configRoot = await tempDir();
  const host1 = await codingHost(configRoot);
  host1.close();
  // User edits the graft skill (applies on next daemon start).
  const editedDescription = "Custom graft guidance: always orient before spelunking.";
  const graftDir = join(configRoot, "skills", "graft");
  await rm(join(graftDir, "SKILL.md"));
  await writeFile(
    join(graftDir, "SKILL.md"),
    `---\nname: graft\ndescription: ${editedDescription}\n---\n\nUser-edited graft workflow body.\n`,
    "utf8",
  );
  const host2 = await codingHost(configRoot);
  const registered = (
    host2 as unknown as { skills: { get(name: string): { description?: string } | undefined } }
  ).skills.get("graft");
  assert.equal(registered?.description, editedDescription, "registry delivers the edited file content");
  await rm(configRoot, { recursive: true, force: true });
  host2.close();
});

test("deleted skill file regenerates from the built-in seed at next start", async () => {
  const configRoot = await tempDir();
  const host1 = await codingHost(configRoot);
  host1.close();
  const graftFile = join(configRoot, "skills", "graft", "SKILL.md");
  await rm(graftFile);
  await access(graftFile).then(
    () => assert.fail("file should be gone"),
    () => undefined,
  );
  const host2 = await codingHost(configRoot);
  const text = await readFile(graftFile, "utf8");
  assert.ok(text.includes(GRAFT_SEED_DESCRIPTION), "regenerated seed carries the built-in description");
  await rm(configRoot, { recursive: true, force: true });
  host2.close();
});

test("malformed skill file falls back to built-in seed content with a warning", async () => {
  const configRoot = await tempDir();
  const graftDir = join(configRoot, "skills", "graft");
  await mkdir(graftDir, { recursive: true });
  // Unterminated frontmatter fence ⇒ parseSkillFile throws ⇒ the daemon
  // warns and delivers the built-in seed content (never a broken skill).
  await writeFile(join(graftDir, "SKILL.md"), "---\nname: graft\ndescription: broken", "utf8");
  const host = await codingHost(configRoot);
  // Registry truth (skill.list truncates descriptions for display).
  const registered = (
    host as unknown as { skills: { get(name: string): { description?: string } | undefined } }
  ).skills.get("graft");
  assert.equal(registered?.description, GRAFT_SEED_DESCRIPTION, "built-in seed content delivered, not the broken file");
  // The user's malformed file is left alone (user-owned; repair, not clobber).
  const raw = await readFile(join(graftDir, "SKILL.md"), "utf8");
  assert.ok(raw.includes("description: broken"), "user file not overwritten");
  await rm(configRoot, { recursive: true, force: true });
  host.close();
});

test("toolNames never come from the file", async () => {
  const configRoot = await tempDir();
  const wikiDir = join(configRoot, "skills", "wiki-searcher");
  await mkdir(wikiDir, { recursive: true });
  await writeFile(
    join(wikiDir, "SKILL.md"),
    "---\nname: wiki-searcher\ndescription: Hijacked description\ntoolNames: [shell, write]\n---\n\nEscalation attempt body.\n",
    "utf8",
  );
  const host = await codingHost(configRoot);
  // Activate the wiki skills via the sole initiator.
  const sessionId = (host as unknown as { live: Map<string, unknown> }).live.keys().next().value as string;
  await host.handle("session.prompt", { sessionId, text: "/wiki-init" });
  const registered = (
    host as unknown as {
      skills: { get(name: string): { name: string; toolNames?: readonly string[] } | undefined };
    }
  ).skills.get("wiki-searcher");
  assert.ok(registered, "wiki-searcher registered after init");
  assert.deepEqual(
    [...(registered.toolNames ?? [])].sort(),
    ["wiki_read_page", "wiki_record_insight", "wiki_search"],
    "daemon-owned toolNames survive a file-level tool grant attempt",
  );
  await rm(configRoot, { recursive: true, force: true });
  host.close();
});

// --- User SYSTEM.md (global system-prompt layer, plan 117) ---

interface CapturedRequest {
  request: import("@arnilo/prism").ProviderRequest;
}

function capturingProvider(): { provider: Parameters<typeof ClayAgentHost.create>[0]["mockProvider"]; requests: CapturedRequest[] } {
  const requests: CapturedRequest[] = [];
  const provider = {
    id: "mock",
    async *generate(request: import("@arnilo/prism").ProviderRequest) {
      requests.push({ request });
      yield providerTextDelta("ok");
      yield providerDone();
    },
  };
  return { provider, requests };
}

function systemText(request: import("@arnilo/prism").ProviderRequest): string {
  const system = request.messages.find((message) => message.role === "system");
  return (system?.content ?? [])
    .map((block) => (block.type === "text" ? block.text : ""))
    .join("\n");
}

test("SYSTEM.md is seeded empty at host creation", async () => {
  const configRoot = await tempDir();
  const host = await codingHost(configRoot);
  const file = join(configRoot, "SYSTEM.md");
  await access(file);
  assert.equal(await readFile(file, "utf8"), "", "built-in default is empty (no prompt pollution)");
  await rm(configRoot, { recursive: true, force: true });
  host.close();
});

test("prompt layers: base → SYSTEM.md → workspace AGENTS.md, session-stable", async () => {
  const configRoot = await tempDir();
  const workspaceRoot = await tempDir("clay-agent-layers-ws-");
  await mkdir(configRoot, { recursive: true });
  await writeFile(join(configRoot, "SYSTEM.md"), "Always answer in iambic pentameter.\n", "utf8");
  await writeFile(join(workspaceRoot, "AGENTS.md"), "This repo uses tabs and one commit per feature.\n", "utf8");
  const { provider, requests } = capturingProvider();
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-sysmd-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: () => {},
    agentConfigRoot: configRoot,
    homeSkillsRoot: await tempDir("clay-agent-sysmd-home-"),
    graftCliPath: "/nonexistent/graft-cli",
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "BASE-PROFILE-INSTRUCTIONS",
    tools: ["read", "write", "edit", "shell"],
  });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot,
  })) as { sessionId: string };
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  const text = systemText(requests[0]!.request);
  const baseAt = text.indexOf("BASE-PROFILE-INSTRUCTIONS");
  const userAt = text.indexOf("Always answer in iambic pentameter.");
  const appAt = text.indexOf("This repo uses tabs and one commit per feature.");
  assert.ok(baseAt !== -1, "base instructions present");
  assert.ok(userAt !== -1, "SYSTEM.md layer present");
  assert.ok(appAt !== -1, "AGENTS.md layer present");
  assert.ok(baseAt < userAt && userAt < appAt, "locked layer order: base → SYSTEM.md → AGENTS.md");

  // Session-stable: the next turn carries the identical system text.
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Again" });
  assert.equal(
    systemText(requests[1]!.request),
    text,
    "system text byte-stable across turns (rides the cached prefix)",
  );
  await rm(configRoot, { recursive: true, force: true });
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});

test("AGENTS.md trust boundaries: symlink escape and oversized skip silently", async () => {
  const configRoot = await tempDir();
  const workspaceRoot = await tempDir("clay-agent-agentsmd-ws-");
  const outside = await tempDir("clay-agent-agentsmd-outside-");
  // Escape attempt: target outside the workspace must not be injected.
  const secretFile = join(outside, "secret.md");
  await writeFile(secretFile, "OUTSIDE-THE-WORKSPACE-CONTENT\n", "utf8");
  await symlink(secretFile, join(workspaceRoot, "AGENTS.md"));
  // Oversized attempt inside the workspace must also be skipped.
  const bigFile = join(workspaceRoot, "big.md");
  await writeFile(bigFile, "x".repeat(64 * 1024 + 1), "utf8");
  const { provider, requests } = capturingProvider();
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-agentsmd-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: () => {},
    agentConfigRoot: configRoot,
    homeSkillsRoot: await tempDir("clay-agent-agentsmd-home-"),
    graftCliPath: "/nonexistent/graft-cli",
  });
  await host.handle("agentProfile.register", { name: "coding", instructions: "BASE", tools: [] });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot,
  })) as { sessionId: string };
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  const text = systemText(requests[0]!.request);
  assert.ok(!text.includes("OUTSIDE-THE-WORKSPACE-CONTENT"), "symlink escape excluded");
  assert.ok(!text.includes("xxxx"), "oversized AGENTS.md skipped");

  // A symlink that stays INSIDE the workspace is fine.
  await rm(join(workspaceRoot, "AGENTS.md"));
  const inner = join(workspaceRoot, "docs");
  await mkdir(inner);
  await writeFile(join(inner, "real.md"), "Inner repo guidance.\n", "utf8");
  await symlink(join(inner, "real.md"), join(workspaceRoot, "AGENTS.md"));
  const created2 = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot,
  })) as { sessionId: string };
  await host.handle("session.prompt", { sessionId: created2.sessionId, text: "Hi" });
  const text2 = systemText(requests[1]!.request);
  assert.ok(text2.includes("Inner repo guidance."), "contained symlink injected");
  await rm(configRoot, { recursive: true, force: true });
  await rm(workspaceRoot, { recursive: true, force: true });
  await rm(outside, { recursive: true, force: true });
  host.close();
});

test("oversized SYSTEM.md is skipped with a warning, session still builds", async () => {
  const configRoot = await tempDir();
  await mkdir(configRoot, { recursive: true });
  await writeFile(join(configRoot, "SYSTEM.md"), "x".repeat(64 * 1024 + 1), "utf8");
  const { provider, requests } = capturingProvider();
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-sysmd-big-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: () => {},
    agentConfigRoot: configRoot,
    homeSkillsRoot: await tempDir("clay-agent-sysmd-big-home-"),
    graftCliPath: "/nonexistent/graft-cli",
  });
  await host.handle("agentProfile.register", { name: "coding", instructions: "BASE", tools: [] });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  assert.ok(!systemText(requests[0]!.request).includes("xxx"), "oversized layer skipped");
  await rm(configRoot, { recursive: true, force: true });
  host.close();
});
