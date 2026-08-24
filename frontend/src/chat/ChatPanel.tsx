// Chat surface (Plan 097 Phase 10).
//
// Host-rendered presentation for the bundled `@clay/chat` empty-tab landing.
// Selection is provenance-exact (mirrors the SettingsPanel precedent): the
// generic inert SDUI renderer still owns every other package's landing, and
// disabling/replacing @clay/chat removes this view automatically. Greeting,
// hint copy, and setup actions are read from the package's declared
// component tree; prompt, streaming, cancellation, transcript, sessions, and
// error states flow through the one AG-UI stream (`TauriClayAgent`).

import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react";

import { ClayButton, ClayText, ClayTextField } from "../components";
import { chatAgent } from "../agent/state";
import { sendRequest } from "../bridge/client";
import type {
  PackageAction,
  PackageComponentNode,
  PackageSurface,
} from "../sdui/types";

import styles from "./chat.module.css";

/** Finds the first declared descendant with the given id. */
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

interface TranscriptRowProps {
  role: string;
  content: string;
  clayKind?: string;
}

/**
 * Memoized transcript row: per-token deltas rerender only the streaming row,
 * never the whole transcript.
 */
const TranscriptRow = memo(function TranscriptRow({
  role,
  content,
  clayKind,
}: TranscriptRowProps) {
  if (clayKind === "usage") {
    return (
      <div className={styles.usage} data-clay-kind="usage">
        <ClayText variant="detail" muted>
          {content}
        </ClayText>
      </div>
    );
  }
  if (clayKind === "error") {
    return (
      <p className={styles.error} data-clay-kind="error">
        <ClayText variant="status">{content}</ClayText>
      </p>
    );
  }
  if (role === "user") {
    return (
      <p className={styles.user}>
        <ClayText variant="body">{content}</ClayText>
      </p>
    );
  }
  if (role === "reasoning") {
    return (
      <p className={styles.thinking}>
        <ClayText variant="detail" muted>
          Thinking: {content}
        </ClayText>
      </p>
    );
  }
  return (
    <p className={styles.assistant}>
      <ClayText variant="body">{content}</ClayText>
    </p>
  );
});

export interface ChatPanelProps {
  surface: PackageSurface;
  /** Current runtime package UI version; validates every intent. */
  uiVersion: number;
}

