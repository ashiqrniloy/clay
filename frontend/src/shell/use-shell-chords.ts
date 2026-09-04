import { useEffect } from "react";

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
 *  `Ctrl+X Ctrl+P` → `controlCenter.open`): the editor keymap owns those
 *  only inside `.cm-editor`, so outside editor focus the shell resolves
 *  them — otherwise the Command Centre is unreachable with no document
 *  open. */
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
