// Transcript list (plan 119 SC-4): the left column's conversation. Turns, not
// cards (DESIGN.md §12) — one hairline-separated block per entry with a turn
// head (role micro-label left, kind right) and either prose or a uniform
// truncated tool box. The AG-UI message carries no timestamp, so none is shown
// rather than invented. Selecting a turn opens its full content in Session Info.
import { memo, useMemo } from "react";
import { ClayKbd } from "../components";
import {
  agentLabel,
  previousAgents,
  type TranscriptBox,
  type TranscriptKind,
} from "./transcript-model";
import styles from "./coding-agent.module.css";

/**
 * The transcript is turns, not cards (DESIGN.md §12): one hairline-separated
 * block per entry with a turn head — role micro-label left, kind right — and
 * either prose (the conversation) or a uniform truncated tool box (machine
 * output). The AG-UI message carries no timestamp, so none is shown rather
 * than invented. Selecting a turn opens its full content in Session Info.
 */
const TURN_HEADS: Record<TranscriptKind, { role: string; note: string }> = {
  user: { role: "you", note: "prompt" },
  assistant: { role: "agent", note: "turn" },
  reasoning: { role: "thinking", note: "reasoning" },
  tool: { role: "tool", note: "call" },
  skill: { role: "skill", note: "skill" },
  usage: { role: "usage", note: "tokens" },
  error: { role: "error", note: "failed" },
};

/** Machine output is clamped to a fixed line count so box height never depends
 *  on content; the conversation reads in full. */
const BOXED_KINDS = new Set<TranscriptKind>([
  "tool",
  "skill",
  "usage",
  "error",
]);

const TranscriptTurn = memo(function TranscriptTurn({
  box,
  selected,
  showAgent,
  agentChangedFrom,
  onSelect,
}: {
  box: TranscriptBox;
  selected: boolean;
  /** More than one agent produced this transcript: label every turn with its
   *  own agent so a switch is readable turn by turn (plan 118 task 35). */
  showAgent: boolean;
  /** The agent the previous turn ran as, when it differs (the switch note). */
  agentChangedFrom?: string;
  onSelect: (id: string) => void;
}) {
  const head = TURN_HEADS[box.kind];
  // Tool/skill rows name the thing that ran where the generic kind would be.
  const note =
    box.kind === "tool" || box.kind === "skill" ? box.label : head.note;
  return (
    <button
      type="button"
      className={styles.turn}
      data-role={box.kind}
      data-selected={selected}
      data-agent={box.agent ?? undefined}
      aria-pressed={selected}
      aria-label={`${box.label} ${box.content}`}
      onClick={() => onSelect(selected ? "" : box.id)}
    >
      {agentChangedFrom && box.agent ? (
        <span className={styles.turnSwitch} data-turn-switch>
          {`Switched to ${agentLabel(box.agent)}`}
        </span>
      ) : null}
      <span className={styles.turnHead}>
        <span className={styles.turnRole}>
          {showAgent && box.agent ? agentLabel(box.agent) : head.role}
        </span>
        <span className={styles.spacer} />
        <span className={styles.turnNote}>{note}</span>
      </span>
      {BOXED_KINDS.has(box.kind) ? (
        <span className={styles.turnBox}>
          <span className={styles.turnBoxText}>{box.content}</span>
        </span>
      ) : (
        <span className={styles.turnBody}>{box.content}</span>
      )}
    </button>
  );
});

/** The first-run state: the keyboard path, and nothing pretending to be a
 *  conversation (DESIGN.md §12 — real values only). */
const EmptyTranscript = memo(function EmptyTranscript() {
  return (
    <div className={styles.empty} role="status">
      <p className={styles.emptyTitle}>No conversation yet.</p>
      <p className={styles.emptyText}>
        Ask a question, or drive the session from the keyboard. Skills and tools
        live in the Context tab until they are needed.
      </p>
      <div className={styles.keyHints}>
        <span className={styles.keyHint}>
          <ClayKbd>/</ClayKbd> commands
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>@</ClayKbd> files &amp; skills
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>⇧</ClayKbd>
          <ClayKbd>↵</ClayKbd> newline
        </span>
      </div>
    </div>
  );
});

/** With more than one producer in the transcript, every turn shows its own
 *  agent and the switch gets a note row (plan 118 task 35) — derived from the
 *  stamped rows, so it survives a reload. */
export function TranscriptList({
  boxes,
  selectedId,
  onSelect,
}: {
  boxes: readonly TranscriptBox[];
  selectedId: string;
  onSelect: (id: string) => void;
}) {
  const multiAgent = useMemo(() => {
    const seen = new Set<string>();
    for (const box of boxes) if (box.agent) seen.add(box.agent);
    return seen.size > 1;
  }, [boxes]);
  // PF-2: one pass for every turn's predecessor, not a scan per turn.
  const previous = useMemo(() => previousAgents(boxes), [boxes]);
  return (
    <div
      className={styles.transcript}
      role="log"
      aria-label="Transcript"
      aria-live="polite"
    >
      <div className={styles.transcriptInner}>
        {boxes.length > 0 ? (
          boxes.map((box, index) => (
            <TranscriptTurn
              key={box.id}
              box={box}
              selected={box.id === selectedId}
              showAgent={multiAgent}
              agentChangedFrom={
                multiAgent && box.agent && previous[index] !== box.agent
                  ? previous[index]
                  : undefined
              }
              onSelect={onSelect}
            />
          ))
        ) : (
          <EmptyTranscript />
        )}
      </div>
    </div>
  );
}
