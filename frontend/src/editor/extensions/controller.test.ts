// @vitest-environment jsdom
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { BridgeEnvelope } from "../../bridge/types";
import type { DocumentMeta } from "../../state/document-store";
import { behaviorCompartment } from "../compartments";
import { EditorProjection } from "./controller";
import type { DecorationSet, ViewportRenderPatchDto } from "./types";

const meta: DocumentMeta = {
  documentId: 4,
  version: 7,
  dirty: false,
  access: { editable: { leaseId: 1 } },
  path: "main.rs",
  workspaceRootId: 1,
  workspaceRoot: "/tmp/ws",
  pending: 0,
  loading: false,
  behaviorVersion: 2,
  diagnostic: null,
};
const provenance = {
  packageName: "@clay/rust",
  packageVersion: "1",
  packagePrefix: "rust",
};

function envelope(set: DecorationSet): BridgeEnvelope {
  return { kind: "event", data: { kind: "decorationSet", data: set } };
}

function patchEnvelope(
  patch: Partial<ViewportRenderPatchDto> & { requestId: number },
): BridgeEnvelope {
  return {
    kind: "event",
    data: {
      kind: "viewportRenderPatch",
      data: {
        documentId: 4,
        documentVersion: 7,
        status: "complete",
        reason: null,
        coveredRanges: [],
        decorations: [],
        diagnostics: [],
        folds: [],
        traceId: null,
        ...patch,
      },
    },
  };
}

function set(version: number): DecorationSet {
  return {
    documentId: 4,
    documentVersion: version,
    packagePrefix: "rust",
    kind: "syntax",
    viewportByteStart: 0,
    viewportByteEnd: 5,
    spans: [
      {
        byteStart: 0,
        byteEnd: 5,
        kind: "syntax",
        tokenType: "keyword",
        modifiers: 0,
        scope: null,
        fontRole: null,
        priority: 1,
        provenance,
        target: null,
        inlay: null,
      },
    ],
  };
}

describe("editor projection boundary", () => {
  it("drops stale decoration versions and requests only the visible byte range", async () => {
    const sent: string[] = [];
    const projection = new EditorProjection({
      send: async (payload) => {
        sent.push(payload);
      },
      meta: () => meta,
      clientId: () => 9,
      openPath: () => undefined,
      report: () => undefined,
    });
    projection.installInitial({
      behaviorVersion: 2,
      documentFontRole: "monospace",
    });
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({
        doc: "const value = 1",
        extensions: [behaviorCompartment.of([]), projection.extensions],
      }),
    });
    projection.attach(view);
    projection.handleEnvelope(envelope(set(6)));
    expect(view.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    projection.handleEnvelope(envelope(set(7)));
    expect(view.dom.querySelector(".cm-clay-t-keyword")?.textContent).toBe(
      "const",
    );
    await Promise.resolve();
    const request = sent
      .map((payload) => JSON.parse(payload))
      .find((value) => value.family === "viewportRenderRequest");
    expect(request.payload).toMatchObject({
      clientId: 9,
      documentId: 4,
      documentVersion: 7,
    });
    expect(request.payload.byteEnd).toBeLessThanOrEqual(15);
    projection.detach(view);
    view.destroy();
  });

  it("routes gated textobject commands and applies only matching results", async () => {
    const sent: string[] = [];
    const projection = new EditorProjection({
      send: async (payload) => {
        sent.push(payload);
      },
      meta: () => meta,
      clientId: () => 9,
      openPath: () => undefined,
      report: () => undefined,
    });
    projection.installInitial({
      behaviorVersion: 2,
      documentFontRole: "monospace",
    });
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({
        doc: "fn main() {}",
        extensions: [behaviorCompartment.of([]), projection.extensions],
      }),
    });
    projection.attach(view);
    projection.handleEnvelope({
      kind: "event",
      data: {
        kind: "editorCommandRequest",
        data: {
          commandId: "editor.clientSelectTextobject.function.around",
          packagePrefix: "rust",
          modeId: "rust",
        },
      },
    });
    await Promise.resolve();
    const request = sent
      .map((payload) => JSON.parse(payload))
      .find((value) => value.family === "selectionQueryRequest");
    expect(request.payload.request.query).toEqual({
      textobject: { kind: "function", around: true, direction: "current" },
    });
    projection.handleEnvelope({
      kind: "event",
      data: {
        kind: "selectionQueryResult",
        data: {
          requestId: request.payload.request.requestId,
          clientId: 9,
          documentId: 4,
          documentVersion: 7,
          behaviorVersion: 2,
          ranges: [{ start: 0, end: 12 }],
        },
      },
    });
    expect(view.state.selection.main).toMatchObject({ from: 0, to: 12 });
    projection.detach(view);
    view.destroy();
  });
});

