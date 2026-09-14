import { describe, expect, it } from "vitest";
import {
  MAX_SESSION_FILES,
  SESSION_FILE_MARK,
  sessionFiles,
  splitSessionFilePath,
} from "./session-files";

/** A transcript row carrying a tool file record. */
function row(path: string, op: string) {
  return {
    id: `clay-tool-${path}-${op}`,
    role: "tool",
    content: `${op} ${path}`,
    metadata: { clayKind: "tool", toolName: op, sessionFile: { path, op } },
  };
}

describe("sessionFiles", () => {
  it("maps tool verbs to the session's file roles, newest first", () => {
    const files = sessionFiles([
      row("src/main.rs", "read"),
      row("DESIGN.md", "write"),
      row("src/shell/theme.rs", "edit"),
      row("notes/old.md", "delete"),
    ]);
    expect(files).toEqual([
      { path: "notes/old.md", role: "deleted" },
      { path: "src/shell/theme.rs", role: "modified" },
      { path: "DESIGN.md", role: "created" },
      { path: "src/main.rs", role: "read" },
    ]);
    // Every role has its artifact mark.
    for (const file of files)
      expect(SESSION_FILE_MARK[file.role]).toHaveLength(1);
  });

  it("keeps one row per path: the newest touch wins", () => {
    const files = sessionFiles([
      row("DESIGN.md", "read"),
      row("DESIGN.md", "write"),
      row("DESIGN.md", "edit"),
    ]);
    expect(files).toEqual([{ path: "DESIGN.md", role: "modified" }]);
  });

  it("reads a write as created only when the session had not touched the path", () => {
    // Read first → the write updated an existing file.
    expect(sessionFiles([row("a.rs", "read"), row("a.rs", "write")])).toEqual([
      { path: "a.rs", role: "modified" },
    ]);
    // Written twice → the second write is a modification.
    expect(sessionFiles([row("b.rs", "write"), row("b.rs", "write")])).toEqual([
      { path: "b.rs", role: "modified" },
    ]);
    // Deleted then written again → created.
    expect(
      sessionFiles([
        row("c.rs", "write"),
        row("c.rs", "delete"),
        row("c.rs", "write"),
      ]),
    ).toEqual([{ path: "c.rs", role: "created" }]);
  });

  it("never demotes a mutation to a later read, and moves a re-touched row up", () => {
    const files = sessionFiles([
      row("kept.rs", "write"),
      row("read.rs", "read"),
      row("kept.rs", "read"),
    ]);
    expect(files).toEqual([
      // Read again after its write: still the creation, and now the newest.
      { path: "kept.rs", role: "created" },
      { path: "read.rs", role: "read" },
    ]);
  });

  it("ignores rows without a file record and bounds the list", () => {
    expect(
      sessionFiles([
        { id: "m1", role: "assistant", content: "hi" },
        {
          id: "m2",
          role: "tool",
          content: "shell ls",
          metadata: { clayKind: "tool" },
        },
        {
          id: "m3",
          role: "tool",
          content: "x",
          metadata: { sessionFile: { path: "" } },
        },
        {
          id: "m4",
          role: "tool",
          content: "x",
          metadata: { sessionFile: { path: "a.rs" } },
        },
        {
          id: "m5",
          role: "tool",
          content: "x",
          metadata: { sessionFile: "a.rs" },
        },
      ]),
    ).toEqual([]);
    const many = Array.from({ length: MAX_SESSION_FILES + 25 }, (_, index) =>
      row(`src/file-${index}.rs`, "read"),
    );
    const files = sessionFiles(many);
    expect(files).toHaveLength(MAX_SESSION_FILES);
    // Newest first, so the oldest rows are the ones dropped.
    expect(files[0]?.path).toBe(`src/file-${MAX_SESSION_FILES + 24}.rs`);
  });

  it("splits a path into basename and muted directory", () => {
    expect(splitSessionFilePath("src/shell/theme.rs")).toEqual({
      name: "theme.rs",
      dir: "src/shell",
    });
    expect(splitSessionFilePath("DESIGN.md")).toEqual({
      name: "DESIGN.md",
      dir: "",
    });
    expect(splitSessionFilePath("/abs/root.md")).toEqual({
      name: "root.md",
      dir: "/abs",
    });
  });
});
