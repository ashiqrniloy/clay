import type { ReactNode } from "react";

import styles from "./chrome.module.css";

/** Catalog badge/tag: `status`/`note` semantics via text content. */
export function ClayBadge({ children }: { children: ReactNode }) {
  return <span className={styles.badge}>{children}</span>;
}

/** Catalog kbd hint: monospace role + bordered token style. */
export function ClayKbd({ children }: { children: ReactNode }) {
  return <kbd className={styles.kbd}>{children}</kbd>;
}

/** `paint_divider` projection: full-width hairline separator. */
export function ClayDivider() {
  return <hr className={styles.divider} />;
}
