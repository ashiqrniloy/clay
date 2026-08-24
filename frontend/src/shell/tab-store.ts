// Window tab list projection. Server TabRegistry is authoritative for
// identity/order; this store is the React-facing snapshot plus local
// dirty/pending-close flags. Isolation: each tab has its own clientId.

export interface ShellTabState {
  /** Server tab id once the registry snapshot arrives; null while binding. */
  tabId: number | null;
  clientId: number;
  workspaceRoot: string;
  label: string;
  dirty: boolean;
  /** True after this tab's connection dropped independently. */
  disconnected: boolean;
}

export interface TabSnapshot {
  tabs: ShellTabState[];
  activeClientId: number | null;
  revision: number;
}

export function tabLabel(workspaceRoot: string): string {
  const trimmed = workspaceRoot.replace(/[/\\]+$/, "");
  const parts = trimmed.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || "Workspace";
}

export function emptyTabs(): TabSnapshot {
  return { tabs: [], activeClientId: null, revision: 0 };
}

export function upsertTab(
  snapshot: TabSnapshot,
  tab: ShellTabState,
): TabSnapshot {
  const tabs = snapshot.tabs.slice();
  const idx = tabs.findIndex((entry) => entry.clientId === tab.clientId);
  if (idx >= 0) tabs[idx] = { ...tabs[idx], ...tab };
  else tabs.push(tab);
  return {
    ...snapshot,
    tabs,
    activeClientId: snapshot.activeClientId ?? tab.clientId,
  };
}

export function removeTab(
  snapshot: TabSnapshot,
  clientId: number,
): TabSnapshot {
  const tabs = snapshot.tabs.filter((tab) => tab.clientId !== clientId);
  const activeClientId =
    snapshot.activeClientId === clientId
      ? (tabs[0]?.clientId ?? null)
      : snapshot.activeClientId;
  return { ...snapshot, tabs, activeClientId };
}

export function applyRegistry(
  snapshot: TabSnapshot,
  registry: {
    tabs: Array<{ tabId: number; clientId: number; workspaceRoot: string }>;
    active: number | null;
    revision: number;
  },
): TabSnapshot {
  if (registry.revision < snapshot.revision) return snapshot;
  const known = new Map(snapshot.tabs.map((tab) => [tab.clientId, tab]));
  const tabs = registry.tabs.map((entry) => {
    const prior = known.get(entry.clientId);
    return {
      tabId: entry.tabId,
      clientId: entry.clientId,
      workspaceRoot: entry.workspaceRoot,
      label: tabLabel(entry.workspaceRoot),
      dirty: prior?.dirty ?? false,
      disconnected: prior?.disconnected ?? false,
    };
  });
  const active =
    tabs.find((tab) => tab.tabId === registry.active)?.clientId ??
    tabs[0]?.clientId ??
    null;
  return { tabs, activeClientId: active, revision: registry.revision };
}

export function markDirty(
  snapshot: TabSnapshot,
  clientId: number,
  dirty: boolean,
): TabSnapshot {
  return {
    ...snapshot,
    tabs: snapshot.tabs.map((tab) =>
      tab.clientId === clientId ? { ...tab, dirty } : tab,
    ),
  };
}

export function createTabStore(initial: TabSnapshot = emptyTabs()) {
  let state = initial;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of [...listeners]) listener();
  };
  return {
    get: () => state,
    set(next: TabSnapshot) {
      state = next;
      notify();
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export type TabStore = ReturnType<typeof createTabStore>;
