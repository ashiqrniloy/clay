import { EditorState, type Extension, type Range } from "@codemirror/state";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { bracketMatching, foldKeymap } from "@codemirror/language";
import {
  Decoration,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  ViewPlugin,
} from "@codemirror/view";

import { insertAtSelections } from "./keymaps";
import type { BehaviorManifestDto, KeyBindingDto, KeyStrokeDto } from "./types";

export function behaviorExtensions(
  manifest: BehaviorManifestDto,
  onCommand?: (commandId: string) => boolean,
): Extension {
  const rules = manifest.editorRules ?? {};
  const tab = rules.tab;
  const spaces = Math.max(1, Math.min(16, tab?.spacesPerTab ?? 4));
  const tabText =
    tab?.mode === "insertTabCharacter" ? "\t" : " ".repeat(spaces);
  const pairs = (rules.pairs ?? [])
    .map((pair) => pair.open)
    .filter((open) => open.length === 1);
  const chrome =
    rules.chrome ??
    (manifest.documentFontRole === "monospace"
      ? {
          gutter: true,
          activeLine: true,
          indentGuides: true,
          bracketMatch: true,
          inlayHints: true,
        }
      : {
          gutter: false,
          activeLine: false,
          indentGuides: false,
          bracketMatch: false,
          inlayHints: false,
        });

  const extensions: Extension[] = [
    EditorState.languageData.of(() => [
      { closeBrackets: { brackets: pairs.length ? pairs : undefined } },
    ]),
    closeBrackets(),
    keymap.of(closeBracketsKeymap),
    keymap.of([
      { key: "Enter", run: (view) => applyEnterRule(view, rules.enter) },
      {
        key: "Tab",
        preventDefault: true,
        run: (view) => insertAtSelections(view, tabText),
      },
    ]),
    electricOutdent(spaces),
    manifestKeymaps(manifest, onCommand),
    fontAndWrap(manifest),
  ];
  if (chrome.gutter)
    extensions.push(lineNumbers(), highlightActiveLineGutter());
  if (chrome.activeLine) extensions.push(highlightActiveLine());
  if (chrome.bracketMatch)
    extensions.push(bracketMatching(), keymap.of(foldKeymap));
  if (chrome.indentGuides) extensions.push(indentGuides(spaces));
  return extensions;
}

export function applyEnterRule(view: EditorView, raw: unknown): boolean {
  const line = view.state.doc.lineAt(view.state.selection.main.head);
  const before = view.state.sliceDoc(line.from, view.state.selection.main.head);
  const indent = before.match(/^\s*/)?.[0] ?? "";
  let suffix = indent;
  if (raw && typeof raw === "object" && "continueLineMarkers" in raw) {
    const rule = (
      raw as {
        continueLineMarkers: { markers?: string[]; exitOnEmptyItem?: boolean };
      }
    ).continueLineMarkers;
    const body = before.slice(indent.length);
    const marker = markerFor(body, rule.markers ?? []);
    if (marker) {
      if (rule.exitOnEmptyItem && body.trim() === marker.trim()) {
        view.dispatch({
          changes: {
            from: line.from + indent.length,
            to: view.state.selection.main.head,
            insert: "",
          },
        });
        return true;
      }
      suffix += marker;
    }
  } else if (raw === "insertNewlineOnly") suffix = "";
  return insertAtSelections(view, `\n${suffix}`);
}

function markerFor(text: string, markers: readonly string[]): string | null {
  const ordered = text.match(/^(\d+)\.\s+/);
  if (ordered && markers.includes("ordered-dot"))
    return `${Number(ordered[1]) + 1}. `;
  return markers.find(
    (marker) => marker !== "ordered-dot" && text.startsWith(`${marker} `),
  )
    ? `${markers.find((marker) => marker !== "ordered-dot" && text.startsWith(`${marker} `))} `
    : null;
}

function electricOutdent(spaces: number): Extension {
  return EditorView.inputHandler.of((view, from, to, text) => {
    if (!"})]".includes(text) || from !== to) return false;
    const line = view.state.doc.lineAt(from);
    const prefix = view.state.sliceDoc(line.from, from);
    if (prefix.trim()) return false;
    const remove = Math.min(prefix.length, spaces);
    view.dispatch({
      changes: [
        { from: from - remove, to: from, insert: "" },
        { from, to, insert: text },
      ],
    });
    return true;
  });
}

