import {
  addCursorAbove,
  addCursorBelow,
  cursorGroupBackward,
  cursorGroupForward,
  selectLine,
  simplifySelection,
  undo,
  redo,
  undoSelection,
} from "@codemirror/commands";
import { foldCode } from "@codemirror/language";
import {
  selectNextOccurrence,
  selectSelectionMatches,
} from "@codemirror/search";
import { EditorSelection, type Extension } from "@codemirror/state";
import { EditorView, hoverTooltip, keymap } from "@codemirror/view";

import type { BridgeEnvelope } from "../../bridge/types";
import type { DocumentMeta } from "../../state/document-store";
import { behaviorCompartment } from "../compartments";
import { utf16ToUtf8, utf8ToUtf16 } from "../position-map";
import { accessibilityExtension } from "./accessibility";
import { behaviorExtensions } from "./behavior";
import { CompletionProjection } from "./completion";
import { DiagnosticProjection, diagnosticExtension } from "./diagnostics";
import {
  decorationExtension,
  linkAt,
  replaceDecorations,
  resetDecorations,
  showInlays,
} from "./decorations";
import { foldingExtension, installFolds, resetFolds } from "./folding";
import { IntelligenceProjection } from "./intelligence";
import { interactionKeymaps } from "./keymaps";
import type {
  BehaviorManifestDto,
  CompletionResultSet,
  DecorationSet,
  DecorationTarget,
  DiagnosticSet,
  FoldingRangeSet,
  LanguageResult,
  SelectionQueryResult,
} from "./types";

interface Options {
  send(payload: string): Promise<void>;
  meta(): DocumentMeta | null;
  clientId(): number;
  openPath(path: string): void;
  report(message: string): void;
}

export class EditorProjection {
  private view: EditorView | null = null;
  private manifest: BehaviorManifestDto = { behaviorVersion: 0 };
  private decorationSets = new Map<string, DecorationSet>();
  private diagnosticSets = new Map<string, DiagnosticSet>();
  private foldSets = new Map<string, FoldingRangeSet>();
  private readonly diagnostics = new DiagnosticProjection();
  private readonly completion: CompletionProjection;
  private readonly intelligence: IntelligenceProjection;
  readonly extensions: Extension[];
  private lastViewport = "";
  private nextSelectionRequest = 1;
  private pendingSelectionRequest: number | null = null;
  private pendingSelections: readonly { anchor: number; head: number }[] = [];

  constructor(private readonly options: Options) {
    const current = () => {
      const meta = options.meta();
      return meta
        ? {
            clientId: options.clientId(),
            documentId: meta.documentId,
            documentVersion: meta.version + meta.pending,
            behaviorVersion: meta.behaviorVersion,
          }
        : null;
    };
    this.completion = new CompletionProjection({
      send: options.send,
      current,
      triggers: () =>
        this.manifest.editorRules?.autocompleteTriggers?.map(
          (item) => item.trigger,
        ) ?? [],
      report: options.report,
    });
    this.intelligence = new IntelligenceProjection({
      send: options.send,
      current,
      openPath: options.openPath,
      report: options.report,
    });
    this.extensions = [
      interactionKeymaps,
      accessibilityExtension,
      decorationExtension,
      diagnosticExtension,
      foldingExtension,
      EditorView.updateListener.of((update) => {
        if (update.viewportChanged || update.docChanged)
          this.requestViewport(update.view);
      }),
      hoverTooltip(
        (view, position) => {
          const target = linkAt(view, position);
          const text = targetText(target);
          if (!text) return null;
          return {
            pos: position,
            create: () => {
              const dom = document.createElement("div");
              dom.className = "cm-clay-hover";
              dom.textContent = text;
              return { dom };
            },
          };
        },
        { hoverTime: 200 },
      ),
      EditorView.domEventHandlers({
        click: (event, view) => {
          const position = view.posAtCoords({
            x: event.clientX,
            y: event.clientY,
          });
          const target = position == null ? null : linkAt(view, position);
          return target ? this.activateLink(view, target) : false;
        },
      }),
      keymap.of([
        {
          key: "Mod-Enter",
          run: (view) => {
            const target = linkAt(view, view.state.selection.main.head);
            return target ? this.activateLink(view, target) : false;
          },
        },
      ]),
      this.completion.extension,
      this.intelligence.extension,
    ];
  }

  installInitial(manifest: BehaviorManifestDto): void {
    this.manifest = manifest;
  }

