// Settings tab (plan 119 SC-4): the agent's delivered config files. Split out
// of the panel so the inspector's wiring stays a composition, not a monolith.
import { useEffect, useState } from "react";
import { ClayText } from "../components";
import {
  AgentSettingsPanel,
  type AgentSettingsFileInfo,
} from "../agent-settings/AgentSettingsPanel";
import type { DocumentSession } from "../editor/sync/session";
import styles from "./coding-agent.module.css";

/**
 * Settings tab (plan 117 follow-up): the agent's delivered config files
 * (SYSTEM.md + seeded SKILL.md), with built-in-vs-edited provenance. The
 * listing and the open both ride this pane's own document session, so the
 * tab needs no shell state and no extra protocol surface: the reply arrives
 * as an `agentSettingsFiles` feature event on that same session.
 */
export function SettingsTab({
  session,
  open,
  sendIntent,
}: {
  session: DocumentSession | null;
  open: boolean;
  sendIntent: (commandId: string) => void;
}) {
  const [files, setFiles] = useState<AgentSettingsFileInfo[] | null>(null);
  useEffect(() => {
    if (!open || !session) return;
    const unsubscribe = session.subscribeFeatures((envelope) => {
      const event = envelope.data as {
        kind?: string;
        data?: { files?: AgentSettingsFileInfo[] };
      };
      if (event.kind !== "agentSettingsFiles" || !event.data) return;
      setFiles(event.data.files ?? []);
    });
    session.listAgentSettings();
    return unsubscribe;
  }, [open, session]);
  if (!session) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          No active agent session.
        </ClayText>
      </div>
    );
  }
  return (
    <AgentSettingsPanel
      files={files}
      loading={files == null}
      onOpen={(name) => {
        session.openAgentSettings(name);
        // The opened file is an ordinary document: release the agent surface
        // so the pane shows the editor instead of this panel (the same
        // release the composer's Close button performs).
        sendIntent("coding-agent.close");
      }}
    />
  );
}
