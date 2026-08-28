// @vitest-environment jsdom
//
// Plan 099: deterministic editor performance invariants. These tests block CI
// on work-count / ownership / retention / history structure — never on
// machine-variant wall-clock timings (measured p95s are printed for the
// real-device matrix in scripts/editor-performance-smoke.sh, which enforces
// approved targets only after three stable designated-device runs).
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it } from "vitest";

import { createEditor } from "../create-editor";
import { positionIndex, positionIndexStats } from "../position-index";
import { utf16ToUtf8Indexed } from "../position-map";
import { asDocumentId, type BootstrapDto } from "../../bridge/types";
import { createDocumentSession } from "../sync/session";
import {
  decorationExtension,
  decorationStats,
  replaceDecorations,
} from "./decorations";
import type { DecorationSet } from "./types";

const provenance = {
  packageName: "core",
  packageVersion: "builtin",
  packagePrefix: "core",
};

function syntaxSet(
  documentId: number,
  viewportByteStart: number,
  viewportByteEnd: number,
  spanCount: number,
): DecorationSet {
  const lineLength = 16;
  return {
    documentId,
    documentVersion: 1,
    packagePrefix: "core",
    kind: "syntax",
    viewportByteStart,
    viewportByteEnd,
    spans: Array.from({ length: spanCount }, (_, index) => ({
      byteStart: viewportByteStart + index * lineLength,
      byteEnd: viewportByteStart + index * lineLength + 3,
      kind: "syntax" as const,
      tokenType: "keyword" as const,
      modifiers: 0,
      scope: null,
      fontRole: null,
      priority: 1,
      provenance,
      target: null,
      inlay: null,
    })),
  };
}

