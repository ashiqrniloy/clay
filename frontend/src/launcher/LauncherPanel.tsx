// The launcher: the content of a fresh window and of every new empty tab
// (plan 118 Part D, DESIGN.md §12). It is the trusted panel for the bundled
// `@clay/launcher` empty-tab contribution; a third-party empty-tab
// contribution still renders through the generic SDUI view (PaneTree).
//
// Every row is server data: recent workspaces and configured agent types.
// The panel fabricates nothing — an entry exists because the server listed it
// (recents are pruned when their folder is gone; agents exist only as
// directories in the data root's `agents/`). Launching rides the ordinary tab
// and agent paths; the panel never hands a path back to the server, and
// removing a recent names an index in the server's own list.
import { useCallback, useEffect, useMemo, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";

import { ClayButton, ClayKbd, ClayText } from "../components";
import { recipeAttributes } from "../components/recipe-attributes";
import type { DocumentSession } from "../editor/sync/session";
import type { TabAgent } from "../shell/tab-store";

import styles from "./launcher.module.css";

export interface LauncherWorkspaceEntry {
  name: string;
  root: string;
}

export interface LauncherAgentEntry {
  name: string;
  label: string;
  configRoot: string;
  skillCount: number;
}

export interface LauncherEntries {
  workspaces: LauncherWorkspaceEntry[];
  agents: LauncherAgentEntry[];
  pruned: number;
}

const EMPTY_ENTRIES: LauncherEntries = {
  workspaces: [],
  agents: [],
  pruned: 0,
};

export interface LauncherPanelProps {
  /** The pane's session, used for the validated launcher requests. */
  session: DocumentSession;
  /** Picks the folder for this tab (in place while the tab is uncommitted,
   *  otherwise as its own tab — plan 118 task 33). */
  onOpenWorkspace: (root: string) => Promise<void> | void;
  onOpenFolder: () => void;
  /** Attaches the picked agent to this tab, without touching the workspace. */
  onPickAgent: (agent: TabAgent) => void;
}

type PaneId = "workspace" | "agent";

/** Read the typed rows out of a `launcherEntries` feature envelope. */
export function launcherEntriesFrom(data: unknown): LauncherEntries | null {
  if (!data || typeof data !== "object") return null;
  const entries = (data as { entries?: unknown }).entries;
  if (!entries || typeof entries !== "object") return null;
  const value = entries as Partial<LauncherEntries>;
  const workspaces = (value.workspaces ?? []).filter(
    (entry): entry is LauncherWorkspaceEntry =>
      !!entry &&
      typeof entry.root === "string" &&
      typeof entry.name === "string",
  );
  const agents = (value.agents ?? []).filter(
    (entry): entry is LauncherAgentEntry =>
      !!entry &&
      typeof entry.name === "string" &&
      typeof entry.label === "string",
  );
  return { workspaces, agents, pruned: value.pruned ?? 0 };
}

function matches(haystack: string, query: string): boolean {
  return haystack.toLowerCase().includes(query);
}

export function LauncherPanel({
  session,
  onOpenWorkspace,
  onOpenFolder,
  onPickAgent,
}: LauncherPanelProps) {
  const [entries, setEntries] = useState<LauncherEntries>(EMPTY_ENTRIES);
  const [queries, setQueries] = useState<Record<PaneId, string>>({
    workspace: "",
    agent: "",
  });
  const [cursors, setCursors] = useState<Record<PaneId, number>>({
    workspace: 0,
    agent: 0,
  });
  const [picked, setPicked] = useState<Record<PaneId, number | null>>({
    workspace: null,
    agent: null,
  });

  // Ask once, then accept every later answer (the listing after a removal is
  // the same request/response pair, so no separate refresh path is needed).
  useEffect(() => {
    const apply = (event: { kind?: string; data?: unknown }) => {
      if (event.kind !== "launcherEntries") return;
      const next = launcherEntriesFrom(event.data);
      if (next) setEntries(next);
    };
    for (const envelope of session.featureSnapshot())
      apply(envelope.data as { kind?: string; data?: unknown });
    const unsubscribe = session.subscribeFeatures((envelope) => {
      apply(envelope.data as { kind?: string; data?: unknown });
    });
    session.listLauncherEntries();
    return unsubscribe;
  }, [session]);

  const rows = useMemo(() => {
    const workspaceQuery = queries.workspace.trim().toLowerCase();
    const agentQuery = queries.agent.trim().toLowerCase();
    return {
      workspace: entries.workspaces.filter(
        (entry) =>
          matches(entry.name, workspaceQuery) ||
          matches(entry.root, workspaceQuery),
      ),
      agent: entries.agents.filter(
        (entry) =>
          matches(entry.label, agentQuery) ||
          matches(entry.configRoot, agentQuery) ||
          matches(entry.name, agentQuery),
      ),
    };
  }, [entries, queries]);

  const pickedWorkspace =
    picked.workspace === null
      ? null
      : (rows.workspace[picked.workspace] ?? null);
  const pickedAgent =
    picked.agent === null ? null : (rows.agent[picked.agent] ?? null);
  const open = Boolean(pickedWorkspace ?? pickedAgent);
  const openLabel = pickedWorkspace
    ? pickedAgent
      ? `Open ${pickedWorkspace.name} + ${pickedAgent.label}`
      : `Open ${pickedWorkspace.name}`
    : pickedAgent
      ? `Open ${pickedAgent.label}`
      : "Open";

  // Workspace first, then the agent: the folder may open a new tab, and the
  // agent must land on that tab. The agent pick leaves the view on the agent
  // (approved `agent-landing.html`: the flagship tab with both halves is the
  // agent view); a workspace-only pick leaves it on the workspace.
  const launch = useCallback(async () => {
    if (pickedWorkspace) await onOpenWorkspace(pickedWorkspace.root);
    if (pickedAgent)
      onPickAgent({
        type: pickedAgent.name,
        configRoot: pickedAgent.configRoot,
      });
  }, [onPickAgent, onOpenWorkspace, pickedAgent, pickedWorkspace]);

  const cursorOf = (pane: PaneId) =>
    Math.min(cursors[pane], Math.max(rows[pane].length - 1, 0));

  const onKeyDown = (event: ReactKeyboardEvent<HTMLElement>, pane: PaneId) => {
    const cursor = cursorOf(pane);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      const count = rows[pane].length;
      if (count === 0) return;
      event.preventDefault();
      setCursors((current) => ({
        ...current,
        [pane]:
          (current[pane] + (event.key === "ArrowDown" ? 1 : -1) + count) %
          count,
      }));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      if (event.metaKey || event.ctrlKey) {
        void launch();
        return;
      }
      setPicked((current) => ({
        ...current,
        [pane]: current[pane] === cursor ? null : cursor,
      }));
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      setPicked({ workspace: null, agent: null });
      return;
    }
    if (
      event.key === "Backspace" &&
      pane === "workspace" &&
      rows.workspace.length > 0
    ) {
      // The cursor row leaves the recents list (server-side, by index).
      event.preventDefault();
      session.removeLauncherRecent(cursor);
      setPicked({ workspace: null, agent: null });
    }
  };

  const renderPane = (pane: PaneId) => {
    const isWorkspace = pane === "workspace";
    const paneRows = rows[pane];
    const cursor = cursorOf(pane);
    const query = queries[pane];
    const label = isWorkspace ? "Recent workspaces" : "Agents";
    const emptyNote = isWorkspace
      ? query
        ? `No workspace matches “${query}”.`
        : "No workspace has been opened yet. Open a folder to start — recent ones appear here afterwards."
      : query
        ? `No agent matches “${query}”.`
        : "No agent is configured. Agents are folders under ~/.clay/agents/; one appears here as soon as it exists.";
    return (
      <section
        key={pane}
        className={styles.pane}
        data-start-pane={pane}
        aria-label={label}
        {...recipeAttributes("panel", "root")}
        onKeyDown={(event) => onKeyDown(event, pane)}
      >
        <div className={styles.paneHead}>
          <h2 className={styles.paneLabel}>
            {isWorkspace ? "Workspaces" : "Agents"}
          </h2>
          <span className={styles.count} {...recipeAttributes("badge", "root")}>
            {paneRows.length}
          </span>
          <span className={styles.spacer} />
          <input
            type="search"
            className={styles.filterInput}
            placeholder="Filter"
            aria-label={isWorkspace ? "Filter workspaces" : "Filter agents"}
            autoComplete="off"
            spellCheck={false}
            value={query}
            {...recipeAttributes("textInput", "input")}
            onChange={(event) =>
              setQueries((current) => ({
                ...current,
                [pane]: event.target.value,
              }))
            }
            onKeyDown={(event) => {
              if (event.key !== "ArrowDown") return;
              event.preventDefault();
              event.currentTarget
                .closest(`[data-start-pane="${pane}"]`)
                ?.querySelector<HTMLButtonElement>("[data-row]")
                ?.focus();
            }}
          />
        </div>
        <div className={styles.paneBody}>
          {paneRows.length === 0 ? (
            <p className={styles.note}>{emptyNote}</p>
          ) : (
            <div
              className={styles.rows}
              role="listbox"
              aria-label={label}
              {...recipeAttributes("list", "root")}
            >
              {paneRows.map((entry, index) => {
                const selected = picked[pane] === index;
                const title = isWorkspace
                  ? (entry as LauncherWorkspaceEntry).name
                  : (entry as LauncherAgentEntry).label;
                const detail = isWorkspace
                  ? (entry as LauncherWorkspaceEntry).root
                  : (entry as LauncherAgentEntry).configRoot;
                const skills = isWorkspace
                  ? 0
                  : (entry as LauncherAgentEntry).skillCount;
                return (
                  <button
                    key={`${pane}-${index}-${detail}`}
                    type="button"
                    className={styles.row}
                    role="option"
                    aria-selected={selected}
                    data-row
                    data-cursor={index === cursor ? "true" : "false"}
                    data-selected={selected ? "true" : "false"}
                    tabIndex={index === cursor ? 0 : -1}
                    {...recipeAttributes("list", "row")}
                    onClick={() =>
                      setCursors((current) => ({ ...current, [pane]: index }))
                    }
                    onFocus={() =>
                      setCursors((current) => ({ ...current, [pane]: index }))
                    }
                  >
                    <span className={styles.rowMain}>
                      <span className={styles.rowName}>{title}</span>
                      <span className={styles.rowDesc}>{detail}</span>
                    </span>
                    {isWorkspace ? null : (
                      <span className={styles.rowMeta}>
                        {skills} {skills === 1 ? "skill" : "skills"}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>
        <div className={styles.paneFoot}>
          {isWorkspace ? (
            <ClayButton onPress={onOpenFolder}>Open folder…</ClayButton>
          ) : (
            <ClayText variant="caption" muted>
              One folder per agent in ~/.clay/agents/
            </ClayText>
          )}
        </div>
      </section>
    );
  };

  return (
    <div
      className={styles.launcher}
      data-launcher
      role="group"
      aria-label="Start"
    >
      <div className={styles.inner}>
        <header className={styles.head}>
          <h1 className={styles.title}>Start</h1>
          <p className={styles.sub}>
            A tab is one workspace plus one agent, and it holds both views: the
            workspace view edits the folder, the agent view talks to the agent.
            Pick either side here — the other can be added later without leaving
            the tab.
          </p>
        </header>
        <div className={styles.grid}>
          {renderPane("workspace")}
          {renderPane("agent")}
        </div>
        <footer className={styles.actions}>
          <p className={styles.hint}>
            <span>
              <ClayKbd>Tab</ClayKbd> panes
            </span>
            <span>
              <ClayKbd>↑</ClayKbd>
              <ClayKbd>↓</ClayKbd> move
            </span>
            <span>
              <ClayKbd>⏎</ClayKbd> pick
            </span>
            <span>
              <ClayKbd>⌘⏎</ClayKbd> open both
            </span>
            <span>
              <ClayKbd>esc</ClayKbd> clear
            </span>
          </p>
          <span className={styles.spacer} />
          <span className={styles.summary} data-start-sum>
            {pickedWorkspace && pickedAgent
              ? `${pickedWorkspace.name} + ${pickedAgent.label}`
              : (pickedWorkspace?.name ??
                pickedAgent?.label ??
                "Nothing picked yet")}
          </span>
          <ClayButton
            variant="primary"
            isDisabled={!open}
            onPress={() => void launch()}
          >
            {openLabel}
          </ClayButton>
        </footer>
      </div>
    </div>
  );
}