  attach(view: EditorView): void {
    this.view = view;
    view.dispatch({
      effects: behaviorCompartment.reconfigure(
        behaviorExtensions(this.manifest, (command) =>
          this.runManifestCommand(command),
        ),
      ),
    });
    const codeInlays =
      this.manifest.editorRules?.chrome?.inlayHints ??
      this.manifest.documentFontRole === "monospace";
    view.dispatch({ effects: showInlays(codeInlays) });
    for (const set of this.decorationSets.values())
      view.dispatch({ effects: replaceDecorations(set) });
    for (const set of this.diagnosticSets.values())
      this.diagnostics.install(view, set);
    for (const set of this.foldSets.values())
      view.dispatch({ effects: installFolds(view.state.doc.toString(), set) });
    this.requestViewport(view);
  }

  detach(view: EditorView): void {
    if (this.view === view) this.view = null;
  }

  clear(): void {
    this.decorationSets.clear();
    this.diagnosticSets.clear();
    this.foldSets.clear();
    this.completion.clear();
    this.intelligence.clear();
    this.lastViewport = "";
    if (this.view) {
      this.view.dispatch({ effects: [resetDecorations(), resetFolds()] });
      this.diagnostics.clear(this.view);
    }
  }

  handleEnvelope(envelope: BridgeEnvelope): void {
    if (envelope.kind !== "event") return;
    const event = envelope.data as { kind: string; data: unknown };
    const meta = this.options.meta();
    const currentVersion = meta ? meta.version + meta.pending : -1;
    switch (event.kind) {
      case "behaviorManifestInstalled": {
        const data = event.data as { manifest: BehaviorManifestDto };
        this.manifest = data.manifest;
        if (this.view)
          this.view.dispatch({
            effects: behaviorCompartment.reconfigure(
              behaviorExtensions(this.manifest, (command) =>
                this.runManifestCommand(command),
              ),
            ),
          });
        break;
      }
      case "decorationSet":
        this.installDecoration(event.data as DecorationSet, currentVersion);
        break;
      case "decorationBatch":
        for (const set of event.data as DecorationSet[])
          this.installDecoration(set, currentVersion);
        break;
      case "diagnosticSet":
        this.installDiagnostic(event.data as DiagnosticSet, currentVersion);
        break;
      case "foldingRangeSet":
        this.installFold(event.data as FoldingRangeSet, currentVersion);
        break;
      case "completionResult":
        this.completion.install(event.data as CompletionResultSet);
        break;
      case "completionRejected":
        this.completion.reject(
          Number((event.data as { requestId: number }).requestId),
        );
        break;
      case "languageIntelligenceResult":
        this.intelligence.install(event.data as LanguageResult);
        break;
      case "languageIntelligenceRejected":
        this.intelligence.reject(
          Number((event.data as { requestId: number }).requestId),
        );
        break;
      case "selectionQueryResult":
        this.applySelectionResult(event.data as SelectionQueryResult);
        break;
      case "editorCommandRequest": {
        const request = event.data as {
          commandId?: unknown;
          packagePrefix?: unknown;
          modeId?: unknown;
        };
        if (
          typeof request.commandId === "string" &&
          request.commandId.length <= 256 &&
          typeof request.packagePrefix === "string" &&
          request.packagePrefix.length <= 64 &&
          typeof request.modeId === "string" &&
          request.modeId.length <= 128
        )
          this.runManifestCommand(request.commandId);
        break;
      }
      case "caretStyleOverride":
        this.applyCaret(event.data);
        break;
      case "editorLayoutOverride":
        this.applyLayout(event.data);
        break;
    }
  }

  toggleInlays(): void {
    const view = this.view;
    if (!view) return;
    const hidden = view.dom.classList.toggle("cm-clay-inlays-hidden");
    view.dispatch({ effects: showInlays(!hidden) });
  }

  private installDecoration(set: DecorationSet, version: number): void {
    if (!this.accepts(set.documentId, set.documentVersion, version)) return;
    for (const [key, cached] of this.decorationSets) {
      if (cached.documentVersion !== set.documentVersion)
        this.decorationSets.delete(key);
    }
    this.decorationSets.set(
      `${set.packagePrefix}:${set.kind}:${set.viewportByteStart}:${set.viewportByteEnd}`,
      set,
    );
    if (this.view) this.view.dispatch({ effects: replaceDecorations(set) });
  }

