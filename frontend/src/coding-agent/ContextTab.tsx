// Context tab (plan 119 SC-4): live context inspector over the daemon's
// server-authoritative `session.context` view — category list → in-tab drawer →
// item detail, plus the session's capability inventories (skills, MCP servers).
import { memo, useEffect, useState } from "react";
import { ClayBadge, ClayButton, ClayIcon, ClayText } from "../components";
import { BoundedText } from "./BoundedText";
import styles from "./coding-agent.module.css";

/** Plan 109 I7: server-authoritative context view (daemon `session.context`).
 *  Counts are real numbers; `items` are capped and may trail `count`. */
export interface ContextItemRef {
  id: string;
  title: string;
  preview: string;
}
export interface ContextCategory {
  kind: string;
  label: string;
  count: number;
  items: ContextItemRef[];
}
export interface ContextView {
  sessionId?: string;
  version?: number;
  categories?: ContextCategory[];
}
/** One open item's full redacted content (`session.context { itemId }`). */
export interface ContextItemDetail {
  itemId?: string;
  kind?: string;
  title?: string;
  content?: string;
}

export function isContextView(value: unknown): value is ContextView {
  return (
    typeof value === "object" &&
    value !== null &&
    Array.isArray((value as ContextView).categories)
  );
}

/** Drawer detail shape guard (see the `clay.agentRpc` handler). */
export function isContextItemDetail(
  value: unknown,
): value is ContextItemDetail {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as ContextItemDetail).itemId === "string"
  );
}

/**
 * Capability inventories — the session's skill catalog and its MCP servers.
 * Reference data, so it lives in the inspector's Context tab and never in the
 * transcript (DESIGN.md §12). Display-only: no restart/connect actions.
 */
