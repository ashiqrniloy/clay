import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it } from "vitest";

import type { BootstrapDto } from "../../bridge/types";
import { clayEditorTheme } from "../create-editor";
import { createDocumentSession } from "./session";

const bootstrap = {
  clientId: 1,
  protocolVersion: 26,
  endpoint: "test",
  generation: 1,
  initialDocument: {
    documentId: 7,
    version: 3,
    text: "hello",
    access: { editable: { leaseId: 9 } },
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: {
    manifestId: "m",
    behaviorVersion: 2,
    commands: [],
    keymaps: [],
  },
} as unknown as BootstrapDto;

function viewWith(doc: string): EditorView {
  return new EditorView({
    state: EditorState.create({ doc, extensions: [clayEditorTheme] }),
  });
}

describe("document session", () => {
  it("acks in order and tracks pending/version", async () => {
    const sent: string[] = [];
    const session = createDocumentSession({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    session.installInitial(bootstrap);
    const view = viewWith("hello");
    session.attachView(view);
    session.emitUserChanges("hello", [{ from: 5, to: 5, insert: "!" }]);
    expect(session.store.get()?.pending).toBe(1);
    expect(session.store.get()?.dirty).toBe(true);
    expect(sent.some((payload) => payload.includes('"family":"edit"'))).toBe(
      true,
    );

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "editAck",
        data: { documentId: 7, version: 4, transactionId: 1 },
      },
    });
    expect(session.store.get()?.pending).toBe(0);
    expect(session.store.get()?.version).toBe(4);
    view.destroy();
  });

  it("requests resync after a stale reject and replaces the view", () => {
    const sent: string[] = [];
    const session = createDocumentSession({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    session.installInitial(bootstrap);
    const view = viewWith("hello");
    session.attachView(view);
    session.emitUserChanges("hello", [{ from: 5, to: 5, insert: "!" }]);
    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "editRejected",
        data: {
          documentId: 7,
          transactionId: 1,
          reason: { staleVersion: { clientBaseVersion: 3, serverVersion: 9 } },
        },
      },
    });
    expect(sent.some((payload) => payload.includes("requestResync"))).toBe(
      true,
    );

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "resyncSnapshot",
        data: {
          documentId: 7,
          version: 9,
          text: "server",
          access: { editable: { leaseId: 9 } },
        },
      },
    });
    expect(view.state.doc.toString()).toBe("server");
    expect(session.store.get()?.version).toBe(9);
    expect(session.store.get()?.pending).toBe(0);
    expect(session.store.get()?.dirty).toBe(false);
    view.destroy();
  });

  it("applies open/save/reload/close metadata", () => {
    const session = createDocumentSession({ send: async () => undefined });
    session.installInitial(bootstrap);
    const view = viewWith("hello");
    session.attachView(view);

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "documentOpened",
        data: {
          metadata: {
            documentId: 8,
            version: 1,
            dirty: false,
            access: { editable: { leaseId: 2 } },
            workspaceRootId: 4,
            path: "notes.md",
          },
          text: "opened",
        },
      },
    });
    expect(view.state.doc.toString()).toBe("opened");
    expect(session.store.get()?.path).toBe("notes.md");
    expect(session.store.get()?.workspaceRootId).toBe(4);

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "documentSaved",
        data: { documentId: 8, version: 1, dirty: false },
      },
    });
    expect(session.store.get()?.dirty).toBe(false);

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "documentClosed",
        data: { documentId: 8, closed: true },
      },
    });
    expect(session.store.get()).toBeNull();
    view.destroy();
  });

  it("keeps local text when send never resolves", async () => {
    const session = createDocumentSession({
      send: () => new Promise(() => undefined),
    });
    session.installInitial(bootstrap);
    const view = viewWith("hello");
    session.attachView(view);
    session.emitUserChanges("hello", [{ from: 5, to: 5, insert: "!" }]);
    expect(view.state.doc.toString()).toBe("hello");
    expect(session.store.get()?.pending).toBe(1);
    view.destroy();
  });

  it("flushes an open queued before the handshake root id arrives", () => {
    const sent: string[] = [];
    const session = createDocumentSession({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    session.installInitial(bootstrap);
    const view = viewWith("hello");
    session.attachView(view);

    // Layout restore races the handshake metadata event: no root id yet.
    session.open("notes.md");
    expect(
      sent.some((payload) => payload.includes('"family":"openDocument"')),
    ).toBe(false);

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "documentStatus",
        data: {
          documentId: 7,
          version: 3,
          dirty: false,
          access: { editable: { leaseId: 9 } },
          workspaceRootId: 4,
          path: "",
        },
      },
    });
    expect(
      sent.some((payload) => payload.includes('"family":"openDocument"')),
    ).toBe(true);
    expect(sent.some((payload) => payload.includes('"path":"notes.md"'))).toBe(
      true,
    );
    view.destroy();
  });
});
