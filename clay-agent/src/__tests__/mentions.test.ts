// Plan 117 @-mentions daemon tests: `@skill:` manual trigger (loaded-set
// mutation + instruction line), `@file:` attachment (server-side resolution,
// image + text blocks, containment), unknown tokens stay plain text, and
// the bounded workspace.files listing for the dropdown.

import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "bun:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(prefix = "clay-agent-mentions-"): Promise<string> {
  return mkdtemp(join(tmpdir(), prefix));
}

interface CapturedRequest {
  request: import("@arnilo/prism").ProviderRequest;
}

function capturingProvider(): {
  provider: Parameters<typeof ClayAgentHost.create>[0]["mockProvider"];
  requests: CapturedRequest[];
} {
  const requests: CapturedRequest[] = [];
  return {
    provider: {
      id: "mock",
      async *generate(request: import("@arnilo/prism").ProviderRequest) {
        requests.push({ request });
        yield providerTextDelta("ok");
        yield providerDone();
      },
    },
    requests,
  };
}

function systemText(request: import("@arnilo/prism").ProviderRequest): string {
  return request.messages
    .filter((message) => message.role === "system")
    .flatMap((message) => message.content)
    .map((block) => (block.type === "text" ? block.text : ""))
    .join("\n");
}

function userText(request: import("@arnilo/prism").ProviderRequest): string {
  return request.messages
    .filter((message) => message.role === "user")
    .flatMap((message) => message.content)
    .map((block) => (block.type === "text" ? block.text : ""))
    .join("\n");
}

function userBlocks(request: import("@arnilo/prism").ProviderRequest): import("@arnilo/prism").ContentBlock[] {
  return request.messages
    .filter((message) => message.role === "user")
    .flatMap((message) => [...message.content]);
}

async function mentionsHost(workspaceRoot: string, registerSkill: boolean) {
  const { provider, requests } = capturingProvider();
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-mentions-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: () => {},
    agentConfigRoot: await tempDir("clay-agent-mentions-config-"),
    homeSkillsRoot: await tempDir("clay-agent-mentions-home-"),
    graftCliPath: "/nonexistent/graft-cli",
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "BASE",
    tools: ["read", "write", "edit", "shell"],
  });
  if (registerSkill) {
    await host.handle("skill.register", {
      name: "brief",
      description: "Answer tersely.",
      instructions: "Keep every answer under ten words.",
      toolNames: ["read"],
    });
  }
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot,
  })) as { sessionId: string };
  return { host, requests, sessionId: created.sessionId };
}

test("@skill mention loads the skill and injects the instruction line", async () => {
  const workspaceRoot = await tempDir();
  const { host, requests, sessionId } = await mentionsHost(workspaceRoot, true);
  await host.handle("session.prompt", {
    sessionId,
    text: "Review this @skill:brief please",
  });
  const text = systemText(requests[0]!.request);
  assert.ok(
    text.includes("Keep every answer under ten words."),
    "the mentioned skill's body renders from the first round (loaded set)",
  );
  const user = userText(requests[0]!.request);
  assert.ok(
    user.includes("[skills loaded by mention: brief]"),
    "the instruction line names the loaded skill",
  );
  assert.ok(user.includes("`brief`"), "the token rewrites to the plain name");
  assert.ok(!user.includes("@skill:"), "no raw token reaches the model");
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});

test("unknown skill mention stays plain text (chat-safe)", async () => {
  const workspaceRoot = await tempDir();
  const { host, requests, sessionId } = await mentionsHost(workspaceRoot, true);
  await host.handle("session.prompt", {
    sessionId,
    text: "Review this @skill:nope please",
  });
  const text = systemText(requests[0]!.request);
  assert.ok(!text.includes("Keep every answer"), "nothing loads");
  const user = userText(requests[0]!.request);
  assert.ok(user.includes("@skill:nope"), "unknown token passes through");
  assert.ok(!user.includes("skills loaded by mention"), "no instruction line");
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});

