// Files tab (plan 119 SC-4): the files *this session* has touched, newest
// first — session history, never a file browser.
import { useEffect, useRef, useState } from "react";
import { ClayBadge, ClayKbd } from "../components";
import { recipeAttributes } from "../components/recipe-attributes";
import {
  SESSION_FILE_MARK,
  splitSessionFilePath,
  type SessionFileRecord,
} from "../agent/session-files";
import styles from "./coding-agent.module.css";

/**
 * Files tab (plan 118 task 36): the files *this session* has touched, newest
 * first — session history, never a file browser (the tree lives in the
 * workspace view, and so does the editor). A row opens in the tab's workspace
 * view, which is the dual-view payoff: the agent keeps its transcript and the
 * workspace takes the file. Rows come from the transcript's own tool records,
 * so the list survives a resume and clears with the session.
 */
export function FilesTab({
  records,
  open,
  onOpen,
}: {
  records: readonly SessionFileRecord[];
  /** The inspector tab is showing (the panel mounts every tab's content). */
  open: boolean;
  /** Open a path in the tab's workspace view. */
  onOpen: (path: string) => void;
}) {
  const [query, setQuery] = useState("");
  const filterRef = useRef<HTMLInputElement | null>(null);
  // The filter's chord, scoped to the panel that shows it and inert while a
  // field has the keyboard (the artifact's `F` hint is a real binding).
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "f") return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName.toLowerCase();
      if (
        tag === "input" ||
        tag === "textarea" ||
        target?.isContentEditable === true
      ) {
        return;
      }
      event.preventDefault();
      filterRef.current?.focus();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open]);

  const needle = query.trim().toLowerCase();
  const visible =
    needle.length === 0
      ? records
      : records.filter((record) => record.path.toLowerCase().includes(needle));

  if (records.length === 0) {
    // The approved empty state: what this panel is, and the two keystrokes
    // that move the session forward instead.
    return (
      <div className={styles.empty}>
        <p className={styles.emptyTitle}>No files in this session yet</p>
        <p className={styles.emptyText}>
          Files the agent reads, writes or searches appear here, newest first.
          Opening one is a keystroke: it switches the tab to its workspace view
          at that file.
        </p>
        <div className={styles.keyHints}>
          <span className={styles.keyHint}>
            <ClayKbd>@</ClayKbd> mention a file
          </span>
          <span className={styles.keyHint}>
            <ClayKbd>Ctrl</ClayKbd>
            <ClayKbd>1</ClayKbd> workspace view
          </span>
        </div>
      </div>
    );
  }

  return (
    <section className={styles.section} aria-label="Session files">
      <div className={styles.sectionHead}>
        <span className={styles.sectionLabel}>Session files</span>
        <ClayBadge tone="muted">{visible.length}</ClayBadge>
        <span className={styles.spacer} />
        <span className={styles.sessionFilter}>
          <input
            ref={filterRef}
            type="search"
            className={styles.sessionFilterInput}
            placeholder="Filter"
            aria-label="Filter session files"
            autoComplete="off"
            spellCheck={false}
            value={query}
            {...recipeAttributes("textInput", "input")}
            onChange={(event) => setQuery(event.target.value)}
          />
          <ClayKbd>F</ClayKbd>
        </span>
      </div>
      <ul
        className={styles.sessionList}
        aria-label="Files this session has touched"
      >
        {visible.map((record) => {
          const { name, dir } = splitSessionFilePath(record.path);
          return (
            <li key={record.path}>
              <button
                type="button"
                className={styles.sessionRow}
                data-role={record.role}
                aria-label={`Open ${record.path} in the workspace view`}
                onClick={() => onOpen(record.path)}
                {...recipeAttributes("sessionRow", "root")}
              >
                <span className={styles.sessionMark} aria-hidden>
                  {SESSION_FILE_MARK[record.role]}
                </span>
                <span className={styles.rowMain}>
                  <span className={styles.rowName}>{name}</span>
                  {dir.length > 0 && (
                    <span className={styles.rowDetail}>{dir}</span>
                  )}
                </span>
                <span className={styles.sessionRole}>{record.role}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <p className={styles.rowDetail}>
        Files this session has read, written or searched. Opening one switches
        the tab to its workspace view at that file; this is session history, not
        a file browser — the tree lives in the workspace view.
      </p>
    </section>
  );
}
