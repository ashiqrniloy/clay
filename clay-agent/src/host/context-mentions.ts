/**
 * Plan 109 I7/I9 + plan 117 surfaces for ClayAgentHost (plan 130 task 2):
 * the live context inspector (categories + item drawer), @-mention
 * resolution (skills + file attachments), and the bounded workspace file
 * listing for the mention dropdown.
 */
import { readFile, readdir, stat } from "node:fs/promises";
import { realpathSync } from "node:fs";
import { join, sep } from "node:path";
import type { ContentBlock, SessionEntry, Skill, SystemPromptConfig, ToolDefinition } from "@arnilo/prism";
import type { ClayAgentHost } from "../host.js";
import { clipUtf8, optString, reqString, rpcError, type LiveSession } from "./internals.js";

// Plan 109 I7 context inspector: bounded previews, bounded items per
// category, and item content capped at the transcript entry budget.
const MAX_CONTEXT_ITEMS = 200;
const MAX_CONTEXT_PREVIEW_CHARS = 160;
const MAX_CONTEXT_ITEM_BYTES = 32 * 1024;
/** Plan 117 @-mentions: attached files cap at the same 256 KiB discipline as
 *  agent skill files; the workspace listing for the mention dropdown is
 *  bounded in entries and walk depth (client-side filter, debounced fetch). */
const MAX_MENTION_FILE_BYTES = 256 * 1024;
const MAX_LISTED_FILES = 200;
const MAX_LIST_WALK_DEPTH = 8;
const LIST_SKIPPED_DIRS = new Set([".git", "node_modules", "target", "dist", "build"]);
const IMAGE_MIME_BY_EXT = new Map([
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".jpeg", "image/jpeg"],
  [".gif", "image/gif"],
  [".webp", "image/webp"],
  [".bmp", "image/bmp"],
]);

/** Wire shape of one bounded context-inspector item reference. */
export interface ContextItemRef {
  id: string;
  title: string;
  preview: string;
}

/** Plan 109 I7: normalize the definition's system prompt config into
 *  contributions for the context inspector (`false` = disabled, absent =
 *  no explicit prompt). */
function systemPromptContributions(config: SystemPromptConfig | undefined): Array<{
  id: string;
  source?: string;
  text: string;
}> {
  if (!config) return [];
  const list = Array.isArray(config) ? config : [config];
  return list
    .filter((entry): entry is { id: string; source?: string; text: string } => typeof entry?.text === "string")
    .map((entry, index) => ({ id: entry.id || `prompt-${index}`, source: entry.source, text: entry.text }));
}

/** Mirror of Prism's `composeSystemPrompt` source ranks (system-prompts.js):
 *  the inspector lists system-prompt items in the order the model actually
 *  receives them. Unknown/custom sources sit after package, before app. */
const PROMPT_SOURCE_RANK: Readonly<Record<string, number>> = { user: 0, package: 1, app: 2, run: 3 };

function rankedSystemPromptContributions(config: SystemPromptConfig | undefined): Array<{
  id: string;
  source?: string;
  text: string;
}> {
  return systemPromptContributions(config)
    .map((contribution, index) => ({ contribution, index }))
    .sort(
      (a, b) =>
        (PROMPT_SOURCE_RANK[a.contribution.source ?? ""] ?? 1.5) -
          (PROMPT_SOURCE_RANK[b.contribution.source ?? ""] ?? 1.5) || a.index - b.index,
    )
    .map(({ contribution }) => contribution);
}

/** Friendly source labels for host-owned prompt layers (plan 117). */
const PROMPT_LAYER_LABELS: Readonly<Record<string, string>> = {
  "user-system-md": "user SYSTEM.md",
  "agents-md": "workspace AGENTS.md",
};

function promptLayerLabel(id: string): string {
  return PROMPT_LAYER_LABELS[id] ?? id;
}