test("a skill whose tools are unavailable never loads via mention", async () => {
  const workspaceRoot = await tempDir();
  const { host, requests, sessionId } = await mentionsHost(workspaceRoot, false);
  await host.handle("skill.register", {
    name: "needs-missing",
    description: "Requires a tool the session lacks.",
    instructions: "BODY-NEVER-LOADS",
    toolNames: ["definitely_absent_tool"],
  });
  await host.handle("session.prompt", {
    sessionId,
    text: "Try @skill:needs-missing",
  });
  assert.ok(
    !systemText(requests[0]!.request).includes("BODY-NEVER-LOADS"),
    "activation discipline (toolNames subset) gates manual loads too",
  );
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});

test("@file mention attaches text files inside the workspace root", async () => {
  const workspaceRoot = await tempDir();
  await writeFile(join(workspaceRoot, "notes.md"), "WORKSPACE-NOTES-CONTENT\n", "utf8");
  const { host, requests, sessionId } = await mentionsHost(workspaceRoot, false);
  await host.handle("session.prompt", {
    sessionId,
    text: "Summarize @file:notes.md for me",
  });
  const user = userText(requests[0]!.request);
  assert.ok(user.includes("[attached file: notes.md]"), "text file rides a fenced block");
  assert.ok(user.includes("WORKSPACE-NOTES-CONTENT"), "file content attached");
  assert.ok(!user.includes("@file:notes.md"), "resolved token removed from the prose");
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});

test("@file attaches images as image content blocks", async () => {
  const workspaceRoot = await tempDir();
  // 1x1 PNG (valid header bytes are enough for the block-shape assertion).
  await writeFile(
    join(workspaceRoot, "shot.png"),
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  );
  const { host, requests, sessionId } = await mentionsHost(workspaceRoot, false);
  await host.handle("session.prompt", { sessionId, text: "What is @file:shot.png" });
  const image = userBlocks(requests[0]!.request).find((block) => block.type === "image") as
    | { type: "image"; mimeType?: string; data?: string }
    | undefined;
  assert.ok(image, "image block attached");
  assert.equal(image.mimeType, "image/png");
  assert.equal(
    image.data,
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).toString("base64"),
  );
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});

test("@file containment: symlink escape and missing files stay plain text", async () => {
  const workspaceRoot = await tempDir();
  const outside = await tempDir("clay-agent-mentions-outside-");
  await writeFile(join(outside, "secret.md"), "OUTSIDE-SECRET\n", "utf8");
  await symlink(join(outside, "secret.md"), join(workspaceRoot, "leak.md"));
  const { host, requests, sessionId } = await mentionsHost(workspaceRoot, false);
  await host.handle("session.prompt", {
    sessionId,
    text: "Peek @file:leak.md and @file:absent.md",
  });
  const user = userText(requests[0]!.request);
  assert.ok(!user.includes("OUTSIDE-SECRET"), "symlink escape never attaches");
  assert.ok(user.includes("@file:leak.md"), "escaped token stays plain text");
  assert.ok(user.includes("@file:absent.md"), "missing token stays plain text");
  await rm(workspaceRoot, { recursive: true, force: true });
  await rm(outside, { recursive: true, force: true });
  host.close();
});

test("workspace.files lists bounded relative paths, skipping dotfiles and build dirs", async () => {
  const workspaceRoot = await tempDir();
  await mkdir(join(workspaceRoot, "src"), { recursive: true });
  await mkdir(join(workspaceRoot, ".git"), { recursive: true });
  await mkdir(join(workspaceRoot, "node_modules"), { recursive: true });
  await writeFile(join(workspaceRoot, "src", "main.rs"), "fn main() {}\n", "utf8");
  await writeFile(join(workspaceRoot, "README.md"), "# readme\n", "utf8");
  await writeFile(join(workspaceRoot, ".hidden"), "x", "utf8");
  await writeFile(join(workspaceRoot, ".git", "config"), "x", "utf8");
  await writeFile(join(workspaceRoot, "node_modules", "leftpad.js"), "x", "utf8");
  const { host, sessionId } = await mentionsHost(workspaceRoot, false);
  const listing = (await host.handle("workspace.files", { sessionId })) as {
    files: string[];
  };
  assert.ok(listing.files.includes(join("src", "main.rs")));
  assert.ok(listing.files.includes("README.md"));
  assert.ok(!listing.files.some((file) => file.startsWith(".")), "dotfiles skipped");
  assert.ok(!listing.files.some((file) => file.includes("node_modules")), "build dirs skipped");
  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});
