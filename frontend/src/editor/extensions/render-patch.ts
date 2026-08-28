import { type ChangeSet, StateEffect } from "@codemirror/state";
import type { Decoration } from "@codemirror/view";

import type { BytePositionIndex } from "../position-index";
import { utf8ToUtf16Indexed } from "../position-map";
import type { DecorationTarget } from "./types";

/**
 * One atomic render patch. Feature fields are the sole render-data owners:
 * the effect carries already-projected UTF-16 items, the covered viewport
 * range it replaces, and nothing else. One server patch = one effect inside
 * one transaction = one state update.
 */
export type RenderPatch =
  | {
      kind: "decoration";
      authority: string;
      covered: ByteRange16;
      marks: MarkItem[];
      inlays: InlayItem[];
      links: LinkItem[];
      /** Default true. False keeps siblings outside `covered` (split payloads). */
      prune?: boolean;
    }
  | {
      kind: "diagnostic";
      authority: string;
      covered: ByteRange16;
      items: DiagnosticItem[];
    }
  | { kind: "fold"; authority: string; ranges: FoldItem[] }
  | { kind: "reset" }
  | { kind: "retain"; covered: ByteRange16 };

export const applyRenderPatch = StateEffect.define<RenderPatch>();

export interface ByteRange16 {
  from: number;
  to: number;
}

/** Render item common shape: UTF-16 range plus the authority that owns it. */
export interface RangedItem extends ByteRange16 {
  authority: string;
}

export interface MarkItem extends RangedItem {
  priority: number;
  decoration: Decoration;
}

export interface InlayItem extends RangedItem {
  priority: number;
  decoration: Decoration;
}

export interface LinkItem extends RangedItem {
  target: DecorationTarget;
}

export interface DiagnosticItem extends RangedItem {
  severity: "error" | "warning" | "info";
  message: string;
  source: string;
}

export interface FoldItem extends RangedItem {
  label: string | null;
}

/**
 * Retained render data stays within the covered range plus this overscan
 * (positions each side, widened to the covered size for small viewports).
 */
const VIEWPORT_OVERSCAN = 4_096;

export function coveredRangeOf(
  index: BytePositionIndex,
  byteStart: number,
  byteEnd: number,
): ByteRange16 {
  const from = utf8ToUtf16Indexed(index, byteStart);
  const to = utf8ToUtf16Indexed(index, byteEnd);
  return from <= to ? { from, to } : { from: to, to: from };
}

export function guardOf(covered: ByteRange16): ByteRange16 {
  const overscan = Math.max(VIEWPORT_OVERSCAN, covered.to - covered.from);
  return {
    from: Math.max(0, covered.from - overscan),
    to: covered.to + overscan,
  };
}

export function unionRange(
  left: ByteRange16 | null,
  right: ByteRange16,
): ByteRange16 {
  if (!left) return right;
  return {
    from: Math.min(left.from, right.from),
    to: Math.max(left.to, right.to),
  };
}

/**
 * Complete covered ranges replace exact prior authority: same-authority
 * items intersecting `covered` are removed, other authorities and
 * same-authority items outside `covered` are retained, additions appended.
 * Result stays sorted by (from, to).
 */
export function replaceCovered<T extends RangedItem>(
  items: readonly T[],
  authority: string,
  covered: ByteRange16,
  additions: readonly T[],
): T[] {
  const merged = items.filter(
    (item) =>
      item.authority !== authority ||
      item.to <= covered.from ||
      item.from >= covered.to,
  );
  merged.push(...additions);
  merged.sort((left, right) => left.from - right.from || left.to - right.to);
  return merged;
}

/** Prune retained items outside the guard (visible + bounded overscan). */
export function pruneOutside<T extends RangedItem>(
  items: readonly T[],
  guard: ByteRange16,
): T[] {
  return items.filter((item) => item.from < guard.to && item.to > guard.from);
}

/**
 * Map items through a document change with copy-on-write: untouched items
 * keep their object identity so history states share structure. Collapsed
 * items clamp to empty (from === to); callers drop them where emptiness is
 * invalid (marks, folds).
 */
export function mapItems<T extends ByteRange16>(
  items: readonly T[],
  changes: ChangeSet,
): T[] {
  let copy: T[] | null = null;
  for (let index = 0; index < items.length; index += 1) {
    const item = items[index];
    if (!item) continue;
    const from = changes.mapPos(item.from);
    const to = Math.max(from, changes.mapPos(item.to, 1));
    if (from === item.from && to === item.to) continue;
    if (!copy) copy = items.slice();
    copy[index] = { ...item, from, to };
  }
  return copy ?? (items as T[]);
}