export function ChatPanel({ surface, uiVersion }: ChatPanelProps) {
  const root = surface.component;
  const greeting = useMemo(() => findNode(root, "chat.greeting"), [root]);
  const providerHint = useMemo(
    () => findNode(root, "chat.providerHint"),
    [root],
  );
  const actionButtons = useMemo(() => {
    const buttons: Array<{ label: string; action: PackageAction }> = [];
    const walk = (node: PackageComponentNode) => {
      if (node.kind === "button" && node.action && node.label) {
        buttons.push({ label: node.label, action: node.action });
      }
      for (const child of node.children ?? []) walk(child);
    };
    walk(root);
    return buttons;
  }, [root]);

  const snapshot = useSyncExternalStore(
    chatAgent.subscribe,
    chatAgent.getSnapshot,
    chatAgent.getSnapshot,
  );

  // Intent validation must match the live runtime generation.
  useEffect(() => {
    chatAgent.agent.setUiVersion(uiVersion);
  }, [uiVersion]);

  // One process-wide AG-UI chat stream; refcounted across ChatPanel mounts.
  useEffect(() => chatAgent.start(), []);

  // Initial inventory: session list + provider configuration state.
  useEffect(() => {
    void sendRequest(agentCommandPayload({ listSessions: {} }));
  }, []);

  const [draft, setDraft] = useState("");
  const configured =
    typeof snapshot.state["provider"] === "string" &&
    (snapshot.state["provider"] as string).length > 0;
  const sessions = Array.isArray(snapshot.state["sessions"])
    ? (snapshot.state["sessions"] as Array<Record<string, unknown>>)
    : [];
  const streaming = snapshot.status.streaming;

  const runDeclaredAction = useCallback(
    (action: PackageAction) => {
      void sendRequest(packageActionPayload(uiVersion, action));
    },
    [uiVersion],
  );

  const submitPrompt = useCallback(async () => {
    const text = draft.trim();
    if (!text || !configured || streaming) return;
    setDraft("");
    chatAgent.agent.sendPrompt(text);
    await chatAgent.agent.runAgent().catch(() => {
      // Failures already landed as RUN_ERROR status.
    });
  }, [draft, configured, streaming]);

  return (
    <section className={styles.chat} aria-label="Chat" data-chat-panel>
      <header className={styles.header}>
        <ClayText variant="display">
          {(greeting?.text as string | undefined) ??
            "What do you want to do today?"}
        </ClayText>
        <ClayText variant="caption" muted>
          {configured
            ? `${String(snapshot.state["profile"] ?? "chat")} · ${String(
                snapshot.state["provider"] ?? "",
              )}/${String(snapshot.state["model"] ?? "")}`
            : ((providerHint?.text as string | undefined) ??
              "Configure a provider to start chatting.")}
        </ClayText>
      </header>

      {!configured && (
        <div className={styles.actions} role="group" aria-label="Chat setup">
          {actionButtons.map((button) => (
            <ClayButton
              key={button.action.commandId}
              onPress={() => runDeclaredAction(button.action)}
            >
              {button.label}
            </ClayButton>
          ))}
        </div>
      )}

      <div
        className={styles.transcriptScroll}
        aria-label="Transcript"
        role="log"
        aria-live="polite"
      >
        {snapshot.messages.length > 0 ? (
          snapshot.messages.map((message) => (
            <TranscriptRow
              key={message.id}
              role={message.role}
              content={
                typeof message.content === "string" ? message.content : ""
              }
              clayKind={
                (message as { metadata?: { clayKind?: string } }).metadata
                  ?.clayKind
              }
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

      {sessions.length > 0 && (
        <nav className={styles.sessions} aria-label="Chat sessions">
          <ClayText variant="section">Sessions</ClayText>
          <ul className={styles.sessionList}>
            {sessions.map((session) => (
              <li key={String(session["id"])} className={styles.sessionRow}>
                <span className={styles.sessionId}>
                  <ClayText variant="detail" muted>
                    {String(session["id"])}
                  </ClayText>
                </span>
                <span className={styles.sessionActions}>
                  <ClayButton
                    onPress={() =>
                      void sendRequest(
                        agentCommandPayload({
                          resumeSession: { sessionId: String(session["id"]) },
                        }),
                      )
                    }
                  >
                    Resume
                  </ClayButton>
                  <ClayButton
                    variant="danger"
                    onPress={() => {
                      void sendRequest(
                        agentCommandPayload({
                          deleteSession: { sessionId: String(session["id"]) },
                        }),
                      ).then(() =>
                        sendRequest(agentCommandPayload({ listSessions: {} })),
                      );
                    }}
                  >
                    Delete
                  </ClayButton>
                </span>
              </li>
            ))}
          </ul>
        </nav>
      )}

      <footer className={styles.composerArea}>
        <p className={styles.statusLine} role="status">
          <ClayText variant="status">
            {streaming ? "Streaming" : (snapshot.status.status ?? "Ready")}
          </ClayText>
        </p>
        <form
          className={styles.composer}
          onSubmit={(event) => {
            event.preventDefault();
            void submitPrompt();
          }}
        >
          <ClayTextField
            label="Message"
            value={draft}
            onChange={setDraft}
            placeholder={
              configured ? "Ask anything" : "Configure a provider first"
            }
            disabled={!configured || streaming}
          />
          <span className={styles.composerActions}>
            {streaming ? (
              <ClayButton onPress={() => chatAgent.agent.abortRun()}>
                Cancel
              </ClayButton>
            ) : (
              <ClayButton type="submit" disabled={!configured || !draft.trim()}>
                Send
              </ClayButton>
            )}
          </span>
        </form>
        <div className={styles.footerCommands}>
          {configured && (
            <>
              <ClayButton
                onPress={() =>
                  runDeclaredAction({
                    commandId: "agent.clientOpenAgentPicker",
                  })
                }
              >
                Agent
              </ClayButton>
              <ClayButton
                onPress={() =>
                  runDeclaredAction({
                    commandId: "agent.clientOpenProviderPicker",
                  })
                }
              >
                Provider
              </ClayButton>
              <ClayButton
                onPress={() =>
                  runDeclaredAction({
                    commandId: "agent.clientOpenModelPicker",
                  })
                }
              >
                Model
              </ClayButton>
            </>
          )}
        </div>
      </footer>
    </section>
  );
}

/** Validated `sduiAction` intent for a declared package action. */
function packageActionPayload(
  uiVersion: number,
  action: PackageAction,
): string {
  return JSON.stringify({
    family: "sduiAction",
    payload: {
      clientId: 0,
      uiVersion,
      intent: {
        commandId: action.commandId,
        source: { button: { nodeId: 1 } },
        arguments: [],
      },
    },
  });
}

/** Typed agent-family request through the validated bridge path. */
function agentCommandPayload(command: Record<string, unknown>): string {
  return JSON.stringify({
    family: "agent",
    payload: { clientId: 0, command },
  });
}
