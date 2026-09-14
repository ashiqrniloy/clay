import type { ReactNode } from "react";

import styles from "./chrome.module.css";
import { recipeAttributes } from "./recipe-attributes";

/** Badge tones: the badge *is* the chip (DESIGN.md §11), so its variants are
 * the chip's roles. `default` paints from the base `badge.default.root.rest`
 * recipe; the other tones come from their own keys. */
export type BadgeTone =
  "default" | "accent" | "muted" | "error" | "warning" | "success";

/** Catalog badge/tag: `status`/`note` semantics via text content and tone. */
export function ClayBadge({
  children,
  tone = "default",
}: {
  children: ReactNode;
  tone?: BadgeTone;
}) {
  const toneClass = tone === "default" ? "" : styles[tone];
  return (
    <span
      className={`${styles.badge} ${toneClass}`.trim()}
      {...recipeAttributes("badge", "root")}
    >
      {children}
    </span>
  );
}

/** Catalog kbd hint: monospace role + bordered token style. */
export function ClayKbd({ children }: { children: ReactNode }) {
  return (
    <kbd className={styles.kbd} {...recipeAttributes("kbd", "root")}>
      {children}
    </kbd>
  );
}

/** `paint_divider` projection: full-width hairline separator. */
export function ClayDivider() {
  return (
    <hr className={styles.divider} {...recipeAttributes("divider", "root")} />
  );
}
