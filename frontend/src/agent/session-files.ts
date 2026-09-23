/**
 * Plan 118 task 36: the session's file history — the Files tab's one data
 * source.
 *
 * Every record comes from a tool row the transcript already carries
 * (`metadata.sessionFile`, written server-side from the call's own
 * arguments), so the panel never scans the workspace, a resumed session
 * rebuilds the same list from its replayed rows, and the list clears with the
 * session because the transcript does.
 */

export type SessionFileRole = "read" | "modified" | "created" | "deleted";

export interface SessionFileRecord {
  path: string;
  role: SessionFileRole;
}

/** The artifact's mark letters: A/M/R/D (agent-landing.html, session files). */
export const SESSION_FILE_MARK: Record<SessionFileRole, string> = {
  created: "A",
  modified: "M",
  read: "R",
  deleted: "D",
};

/** One row per path is enough for any session, and the transcript that feeds
 *  it is itself capped at `AGENT_MAX_SNAPSHOT_ENTRIES` (200): the panel renders
 *  a bounded list rather than windowing an unbounded one. */
export const MAX_SESSION_FILES = 200;

interface ToolFile {
  path: string;
  op: string;
}

/** The file record a transcript row carries, or null for rows with none. */
function toolFile(message: unknown): ToolFile | null {
  const metadata = (message as { metadata?: { sessionFile?: unknown } })
    ?.metadata;
  const file = metadata?.sessionFile;
  if (typeof file !== "object" || file === null) return null;
  const { path, op } = file as { path?: unknown; op?: unknown };
  if (typeof path !== "string" || path.length === 0) return null;
  if (typeof op !== "string" || op.length === 0) return null;
  return { path, op };
}

/**
 * The role a touch records. `write` is the only verb whose meaning depends on
 * the session's own history: a path the session had not touched (or had
 * deleted) reads as created.
 *
 * ponytail: the daemon's write goes through the server's document op, which
 * knows whether it created the file, but that result is not on the wire — a
 * forced overwrite of a file this session never read therefore reads as
 * created. Carry the op result on the tool event to make it exact.
 */
function roleFor(
  op: string,
  prior: SessionFileRole | undefined,
): SessionFileRole {
  switch (op) {
    case "write":
      return prior === undefined || prior === "deleted"
        ? "created"
        : "modified";
    case "edit":
      return "modified";
    case "delete":
      return "deleted";
    default:
      return "read";
  }
}

/**
 * The files this session touched, newest first, one row per path.
 *
 * Order and role both follow the newest touch, with one exception: a later
 * read never demotes a mutation — a file the session created, edited or deleted
 * keeps that mark when it is read again (the more informative fact, and what
 * the approved artifact shows: DESIGN.md stays `modified`, not `read`). The
 * row still moves up on that read: newest first orders by the last touch, while
 * the mark keeps the strongest one.
 */
export function sessionFiles(
  messages: readonly unknown[],
): SessionFileRecord[] {
  const byPath = new Map<string, SessionFileRole>();
  for (const message of messages) {
    const file = toolFile(message);
    if (!file) continue;
    const prior = byPath.get(file.path);
    const touched = roleFor(file.op, prior);
    const role =
      prior !== undefined && touched === "read" && prior !== "read"
        ? prior
        : touched;
    // Re-insert so the row moves to the front of the newest-first list.
    byPath.delete(file.path);
    byPath.set(file.path, role);
  }
  return [...byPath.entries()]
    .reverse()
    .slice(0, MAX_SESSION_FILES)
    .map(([path, role]) => ({ path, role }));
}

/** Basename first, directory muted behind it (the artifact's row shape). */
export function splitSessionFilePath(path: string): {
  name: string;
  dir: string;
} {
  const cut = path.lastIndexOf("/");
  if (cut < 0) return { name: path, dir: "" };
  return { name: path.slice(cut + 1), dir: path.slice(0, cut) };
}
