// Transcript view model (plan 119 SC-4): the ONE derivation from AG-UI
// messages to the turn boxes the transcript renders. Pure — no React, no
// styles — so the panel (stream → boxes), `TranscriptList` (boxes → turns)
// and the per-turn agent attribution share it without a second copy.

export type TranscriptKind =
  "user" | "assistant" | "reasoning" | "usage" | "error" | "tool" | "skill";

export interface TranscriptBox {
  id: string;
  kind: TranscriptKind;
  label: string;
  content: string;
  /** Tool rows only: the tool name for Context-tab category counts. */
  toolName?: string;
  /** Plan 118 task 35: the agent type that produced the turn, when the
   *  server stamped one (rows carry their producer's name across a switch). */
  agent?: string;
}

/** `reviewer` → `Reviewer` (the launcher's label rule, client side). */
export function agentLabel(type: string): string {
  return type
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

/** Per-turn agent attribution (plan 118 task 35): read from the message
 *  metadata the server projects per transcript row. */
export function agentOf(message: unknown): string | undefined {
  const agent =
    typeof message === "object" && message !== null
      ? (message as { metadata?: { agent?: unknown } }).metadata?.agent
      : undefined;
  return typeof agent === "string" && agent.length > 0 ? agent : undefined;
}

export function kindOf(message: {
  role: string;
  content?: unknown;
  metadata?: { clayKind?: string };
}): TranscriptKind | null {
  if (message.metadata?.clayKind === "usage") return "usage";
  if (message.metadata?.clayKind === "error") return "error";
  if (message.role === "user") return "user";
  if (message.role === "reasoning") return "reasoning";
  if (message.role === "assistant") return "assistant";
  // Plan 109 I5: bounded tool rows ride the standard AG-UI tool role;
  // load_skill rows present as skill boxes.
  if (message.role === "tool") {
    return message.metadata?.clayKind === "skill" ? "skill" : "tool";
  }
  return null;
}

/** Tool-row metadata for box labels and Context-tab category counts. */
export function toolMeta(message: unknown): {
  toolName: string;
  skillName: string;
} {
  const meta =
    typeof message === "object" && message !== null
      ? (message as { metadata?: { toolName?: unknown; skillName?: unknown } })
          .metadata
      : undefined;
  return {
    toolName: typeof meta?.toolName === "string" ? meta.toolName : "",
    skillName: typeof meta?.skillName === "string" ? meta.skillName : "",
  };
}

/**
 * The producer of the turn before each index (plan 119 PF-2): what the old
 * per-turn `boxes.slice(0, index).reverse().find(...)` computed in O(n²) per
 * render is one forward pass here — `previous[i]` is the agent of the last
 * stamped turn *before* `i`, exactly the reverse-scan's answer. Callers memoize
 * the result on the box list, so a render costs O(n) total.
 */
export function previousAgents(
  boxes: readonly TranscriptBox[],
): Array<string | undefined> {
  const previous = new Array<string | undefined>(boxes.length);
  let last: string | undefined;
  for (let index = 0; index < boxes.length; index += 1) {
    const box = boxes[index];
    previous[index] = last;
    if (box?.agent) last = box.agent;
  }
  return previous;
}