  private installDiagnostic(set: DiagnosticSet, version: number): void {
    if (!this.accepts(set.documentId, set.documentVersion, version)) return;
    for (const [key, cached] of this.diagnosticSets) {
      if (cached.documentVersion !== set.documentVersion)
        this.diagnosticSets.delete(key);
    }
    this.diagnosticSets.set(
      `${set.source}:${set.provenance.packagePrefix}:${set.viewportByteStart}:${set.viewportByteEnd}`,
      set,
    );
    if (this.view) this.diagnostics.install(this.view, set);
  }

  private installFold(set: FoldingRangeSet, version: number): void {
    if (!this.accepts(set.documentId, set.documentVersion, version)) return;
    for (const [key, cached] of this.foldSets) {
      if (cached.documentVersion !== set.documentVersion)
        this.foldSets.delete(key);
    }
    this.foldSets.set(set.packagePrefix, set);
    if (this.view)
      this.view.dispatch({
        effects: installFolds(this.view.state.doc.toString(), set),
      });
  }

  private accepts(
    documentId: number,
    version: number,
    currentVersion: number,
  ): boolean {
    const meta = this.options.meta();
    return (
      !!meta && meta.documentId === documentId && version === currentVersion
    );
  }

  private requestViewport(view: EditorView): void {
    const meta = this.options.meta();
    if (!meta || !view.inView) return;
    const text = view.state.doc.toString();
    const from = Math.min(...view.visibleRanges.map((range) => range.from));
    const to = Math.max(...view.visibleRanges.map((range) => range.to));
    const byteStart = utf16ToUtf8(text, from);
    const byteEnd = utf16ToUtf8(text, to);
    const key = `${meta.documentId}:${meta.version + meta.pending}:${byteStart}:${byteEnd}`;
    if (key === this.lastViewport || byteStart === byteEnd) return;
    this.lastViewport = key;
    void this.options.send(
      JSON.stringify({
        family: "decorationViewportRequest",
        payload: {
          clientId: this.options.clientId(),
          documentId: meta.documentId,
          documentVersion: meta.version + meta.pending,
          byteStart,
          byteEnd,
        },
      }),
    );
  }

  private activateLink(view: EditorView, target: DecorationTarget): boolean {
    if ("documentRange" in target) {
      const at = utf8ToUtf16(
        view.state.doc.toString(),
        target.documentRange.range.byteStart,
      );
      view.dispatch({ selection: { anchor: at }, scrollIntoView: true });
      return true;
    }
    if (
      "workspacePath" in target &&
      safeRelativePath(target.workspacePath.relativePath)
    ) {
      this.options.openPath(target.workspacePath.relativePath);
      return true;
    }
    return false;
  }

  private applyCaret(raw: unknown): void {
    if (!this.view) return;
    const style =
      raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
    const shape = String(style.shape ?? "bar");
    const width = Math.max(1, Math.min(8, Number(style.widthPx ?? 2)));
    this.view.dispatch({
      effects: behaviorCompartment.reconfigure([
        behaviorExtensions(this.manifest, (command) =>
          this.runManifestCommand(command),
        ),
        EditorView.theme({
          ".cm-cursor":
            shape === "underline"
              ? {
                  borderLeft: "0",
                  borderBottom: `${width}px solid var(--clay-accent-primary)`,
                  width: "1ch",
                }
              : shape === "block"
                ? {
                    borderLeft: "0",
                    backgroundColor: "var(--clay-accent-primary)",
                    opacity: "0.65",
                    width: "1ch",
                  }
                : { borderLeftWidth: `${width}px` },
        }),
      ]),
    });
  }

  private applyLayout(raw: unknown): void {
    if (!raw || typeof raw !== "object") return;
    const wrap = raw as { none?: unknown; viewport?: unknown; column?: number };
    this.manifest = {
      ...this.manifest,
      editorRules: {
        ...this.manifest.editorRules,
        layout: {
          wrap:
            "none" in wrap
              ? "none"
              : "column" in wrap
                ? { column: wrap.column }
                : "viewport",
        },
      },
    };
    if (this.view)
      this.view.dispatch({
        effects: behaviorCompartment.reconfigure(
          behaviorExtensions(this.manifest, (command) =>
            this.runManifestCommand(command),
          ),
        ),
      });
  }

  runClientCommand(commandId: string): boolean {
    return this.runManifestCommand(commandId);
  }

