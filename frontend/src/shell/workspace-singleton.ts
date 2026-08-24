import { sendRequest } from "../bridge/client";
import { createWorkspace } from "./workspace-controller";

export const workspace = createWorkspace({
  send: sendRequest,
  openTab: async (workspaceRoot) => {
    const { openTab } = await import("../bridge/client");
    return openTab(workspaceRoot);
  },
  closeTab: async (tabId) => {
    const { closeTab } = await import("../bridge/client");
    return closeTab(tabId);
  },
  activateTab: async (tabId) => {
    const { activateTab } = await import("../bridge/client");
    return activateTab(tabId);
  },
  loadLayout: async () => {
    const { loadLayout } = await import("../bridge/client");
    return loadLayout();
  },
  saveLayout: async (state) => {
    const { saveLayout } = await import("../bridge/client");
    return saveLayout(state);
  },
  openFileDialog: async (tabId) => {
    const { openFileDialog } = await import("../bridge/client");
    return openFileDialog(tabId);
  },
  openFolderDialog: async (tabId) => {
    const { openFolderDialog } = await import("../bridge/client");
    return openFolderDialog(tabId);
  },
  openTabDialog: async () => {
    const { openTabDialog } = await import("../bridge/client");
    return openTabDialog();
  },
});
