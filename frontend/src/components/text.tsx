import type { HTMLAttributes, ReactNode } from "react";

import styles from "./text.module.css";

export type TextVariant =
  "display" | "title" | "section" | "body" | "status" | "detail" | "caption";

export type FontRole = "ui" | "monospace" | "proportional";

export interface ClayTextProps extends HTMLAttributes<HTMLElement> {
  /** Semantic typography variant (never concrete sizes). */
  variant?: TextVariant;
  role?: FontRole;
  muted?: boolean;
  disabled?: boolean;
  children: ReactNode;
}

/** Catalog `label`/`statusItem` text. Plain semantics, token styling. */
export function ClayText({
  variant = "body",
  role = "ui",
  muted = false,
  disabled = false,
  children,
  ...rest
}: ClayTextProps) {
  const classes = [
    styles.text,
    styles[variant],
    role !== "ui" ? styles[role] : "",
    disabled ? styles.disabled : muted ? styles.muted : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={classes} {...rest}>
      {children}
    </span>
  );
}
