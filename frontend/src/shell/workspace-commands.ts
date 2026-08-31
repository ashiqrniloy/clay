// Client-side shell command dispatch for the per-window workspace
// controller: command id -> shell/tab operator with pane-local client
// command fallback. Extracted from the createWorkspace closure
// (2026-08-31 review P2-2) with an explicit context seam.

import type { BootstrapDto } from "../bridge/types";
import type { TabStore } from "./tab-store";
import {
  addEqualPane,
  closePane,
  focusPane,
  movePane,
  nextPane,
  prevPane,
  resizeActive,
  splitPane,
  type SplitTree,
} from "./split-tree";
import type { TabRuntime, WorkspaceAdapters } from "./workspace-controller";

export interface CommandContext {
  adapters: WorkspaceAdapters;
  tabs: TabStore;
  notify: () => void;
  setTree: (runtime: TabRuntime, tree: SplitTree | null) => void;
  mountRuntime: (bootstrap: BootstrapDto, tree?: SplitTree) => TabRuntime;
}

const sendTabCommand = (
  ctx: CommandContext,
  runtime: TabRuntime,
  command: unknown,
) =>
  ctx.adapters.send(
    JSON.stringify({
      family: "tabCommand",
      payload: { clientId: runtime.clientId, command },
    }),
    runtime.tabId ?? undefined,
  );

export function dispatchClientCommand(
  ctx: CommandContext,
  runtime: TabRuntime,
  commandId: string,
) {
  const activePane = runtime.panes.get(runtime.tree.activePaneId);
  if (activePane?.session.runClientCommand(commandId)) return;
  const direct: Record<string, () => void> = {
    "shell.clientSplitPaneVertical": () =>
      ctx.setTree(runtime, splitPane(runtime.tree, "horizontal")),
    "shell.clientSplitPaneHorizontal": () =>
      ctx.setTree(runtime, splitPane(runtime.tree, "vertical")),
    "shell.clientSplitPaneRight": () =>
      ctx.setTree(runtime, splitPane(runtime.tree, "horizontal")),
    "shell.clientSplitPaneDown": () =>
      ctx.setTree(runtime, splitPane(runtime.tree, "vertical")),
    "shell.clientAddEqualPane": () =>
      ctx.setTree(runtime, addEqualPane(runtime.tree)),
    "shell.clientClosePane": () => {
      activePane?.session.close(false);
      ctx.setTree(runtime, closePane(runtime.tree));
    },
    "shell.clientFocusPaneNext": () =>
      ctx.setTree(runtime, focusPane(runtime.tree, nextPane(runtime.tree))),
    "shell.clientFocusPanePrev": () =>
      ctx.setTree(runtime, focusPane(runtime.tree, prevPane(runtime.tree))),
    "shell.clientResizePaneLeft": () =>
      ctx.setTree(runtime, resizeActive(runtime.tree, "left")),
    "shell.clientResizePaneRight": () =>
      ctx.setTree(runtime, resizeActive(runtime.tree, "right")),
    "shell.clientResizePaneUp": () =>
      ctx.setTree(runtime, resizeActive(runtime.tree, "up")),
    "shell.clientResizePaneDown": () =>
      ctx.setTree(runtime, resizeActive(runtime.tree, "down")),
    "shell.clientMovePaneNext": () =>
      ctx.setTree(runtime, movePane(runtime.tree, "second")),
    "shell.clientMovePanePrev": () =>
      ctx.setTree(runtime, movePane(runtime.tree, "first")),
    "documents.clientOpenFileDialog": () => {
      ctx.adapters
        .openFileDialog?.(runtime.tabId ?? undefined)
        ?.catch((error: unknown) => {
          // Dialog failures must be visible: a busy portal lock or a failed
          // native dialog otherwise looks like a dead button.
          runtime.diagnostic = {
            severity: "error",
            code: "dialog.failed",
            message:
              error instanceof Error
                ? error.message
                : "File dialog could not open",
          };
          ctx.notify();
        });
    },
    "workspace.clientOpenFolderDialog": () => {
      ctx.adapters
        .openFolderDialog?.(runtime.tabId ?? undefined)
        ?.catch((error: unknown) => {
          runtime.diagnostic = {
            severity: "error",
            code: "dialog.failed",
            message:
              error instanceof Error
                ? error.message
                : "Folder dialog could not open",
          };
          ctx.notify();
        });
    },
    "settings.open": () => {
      runtime.settingsOpen = true;
      ctx.notify();
    },
    "settings.close": () => {
      runtime.settingsOpen = false;
      ctx.notify();
    },
  };
  if (direct[commandId]) {
    direct[commandId]();
    return;
  }
  const snapshot = ctx.tabs.get();
  const index = snapshot.tabs.findIndex(
    (tab) => tab.clientId === runtime.clientId,
  );
  const activateOffset = (offset: number) => {
    const target = snapshot.tabs.at(
      (index + offset + snapshot.tabs.length) % snapshot.tabs.length,
    );
    if (target?.tabId != null) void ctx.adapters.activateTab?.(target.tabId);
  };
  if (commandId === "shell.clientTabNext") activateOffset(1);
  else if (commandId === "shell.clientTabPrev") activateOffset(-1);
  else if (commandId === "shell.clientTabClose" && runtime.tabId != null)
    void ctx.adapters.closeTab?.(runtime.tabId);
  else if (commandId === "shell.clientTabMoveLeft" && runtime.tabId != null)
    void sendTabCommand(ctx, runtime, { moveLeft: { tabId: runtime.tabId } });
  else if (commandId === "shell.clientTabMoveRight" && runtime.tabId != null)
    void sendTabCommand(ctx, runtime, { moveRight: { tabId: runtime.tabId } });
  else if (commandId === "shell.clientTabNew")
    void (async () => {
      const bootstrap = await ctx.adapters.openTabDialog?.();
      if (bootstrap) ctx.mountRuntime(bootstrap);
      ctx.notify();
    })();
  else {
    const position = Number(commandId.split(".").at(-1));
    if (Number.isInteger(position) && position >= 1 && position <= 9) {
      if (commandId.startsWith("shell.clientTabActivate.")) {
        const target = snapshot.tabs[position - 1];
        if (target?.tabId != null)
          void ctx.adapters.activateTab?.(target.tabId);
      } else if (
        commandId.startsWith("shell.clientTabMoveTo.") &&
        runtime.tabId != null
      ) {
        void sendTabCommand(ctx, runtime, {
          moveTo: { tabId: runtime.tabId, position },
        });
      }
    }
  }
}
