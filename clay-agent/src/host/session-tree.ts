/**
 * Branch/tree surfaces for ClayAgentHost (plan 130 task 2): checkout with
 * branch summaries (deterministic preview + background LLM refine), the
 * /tree summary + bounded text render, and the branch-path helpers shared
 * with session.load.
 */
import {
  createAgent,
  createMemorySessionStore,
  createProviderResolver,
  createSessionEntry,
  listSessionBranches,
  type SessionEntry,
} from "@arnilo/prism";
import type { ClayAgentHost } from "../host.js";
import { clipUtf8, MAX_LOAD_ENTRIES, reqString, rpcError, type LiveSession } from "./internals.js";

/** Bounded branch-summary text (plan 108 task 10) and in-place tree render. */
const MAX_BRANCH_SUMMARY_BYTES = 2_048;
const MAX_TREE_RENDER_BYTES = 8_192;
const MAX_TREE_RENDER_ENTRIES = 60;

/** Path from the tree root to `entryId` (inclusive), oldest first. Used by
 *  `session.load { entryId }` for search-result opens (plan 108 task 11):
 *  read-only view of that branch — never mutates the tree. Unknown entry
 *  ids return undefined so the caller falls back to the default tail. */
export function branchEntries(
  entries: readonly SessionEntry[],
  entryId: string,
): SessionEntry[] | undefined {
  const byId = new Map(entries.map((entry) => [entry.id, entry]));
  if (!byId.has(entryId)) return undefined;
  const path: SessionEntry[] = [];
  let current: string | undefined = entryId;
  while (current) {
    const entry = byId.get(current);
    if (!entry) break;
    path.unshift(entry);
    current = entry.parentId;
  }
  return path;
}

/** Entry text preview for branch summaries, bounded and redaction-safe
 *  (redaction happens on the caller side via entry redaction upstream). */
function branchText(entries: readonly SessionEntry[], fromLeaf: string | undefined, branchPointId: string): string {
  return branchTextForPrompt(entries, fromLeaf, branchPointId, 160);
}

/** Bounded prompt form: role/kind per entry, ≤ `previewChars` per line. */
function branchTextForPrompt(
  entries: readonly SessionEntry[],
  fromLeaf: string | undefined,
  branchPointId: string,
  previewChars = 200,
): string {
  const lines: string[] = [];
  let current: string | undefined = fromLeaf;
  let bytes = 0;
  while (current && current !== branchPointId) {
    const entry = entries.find((item) => item.id === current);
    if (!entry) break;
    const content: unknown = entry.message?.content;
    const body =
      entry.summary ??
      entry.label ??
      (typeof content === "string"
        ? content
        : Array.isArray(content)
          ? content
              .map((part) => (typeof part === "object" && part !== null && "text" in part ? String((part as { text: unknown }).text) : ""))
              .join("")
          : "");
    const line = `[${entry.kind}] ${body.slice(0, previewChars)}`;
    bytes += line.length + 1;
    if (bytes > 4096 || lines.length >= 24) {
      lines.unshift("…(older entries omitted)");
      break;
    }
    lines.unshift(line);
    current = entry.parentId;
  }
  return lines.join("\n");
}

function shortId(id: string | undefined): string {
  return (id ?? "?").slice(0, 8);
}

/** Checkout the conversation leaf AND restore the document checkpoint
 *  recorded at that entry (decision 2200): server restores document
 *  versions first, then the session history rebuilds. Fails closed if the
 *  server cannot restore (e.g. lease held elsewhere) — leaf stays put.
 *  Before the leaf moves, the abandoned path (current leaf back to the
 *  branch point) gets a summary entry parented at the branch point — on
 *  the NEW branch, pi's branch_summary model — immediately with a
 *  deterministic preview, refined in the background by an LLM run that
 *  replaces the leaf when the session has not moved on. Nothing is ever
 *  deleted: the abandoned side keeps its entries. */
export async function sessionCheckout(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const entryId = reqString(params, "entryId");
  const live = await host.ensureLive(sessionId);
  await host.request("checkpoint.restore", { sessionId, entryId });
  const fromLeaf = live.session.leafId;
  const preview = await appendBranchSummary(host, live, sessionId, entryId, fromLeaf);
  await live.session.checkout(preview.entryId);
  void refineBranchSummary(host, live, sessionId, entryId, fromLeaf, preview);
  return { sessionId, leafId: live.session.leafId, summaryEntryId: preview.entryId, summarizing: true };
}

