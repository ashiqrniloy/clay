import { ClayTabStrip, type TabItem, type ClayTabStripProps } from "../../components";

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
 * Unified primitive wrapper delegating to catalog `ClayTabStrip`.
 */
export function TabBar({
  tabs,
  activeId,
  onActivate,
  onClose,
  onNew,
}: TabBarProps) {
  return (
    <ClayTabStrip
      tabs={tabs}
      activeId={activeId}
      onActivate={onActivate}
      onClose={onClose}
      onNew={onNew}
      ariaLabel="Window tabs"
      emptyLabel="No tabs"
    />
  );
}

export { ClayTabStrip, type TabItem, type ClayTabStripProps };
