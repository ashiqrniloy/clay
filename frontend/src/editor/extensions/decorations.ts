import {
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
  type StateEffect as StateEffectValue,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  WidgetType,
  type DecorationSet as CmDecorationSet,
} from "@codemirror/view";

import { positionIndex } from "../position-index";
import { utf8ToUtf16Batch } from "../position-map";
import type { DecorationSet, DecorationTarget, TokenType } from "./types";
import {
  applyRenderPatch,
  coveredRangeOf,
  guardOf,
  mapItems,
  pruneOutside,
  replaceCovered,
  unionRange,
  type ByteRange16,
  type InlayItem,
  type LinkItem,
  type MarkItem,
  type RenderPatch,
} from "./render-patch";

const setInlaysVisible = StateEffect.define<boolean>();

interface DecorationState {
  marks: MarkItem[];
  inlays: InlayItem[];
  links: LinkItem[];
  inlaysVisible: boolean;
  ranges: CmDecorationSet;
}

/** Closed token-class table: wire token types can never inject CSS. */
const TOKEN_CLASSES: Readonly<Record<TokenType | "searchMatch", string>> = {
  namespace: "cm-clay-t-namespace",
  type: "cm-clay-t-type",
  class: "cm-clay-t-class",
  enum: "cm-clay-t-enum",
  interface: "cm-clay-t-interface",
  struct: "cm-clay-t-struct",
  typeParameter: "cm-clay-t-typeparameter",
  parameter: "cm-clay-t-parameter",
  variable: "cm-clay-t-variable",
  property: "cm-clay-t-property",
  enumMember: "cm-clay-t-enummember",
  event: "cm-clay-t-event",
  function: "cm-clay-t-function",
  method: "cm-clay-t-method",
  macro: "cm-clay-t-macro",
  keyword: "cm-clay-t-keyword",
  modifier: "cm-clay-t-modifier",
  comment: "cm-clay-t-comment",
  string: "cm-clay-t-string",
  number: "cm-clay-t-number",
  regexp: "cm-clay-t-regexp",
  operator: "cm-clay-t-operator",
  decorator: "cm-clay-t-decorator",
  heading1: "cm-clay-t-heading1",
  heading2: "cm-clay-t-heading2",
  heading3: "cm-clay-t-heading3",
  heading4: "cm-clay-t-heading4",
  heading5: "cm-clay-t-heading5",
  heading6: "cm-clay-t-heading6",
  listItem: "cm-clay-t-listitem",
  quote: "cm-clay-t-quote",
  codeBlock: "cm-clay-t-codeblock",
  codeSpan: "cm-clay-t-codespan",
  link: "cm-clay-t-link",
  paragraph: "cm-clay-t-paragraph",
  searchMatch: "cm-clay-t-searchmatch",
};

