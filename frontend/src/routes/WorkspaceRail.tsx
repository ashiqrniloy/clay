import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import {
  ClayBadge,
  ClayDivider,
  ClayIconButton,
  ClayText,
} from "../components";
import { accessIsEditable } from "../state/document-store";
import type { DocumentSession } from "../editor/sync/session";
import { workspaceRail } from "../shell/layout-state";

import styles from "./workspace.module.css";

interface OutlineEntry {
  /** Zero-based document line the entry starts on. */
  line: number;
  level: number;
  /** `HH:MM` for the entry-stamped documents the workspace is used for. */
  time: string;
  title: string;
}

/** The last `—`/`–` separator in a heading, or -1. */
function entrySeparator(text: string): number {
  return Math.max(text.lastIndexOf("—"), text.lastIndexOf("–"));
}

/**
 * Outline of a markdown document: every ATX heading, in document order. A
 * heading written as `## 26-08-12 01:15 — title` (the workspace's entry
 * format, `VENT.md`) yields its time and title; any other heading is its own
 * title. Real document text only — nothing here is inferred or invented.
 */
function outlineOf(doc: string): OutlineEntry[] {
  const entries: OutlineEntry[] = [];
  const lines = doc.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^(#{1,6})\s+(.*\S)\s*$/.exec(lines[index] ?? "");
    if (!match) continue;
    let title = match[2] ?? "";
    let time = "";
    const cut = entrySeparator(title);
    if (cut > 0) {
      const stamp = /(\d{2}:\d{2})\s*$/.exec(title.slice(0, cut));
      if (stamp) {
        time = stamp[1] ?? "";
        title = title.slice(cut + 1).trim();
      }
    }
    entries.push({ line: index, level: match[1]?.length ?? 1, time, title });
  }
  return entries;
}

function wordCount(doc: string): number {
  const trimmed = doc.trim();
  return trimmed.length === 0 ? 0 : trimmed.split(/\s+/).length;
}

/** How long a click keeps the spy off the outline (the approved prototype's
 *  window: long enough for the jump's own scroll to settle). */
const SPY_LOCK_MS = 800;

const NO_SUBSCRIPTION = () => () => {};

/**
 * The workspace view's optional right rail (DESIGN.md §12): the document's
 * facts and its outline, with `⌘I` toggling it. Facts are the open document's
 * real metadata — nothing is fabricated, and nothing unknown is shown.
 */
export function WorkspaceRail({
  session,
}: {
  session: DocumentSession | null;
}) {
  const meta = useSyncExternalStore(
    session ? session.store.subscribe : NO_SUBSCRIPTION,
    () => session?.store.get() ?? null,
  );
  const [active, setActive] = useState<number | null>(null);
  // An explicit jump scrolls the editor; the spy must not fight it back, so a
  // click locks the spy briefly (the approved prototype's 800ms window). A
  // manual scroll after that takes the rail over again.
  const spyLockUntil = useRef(0);
  const [locked, setLocked] = useState(false);

  // The immutable CodeMirror Text is the cache key: its identity changes on
  // every acknowledged edit, so the scan runs once per document version.
  const snapshot = session?.snapshotDoc() ?? null;
  const [entries, words] = useMemo(() => {
    if (!snapshot || !meta?.path) return [[] as OutlineEntry[], 0];
    // ponytail: one linear scan per acknowledged version. Fine at workspace
    // document sizes; cache per-line headings if a document ever gets huge.
    const doc = snapshot.toString();
    return [outlineOf(doc), wordCount(doc)];
  }, [snapshot, meta?.path]);

  /** The outline entry the reader has reached: the last one at or above the
   *  viewport's reading inset (the approved prototype's rule). */
  const entryForLine = useCallback(
    (line: number): number | null => {
      let found: number | null = null;
      for (const entry of entries) {
        if (entry.line <= line) found = entry.line;
        else break;
      }
      return found;
    },
    [entries],
  );

  useEffect(() => {
    setLocked(false);
    setActive((current) =>
      current != null && entries.some((entry) => entry.line === current)
        ? current
        : null,
    );
  }, [entries]);

  useEffect(() => {
    if (!session) return;
    let frame: number | null = null;
    const sync = () => {
      if (Date.now() < spyLockUntil.current) return;
      setLocked(false);
      setActive(entryForLine(session.topVisibleLine()));
    };
    const onScroll = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        sync();
      });
    };
    const unsubscribe = session.onViewportChange(onScroll);
    sync();
    return () => {
      unsubscribe();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [session, entryForLine]);

  const name = meta?.path ? meta.path.split(/[\\/]/).pop() : "";
  const state = !meta
    ? "—"
    : !accessIsEditable(meta.access)
      ? "read-only"
      : meta.dirty
        ? "dirty"
        : "clean";

  return (
    <aside
      className={styles.rail}
      aria-label="Document outline"
      data-testid="workspace-rail"
    >
      <div className={styles.railHead}>
        <span className={styles.railLabel}>Outline</span>
        <ClayBadge tone="accent">{entries.length}</ClayBadge>
        <span className={styles.spacer} />
        <ClayIconButton
          icon="disclosure.right"
          label="Hide outline"
          shortcut="Ctrl+I"
          variant="muted"
          onPress={() => workspaceRail.toggle()}
        />
      </div>
      <dl className={styles.railFacts}>
        <dt>File</dt>
        <dd>{name || "—"}</dd>
        <dt>Revision</dt>
        <dd>{meta ? `v${meta.version}` : "—"}</dd>
        <dt>State</dt>
        <dd data-testid="rail-state">{state}</dd>
        <dt>Entries</dt>
        <dd>{entries.length}</dd>
        <dt>Words</dt>
        <dd>{words}</dd>
      </dl>
      <ClayDivider />
      {entries.length === 0 ? (
        <p className={styles.railEmpty}>
          <ClayText variant="detail" muted>
            No headings in this document.
          </ClayText>
        </p>
      ) : (
        <ul className={styles.railList} role="list">
          {entries.map((entry) => (
            <li key={`${entry.line}-${entry.title}`}>
              <button
                type="button"
                className={styles.entry}
                data-active={active === entry.line ? "true" : undefined}
                data-locked={
                  locked && active === entry.line ? "true" : undefined
                }
                onClick={() => {
                  spyLockUntil.current = Date.now() + SPY_LOCK_MS;
                  setLocked(true);
                  setActive(entry.line);
                  session?.revealLine(entry.line);
                }}
              >
                <span className={styles.entryTime}>{entry.time}</span>
                <span className={styles.entryTitle}>{entry.title}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className={styles.railFoot}>
        <span>Ctrl I hide</span>
      </div>
    </aside>
  );
}
