import { appendFileSync, mkdtempSync } from "node:fs";
import { providerDone, providerTextDelta, providerToolCall } from "@arnilo/prism";
import { ClayAgentHost } from "./dist/host.js";
const LOG = "/tmp/om-dbg.log";
function log(...args) {
  appendFileSync(LOG, args.join(" ") + "\n");
}
function routingProvider(tag) {
  return {
    id: "mock",
    async *generate(request) {
      const toolNames = (request.tools ?? []).map((t) => t.name).join(",");
      const text = JSON.stringify(request.messages);
      log(tag, "REQ tools=[" + toolNames + "]");
      if (toolNames.includes("record_observation")) {
        const entryId = text.match(/\[(entry_[0-9a-f-]+)\]/)?.[1];
        log(tag, "OBS entryId=" + entryId);
        if (entryId)
          yield providerToolCall({
            type: "tool_call",
            id: "o1",
            name: "record_observation",
            arguments: {
              content: "User prefers dark mode",
              sourceEntryIds: [entryId],
              relevance: "high",
            },
          });
        yield providerDone();
        return;
      }
      if (toolNames.includes("record_reflection")) {
        const obsId = text.match(/\[([0-9a-f]{12})\]/)?.[1];
        log(tag, "REFL obsId=" + obsId);
        if (obsId)
          yield providerToolCall({
            type: "tool_call",
            id: "r1",
            name: "record_reflection",
            arguments: {
              content: "User values dark mode",
              supportingObservationIds: [obsId],
            },
          });
        yield providerDone();
        return;
      }
      yield providerTextDelta("answer");
      yield providerDone();
    },
  };
}
const dataDir = mkdtempSync("/tmp/om-dbg5-");
const worker = routingProvider("WORKER");
const host = await ClayAgentHost.create({
  dataDir,
  passphrase: "pass-phrase-ok",
  mock: true,
  mockProvider: worker,
  emit: () => {},
  observationalMemory: {
    observation: { provider: worker, messageTokens: 1 },
    reflection: { provider: worker, observationTokens: 1 },
    dropper: { targetTokens: 1, policy: "lowest-relevance" },
  },
});
await host.handle("agentProfile.register", {
  name: "Om",
  description: "OM fixture",
  observationalMemory: true,
});
const created = await host.handle("session.new", {
  profile: "Om",
  provider: "mock",
  model: "demo",
});
await host.handle("session.om.set", {
  sessionId: created.sessionId,
  workers: {
    observation: { provider: "mock", model: "demo" },
    reflection: { provider: "mock", model: "demo" },
  },
});
await host.handle("session.prompt", {
  sessionId: created.sessionId,
  text: "hello",
});
await new Promise((r) => setTimeout(r, 600));
const view = await host.handle("session.om.activity", {
  sessionId: created.sessionId,
});
log("ACTIVITY", JSON.stringify(view.activity));
process.exit(0);
