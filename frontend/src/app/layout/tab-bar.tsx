import { Tabs, TabList, Tab } from "react-aria-components";

import { ClayButton, ClayText } from "../../components";
import styles from "./tab-bar.module.css";

export interface ShellTab {
  id: string;
  label: string;
  dirty?: boolean;
  closable?: boolean;
}

export interface TabBarProps {
  tabs: ShellTab[];
  activeId: string | null;
  onActivate?: (id: string) => void;
  onClose?: (id: string) => void;
  onNew?: () => void;
}

/**
 * Shell-owned window tab strip (`tab`/`tablist` semantics via React Aria).
 * Tabs stay application state; the router never owns them.
 */
export function TabBar({
  tabs,
  activeId,
  onActivate,
  onClose,
  onNew,
}: TabBarProps) {
  if (tabs.length === 0) {
    return (
      <div className={styles.tabList} role="presentation">
        <span className={styles.empty}>
          <ClayText variant="detail" muted>
            No tabs
          </ClayText>
        </span>
        {onNew ? (
          <ClayButton variant="muted" onPress={onNew}>
            New tab
          </ClayButton>
        ) : null}
      </div>
    );
  }
  return (
    <div className={styles.tabList}>
      <Tabs
        selectedKey={activeId ?? undefined}
        onSelectionChange={(key) => onActivate?.(String(key))}
      >
        <TabList aria-label="Window tabs" style={{ display: "contents" }}>
          {tabs.map((tab) => (
            <Tab key={tab.id} id={tab.id} className={styles.tab}>
              {tab.dirty ? <span className={styles.dirty} aria-hidden /> : null}
              {tab.label}
              {tab.closable && onClose ? (
                <button
                  type="button"
                  className={styles.close}
                  aria-label={`Close ${tab.label}`}
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
      </Tabs>
      {onNew ? (
        <ClayButton variant="muted" onPress={onNew} aria-label="New tab">
          +
        </ClayButton>
      ) : null}
    </div>
  );
}
