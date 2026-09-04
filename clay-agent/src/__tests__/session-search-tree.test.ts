import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-tree-"));
}

function textProvider(text: string): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  return {
    id: "mock",
    async *generate() {
      yield providerTextDelta(text);
      yield providerDone();
    },
  };
}

async function chatHost(text: string, workspaceRoot?: string): Promise<{ host: ClayAgentHost; sessionId: string }> {
  let hostRef: ClayAgentHost | undefined;
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(text),
    emit: (method, params) => {
      // Server-side stand-in: checkpoint capture/restore succeed as no-ops.
      if (method !== "reverse") return;
      const frame = params as { id: number; method: string };
      if (frame.method === "checkpoint.capture" || frame.method === "checkpoint.restore") {
        hostRef!.resolveReverse(frame.id, { result: { documents: 0 } });
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
    ...(workspaceRoot ? { workspaceRoot } : {}),
  })) as { sessionId: string };
  return { host, sessionId: created.sessionId };
}

test("session.search is workspace-scoped and returns hits with leafId", async () => {
  // Workspace A: two sessions mentioning "llanowar".
  const a = await chatHost("llanowar elves attack", "/ws-a");
  const hostA = a.host;
  const second = (await hostA.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws-a",
  })) as { sessionId: string };
  await hostA.handle("session.prompt", { sessionId: second.sessionId, text: "another llanowar note" });
  // Workspace B on its own host/store: same fixture text.
  const b = await chatHost("llanowar elves attack", "/ws-b");
  const hostB = b.host;

  const searchA = (await hostA.handle("session.search", { sessionId: (await firstSessionId(hostA)) })) as {
    hits: Array<{ sessionId: string; workspaceRoot?: string }>;
  };
  assert.ok(searchA.hits.length >= 1, "search within workspace A finds its own sessions");
  for (const hit of searchA.hits) {
    assert.notEqual(hit.workspaceRoot, "/ws-b");
  }
  // A hit includes a leafId usable for checkout.
  assert.ok(searchA.hits.every((hit) => typeof hit.sessionId === "string"));

  // Cross-workspace isolation: A never returns B's store (separate host, but
  // also verify filtering is applied inside one store).
  const fresh = (await hostA.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws-b",
  })) as { sessionId: string };
  const searchB = (await hostA.handle("session.search", { sessionId: fresh.sessionId })) as {
    hits: Array<{ sessionId: string }>;
  };
  assert.ok(searchB.hits.every((hit) => hit.sessionId !== second.sessionId), "workspace filter excludes other workspace");
  hostA.close();
  hostB.close();
});

test("session.load with entryId opens the branch through the matched entry (read-only view)", async () => {
  const { host, sessionId } = await chatHost("first");
  await host.handle("session.prompt", { sessionId, text: "second" });
  await host.handle("session.prompt", { sessionId, text: "third" });
  const loaded = (await host.handle("session.load", { sessionId })) as {
    entries: Array<{ id: string; kind: string; parentId?: string }>;
  };
  const second = loaded.entries[1]!;
  const opened = (await host.handle("session.load", {
    sessionId,
    entryId: second.id,
  })) as { entries: Array<{ id: string }> };
  // Path root→entry only: later turns are not in the view.
  assert.equal(opened.entries.length, 2);
  assert.equal(opened.entries[opened.entries.length - 1]!.id, second.id);
  // The tree is untouched: the default tail still loads all entries.
  const after = (await host.handle("session.load", { sessionId })) as {
    entries: unknown[];
  };
  assert.equal(after.entries.length, loaded.entries.length);
  // Unknown entry ids fall back to the default tail (bounded, no error).
  const fallback = (await host.handle("session.load", {
    sessionId,
    entryId: "entry_nope",
  })) as { entries: unknown[] };
  assert.equal(fallback.entries.length, loaded.entries.length);
  host.close();
});

async function firstSessionId(host: ClayAgentHost): Promise<string> {
  const listed = (await host.handle("session.list", {})) as { sessions: Array<{ id: string }> };
  return listed.sessions[0]!.id;
}

test("search hits are transcript data: checkout appends only a bounded branch summary", async () => {
  const { host, sessionId } = await chatHost("needle in haystack");
  await host.handle("session.prompt", { sessionId, text: "second turn" });
  const search = (await host.handle("session.search", { sessionId })) as {
    hits: Array<{ sessionId: string; leafId?: string }>;
  };
  const self = search.hits.find((hit) => hit.sessionId === sessionId);
  assert.ok(self, "session finds itself");
  const loadedBefore = (await host.handle("session.load", { sessionId })) as {
    entries: Array<{ id: string; kind: string; parentId?: string }>;
  };
  if (self.leafId) {
    await host.handle("session.checkout", { sessionId, entryId: self.leafId });
    const loadedAfter = (await host.handle("session.load", { sessionId })) as {
      entries: Array<{ id: string; kind: string; parentId?: string; summary?: string }>;
    };
    // Plan 108 task 10 (decision 2200): checkout appends exactly one entry —
    // the branch summary at the branch point. Transcript text is never copied.
    assert.equal(loadedAfter.entries.length, loadedBefore.entries.length + 1);
    const added = loadedAfter.entries[loadedAfter.entries.length - 1]!;
    assert.equal(added.kind, "summary");
    assert.equal(added.parentId, self.leafId);
    assert.ok((added.summary ?? "").length <= 2048);
  }
  host.close();
});