function indentGuides(spaces: number): Extension {
  const build = (view: EditorView) => {
    const ranges: Range<Decoration>[] = [];
    for (const viewport of view.visibleRanges) {
      let line = view.state.doc.lineAt(viewport.from);
      while (line.from <= viewport.to) {
        const indent = line.text.match(/^[ \t]+/)?.[0].length ?? 0;
        if (indent > 0) {
          ranges.push(
            Decoration.mark({ class: "cm-clay-indent" }).range(
              line.from,
              line.from + indent,
            ),
          );
        }
        if (line.to >= viewport.to || line.number >= view.state.doc.lines)
          break;
        line = view.state.doc.line(line.number + 1);
      }
    }
    return Decoration.set(ranges, true);
  };
  return [
    ViewPlugin.define(
      (view) => ({
        decorations: build(view),
        update(update) {
          if (update.docChanged || update.viewportChanged)
            this.decorations = build(update.view);
        },
      }),
      { decorations: (value) => value.decorations },
    ),
    EditorView.theme({
      ".cm-clay-indent": {
        backgroundImage:
          "linear-gradient(to right, var(--clay-border-hairline) 1px, transparent 1px)",
        backgroundSize: `${spaces}ch 100%`,
      },
    }),
  ];
}

function manifestKeymaps(
  manifest: BehaviorManifestDto,
  onCommand?: (commandId: string) => boolean,
): Extension {
  if (!onCommand) return [];
  const bindings = manifest.keymaps ?? [];
  const singles = bindings
    .filter(
      (binding) =>
        binding.sequence.length === 1 &&
        binding.commandId !== "text.insert_newline" &&
        binding.commandId !== "text.insert_tab",
    )
    .map((binding) => ({
      key: cmKey(binding.sequence[0]),
      preventDefault: true,
      run: () => onCommand(binding.commandId),
    }))
    .filter((binding) => binding.key.length > 0);
  const chords = bindings.filter((binding) => binding.sequence.length > 1);
  return [keymap.of(singles), chordKeymap(chords, onCommand)];
}

function cmKey(stroke: KeyStrokeDto | undefined): string {
  if (!stroke) return "";
  const parts: string[] = [];
  if (stroke.modifiers.control) parts.push("Ctrl");
  if (stroke.modifiers.alt) parts.push("Alt");
  if (stroke.modifiers.shift) parts.push("Shift");
  if (stroke.modifiers.superKey) parts.push("Meta");
  const key =
    typeof stroke.key === "string"
      ? ({
          arrowUp: "ArrowUp",
          arrowDown: "ArrowDown",
          arrowLeft: "ArrowLeft",
          arrowRight: "ArrowRight",
        }[stroke.key] ?? stroke.key)
      : stroke.key.character;
  parts.push(key.length === 1 ? key.toLowerCase() : key);
  return parts.join("-");
}

function chordKeymap(
  bindings: readonly KeyBindingDto[],
  onCommand: (commandId: string) => boolean,
): Extension {
  if (!bindings.length) return [];
  return ViewPlugin.define(
    () => {
      let pending: KeyBindingDto[] = [];
      let index = 0;
      let timer = 0;
      const reset = () => {
        pending = [];
        index = 0;
        window.clearTimeout(timer);
      };
      return {
        update() {},
        destroy: reset,
        handleKey(event: KeyboardEvent): boolean {
          const candidates = (pending.length ? pending : bindings).filter(
            (binding) => eventMatches(event, binding.sequence[index]),
          );
          if (!candidates.length) {
            reset();
            return false;
          }
          event.preventDefault();
          const complete = candidates.find(
            (binding) => binding.sequence.length === index + 1,
          );
          if (complete) {
            reset();
            return onCommand(complete.commandId);
          }
          pending = candidates;
          index += 1;
          window.clearTimeout(timer);
          timer = window.setTimeout(reset, 1_000);
          return true;
        },
      };
    },
    {
      eventHandlers: {
        keydown(event) {
          return (
            this as unknown as { handleKey(event: KeyboardEvent): boolean }
          ).handleKey(event);
        },
      },
    },
  );
}

function eventMatches(
  event: KeyboardEvent,
  stroke: KeyStrokeDto | undefined,
): boolean {
  if (!stroke) return false;
  const expected =
    typeof stroke.key === "string"
      ? stroke.key.toLowerCase()
      : stroke.key.character.toLowerCase();
  return (
    event.key.toLowerCase() === expected &&
    event.ctrlKey === stroke.modifiers.control &&
    event.altKey === stroke.modifiers.alt &&
    event.shiftKey === stroke.modifiers.shift &&
    event.metaKey === stroke.modifiers.superKey
  );
}

function fontAndWrap(manifest: BehaviorManifestDto): Extension {
  const role =
    manifest.documentFontRole === "proportional" ? "proportional" : "monospace";
  const wrap = manifest.editorRules?.layout?.wrap;
  const extensions: Extension[] = [
    EditorView.theme({
      "&": { fontFamily: `var(--clay-font-${role})` },
      ".cm-content": { fontFamily: "inherit" },
    }),
  ];
  if (wrap !== "none") extensions.push(EditorView.lineWrapping);
  if (wrap && typeof wrap === "object" && "column" in wrap) {
    const column = Math.max(
      16,
      Math.min(240, Number((wrap as { column: number }).column)),
    );
    extensions.push(
      EditorView.theme({
        ".cm-content": {
          maxWidth: `${column}ch`,
          marginInline: "auto",
          width: "100%",
        },
      }),
    );
  }
  return extensions;
}
