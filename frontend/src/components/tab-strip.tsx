import type { CSSProperties, ReactNode } from "react";
import { Tabs, TabList, Tab, TabPanel } from "react-aria-components";

import { ClayButton } from "./button";
import { ClayText } from "./text";
import { recipeAttributes } from "./recipe-attributes";
import styles from "./tab-strip.module.css";

export interface TabItem {
  id: string;
  label: ReactNode;
  dirty?: boolean;
  closable?: boolean;
  disabled?: boolean;
  content?: ReactNode;
}

export interface ClayTabStripProps {
  tabs: TabItem[];
  activeId?: string | null;
  defaultActiveId?: string;
  onActivate?: (id: string) => void;
  onClose?: (id: string) => void;
  onNew?: () => void;
  ariaLabel?: string;
  emptyLabel?: string;
  disabled?: boolean;
  className?: string;
  style?: CSSProperties;
  actions?: ReactNode;
}

/**
 * Catalog `tabList` kind (plan 108 G2, plan 110 task 5):
 * Unified tab strip and panel component over React Aria Tabs.
 * Shared across shell window tab bar and SDUI package panels.
 */
export function ClayTabStrip({
  tabs,
  activeId,
  defaultActiveId,
  onActivate,
  onClose,
  onNew,
  ariaLabel = "Tabs",
  emptyLabel = "No tabs",
  disabled = false,
  className,
  style,
  actions,
}: ClayTabStripProps) {
  const hasPanels = tabs.some((tab) => tab.content !== undefined);

  if (tabs.length === 0) {
    return (
      <div
        className={`${styles.emptyContainer} ${className ?? ""}`}
        style={style}
        role="presentation"
        {...recipeAttributes("tabList", "root")}
      >
        <span className={styles.empty}>
          <ClayText variant="detail" muted>
            {emptyLabel}
          </ClayText>
        </span>
        {actions}
        {onNew ? (
          <ClayButton variant="muted" onPress={onNew} aria-label="New tab">
            New tab
          </ClayButton>
        ) : null}
      </div>
    );
  }

  const firstSelected =
    activeId !== undefined
      ? (activeId ?? undefined)
      : (defaultActiveId ?? tabs[0]?.id);

  return (
    <Tabs
      className={`${hasPanels ? styles.withPanels : styles.stripOnly} ${className ?? ""}`}
      style={style}
      selectedKey={activeId !== undefined ? (activeId ?? undefined) : undefined}
      defaultSelectedKey={activeId === undefined ? firstSelected : undefined}
      onSelectionChange={(key) => onActivate?.(String(key))}
      isDisabled={disabled}
      {...recipeAttributes("tabList", "root")}
    >
      <div className={styles.stripWrapper}>
        <TabList
          aria-label={ariaLabel}
          className={styles.tabStrip}
          {...recipeAttributes("tabList", "strip")}
        >
          {tabs.map((tab) => (
            <Tab
              key={tab.id}
              id={tab.id}
              isDisabled={tab.disabled}
              className={styles.tab}
              {...recipeAttributes("tabList", "tab")}
            >
              {tab.dirty ? <span className={styles.dirty} aria-hidden /> : null}
              {typeof tab.label === "string" ? (
                <span>{tab.label}</span>
              ) : (
                tab.label
              )}
              {tab.closable && onClose ? (
                <button
                  type="button"
                  className={styles.close}
                  aria-label={`Close ${typeof tab.label === "string" ? tab.label : tab.id}`}
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    onClose(tab.id);
                  }}
                >
                  ×
                </button>
              ) : null}
            </Tab>
          ))}
        </TabList>
        {actions}
        {onNew ? (
          <ClayButton variant="muted" onPress={onNew} aria-label="New tab">
            +
          </ClayButton>
        ) : null}
      </div>
      {hasPanels &&
        tabs.map((tab) =>
          tab.content !== undefined ? (
            <TabPanel
              key={tab.id}
              id={tab.id}
              className={styles.tabPanel}
              {...recipeAttributes("tabList", "panel")}
            >
              {tab.content}
            </TabPanel>
          ) : null,
        )}
    </Tabs>
  );
}
