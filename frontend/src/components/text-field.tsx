import type { KeyboardEventHandler } from "react";
import { useLayoutEffect, useRef } from "react";
import {
  TextField as RACTextField,
  Input,
  Label,
  TextArea,
} from "react-aria-components";

import styles from "./text-field.module.css";
import { recipeAttributes } from "./recipe-attributes";

export type ValidationState = "none" | "error" | "warning" | "success";

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
  description?: string;
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
  description,
  onSubmit,
  autoFocus = false,
  onKeyDown,
}: ClayTextFieldProps) {
  const areaRef = useRef<HTMLTextAreaElement | null>(null);
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
  return (
    <RACTextField
      className={styles.field}
      value={value}
      onChange={onChange}
      isDisabled={disabled}
      isInvalid={validationState === "error"}
      aria-describedby={description ? `${label}-description` : undefined}
      {...recipeAttributes("textInput", "field")}
    >
      <Label
        className={styles.label}
        {...recipeAttributes("textInput", "label")}
      >
        {label}
      </Label>
      {multiline ? (
        <TextArea
          ref={areaRef}
          className={`${styles.input} ${validationClass} ${
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
          className={`${styles.input} ${validationClass}`}
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
      {description && (
        <span
          id={`${label}-description`}
          className={styles.label}
          {...recipeAttributes("textInput", "description")}
        >
          {description}
        </span>
      )}
    </RACTextField>
  );
}
