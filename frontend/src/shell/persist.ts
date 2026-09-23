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

import type { TabAgent, TabView } from "./tab-store";

export interface PersistedTab {
  workspaceRoot: string;
  /** Tab agent identity (plan 118 task 33); absent/null when none. */
  agent?: TabAgent | null;
  /** Which view was up; absent means `workspace` (older v2 documents). */
  view?: TabView;
  /** Rail visibility for this tab; absent means visible (plan 118 task E2). */
  railVisible?: boolean;
  /** Agent-inspector visibility for this tab; absent means visible. */
  inspectorVisible?: boolean;
  /** Agent-lane visibility for this tab; absent means visible (plan 124). */
  laneVisible?: boolean;
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
  agent: TabAgent | null;
  view: TabView;
  /** Per-tab layout visibility (plan 118 task E2; the lane is plan 124). */
  railVisible: boolean;
  inspectorVisible: boolean;
  laneVisible: boolean;
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
      agent: tab.agent,
      view: tab.view,
      railVisible: tab.railVisible,
      inspectorVisible: tab.inspectorVisible,
      laneVisible: tab.laneVisible,
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

/** A persisted agent is inert display data; malformed entries are dropped
 *  rather than half-adopted. */
function agentFrom(raw: unknown): TabAgent | null {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Partial<TabAgent>;
  if (typeof value.type !== "string" || !value.type) return null;
  return {
    type: value.type,
    configRoot: typeof value.configRoot === "string" ? value.configRoot : "",
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
    if (!entry || typeof entry !== "object") continue;
    const agent = agentFrom(entry.agent);
    const workspaceRoot =
      typeof entry.workspaceRoot === "string" ? entry.workspaceRoot : "";
    // A tab with neither half is not a tab (Rust applies the same rule); a
    // document with no folder is still real, so one half is enough.
    if (!workspaceRoot && !agent) continue;
    const view: TabView = entry.view === "agent" ? "agent" : "workspace";
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
    tabs.push({
      workspaceRoot,
      agent,
      view,
      // Absent means visible: the tab's own default, and what a v2 document
      // written before the fields existed means.
      railVisible: entry.railVisible !== false,
      inspectorVisible: entry.inspectorVisible !== false,
      laneVisible: entry.laneVisible !== false,
      tree,
      documents,
    });
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
    agent: null,
    view: "workspace",
    railVisible: true,
    inspectorVisible: true,
    laneVisible: true,
    tree: singlePane(),
    documents: new Map([[DEFAULT_PANE_ID, null]]),
  };
}