/** Append a summary entry (kind "summary") for the abandoned path from
 *  `fromLeaf` back to `branchPointId` (exclusive). Deterministic bounded
 *  preview; the LLM refine lands as a sibling entry later. */
async function appendBranchSummary(
  host: ClayAgentHost,
  live: LiveSession,
  sessionId: string,
  branchPointId: string,
  fromLeaf: string | undefined,
): Promise<{ entryId: string }> {
  const entries = await live.session.entries();
  const text = branchText(entries, fromLeaf, branchPointId);
  const summaryEntry = createSessionEntry({
    sessionId,
    parentId: branchPointId,
    kind: "summary",
    summary: clipUtf8(text, MAX_BRANCH_SUMMARY_BYTES) || "(empty branch)",
    data: { fromId: fromLeaf, pending: true },
  });
  await host.persistence.append(summaryEntry);
  return { entryId: summaryEntry.id };
}

/** Background LLM refine (mock-provider testable): one-shot no-tool run
 *  over the abandoned path; the refined entry re-roots the leaf when the
 *  session has not moved on. Never throws into the checkout path. */
async function refineBranchSummary(
  host: ClayAgentHost,
  live: LiveSession,
  sessionId: string,
  branchPointId: string,
  fromLeaf: string | undefined,
  preview: { entryId: string },
): Promise<void> {
  host.branchSummarizing.set(sessionId, branchPointId);
  try {
    const entries = await live.session.entries();
    const prompt = branchTextForPrompt(entries, fromLeaf, branchPointId);
    const runModel = host.kernel.registries.models.get(live.provider, live.model) ?? {
      provider: live.provider,
      model: live.model,
    };
    const worker = createAgent({
      id: "branch-summary",
      model: runModel,
      providerSource: createProviderResolver(host.kernel.registries.providers),
      store: createMemorySessionStore(),
      redactor: host.redactor,
    });
    const workerSession = worker.createSession();
    const result = await workerSession.run(
      `Summarize the abandoned branch of this coding session in at most 6 sentences,\n`
        + `covering what was attempted and why it was left behind. Entries:\n${prompt}`,
    );
    const text = clipUtf8(result.text, MAX_BRANCH_SUMMARY_BYTES);
    if (!text) return;
    const refined = createSessionEntry({
      sessionId,
      parentId: branchPointId,
      kind: "summary",
      summary: text,
      data: { fromId: fromLeaf, pending: false },
    });
    await host.persistence.append(refined);
    // Re-root only when nothing else moved the leaf meanwhile.
    const current = host.live.get(sessionId);
    if (current && current.session.leafId === preview.entryId) {
      await current.session.checkout(refined.id);
    }
  } catch {
    // The deterministic preview stays; the summary is never lost.
  } finally {
    if (host.branchSummarizing.get(sessionId) === branchPointId) {
      host.branchSummarizing.delete(sessionId);
    }
  }
}

/** Branch summary for /tree (plan 108 task 10): every entry's id/parent/kind
 *  plus a short redacted preview, the branch list, stored branch summaries,
 *  checkpoint flags (server authority), and the in-flight summary markers.
 *  Bounded by the store's own entry list. */
export async function sessionTreeSummary(host: ClayAgentHost, sessionId: string): Promise<unknown> {
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const entries = await live.session.entries();
  const checkpointSet = await checkpointedEntryIds(host, sessionId);
  const summaries: Array<Record<string, unknown>> = [];
  const treeEntries = entries.slice(-MAX_LOAD_ENTRIES).map((entry) => treeEntryRow(host, entry, checkpointSet, summaries));
  const branches = listSessionBranches(entries)
    .slice(-MAX_LOAD_ENTRIES)
    .map((branch) => ({ leafId: branch.leafId, entries: branch.entries.length }));
  return {
    sessionId,
    leafId: live.session.leafId,
    entries: treeEntries,
    summaries,
    branches,
    ...(host.branchSummarizing.has(sessionId)
      ? { summarizing: host.branchSummarizing.get(sessionId) }
      : {}),
  };
}

