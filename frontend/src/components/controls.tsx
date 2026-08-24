import { useState, type ReactNode } from "react";
import {
  ListBox,
  ListBoxItem,
  Select,
  SelectValue,
  Button,
  Popover,
  Disclosure,
  DisclosurePanel,
} from "react-aria-components";

import styles from "./controls.module.css";

// ---------------------------------------------------------------- dropdown

export interface DropdownOption {
  id: string;
  label: string;
  disabled?: boolean;
}

export interface ClayDropdownProps {
  label: string;
  options: DropdownOption[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  disabled?: boolean;
}

/** Catalog `dropdown` kind: button trigger + listbox, arrow/typeahead nav. */
export function ClayDropdown({
  label,
  options,
  selectedId,
  onSelect,
  disabled = false,
}: ClayDropdownProps) {
  const selected = options.find((option) => option.id === selectedId);
  return (
    <Select
      aria-label={label}
      selectedKey={selectedId}
      onSelectionChange={(key) => onSelect(String(key))}
      isDisabled={disabled}
    >
      <Button className={styles.selectTrigger}>
        <SelectValue>{selected?.label ?? label}</SelectValue>
        <span aria-hidden="true">▾</span>
      </Button>
      <Popover className={styles.popover}>
        <ListBox className={styles.listBox}>
          {options.map((option) => (
            <ListBoxItem
              key={option.id}
              id={option.id}
              className={styles.listRow}
              isDisabled={option.disabled}
            >
              {option.label}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </Select>
  );
}

// -------------------------------------------------------------------- list

export interface ListItem {
  id: string;
  title: string;
  detail?: string;
  disabled?: boolean;
}

export interface ClayListProps {
  items: ListItem[];
  selectedId?: string | null;
  onSelect?: (id: string) => void;
  onAction?: (id: string) => void;
  ariaLabel: string;
}

/** Catalog `list` kind: rows with title/detail and selection semantics. */
export function ClayList({
  items,
  selectedId = null,
  onSelect,
  onAction,
  ariaLabel,
}: ClayListProps) {
  return (
    <ListBox
      className={styles.listBox}
      aria-label={ariaLabel}
      selectionMode={onSelect ? "single" : "none"}
      selectedKeys={
        onSelect ? new Set(selectedId ? [selectedId] : []) : undefined
      }
      onSelectionChange={
        onSelect
          ? (keys) => {
              const [key] = keys;
              if (key !== undefined) onSelect(String(key));
            }
          : undefined
      }
      onAction={onAction ? (key) => onAction(String(key)) : undefined}
    >
      {items.map((item) => (
        <ListBoxItem
          key={item.id}
          id={item.id}
          textValue={item.title}
          className={styles.listRow}
          isDisabled={item.disabled}
        >
          <span>{item.title}</span>
          {item.detail && (
            <span className={styles.rowDetail}>{item.detail}</span>
          )}
        </ListBoxItem>
      ))}
    </ListBox>
  );
}

// ---------------------------------------------------------------- collapse

export interface ClayCollapseProps {
  title: string;
  children: ReactNode;
  defaultExpanded?: boolean;
}

/**
 * Catalog `collapse` kind: disclosure pattern (`aria-expanded`/`aria-controls`)
 * with widget-local expanded state.
 */
export function ClayCollapse({
  title,
  children,
  defaultExpanded = false,
}: ClayCollapseProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  return (
    <Disclosure isExpanded={expanded} onExpandedChange={setExpanded}>
      <button
        type="button"
        className={styles.collapseHeader}
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
      >
        <span>{title}</span>
        <span
          aria-hidden="true"
          className={`${styles.collapseChevron} ${expanded ? styles.collapseChevronExpanded : ""}`}
        >
          ▸
        </span>
      </button>
      {expanded && (
        <DisclosurePanel className={styles.collapseBody}>
          {children}
        </DisclosurePanel>
      )}
    </Disclosure>
  );
}
