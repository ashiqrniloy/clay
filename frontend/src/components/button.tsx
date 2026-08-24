import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Button as RACButton } from "react-aria-components";

import styles from "./button.module.css";

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
  return (
    <RACButton
      className={`${styles.button} ${styles[variant] ?? styles.default}`}
      isDisabled={isDisabled}
      {...(rest as Record<string, unknown>)}
    >
      {children}
    </RACButton>
  );
}
