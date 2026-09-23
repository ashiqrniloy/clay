// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ClayEditor } from "../editor/ClayEditor";
import { createDocumentSession } from "../editor/sync/session";
import { SduiRenderer } from "../sdui/renderer";
import { installSduiTree } from "../sdui/state";
import { workspaceRail } from "../shell/layout-state";
import { WorkspaceView } from "../routes/workspace";
import type { BootstrapDto } from "../bridge/types";
import { behaviorManifestFixture } from "../test/contract-fixtures";

afterEach(cleanup);
beforeEach(() => workspaceRail.setVisible(true));

const DOC = `# Notes

Fixture document.

## 26-08-12 01:15 — outline-entry-one

Body text.

## 26-08-13 09:40 — outline-entry-two

More body text.
`;

function session(doc = DOC) {
  const created = createDocumentSession({ send: async () => undefined });
  created.installInitial({
    clientId: 1,
    protocolVersion: 28,
    endpoint: "test",
    generation: 1,
    initialDocument: {
      documentId: 1,
      version: 7,
      head: { totalBytes: doc.length, firstChunk: doc },
      access: { editable: { leaseId: 1 } },
      workspaceRoot: "/tmp/clay",
    },
    behaviorManifest: behaviorManifestFixture({ behaviorVersion: 1 }),
  } as unknown as BootstrapDto);
  created.store.update({
    workspaceRootId: 1,
    path: "notes.png".replace("png", "md"),
  });
  return created;
}

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);
const readRepo = (relative: string) =>
  fs.readFileSync(path.join(repoRoot, relative), "utf8");

