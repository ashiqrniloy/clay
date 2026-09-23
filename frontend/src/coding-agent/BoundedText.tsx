// Plan 109 I10: the shared bounded-text viewer (read-only, redacted,
// wrapping) — used by the Session Info detail and the Context drawer, whose
// kinds overlap (both render transcript-derived bounded text).
import styles from "./coding-agent.module.css";

/**
 * Plan 109 I10: shared bounded-text viewer — read-only, redacted,
 * wrapping. Used by the Session Info detail and the Context drawer
 * (the kinds overlap: both render transcript-derived bounded text).
 */
export function BoundedText({ text }: { text: string }) {
  return <pre className={styles.detailContent}>{text}</pre>;
}
