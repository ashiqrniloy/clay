// Session Info tab (plan 119 SC-4): the card-detail destination. Renders the
// selected transcript entry's kind, complete redacted content, and type-specific
// metadata from the already-loaded transcript — no refetch, no new data path.
import { ClayButton, ClayIcon, ClayText } from "../components";
import { BoundedText } from "./BoundedText";
import type { TranscriptBox } from "./transcript-model";
import styles from "./coding-agent.module.css";

/**
 * Session Info tab (plan 109 I10): the card-detail destination. Renders
 * the selected transcript entry's kind, complete redacted content, and
 * type-specific metadata from the already-loaded transcript — no refetch,
 * no new data path. No selection → guidance. Back restores the tab the
 * selection came from.
 */
export function SessionInfoTab({
  selected,
  onBack,
}: {
  selected: TranscriptBox | null;
  onBack: () => void;
}) {
  if (!selected) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          Select a transcript card to inspect its full detail here.
        </ClayText>
      </div>
    );
  }
  const rows: Array<[string, string]> = [["Kind", selected.kind]];
  if (selected.kind === "tool" || selected.kind === "skill") {
    rows.push([
      selected.kind === "skill" ? "Skill" : "Tool",
      selected.toolName || selected.label,
    ]);
  }
  return (
    <div className={styles.tabEmpty}>
      <div className={styles.detailHeader}>
        <ClayText variant="caption" muted>
          {selected.label}
        </ClayText>
        <ClayButton variant="muted" onPress={onBack}>
          <ClayIcon name="navigation.back" />
          Back
        </ClayButton>
      </div>
      <dl className={styles.infoMeta}>
        {rows.map(([term, value]) => (
          <div key={term} className={styles.infoRow}>
            <dt>{term}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <BoundedText text={selected.content} />
    </div>
  );
}
