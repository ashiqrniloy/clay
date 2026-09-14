// Window tab list projection. Server TabRegistry is authoritative for
// identity/order; this store is the React-facing snapshot plus local
// dirty/pending-close flags. Isolation: each tab has its own clientId.

/** Which of a tab's two views is up (DESIGN.md §12). The tab is the unit:
 *  one workspace and one agent, one view at a time. */
export type TabView = "workspace" | "agent";

/** The agent half of a tab (plan 118 task 33). Inert identity data: `type` is
 *  the agent's directory name (its registry key), `configRoot` its config
 *  folder. It grants nothing — rendering an agent surface still needs the
 *  package's exact provenance and trusted-domain status. */
export interface TabAgent {
  type: string;
  configRoot: string;
}

export interface ShellTabState {
  /** Server tab id once the registry snapshot arrives; null while binding. */
  tabId: number | null;
  clientId: number;
  /** The folder this tab is about. Empty while the tab is uncommitted — the
   *  launcher's state, where nothing has been picked yet. The *session* root
   *  the server actually rooted the tab at lives on the runtime, because the
   *  server always has one (configured root or cwd fallback). */
  workspaceRoot: string;
  agent: TabAgent | null;
  view: TabView;
  /** Agent session is running in this tab: the strip's marker pulses. */
  agentBusy: boolean;
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

/** Strip label: the folder's basename, else the agent's type, else the
 *  approved faint "New tab" of an uncommitted launcher tab. */
export function tabLabel(
  workspaceRoot: string,
  agent: TabAgent | null = null,
): string {
  const trimmed = workspaceRoot.replace(/[/\\]+$/, "");
  const parts = trimmed.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || agent?.type || "New tab";
}

/** Tab tooltip: the full path, and the attached agent when there is one
 *  (approved `… — one tab, two views, agent: <type>` shape). */
export function tabTitle(
  workspaceRoot: string,
  agent: TabAgent | null = null,
): string {
  const root = workspaceRoot || "No workspace picked";
  return agent
    ? `${root} — one tab, two views, agent: ${agent.type}`
    : `${root} — one tab, two views`;
}

/** A tab that has picked nothing yet: the launcher is its landing. */
export function tabUncommitted(tab: ShellTabState): boolean {
  return !tab.workspaceRoot && !tab.agent;
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

/** Patch one tab's identity (folder, agent, view, busy flag, dirty, …). The
 *  label is always derived, never set by a caller: one rule for the strip. */
export function patchTab(
  snapshot: TabSnapshot,
  clientId: number,
  patch: Partial<Omit<ShellTabState, "clientId" | "label">>,
): TabSnapshot {
  return {
    ...snapshot,
    tabs: snapshot.tabs.map((tab) => {
      if (tab.clientId !== clientId) return tab;
      const next = { ...tab, ...patch };
      return { ...next, label: tabLabel(next.workspaceRoot, next.agent) };
    }),
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
    // The registry carries the server's root, which exists even for a tab that
    // has picked nothing (configured root or cwd fallback). The client's own
    // record wins once it exists: an uncommitted tab must stay uncommitted,
    // and a folder picked in place is rebroadcast by the server anyway.
    const workspaceRoot = prior?.workspaceRoot ?? entry.workspaceRoot;
    const agent = prior?.agent ?? null;
    return {
      tabId: entry.tabId,
      clientId: entry.clientId,
      workspaceRoot,
      agent,
      view: prior?.view ?? "workspace",
      agentBusy: prior?.agentBusy ?? false,
      label: tabLabel(workspaceRoot, agent),
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
