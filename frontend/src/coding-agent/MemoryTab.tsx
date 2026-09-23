// Memory tab (plan 119 SC-4): the Observer's activity log plus OM worker-model
// selection. Server-authoritative view (`session.om.activity`), fetched on tab
// open / transcript change (completion boundaries, never polled).
import { useEffect } from "react";
import { ClayDropdown, ClayText, type DropdownOption } from "../components";
import styles from "./coding-agent.module.css";

/**
 * Observational Memory tab (plan 109 I8): the Observer's activity log —
 * recorded observations with fact summaries, reflections, drops (visible,
 * never silently vanished), and compaction folds — plus OM worker-model
 * selection over the same grouped model list as the session model. The
 * view is server-authoritative (`session.om.activity`), fetched on tab
 * open / transcript change (completion boundaries, never polled); worker
 * selections ride `SelectWorker` (per-workspace book + per-session
 * daemon metadata) and may differ from the session model.
 */
interface OmActivityRow {
  id: string;
  kind: string;
  summary: string;
}

export interface OmView {
  sessionId?: string;
  attached?: boolean;
  observation?: { provider?: string; model?: string } | null;
  reflection?: { provider?: string; model?: string } | null;
  activity?: OmActivityRow[];
}

const OM_WORKER_CLEAR = "om-worker:none";

function OmWorkerDropdown({
  label,
  selection,
  groups,
  onSelect,
}: {
  label: string;
  selection: { provider?: string; model?: string } | null;
  groups: Array<{ label: string; options: DropdownOption[] }>;
  onSelect: (id: string) => void;
}) {
  const selectedId = selection?.provider
    ? `model:${selection.provider}/${selection.model}`
    : OM_WORKER_CLEAR;
  const clearOption = { id: OM_WORKER_CLEAR, label: "Not set (workers off)" };
  // The clear option must live in the rendered set: ClayDropdown ignores
  // `options` when `groups` is present, so the clear entry is prepended to
  // the first group (or stands alone when no models are configured).
  const firstGroup = groups[0];
  const groupsWithClear = firstGroup
    ? [
        {
          label: firstGroup.label,
          options: [clearOption, ...firstGroup.options],
        },
        ...groups.slice(1),
      ]
    : undefined;
  return (
    <div className={styles.omWorkerRow}>
      <ClayText variant="caption" muted>
        {label}
      </ClayText>
      <ClayDropdown
        label={label}
        options={[clearOption]}
        groups={groupsWithClear}
        selectedId={selectedId}
        onSelect={onSelect}
      />
    </div>
  );
}

export function MemoryTab({
  sessionId,
  open,
  transcriptLength,
  view,
  modelGroups,
  command,
}: {
  sessionId: string;
  open: boolean;
  transcriptLength: number;
  view: OmView | undefined;
  modelGroups: Array<{ label: string; options: DropdownOption[] }>;
  /** The tab's own agent command lane (plan 119 SC-6). */
  command: (command: Record<string, unknown> | string) => void;
}) {
  // The activity list follows run/OM-worker completion via the transcript
  // length (the same completion-boundary signal the transcript uses).
  useEffect(() => {
    if (!open || !sessionId) return;
    command({ omActivity: { sessionId } });
  }, [command, open, sessionId, transcriptLength]);
  const selectWorker =
    (worker: "observation" | "reflection") => (id: string) => {
      if (!sessionId) return;
      const clear = id === OM_WORKER_CLEAR;
      command({
        selectWorker: {
          worker,
          id: clear ? "" : id,
          sessionId,
        },
      });
    };
  if (!sessionId) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          No active agent session.
        </ClayText>
      </div>
    );
  }
  const stale =
    view && typeof view.sessionId === "string" && view.sessionId !== sessionId;
  const activity = (!stale && view?.activity) || [];
  const attached = !stale && view?.attached === true;
  return (
    <div className={styles.omTab}>
      <div className={styles.omWorkers}>
        <OmWorkerDropdown
          label="Observation worker"
          selection={stale ? null : (view?.observation ?? null)}
          groups={modelGroups}
          onSelect={selectWorker("observation")}
        />
        <OmWorkerDropdown
          label="Reflection worker"
          selection={stale ? null : (view?.reflection ?? null)}
          groups={modelGroups}
          onSelect={selectWorker("reflection")}
        />
      </div>
      <ClayText variant="caption" muted>
        {attached
          ? "Observer activity this session"
          : "Observational Memory is off for this session."}
      </ClayText>
      {activity.length > 0 ? (
        <ul className={styles.omActivity} aria-label="Observer activity">
          {activity.map((row) => (
            <li key={row.id} className={styles.omActivityRow}>
              <span
                className={`${styles.omKind} ${
                  styles[`omKind_${row.kind}` as keyof typeof styles] ?? ""
                }`}
              >
                {row.kind}
              </span>
              <ClayText variant="detail">{row.summary}</ClayText>
            </li>
          ))}
        </ul>
      ) : (
        <div className={styles.tabEmpty}>
          <ClayText variant="caption" muted>
            {attached
              ? "No observer activity yet."
              : "Select worker models to let the Observer record activity."}
          </ClayText>
        </div>
      )}
    </div>
  );
}
