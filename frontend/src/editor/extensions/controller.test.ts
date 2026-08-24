// @vitest-environment jsdom
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it } from "vitest";

import type { BridgeEnvelope } from "../../bridge/types";
import type { DocumentMeta } from "../../state/document-store";
import { behaviorCompartment } from "../compartments";
import { EditorProjection } from "./controller";
import type { DecorationSet } from "./types";

const meta: DocumentMeta = {
  documentId: 4,
  version: 7,
  dirty: false,
  access: { editable: { leaseId: 1 } },
  path: "main.rs",
  workspaceRootId: 1,
  workspaceRoot: "/tmp/ws",
  pending: 0,
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
    expect(view.dom.querySelector("[style*='editor-keyword']")).toBeNull();
    projection.handleEnvelope(envelope(set(7)));
    expect(
      view.dom.querySelector("[style*='editor-keyword']")?.textContent,
    ).toBe("const");
    await Promise.resolve();
    const request = sent
      .map((payload) => JSON.parse(payload))
      .find((value) => value.family === "decorationViewportRequest");
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
