// Inspector tabs (plan 119 SC-4): the agent view's right column. Owns the tab
// strip and the five tab bodies' wiring — the tab ids/labels live here, so the
// panel stays the left column's composition root and knows none of them.
//
// Every tab is mounted with its `open` flag (the strip keeps its content
// mounted), and each fetch keys off that flag plus the transcript length, which
// is the completion-boundary signal the whole panel invalidates on.
import {
  ClayIconButton,
  ClayTabStrip,
  type DropdownOption,
} from "../components";
import type { DocumentSession } from "../editor/sync/session";
import type { SessionFileRecord } from "../agent/session-files";
import {
  ContextTab,
  type ContextItemDetail,
  type ContextView,
} from "./ContextTab";
import { FilesTab } from "./FilesTab";
import { MemoryTab, type OmView } from "./MemoryTab";
import { SessionInfoTab } from "./SessionInfoTab";
import { SettingsTab } from "./SettingsTab";
import type { TranscriptBox } from "./transcript-model";
import styles from "./coding-agent.module.css";

export interface InspectorTabsProps {
  /** Open tab id (the panel owns it: a transcript card switches to Session
   *  Info and Back restores the previous tab). */
  activeTab: string;
  onActivate: (id: string) => void;
  /** Hide the inspector (its visibility is the tab's own layout state). */
  onHide: () => void;
  /** The session's file records (transcript tool rows). */
  files: readonly SessionFileRecord[];
  /** Open a path in the tab's workspace view. */
  onOpenInWorkspace: (path: string) => void;
  /** The bound agent session id (`""` = no session yet). */
  sessionId: string;
  /** Transcript length: the invalidation signal every tab fetch keys off. */
  transcriptLength: number;
  omView: OmView | undefined;
  modelGroups: Array<{ label: string; options: DropdownOption[] }>;
  contextView: ContextView | null;
  contextDetail: ContextItemDetail | null;
  /** Capability inventories: reference data, so it lives in the Context tab
   *  and never in the transcript (DESIGN.md §12). */
  skills: ReadonlyArray<{ name: string; description: string }>;
  servers: ReadonlyArray<{
    serverId: string;
    connected: boolean;
    tools: number;
    error: string;
  }>;
  /** The tab's own agent command lane (plan 119 SC-6). */
  command: (command: Record<string, unknown> | string) => void;
  /** The selected transcript turn (Session Info's detail). */
  selected: TranscriptBox | null;
  onBack: () => void;
  /** The pane's document session (Settings tab). */
  session: DocumentSession | null;
  /** Intent sender for the Settings tab's open action. */
  sendIntent: (commandId: string) => void;
}

export function InspectorTabs({
  activeTab,
  onActivate,
  onHide,
  files,
  onOpenInWorkspace,
  sessionId,
  transcriptLength,
  omView,
  modelGroups,
  contextView,
  contextDetail,
  skills,
  servers,
  command,
  selected,
  onBack,
  session,
  sendIntent,
}: InspectorTabsProps) {
  return (
    <aside className={styles.inspector} aria-label="Agent inspector">
      <ClayTabStrip
        className={styles.tabs}
        activeId={activeTab}
        onActivate={onActivate}
        ariaLabel="Agent detail"
        actions={
          <ClayIconButton
            icon="preview.toggle"
            label="Hide inspector"
            variant="muted"
            onPress={onHide}
          />
        }
        tabs={[
          {
            id: "files",
            label: "Files",
            content: (
              <FilesTab
                records={files}
                open={activeTab === "files"}
                onOpen={onOpenInWorkspace}
              />
            ),
          },
          {
            id: "memory",
            label: "Memory",
            content: (
              <MemoryTab
                sessionId={sessionId}
                open={activeTab === "memory"}
                transcriptLength={transcriptLength}
                view={omView}
                modelGroups={modelGroups}
                command={command}
              />
            ),
          },
          {
            id: "context",
            label: "Context",
            content: (
              <ContextTab
                sessionId={sessionId}
                open={activeTab === "context"}
                transcriptLength={transcriptLength}
                view={contextView}
                skills={skills}
                servers={servers}
                detail={contextDetail}
                command={command}
              />
            ),
          },
          {
            // Plan 109 I10: card-detail destination. Selecting a transcript
            // card auto-switches here and renders that entry's full redacted
            // content from the already-loaded transcript — no refetch.
            id: "session-info",
            label: "Session Info",
            content: <SessionInfoTab selected={selected} onBack={onBack} />,
          },
          {
            // Plan 117 follow-up: the agent's delivered config files. Replaces
            // the shell-owned side panel — the listing rides this pane's own
            // document session, so the shell keeps no agent-settings state.
            id: "settings",
            label: "Settings",
            content: (
              <SettingsTab
                session={session}
                open={activeTab === "settings"}
                sendIntent={sendIntent}
              />
            ),
          },
        ]}
      />
    </aside>
  );
}
