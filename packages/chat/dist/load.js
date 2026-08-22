// @clay/chat package load entry (Phase 25).
//
// Registers the Chat profile contract (no tools) and the empty-tab landing.
// Greeting copy lives here so a replacement package can change it. Controls
// emit inert command intents; Clay owns dialogs, Command Centre pickers, and
// the Prism daemon. No Masonry, no client JS, no raw CSS.
import { serverRegisterPaneContentContribution } from "clay:ui";

export const packageName = "@clay/chat";
export const apiPrefix = "chat";

export const CHAT_PROFILE = Object.freeze({
  name: "Chat",
  description: "General assistant. No tools.",
  tools: Object.freeze([])
});

const CHAT_COMMANDS = Object.freeze([
  { id: "chat.profile", displayName: "Chat", routingPolicy: "server-first" },
  { id: "chat.submit", displayName: "Send Message", routingPolicy: "server-first" },
  { id: "chat.cancel", displayName: "Cancel", routingPolicy: "server-first" }
]);

const CHAT_ENTRY = Object.freeze({
  id: "chat.entry",
  activation: "empty-tab",
  actionTargets: [
    "agent.clientOpenAgentPicker",
    "agent.clientOpenProviderPicker",
    "agent.clientOpenModelPicker",
    "chat.submit",
    "chat.cancel",
    "documents.clientOpenFileDialog",
    "workspace.clientOpenFolderDialog"
  ],
  component: {
    kind: "panel",
    id: "chat.root",
    title: "Chat",
    children: [
      {
        kind: "label",
        id: "chat.greeting",
        text: "What do you want to do today?",
        style: { typography: "typography.display" }
      },
      {
        kind: "label",
        id: "chat.providerHint",
        text: "Configure a provider to start chatting.",
        style: { typography: "typography.caption" }
      },
      {
        kind: "scroll",
        id: "chat.transcriptScroll",
        children: [
          {
            kind: "list",
            id: "chat.transcript",
            items: []
          }
        ]
      },
      {
        kind: "label",
        id: "chat.status",
        text: "Ready",
        style: { typography: "typography.status" }
      },
      {
        kind: "button",
        id: "chat.button.agent",
        label: "Agent",
        action: { commandId: "agent.clientOpenAgentPicker" }
      },
      {
        kind: "button",
        id: "chat.button.provider",
        label: "Provider",
        action: { commandId: "agent.clientOpenProviderPicker" }
      },
      {
        kind: "button",
        id: "chat.button.model",
        label: "Model",
        action: { commandId: "agent.clientOpenModelPicker" }
      },
      {
        kind: "button",
        id: "chat.button.openFile",
        label: "Open File",
        action: { commandId: "documents.clientOpenFileDialog" }
      },
      {
        kind: "button",
        id: "chat.button.openFolder",
        label: "Open Folder",
        action: { commandId: "workspace.clientOpenFolderDialog" }
      },
      {
        kind: "button",
        id: "chat.button.cancel",
        label: "Cancel",
        action: { commandId: "chat.cancel" }
      },
      {
        kind: "textInput",
        id: "chat.composer",
        title: "Message",
        label: "Message",
        action: { commandId: "chat.submit" },
        style: { validationState: "none", placeholderColor: "text.muted" }
      }
    ]
  }
});

export function chatPackageContract() {
  return {
    packageName,
    apiPrefix,
    profile: CHAT_PROFILE,
    commands: CHAT_COMMANDS,
    entry: CHAT_ENTRY
  };
}

export async function loadChatPackage(_options = {}) {
  await serverRegisterPaneContentContribution(CHAT_ENTRY);
  return chatPackageContract();
}

// Default activation entry for `loadPackage("@clay/chat")`.
export default loadChatPackage;