describe("workspace composition (plan 118: shell and Workspace page)", () => {
  it("keeps the file listing's rows in a scrolling box", () => {
    const css = readRepo("frontend/src/components/controls.module.css");
    expect(css).toMatch(
      /\.listFiltered\s*>\s*\.listBox\s*\{[^}]*overflow-y:\s*auto/s,
    );
  });

  // The path strip and the two alert rows above the canvas only exist in some
  // states, so the canvas is pinned to the last row: auto-placement would drop
  // it into an `auto` row and collapse the editor to its content height.
  it("keeps the editor canvas filling the last host row", () => {
    const css = readRepo("frontend/src/editor/editor.module.css");
    expect(css).toMatch(
      /\.host\s*\{[^}]*grid-template-rows:\s*auto auto auto minmax\(0,\s*1fr\)/s,
    );
    expect(css).toMatch(/\.column\s*\{[^}]*grid-row:\s*-2\s*\/\s*-1/s);
  });

  it("keeps the sidebar title row at one line, not a full-height row", () => {
    const { container } = render(
      <SduiRenderer
        state={installSduiTree({
          uiVersion: 6,
          rootId: 1,
          nodes: [
            { id: 1, kind: { flex: { direction: "row", children: [2, 4] } } },
            { id: 2, kind: { stack: { children: [7, 5] } } },
            {
              id: 3,
              kind: { label: { text: "Workspace · clay", icon: null } },
            },
            {
              id: 6,
              kind: {
                button: {
                  label: "Hide file browser",
                  icon: "disclosure.right",
                  action: {
                    commandId: "workspace.toggleFileBrowser",
                    source: { button: { nodeId: 6 } },
                    arguments: [],
                  },
                },
              },
            },
            { id: 7, kind: { flex: { direction: "row", children: [3, 6] } } },
            { id: 5, kind: { list: { items: [], filter: null } } },
            {
              id: 4,
              kind: {
                editorView: { binding: { documentId: 1, expectedVersion: 2 } },
              },
            },
          ],
        })}
        send={async () => undefined}
        editorSlot={<div>editor</div>}
      />,
    );
    const head = container.querySelector("div[class*='sidebarHead']");
    expect(head).not.toBeNull();
    const css = readRepo("frontend/src/sdui/renderer.module.css");
    // The head is a nested `.row`, so it must opt out of the root row's
    // `height: 100%` claim — otherwise it takes ~a sidebar of rows.
    expect(css).toMatch(/\.sidebarHead\s*\{[^}]*height:\s*auto/s);
    expect(css).toMatch(/\.sidebarHead\s*\{[^}]*flex:\s*none/s);
    // Its children must opt out of the generic row share the same way, so
    // the label takes the line and the `>` keeps its fixed size.
    expect(css).toMatch(/\.sidebarHead\s*>\s*\*\s*\{[^}]*flex:\s*none/s);
    // Long `Workspace · name · nested/dir` titles truncate on the shared line
    // instead of wrapping the head onto many rows (verified live: a long
    // title keeps the head at one 32px line).
    expect(css).toMatch(
      /\.sidebarHead\s*>\s*:first-child\s*\{[^}]*white-space:\s*nowrap/s,
    );
    expect(css).toMatch(
      /\.sidebarHead\s*>\s*:first-child\s*\{[^}]*text-overflow:\s*ellipsis/s,
    );
  });

  it("renders sidebar · editor · rail from real document metadata", () => {
    const active = session();
    render(
      <WorkspaceView session={active}>
        <ClayEditor session={active} />
      </WorkspaceView>,
    );

    const rail = screen.getByTestId("workspace-rail");
    expect(rail).toBeInTheDocument();
    // Facts are the open document's own metadata: nothing is fabricated.
    expect(rail).toHaveTextContent("notes.md");
    expect(rail).toHaveTextContent("v7");
    expect(rail).toHaveTextContent("clean");
    // Outline: every heading, entry-stamped headings split into time + title.
    const entries = rail.querySelectorAll("li button");
    expect(entries).toHaveLength(3);
    expect(entries[0]?.textContent).toContain("Notes");
    expect(entries[1]?.textContent).toContain("01:15");
    expect(entries[1]?.textContent).toContain("outline-entry-one");
    expect(rail.querySelector("dl")).toHaveTextContent("3");
  });

  it("removes the workspace rail when the agent owns the right slot", () => {
    const active = session();
    render(
      <WorkspaceView session={active} showRail={false}>
        <ClayEditor session={active} />
      </WorkspaceView>,
    );

    expect(screen.queryByTestId("workspace-rail")).not.toBeInTheDocument();
    expect(document.querySelector("[data-rail]")).toHaveAttribute(
      "data-rail",
      "collapsed",
    );
  });

  it("toggles the rail without remounting the editor, from the rail and the shell", () => {
    const active = session();
    render(
      <WorkspaceView session={active}>
        <ClayEditor session={active} data-testid="editor-slot" />
      </WorkspaceView>,
    );
    const view = document.querySelector("[data-rail]");
    expect(view?.getAttribute("data-rail")).toBe("expanded");

    fireEvent.click(screen.getByRole("button", { name: "Hide outline" }));
    expect(screen.queryByTestId("workspace-rail")).not.toBeInTheDocument();
    expect(
      document.querySelector("[data-rail]")?.getAttribute("data-rail"),
    ).toBe("collapsed");

    // The same toggle the titlebar and the status hint run.
    act(() => workspaceRail.toggle());
    expect(screen.getByTestId("workspace-rail")).toBeInTheDocument();
  });

  it("spies on the editor viewport and lets an explicit jump keep its entry (plan 118 E2)", () => {
    vi.useFakeTimers();
    const active = session();
    let topLine = 0;
    const listeners = new Set<() => void>();
    active.topVisibleLine = () => topLine;
    active.onViewportChange = (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    };
    render(
      <WorkspaceView session={active}>
        <ClayEditor session={active} />
      </WorkspaceView>,
    );
    const rail = screen.getByTestId("workspace-rail");
    const entries = [...rail.querySelectorAll("li button")];
    const label = () =>
      entries.find((entry) => entry.getAttribute("data-active") === "true")
        ?.textContent ?? "";
    /** The rail coalesces scrolls into one animation frame; fake timers run it. */
    const scrollTo = (line: number) => {
      topLine = line;
      act(() => {
        for (const listener of [...listeners]) listener();
        vi.advanceTimersByTime(20);
      });
    };

    // At the top of the document the first heading is the reached entry.
    expect(label()).toContain("Notes");
    // Scrolling to a later heading moves the mark with it.
    scrollTo(8);
    expect(label()).toContain("outline-entry-two");

    // An explicit jump locks the spy, so the jump's own scroll cannot move it.
    fireEvent.click(entries[0] as HTMLElement);
    expect(label()).toContain("Notes");
    scrollTo(8);
    expect(label()).toContain("Notes");

    // After the lock window a manual scroll takes the rail over again.
    act(() => vi.advanceTimersByTime(900));
    scrollTo(8);
    expect(label()).toContain("outline-entry-two");
    vi.useRealTimers();
  });

  it("keeps every document action in one bar and the path field on demand", () => {
    const active = session();
    render(<ClayEditor session={active} />);
    const chrome = document.querySelector('[data-clay-ds="editor.chrome"]');
    expect(chrome).not.toBeNull();
    for (const name of ["Open", "Undo", "Redo", "Reload", "Save", "Close"]) {
      expect(
        [...(chrome?.querySelectorAll("button") ?? [])].some((button) =>
          (button.getAttribute("aria-label") ?? button.textContent)?.includes(
            name,
          ),
        ),
      ).toBe(true);
    }
    // The relative-path control is revealed, not permanently visible.
    expect(
      screen.queryByLabelText("Open relative path"),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    const field = screen.getByLabelText("Open relative path");
    expect(field).toHaveFocus();
    fireEvent.keyDown(field, { key: "Escape" });
    expect(
      screen.queryByLabelText("Open relative path"),
    ).not.toBeInTheDocument();
  });

  it("animates nothing, blurs nothing and holds the measure (DESIGN.md §5–§7)", () => {
    const shell = readRepo("frontend/src/app/layout/shell.module.css");
    const workspace = readRepo("frontend/src/routes/workspace.module.css");
    const editor = readRepo("frontend/src/editor/editor.module.css");

    // Document canvas: full-bleed and left-aligned — no centred measure,
    // no side gutters on the column or the CodeMirror content.
    expect(editor).toMatch(/\.column\s*{[^}]*justify-content:\s*flex-start/s);
    expect(editor).toMatch(/\.column\s*{[^}]*padding:\s*0/s);
    expect(editor).not.toContain("92ch");
    expect(
      readRepo("frontend/src/editor/create-editor.ts").replace(
        /caret-color[^,}]*,?/g,
        "",
      ),
    ).not.toMatch(/\.cm-content[\s\S]{0,200}?padding/);
    // Rail width is host geometry with a token override, not a magic number.
    expect(workspace).toContain("--clay-dimension-rail-width, 340px");
    // Datum rows: micro-label tracking, mono values, list-family row geometry.
    expect(workspace).toContain("letter-spacing: 0.14em");
    expect(workspace).toContain(
      "--clay-ds-list-default-row-rest-border-radius",
    );
    // Chrome strips: host-owned heights, recipe-driven hairlines, mono data.
    expect(shell).toContain("--clay-dimension-titlebar-height, 40px");
    expect(shell).toContain("--clay-dimension-statusbar-height, 28px");
    expect(shell).toContain("font-variant-numeric: tabular-nums");
    expect(shell).toMatch(
      /\.footer\s*{[^}]*font-family: var\(--clay-font-monospace\)/,
    );
    // No blur and no filter on any of the three surfaces.
    for (const css of [shell, workspace, editor]) {
      expect(css).not.toMatch(/backdrop-filter/);
      expect(css).not.toMatch(/\bfilter:/);
    }
    // Motion: the shell carries exactly one looping animation — the window
    // mark's run pulse (plan 124, DESIGN.md §7) — and the tab strip none. The
    // window shows one blinking dot, so a second keyframe anywhere on these
    // surfaces (or the retired tab-marker `agentPulse`) is the regression this
    // pins.
    expect(shell.match(/@keyframes/g) ?? []).toHaveLength(1);
    expect(shell).toMatch(/@keyframes markPulse/);
    expect(workspace).not.toMatch(/@keyframes/);
    expect(editor).not.toMatch(/@keyframes/);
    expect(
      readRepo("frontend/src/components/tab-strip.module.css"),
    ).not.toMatch(/@keyframes|animation:/);
  });

  it("keeps the lane in the view pane's column and both rails at full height", () => {
    // Plan 125 (superseding plan 124's D1 full-width rule): the lane is the
    // *view pane's* chrome strip — the pane's own column, second row — so both
    // rails run the working area's full height, a hidden rail gives its width
    // back to the pane, and the field inside the lane anchors the palette to
    // the pane's width (DESIGN.md §6/§12). Plan 126 removed the last exception:
    // the workspace sidebar is a rail of this grid too, not a column inside the
    // pane, so the lane stops at *its* edge as well.
    const view = readRepo("frontend/src/routes/workspace.module.css");
    const panes = readRepo("frontend/src/shell/workspace-panes.module.css");
    const lane = readRepo("frontend/src/shell/agent-lane.module.css");
    // Three tracks: files rail · pane · inspector rail, and two rows: the views
    // above the lane in the pane's column. The route wrapper and the panes host
    // pass through so the tab's own lane can be a grid item.
    expect(view).toMatch(/grid-template-rows:\s*minmax\(0,\s*1fr\)\s*auto/);
    expect(view).toMatch(
      /grid-template-columns:\s*auto\s+minmax\(0,\s*1fr\)\s+var\(--rail-inspector\)/,
    );
    expect(view).toMatch(
      /--rail-inspector:\s*var\(--clay-dimension-rail-width/,
    );
    expect(view).toMatch(
      /--rail-files:\s*var\(--clay-dimension-sidebar-default/,
    );
    expect(view).toMatch(/\.viewMain\s*{\s*display:\s*contents/);
    expect(panes).toMatch(/\.host\s*{\s*display:\s*contents/);
    // The lane stops at both rails' inner edges: the pane's column only, never a
    // full-row span across a rail.
    expect(view).toMatch(
      /\.view\s*\[data-clay-ds="shell\.default\.footer\.rest"\]\s*{[^}]*grid-column:\s*2;[^}]*grid-row:\s*2;/s,
    );
    expect(view).not.toMatch(
      /\[data-clay-ds="shell\.default\.footer\.rest"\][^}]*grid-column:\s*1\s*\/\s*-1/s,
    );
    // Both rails span both rows, so neither ends at the lane's hairline: the
    // files rail in column 1 (its box is the shell's, sized by its own token),
    // the inspector in column 3.
    expect(view).toMatch(
      /\.view\s*\[data-panes="side"\]\s*{[^}]*grid-column:\s*1;[^}]*grid-row:\s*1\s*\/\s*-1/s,
    );
    expect(view).toMatch(
      /\.rail\s*{[^}]*grid-column:\s*3[^}]*grid-row:\s*1\s*\/\s*-1/s,
    );
    expect(panes).toMatch(
      /\.side\s*{[^}]*width:\s*var\(--rail-files[^}]*background:\s*var\(--clay-ds-shell-default-working-area-rest-background-color\)/s,
    );
    // The veil covers the views, the lane's own cell and both rails' whole
    // height; the lane sits above it, so its field — the palette's query —
    // never dims.
    expect(view).toMatch(
      /\.view\s*\[data-panes="veil"\]\s*{[^}]*grid-area:\s*1\s*\/\s*1\s*\/\s*-1\s*\/\s*-1/s,
    );
    expect(view).toMatch(/\.view\s*{[^}]*isolation:\s*isolate/s);
    expect(panes).toMatch(/z-index:\s*var\(--clay-z-modal,\s*40\)/);
    expect(lane).toMatch(/\.lane\s*{[^}]*z-index:\s*41/s);
    // …and it paints its own canvas, so the veil it sits above cannot show
    // through the chrome strip (plan 125 review, defect D8: `inherit` left the
    // lane transparent and the sheet's scrim dimmed the whole strip).
    expect(lane).toMatch(
      /\.lane\s*{[^}]*background:\s*var\(--clay-ds-shell-default-root-rest-background-color\)/s,
    );
    expect(lane).not.toMatch(/background:\s*inherit/);
  });

  it("places every grid item explicitly, so a hidden rail cannot move the lane", () => {
    // A rail leaves the grid when it is hidden and the flow when it is the
    // narrow fixed drawer, so nothing may rely on auto-placement: an item that
    // left the flow would otherwise slide the view area and the lane into its
    // track (found in the plan-125 prototype, where the lane collapsed to 0px).
    const view = readRepo("frontend/src/routes/workspace.module.css");
    for (const placement of [
      /\.view\s*\[data-panes="side"\]\s*{[^}]*grid-column:\s*1;[^}]*grid-row:\s*1\s*\/\s*-1/s,
      /\.view\s*\[data-panes="view-area"\]\s*{[^}]*grid-column:\s*2;[^}]*grid-row:\s*1;/s,
      /\.view\s*\[data-clay-ds="shell\.default\.footer\.rest"\]\s*{[^}]*grid-column:\s*2;[^}]*grid-row:\s*2;/s,
      /\.view\s*\[data-panes="veil"\]\s*{[^}]*grid-area:\s*1\s*\/\s*1\s*\/\s*-1\s*\/\s*-1/s,
      /\.rail\s*{[^}]*grid-column:\s*3[^}]*grid-row:\s*1\s*\/\s*-1/s,
    ]) {
      expect(view).toMatch(placement);
    }
    expect(view).not.toMatch(/grid-auto-flow/);
    // Collapsing the inspector drops its track, so the pane (and the lane in
    // row 2 of it) widens without moving in the DOM; the files rail's track is
    // `auto`, so a rail that is not rendered already takes no width.
    expect(view).toMatch(
      /\.view\[data-rail="collapsed"\]\s*{\s*--rail-inspector:\s*0px/s,
    );
    expect(view).toMatch(/grid-template-columns:\s*auto\s+minmax\(0,\s*1fr\)/);
  });

  it("keeps the rails flush regions and makes both transient drawers when narrow", () => {
    const workspace = readRepo("frontend/src/routes/workspace.module.css");
    // Below 1000px both rails leave the flow instead of crushing the measure,
    // and the pane is the working area's only track (DESIGN.md §12: the lane
    // keeps the pane's full width throughout).
    expect(workspace).toMatch(
      /@media \(max-width: 1000px\)[\s\S]*\.rail\s*{[^}]*position: fixed/,
    );
    expect(workspace).toMatch(
      /@media \(max-width: 1000px\)[\s\S]*\[data-panes="side"\]\s*{[^}]*position: fixed/,
    );
    expect(workspace).toMatch(
      /@media \(max-width: 1000px\)[\s\S]*\.view\s*\[data-panes="view-area"\],\s*\.view\s*\[data-clay-ds="shell\.default\.footer\.rest"\]\s*{\s*grid-column:\s*1;/,
    );
    // The workspace host paints the sidebar region (fileBrowser + divider); the
    // SDUI tree no longer wraps it in a panel.
    const fileBrowser = readRepo("src/shell/file_browser.rs");
    expect(fileBrowser).toMatch(/fn to_sdui_tree[\s\S]*SduiNodeKind::Stack/);
    expect(fileBrowser).not.toMatch(
      /fn to_sdui_tree[\s\S]{0,2000}?SduiNodeKind::Panel/,
    );
  });
});
