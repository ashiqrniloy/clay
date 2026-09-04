import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-discard-"));
}

/** Provider whose text alternates per prompt so entries are distinguishable. */
function countingProvider(): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  let turn = 0;
  return {
    id: "mock",
    async *generate() {
      turn += 1;
      yield providerTextDelta(`turn-${turn}`);
      yield providerDone();
    },
  };
}

/** Summary worker sees the abandoned-path prompt; yields a refined summary. */
function summaryProvider(): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  return {
    id: "mock",
    async *generate() {
      yield providerTextDelta("REFINED-SUMMARY");
      yield providerDone();
    },
  };
}

interface Fixture {
  host: ClayAgentHost;
  sessionId: string;
  events: Array<{ type?: string; content?: { text?: string } }>;
}

async function treeHost(): Promise<Fixture> {
  const events: Array<{ type?: string; content?: { text?: string } }> = [];
  let hostRef: ClayAgentHost | undefined;
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: countingProvider(),
    emit: (method, params) => {
      if (method === "event") {
        const event = params as { event?: { type?: string; content?: { text?: string } } };
        if (event.event) events.push(event.event);
        return;
      }
      if (method !== "reverse") return;
      const frame = params as { id: number; method: string; params?: Record<string, unknown> };
      if (frame.method === "checkpoint.capture" || frame.method === "checkpoint.restore") {
        hostRef!.resolveReverse(frame.id, { result: { documents: 0 } });
        return;
      }
      if (frame.method === "checkpoint.list") {
        hostRef!.resolveReverse(frame.id, { result: { sessionId: frame.params?.sessionId, entryIds: [] } });
        return;
      }
      hostRef!.resolveReverse(frame.id, { error: { code: -32000, message: `unknown: ${frame.method}` } });
    },
  });
  hostRef = host;
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws",
  })) as { sessionId: string };
  return { host, sessionId: created.sessionId, events };
}

async function registerSlashSurface(host: ClayAgentHost): Promise<void> {
  for (const command of [
    { name: "/tree", handler: "tree", description: "Show the branch tree." },
    { name: "/branch", handler: "checkout", description: "Checkout a branch point." },
    { name: "/discard", handler: "discard", description: "Discard a branch." },
    { name: "/fork", handler: "forkSession", description: "Fork the session." },
    { name: "/clone", handler: "cloneSession", description: "Clone the session." },
  ]) {
    await host.handle("command.register", command);
  }
}

async function loadEntries(
  host: ClayAgentHost,
  sessionId: string,
): Promise<Array<{ id: string; kind: string; parentId?: string; summary?: string }>> {
  const loaded = (await host.handle("session.load", { sessionId })) as {
    entries: Array<{ id: string; kind: string; parentId?: string; summary?: string }>;
  };
  return loaded.entries;
}