function tokenClass(token: TokenType | "searchMatch"): string {
  // Unknown wire values fall back to an inert class instead of reaching CSS.
  return TOKEN_CLASSES[token] ?? TOKEN_CLASSES.paragraph;
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

function markClasses(
  token: TokenType | "searchMatch",
  modifiers: number,
  fontRole: string | null,
  link: boolean,
): string {
  const classes = [tokenClass(token)];
  if (modifiers & (1 << 10)) classes.push("cm-clay-m-bold");
  if (modifiers & (1 << 11)) classes.push("cm-clay-m-italic");
  if (modifiers & (1 << 12)) classes.push("cm-clay-m-underline");
  if (modifiers & (1 << 13)) classes.push("cm-clay-m-strikethrough");
  if (fontRole === "monospace" || fontRole === "proportional")
    classes.push(`cm-clay-f-${fontRole}`);
  if (link) classes.push("cm-clay-link");
  return classes.join(" ");
}

/**
 * Project a validated server set into UTF-16 render items exactly once, at
 * patch-construction time. Retained items are never re-projected — later
 * patches only replace their covered range and prune outside the guard.
 */
export function decorationPatch(
  state: EditorState,
  set: DecorationSet,
  prune = true,
): StateEffectValue<RenderPatch> {
  const index = positionIndex(state);
  const covered = coveredRangeOf(
    index,
    set.viewportByteStart,
    set.viewportByteEnd,
  );
  const authority = `${set.packagePrefix}:${set.kind}`;
  // One resumable scan per line: batch-convert every span boundary first
  // instead of one line scan per span.
  const converted = utf8ToUtf16Batch(
    index,
    set.spans.flatMap((span) => [span.byteStart, span.byteEnd]),
  );
  const marks: MarkItem[] = [];
  const inlays: InlayItem[] = [];
  const links: LinkItem[] = [];
  for (let i = 0; i < set.spans.length; i += 1) {
    const span = set.spans[i];
    if (!span) continue;
    const from = converted[i * 2] ?? 0;
    const to = converted[i * 2 + 1] ?? 0;
    if (from > to || to > index.totalUtf16) continue;
    if (span.kind === "inlayHint" && span.inlay) {
      const side = span.inlay.placement === "before" ? -1 : 1;
      const at = side < 0 ? from : to;
      inlays.push({
        from: at,
        to: at,
        authority,
        priority: span.priority,
        decoration: Decoration.widget({
          widget: new InlayWidget(span.inlay.label),
          side,
        }),
      });
      continue;
    }
    if (from === to) continue;
    if (span.kind === "link" && span.target)
      links.push({ from, to, authority, target: span.target });
    marks.push({
      from,
      to,
      authority,
      priority: span.priority,
      decoration: Decoration.mark({
        class: markClasses(
          span.kind === "searchMatch" ? "searchMatch" : span.tokenType,
          span.modifiers,
          span.fontRole,
          span.kind === "link",
        ),
      }),
    });
  }
  return applyRenderPatch.of({
    kind: "decoration",
    authority,
    covered,
    marks,
    inlays,
    links,
    prune,
  });
}

export function retainDecorations(covered: ByteRange16) {
  return applyRenderPatch.of({ kind: "retain", covered });
}

function buildRanges(
  marks: readonly MarkItem[],
  inlays: readonly InlayItem[],
  inlaysVisible: boolean,
): CmDecorationSet {
  const all = inlaysVisible
    ? [...marks, ...(inlays as readonly MarkItem[])]
    : marks;
  if (!all.length) return Decoration.none;
  return Decoration.set(
    all.map((item) => item.decoration.range(item.from, item.to)),
    true,
  );
}

const decorationField = StateField.define<DecorationState>({
  create: () => ({
    marks: [],
    inlays: [],
    links: [],
    inlaysVisible: true,
    ranges: Decoration.none,
  }),
  update(value, transaction) {
    let { marks, inlays, links, inlaysVisible } = value;
    let dirty = false;
    let pruneCovered: ByteRange16 | null = null;
    if (transaction.docChanged) {
      const mappedMarks = mapItems(value.marks, transaction.changes);
      marks =
        mappedMarks === value.marks
          ? value.marks
          : mappedMarks.filter((item) => item.from < item.to);
      inlays = mapItems(value.inlays, transaction.changes);
      links = mapItems(value.links, transaction.changes);
      dirty = true;
    }
    for (const effect of transaction.effects) {
      if (effect.is(setInlaysVisible)) {
        inlaysVisible = effect.value;
        dirty = true;
      } else if (effect.is(applyRenderPatch)) {
        const patch = effect.value;
        if (patch.kind === "reset") {
          marks = [];
          inlays = [];
          links = [];
          pruneCovered = null;
          dirty = true;
        } else if (patch.kind === "retain") {
          pruneCovered = unionRange(pruneCovered, patch.covered);
          dirty = true;
        } else if (patch.kind === "decoration") {
          marks = replaceCovered(
            marks,
            patch.authority,
            patch.covered,
            patch.marks,
          );
          inlays = replaceCovered(
            inlays,
            patch.authority,
            patch.covered,
            patch.inlays,
          );
          links = replaceCovered(
            links,
            patch.authority,
            patch.covered,
            patch.links,
          );
          if (patch.prune !== false)
            pruneCovered = unionRange(pruneCovered, patch.covered);
          dirty = true;
        }
      }
    }
    if (pruneCovered) {
      const guard = guardOf(pruneCovered);
      marks = pruneOutside(marks, guard);
      inlays = pruneOutside(inlays, guard);
      links = pruneOutside(links, guard);
    }
    if (!dirty) return value;
    return {
      marks,
      inlays,
      links,
      inlaysVisible,
      ranges: buildRanges(marks, inlays, inlaysVisible),
    };
  },
  provide: (field) =>
    EditorView.decorations.from(field, (value) => value.ranges),
});

export const decorationExtension: Extension = decorationField;

export const replaceDecorations = (state: EditorState, set: DecorationSet) =>
  decorationPatch(state, set);
export const resetDecorations = () => applyRenderPatch.of({ kind: "reset" });
export const showInlays = (visible: boolean) => setInlaysVisible.of(visible);

export interface DecorationStats {
  marks: number;
  inlays: number;
  links: number;
}

export function decorationStats(state: EditorState): DecorationStats {
  const value = state.field(decorationField);
  return {
    marks: value.marks.length,
    inlays: value.inlays.length,
    links: value.links.length,
  };
}

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