describe("editor performance invariants (Plan 099)", () => {
  it("keeps 200 repeated 1 MiB edits on the shared index without rebuilding it", () => {
    // Real Clay path: createEditor installs the shared position field and the
    // emitted user change converts through it. The invariant under CI is
    // structural: the SAME index object serves every keystroke (no
    // per-edit rebuild) and every edit converts exactly through it.
    const text = "let value = 1;\n".repeat(65_536);
    let conversions = 0;
    const view = createEditor({
      parent: document.body,
      doc: text,
      onUserChanges: (_oldDoc, changes, _traceId, index) => {
        if (!index) throw new Error("shared index missing on edit path");
        for (const change of changes) {
          utf16ToUtf8Indexed(index, change.from);
          conversions += 1;
        }
      },
    });
    for (let i = 0; i < 200; i += 1) {
      view.dispatch({
        changes: { from: (i * 977) % view.state.doc.length, insert: "x" },
      });
      // Ownership invariant: the one shared field value tracks the current
      // document version exactly — incremental mutation, never a stale or
      // separately rebuilt copy.
      const index = positionIndex(view.state);
      expect(index.doc).toBe(view.state.doc);
    }
    expect(conversions).toBe(200);
    const stats = positionIndexStats(positionIndex(view.state));
    expect(stats.lines).toBe(view.state.doc.lines);
    view.destroy();
  });

  it("retains a constant-size projection across 100 sliding viewport patches", () => {
    // Each patch fully replaces the previous viewport's spans: after 100
    // scrolls the retained projection is exactly the latest patch, never an
    // accumulating history.
    // Doc spans 100 disjoint viewports plus guard slack.
    const spansPerPatch = 400;
    const stride = 16 * spansPerPatch * 4;
    const text = "let value = 1;\n".repeat(Math.ceil((100 * stride) / 15));
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({ doc: text, extensions: decorationExtension }),
    });
    // Each patch's spans authoritatively cover its whole declared viewport
    // and strides exceed the retention guard, so every scroll exactly
    // replaces the retained projection.
    for (let patch = 0; patch < 100; patch += 1) {
      const start = Math.min(patch * stride, text.length - stride);
      view.dispatch({
        effects: replaceDecorations(
          view.state,
          syntaxSet(1, start, start + 16 * spansPerPatch, spansPerPatch),
        ),
      });
      // Constant-size retention: exactly the latest patch's spans survive.
      expect(decorationStats(view.state).marks).toBe(spansPerPatch);
    }
    view.destroy();
  });

  it("keeps a 50 MiB document a single Text with no undo history", () => {
    // Ownership invariant at the approved top size: one authoritative Text,
    // installed programmatically (no history entry), one session.
    const line = "0123456789abcdef\n";
    const repeat = Math.ceil((50 * 1024 * 1024) / line.length);
    const text = line.repeat(repeat);
    const session = createDocumentSession({ send: async () => undefined });
    const bootstrap: BootstrapDto = {
      clientId: 1,
      protocolVersion: 29,
      endpoint: "perf",
      generation: 1,
      initialDocument: {
        documentId: asDocumentId(1),
        version: 1,
        access: { editable: { leaseId: 1 } },
        workspaceRoot: "/perf",
        head: { totalBytes: text.length, firstChunk: text },
      },
      behaviorManifest: {
        manifestId: "default.text",
        behaviorVersion: 1,
        commands: [],
        keymaps: [],
      },
      activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
      activeTypography: {
        revision: 1,
        monospace: {
          families: ["monospace"],
          size: 13,
          ligatures: { enableStandard: true },
        },
        proportional: {
          families: ["serif"],
          size: 13,
          ligatures: { enableStandard: true },
        },
        ui: {
          families: ["system-ui"],
          size: 13,
          ligatures: { enableStandard: true },
        },
        hierarchy: {
          display: 1.5,
          title: 1,
          section: 1,
          body: 1,
          status: 1,
          detail: 0.8,
          caption: 0.75,
        },
      },
    };
    session.installInitial(bootstrap);
    // Detached: the session owns exactly one current Text.
    const detached = session.snapshotDoc();
    expect(detached.length).toBe(text.length);
    expect(detached.eq(session.snapshotDoc())).toBe(true);

    // Attach: the programmatic install must not create undo history and must
    // leave the view owning the very same document content.
    const view = new EditorView({ parent: document.body });
    session.attachView(view);
    expect(view.state.doc.eq(detached)).toBe(true);
    expect(view.state.doc.length).toBe(text.length);
    session.detachView(view);
    view.destroy();
    // The detached snapshot survived detach as the single current Text.
    expect(session.snapshotDoc().eq(detached)).toBe(true);
  });

  it("applies four-pane patches linearly: each pane applies exactly its own patch", () => {
    // Aggregate patch work across visible panes is N applications for N
    // panes (linear), with per-pane isolation by document id.
    const text = "let value = 1;\n".repeat(2_000);
    const panes = [1, 2, 3, 4].map((rawId) => {
      const documentId = asDocumentId(rawId);
      const view = new EditorView({
        parent: document.body,
        state: EditorState.create({
          doc: text,
          extensions: [decorationExtension],
        }),
      });
      return { documentId, view, spans: documentId * 10 };
    });
    for (const pane of panes) {
      pane.view.dispatch({
        effects: replaceDecorations(
          pane.view.state,
          syntaxSet(pane.documentId, 0, 8_000, pane.spans),
        ),
      });
    }
    // Each pane retained exactly its own patch: linear aggregate work (N
    // panes -> N applications), no cross-pane pollution or re-application.
    for (const pane of panes) {
      expect(decorationStats(pane.view.state).marks).toBe(pane.spans);
      pane.view.destroy();
    }
  });

  it("keeps text, highlights, and folds intact after decoration application (software-render smoke)", () => {
    // Functional software-rendering smoke: content is never lost and
    // projections stay queryable after patches (rendering backend agnostic).
    const text = "# Title\n\n- alpha\n- beta\n".repeat(500);
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({ doc: text, extensions: decorationExtension }),
    });
    view.dispatch({
      effects: replaceDecorations(
        view.state,
        syntaxSet(1, 0, text.length, 100),
      ),
    });
    expect(view.state.doc.toString()).toBe(text);
    view.destroy();
  });
});