  private runManifestCommand(commandId: string): boolean {
    const view = this.view;
    if (!view) return false;
    const query = selectionQuery(commandId);
    if (query) return this.requestSelectionQuery(query);
    const local: Record<string, () => boolean> = {
      "editor.clientMoveCursor.nextWordStart": () => cursorGroupForward(view),
      "editor.clientMoveCursor.prevWordStart": () => cursorGroupBackward(view),
      "editor.clientSetSelection.selectWord": () => selectNextOccurrence(view),
      "editor.clientSetSelection.selectLine": () => selectLine(view),
      "editor.clientAddCursor.below": () => addCursorBelow(view),
      "editor.clientAddCursor.above": () => addCursorAbove(view),
      "editor.clientColumnSelect.down": () => addCursorBelow(view),
      "editor.clientColumnSelect.up": () => addCursorAbove(view),
      "editor.clientSelectNextMatch": () => selectNextOccurrence(view),
      "editor.clientSelectPrevMatch": () => selectPreviousMatch(view),
      "editor.clientSelectAllMatches": () => selectSelectionMatches(view),
      "editor.clientCancelMultipleSelections": () => simplifySelection(view),
      "editor.clientKeepSelection": () => true,
      "editor.clientRemoveSelection": () => removeSecondarySelection(view),
      "editor.clientUndoCursorMove": () => undoSelection(view),
      "editor.clientToggleFold": () => foldCode(view),
      "editor.clientUndo": () => undo(view),
      "editor.clientRedo": () => redo(view),
      "editor.clientCopySelection": () => clipboardCopy(view, false),
      "editor.clientCutSelection": () => clipboardCopy(view, true),
      "editor.clientPasteClipboard": () => clipboardPaste(view),
      "editor.toggleInlayHints": () => {
        this.toggleInlays();
        return true;
      },
      "editor.toggleComment": () =>
        this.toggleLinePrefix(
          this.manifest.editorRules?.comments?.[0]?.linePrefix ?? "//",
        ),
      "editor.toggleListMarker": () => this.toggleLinePrefix("- "),
      "editor.rotateHeading": () => this.rotateHeading(),
    };
    const alias =
      commandId.endsWith(".toggleLineComment") ||
      commandId === "markdown.toggleComment"
        ? "editor.toggleComment"
        : commandId === "markdown.toggleList"
          ? "editor.toggleListMarker"
          : commandId === "markdown.insertHeading"
            ? "editor.rotateHeading"
            : commandId;
    if (local[alias]) return local[alias]();
    const meta = this.options.meta();
    if (!meta) return false;
    void this.options.send(
      JSON.stringify({
        family: "commandIntent",
        payload: {
          clientId: this.options.clientId(),
          documentId: meta.documentId,
          behaviorVersion: meta.behaviorVersion,
          commandId,
        },
      }),
    );
    return true;
  }

  private requestSelectionQuery(query: unknown): boolean {
    const view = this.view;
    const meta = this.options.meta();
    if (!view || !meta) return false;
    const requestId = this.nextSelectionRequest++;
    this.pendingSelectionRequest = requestId;
    this.pendingSelections = view.state.selection.ranges.map((range) => ({
      anchor: range.anchor,
      head: range.head,
    }));
    const text = view.state.doc.toString();
    void this.options.send(
      JSON.stringify({
        family: "selectionQueryRequest",
        payload: {
          request: {
            requestId,
            clientId: this.options.clientId(),
            documentId: meta.documentId,
            documentVersion: meta.version + meta.pending,
            behaviorVersion: meta.behaviorVersion,
            query,
            selections: this.pendingSelections.map((range) => ({
              anchor: utf16ToUtf8(text, range.anchor),
              focus: utf16ToUtf8(text, range.head),
            })),
          },
        },
      }),
    );
    return true;
  }

  private applySelectionResult(result: SelectionQueryResult): void {
    const view = this.view;
    const meta = this.options.meta();
    if (
      !view ||
      !meta ||
      result.requestId !== this.pendingSelectionRequest ||
      result.documentId !== meta.documentId ||
      result.documentVersion !== meta.version + meta.pending ||
      result.behaviorVersion !== meta.behaviorVersion
    )
      return;
    this.pendingSelectionRequest = null;
    const text = view.state.doc.toString();
    const ranges = this.pendingSelections.map((original, index) => {
      const resultRange = result.ranges[index];
      if (!resultRange)
        return EditorSelection.range(original.anchor, original.head);
      const start = utf8ToUtf16(
        text,
        Math.min(resultRange.start, resultRange.end),
      );
      const end = utf8ToUtf16(
        text,
        Math.max(resultRange.start, resultRange.end),
      );
      return original.anchor > original.head
        ? EditorSelection.range(end, start)
        : EditorSelection.range(start, end);
    });
    view.dispatch({ selection: EditorSelection.create(ranges) });
  }

