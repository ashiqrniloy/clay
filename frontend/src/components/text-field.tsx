import type { KeyboardEventHandler, ReactNode } from "react";
import { useId, useLayoutEffect, useRef } from "react";
import {
  TextField as RACTextField,
  Input,
  Label,
  TextArea,
} from "react-aria-components";

import styles from "./text-field.module.css";
import { recipeAttributes } from "./recipe-attributes";
import type { FontRole } from "./text";

export type ValidationState = "none" | "error" | "warning" | "success";

/** `default` paints the boundary on the control (`input`, 8px, single line);
 *  `composer` paints it on the shell (`field`, 12px, multiline) so the shell
 *  owns the one focus ring and anything passed as `endContent` sits inside it
 *  (DESIGN.md §11 composer/textarea radius, §12 one boundary per surface). */
export type TextFieldVariant = "default" | "composer";

export interface ClayTextFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  validationState?: ValidationState;
  disabled?: boolean;
  /** Multiline composer variant (catalog gap; generic kind lands later). */
  multiline?: boolean;
  /** Multiline auto-grow: height follows content up to the CSS max-height. */
  autoGrow?: boolean;
  /** Which slot paints the well (see `TextFieldVariant`). */
  variant?: TextFieldVariant;
  /** Font role for the control's value: `monospace` for data (sizes, ratios,
   *  paths), `ui` for prose (DESIGN.md §8). */
  role?: FontRole;
  /** Row-style layouts label the field outside the control (a settings row
   *  label): the field keeps its accessible name but paints no second label. */
  labelHidden?: boolean;
  /** Rendered inside the field, after the control — the composer's
   *  send/cancel actions, which the approved layout places inside the
   *  shell's trailing edge. */
  endContent?: ReactNode;
  description?: string;
  /** The field's error message, rendered as the declared error slot below the
   *  well (`textInput.default.error.rest`) and announced through
   *  `aria-describedby`. Absent, no slot and no association (plan 118 E6). */
  errorMessage?: ReactNode;
  onSubmit?: (value: string) => void;
  autoFocus?: boolean;
  onKeyDown?: KeyboardEventHandler<HTMLInputElement | HTMLTextAreaElement>;
}

/** Catalog `textInput` kind over native input/textarea semantics. */
export function ClayTextField({
  label,
  value,
  onChange,
  placeholder,
  validationState = "none",
  disabled = false,
  multiline = false,
  autoGrow = false,
  variant = "default",
  role = "ui",
  labelHidden = false,
  endContent,
  description,
  errorMessage,
  onSubmit,
  autoFocus = false,
  onKeyDown,
}: ClayTextFieldProps) {
  const areaRef = useRef<HTMLTextAreaElement | null>(null);
  // Slot ids come from `useId`, not from the label: an ARIA IDREF list is
  // space-separated, so a label like "UI size" produced `UI` + `size-error`
  // and announced nothing (plan 118 task E6 caught this while wiring the error
  // slot).
  const fieldId = useId();
  const descriptionId = `${fieldId}-description`;
  const errorId = `${fieldId}-error`;
  // Auto-grow (plan 108 G3): presentation-only height sync; the cap lives in
  // CSS (max-height), so this never blocks input or waits on IPC.
  useLayoutEffect(() => {
    const element = areaRef.current;
    if (!autoGrow || !multiline || !element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [autoGrow, multiline, value]);
  const validationClass =
    validationState === "none" ? "" : (styles[validationState] ?? "");
  // One description id and one error id; both are announced, in that order.
  const describedBy =
    [description && descriptionId, errorMessage && errorId]
      .filter(Boolean)
      .join(" ") || undefined;
  const roleClass = role === "monospace" ? styles.monospace : "";
  return (
    <RACTextField
      className={`${styles.field} ${variant === "composer" ? styles.composer : ""}`}
      value={value}
      onChange={onChange}
      isDisabled={disabled}
      isInvalid={validationState === "error"}
      aria-describedby={describedBy}
      {...recipeAttributes("textInput", "field")}
    >
      <Label
        className={labelHidden ? styles.labelHidden : styles.label}
        {...recipeAttributes("textInput", "label")}
      >
        {label}
      </Label>
      {multiline ? (
        <TextArea
          ref={areaRef}
          className={`${styles.input} ${roleClass} ${validationClass} ${
            autoGrow ? styles.autoGrow : ""
          }`}
          placeholder={placeholder}
          rows={3}
          autoFocus={autoFocus}
          {...recipeAttributes("textInput", "input")}
          onKeyDown={(event) => {
            onKeyDown?.(event);
            if (event.defaultPrevented) return;
            if (event.key === "Enter" && !event.shiftKey && onSubmit) {
              event.preventDefault();
              onSubmit(value);
            }
          }}
        />
      ) : (
        <Input
          className={`${styles.input} ${roleClass} ${validationClass}`}
          placeholder={placeholder}
          autoFocus={autoFocus}
          {...recipeAttributes("textInput", "input")}
          onKeyDown={(event) => {
            onKeyDown?.(event);
            if (!event.defaultPrevented && event.key === "Enter")
              onSubmit?.(value);
          }}
        />
      )}
      {endContent}
      {description && (
        <span
          id={descriptionId}
          className={styles.description}
          {...recipeAttributes("textInput", "description")}
        >
          {description}
        </span>
      )}
      {errorMessage && (
        <span
          id={errorId}
          className={styles.error}
          {...recipeAttributes("textInput", "error")}
        >
          {errorMessage}
        </span>
      )}
    </RACTextField>
  );
}