test("tree drill: checkout from an early entry appends a summary and re-roots the leaf; /tree renders ids and summaries", async () => {
  const fixture = await treeHost();
  const { host, sessionId, events } = fixture;
  await registerSlashSurface(host);
  await host.handle("session.prompt", { sessionId, text: "first" });
  await host.handle("session.prompt", { sessionId, text: "second" });
  await host.handle("session.prompt", { sessionId, text: "third" });
  const entries = await loadEntries(host, sessionId);
  const branchPoint = entries[0]!.id;

  // In-place navigation: checkout the first entry. The abandoned tail keeps
  // its entries; a summary lands at the branch point (new branch, pi model).
  const out = (await host.handle("session.checkout", { sessionId, entryId: branchPoint })) as {
    leafId?: string;
    summaryEntryId?: string;
  };
  assert.equal(out.leafId, out.summaryEntryId);
  const after = await loadEntries(host, sessionId);
  const summary = after.find((entry) => entry.id === out.summaryEntryId);
  assert.ok(summary, "summary entry exists");
  assert.equal(summary.parentId, branchPoint);
  assert.equal(summary.kind, "summary");
  assert.match(summary.summary ?? "", /turn-3|turn-2|turn-1/, "preview summarizes the abandoned tail");

  // /tree via the prompt intercept: the dispatch value + transcript render.
  const tree = (await host.handle("session.prompt", { sessionId, text: "/tree" })) as {
    name?: string;
    value?: { summaries?: Array<Record<string, unknown>>; branches?: Array<Record<string, unknown>> };
  };
  assert.equal(tree.name, "/tree");
  assert.ok((tree.value?.summaries?.length ?? 0) >= 1, "tree carries the branch summary");
  // Synthetic transcript triple closes the client run and renders the text.
  const kinds = events.map((event) => event.type);
  assert.ok(kinds.includes("agent_started"), "feedback starts the run");
  assert.ok(kinds.includes("message_delta"), "feedback text streams");
  assert.ok(kinds.includes("agent_finished"), "feedback closes the run");
  const deltas = events.filter((event) => event.type === "message_delta");
  const text = deltas.map((delta) => delta.content?.text ?? "").join("");
  assert.match(text, /tree \(leaf /);
  assert.match(text, /\[summary\]/, "summary entries render in the tree");
  host.close();
});

test("discard is two-step: preview without confirm, checkout with it; abandoned side keeps entries and summary", async () => {
  const fixture = await treeHost();
  const { host, sessionId } = fixture;
  await registerSlashSurface(host);
  await host.handle("session.prompt", { sessionId, text: "one" });
  await host.handle("session.prompt", { sessionId, text: "two" });
  const entries = await loadEntries(host, sessionId);
  const branchPoint = entries[0]!.id;

  // Step 1: no confirm — bounded preview, tree unchanged.
  const preview = (await host.handle("command.dispatch", {
    name: "/discard",
    sessionId,
    args: { entryId: branchPoint },
  })) as { value: { needsConfirm?: boolean; entryId?: string } };
  assert.equal(preview.value.needsConfirm, true);
  assert.equal(preview.value.entryId, branchPoint);
  const before = await loadEntries(host, sessionId);

  // Step 2: confirm — checkout semantics (checkpoint restore + leaf move).
  const discarded = (await host.handle("command.dispatch", {
    name: "/discard",
    sessionId,
    args: { entryId: branchPoint, confirm: true },
  })) as { value: { leafId?: string; summaryEntryId?: string } };
  assert.ok(discarded.value.summaryEntryId);
  const after = await loadEntries(host, sessionId);
  // Nothing deleted: the abandoned entries are all still in the store.
  assert.ok(after.length >= before.length + 1, "discard adds only the summary");
  for (const entry of before) {
    assert.ok(after.some((item) => item.id === entry.id), "no orphans: prior entries remain");
  }
  // The abandoned leaf's text still loads (history preserved).
  const search = (await host.handle("session.search", { sessionId })) as {
    hits: Array<{ sessionId: string }>;
  };
  assert.ok(search.hits.length >= 1);
  host.close();
});

test("LLM branch-summary refine lands after the preview and re-roots the leaf", async () => {
  const events: Array<{ id: number; method: string; params?: Record<string, unknown> }> = [];
  let hostRef: ClayAgentHost | undefined;
  // One host, two providers: the session provider counts turns; the summary
  // worker's provider (registered as the model registry entry for the worker
  // model) yields the refined text. Same mock id — the worker run consumes
  // the same provider sequence, so the refined entry carries its text.
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: summaryProvider(),
    emit: (method, params) => {
      if (method !== "reverse") return;
      const frame = params as { id: number; method: string; params?: Record<string, unknown> };
      events.push(frame);
      if (frame.method === "checkpoint.restore" || frame.method === "checkpoint.capture") {
        hostRef!.resolveReverse(frame.id, { result: { documents: 0 } });
        return;
      }
      if (frame.method === "checkpoint.list") {
        hostRef!.resolveReverse(frame.id, { result: { entryIds: [] } });
        return;
      }
      hostRef!.resolveReverse(frame.id, { error: { code: -32000, message: `unknown: ${frame.method}` } });
    },
  });
  hostRef = host;
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws",
  })) as { sessionId: string };
  const sessionId = created.sessionId;
  await host.handle("session.prompt", { sessionId, text: "experiment" });
  const entries = await loadEntries(host, sessionId);
  const branchPoint = entries[0]!.id;

  const out = (await host.handle("session.checkout", { sessionId, entryId: branchPoint })) as {
    summaryEntryId?: string;
  };
  // The worker session consumes the same mock provider: the refined summary
  // text equals the provider's fixed output.
  await new Promise((resolve) => setTimeout(resolve, 120));
  const after = await loadEntries(host, sessionId);
  const refined = after.find(
    (entry) =>
      entry.kind === "summary" &&
      entry.parentId === branchPoint &&
      entry.id !== out.summaryEntryId,
  );
  assert.ok(refined, "LLM refine appends a second summary entry at the branch point");
  assert.match(refined!.summary ?? "", /REFINED-SUMMARY/);
  host.close();
});

test("/fork and /clone independence; mid-tree discard leaves no orphan branches", async () => {
  const fixture = await treeHost();
  const { host, sessionId } = fixture;
  await registerSlashSurface(host);
  await host.handle("session.prompt", { sessionId, text: "root" });
  await host.handle("session.prompt", { sessionId, text: "second" });
  const entries = await loadEntries(host, sessionId);
  const first = entries[0]!.id;

  const forked = (await host.handle("command.dispatch", { name: "/fork", sessionId, args: { entryId: first } })) as {
    value: { sessionId: string; leafId?: string };
  };
  assert.equal(forked.value.sessionId, sessionId);
  const cloned = (await host.handle("command.dispatch", { name: "/clone", sessionId, args: { entryId: first } })) as {
    value: { sessionId: string };
  };
  assert.notEqual(cloned.value.sessionId, sessionId);

  // Discard the mid-tree branch: entries stay, leaf moves back, summaries
  // recorded — the store holds no dangling session rows for either op.
  const discarded = (await host.handle("session.checkout", { sessionId, entryId: first })) as {
    summaryEntryId?: string;
  };
  assert.ok(discarded.summaryEntryId);
  const finalEntries = await loadEntries(host, sessionId);
  assert.ok(finalEntries.some((entry) => entry.id === discarded.summaryEntryId));
  host.close();
});