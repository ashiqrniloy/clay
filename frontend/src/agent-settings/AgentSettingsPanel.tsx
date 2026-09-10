import { ClayText } from "../components";

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
 *  here. */
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
      <ClayText variant="caption" muted>
        Delivered agent files. Edits to skills apply on the next daemon start.
      </ClayText>
      {loading && files == null && (
        <ClayText variant="body" muted>
          Loading…
        </ClayText>
      )}
      {files != null && files.length === 0 && (
        <ClayText variant="body" muted>
          No agent files delivered yet.
        </ClayText>
      )}
      {files != null && files.length > 0 && (
        <ul className={styles.files} aria-label="Agent files">
          {files.map((file) => (
            <li key={file.name}>
              <button
                type="button"
                className={styles.fileButton}
                onClick={() => onOpen(file.name)}
              >
                <span className={styles.name}>{file.name}</span>
                <span className={styles.meta}>
                  <ClayText variant="detail" muted>
                    {formatSize(file.sizeBytes)}
                  </ClayText>
                  <span
                    className={
                      file.edited ? styles.badgeEdited : styles.badgeBuiltin
                    }
                  >
                    {file.edited ? "edited" : "built-in"}
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} kB`;
}
