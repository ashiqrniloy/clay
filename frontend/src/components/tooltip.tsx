/**
 * Catalog tooltip (Plan 112 Task 7). Accessible shared tooltip over React
 * Aria `TooltipTrigger`/`Tooltip`: shows on hover **and** keyboard focus,
 * closes on Escape and blur (WCAG 1.4.13 dismissible), and renders as
 * `role="tooltip"` with an automatic `aria-describedby` on the trigger.
 *
 * Host-owned content only: the label is a plain string (action name plus the
 * configured shortcut when available). Packages cannot inject tooltip JSX,
 * HTML, or attributes.
 */

import type { ReactElement } from "react";
import {
  Tooltip as RACTooltip,
  TooltipTrigger,
  type Placement,
} from "react-aria-components";

import styles from "./tooltip.module.css";
import { recipeAttributes } from "./recipe-attributes";

export interface ClayTooltipProps {
  /** Tooltip text: the action's accessible name, plus the configured
   *  keyboard shortcut when one exists. */
  label: string;
  /** Placement relative to the trigger. Default `top`. */
  placement?: "top" | "bottom" | "left" | "right" | "start" | "end";
  /** Single trigger element (Clay control or native element). Receives
   *  hover/focus listeners and `aria-describedby` from the tooltip. */
  children: ReactElement;
}

export function ClayTooltip({
  label,
  placement = "top",
  children,
}: ClayTooltipProps) {
  return (
    <TooltipTrigger>
      {children}
      <RACTooltip
        placement={placement as Placement}
        className={styles.tooltip}
        {...recipeAttributes("tooltip", "root")}
      >
        {label}
      </RACTooltip>
    </TooltipTrigger>
  );
}