  private toggleLinePrefix(prefix: string): boolean {
    const view = this.view;
    if (!view || !prefix || prefix.length > 32) return false;
    const lines = new Map<
      number,
      { from: number; to: number; number: number; text: string }
    >();
    for (const range of view.state.selection.ranges) {
      const start = view.state.doc.lineAt(range.from).number;
      const end = view.state.doc.lineAt(range.to).number;
      for (let number = start; number <= end; number += 1)
        lines.set(number, view.state.doc.line(number));
    }
    const changes = [...lines.values()].map((line) => {
      const indent = line.text.match(/^\s*/)?.[0].length ?? 0;
      const at = line.from + indent;
      return view.state.sliceDoc(at, at + prefix.length) === prefix
        ? { from: at, to: at + prefix.length, insert: "" }
        : { from: at, insert: prefix };
    });
    view.dispatch({ changes });
    return true;
  }

  private rotateHeading(): boolean {
    const view = this.view;
    const prefixes = this.manifest.editorRules?.headingPrefixes ?? [];
    if (!view || !prefixes.length) return false;
    const line = view.state.doc.lineAt(view.state.selection.main.head);
    const current = prefixes.find((prefix) => line.text.startsWith(prefix));
    const next = current
      ? prefixes[(prefixes.indexOf(current) + 1) % prefixes.length]
      : prefixes[0];
    view.dispatch({
      changes: {
        from: line.from,
        to: line.from + (current?.length ?? 0),
        insert: next,
      },
    });
    return true;
  }
}

function clipboardCopy(view: EditorView, cut: boolean): boolean {
  const clipboard = navigator.clipboard;
  if (!clipboard) return false;
  const state = view.state;
  const text = state.selection.ranges
    .map((range) => state.sliceDoc(range.from, range.to))
    .join("\n");
  void clipboard.writeText(text).then(() => {
    if (!cut || view.state !== state) return;
    view.dispatch(
      state.changeByRange((range) => ({
        changes: { from: range.from, to: range.to, insert: "" },
        range: EditorSelection.cursor(range.from),
      })),
    );
  });
  return true;
}

function clipboardPaste(view: EditorView): boolean {
  const clipboard = navigator.clipboard;
  if (!clipboard) return false;
  void clipboard.readText().then((text) => {
    view.dispatch(
      view.state.changeByRange((range) => ({
        changes: { from: range.from, to: range.to, insert: text },
        range: EditorSelection.cursor(range.from + text.length),
      })),
    );
  });
  return true;
}

function selectionQuery(commandId: string): unknown | null {
  const textobject = commandId.match(
    /^editor\.clientSelectTextobject\.(function|class|argument|comment|loop|conditional|call|statement)\.(inner|around)(?:\.(current|next|previous))?$/,
  );
  if (textobject)
    return {
      textobject: {
        kind: textobject[1],
        around: textobject[2] === "around",
        direction: textobject[3] ?? "current",
      },
    };
  const smart = commandId.match(/^editor\.clientSmartSelect\.(expand|shrink)$/);
  return smart ? { smartSelect: { action: smart[1] } } : null;
}

function selectPreviousMatch(view: EditorView): boolean {
  const selected = view.state.selection.main;
  const needle = selected.empty ? view.state.wordAt(selected.head) : selected;
  if (!needle || needle.empty) return false;
  const text = view.state.doc.toString();
  const value = text.slice(needle.from, needle.to);
  const before = text.lastIndexOf(value, Math.max(0, needle.from - 1));
  const at = before >= 0 ? before : text.lastIndexOf(value);
  if (at < 0 || at === needle.from) return false;
  view.dispatch({
    selection: EditorSelection.create([
      ...view.state.selection.ranges,
      EditorSelection.range(at, at + value.length),
    ]),
  });
  return true;
}

function removeSecondarySelection(view: EditorView): boolean {
  const ranges = view.state.selection.ranges;
  if (ranges.length <= 1) return false;
  view.dispatch({ selection: EditorSelection.create(ranges.slice(0, -1)) });
  return true;
}

function targetText(target: DecorationTarget | null): string | null {
  if (!target) return null;
  if ("workspacePath" in target) return target.workspacePath.relativePath;
  if ("displayOnly" in target) return target.displayOnly.text;
  return "Go to definition";
}

function safeRelativePath(path: string): boolean {
  return (
    !!path &&
    !path.startsWith("/") &&
    !path.includes(":") &&
    path.split(/[\\/]/).every((part) => part && part !== "." && part !== "..")
  );
}
