// Coding Agent split surface (plan 108 task 8) — binding-spec presentation
// for the bundled `@clay/coding-agent` package.
//
// Provenance-exact host rendering, mirroring the ChatPanel/SettingsPanel
// precedent: static copy comes from the package's declared component tree;
// every dynamic value rides the ONE core-owned AG-UI stream (`chatAgent` —
// same daemon session as chat); every interaction emits declared inert
// command intents. Third-party replacements render through the unchanged
// generic SDUI renderer (PaneTree), never this module.
//
// Layout: 50/50 vertical split (react-resizable-panels, ratio-clamped,
// keyboard-operable separator). Left: transcript + composer + status row +
// extension strip. Right: Files / Memory / Context tabs, plus the full
// content of a selected truncated transcript box.
//
// ponytail: reasoning-effort levels have no daemon field yet (task 9 wires
// context tokens; the effort chord still reads `state.effortLevels` when a
// model exposes them and stays a no-op for every current model).

import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactElement,
  type ReactNode,
} from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { Tab, TabList, TabPanel, Tabs } from "react-aria-components";

import { ClayButton, ClayText, ClayTextField } from "../components";
import { chatAgent } from "../agent/state";
import { sendRequest } from "../bridge/client";
import { sduiActionPayload, type IntentSender } from "../sdui/actions";
import type { SduiState } from "../sdui/state";
import type {
  PackageComponentNode,
  PackageSurface,
} from "../sdui/types";

import styles from "./coding-agent.module.css";

/** Slash surface, mirroring the bundled package's daemon registrations. */
const SLASH_COMMANDS: ReadonlyArray<{ name: string; description: string }> = [
  { name: "/compact", description: "Compact the current session." },
  { name: "/new", description: "Start a fresh session." },
  { name: "/n", description: "Alias of /new." },
  { name: "/branch", description: "Checkout a branch point." },
  { name: "/tree", description: "Show the session branch tree." },
  { name: "/discard", description: "Discard a branch (two-step confirm)." },
  { name: "/fork", description: "Fork at leaf (or entry)." },
  { name: "/clone", description: "Clone into a new session." },
  { name: "/open-session", description: "Load a session by id." },
  {
    name: "/open-session-as-fork",
    description: "Load and immediately fork.",
  },
];

type TranscriptKind =
  | "user"
  | "assistant"
  | "reasoning"
  | "usage"
  | "error"
  | "tool"
  | "skill";

interface TranscriptBox {
  id: string;
  kind: TranscriptKind;
  label: string;
  content: string;
}

function findNode(
  node: PackageComponentNode,
  id: string,
): PackageComponentNode | null {
  if (node.id === id) return node;
  for (const child of node.children ?? []) {
    const found = findNode(child, id);
    if (found) return found;
  }
  return null;
}

function kindOf(message: {
  role: string;
  content?: unknown;
  metadata?: { clayKind?: string };
}): TranscriptKind | null {
  if (message.metadata?.clayKind === "usage") return "usage";
  if (message.metadata?.clayKind === "error") return "error";
  if (message.role === "user") return "user";
  if (message.role === "reasoning") return "reasoning";
  if (message.role === "assistant") return "assistant";
  return null;
}

/**
 * Uniform-height truncated transcript boxes (fixed 3-line clamp — the
 * metadata-driven fixed-line-count truncation from the plan), colored by
 * content type from typed theme tokens. Selection shows the full content in
 * the right pane.
 */
const TranscriptBoxRow = memo(function TranscriptBoxRow({
  box,
  selected,
  onSelect,
}: {
  box: TranscriptBox;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <button
      type="button"
      className={`${styles.box} ${styles[`box_${box.kind}`]}`}
      aria-pressed={selected}
      aria-label={`${box.label} ${box.content}`}
      onClick={() => onSelect(selected ? "" : box.id)}
    >
      <span className={styles.boxLabel}>{box.label}</span>
      <span className={styles.boxContent}>{box.content}</span>
    </button>
  );
});

export interface CodingAgentPanelProps {
  surface: PackageSurface;
  /** Current runtime package UI version; validates every intent. */
  uiVersion: number;
  /** Workspace path for the status row. */
  workspaceRoot: string;
  /** File-browser SDUI tree for the Files tab (existing file view). */
  sdui?: SduiState | null;
  /** Intent sender for the embedded SDUI file browser. */
  send?: (payload: string) => Promise<void>;
}