/** Server-authoritative checkpoint flags for the tree. Advisory data: never
 *  block the tree on a slow reverse consumer (2 s budget), and a server
 *  without the method still gets a tree. */
async function checkpointedEntryIds(host: ClayAgentHost, sessionId: string): Promise<Set<string>> {
  try {
    const request = host.request("checkpoint.list", { sessionId });
    const budget = new Promise<{ entryIds?: unknown }>((resolve) => {
      setTimeout(() => resolve({}), 2_000);
    });
    const listed = (await Promise.race([request, budget])) as { entryIds?: unknown };
    if (Array.isArray(listed?.entryIds)) {
      return new Set(listed.entryIds.filter((id): id is string => typeof id === "string"));
    }
  } catch {
    // Checkpoint flags are advisory; the tree renders without them.
  }
  return new Set();
}

/** One bounded, redacted tree row; a stored summary also lands in the
 *  sibling `summaries` list (latest refine wins there by append order). */
function treeEntryRow(
  host: ClayAgentHost,
  entry: SessionEntry,
  checkpointSet: Set<string>,
  summaries: Array<Record<string, unknown>>,
): Record<string, unknown> {
  const redacted = host.redactor.redact(entry);
  if (redacted.kind === "summary" && redacted.summary) {
    const data = redacted.data && typeof redacted.data === "object" ? (redacted.data as Record<string, unknown>) : {};
    summaries.push({
      entryId: redacted.id,
      parentId: redacted.parentId,
      fromId: data.fromId,
      text: redacted.summary,
      pending: data.pending === true,
    });
  }
  const preview = entryPreview(redacted);
  return {
    id: redacted.id,
    ...(redacted.parentId ? { parentId: redacted.parentId } : {}),
    kind: redacted.kind,
    ...(checkpointSet.has(redacted.id) ? { checkpointed: true } : {}),
    ...(preview !== undefined && preview !== "" ? { preview } : {}),
  };
}

/** Row preview: the entry label/summary, else the first 80 chars of a text
 *  message's joined content. */
function entryPreview(redacted: SessionEntry): string | undefined {
  const content: unknown = redacted.message?.content;
  return (
    redacted.label ??
    redacted.summary ??
    (typeof content === "string"
      ? content.slice(0, 80)
      : Array.isArray(content)
        ? content
            .map((part) =>
              typeof part === "object" && part !== null && "text" in part
                ? String((part as { text: unknown }).text)
                : "",
            )
            .join("")
            .slice(0, 80)
        : undefined)
  );
}

/** Render the /tree payload as bounded transcript text (pi-parity in-place
 *  view: entries with ids, leaf marker, checkpoints, branch summaries). */
export async function renderTreeText(host: ClayAgentHost, sessionId: string): Promise<string> {
  const tree = (await sessionTreeSummary(host, sessionId)) as {
    leafId?: string;
    entries?: Array<Record<string, unknown>>;
    summaries?: Array<Record<string, unknown>>;
    summarizing?: string;
  };
  const lines: string[] = [`tree (leaf ${shortId(tree.leafId)}):`];
  for (const entry of (tree.entries ?? []).slice(-MAX_TREE_RENDER_ENTRIES)) {
    const marker = entry.id === tree.leafId ? " ←" : "";
    const checkpoint = entry.checkpointed === true ? " ⚑" : "";
    const preview = typeof entry.preview === "string" ? ` ${entry.preview}` : "";
    lines.push(`${entry.id === tree.leafId ? "●" : "○"} ${String(entry.id).slice(0, 8)} [${String(entry.kind)}]${preview}${checkpoint}${marker}`);
  }
  for (const summary of tree.summaries ?? []) {
    lines.push(
      `summary at ${String(summary.entryId).slice(0, 8)} (from ${String(summary.fromId ?? "?").slice(0, 8)}${summary.pending === true ? ", pending" : ""}): ${String(summary.text).slice(0, 160)}`,
    );
  }
  if (typeof tree.summarizing === "string") {
    lines.push(`(summarizing branch at ${tree.summarizing.slice(0, 8)}…)`);
  }
  return clipUtf8(lines.join("\n"), MAX_TREE_RENDER_BYTES);
}
