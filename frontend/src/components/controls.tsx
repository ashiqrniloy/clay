import { useState, type ReactNode } from "react";
import {
  ListBox,
  ListBoxItem,
  Section,
  Header,
  Select,
  SelectValue,
  Button,
  Popover,
  Disclosure,
  DisclosurePanel,
} from "react-aria-components";

import styles from "./controls.module.css";
import { recipeAttributes } from "./recipe-attributes";
import { ClayIcon } from "./icon";

// ---------------------------------------------------------------- dropdown

export interface DropdownOption {
  id: string;
  label: string;
  disabled?: boolean;
}

export interface DropdownGroup {
  label: string;
  options: DropdownOption[];
}

export interface ClayDropdownProps {
  label: string;
  options: DropdownOption[];
  /** Optional provider-style grouping (plan 109 I3): rendered as labeled
   *  sections; `options` is ignored when `groups` is present. */
  groups?: DropdownGroup[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  disabled?: boolean;
}

/** Catalog `dropdown` kind: button trigger + listbox, arrow/typeahead nav. */
export function ClayDropdown({
  label,
  options,
  groups,
  selectedId,
  onSelect,
  disabled = false,
}: ClayDropdownProps) {
  const selected = options.find((option) => option.id === selectedId) ??
    groups?.flatMap((group) => group.options).find(
      (option) => option.id === selectedId,
    );
  return (
    <Select
      aria-label={label}
      selectedKey={selectedId}
      onSelectionChange={(key) => onSelect(String(key))}
      isDisabled={disabled}
    >
      <Button
        className={styles.selectTrigger}
        {...recipeAttributes("dropdown", "trigger")}
      >
        <SelectValue>{selected?.label ?? label}</SelectValue>
        <span
          aria-hidden="true"
          className={styles.selectIndicator}
          {...recipeAttributes("dropdown", "indicator")}
        >
          <ClayIcon name="disclosure.down" />
        </span>
      </Button>
      <Popover
        className={styles.popover}
        {...recipeAttributes("dropdown", "popover")}
      >
        <ListBox
          className={styles.listBox}
          {...recipeAttributes("dropdown", "list")}
        >
          {groups
            ? groups.map((group) => (
                <Section key={group.label} id={group.label} className={styles.listSection}>
                  <Header className={styles.listSectionHeader}>{group.label}</Header>
                  {group.options.map((option) => (
                    <ListBoxItem
                      key={option.id}
                      id={option.id}
                      className={styles.listRow}
                      isDisabled={option.disabled}
                      {...recipeAttributes("dropdown", "item")}
                    >
                      {option.label}
                    </ListBoxItem>
                  ))}
                </Section>
              ))
            : options.map((option) => (
                <ListBoxItem
                  key={option.id}
                  id={option.id}
                  className={styles.listRow}
                  isDisabled={option.disabled}
                  {...recipeAttributes("dropdown", "item")}
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
  /** Semantic icon key resolved against the active icon pack (Plan 112). */
  icon?: string;
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
      {...recipeAttributes("list", "root")}
    >
      {items.map((item) => (
        <ListBoxItem
          key={item.id}
          id={item.id}
          textValue={item.title}
          className={styles.listRow}
          isDisabled={item.disabled}
          {...recipeAttributes("list", "row")}
        >
          <span {...recipeAttributes("list", "rowTitle")}>
            {item.icon && <ClayIcon name={item.icon} />}
            {item.title}
          </span>
          {item.detail && (
            <span
              className={styles.rowDetail}
              {...recipeAttributes("list", "rowDetail")}
            >
              {item.detail}
            </span>
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
        {...recipeAttributes("collapse", "header")}
      >
        <span {...recipeAttributes("collapse", "title")}>{title}</span>
        <span
          aria-hidden="true"
          className={`${styles.collapseChevron} ${expanded ? styles.collapseChevronExpanded : ""}`}
          {...recipeAttributes("collapse", "chevron")}
        >
          <ClayIcon name="disclosure.right" />
        </span>
      </button>
      {expanded && (
        <DisclosurePanel
          className={styles.collapseBody}
          {...recipeAttributes("collapse", "body")}
        >
          {children}
        </DisclosurePanel>
      )}
    </Disclosure>
  );
}
