// Host-owned visibility for the optional surfaces that are *not* pane slots:
// the workspace view's right rail, the agent view's inspector, and the agent
// lane shared by both views. All are per-tab layout state (DESIGN.md §5
// "layout state persists per surface"; plan 118 task 33's acceptance
// criteria), so the values are keyed by the tab's clientId and mirrored into
// `layout.json` by the workspace controller — the store itself stays
// dependency-free so the titlebar, the chords, the rail, the lane and the
// agent panel all read one owner instead of threading props.

const listeners = new Set<() => void>();

/** tab clientId → visible. Absent means visible (what a tab starts with). */
/** Tab key for a surface rendered outside a tab (the DEV fixtures): it behaves
 *  like one more tab, so a toggle there still works and is never persisted. */
const DEFAULT_KEY = "";
const visibleByTab = new Map<string, boolean>();
let activeTab: string = DEFAULT_KEY;

function notify(): void {
  for (const listener of [...listeners]) listener();
}

function createVisibility() {
  const isVisible = (): boolean => visibleByTab.get(activeTab) ?? true;
  const setVisible = (next: boolean): void => {
    if (isVisible() === next) return;
    visibleByTab.set(activeTab, next);
    notify();
  };
  return {
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    /** Visible for the active tab (true when nothing is recorded: the default a
     *  tab starts with, and what an older layout file means). */
    isVisible,
    setVisible,
    toggle(): void {
      setVisible(!isVisible());
    },
    /** The shell tells the store which tab is up; switching tabs re-reads. */
    setActiveTab(clientId: number | null): void {
      const next = clientId == null ? DEFAULT_KEY : String(clientId);
      if (next === activeTab) return;
      activeTab = next;
      notify();
    },
    /** Restore per-tab values (the persisted layout's round trip). */
    restore(entries: Iterable<[number, boolean]>): void {
      visibleByTab.clear();
      for (const [clientId, visible] of entries) {
        visibleByTab.set(String(clientId), visible);
      }
      notify();
    },
    /** What the workspace controller persists, one entry per tab. */
    snapshot(): Map<number, boolean> {
      const out = new Map<number, boolean>();
      for (const [id, visible] of visibleByTab) {
        const clientId = Number(id);
        if (id !== DEFAULT_KEY && Number.isFinite(clientId)) {
          out.set(clientId, visible);
        }
      }
      return out;
    },
    /** Test seam: the store is module state, like the layout it mirrors. */
    resetForTests(): void {
      visibleByTab.clear();
      activeTab = DEFAULT_KEY;
      notify();
    },
  };
}

export const workspaceRail = createVisibility();
export const agentInspector = createVisibility();
/** The agent lane (plan 124): one persistent bottom section per tab, in both
 *  views, toggled by `Ctrl+X Ctrl+P` / `shell.toggleAgentLane`. */
export const agentLane = createVisibility();