/**
 * Plan 109 I7: live context inspector over the active session branch.
 * Active context = the branch chain from the latest `kind: "compaction"`
 * entry onward (compacted-away items leave the list; the summary item
 * stays) — a branch checkout rebinds `entries()` to the checked-out
 * branch, so the same derivation restores the earlier context. Builds
 * on demand (never on keystroke/paint paths), redacted through the
 * session redactor, previews and content bounded.
 */
export async function sessionContext(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const entries = await live.session.entries();
  const itemId = optString(params, "itemId");
  if (itemId) {
    const item = contextItem(host, live, entries, itemId);
    if (!item) throw rpcError(-32000, `Unknown context item: ${itemId}`);
    return { sessionId, itemId, ...item };
  }
  return {
    sessionId,
    // Cheap invalidation version: every entry append (run finished,
    // compaction, steer, skill row) bumps it; the client refetches when
    // its transcript length changes.
    version: entries.length,
    categories: contextCategories(host, live, entries),
  };
}

/** Active-context slice: from the latest compaction entry onward. */
export function activeContextEntries(entries: readonly SessionEntry[]): readonly SessionEntry[] {
  let boundary = -1;
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    if (entries[index]?.kind === "compaction") {
      boundary = index;
      break;
    }
  }
  return boundary === -1 ? entries : entries.slice(boundary);
}

/**
 * Categorized active context. One category model feeds counts and items
 * (the item list is capped; `count` stays the real number so the client
 * can show "…and N more").
 */
interface ContextBuckets {
  system: ContextItemRef[];
  user: ContextItemRef[];
  skills: ContextItemRef[];
  tools: ContextItemRef[];
  thinking: ContextItemRef[];
  agent: ContextItemRef[];
  summaries: ContextItemRef[];
}

export function contextCategories(
  host: ClayAgentHost,
  live: LiveSession,
  entries: readonly SessionEntry[],
): Array<{ kind: string; label: string; count: number; items: Array<ContextItemRef> }> {
  const buckets: ContextBuckets = {
    system: systemPromptItems(live),
    user: [],
    skills: [],
    tools: [],
    thinking: [],
    agent: [],
    summaries: [],
  };
  for (const entry of activeContextEntries(entries)) {
    const redacted = host.redactor.redact(entry);
    if (redacted.kind === "summary" || redacted.kind === "compaction") {
      buckets.summaries.push({
        id: redacted.id,
        title: "Compaction summary",
        preview: clipUtf8(redacted.summary ?? "", MAX_CONTEXT_PREVIEW_CHARS),
      });
      continue;
    }
    if (redacted.kind === "message" && redacted.message) {
      collectMessageItems(redacted, buckets);
      continue;
    }
    // Loaded skills: load_skill tool calls are persisted as tool events.
    buckets.skills.push(...skillEventItems(redacted));
  }
  const build = (kind: string, label: string, items: ContextItemRef[]) => ({
    kind,
    label,
    count: items.length,
    items: items.slice(-MAX_CONTEXT_ITEMS),
  });
  return [
    build("systemPrompt", "System prompt", buckets.system),
    build("userMessage", "User prompts", buckets.user),
    build("skill", "Skills loaded", buckets.skills),
    build("toolOutput", "Tool calls + outputs", buckets.tools),
    build("thinking", "Thinking", buckets.thinking),
    build("agentMessage", "Agent messages", buckets.agent),
    build("compactionSummary", "Compaction summaries", buckets.summaries),
  ];
}

/** The composed system prompt the model actually receives: base instructions
 *  first, then rank-ordered contributions (plan 117 — the group is no longer
 *  empty for profiles with only base instructions). */
function systemPromptItems(live: LiveSession): ContextItemRef[] {
  const items: ContextItemRef[] = [];
  const baseInstructions = live.agent.config.instructions;
  if (typeof baseInstructions === "string" && baseInstructions.length > 0) {
    items.push({
      id: "system-prompt-base",
      title: "System prompt (base instructions)",
      preview: clipUtf8(baseInstructions, MAX_CONTEXT_PREVIEW_CHARS),
    });
  }
  for (const [index, contribution] of rankedSystemPromptContributions(live.systemPrompt).entries()) {
    items.push({
      id: `system-prompt-${index}`,
      title: `System prompt (${promptLayerLabel(contribution.id)})`,
      preview: clipUtf8(contribution.text, MAX_CONTEXT_PREVIEW_CHARS),
    });
  }
  return items;
}

