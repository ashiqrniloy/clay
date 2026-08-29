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
import {
  EditorSelection,
  type Extension,
  type TransactionSpec,
} from "@codemirror/state";
import { EditorView, hoverTooltip, keymap } from "@codemirror/view";

import type { BridgeEnvelope } from "../../bridge/types";
import type { DocumentMeta } from "../../state/document-store";
import { behaviorCompartment } from "../compartments";
import { positionIndex } from "../position-index";
import { utf16ToUtf8Indexed, utf8ToUtf16Indexed } from "../position-map";
import {
  editorPerformance,
  PERFORMANCE_STAGE,
  type PerformanceSpan,
} from "../performance";
import { viewportRenderRequestPayload } from "../sync/messages";
import { accessibilityExtension } from "./accessibility";
import { behaviorExtensions } from "./behavior";
import { CompletionProjection } from "./completion";
import {
  clearSyntaxDecorations,
  decorationExtension,
  decorationPatch,
  linkAt,
  retainDecorations,
  showInlays,
} from "./decorations";
import {
  diagnosticExtension,
  diagnosticPatch,
  resetDiagnostics,
} from "./diagnostics";
import { foldingExtension, foldPatch } from "./folding";
import { IntelligenceProjection } from "./intelligence";
import { interactionKeymaps } from "./keymaps";
import type {
  BehaviorManifestDto,
  CompletionResultSet,
  DecorationSet,
  DecorationTarget,
  DiagnosticSet,
  FoldingRangeSet,
  ViewportRenderPatchDto,
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

/**
 * Viewport pacing is explicit: every request carries a monotonic id and the
 * server answers with exactly one atomic `ViewportRenderPatch` (complete,
 * empty, or rejected). The reply — not a timer — frees the pipe.
 */

/** One screen of monospace text is a few KiB; keep the request inside one parse window. */
const VIEWPORT_REQUEST_MAX_CHARS = 64 * 1024;

function visibleViewportRange(view: EditorView): { from: number; to: number } {
  const docLen = view.state.doc.length;
  const first = view.visibleRanges[0];
  let from = first?.from ?? 0;
  let to = first?.to ?? 0;
  if (from > to) {
    const swap = from;
    from = to;
    to = swap;
  }
  from = Math.max(0, Math.min(from, docLen));
  to = Math.max(from, Math.min(to, docLen));
  if (from === to) to = Math.min(docLen, from + VIEWPORT_REQUEST_MAX_CHARS);
  else if (to - from > VIEWPORT_REQUEST_MAX_CHARS)
    to = from + VIEWPORT_REQUEST_MAX_CHARS;
  return { from, to };
}

export class EditorProjection {
  private view: EditorView | null = null;
  private manifest: BehaviorManifestDto = { behaviorVersion: 0 };
  private readonly completion: CompletionProjection;
  private readonly intelligence: IntelligenceProjection;
  readonly extensions: Extension[];
  private lastViewport = "";
  private nextSelectionRequest = 1;
  private pendingSelectionRequest: number | null = null;
  private pendingSelections: readonly { anchor: number; head: number }[] = [];
  /** A viewport request is on the wire; new viewports queue behind it. */
  private viewportInflight = false;
  private viewportTraceId = 0;
  private readonly viewportSpans = new Map<number, PerformanceSpan>();
  /** Newest viewport changed while one was inflight; sent on arrival. */
  private viewportPending = false;
  /** Monotonic viewport request identity; stale patch ids drop on arrival. */
  private nextViewportRequestId = 1;
  private latestViewportRequestId = 0;
  /** Bumped on every local doc change; inflight replies from before the edit drop. */
  private docEpoch = 0;
  private sentEpoch = 0;

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
        if (update.docChanged) this.docEpoch += 1;
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
    // No retained-set replay: fields own render data; the fresh viewport
    // request below repopulates them from the server.
    this.requestViewport(view);
  }

  detach(view: EditorView): void {
    if (this.view === view) this.view = null;
    this.clearViewportTimers();
    this.viewportInflight = false;
    this.viewportSpans.clear();
  }

  clear(): void {
    this.completion.clear();
    this.intelligence.clear();
    this.lastViewport = "";
    this.clearViewportTimers();
    this.viewportInflight = false;
    this.viewportSpans.clear();
    if (this.view) this.view.dispatch(resetDiagnostics(this.view.state));
  }

  /**
   * One editor transaction per envelope: decoration/diagnostic/fold arrivals
   * buffer their state effects and dispatch together. Each dispatch costs a
   * full update cycle (transaction → decorations → DOM), so N frames per
   * batch meant N reflows on every server reply.
   */
  handleEnvelope(envelope: BridgeEnvelope): void {
    if (envelope.kind !== "event") return;
    const event = envelope.data as { kind: string; data: unknown };
    const meta = this.options.meta();
    const currentVersion = meta ? meta.version + meta.pending : -1;
    const effects: TransactionSpec[] = [];
    let patchTraceId = 0;
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
      case "viewportRenderPatch": {
        const patch = event.data as ViewportRenderPatchDto;
        // Stale request: a newer viewport already superseded this one.
        if (patch.requestId < this.latestViewportRequestId) break;
        patchTraceId = patch.traceId ?? this.viewportTraceId;
        if (this.sentEpoch !== this.docEpoch || patch.status === "rejected") {
          // Doc moved since this request, or the server rejected it: do not
          // mutate the current projection, but free the request pipe.
          this.viewportArrived(patchTraceId);
          break;
        }
        if (patch.status === "empty") {
          // Empty is authoritative: no renderable decorations remain for the
          // current document/viewport. Treating it as a no-op left the last
          // keyword fragment painted after deleting the document to empty.
          effects.push({ effects: clearSyntaxDecorations() });
          this.viewportArrived(patchTraceId);
          break;
        }
        if (patch.traceId)
          editorPerformance.mark(
            PERFORMANCE_STAGE.patchDelivery,
            patch.traceId,
            {
              documentId: patch.documentId,
              version: patch.documentVersion,
              byteCount: patch.coveredRanges.reduce(
                (total, range) => total + (range.byteEnd - range.byteStart),
                0,
              ),
              feature: "viewportPatch",
            },
          );
        for (const set of patch.decorations) {
          const effect = this.prepareDecoration(set, currentVersion);
          if (effect) effects.push(effect);
        }
        for (const set of patch.diagnostics) {
          const effect = this.prepareDiagnostic(set, currentVersion);
          if (effect) effects.push(effect);
        }
        for (const set of patch.folds) {
          const effect = this.prepareFold(set, currentVersion);
          if (effect) effects.push(effect);
        }
        if (this.view) {
          const { from, to } = visibleViewportRange(this.view);
          if (from !== to)
            effects.push({ effects: retainDecorations({ from, to }) });
        }
        this.viewportArrived(patchTraceId);
        break;
      }
      case "decorationSet": {
        const set = event.data as DecorationSet;
        patchTraceId = set.traceId ?? this.viewportTraceId;
        editorPerformance.mark(PERFORMANCE_STAGE.patchDelivery, patchTraceId, {
          documentId: set.documentId,
          version: set.documentVersion,
          byteCount: Math.max(0, set.viewportByteEnd - set.viewportByteStart),
          feature: set.kind,
        });
        const effect = this.prepareDecoration(set, currentVersion);
        if (effect) effects.push(effect);
        break;
      }
      case "decorationBatch": {
        const sets = event.data as DecorationSet[];
        patchTraceId = sets[0]?.traceId ?? this.viewportTraceId;
        const first = sets[0];
        if (first)
          editorPerformance.mark(
            PERFORMANCE_STAGE.patchDelivery,
            patchTraceId,
            {
              documentId: first.documentId,
              version: first.documentVersion,
              byteCount: Math.max(
                0,
                sets.reduce(
                  (total, set) =>
                    total + set.viewportByteEnd - set.viewportByteStart,
                  0,
                ),
              ),
              feature: first.kind,
            },
          );
        for (const set of sets) {
          const effect = this.prepareDecoration(set, currentVersion);
          if (effect) effects.push(effect);
        }
        break;
      }
      case "diagnosticSet": {
        patchTraceId = this.viewportTraceId;
        if (!this.view) break;
        const effect = this.prepareDiagnostic(
          event.data as DiagnosticSet,
          currentVersion,
        );
        if (effect) effects.push(effect);
        break;
      }
      case "foldingRangeSet": {
        patchTraceId = this.viewportTraceId;
        const effect = this.prepareFold(
          event.data as FoldingRangeSet,
          currentVersion,
        );
        if (effect) effects.push(effect);
        break;
      }
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
    // Multiple specs join into one transaction — one update cycle total.
    if (effects.length && this.view) {
      const apply = editorPerformance.span(
        PERFORMANCE_STAGE.patchApply,
        patchTraceId,
        {
          documentId: meta?.documentId,
          version: meta?.version,
        },
      );
      this.view.dispatch(...effects);
      apply.end();
      editorPerformance.frame(patchTraceId, {
        documentId: meta?.documentId,
        version: meta?.version,
      });
    }
    // Edit-driven member traffic no longer paces the viewport pipe; only
    // the atomic patch reply does (see the viewportRenderPatch arm).
  }

  toggleInlays(): void {
    const view = this.view;
    if (!view) return;
    const hidden = view.dom.classList.toggle("cm-clay-inlays-hidden");
    view.dispatch({ effects: showInlays(!hidden) });
  }

  private prepareDecoration(
    set: DecorationSet,
    version: number,
  ): TransactionSpec | null {
    if (
      !this.view ||
      !this.accepts(set.documentId, set.documentVersion, version)
    )
      return null;
    // Split payloads share one parse window; pruning each fragment would
    // drop syntax at the top of large files. Viewport retain handles bounds.
    return { effects: decorationPatch(this.view.state, set, false) };
  }

  private prepareDiagnostic(
    set: DiagnosticSet,
    version: number,
  ): TransactionSpec | null {
    if (
      !this.view ||
      !this.accepts(set.documentId, set.documentVersion, version)
    )
      return null;
    return diagnosticPatch(this.view.state, set);
  }

  private prepareFold(
    set: FoldingRangeSet,
    version: number,
  ): TransactionSpec | null {
    if (
      !this.view ||
      !this.accepts(set.documentId, set.documentVersion, version)
    )
      return null;
    return { effects: foldPatch(this.view.state, set) };
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
    if (!meta) return;
    // Progressive loading gates edits, not syntax. The loaded prefix is
    // authoritative at this version, so its visible viewport can be parsed
    // while later chunks continue appending.
    // First on-screen fragment only: min/max across line-gap fragments of a
    // long line is 0..doc.length, which used to schedule 24 parse windows,
    // stall the atomic remaining counter, and show no syntax until scroll.
    // Skip view.inView — WebKitGTK's first measure often has a 0-height
    // pixel viewport, which would drop the open request entirely.
    const { from, to } = visibleViewportRange(view);
    // Indexed conversion: O(log lines). The previous implementation
    // flattened the whole document and linearly scanned it on every scroll
    // tick and keystroke, which froze large files.
    const index = positionIndex(view.state);
    const byteStart = utf16ToUtf8Indexed(index, from);
    const byteEnd = utf16ToUtf8Indexed(index, to);
    const key = `${meta.documentId}:${meta.version + meta.pending}:${byteStart}:${byteEnd}`;
    if (key === this.lastViewport || byteStart === byteEnd) return;
    // Inflight pacing instead of a fixed debounce: the first request for a
    // new viewport goes out immediately (highlight latency ≈ round trip),
    // scroll storms collapse into latest-wins follow-ups that fire the moment
    // the previous reply lands.
    if (this.viewportInflight) {
      this.viewportPending = true;
      return;
    }
    this.lastViewport = key;
    this.viewportInflight = true;
    this.sentEpoch = this.docEpoch;
    const requestId = this.nextViewportRequestId;
    this.nextViewportRequestId += 1;
    this.latestViewportRequestId = requestId;
    const traceId = editorPerformance.trace();
    this.viewportTraceId = traceId;
    const metadata = {
      documentId: meta.documentId,
      version: meta.version + meta.pending,
      byteCount: Math.max(0, byteEnd - byteStart),
    };
    editorPerformance.mark(
      PERFORMANCE_STAGE.browserViewport,
      traceId,
      metadata,
    );
    const scroll = editorPerformance.span(
      PERFORMANCE_STAGE.editorScroll,
      traceId,
      metadata,
    );
    scroll.end();
    this.viewportSpans.set(
      traceId,
      editorPerformance.span(
        PERFORMANCE_STAGE.editorSyntaxFresh,
        traceId,
        metadata,
      ),
    );
    const enqueue = editorPerformance.span(
      PERFORMANCE_STAGE.bridgeEnqueue,
      traceId,
      metadata,
    );
    void this.options
      .send(
        viewportRenderRequestPayload(
          meta.documentId,
          meta.version + meta.pending,
          requestId,
          byteStart,
          byteEnd,
          traceId,
          this.options.clientId(),
        ),
      )
      .then(() => enqueue.end())
      .catch(() => enqueue.end());
  }

  /** The atomic patch reply freed the pipe; send the newest viewport. */
  private viewportArrived(traceId = this.viewportTraceId): void {
    this.finishViewportTrace(traceId);
    this.viewportInflight = false;
    this.pumpViewport();
  }

  private finishViewportTrace(traceId: number): void {
    if (traceId <= 0) return;
    this.viewportSpans.get(traceId)?.end();
    this.viewportSpans.delete(traceId);
    if (traceId === this.viewportTraceId) this.viewportTraceId = 0;
  }

  private pumpViewport(): void {
    if (!this.viewportPending) return;
    this.viewportPending = false;
    if (this.view) this.requestViewport(this.view);
  }

  private clearViewportTimers(): void {
    this.viewportPending = false;
  }

  private activateLink(view: EditorView, target: DecorationTarget): boolean {
    if ("documentRange" in target) {
      const at = utf8ToUtf16Indexed(
        positionIndex(view.state),
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
    const index = positionIndex(view.state);
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
              anchor: utf16ToUtf8Indexed(index, range.anchor),
              focus: utf16ToUtf8Indexed(index, range.head),
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
    const index = positionIndex(view.state);
    const ranges = this.pendingSelections.map((original, index_) => {
      const resultRange = result.ranges[index_];
      if (!resultRange)
        return EditorSelection.range(original.anchor, original.head);
      const start = utf8ToUtf16Indexed(
        index,
        Math.min(resultRange.start, resultRange.end),
      );
      const end = utf8ToUtf16Indexed(
        index,
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
