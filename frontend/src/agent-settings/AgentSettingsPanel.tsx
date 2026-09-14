import { ClayBadge, ClayText, recipeAttributes } from "../components";

import styles from "./agent-settings.module.css";

/** One delivered agent config file (plan 117). Mirrors the server's
 *  `AgentSettingsFileInfo`; `displayPath` is display-only (the webview never
 *  supplies a path — opens are by `name`). */
export interface AgentSettingsFileInfo {
  name: string;
  displayPath: string;
  sizeBytes: number;
  modifiedMs?: number | null;
  edited: boolean;
}

/** Delivered agent config files (plan 117): the body of the coding agent's
 *  Settings tab. Lists the files the daemon seeds with built-in-vs-edited
 *  provenance; selecting one opens it into the normal document pipeline.
 *  The tab frame (and its close affordance) belongs to the tab strip, not
 *  here.
 *
 *  Composition (approved `agent-settings.html`, plan 118): one column — the
 *  caption states where the numbers come from, then hairline-separated file
 *  rows with the size as data (mono) and provenance as a badge. The listing
 *  has no error scene: the server treats an unreadable skills directory as an
 *  empty listing (`src/server/agent_settings.rs`), so inventing one here would
 *  be a state the product cannot reach. */
export function AgentSettingsPanel({
  files,
  loading,
  onOpen,
}: {
  files: AgentSettingsFileInfo[] | null;
  loading: boolean;
  onOpen: (name: string) => void;
}) {
  return (
    <div className={styles.body}>
      <div className={styles.column} data-agent-settings-column>
        <ClayText
          variant="caption"
          muted
          className={styles.caption}
          data-agent-settings-caption
        >
          Delivered agent files. Edits to skills apply on the next daemon start.
          Skill sizes are read from{" "}
          <span className={styles.mono}>.agents/skills/*/SKILL.md</span>;{" "}
          <span className={styles.mono}>SYSTEM.md</span> is written by the
          daemon at runtime.
        </ClayText>
        {loading && files == null && (
          <ClayText variant="body" muted>
            Loading…
          </ClayText>
        )}
        {files != null && files.length === 0 && (
          <div className={styles.empty} data-agent-settings-empty>
            <p className={styles.emptyTitle}>No agent files yet.</p>
            <p className={styles.emptyText}>
              The daemon seeds <span className={styles.mono}>SYSTEM.md</span>{" "}
              and one file per bundled skill the first time this agent starts.
              Until then there is nothing to list.
            </p>
          </div>
        )}
        {files != null && files.length > 0 && (
          <ul className={styles.files} aria-label="Agent files">
            {files.map((file) => (
              <li key={file.name}>
                <button
                  type="button"
                  className={styles.fileRow}
                  onClick={() => onOpen(file.name)}
                  {...recipeAttributes("list", "row")}
                >
                  <span className={styles.fileName}>{file.name}</span>
                  <span className={styles.fileMeta}>
                    <ClayText variant="detail" role="monospace" muted>
                      {formatSize(file.sizeBytes)}
                    </ClayText>
                    <ClayBadge tone={file.edited ? "warning" : "muted"}>
                      {file.edited ? "edited" : "built-in"}
                    </ClayBadge>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} kB`;
}
