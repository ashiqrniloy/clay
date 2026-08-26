import {
  StateEffect,
  StateField,
  type Extension,
  type Text,
} from "@codemirror/state";
import { foldGutter, foldKeymap, foldService } from "@codemirror/language";
import { keymap } from "@codemirror/view";

import { utf8ToUtf16Indexed, textIndex, type TextIndex } from "../position-map";
import type { FoldingRangeSet } from "./types";

interface FoldRange {
  from: number;
  to: number;
  label: string | null;
}
const replaceFolds = StateEffect.define<{
  key: string;
  version: number;
  ranges: FoldRange[];
}>();
const clearFolds = StateEffect.define<null>();

interface FoldState {
  version: number | null;
  sets: ReadonlyMap<string, readonly FoldRange[]>;
}

const foldField = StateField.define<FoldState>({
  create: () => ({ version: null, sets: new Map() }),
  update(value, transaction) {
    let next = value;
    for (const effect of transaction.effects) {
      if (effect.is(clearFolds)) next = { version: null, sets: new Map() };
      else if (effect.is(replaceFolds)) {
        const map =
          effect.value.version === next.version
            ? new Map(next.sets)
            : new Map<string, readonly FoldRange[]>();
        map.set(effect.value.key, effect.value.ranges);
        next = { version: effect.value.version, sets: map };
      }
    }
    return next;
  },
});

export const foldingExtension: Extension = [
  foldField,
  foldService.of((state, lineStart, lineEnd) => {
    const sets = state.field(foldField).sets;
    let candidate: FoldRange | null = null;
    for (const ranges of sets.values())
      for (const range of ranges) {
        if (
          range.from < lineStart ||
          range.from > lineEnd ||
          range.to <= lineEnd
        )
          continue;
        if (!candidate || range.to < candidate.to) candidate = range;
      }
    return candidate ? { from: lineEnd, to: candidate.to } : null;
  }),
  foldGutter(),
  keymap.of(foldKeymap),
];

export function installFolds(
  docOrIndex: Text | TextIndex,
  set: FoldingRangeSet,
) {
  const index = isIndex(docOrIndex) ? docOrIndex : textIndex(docOrIndex);
  return replaceFolds.of({
    key: set.packagePrefix,
    version: set.documentVersion,
    ranges: set.ranges
      .map((range) => ({
        from: utf8ToUtf16Indexed(index, range.byteStart),
        to: utf8ToUtf16Indexed(index, range.byteEnd),
        label: range.label,
      }))
      .filter((range) => range.from < range.to),
  });
}

function isIndex(value: Text | TextIndex): value is TextIndex {
  return "utf16Starts" in value;
}

export const resetFolds = () => clearFolds.of(null);