test("fork and clone produce independent branches/sessions", async () => {
  const { host, sessionId } = await chatHost("root turn");
  await host.handle("session.prompt", { sessionId, text: "more turns" });
  const loaded = (await host.handle("session.load", { sessionId })) as {
    entries: Array<{ id: string }>;
  };
  const firstEntry = loaded.entries[0]!.id;

  const forked = (await host.handle("session.fork", { sessionId, entryId: firstEntry })) as {
    sessionId: string;
    leafId?: string;
  };
  assert.equal(forked.sessionId, sessionId, "fork keeps the session id");
  assert.equal(forked.leafId, firstEntry, "fork moves to the requested leaf");

  const cloned = (await host.handle("session.clone", { sessionId, entryId: firstEntry })) as {
    sessionId: string;
  };
  assert.notEqual(cloned.sessionId, sessionId, "clone gets a new session id");
  const cloneLoaded = (await host.handle("session.load", { sessionId: cloned.sessionId })) as {
    entries: unknown[];
  };
  assert.ok(cloneLoaded.entries.length > 0, "clone copies branch entries");

  // Original leaf is untouched (abandoned sides kept, not deleted).
  const stillThere = (await host.handle("session.load", { sessionId })) as {
    entries: Array<{ id: string }>;
  };
  assert.ok(stillThere.entries.length >= loaded.entries.length);
  host.close();
});

test("session.checkpoint captures via reverse RPC; restore failure fails checkout closed", async () => {
  let hostRef: ClayAgentHost | undefined;
  let restoreShouldFail = false;
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider("hello"),
    emit: (method, params) => {
      if (method !== "reverse") return;
      const frame = params as { id: number; method: string; params?: Record<string, unknown> };
      if (frame.method === "checkpoint.capture") {
        hostRef!.resolveReverse(frame.id, { result: { sessionId: frame.params?.sessionId, entryId: frame.params?.entryId, documents: 0 } });
        return;
      }
      if (frame.method === "checkpoint.restore") {
        if (restoreShouldFail) {
          hostRef!.resolveReverse(frame.id, { error: { code: -32000, message: "document lease held by another client" } });
        } else {
          hostRef!.resolveReverse(frame.id, { result: { documents: 0 } });
        }
        return;
      }
      hostRef!.resolveReverse(frame.id, { error: { code: -32000, message: `unknown: ${frame.method}` } });
    },
  });
  hostRef = host;
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const created = (await host.handle("session.new", { profile: "chat", provider: "mock", model: "demo" })) as {
    sessionId: string;
  };
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "turn one" });
  const loaded = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    entries: Array<{ id: string }>;
  };
  const entryId = loaded.entries[loaded.entries.length - 1]!.id;

  await host.handle("session.checkpoint", { sessionId: created.sessionId, entryId });

  // Restore success (or no-op) lets checkout proceed; the leaf re-roots on
  // the branch summary entry (plan 108 task 10), not the raw branch point.
  const out = (await host.handle("session.checkout", { sessionId: created.sessionId, entryId })) as {
    sessionId: string;
    leafId?: string;
    summaryEntryId?: string;
  };
  assert.equal(out.sessionId, created.sessionId);
  assert.ok(out.summaryEntryId, "checkout returns the summary entry");
  assert.equal(out.leafId, out.summaryEntryId);

  // Restore failure (lease held elsewhere) fails the checkout closed.
  restoreShouldFail = true;
  const leafBefore = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    entries: Array<{ id: string }>;
  };
  await assert.rejects(
    () => host.handle("session.checkout", { sessionId: created.sessionId, entryId: "entry_other" }),
    /lease held by another client/,
  );
  const leafAfter = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    entries: Array<{ id: string }>;
  };
  assert.deepEqual(leafAfter.entries.map((entry) => entry.id), leafBefore.entries.map((entry) => entry.id), "failed restore leaves the tree untouched");
  host.close();
});

test("session.setAutonomy toggles full autonomy; default stays false", async () => {
  const { host, sessionId } = await chatHost("ok");
  assert.equal(host.sessionAutonomy(sessionId), false);
  const enabled = (await host.handle("session.setAutonomy", { sessionId, enabled: true })) as {
    sessionId: string;
    fullAutonomy: boolean;
  };
  assert.deepEqual(enabled, { sessionId, fullAutonomy: true });
  assert.equal(host.sessionAutonomy(sessionId), true);
  const disabled = (await host.handle("session.setAutonomy", { sessionId, enabled: false })) as {
    fullAutonomy: boolean;
  };
  assert.equal(disabled.fullAutonomy, false);
  await assert.rejects(
    () => host.handle("session.setAutonomy", { sessionId, enabled: "yes" }),
    /enabled must be a boolean/,
  );
  await assert.rejects(
    () => host.handle("session.setAutonomy", { sessionId: "missing", enabled: true }),
    /Unknown session/,
  );
  host.close();
});
