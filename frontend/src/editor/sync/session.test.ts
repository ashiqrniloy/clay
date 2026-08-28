import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { undo } from "@codemirror/commands";
import { afterEach, describe, expect, it } from "vitest";

import type { BootstrapDto } from "../../bridge/types";
import { clayEditorTheme, createEditor } from "../create-editor";
import { createDocumentSession } from "./session";

const bootstrap = {
  clientId: 1,
  protocolVersion: 28,
  endpoint: "test",
  generation: 1,
  initialDocument: {
    documentId: 7,
    version: 3,
    head: { totalBytes: 5, firstChunk: "hello" },
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
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("forwards atomic viewport patches to the editor projection", () => {
    const session = createDocumentSession({ send: async () => undefined });
    session.installInitial(bootstrap);
    const received: string[] = [];
    const unsubscribe = session.subscribeFeatures((envelope) => {
      if (envelope.kind === "event") received.push(envelope.data.kind);
    });

    session.handleEnvelope({
      kind: "event",
      data: {
        kind: "viewportRenderPatch",
        data: {
          requestId: 1,
          documentId: 7,
          documentVersion: 3,
          status: "complete",
          coveredRanges: [{ byteStart: 0, byteEnd: 5 }],
          decorations: [],
          diagnostics: [],
          folds: [],
        },
      },
    });

    expect(received).toEqual(["viewportRenderPatch"]);
    expect(session.featureSnapshot()).toHaveLength(1);
    unsubscribe();
  });

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
    session.emitUserChanges(EditorState.create({ doc: "hello" }).doc, [
      { from: 5, to: 5, insert: "!" },
    ]);
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
    session.emitUserChanges(EditorState.create({ doc: "hello" }).doc, [
      { from: 5, to: 5, insert: "!" },
    ]);
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
          head: { totalBytes: 6, firstChunk: "server" },
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
          head: { totalBytes: 6, firstChunk: "opened" },
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
    session.emitUserChanges(EditorState.create({ doc: "hello" }).doc, [
      { from: 5, to: 5, insert: "!" },
    ]);
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

  describe("progressive chunk loading", () => {
    const utf8Length = (text: string): number =>
      new TextEncoder().encode(text).length;

    const chunkEvent = (
      offset: number,
      text: string,
      version = 3,
      documentId = 7,
    ) => ({
      kind: "event" as const,
      data: {
        kind: "documentChunk",
        data: { documentId, documentVersion: version, offset, text },
      },
    });

    const bootstrapWithHead = (firstChunk: string, totalBytes: number) =>
      ({
        ...bootstrap,
        initialDocument: {
          ...bootstrap.initialDocument,
          head: { totalBytes, firstChunk },
        },
      }) as unknown as BootstrapDto;

    it("paints the head immediately and assembles chunks byte-identically", () => {
      const sent: string[] = [];
      const session = createDocumentSession({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      const part1 = "h\u00e9llo "; // multibyte head boundary
      const part2 = "w\u00f6rld \ud83c\udf0d"; // astral char in a later chunk
      const totalBytes = utf8Length(part1) + utf8Length(part2);
      session.installInitial(bootstrapWithHead(part1, totalBytes));

      // First paint uses the head only, before any chunk arrives.
      expect(session.snapshotDoc().toString()).toBe(part1);
      expect(session.store.get()?.loading).toBe(true);

      const request = sent.find((payload) =>
        payload.includes('"family":"documentChunkRequest"'),
      );
      expect(request).toBeDefined();
      expect(request).toContain(`"offset":${utf8Length(part1)}`);

      session.handleEnvelope(chunkEvent(utf8Length(part1), part2) as never);
      expect(session.snapshotDoc().toString()).toBe(part1 + part2);
      expect(session.store.get()?.loading).toBe(false);
    });

    it("keeps one chunk request in flight and continues from received bytes", () => {
      const sent: string[] = [];
      const session = createDocumentSession({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      // Server replies are clamped to UTF-8 char boundaries, so each reply
      // may be shorter than the requested range; the next request must
      // continue from the received end, not a fixed stride.
      const head = "head";
      session.installInitial(bootstrapWithHead(head, 20));

      const requestOffsets = () =>
        sent
          .filter((payload) =>
            payload.includes('"family":"documentChunkRequest"'),
          )
          .map((payload) => Number(payload.match(/"offset":(\d+)/)?.[1]));
      expect(requestOffsets()).toEqual([4]);

      // Short reply (char-boundary clamp simulation).
      session.handleEnvelope(chunkEvent(4, "1234567890") as never);
      expect(requestOffsets()).toEqual([4, 14]);
      expect(session.store.get()?.loading).toBe(true);

      session.handleEnvelope(chunkEvent(14, "xyzw") as never);
      expect(requestOffsets()).toEqual([4, 14, 18]);

      session.handleEnvelope(chunkEvent(18, "pqrs") as never);
      expect(requestOffsets()).toEqual([4, 14, 18]);
      expect(session.snapshotDoc().toString()).toBe("head1234567890xyzwpqrs");
      expect(session.store.get()?.loading).toBe(false);
    });

    it("gates edits until ready and drops wrong-version chunks", () => {
      const sent: string[] = [];
      const session = createDocumentSession({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      session.installInitial(bootstrapWithHead("hel", utf8Length("hello!")));

      session.emitUserChanges(EditorState.create({ doc: "hel" }).doc, [
        { from: 3, to: 3, insert: "!" },
      ]);
      expect(session.store.get()?.pending).toBe(0);
      expect(sent.some((payload) => payload.includes('"family":"edit"'))).toBe(
        false,
      );

      // Wrong-version arrival must not satisfy the pending offset.
      session.handleEnvelope(chunkEvent(3, "lo", 4) as never);
      expect(session.store.get()?.loading).toBe(true);

      session.handleEnvelope(chunkEvent(3, "lo!") as never);
      expect(session.store.get()?.loading).toBe(false);
      session.emitUserChanges(EditorState.create({ doc: "hello!" }).doc, [
        { from: 6, to: 6, insert: "?" },
      ]);
      expect(session.store.get()?.pending).toBe(1);
      expect(sent.some((payload) => payload.includes('"family":"edit"'))).toBe(
        true,
      );
    });

    it("restarts via resync when a chunk is rejected as stale", () => {
      const sent: string[] = [];
      const session = createDocumentSession({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      session.installInitial(bootstrapWithHead("old", 4096));

      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "documentChunkRejected",
          data: {
            documentId: 7,
            documentVersion: 3,
            offset: 256 * 1024,
            reason: { staleVersion: { currentVersion: 9 } },
          },
        },
      });
      expect(
        sent.some((payload) => payload.includes('"family":"requestResync"')),
      ).toBe(true);

      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "resyncSnapshot",
          data: {
            documentId: 7,
            version: 9,
            head: { totalBytes: 6, firstChunk: "server" },
            access: { editable: { leaseId: 9 } },
          },
        },
      });
      expect(session.snapshotDoc().toString()).toBe("server");
      expect(session.store.get()?.version).toBe(9);
      expect(session.store.get()?.loading).toBe(false);
    });

    it("requests each 50 MiB-head offset exactly once", () => {
      const sent: string[] = [];
      const session = createDocumentSession({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      session.installInitial(bootstrapWithHead("head", 50 * 1024 * 1024));
      const requestOffsets = () =>
        sent
          .filter((payload) =>
            payload.includes('"family":"documentChunkRequest"'),
          )
          .map((payload) => Number(payload.match(/"offset":(\d+)/)?.[1]));
      expect(requestOffsets()).toEqual([4]);
      session.handleEnvelope(chunkEvent(4, "1234567890") as never);
      expect(requestOffsets()).toEqual([4, 14]);
      // Duplicate delivery of an already-consumed offset must not re-request.
      session.handleEnvelope(chunkEvent(4, "1234567890") as never);
      expect(requestOffsets()).toEqual([4, 14]);
    });

    it("keeps small files on the single-frame path with no loading state", () => {
      const sent: string[] = [];
      const session = createDocumentSession({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      session.installInitial(bootstrap); // head complete: 5 bytes, "hello"
      expect(session.store.get()?.loading).toBe(false);
      expect(
        sent.some((payload) =>
          payload.includes('"family":"documentChunkRequest"'),
        ),
      ).toBe(false);
    });
  });

  describe("single-owner document text", () => {
    it("installs a same-length reload with changed content", () => {
      const session = createDocumentSession({ send: async () => undefined });
      session.installInitial(bootstrap); // "hello"
      const view = viewWith("hello");
      session.attachView(view);

      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "documentReloaded",
          data: {
            metadata: {
              documentId: 7,
              version: 4,
              dirty: false,
              access: { editable: { leaseId: 9 } },
              path: "",
            },
            head: { totalBytes: 5, firstChunk: "helpo" },
          },
        },
      });
      expect(view.state.doc.toString()).toBe("helpo");
      expect(session.snapshotDoc().toString()).toBe("helpo");
      view.destroy();
    });

    it("replaces same-length content while detached too", () => {
      const session = createDocumentSession({ send: async () => undefined });
      session.installInitial(bootstrap);
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "resyncSnapshot",
          data: {
            documentId: 7,
            version: 9,
            head: { totalBytes: 5, firstChunk: "helpo" },
            access: { editable: { leaseId: 9 } },
          },
        },
      });
      expect(session.snapshotDoc().toString()).toBe("helpo");
    });

    it("keeps programmatic installs out of undo history", () => {
      const host = document.createElement("div");
      document.body.append(host);
      const session = createDocumentSession({ send: async () => undefined });
      session.installInitial({
        ...bootstrap,
        initialDocument: {
          ...bootstrap.initialDocument,
          head: { totalBytes: 8, firstChunk: "hel" },
        },
      } as unknown as BootstrapDto);
      const view = createEditor({
        parent: host,
        doc: session.snapshotDoc(),
        onUserChanges: (oldDoc, changes) =>
          session.emitUserChanges(oldDoc, changes),
      });
      session.attachView(view);
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "documentChunk",
          data: { documentId: 7, documentVersion: 3, offset: 3, text: "lo!" },
        },
      });
      expect(view.state.doc.toString()).toBe("hello!");
      expect(
        undo({ state: view.state, dispatch: (tr) => view.dispatch(tr) }),
      ).toBe(false);
      expect(view.state.doc.toString()).toBe("hello!");
      view.destroy();
    });

    it("restores the latest user text on remount without a live copy", () => {
      const host = document.createElement("div");
      document.body.append(host);
      const session = createDocumentSession({ send: async () => undefined });
      session.installInitial(bootstrap); // "hello", complete
      const first = createEditor({
        parent: host,
        doc: session.snapshotDoc(),
        onUserChanges: (oldDoc, changes) =>
          session.emitUserChanges(oldDoc, changes),
      });
      session.attachView(first);
      first.dispatch({ changes: { from: 5, insert: "!" } });
      expect(session.store.get()?.pending).toBe(1);

      session.detachView(first);
      first.destroy();
      expect(session.snapshotDoc().toString()).toBe("hello!");

      const second = createEditor({
        parent: host,
        doc: session.snapshotDoc(),
      });
      session.attachView(second);
      expect(second.state.doc.toString()).toBe("hello!");
      second.destroy();
    });
  });
});
