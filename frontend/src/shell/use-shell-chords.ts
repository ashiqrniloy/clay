import { useEffect } from "react";

import type { WorkspaceController } from "./workspace-controller";

/** Default client-local shell chords from test-plan/13 and /14. */
export function useShellChords(
  workspace: WorkspaceController,
  enabled: boolean,
) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (!event.ctrlKey) return;
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
