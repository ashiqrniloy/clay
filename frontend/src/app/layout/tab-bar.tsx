import {
  ClayTabStrip,
  type TabItem,
  type ClayTabStripProps,
} from "../../components";

export interface ShellTab {
  id: string;
  label: string;
  /** Full tooltip: the folder path and the attached agent. */
  title?: string;
  dirty?: boolean;
  closable?: boolean;
  /** Agent half attached (plan 118 task 33): the strip draws its marker and
   *  pulses it while the agent is working. */
  agent?: { busy: boolean } | null;
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
      variant="inline"
    />
  );
}

export { ClayTabStrip, type TabItem, type ClayTabStripProps };
