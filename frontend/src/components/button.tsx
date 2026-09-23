import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Button as RACButton } from "react-aria-components";

import styles from "./button.module.css";
import { recipeAttributes } from "./recipe-attributes";
import { ClayIcon, useIconGeometry } from "./icon";
import { ClayTooltip } from "./tooltip";

export type ButtonVariant = "default" | "muted" | "primary" | "danger";

export interface ClayButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  children: ReactNode;
  /** Rendered when the button is disabled; keeps aria wiring host-owned. */
  onPress?: () => void;
}

/**
 * Catalog `button` kind. React Aria supplies press/hover/focus state
 * attributes; styling is token-only and state-complete
 * (rest/hover/active/focus/disabled).
 */
export function ClayButton({
  variant = "default",
  children,
  isDisabled,
  ...rest
}: ClayButtonProps & { isDisabled?: boolean }) {
  const attrs = recipeAttributes("button", "root", variant);
  return (
    <RACButton
      className={`${styles.button} ${styles[variant] ?? styles.default}`}
      isDisabled={isDisabled}
      {...attrs}
      {...(rest as Record<string, unknown>)}
    >
      {children}
    </RACButton>
  );
}

export interface ClayIconButtonProps {
  /** Semantic icon key from the active icon pack. */
  icon: string;
  /**
   * Required accessible name — the type makes unlabeled icon-only usage
   * impossible. Doubles as the tooltip text and the visible fallback label
   * when the icon key has no active or bundled geometry.
   */
  label: string;
  /** Configured keyboard shortcut shown in the tooltip when available. */
  shortcut?: string;
  variant?: ButtonVariant;
  isDisabled?: boolean;
  /** Native submit semantics for icon-only form actions (e.g. composer Send). */
  type?: "button" | "submit" | "reset";
  onPress?: () => void;
}

/**
 * Icon-only catalog `button` composition (Plan 112 Task 7): one accessible
 * name per control, decorative 16px glyph inside a ≥24px CSS hit target
 * (WCAG 2.2 minimum), contextual hover/focus tooltip via the catalog tooltip,
 * and the shared button state set. Missing icon never creates a blank action:
 * the button falls back to the visible text label.
 */
export function ClayIconButton({
  icon,
  label,
  shortcut,
  variant = "default",
  isDisabled = false,
  type,
  onPress,
}: ClayIconButtonProps) {
  const hasGlyph = useIconGeometry(icon) !== null;
  const tooltipText = shortcut ? `${label} (${shortcut})` : label;

  const button = (
    <RACButton
      className={`${styles.button} ${styles.iconButton} ${styles[variant] ?? styles.default}`}
      isDisabled={isDisabled}
      aria-label={label}
      type={type}
      onPress={onPress}
      {...recipeAttributes("button", "root", variant)}
    >
      {hasGlyph ? (
        <ClayIcon name={icon} />
      ) : (
        <span className={styles.iconButtonFallbackLabel}>{label}</span>
      )}
    </RACButton>
  );

  return (
    <ClayTooltip label={tooltipText} placement="top">
      {button}
    </ClayTooltip>
  );
}