/** One transcript message: content blocks land in their category — thinking,
 *  tool call/output (load_skill calls feed both the skill and tool
 *  categories; skill tool-execution events are not persisted as entries),
 *  and user/agent text. */
function collectMessageItems(redacted: SessionEntry, buckets: ContextBuckets): void {
  const message = redacted.message;
  if (!message) return;
  message.content.forEach((block, index) => {
    const blockId = `${redacted.id}#${index}`;
    if (block.type === "thinking") {
      buckets.thinking.push({
        id: blockId,
        title: "Thinking",
        preview: clipUtf8(block.text, MAX_CONTEXT_PREVIEW_CHARS),
      });
      return;
    }
    if (block.type === "tool_call") {
      if (block.name === "load_skill") {
        const skillName =
          typeof (block.arguments as Record<string, unknown> | undefined)?.name === "string"
            ? ((block.arguments as Record<string, unknown>).name as string)
            : "unknown";
        buckets.skills.push({
          id: blockId,
          title: `load_skill: ${skillName}`,
          preview: clipUtf8(JSON.stringify(block.arguments), MAX_CONTEXT_PREVIEW_CHARS),
        });
      }
      buckets.tools.push({
        id: blockId,
        title: `tool call: ${block.name}`,
        preview: clipUtf8(JSON.stringify(block.arguments), MAX_CONTEXT_PREVIEW_CHARS),
      });
      return;
    }
    if (block.type === "tool_result") {
      buckets.tools.push({
        id: blockId,
        title: `tool output: ${block.name}`,
        preview: clipUtf8(
          block.error ? `error: ${block.error.message ?? ""}` : JSON.stringify(block.result ?? null),
          MAX_CONTEXT_PREVIEW_CHARS,
        ),
      });
      return;
    }
    if (block.type !== "text" || !block.text) return;
    if (message.role === "user") {
      buckets.user.push({ id: blockId, title: "User prompt", preview: clipUtf8(block.text, MAX_CONTEXT_PREVIEW_CHARS) });
    } else if (message.role === "assistant") {
      buckets.agent.push({ id: blockId, title: "Agent message", preview: clipUtf8(block.text, MAX_CONTEXT_PREVIEW_CHARS) });
    }
  });
}

/** Loaded skills recorded as tool-execution events rather than message
 *  blocks (legacy entries): the `load_skill` call's arguments are the skill
 *  body preview. */
function skillEventItems(redacted: SessionEntry): ContextItemRef[] {
  if (redacted.kind !== "event" || !redacted.event) return [];
  const event = redacted.event as Record<string, unknown>;
  if (
    event.type !== "tool_execution_started" ||
    typeof event.call !== "object" || event.call === null ||
    (event.call as Record<string, unknown>).name !== "load_skill"
  ) {
    return [];
  }
  const call = (event.call as Record<string, unknown>).arguments as Record<string, unknown> | undefined;
  const name = typeof call?.name === "string" ? call.name : "unknown";
  return [{
    id: redacted.id,
    title: `load_skill: ${name}`,
    preview: clipUtf8(JSON.stringify(call ?? {}), MAX_CONTEXT_PREVIEW_CHARS),
  }];
}

