import { useRef, useState, type CSSProperties, type ReactNode } from "react";
import { Tabs, TabList, Tab, TabPanel } from "react-aria-components";

import { ClayButton, ClayIconButton } from "./button";
import { ClayIcon } from "./icon";
import { ClayText } from "./text";
import { recipeAttributes } from "./recipe-attributes";
import styles from "./tab-strip.module.css";

export interface TabItem {
  id: string;
  label: ReactNode;
  /** Tooltip on the tab itself (the shell uses the full folder path here,
   *  because the label is only the basename). */
  title?: string;
  dirty?: boolean;
  closable?: boolean;
  disabled?: boolean;
  /** Agent marker (plan 118 task 33): shown as the approved mono word, and
   *  `busy` marks it in the accent colour while the agent works (the pulse
   *  itself lives in the window mark — plan 124, DESIGN.md §7). */
  agent?: { busy: boolean } | null;
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
  /** Fixed chevron that pages hidden tab labels without exposing a scrollbar. */
  overflowNavigation?: boolean;
  /** `panel` (default) is a standalone tab bar with its own inner hairline;
   *  `inline` sits inside chrome that already draws the boundary — the shell's
   *  titlebar — so the strip adds no second line (DESIGN.md §14.4). */
  variant?: "panel" | "inline";
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
  overflowNavigation = false,
  variant = "panel",
}: ClayTabStripProps) {
  const hasPanels = tabs.some((tab) => tab.content !== undefined);
  const tabListRef = useRef<HTMLDivElement | null>(null);
  const [showPreviousTabs, setShowPreviousTabs] = useState(false);
  const pageTabs = () => {
    const tabList = tabListRef.current;
    if (!tabList) return;
    if (showPreviousTabs) tabList.scrollTo({ left: 0, behavior: "smooth" });
    else tabList.scrollBy({ left: tabList.clientWidth, behavior: "smooth" });
    setShowPreviousTabs((current) => !current);
  };
  const endActions = (
    <>
      {overflowNavigation ? (
        <span className={showPreviousTabs ? styles.overflowBack : undefined}>
          <ClayIconButton
            icon="disclosure.right"
            label={`${showPreviousTabs ? "Show previous" : "Show more"} ${ariaLabel.toLowerCase()} tabs`}
            variant="muted"
            onPress={pageTabs}
          />
        </span>
      ) : null}
      {actions}
      {onNew ? (
        <ClayButton variant="muted" onPress={onNew} aria-label="New tab">
          <ClayIcon name="action.new" />
        </ClayButton>
      ) : null}
    </>
  );

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
      <div
        className={`${styles.stripWrapper} ${
          variant === "inline" ? styles.inline : ""
        }`}
      >
        <TabList
          ref={tabListRef}
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
                <span title={tab.title}>{tab.label}</span>
              ) : (
                tab.label
              )}
              {tab.agent ? (
                <span
                  className={styles.agent}
                  data-busy={tab.agent.busy ? "true" : "false"}
                  title={tab.agent.busy ? "Agent working" : "Agent attached"}
                >
                  agent
                </span>
              ) : null}
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
                  <ClayIcon name="action.close" />
                </button>
              ) : null}
            </Tab>
          ))}
        </TabList>
        {!hasPanels && endActions}
      </div>
      {hasPanels && (overflowNavigation || actions || onNew) ? (
        <div className={styles.actions}>{endActions}</div>
      ) : null}
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
