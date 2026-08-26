import { EditorState } from "@codemirror/state";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, act } from "@testing-library/react";

import type { BootstrapDto } from "../bridge/types";
import { ClayEditor } from "../editor/ClayEditor";
import {
  clayEditorTheme,
  createEditor,
  setReadOnly,
  setTheme,
} from "../editor/create-editor";
import { createDocumentSession } from "../editor/sync/session";

afterEach(cleanup);

const bootstrap = {
  clientId: 1,
  protocolVersion: 27,
  endpoint: "test",
  generation: 1,
  initialDocument: {
    documentId: 1,
    version: 1,
    head: { totalBytes: 4, firstChunk: "seed" },
    access: { editable: { leaseId: 1 } },
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: {
    manifestId: "m",
    behaviorVersion: 2,
    commands: [],
    keymaps: [],
  },
} as unknown as BootstrapDto;

describe("editor lifecycle", () => {
  it("preserves document text across theme and read-only reconfigure", () => {
    const host = document.createElement("div");
    document.body.append(host);
    const view = createEditor({ parent: host, doc: "keep me" });
    expect(view.state.doc.toString()).toBe("keep me");
    setTheme(view, clayEditorTheme);
    setReadOnly(view, true);
    expect(view.state.doc.toString()).toBe("keep me");
    expect(view.state.facet(EditorState.readOnly)).toBe(true);
    view.destroy();
    host.remove();
  });

  it("renders chrome from metadata without putting text in React", () => {
    const session = createDocumentSession({ send: async () => undefined });
    session.installInitial(bootstrap);
    render(<ClayEditor session={session} />);
    expect(screen.getByTestId("clay-editor")).toBeInTheDocument();
    expect(screen.getByText("ws")).toBeInTheDocument();
    expect(screen.queryByText("/tmp/ws")).not.toBeInTheDocument();
    expect(screen.getByText(/editable/)).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: /Editor ws/ }),
    ).toBeInTheDocument();
  });

  it("applies a local transaction before a blocked send settles", async () => {
    let release: (() => void) | undefined;
    const blocked = new Promise<void>((resolve) => {
      release = resolve;
    });
    const session = createDocumentSession({ send: () => blocked });
    session.installInitial(bootstrap);
    const host = document.createElement("div");
    document.body.append(host);
    const view = createEditor({
      parent: host,
      doc: "seed",
      onUserChanges: (oldText, changes) => {
        session.emitUserChanges(oldText, changes);
      },
    });
    session.attachView(view);
    view.dispatch({ changes: { from: 4, insert: "!" } });
    expect(view.state.doc.toString()).toBe("seed!");
    expect(session.store.get()?.pending).toBe(1);
    release?.();
    view.destroy();
    host.remove();
  });

  it("shows a loading status during chunk assembly that clears at ready", () => {
    const sent: string[] = [];
    const session = createDocumentSession({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    const loadingBootstrap = {
      ...bootstrap,
      initialDocument: {
        ...bootstrap.initialDocument,
        head: { totalBytes: 8, firstChunk: "seed" },
      },
    } as unknown as BootstrapDto;
    session.installInitial(loadingBootstrap);
    render(<ClayEditor session={session} />);

    expect(screen.getByRole("status")).toHaveTextContent(
      "Loading full document…",
    );
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

    act(() => {
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "documentChunk",
          data: { documentId: 1, documentVersion: 1, offset: 4, text: "-full" },
        },
      });
    });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
    expect(screen.getByText(/seed-full/)).toBeInTheDocument();
  });
});
