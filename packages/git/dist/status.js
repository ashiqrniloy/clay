// @clay/git read-only status adapter.
//
// Consumes the server-owned `clay:git` facade (no shell/network/mutation
// authority reaches this code) and produces an inert SDUI status tree. The
// adapter never emits absolute repository/workspace paths, control
// characters, or executable code: only sanitized branch/dirty/count labels.

import { serverListGitStatuses } from "clay:git";

// ponytail: 160-char cap mirrors the Markdown status sanitizer; Git branch
// names and short SHAs are short, so this only guards pathological input.
const MAX_STATUS_TEXT = 160;

function sanitizeText(value, fallback) {
  const text = String(value ?? fallback)
    .replace(/[\u0000-\u001f\u007f]+/g, " ")
    .replace(/[A-Za-z]:[\\/][^\s]+/g, "[path]")
    .replace(/(?:~|\.\.?)[\\/][^\s]+/g, "[path]")
    .replace(/\b[\w.-]+(?:[\\/][\w.-]+)+\b/g, "[path]")
    .trim();
  return text.length > 0 ? text.slice(0, MAX_STATUS_TEXT) : fallback;
}

function rootLabel(workspaceRoot) {
  const raw = String(workspaceRoot ?? "workspace").replace(/[\u0000-\u001f\u007f]+/g, " ").trim();
  const base = raw.split(/[\\/]/).filter(Boolean).pop() ?? "workspace";
  return sanitizeText(base, "workspace");
}

function headLabel(head) {
  if (!head) {
    return "HEAD: unknown";
  }
  switch (head.kind) {
    case "branch":
      return `Branch: ${sanitizeText(head.name, "main")}`;
    case "detached":
      return `Detached: ${sanitizeText(head.shortSha, "HEAD")}`;
    case "unborn":
      return "Branch: (no commits yet)";
    default:
      return "HEAD: unknown";
  }
}

function dirtyLabel(snapshot) {
  if (!snapshot) {
    return "Status: not refreshed";
  }
  if (!snapshot.dirty) {
    return "Status: clean";
  }
  const count = Number(snapshot.changedFileCount ?? 0);
  return `Status: ${count > 0 ? `${count} changed` : "dirty"}`;
}

function refreshLabel(refreshState) {
  if (!refreshState) {
    return "Refresh: idle";
  }
  switch (refreshState.kind) {
    case "idle":
      return "Refresh: idle";
    case "refreshing":
      return "Refresh: refreshing";
    case "last-success":
      return "Refresh: current";
    case "last-error": {
      const status = refreshState.status && refreshState.status.kind
        ? refreshState.status.kind
        : "error";
      return `Refresh: ${status}`;
    }
    default:
      return "Refresh: idle";
  }
}

// Build the package-owned status model from cached Git statuses. Public so the
// load entry and tests can assert the data path without re-querying the facade.
export function gitStatusModel(statuses = []) {
  return statuses.map((entry) => ({
    workspaceRootId: String(entry.workspaceRootId ?? ""),
    rootLabel: rootLabel(entry.workspaceRoot),
    head: entry.snapshot ? headLabel(entry.snapshot.head) : "HEAD: unknown",
    dirty: entry.snapshot ? dirtyLabel(entry.snapshot) : "Status: not refreshed",
    refresh: refreshLabel(entry.refreshState)
  }));
}

// Build an inert SDUI tree (labels only, no action targets, no callbacks) from
// the sanitized status model. Clay owns rendering; this never runs in a paint
// or keypress hot path.
export function buildGitStatusTree(claySdui, statuses = []) {
  const { definePanel, defineLabel, defineStack } = claySdui;
  const model = gitStatusModel(statuses);

  const children = model.length > 0
    ? model.flatMap((root) => [
        defineLabel({ id: `git.root.${root.workspaceRootId}`, text: root.rootLabel }),
        defineLabel({ id: `git.head.${root.workspaceRootId}`, text: root.head }),
        defineLabel({ id: `git.dirty.${root.workspaceRootId}`, text: root.dirty }),
        defineLabel({ id: `git.refresh.${root.workspaceRootId}`, text: root.refresh })
      ])
    : [defineLabel({ id: "git.empty", text: "No workspace roots" })];

  return definePanel({
    id: "git.status.panel",
    title: "Git",
    children: [
      defineStack({ id: "git.status.stack", children })
    ]
  });
}

// Fetch cached Git statuses from the server-owned facade and publish the
// read-only status tree. Returns the sanitized model for callers/tests.
export async function publishGitStatus(clay) {
  const statuses = await serverListGitStatuses();
  const tree = buildGitStatusTree(clay.sdui, statuses);
  await clay.sdui.publishTree(tree);
  return gitStatusModel(statuses);
}