const CapabilitySections = memo(function CapabilitySections({
  skills,
  servers,
}: {
  skills: ReadonlyArray<{ name: string; description: string }>;
  servers: ReadonlyArray<{
    serverId: string;
    connected: boolean;
    tools: number;
    error: string;
  }>;
}) {
  if (skills.length === 0 && servers.length === 0) return null;
  return (
    <>
      {skills.length > 0 && (
        <section className={styles.section} aria-label="Skills loaded">
          <div className={styles.sectionHead}>
            <span className={styles.sectionLabel}>Skills</span>
            <ClayBadge tone="muted">{skills.length}</ClayBadge>
          </div>
          <ul className={styles.skillRows}>
            {skills.map((skill) => (
              <li key={skill.name} className={styles.skillRow}>
                <span className={styles.rowMain}>
                  <span className={styles.rowName}>{skill.name}</span>
                  {skill.description.length > 0 && (
                    <span className={styles.rowDetail}>
                      {skill.description}
                    </span>
                  )}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {servers.length > 0 && (
        <section className={styles.section} aria-label="MCP servers connected">
          <div className={styles.sectionHead}>
            <span className={styles.sectionLabel}>MCP servers</span>
            <ClayBadge tone="muted">{servers.length}</ClayBadge>
          </div>
          <ul className={styles.skillRows}>
            {servers.map((server) => (
              <li key={server.serverId} className={styles.serverRow}>
                <span className={styles.rowName}>{server.serverId}</span>
                <span
                  className={styles.rowDetail}
                  data-tone={server.connected ? "ok" : "err"}
                >
                  {server.connected
                    ? `${server.tools} ${server.tools === 1 ? "tool" : "tools"}`
                    : server.error.length > 0
                      ? `hidden: ${server.error}`
                      : "hidden"}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
});

/**
 * Context tab (plan 109 I7): live context inspector over the daemon's
 * server-authoritative `session.context` view. Category list → in-tab
 * drawer (item list) → item detail — all inside the tab panel (no
 * modal/overlay), catalog list/label composition, recipe vars only.
 * Fetch is on-demand (tab open + transcript-length invalidation); the
 * server view is cached per version in agent state.
 */
export function ContextTab({
  sessionId,
  open,
  transcriptLength,
  view,
  skills,
  servers,
  detail,
  command,
}: {
  sessionId: string;
  open: boolean;
  transcriptLength: number;
  view: ContextView | null;
  /** Capability inventories: reference data, so it lives here and not in the
   *  transcript (DESIGN.md §12). */
  skills: ReadonlyArray<{ name: string; description: string }>;
  servers: ReadonlyArray<{
    serverId: string;
    connected: boolean;
    tools: number;
    error: string;
  }>;
  detail: ContextItemDetail | null;
  /** The tab's own agent command lane (plan 119 SC-6). */
  command: (command: Record<string, unknown> | string) => void;
}) {
  const capabilities = <CapabilitySections skills={skills} servers={servers} />;
  const [drawer, setDrawer] = useState<string | null>(null);
  const [requestedItem, setRequestedItem] = useState<string | null>(null);
  useEffect(() => {
    if (!open || !sessionId) return;
    command({ context: { sessionId } });
  }, [command, open, sessionId, transcriptLength]);
  const openItem = (id: string) => {
    if (!sessionId) return;
    setRequestedItem(id);
    command({ context: { sessionId, itemId: id } });
  };
  const categories = view?.categories ?? [];
  const drawerCategory =
    drawer !== null ? categories.find((c) => c.kind === drawer) : undefined;
  // The drawer detail follows the last requested item id.
  const shownDetail = detail && requestedItem === detail.itemId ? detail : null;

  if (!sessionId) {
    return (
      <div className={styles.contextTab}>
        {capabilities}
        <div className={styles.tabEmpty}>
          <ClayText variant="body" muted>
            No active agent session.
          </ClayText>
        </div>
      </div>
    );
  }
  // First response not in yet (or stale view from a previous session).
  if (
    !view ||
    (typeof view.sessionId === "string" && view.sessionId !== sessionId)
  ) {
    return (
      <div className={styles.tabEmpty} role="status">
        <ClayText variant="body" muted>
          Loading context…
        </ClayText>
      </div>
    );
  }
  if (drawerCategory) {
    return (
      <div className={styles.contextTab}>
        {capabilities}
        <div className={styles.contextDrawer}>
          <div className={styles.detailHeader}>
            <ClayText variant="caption">{drawerCategory.label}</ClayText>
            <ClayButton
              variant="muted"
              onPress={() => {
                setDrawer(null);
                setRequestedItem(null);
              }}
            >
              <ClayIcon name="navigation.back" />
              Back
            </ClayButton>
          </div>
          {shownDetail ? (
            <div className={styles.contextDetail}>
              <ClayText variant="detail">{shownDetail.title}</ClayText>
              <BoundedText text={shownDetail.content ?? ""} />
              <ClayButton
                variant="muted"
                onPress={() => setRequestedItem(null)}
              >
                <ClayIcon name="navigation.back" />
                Back to list
              </ClayButton>
            </div>
          ) : (
            <ul
              className={styles.contextScroll}
              aria-label={`${drawerCategory.label} items`}
            >
              {drawerCategory.items.map((item) => (
                <li key={item.id}>
                  <button
                    type="button"
                    className={styles.contextItem}
                    onClick={() => openItem(item.id)}
                  >
                    <ClayText variant="detail">{item.title}</ClayText>
                    <ClayText variant="caption" muted>
                      {item.preview}
                    </ClayText>
                  </button>
                </li>
              ))}
              {drawerCategory.count > drawerCategory.items.length && (
                <li className={styles.contextMore}>
                  <ClayText variant="caption" muted>
                    …and {drawerCategory.count - drawerCategory.items.length}{" "}
                    more (not listed)
                  </ClayText>
                </li>
              )}
            </ul>
          )}
        </div>
      </div>
    );
  }
  // Count bars are relative to the largest real count on screen — a share of
  // the session's own context, never a fabricated ceiling.
  const maxCount = categories.reduce(
    (max, category) => Math.max(max, category.count),
    0,
  );
  return (
    <div className={styles.contextTab}>
      {capabilities}
      {categories.length > 0 && (
        <section className={styles.section} aria-label="Context items">
          <div className={styles.sectionHead}>
            <span className={styles.sectionLabel}>Context items</span>
          </div>
          <ul className={styles.statRows}>
            {categories.map((category) => (
              <li key={category.kind} className={styles.statRow}>
                <button
                  type="button"
                  className={styles.statButton}
                  onClick={() => setDrawer(category.kind)}
                >
                  <span className={styles.statLabel}>{category.label}</span>
                  <span className={styles.statValue}>{category.count}</span>
                </button>
                <span className={styles.statBar} aria-hidden>
                  <span
                    className={styles.statBarFill}
                    style={{
                      width: `${maxCount > 0 ? Math.round((category.count / maxCount) * 100) : 0}%`,
                    }}
                  />
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {categories.length === 0 && (
        <div className={styles.tabEmpty}>
          <ClayText variant="body" muted>
            Loading context…
          </ClayText>
        </div>
      )}
    </div>
  );
}
