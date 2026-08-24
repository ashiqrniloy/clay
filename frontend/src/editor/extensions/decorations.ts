import { StateEffect, StateField, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  WidgetType,
  type DecorationSet as CmDecorationSet,
} from "@codemirror/view";

import { utf8ToUtf16 } from "../position-map";
import type { DecorationSet, DecorationTarget, TokenType } from "./types";

const replaceDecorationChunk = StateEffect.define<DecorationSet>();
const clearDecorations = StateEffect.define<null>();
const setInlaysVisible = StateEffect.define<boolean>();

interface DecorationState {
  chunks: ReadonlyMap<string, DecorationSet>;
  ranges: CmDecorationSet;
  links: readonly { from: number; to: number; target: DecorationTarget }[];
  inlaysVisible: boolean;
  documentVersion: number | null;
}

function chunkKey(set: DecorationSet): string {
  return `${set.packagePrefix}:${set.kind}:${set.viewportByteStart}:${set.viewportByteEnd}`;
}

function modifierStyle(bits: number): Record<string, string> {
  const style: Record<string, string> = {};
  if (bits & (1 << 10)) style.fontWeight = "bold";
  if (bits & (1 << 11)) style.fontStyle = "italic";
  const lines: string[] = [];
  if (bits & (1 << 12)) lines.push("underline");
  if (bits & (1 << 13)) lines.push("line-through");
  if (lines.length) style.textDecoration = lines.join(" ");
  return style;
}

function tokenPrefix(token: TokenType | "searchMatch"): string {
  return `--clay-editor-${token.toLowerCase()}`;
}

class InlayWidget extends WidgetType {
  constructor(readonly label: string) {
    super();
  }
  override eq(other: InlayWidget): boolean {
    return other.label === this.label;
  }
  override toDOM(): HTMLElement {
    const node = document.createElement("span");
    node.className = "cm-clay-inlay";
    node.dataset.label = this.label;
    node.setAttribute("aria-hidden", "true");
    node.setAttribute("contenteditable", "false");
    return node;
  }
  override ignoreEvent(): boolean {
    return true;
  }
}

function project(
  chunks: Iterable<DecorationSet>,
  text: string,
  inlaysVisible: boolean,
): Pick<DecorationState, "ranges" | "links"> {
  const marks: Array<{
    from: number;
    to: number;
    priority: number;
    decoration: Decoration;
  }> = [];
  const links: Array<{ from: number; to: number; target: DecorationTarget }> =
    [];
  for (const set of chunks) {
    for (const span of set.spans) {
      const from = utf8ToUtf16(text, span.byteStart);
      const to = utf8ToUtf16(text, span.byteEnd);
      if (from > to || to > text.length) continue;
      if (span.kind === "inlayHint" && span.inlay) {
        if (!inlaysVisible) continue;
        const side = span.inlay.placement === "before" ? -1 : 1;
        marks.push({
          from: side < 0 ? from : to,
          to: side < 0 ? from : to,
          priority: span.priority,
          decoration: Decoration.widget({
            widget: new InlayWidget(span.inlay.label),
            side,
          }),
        });
        continue;
      }
      if (from === to) continue;
      const prefix = tokenPrefix(
        span.kind === "searchMatch" ? "searchMatch" : span.tokenType,
      );
      const style: Record<string, string> = {
        color: `var(${prefix}-color, var(--clay-text-primary))`,
        fontWeight: `var(${prefix}-weight, normal)`,
        fontStyle: `var(${prefix}-style, normal)`,
        textDecoration: `var(${prefix}-decoration, none)`,
        ...modifierStyle(span.modifiers),
      };
      if (span.fontRole && span.fontRole !== "inherit") {
        style.fontFamily = `var(--clay-font-${span.fontRole})`;
      }
      style.backgroundColor = `var(${prefix}-background, transparent)`;
      style.fontSize = `calc(1em * var(${prefix}-scale, 1))`;
      if (span.kind === "link") style.textDecoration = "underline";
      marks.push({
        from,
        to,
        priority: span.priority,
        decoration: Decoration.mark({
          class: span.kind === "link" ? "cm-clay-link" : undefined,
          attributes: {
            style: Object.entries(style)
              .map(
                ([k, v]) =>
                  `${k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}:${v}`,
              )
              .join(";"),
          },
        }),
      });
      if (span.kind === "link" && span.target)
        links.push({ from, to, target: span.target });
    }
  }
  marks.sort(
    (a, b) => a.from - b.from || a.priority - b.priority || a.to - b.to,
  );
  return {
    ranges: Decoration.set(
      marks.map((item) => item.decoration.range(item.from, item.to)),
      true,
    ),
    links,
  };
}

const decorationField = StateField.define<DecorationState>({
  create: () => ({
    chunks: new Map(),
    ranges: Decoration.none,
    links: [],
    inlaysVisible: true,
    documentVersion: null,
  }),
  update(value, transaction) {
    let chunks = value.chunks;
    let inlaysVisible = value.inlaysVisible;
    let documentVersion = value.documentVersion;
    let replaced = false;
    for (const effect of transaction.effects) {
      if (effect.is(clearDecorations)) {
        chunks = new Map();
        replaced = true;
      } else if (effect.is(setInlaysVisible)) {
        inlaysVisible = effect.value;
        replaced = true;
      } else if (effect.is(replaceDecorationChunk)) {
        const next =
          effect.value.documentVersion === documentVersion
            ? new Map(chunks)
            : new Map<string, DecorationSet>();
        documentVersion = effect.value.documentVersion;
        next.set(chunkKey(effect.value), effect.value);
        chunks = next;
        replaced = true;
      }
    }
    if (replaced) {
      const projected = project(
        chunks.values(),
        transaction.state.doc.toString(),
        inlaysVisible,
      );
      return { chunks, inlaysVisible, documentVersion, ...projected };
    }
    if (transaction.docChanged)
      return { ...value, ranges: value.ranges.map(transaction.changes) };
    return value;
  },
  provide: (field) =>
    EditorView.decorations.from(field, (value) => value.ranges),
});

export const decorationExtension: Extension = decorationField;
export const replaceDecorations = (set: DecorationSet) =>
  replaceDecorationChunk.of(set);
export const resetDecorations = () => clearDecorations.of(null);
export const showInlays = (visible: boolean) => setInlaysVisible.of(visible);

export function linkAt(
  view: EditorView,
  position: number,
): DecorationTarget | null {
  const state = view.state.field(decorationField, false);
  return (
    state?.links.find((link) => link.from <= position && position <= link.to)
      ?.target ?? null
  );
}