/** One context item's full redacted content for the drawer detail. */
export function contextItem(
  host: ClayAgentHost,
  live: LiveSession,
  entries: readonly SessionEntry[],
  itemId: string,
): { kind: string; title: string; content: string } | undefined {
  // System-prompt items live outside the transcript.
  if (itemId === "system-prompt-base") {
    const base = live.agent.config.instructions;
    if (typeof base !== "string" || base.length === 0) return undefined;
    return {
      kind: "systemPrompt",
      title: "System prompt (base instructions)",
      content: clipUtf8(base, MAX_CONTEXT_ITEM_BYTES),
    };
  }
  if (itemId.startsWith("system-prompt-")) {
    const index = Number(itemId.slice("system-prompt-".length));
    const contribution = rankedSystemPromptContributions(live.systemPrompt)[index];
    if (!contribution) return undefined;
    return {
      kind: "systemPrompt",
      title: `System prompt (${promptLayerLabel(contribution.id)})`,
      content: clipUtf8(contribution.text, MAX_CONTEXT_ITEM_BYTES),
    };
  }
  const [entryId, blockIndex] = itemId.split("#");
  const entry = activeContextEntries(entries).find((candidate) => candidate.id === entryId);
  if (!entry) return undefined;
  const redacted = host.redactor.redact(entry);
  const clip = (text: string) => clipUtf8(text, MAX_CONTEXT_ITEM_BYTES);
  if (redacted.kind === "summary" || redacted.kind === "compaction") {
    return { kind: "compactionSummary", title: "Compaction summary", content: clip(redacted.summary ?? "") };
  }
  if (redacted.kind === "event" && redacted.event) {
    const event = redacted.event as Record<string, unknown>;
    if (event.type === "tool_execution_started" && typeof event.call === "object" && event.call !== null) {
      return {
        kind: "skill",
        title: "load_skill",
        content: clip(JSON.stringify((event.call as Record<string, unknown>).arguments ?? {})),
      };
    }
    return undefined;
  }
  if (redacted.kind !== "message" || !redacted.message) return undefined;
  const index = Number(blockIndex);
  const block = Number.isInteger(index) ? redacted.message.content[index] : undefined;
  if (!block) return undefined;
  if (block.type === "thinking") {
    return { kind: "thinking", title: "Thinking", content: clip(block.text) };
  }
  if (block.type === "tool_call") {
    return {
      kind: "toolOutput",
      title: `tool call: ${block.name}`,
      content: clip(JSON.stringify({ name: block.name, arguments: block.arguments })),
    };
  }
  if (block.type === "tool_result") {
    return {
      kind: "toolOutput",
      title: `tool output: ${block.name}`,
      content: clip(
        block.error ? `error: ${block.error.message ?? JSON.stringify(block.error)}` : JSON.stringify(block.result ?? null),
      ),
    };
  }
  if (block.type === "text") {
    const kind = redacted.message.role === "user" ? "userMessage" : "agentMessage";
    const title = redacted.message.role === "user" ? "User prompt" : "Agent message";
    return { kind, title, content: clip(block.text) };
  }
  return undefined;
}

/** Plan 117 @-mentions: parse `@skill:` / `@file:` tokens out of the raw
 *  prompt. Valid skill mentions join the session's loaded set (the same
 *  LoadedSkillSet the load_skill tool mutates — bodies re-resolve from
 *  the registry, persistence rides the snapshot names-only), append to
 *  the session's mention set (rides every run's skills option), and the
 *  prompt gains one instruction line. Resolvable file mentions become
 *  content blocks (images as base64 image content, text files as a
 *  fenced text block) resolved server-side inside the workspace root.
 *  Everything unresolvable stays plain text: chat-safe, no throw. */
