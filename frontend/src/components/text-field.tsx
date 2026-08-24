import type { KeyboardEventHandler } from "react";
import {
  TextField as RACTextField,
  Input,
  Label,
  TextArea,
} from "react-aria-components";

import styles from "./text-field.module.css";

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
  description,
  onSubmit,
  autoFocus = false,
  onKeyDown,
}: ClayTextFieldProps) {
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
    >
      <Label className={styles.label}>{label}</Label>
      {multiline ? (
        <TextArea
          className={`${styles.input} ${validationClass}`}
          placeholder={placeholder}
          rows={3}
          autoFocus={autoFocus}
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
          onKeyDown={(event) => {
            onKeyDown?.(event);
            if (!event.defaultPrevented && event.key === "Enter")
              onSubmit?.(value);
          }}
        />
      )}
      {description && (
        <span id={`${label}-description`} className={styles.label}>
          {description}
        </span>
      )}
    </RACTextField>
  );
}
