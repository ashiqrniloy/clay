// layout.json v2 projection. Rust re-validates on save/load; this module
// only shapes the document the bridge persists.

import {
  DEFAULT_PANE_ID,
  type PersistedSplitNode,
  type SplitTree,
  paneIds,
  singlePane,
  toPersisted,
  treeFromPersisted,
} from "./split-tree";

export interface PersistedTab {
  workspaceRoot: string;
  activePane: number;
  splitTree: PersistedSplitNode | null;
  slots: unknown[];
  panes: Record<string, string | null>;
}

export interface PersistedWindow {
  version: 2;
  activeTab: number | null;
  tabs: PersistedTab[];
}

export interface TabLayout {
  workspaceRoot: string;
  tree: SplitTree;
  /** paneId → workspace-relative path (null = empty pane). */
  documents: Map<number, string | null>;
}

export function windowFromTabs(
  tabs: TabLayout[],
  activeIndex: number | null,
): PersistedWindow {
  return {
    version: 2,
    activeTab: activeIndex,
    tabs: tabs.map((tab) => ({
      workspaceRoot: tab.workspaceRoot,
      activePane: tab.tree.activePaneId,
      splitTree: toPersisted(tab.tree.root),
      slots: [],
      panes: Object.fromEntries(
        paneIds(tab.tree.root).map((id) => [
          String(id),
          tab.documents.get(id) ?? null,
        ]),
      ),
    })),
  };
}

export function tabsFromWindow(raw: unknown): {
  tabs: TabLayout[];
  activeIndex: number | null;
} | null {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Partial<PersistedWindow>;
  if (
    value.version !== 2 ||
    !Array.isArray(value.tabs) ||
    value.tabs.length === 0
  ) {
    return null;
  }
  const tabs: TabLayout[] = [];
  for (const entry of value.tabs) {
    if (
      !entry ||
      typeof entry.workspaceRoot !== "string" ||
      !entry.workspaceRoot
    ) {
      continue;
    }
    const tree = treeFromPersisted(
      entry.splitTree ?? null,
      entry.activePane ?? DEFAULT_PANE_ID,
    );
    const documents = new Map<number, string | null>();
    if (entry.panes && typeof entry.panes === "object") {
      for (const [key, path] of Object.entries(entry.panes)) {
        const paneId = Number(key);
        if (!Number.isFinite(paneId)) continue;
        documents.set(paneId, typeof path === "string" && path ? path : null);
      }
    }
    tabs.push({ workspaceRoot: entry.workspaceRoot, tree, documents });
  }
  if (tabs.length === 0) return null;
  const activeIndex =
    typeof value.activeTab === "number" &&
    value.activeTab >= 0 &&
    value.activeTab < tabs.length
      ? value.activeTab
      : null;
  return { tabs, activeIndex };
}

export function emptyLayout(workspaceRoot: string): TabLayout {
  return {
    workspaceRoot,
    tree: singlePane(),
    documents: new Map([[DEFAULT_PANE_ID, null]]),
  };
}
