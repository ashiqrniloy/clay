import { type Extension } from "@codemirror/state";
import {
  EditorView,
  hoverTooltip,
  keymap,
  showDialog,
  type Tooltip,
} from "@codemirror/view";

import {
  textIndex,
  utf16ToUtf8Indexed,
  utf8ToUtf16Indexed,
} from "../position-map";
import type { LanguageFeature, LanguageResult, TextLocation } from "./types";

interface Current {
  clientId: number;
  documentId: number;
  documentVersion: number;
  behaviorVersion: number;
}
interface Options {
  send(payload: string): Promise<void>;
  current(): Current | null;
  openPath(path: string): void;
  report(message: string): void;
}

export class IntelligenceProjection {
  private requestId = 1;
  private waiting = new Map<number, (result: LanguageResult | null) => void>();

  constructor(private readonly options: Options) {}

  readonly extension: Extension = [
    hoverTooltip(
      async (view, position) => {
        const result = await this.request("hover", view, position);
        const hover =
          result && "hover" in result.payload ? result.payload.hover : null;
        if (!hover?.markdown) return null;
        const index = textIndex(view.state.doc);
        return {
          pos: hover.range
            ? utf8ToUtf16Indexed(index, hover.range.byteStart)
            : position,
          end: hover.range
            ? utf8ToUtf16Indexed(index, hover.range.byteEnd)
            : undefined,
          create: () => {
            const dom = document.createElement("div");
            dom.className = "cm-clay-hover";
            dom.textContent = inertText(hover.markdown);
            return { dom };
          },
        } satisfies Tooltip;
      },
      { hoverTime: 300 },
    ),
    keymap.of([
      {
        key: "F12",
        run: (view) => {
          void this.definition(view);
          return true;
        },
      },
      {
        key: "Mod-.",
        run: (view) => {
          void this.codeActions(view);
          return true;
        },
      },
      {
        key: "Ctrl-Shift-Space",
        run: (view) => {
          void this.signature(view);
          return true;
        },
      },
    ]),
  ];

  install(result: LanguageResult): void {
    if (result.status === "timeout")
      this.options.report("Language provider timed out");
    if (result.status === "providerError")
      this.options.report("Language provider failed");
    this.waiting.get(result.requestId)?.(result);
    this.waiting.delete(result.requestId);
  }

  reject(requestId: number): void {
    this.waiting.get(requestId)?.(null);
    this.waiting.delete(requestId);
  }

  clear(): void {
    for (const resolve of this.waiting.values()) resolve(null);
    this.waiting.clear();
  }

  private async definition(view: EditorView): Promise<void> {
    const result = await this.request("goToDefinition", view);
    const locations =
      result && "goToDefinition" in result.payload
        ? result.payload.goToDefinition.locations
        : [];
    const first = locations[0];
    if (!first) return;
    this.navigate(view, first);
  }

  private async signature(view: EditorView): Promise<void> {
    const result = await this.request("signatureHelp", view);
    const help =
      result && "signatureHelp" in result.payload
        ? result.payload.signatureHelp
        : null;
    const signature = help?.signatures[help.activeSignature ?? 0];
    if (signature)
      view.dispatch({ effects: EditorView.announce.of(signature.label) });
  }

  private async codeActions(view: EditorView): Promise<void> {
    const result = await this.request("codeAction", view);
    const actions =
      result && "codeAction" in result.payload
        ? result.payload.codeAction.actions
        : [];
    if (!actions.length) return;
    showDialog(view, {
      class: "cm-clay-actions",
      focus: true,
      content: (_view, close) => {
        const form = document.createElement("form");
        form.setAttribute("aria-label", "Code actions");
        for (const action of actions) {
          const button = document.createElement("button");
          button.type = "button";
          button.textContent = action.title;
          button.disabled = !action.commandId;
          button.addEventListener("click", () => {
            if (action.commandId) this.sendCommand(action.commandId);
            close();
          });
          form.append(button);
        }
        return form;
      },
    });
  }

  private navigate(view: EditorView, location: TextLocation): void {
    if ("openDocument" in location) {
      const meta = this.options.current();
      if (meta?.documentId !== location.openDocument.documentId) {
        view.dispatch({
          effects: EditorView.announce.of(
            "Definition is open in another document",
          ),
        });
        return;
      }
      const at = utf8ToUtf16Indexed(
        textIndex(view.state.doc),
        location.openDocument.range.byteStart,
      );
      view.dispatch({ selection: { anchor: at }, scrollIntoView: true });
      return;
    }
    const path = location.workspaceFile.relativePath;
    if (safeRelativePath(path)) this.options.openPath(path);
  }

  private async request(
    feature: LanguageFeature,
    view: EditorView,
    position = view.state.selection.main.head,
  ): Promise<LanguageResult | null> {
    const meta = this.options.current();
    if (!meta) return null;
    const id = this.requestId++;
    const resultPromise = new Promise<LanguageResult | null>((resolve) => {
      this.waiting.set(id, resolve);
      window.setTimeout(() => {
        if (this.waiting.delete(id)) resolve(null);
      }, 5_000);
    });
    const payload = JSON.stringify({
      family: "languageIntelligenceRequest",
      payload: {
        request: {
          requestId: id,
          clientId: meta.clientId,
          documentId: meta.documentId,
          documentVersion: meta.documentVersion,
          behaviorVersion: meta.behaviorVersion,
          cursorByteOffset: utf16ToUtf8Indexed(
            textIndex(view.state.doc),
            position,
          ),
          feature,
          providerGeneration: 0,
        },
      },
    });
    try {
      await this.options.send(payload);
    } catch {
      this.reject(id);
    }
    const result = await resultPromise;
    return result?.status === "ok" &&
      result.documentId === meta.documentId &&
      result.behaviorVersion === meta.behaviorVersion
      ? result
      : null;
  }

  private sendCommand(commandId: string): void {
    const meta = this.options.current();
    if (!meta) return;
    void this.options.send(
      JSON.stringify({
        family: "commandIntent",
        payload: {
          clientId: meta.clientId,
          documentId: meta.documentId,
          behaviorVersion: meta.behaviorVersion,
          commandId,
        },
      }),
    );
  }
}

function safeRelativePath(path: string): boolean {
  return (
    !!path &&
    !path.startsWith("/") &&
    !path.includes(":") &&
    path.split(/[\\/]/).every((part) => part && part !== "." && part !== "..")
  );
}

function inertText(markdown: string): string {
  return markdown
    .replace(/[`*_>#-]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
