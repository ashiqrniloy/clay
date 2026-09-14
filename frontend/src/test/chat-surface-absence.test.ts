// Plan 118 task "Remove the chat frontend surface and de-chat the shared agent
// module": the `@clay/chat` React surface is deleted, not merely unreferenced,
// and the shared agent module no longer advertises a removed product surface to
// readers or to `grep chat`.
//
// This file names the retired paths and symbols on purpose — it is the only
// exemption in the scan below. The matching Rust gate
// (`plan118_chat_landing_and_its_commands_are_absent` in
// `tests/package_ui_conformance.rs`) scans the same tree for the removed
// package ids, so a resurrection fails from both sides.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const srcRoot = path.resolve(here, "..");

/** Retired paths and symbols (the chat-era naming the module moved away from). */
const RETIRED = [
  "frontend/src/chat",
  "chatAgent",
  "ChatAgentModule",
  "ChatSnapshot",
  "ChatStatus",
  "ChatIntentContext",
  "ChatPanel",
  "seedChatFixture",
  "resetChatAgentForTests",
  "__clayChatAgent",
];

const SELF = fileURLToPath(import.meta.url);

function sourceFiles(dir: string): string[] {
  const found: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      found.push(...sourceFiles(full));
    } else if (entry.isFile() && full !== SELF) {
      found.push(full);
    }
  }
  return found.sort();
}

/** Hits for the retired names in one text — the matcher the scan relies on. */
function hitsIn(text: string): string[] {
  return RETIRED.filter((needle) => text.includes(needle));
}

describe("retired chat surface", () => {
  it("matches the retired names (the scan below cannot pass vacuously)", () => {
    expect(hitsIn(`${"chat"}Agent`)).toEqual(["chatAgent"]);
    expect(hitsIn("nothing retired here")).toEqual([]);
  });

  it("keeps no chat path or chat-era symbol anywhere in frontend/src", () => {
    const hits: string[] = [];
    for (const file of sourceFiles(srcRoot)) {
      const relative = path.relative(path.resolve(srcRoot, ".."), file);
      if (relative.split(/[\\/]/).includes("chat")) {
        hits.push(`${relative}: retired directory`);
        continue;
      }
      if (!/\.(ts|tsx|css)$/.test(file)) continue;
      for (const needle of hitsIn(fs.readFileSync(file, "utf8"))) {
        hits.push(`${relative}: ${needle}`);
      }
    }
    expect(hits, "the removed chat surface still referenced").toEqual([]);
  });

  it("keeps the shared agent module under session naming", () => {
    const state = fs.readFileSync(
      path.join(srcRoot, "agent", "state.ts"),
      "utf8",
    );
    expect(state).toContain("export const agentSession");
    expect(state).toContain("export function resetAgentSessionForTests");
    expect(state).toContain("export interface AgentSnapshot");
    expect(fs.existsSync(path.join(srcRoot, "chat"))).toBe(false);
  });
});
