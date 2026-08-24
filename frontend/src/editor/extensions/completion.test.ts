// @vitest-environment jsdom
import {
  acceptCompletion,
  completionStatus,
  currentCompletions,
  selectedCompletionIndex,
  setSelectedCompletion,
  startCompletion,
} from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it, vi } from "vitest";

import { CompletionProjection } from "./completion";

const provenance = {
  packageName: "@clay/rust",
  packageVersion: "1",
  packagePrefix: "rust",
};

describe("completion projection", () => {
  it("round-trips a request and expands inert snippets locally", async () => {
    const sent: string[] = [];
    const projection = new CompletionProjection({
      send: async (payload) => {
        sent.push(payload);
      },
      current: () => ({
        clientId: 3,
        documentId: 4,
        documentVersion: 7,
        behaviorVersion: 2,
      }),
      triggers: () => ["."],
      report: () => undefined,
    });
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({
        doc: "fn",
        extensions: projection.extension,
      }),
    });
    view.dispatch({ selection: { anchor: 2 } });
    expect(startCompletion(view)).toBe(true);
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    const request = JSON.parse(sent[0] ?? "").payload.request;
    projection.install({
      requestId: request.requestId,
      clientId: 3,
      documentId: 4,
      documentVersion: 7,
      behaviorVersion: 2,
      providerGeneration: 0,
      replacementRange: { byteStart: 0, byteEnd: 2 },
      status: "ok",
      items: [
        {
          label: "function",
          insertText: "fn ${1:name}() {\n  $0\n}",
          detail: "snippet",
          commitCharacters: "",
          textFormat: "snippet",
          provenance,
        },
      ],
      provenance,
    });
    await vi.waitFor(() => expect(completionStatus(view.state)).toBe("active"));
    expect(currentCompletions(view.state)[0]?.label).toBe("function");
    view.dispatch({ effects: setSelectedCompletion(0) });
    await vi.waitFor(() => expect(selectedCompletionIndex(view.state)).toBe(0));
    expect(acceptCompletion(view)).toBe(true);
    expect(view.state.doc.toString()).toBe("fn name() {\n  \n}");
    expect(view.state.selection.main.from).toBe(3);
    expect(view.state.selection.main.to).toBe(7);
    view.destroy();
  });
});