export function CodingAgentPanel({
  surface,
  uiVersion,
  workspaceRoot,
  sdui = null,
  send,
}: CodingAgentPanelProps) {
  const snapshot = useSyncExternalStore(
    chatAgent.subscribe,
    chatAgent.getSnapshot,
    chatAgent.getSnapshot,
  );

  useEffect(() => {
    chatAgent.agent.setUiVersion(uiVersion);
  }, [uiVersion]);
  useEffect(() => {
    if (send) chatAgent.agent.setSender(send);
  }, [send]);
  useEffect(() => chatAgent.start(), []);
  useEffect(() => {
    void sendRequest(agentCommandPayload({ listSessions: {} }));
  }, []);

  const declared = surface.component;
  const transcriptTitle = useMemo(
    () => findNode(declared, "coding-agent.transcriptTitle")?.text ?? "Agent",
    [declared],
  );

  const [draft, setDraft] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [effortIndex, setEffortIndex] = useState(0);
  const [completionIndex, setCompletionIndex] = useState(0);
  const [completionDismissed, setCompletionDismissed] = useState(false);

  const configured =
    typeof snapshot.state["provider"] === "string" &&
    (snapshot.state["provider"] as string).length > 0;
  const streaming = snapshot.status.streaming;
  const provider = String(snapshot.state["provider"] ?? "");
  const model = String(snapshot.state["model"] ?? "");
  const mcpServers = Array.isArray(snapshot.state["mcpServers"])
    ? (snapshot.state["mcpServers"] as string[])
    : [];
  const effortLevels = Array.isArray(snapshot.state["effortLevels"])
    ? (snapshot.state["effortLevels"] as string[])
    : null;
  // Context-used-vs-window (plan 108 task 9): numerator rides every snapshot
  // STATE event, denominator the model registry's contextWindow. Both are
  // bounded counters/ints — counters only, never transcript content.
  const contextTokens =
    typeof snapshot.state["contextTokens"] === "number"
      ? (snapshot.state["contextTokens"] as number)
      : null;
  const contextWindow =
    typeof snapshot.state["contextWindow"] === "number"
      ? (snapshot.state["contextWindow"] as number)
      : null;

  const transcript = useMemo<TranscriptBox[]>(() => {
    const boxes: TranscriptBox[] = [];
    for (const message of snapshot.messages) {
      const kind = kindOf(message);
      if (!kind) continue;
      const content =
        typeof message.content === "string" ? message.content : "";
      boxes.push({
        id: message.id,
        kind,
        label:
          kind === "usage"
            ? "usage"
            : kind === "error"
              ? "error"
              : kind === "reasoning"
                ? "thinking"
                : kind,
        content,
      });
    }
    for (const tool of snapshot.tools) {
      boxes.push({
        id: tool.id,
        kind: tool.name === "load_skill" ? "skill" : "tool",
        label: tool.name === "load_skill" ? "skill" : tool.name,
        content:
          tool.name === "load_skill"
            ? `loaded skill (${tool.phase})`
            : `tool ${tool.phase}`,
      });
    }
    return boxes;
  }, [snapshot.messages, snapshot.tools]);

  const lastUsage = useMemo(() => {
    for (let index = snapshot.messages.length - 1; index >= 0; index -= 1) {
      const message = snapshot.messages[index];
      if (
        message &&
        (message as { metadata?: { clayKind?: string } }).metadata?.clayKind ===
          "usage"
      ) {
        return typeof message.content === "string" ? message.content : "";
      }
    }
    return "";
  }, [snapshot.messages]);

  const selected = transcript.find((box) => box.id === selectedId) ?? null;

  const slashMatches = useMemo(() => {
    if (completionDismissed) return null;
    if (!draft.startsWith("/") || draft.includes(" ")) return null;
    return SLASH_COMMANDS.filter((command) =>
      command.name.startsWith(draft),
    ).slice(0, 8);
  }, [completionDismissed, draft]);

  const sendIntent = useCallback(
    (commandId: string) => {
      void sendRequest(
        sduiActionPayload(uiVersion, {
          commandId,
          source: { button: { nodeId: 1 } },
          arguments: [],
        }),
      ).catch(() => {
        // Disconnect flow owns recovery; nothing to do here.
      });
    },
    [uiVersion],
  );

  const submit = useCallback(
    (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || !configured) return;
      setDraft("");
      setSelectedId("");
      if (streaming) {
        // Mid-run queue (pi-parity steer): the daemon folds the message into
        // the active run; the transcript keeps streaming.
        chatAgent.agent.steer(trimmed);
        return;
      }
      chatAgent.agent.sendPrompt(trimmed);
      void chatAgent.agent.runAgent().catch(() => {
        // Failures already landed as RUN_ERROR status.
      });
    },
    [configured, streaming],
  );

  const onComposerKeyDown = useCallback(
    (event: { key: string; shiftKey: boolean; preventDefault: () => void }) => {
      if (event.shiftKey && event.key === "Tab") {
        // Shift+Tab cycles reasoning effort — only when the model exposes
        // levels; a no-op (no focus change) otherwise.
        if (effortLevels && effortLevels.length > 0) {
          event.preventDefault();
          setEffortIndex((index) => (index + 1) % effortLevels.length);
        }
        return;
      }
      if (!slashMatches || slashMatches.length === 0) return;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setCompletionIndex((index) => (index + 1) % slashMatches.length);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setCompletionIndex(
          (index) => (index - 1 + slashMatches.length) % slashMatches.length,
        );
      } else if (event.key === "Escape") {
        event.preventDefault();
        setCompletionDismissed(true);
      }
    },
    [effortLevels, slashMatches],
  );

  const onComposerSubmit = useCallback(
    (value: string) => {
      if (slashMatches && slashMatches.length > 0) {
        const exact = slashMatches.find((command) => command.name === value);
        const highlighted: string | undefined = slashMatches[completionIndex]?.name;
        if (exact || highlighted) {
          submit(exact?.name ?? highlighted ?? "");
          return;
        }
      }
      submit(value);
    },
    [completionIndex, slashMatches, submit],
  );

  const complete = useCallback(
    (commandName: string) => {
      setDraft(commandName);
      setCompletionIndex(0);
    },
    [],
  );

  const toolStats = snapshot.toolStats;
  // Tool approval (durable runs): Allow/Deny strip for suspended runs.
  const pendingApproval = snapshot.pendingApproval;
  const resolveApproval = useCallback(
    (outcome: "allow_once" | "reject_once") => {
      if (!pendingApproval) return;
      void sendRequest(
        agentCommandPayload({
          runResume: {
            sessionId: pendingApproval.sessionId,
            runId: pendingApproval.runId,
            decisionJson: JSON.stringify({
              decisions: [{ approvalId: pendingApproval.requestId, outcome }],
            }),
          },
        }),
      );
      // Optimistic clear; the resumed run re-announces via RUN_STARTED.
      chatAgent.clearPendingApproval();
    },
    [pendingApproval],
  );
  // Session list/resume (plan 108 task 9): the same bounded listSessions
  // inventory Chat uses; Resume rebinds this tab's session server-side.
  const sessions = Array.isArray(snapshot.state["sessions"])
    ? (snapshot.state["sessions"] as Array<Record<string, unknown>>)
    : [];
  const lastUsageText =
    contextTokens !== null && contextWindow !== null
      ? `context ${contextTokens}/${contextWindow} tok`
      : lastUsage;
  const contextCounts: Array<[string, number]> = [
    ["System prompts", configured ? 1 : 0],
    [
      "User prompts",
      transcript.filter((box) => box.kind === "user").length,
    ],
    [
      "Agent messages",
      transcript.filter((box) => box.kind === "assistant").length,
    ],
    ["Tool outputs", toolStats.total],
    ["Skills loaded", toolStats.skills],
    ["Files loaded", toolStats.files],
  ];

  return (
    <section
      className={styles.surface}
      aria-label="Coding Agent"
      data-coding-agent-surface
    >
      <Group
        orientation="horizontal"
        className={styles.split}
        aria-label="Coding Agent split"
      >
        <Panel
          defaultSize="50%"
          minSize="20%"
          maxSize="80%"
          className={styles.leftPane}
        >
          <div className={styles.left}>
            <header className={styles.header}>
              <ClayText variant="title">{transcriptTitle}</ClayText>
              <ClayText variant="caption" muted>
                {configured
                  ? `${String(snapshot.state["profile"] ?? "coding")} · ${provider}/${model}`
                  : "Configure a provider to start."}
              </ClayText>
            </header>

            <div
              className={styles.transcript}
              role="log"
              aria-label="Transcript"
              aria-live="polite"
            >
              {transcript.length > 0 ? (
                transcript.map((box) => (
                  <TranscriptBoxRow
                    key={box.id}
                    box={box}
                    selected={box.id === selectedId}
                    onSelect={setSelectedId}
                  />
                ))
              ) : (
                <p className={styles.empty} role="status">
                  <ClayText variant="body" muted>
                    No conversation yet.
                  </ClayText>
                </p>
              )}
            </div>

            <footer className={styles.composerArea}>
              {pendingApproval && (
                <div className={styles.approvalStrip} role="alertdialog" aria-label="Tool approval">
                  <ClayText variant="caption">
                    Tool “{pendingApproval.toolName}” needs approval
                  </ClayText>
                  <ClayButton onPress={() => resolveApproval("allow_once")}>Allow</ClayButton>
                  <ClayButton onPress={() => resolveApproval("reject_once")}>Deny</ClayButton>
                </div>
              )}
              <p className={styles.statusLine} role="status">
                <ClayText variant="status">
                  {streaming ? "Streaming" : (snapshot.status.status ?? "Ready")}
                </ClayText>
              </p>
              {slashMatches && slashMatches.length > 0 && (
                <ul
                  className={styles.completions}
                  aria-label="Slash commands"
                  role="listbox"
                >
                  {slashMatches.map((command, index) => (
                    <li key={command.name}>
                      <button
                        type="button"
                        role="option"
                        aria-selected={index === completionIndex}
                        className={styles.completionRow}
                        onClick={() => complete(command.name)}
                      >
                        <span className={styles.completionName}>
                          {command.name}
                        </span>
                        <ClayText variant="caption" muted>
                          {command.description}
                        </ClayText>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <form
                className={styles.composer}
                onSubmit={(event) => {
                  event.preventDefault();
                  onComposerSubmit(draft);
                }}
              >
                <ClayTextField
                  label="Message"
                  value={draft}
                  onChange={(value) => {
                    setDraft(value);
                    setCompletionIndex(0);
                    setCompletionDismissed(false);
                  }}
                  multiline
                  autoGrow
                  placeholder={
                    configured
                      ? streaming
                        ? "Steer the agent, or wait"
                        : "Ask, or type /"
                      : "Configure a provider first"
                  }
                  disabled={!configured}
                  onKeyDown={onComposerKeyDown}
                  onSubmit={onComposerSubmit}
                />
                <span className={styles.composerActions}>
                  {streaming ? (
                    <ClayButton onPress={() => chatAgent.agent.abortRun()}>
                      Cancel
                    </ClayButton>
                  ) : (
                    <ClayButton
                      type="submit"
                      disabled={!configured || !draft.trim()}
                    >
                      Send
                    </ClayButton>
                  )}
                  <ClayButton
                    variant="muted"
                    onPress={() => sendIntent("coding-agent.close")}
                  >
                    Close
                  </ClayButton>
                </span>
              </form>
            </footer>

            <div className={styles.statusRow} role="status">
              <span className={styles.statusLeft}>
                <ClayText variant="caption" muted>
                  {workspaceRoot} · git —
                </ClayText>
              </span>
              <span className={styles.statusRight}>
                <ClayText variant="caption" muted>
                  {configured ? `${provider}/${model}` : "no provider"}
                  {effortLevels && effortLevels.length > 0
                    ? ` · effort ${effortLevels[effortIndex]}`
                    : ""}
                  {lastUsageText ? ` · ${lastUsageText}` : ""}
                </ClayText>
              </span>
            </div>

            <div className={styles.extensionStrip} aria-label="Extensions">
              <ClayText variant="caption" muted>
                Extensions: core
              </ClayText>
              <ClayText variant="caption" muted>
                MCP: {mcpServers.length > 0 ? mcpServers.join(", ") : "none"}
              </ClayText>
            </div>
          </div>
        </Panel>

        <Separator className={styles.separator} aria-label="Resize split" />

        <Panel minSize="20%" maxSize="80%" className={styles.rightPane}>
          <div className={styles.right}>
            {selected && (
              <div
                className={styles.detail}
                role="region"
                aria-label="Selected transcript entry"
              >
                <div className={styles.detailHeader}>
                  <ClayText variant="caption" muted>
                    {selected.label}
                  </ClayText>
                  <ClayButton
                    variant="muted"
                    onPress={() => setSelectedId("")}
                  >
                    Back
                  </ClayButton>
                </div>
                <pre className={styles.detailContent}>{selected.content}</pre>
              </div>
            )}
            <Tabs className={styles.tabs} defaultSelectedKey="files">
              <TabList aria-label="Agent detail" className={styles.tabStrip}>
                <Tab id="files" className={styles.tab}>
                  Files
                </Tab>
                <Tab id="memory" className={styles.tab}>
                  Memory
                </Tab>
                <Tab id="context" className={styles.tab}>
                  Context
                </Tab>
              </TabList>
              <TabPanel id="files" className={styles.tabPanel}>
                {sdui && send ? (
                  <FilesTab sdui={sdui} send={send} />
                ) : (
                  <div className={styles.tabEmpty}>
                    <ClayText variant="body" muted>
                      Open a file to work on it alongside the agent.
                    </ClayText>
                    <ClayButton
                      onPress={() => sendIntent("documents.clientOpenFileDialog")}
                    >
                      Open file
                    </ClayButton>
                    <ClayButton
                      variant="muted"
                      onPress={() => sendIntent("agent.clientOpenSessionPicker")}
                    >
                      Resume session
                    </ClayButton>
                    <ClayButton
                      variant="muted"
                      onPress={() => sendIntent("agent.clientOpenSessionSearchPicker")}
                    >
                      Search sessions…
                    </ClayButton>
                    {sessions.length > 0 && (
                      <ul className={styles.sessionList} aria-label="Recent sessions">
                        {sessions.slice(0, 5).map((session) => (
                          <li key={String(session["id"])} className={styles.sessionRow}>
                            <ClayText variant="detail" muted>
                              {String(session["id"]).slice(0, 12)}
                            </ClayText>
                            <ClayButton
                              variant="muted"
                              onPress={() =>
                                void sendRequest(
                                  agentCommandPayload({
                                    resumeSession: {
                                      sessionId: String(session["id"]),
                                    },
                                  }),
                                )
                              }
                            >
                              Resume
                            </ClayButton>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}
              </TabPanel>
              <TabPanel id="memory" className={styles.tabPanel}>
                <div className={styles.tabEmpty}>
                  <ClayText variant="body" muted>
                    Observational Memory
                  </ClayText>
                  <ClayText variant="caption" muted>
                    No observational-memory activity this session.
                  </ClayText>
                </div>
              </TabPanel>
              <TabPanel id="context" className={styles.tabPanel}>
                <dl className={styles.contextList}>
                  {contextCounts.map(([label, count]) => (
                    <div key={label} className={styles.contextRow}>
                      <dt>
                        <ClayText variant="detail">{label}</ClayText>
                      </dt>
                      <dd>
                        <ClayText variant="detail" muted>
                          {count}
                        </ClayText>
                      </dd>
                    </div>
                  ))}
                </dl>
              </TabPanel>
            </Tabs>
          </div>
        </Panel>
      </Group>
    </section>
  );
}

/**
 * Files tab: the existing file view. The server already projects the
 * workspace browser as the SDUI tree, so the tab embeds the same
 * renderer with the same inert intent path — no new authority.
 */
function FilesTab({
  sdui,
  send,
}: {
  sdui: SduiState;
  send: IntentSender;
}) {
  const [Renderer, setRenderer] = useState<
    null | ((props: {
      state: SduiState;
      send: IntentSender;
      editorSlot: ReactNode;
    }) => ReactElement)
  >(null);
  useEffect(() => {
    let cancelled = false;
    void import("../sdui/renderer").then((module) => {
      if (!cancelled) setRenderer(() => module.SduiRenderer);
    });
    return () => {
      cancelled = true;
    };
  }, []);
  if (!Renderer) {
    return (
      <div className={styles.tabEmpty} role="status">
        <ClayText variant="body" muted>
          Loading files…
        </ClayText>
      </div>
    );
  }
  return <Renderer state={sdui} send={send} editorSlot={null} />;
}

/** Typed agent-family request through the validated bridge path. */
function agentCommandPayload(command: Record<string, unknown>): string {
  return JSON.stringify({
    family: "agent",
    payload: { clientId: 0, command },
  });
}
