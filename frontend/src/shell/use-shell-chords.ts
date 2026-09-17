import { useEffect } from "react";

import { workspaceRail } from "./layout-state";
import type { WorkspaceController } from "./workspace-controller";
import type { ServerKeyStroke } from "./workspace-controller";

interface ServerKeyBinding {
  commandId: string;
  sequence: ServerKeyStroke[];
}

function strokeKey(stroke: ServerKeyStroke | undefined): string {
  if (!stroke) return "";
  const raw =
    typeof stroke.key === "string" ? stroke.key : stroke.key.character;
  return raw.toLowerCase();
}

/** Modifier keydowns and held-key auto-repeats never advance or cancel a
 *  pending chord: holding Ctrl across `Ctrl+X Ctrl+P` delivers Control
 *  keydowns, and a held first stroke repeats — both must leave the pending
 *  chord alone or the chord can never resolve. */
const MODIFIER_KEYS = new Set(["control", "shift", "alt", "meta"]);

function isChordNoise(event: KeyboardEvent): boolean {
  return event.repeat || MODIFIER_KEYS.has(event.key.toLowerCase());
}

function eventMatchesStroke(
  event: KeyboardEvent,
  stroke: ServerKeyStroke | undefined,
): boolean {
  if (!stroke) return false;
  return (
    event.key.toLowerCase() === strokeKey(stroke) &&
    event.ctrlKey === stroke.modifiers.control &&
    event.altKey === stroke.modifiers.alt &&
    event.shiftKey === stroke.modifiers.shift &&
    event.metaKey === stroke.modifiers.superKey
  );
}

/** Default client-local shell chords from test-plan/13 and /14, plus the
 *  shell-side matcher for Global server-first manifest chords (e.g.
 *  `Ctrl+X Ctrl+P` → `shell.toggleAgentLane`, `Ctrl+X Ctrl+O` →
 *  `controlCenter.open`): the editor keymap owns those only inside
 *  `.cm-editor`, so outside editor focus the shell resolves them —
 *  otherwise the lane and the palette are unreachable with no document
 *  open. The lane's chord dispatches a server intent like any other
 *  server-first command; the server answers it with the client command
 *  (`workspace-commands.ts` flips the per-tab layout state). */
export function useShellChords(
  workspace: WorkspaceController,
  enabled: boolean,
) {
  useEffect(() => {
    if (!enabled) return;
    let pending: ServerKeyBinding[] = [];
    let pendingIndex = 0;
    let chordTimer = 0;
    const resetChord = () => {
      pending = [];
      pendingIndex = 0;
      window.clearTimeout(chordTimer);
    };
    const onKey = (event: KeyboardEvent) => {
      if (isChordNoise(event)) return;
      if (!event.ctrlKey) {
        resetChord();
        return;
      }
      // Global server-first chords: only outside editor views (the editor
      // keymap handles them when an editor has focus).
      const target = event.target as HTMLElement | null;
      if (!target?.closest?.(".cm-editor")) {
        const bindings = workspace.serverKeymaps() as ServerKeyBinding[];
        if (bindings.length) {
          const candidates = (pending.length ? pending : bindings).filter(
            (binding) =>
              eventMatchesStroke(event, binding.sequence[pendingIndex]),
          );
          if (candidates.length) {
            event.preventDefault();
            const complete = candidates.find(
              (binding) => binding.sequence.length === pendingIndex + 1,
            );
            if (complete) {
              resetChord();
              workspace.dispatchServerCommand(complete.commandId);
              return;
            }
            pending = candidates;
            pendingIndex += 1;
            window.clearTimeout(chordTimer);
            chordTimer = window.setTimeout(resetChord, 1_000);
            return;
          }
          resetChord();
        }
      }
      const key = event.key;
      if (event.altKey && event.shiftKey) {
        const map: Record<string, "left" | "right" | "up" | "down"> = {
          ArrowLeft: "left",
          ArrowRight: "right",
          ArrowUp: "up",
          ArrowDown: "down",
        };
        const direction = map[key];
        if (!direction) return;
        event.preventDefault();
        workspace.resize(direction);
        return;
      }
      if (event.altKey) {
        if (key === "ArrowRight") {
          event.preventDefault();
          workspace.focus("next");
          return;
        }
        if (key === "ArrowLeft") {
          event.preventDefault();
          workspace.focus("prev");
          return;
        }
        if (key === "w" || key === "W") {
          event.preventDefault();
          workspace.closeActivePane();
          return;
        }
        if (key === "]") {
          event.preventDefault();
          workspace.move("second");
          return;
        }
        if (key === "[") {
          event.preventDefault();
          workspace.move("first");
          return;
        }
      }
      if (event.shiftKey && (key === "\\" || key === "|")) {
        event.preventDefault();
        workspace.addEqual();
        return;
      }
      if (key === "\\") {
        event.preventDefault();
        workspace.split("horizontal");
        return;
      }
      if (key === "-" || key === "_") {
        event.preventDefault();
        workspace.split("vertical");
        return;
      }
      // The tab's two views (plan 118 task 33): the switcher is tab chrome, so
      // the chord fires wherever focus sits. A pick that would be a no-op (an
      // uncommitted tab) leaves the view alone.
      if (!event.altKey && (key === "1" || key === "2")) {
        const snapshot = workspace.getSnapshot();
        const tab = snapshot.tabs.find(
          (entry) => entry.clientId === snapshot.activeClientId,
        );
        if (!tab || (!tab.workspaceRoot && !tab.agent)) return;
        event.preventDefault();
        workspace.setView(key === "1" ? "workspace" : "agent");
        return;
      }
      if (key === "t" || key === "T") {
        event.preventDefault();
        void workspace.newTab();
        return;
      }
      // The filter chord is the approved sidebar behaviour: `/` puts the caret
      // in the visible list filter (the workspace file listing). Generic: it
      // focuses whichever filter the host is showing, never a named pane.
      if (key === "/" && !event.shiftKey) {
        const field = document.querySelector<HTMLInputElement>(
          "[data-clay-list-filter] input",
        );
        if (!field) return;
        event.preventDefault();
        field.focus();
        field.select();
        return;
      }
      // The workspace rail is view chrome, not editor text: the chord fires
      // wherever focus sits, and the titlebar and rail buttons run the same
      // toggle (DESIGN.md §12).
      if (key === "i" || key === "I") {
        event.preventDefault();
        workspaceRail.toggle();
        return;
      }
      if (key === "Tab") {
        event.preventDefault();
        const snapshot = workspace.getSnapshot();
        const tabs = snapshot.tabs;
        if (tabs.length < 2) return;
        const idx = tabs.findIndex(
          (tab) => tab.clientId === snapshot.activeClientId,
        );
        const next = event.shiftKey
          ? tabs[(idx - 1 + tabs.length) % tabs.length]
          : tabs[(idx + 1) % tabs.length];
        if (next) void workspace.activate(next.clientId);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [workspace, enabled]);
}
