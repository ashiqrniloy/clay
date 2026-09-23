// Transcript view model (plan 119 SC-4): the pure AG-UI-message → box
// derivation, and the PF-2 guard that its previous-agent scan stays one pass.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  agentLabel,
  agentOf,
  kindOf,
  previousAgents,
  toolMeta,
  type TranscriptBox,
} from "./transcript-model";

const box = (id: string, agent?: string): TranscriptBox => ({
  id,
  kind: "assistant",
  label: "agent",
  content: id,
  ...(agent ? { agent } : {}),
});

describe("transcript-model", () => {
  it("names agents the way the launcher does", () => {
    expect(agentLabel("reviewer")).toBe("Reviewer");
    expect(agentLabel("code-review_bot")).toBe("Code Review Bot");
    expect(agentLabel("")).toBe("");
  });

  it("reads the per-turn producer from the server-stamped row metadata", () => {
    expect(agentOf({ metadata: { agent: "planner" } })).toBe("planner");
    expect(agentOf({ metadata: { agent: "" } })).toBeUndefined();
    expect(agentOf({ metadata: {} })).toBeUndefined();
    expect(agentOf(null)).toBeUndefined();
  });

  it("classifies rows by clay kind first, then role", () => {
    expect(kindOf({ role: "assistant", metadata: { clayKind: "usage" } })).toBe(
      "usage",
    );
    expect(kindOf({ role: "assistant", metadata: { clayKind: "error" } })).toBe(
      "error",
    );
    expect(kindOf({ role: "tool", metadata: { clayKind: "skill" } })).toBe(
      "skill",
    );
    expect(kindOf({ role: "tool" })).toBe("tool");
    expect(kindOf({ role: "user" })).toBe("user");
    expect(kindOf({ role: "system" })).toBeNull();
  });

  it("carries tool/skill names for labels and category counts", () => {
    expect(
      toolMeta({ metadata: { toolName: "read", skillName: "wiki" } }),
    ).toEqual({ toolName: "read", skillName: "wiki" });
    expect(toolMeta({})).toEqual({ toolName: "", skillName: "" });
  });

  it("hands each turn the producer that ran before it", () => {
    const boxes = [
      box("a", "planner"),
      box("b"),
      box("c", "reviewer"),
      box("d"),
      box("e", "reviewer"),
    ];
    expect(previousAgents(boxes)).toEqual([
      undefined,
      "planner",
      "planner",
      "reviewer",
      "reviewer",
    ]);
  });

  it("answers null-producer turns like the old reverse scan did", () => {
    const boxes = [box("a"), box("b"), box("c", "planner"), box("d")];
    expect(previousAgents(boxes)).toEqual([
      undefined,
      undefined,
      undefined,
      "planner",
    ]);
    expect(previousAgents([])).toEqual([]);
  });

  it("PF-2: reads the box list a bounded number of times, never per-box", () => {
    // A per-box `slice(0, index).reverse()` scan is O(n²) *reads*; the fix is
    // one forward pass. Counting property reads pins the complexity, not just
    // the output: `length` + each box + its `agent` is ≤ 3n.
    const boxes = Array.from({ length: 500 }, (_, index) =>
      box(`t${index}`, index % 3 === 0 ? "planner" : undefined),
    );
    let reads = 0;
    const counted = new Proxy(boxes, {
      get(target, property, receiver) {
        reads += 1;
        return Reflect.get(target, property, receiver);
      },
    });
    expect(previousAgents(counted)).toHaveLength(500);
    expect(reads).toBeLessThanOrEqual(boxes.length * 3);
  });

  it("PF-2: the render path no longer scans per turn", () => {
    // Source-shape guard (the frontend's `chat-surface-absence` precedent):
    // the O(n²) reverse scan must not come back through a later edit.
    const here = fileURLToPath(new URL(".", import.meta.url));
    for (const file of ["TranscriptList.tsx", "CodingAgentPanel.tsx"]) {
      const source = readFileSync(`${here}${file}`, "utf8");
      expect(source).not.toMatch(/\.slice\(0, index\)\s*\n?\s*\.reverse\(\)/);
      expect(source).not.toContain(".reverse().find(");
    }
    expect(readFileSync(`${here}transcript-model.ts`, "utf8")).toContain(
      "export function previousAgents",
    );
  });
});
