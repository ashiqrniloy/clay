import {
  autocompletion,
  snippetCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import type { Extension } from "@codemirror/state";

import { utf16ToUtf8, utf8ToUtf16 } from "../position-map";
import type { CompletionItemDto, CompletionResultSet } from "./types";

interface CompletionContextData {
  clientId: number;
  documentId: number;
  documentVersion: number;
  behaviorVersion: number;
}

interface Options {
  send(payload: string): Promise<void>;
  current(): CompletionContextData | null;
  triggers(): readonly string[];
  report(message: string): void;
}

export class CompletionProjection {
  private requestId = 1;
  private waiting = new Map<
    number,
    (result: CompletionResultSet | null) => void
  >();

  constructor(private readonly options: Options) {}

  readonly extension: Extension = autocompletion({
    override: [(context) => this.source(context)],
    activateOnTyping: true,
    maxRenderedOptions: 64,
    interactionDelay: 50,
  });

  install(result: CompletionResultSet): void {
    if (result.status === "timeout")
      this.options.report("Completion provider timed out");
    if (result.status === "providerError")
      this.options.report("Completion provider failed");
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

  private async source(
    context: CompletionContext,
  ): Promise<CompletionResult | null> {
    const meta = this.options.current();
    if (!meta || !context.view) return null;
    const word = context.matchBefore(/[\p{L}\p{N}_$]*$/u);
    const preceding =
      context.pos > 0
        ? context.state.sliceDoc(context.pos - 1, context.pos)
        : "";
    const trigger = this.options.triggers().includes(preceding)
      ? { character: preceding }
      : "manual";
    if (!context.explicit && !word?.text && trigger === "manual") return null;

    const id = this.requestId++;
    const text = context.state.doc.toString();
    const from = word?.from ?? context.pos;
    const resultPromise = new Promise<CompletionResultSet | null>((resolve) => {
      this.waiting.set(id, resolve);
      const timer = window.setTimeout(() => {
        if (this.waiting.delete(id)) resolve(null);
      }, 2_000);
      context.addEventListener(
        "abort",
        () => {
          window.clearTimeout(timer);
          if (this.waiting.delete(id)) resolve(null);
        },
        { onDocChange: true },
      );
    });
    const payload = JSON.stringify({
      family: "completionRequest",
      payload: {
        request: {
          requestId: id,
          clientId: meta.clientId,
          documentId: meta.documentId,
          documentVersion: meta.documentVersion,
          behaviorVersion: meta.behaviorVersion,
          cursorByteOffset: utf16ToUtf8(text, context.pos),
          replacementRange: {
            byteStart: utf16ToUtf8(text, from),
            byteEnd: utf16ToUtf8(text, context.pos),
          },
          trigger,
          providerGeneration: 0,
          recentCompletions: [],
        },
      },
    });
    try {
      await this.options.send(payload);
    } catch {
      this.reject(id);
    }
    const result = await resultPromise;
    if (
      !result ||
      result.status !== "ok" ||
      result.documentId !== meta.documentId ||
      result.behaviorVersion !== meta.behaviorVersion
    )
      return null;
    const currentText = context.state.doc.toString();
    const resultFrom = utf8ToUtf16(
      currentText,
      result.replacementRange.byteStart,
    );
    const resultTo = utf8ToUtf16(currentText, result.replacementRange.byteEnd);
    return {
      from: resultFrom,
      to: resultTo,
      options: result.items.map((item) => completion(item)),
      filter: false,
    };
  }
}

function completion(item: CompletionItemDto): Completion {
  const base: Completion = {
    label: item.label,
    detail: item.detail || undefined,
    apply: item.insertText,
    commitCharacters: [...item.commitCharacters],
    type: "text",
    section: item.provenance.packagePrefix,
  };
  return item.textFormat === "snippet"
    ? snippetCompletion(lspSnippetToCodeMirror(item.insertText), base)
    : base;
}

/** CodeMirror and LSP differ only for bare tabstops and choices. */
export function lspSnippetToCodeMirror(template: string): string {
  return template
    .replace(
      /\$\{(\d+)\|([^|}]*)\|\}/g,
      (_match, index: string, choices: string) =>
        `\${${index}:${choices.split(",", 1)[0] ?? ""}}`,
    )
    .replace(/\$(\d+)/g, "\${$1}");
}
