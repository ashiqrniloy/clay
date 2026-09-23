import { useMemo, useRef, useState, type ReactNode } from "react";
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
import { ClayKbd } from "./chrome";
import { ClayTextField } from "./text-field";

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
  /** Recipe family for the *trigger* (plan 118 task 35): the agent-type
   *  picker is a dropdown whose trigger is the agent view's title, so it
   *  consumes `agentPicker.default.trigger.*` while the popover, list and
   *  rows stay the shared dropdown ones. */
  triggerFamily?: "dropdown" | "agentPicker";
  /** Row annotation for the selected option (e.g. `current`). */
  selectedHint?: string;
  /** Content under the list inside the popover: where the values come from,
   *  or why one cannot be picked yet. */
  footer?: ReactNode;
}

/** Catalog `dropdown` kind: button trigger + listbox, arrow/typeahead nav. */
export function ClayDropdown({
  label,
  options,
  groups,
  selectedId,
  onSelect,
  disabled = false,
  triggerFamily = "dropdown",
  selectedHint,
  footer,
}: ClayDropdownProps) {
  const triggerAttributes = recipeAttributes(triggerFamily, "trigger");
  const row = (option: DropdownOption) => (
    <ListBoxItem
      key={option.id}
      id={option.id}
      textValue={option.label}
      className={`${styles.listRow} ${styles.dropdownItem}`}
      isDisabled={option.disabled}
      {...recipeAttributes("dropdown", "item")}
    >
      {({ isSelected }) => (
        <>
          <span className={styles.dropdownItemLabel}>{option.label}</span>
          {isSelected && selectedHint ? (
            <span className={styles.dropdownItemHint}>{selectedHint}</span>
          ) : null}
        </>
      )}
    </ListBoxItem>
  );
  const selected =
    options.find((option) => option.id === selectedId) ??
    groups
      ?.flatMap((group) => group.options)
      .find((option) => option.id === selectedId);
  return (
    <Select
      aria-label={label}
      selectedKey={selectedId}
      onSelectionChange={(key) => onSelect(String(key))}
      isDisabled={disabled}
    >
      <Button
        className={styles.selectTrigger}
        aria-label={label}
        {...triggerAttributes}
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
          className={`${styles.listBox} ${styles.menuList}`}
          {...recipeAttributes("dropdown", "list")}
        >
          {groups
            ? groups.map((group) => (
                <Section
                  key={group.label}
                  id={group.label}
                  className={styles.listSection}
                >
                  <Header className={styles.listSectionHeader}>
                    {group.label}
                  </Header>
                  {group.options.map(row)}
                </Section>
              ))
            : options.map(row)}
        </ListBox>
        {footer ? <div className={styles.dropdownFooter}>{footer}</div> : null}
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

/** A list's filter affordance (the SDUI list node's `filter`, plan 118 task
 *  E1): inert presentation data — the host renders the tools row, filters the
 *  delivered rows locally and shows the live match count. */
export interface ClayListFilter {
  placeholder: string;
  /** Single-key shortcut the shell binds to focus the field (`/`). */
  shortcut?: string;
}

export interface ClayListProps {
  items: ListItem[];
  selectedId?: string | null;
  onSelect?: (id: string) => void;
  onAction?: (id: string) => void;
  ariaLabel: string;
  filter?: ClayListFilter | null;
}

/** Catalog `list` kind: rows with title/detail and selection semantics. */
export function ClayList({
  items,
  selectedId = null,
  onSelect,
  onAction,
  ariaLabel,
  filter = null,
}: ClayListProps) {
  const [query, setQuery] = useState("");
  const listRef = useRef<HTMLDivElement | null>(null);
  // The listing arrives whole and bounded, so the filter is presentation over
  // what the server already authorized: keystroke-local, no round-trip.
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((item) =>
      `${item.id} ${item.title} ${item.detail ?? ""}`
        .toLowerCase()
        .includes(needle),
    );
  }, [items, query]);
  if (!filter) {
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
          <ListRow key={item.id} item={item} />
        ))}
      </ListBox>
    );
  }
  const firstMatch = visible.find((item) => !item.disabled);
  return (
    <div className={styles.listFiltered} {...recipeAttributes("list", "root")}>
      <div className={styles.listTools} data-clay-list-filter>
        <ClayTextField
          label={filter.placeholder}
          labelHidden
          value={query}
          onChange={setQuery}
          placeholder={filter.placeholder}
          role="monospace"
          aria-describedby={undefined}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              setQuery("");
              return;
            }
            if (event.key === "Enter" && firstMatch && onAction) {
              event.preventDefault();
              onAction(firstMatch.id);
            }
            if (event.key === "ArrowDown") {
              event.preventDefault();
              listRef.current?.focus();
            }
          }}
        />
        {filter.shortcut && <ClayKbd>{filter.shortcut}</ClayKbd>}
      </div>
      <ListBox
        ref={listRef}
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
        {visible.map((item) => (
          <ListRow key={item.id} item={item} />
        ))}
      </ListBox>
      <div className={styles.listFoot}>
        <span className={styles.listCount} aria-live="polite">
          {query.trim()
            ? `${visible.length} ${visible.length === 1 ? "match" : "matches"}`
            : ""}
        </span>
        <span className={styles.spacer} />
        <span className={styles.listHint}>
          <ClayKbd>↑↓</ClayKbd> move
        </span>
        <span className={styles.listHint}>
          <ClayKbd>↵</ClayKbd> open
        </span>
      </div>
    </div>
  );
}

/** One row: title (with its optional icon) over the muted detail line. */
function ListRow({ item }: { item: ListItem }) {
  return (
    <ListBoxItem
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
  );
}

// ---------------------------------------------------------------- collapse

export interface ClayCollapseProps {
  /** Section title: a node so a surface can render its own eyebrow treatment
   *  (the settings rows use the micro-label style) without a second component. */
  title: ReactNode;
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
    <Disclosure
      isExpanded={expanded}
      onExpandedChange={setExpanded}
      className={styles.collapseRoot}
      {...recipeAttributes("collapse", "root")}
    >
      <button
        type="button"
        className={styles.collapseHeader}
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
        {...recipeAttributes("collapse", "header")}
      >
        <span
          aria-hidden="true"
          className={`${styles.collapseChevron} ${expanded ? styles.collapseChevronExpanded : ""}`}
          {...recipeAttributes("collapse", "chevron")}
        >
          <ClayIcon name="disclosure.right" />
        </span>
        <span {...recipeAttributes("collapse", "title")}>{title}</span>
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