export async function resolveMentions(
  host: ClayAgentHost,
  text: string,
  live: LiveSession,
): Promise<{ prompt: string; attachments: ContentBlock[] }> {
  const toolNames = new Set(live.tools.map((tool) => tool.name));
  const loaded: Skill[] = [];
  // Mention tokens are whitespace-delimited, so skill names with spaces
  // cannot be mentioned (the dropdown embeds dir-form names; ceiling:
  // add an escape syntax if space-y names ever need mentioning).
  // # ponytail: no-space mention names; escape syntax if ever needed
  let prompt = text.replace(/@skill:([A-Za-z0-9_-]+)/g, (token, raw) => {
    const name = String(raw).trim();
    const skill = host.skills.get(name);
    // Same activation discipline as resolveActiveSkills: a skill whose
    // tools are unavailable never loads (a hand-typed mention is still
    // gated; catalog names already passed this filter).
    if (skill === undefined) return token;
    if (skill.toolNames && !skill.toolNames.every((tool) => toolNames.has(tool))) return token;
    if (!loaded.some((entry) => entry.name === name)) loaded.push(skill);
    return `\`${name}\``;
  });
  const attachments: ContentBlock[] = [];
  const fileMatches = [...prompt.matchAll(/@file:([^\s@]+)/g)];
  if (fileMatches.length > 0) {
    const blocks = new Map<string, ContentBlock | undefined>();
    for (const match of fileMatches) {
      const relPath = String(match[1]);
      if (!blocks.has(relPath)) blocks.set(relPath, await readMentionFile(relPath, live.workspaceRoot));
    }
    prompt = prompt.replace(/@file:([^\s@]+)/g, (token, raw) => {
      const block = blocks.get(String(raw));
      if (block === undefined) return token;
      attachments.push(block);
      return "";
    });
  }
  if (loaded.length > 0) {
    // RuntimeAgentSession exposes restoreLoadedSkills (public since plan 015
    // task 4); the AgentSession interface predates it, hence the narrow cast.
    (live.session as unknown as { restoreLoadedSkills(names: readonly string[]): void }).restoreLoadedSkills(
      loaded.map((skill) => skill.name),
    );
    for (const skill of loaded) {
      if (!live.mentionSkills.some((entry) => entry.name === skill.name)) live.mentionSkills.push(skill);
    }
    prompt = `[skills loaded by mention: ${loaded.map((skill) => skill.name).join(", ")}]

${prompt}`;
  }
  return { prompt, attachments };
}

/** Read one @file mention inside the workspace root. Returns undefined
 *  (token stays plain text) for paths that escape the root, are missing,
 *  unreadable, or — for images — oversized. Images attach as base64
 *  image content; other files as a bounded fenced text block. */
async function readMentionFile(relPath: string, workspaceRoot: string): Promise<ContentBlock | undefined> {
  const full = join(workspaceRoot, relPath);
  try {
    const rootReal = realpathSync(workspaceRoot);
    const fileReal = realpathSync(full);
    if (!fileReal.startsWith(rootReal + sep)) return undefined;
    const info = await stat(fileReal);
    if (!info.isFile() || info.size > MAX_MENTION_FILE_BYTES) return undefined;
    const ext = relPath.slice(relPath.lastIndexOf(".")).toLowerCase();
    const mime = IMAGE_MIME_BY_EXT.get(ext);
    if (mime) {
      const data = (await readFile(fileReal)).toString("base64");
      return { type: "image", mimeType: mime, data, name: relPath };
    }
    const body = (await readFile(fileReal, "utf8")).slice(0, MAX_MENTION_FILE_BYTES);
    return { type: "text", text: `\n\n[attached file: ${relPath}]\n\`\`\`\n${body}\n\`\`\`` };
  } catch {
    return undefined;
  }
}

/** Plan 117: bounded workspace file list for the @-mention dropdown —
 *  workspace-relative paths, dotfiles and build dirs skipped, capped in
 *  entries and depth. Absent/unreachable root ⇒ empty list (never a
 *  throw; the dropdown degrades to skills-only). */
export async function workspaceListFiles(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = await host.ensureLive(sessionId);
  const files: string[] = [];
  const walk = async (dir: string, depth: number): Promise<void> => {
    if (depth > MAX_LIST_WALK_DEPTH || files.length >= MAX_LISTED_FILES) return;
    let entries;
    try {
      entries = await readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
      if (files.length >= MAX_LISTED_FILES) return;
      const rel = join(dir, entry.name).slice(live.workspaceRoot.length + 1);
      if (entry.isDirectory()) {
        if (!LIST_SKIPPED_DIRS.has(entry.name) && !entry.name.startsWith(".")) await walk(join(dir, entry.name), depth + 1);
      } else if (entry.isFile() && !entry.name.startsWith(".")) {
        files.push(rel);
      }
    }
  };
  await walk(live.workspaceRoot, 1);
  return { files: files.slice(0, MAX_LISTED_FILES) };
}