describe("viewport request pacing", () => {
  afterEach(() => vi.useRealTimers());

  function projectionWith(
    sent: string[],
    metaOverride?: () => DocumentMeta,
  ): EditorProjection {
    return new EditorProjection({
      send: async (payload) => {
        sent.push(payload);
      },
      meta: metaOverride ?? (() => meta),
      clientId: () => 9,
      openPath: () => undefined,
      report: () => undefined,
    });
  }

  function mounted(projection: EditorProjection, doc: string): EditorView {
    return new EditorView({
      parent: document.body,
      state: EditorState.create({
        doc,
        extensions: [behaviorCompartment.of([]), projection.extensions],
      }),
    });
  }

  it("collapses a scroll storm into one latest-wins follow-up on reply", () => {
    vi.useFakeTimers();
    const sent: string[] = [];
    const projection = projectionWith(sent);
    projection.installInitial({ behaviorVersion: 2 });
    const view = mounted(projection, "line\n".repeat(400));
    projection.attach(view);
    const initial = sent.length;
    expect(initial).toBeGreaterThan(0);
    // Scroll/edit storm while the first request is on the wire: edits shift
    // every byte offset, so each tick wants a different viewport.
    for (let i = 0; i < 20; i += 1)
      view.dispatch({ changes: { from: 0, insert: "x" } });
    expect(sent.length).toBe(initial);
    // The atomic patch reply frees the pipe; exactly one follow-up carries
    // the newest viewport — no timer, no per-tick requests.
    projection.handleEnvelope(patchEnvelope({ requestId: 1 }));
    expect(sent.length).toBe(initial + 1);
    const lastPayload = sent.at(-1);
    expect(lastPayload).toBeDefined();
    const last = JSON.parse(lastPayload as string);
    expect(last.family).toBe("viewportRenderRequest");
    expect(last.payload.requestId).toBe(2);
    projection.detach(view);
    view.destroy();
  });

  it("detaching cancels pending viewport work", () => {
    vi.useFakeTimers();
    const sent: string[] = [];
    const projection = projectionWith(sent);
    projection.installInitial({ behaviorVersion: 2 });
    const view = mounted(projection, "x");
    projection.attach(view);
    const before = sent.length;
    view.dispatch({ selection: { anchor: 0 } });
    projection.detach(view);
    vi.advanceTimersByTime(1_000);
    expect(sent.length).toBe(before);
    view.destroy();
  });

  it("drops stale patch ids and applies the newest patch members", () => {
    const sent: string[] = [];
    const projection = projectionWith(sent);
    projection.installInitial({ behaviorVersion: 2 });
    const view = mounted(projection, "const value = 1\n".repeat(30));
    projection.attach(view);
    expect(sent.length).toBeGreaterThan(0);
    // A pending viewport change queues behind the inflight request.
    view.dispatch({ changes: { from: 0, insert: "x" } });
    // The newest request goes out only after the first reply frees the pipe.
    projection.handleEnvelope(patchEnvelope({ requestId: 1 }));
    expect(sent.length).toBe(2);
    // A stale patch (request 1 after 2 went out) is dropped entirely —
    // its members never reach the editor and it does not free the pipe.
    projection.handleEnvelope(patchEnvelope({ requestId: 1 }));
    expect(sent.length).toBe(2);
    expect(view.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    // The current request's patch applies its decoration members and frees
    // the pipe for the next request.
    projection.handleEnvelope(
      patchEnvelope({ requestId: 2, decorations: [set(7)] }),
    );
    expect(view.dom.querySelector(".cm-clay-t-keyword")).not.toBeNull();
    projection.detach(view);
    view.destroy();
  });

  it("an explicit empty completion immediately frees the latest request", () => {
    const sent: string[] = [];
    const projection = projectionWith(sent);
    projection.installInitial({ behaviorVersion: 2 });
    const view = mounted(projection, "line\n".repeat(400));
    projection.attach(view);
    const initial = sent.length;
    expect(initial).toBeGreaterThan(0);
    view.dispatch({ changes: { from: 0, insert: "x" } });
    // Empty terminal answer (e.g. no registered handler): no members, but
    // the pipe frees without any timer.
    projection.handleEnvelope(patchEnvelope({ requestId: 1, status: "empty" }));
    expect(sent.length).toBe(initial + 1);
    view.dispatch({ changes: { from: 0, insert: "y" } });
    // Rejected answers free the pipe identically.
    projection.handleEnvelope(
      patchEnvelope({
        requestId: 2,
        status: "rejected",
        reason: "staleVersion",
      }),
    );
    expect(sent.length).toBe(initial + 2);
    projection.detach(view);
    view.destroy();
  });

  it("clamps a huge visible span to one parse window", () => {
    const sent: string[] = [];
    const projection = projectionWith(sent);
    projection.installInitial({ behaviorVersion: 2 });
    const view = mounted(projection, "fn item() {}\n".repeat(20_000));
    projection.attach(view);
    const request = sent
      .map((payload) => JSON.parse(payload))
      .find((value) => value.family === "viewportRenderRequest");
    expect(request).toBeDefined();
    expect(
      request.payload.byteEnd - request.payload.byteStart,
    ).toBeLessThanOrEqual(64 * 1024);
    expect(request.payload.byteEnd).toBeGreaterThan(request.payload.byteStart);
    projection.detach(view);
    view.destroy();
  });

  it("requests syntax for the loaded head while later chunks are in flight", () => {
    const sent: string[] = [];
    const projection = projectionWith(sent, () => ({ ...meta, loading: true }));
    projection.installInitial({ behaviorVersion: 2 });
    const view = mounted(projection, "const loadedHead = true;\n");
    projection.attach(view);

    const requests = sent
      .map((payload) => JSON.parse(payload))
      .filter((value) => value.family === "viewportRenderRequest");
    expect(requests).toHaveLength(1);
    expect(requests[0]?.payload).toMatchObject({
      documentId: 4,
      documentVersion: 7,
      byteStart: 0,
    });
    expect(requests[0]?.payload.byteEnd).toBeGreaterThan(0);
    projection.detach(view);
    view.destroy();
  });
});
