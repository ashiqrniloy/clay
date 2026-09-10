// @clay/coding-agent package load entry (Phase 2).
//
// Registers the coding profile (nine Prism coding tools + ask_user_decision,
// coding system prompt layer) on the daemon's validated registries via the
// trusted-only `clay:agent` facade. Disk skills (`.agents/skills/` at the
// workspace root and under the clay config root) are discovered by the
// daemon itself — nothing skill-shaped is hardcoded here anymore.
//
// Also registers the named pane surface (activation "pane"): the Coding
// Agent split surface launched through the `coding-agent.profile` chrome
// command. The declared tree is inert data — dynamic content rides the
// core-owned AG-UI stream in the host-rendered surface module; the empty-tab
// landing stays @clay/chat. Tool execution keeps the Phase 1 acceptance
// policy; registration grants no execution authority. No raw ops, no client
// JavaScript.
import { profileRegister, commandRegister } from "clay:agent";
import { serverRegisterPaneContentContribution } from "clay:ui";

export const packageName = "@clay/coding-agent";
export const apiPrefix = "coding-agent";

export const CODING_TOOLS = Object.freeze([
  "shell",
  "read",
  "write",
  "edit",
  "repo_list",
  "repo_search",
  "glob",
  "delete",
  "move",
  "ask_user_decision"
]);

export const CODING_SYSTEM_PROMPT = `You are a workspace coding agent.
Your tools execute real host operations under Clay's acceptance policy:
in-workspace reads and writes run directly; out-of-workspace writes,
delete/move operations, and shell metacharacter commands require user
approval; full autonomy stays off unless the user enables it.
Inspect before editing (repo_list, repo_search, glob, read); keep edits
inside the open workspace; surface destructive operations before running
them. For multi-step work, maintain numbered plan documents under plans/.`;

export const CODING_PROFILE = Object.freeze({
  name: "coding",
  description: "Workspace coding agent",
  instructions: CODING_SYSTEM_PROMPT,
  tools: CODING_TOOLS
});

// Slash commands (pi-parity surface). Names are the daemon dispatch keys;
// manifest contributions mirror them as coding-agent.* ids for discovery.
// /model is a core built-in (agent.clientOpenModelPicker) and is not
// re-declared here.
export const SLASH_COMMANDS = Object.freeze([
  { name: "/compact", handler: "compact", description: "Compact the current session manually." },
  { name: "/new", handler: "newSession", description: "Start a fresh session with the same profile, provider, and model." },
  { name: "/n", handler: "newSession", description: "Alias of /new." },
  { name: "/branch", handler: "checkout", description: "Checkout a branch point: args { entryId }." },
  { name: "/tree", handler: "tree", description: "Show the session branch tree with summaries." },
  { name: "/discard", handler: "discard", description: "Discard a branch: args { entryId, confirm } (two-step)." },
  { name: "/fork", handler: "forkSession", description: "Fork the session at its leaf (or args { entryId })." },
  { name: "/clone", handler: "cloneSession", description: "Clone the session to a new id (or args { entryId })." },
  { name: "/open-session", handler: "openSession", description: "Open a session by id: args { sessionId } (--session equivalent)." },
  { name: "/open-session-as-fork", handler: "openSessionAsFork", description: "Open a session as a fork: args { sessionId } (--fork equivalent)." }
]);

export function codingAgentPackageContract() {
  return {
    packageName,
    apiPrefix,
    profile: CODING_PROFILE,
    slashCommands: SLASH_COMMANDS
  };
}

export const AGENT_SURFACE = Object.freeze({
  id: "coding-agent.surface",
  activation: "pane",
  actionTargets: Object.freeze([
    "coding-agent.profile",
    "coding-agent.close",
    "agent.clientOpenModelPicker",
    "documents.clientOpenFileDialog"
  ]),
  component: Object.freeze({
    kind: "panel",
    id: "coding-agent.root",
    title: "Coding Agent",
    children: Object.freeze([
      Object.freeze({
        kind: "label",
        id: "coding-agent.transcriptTitle",
        text: "Coding Agent",
        style: Object.freeze({ typography: "typography.title" })
      }),
      Object.freeze({
        kind: "label",
        id: "coding-agent.emptyHint",
        text: "No conversation yet.",
        style: Object.freeze({ typography: "typography.caption" })
      }),
      Object.freeze({
        kind: "textInput",
        id: "coding-agent.composer",
        title: "Message",
        multiline: true
      })
    ])
  })
});

export async function loadCodingAgentPackage(_options = {}) {
  await profileRegister(CODING_PROFILE);
  for (const command of SLASH_COMMANDS) {
    await commandRegister(command);
  }
  await serverRegisterPaneContentContribution(AGENT_SURFACE);
  return codingAgentPackageContract();
}

// Default activation entry for `loadPackage("@clay/coding-agent")`.
export default loadCodingAgentPackage;
