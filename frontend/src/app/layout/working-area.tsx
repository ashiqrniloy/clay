import { Group, Panel, Separator } from "react-resizable-panels";

import styles from "./shell.module.css";

export interface WorkingAreaProps {
  /** Optional left fixed-slot content (e.g. workspace browser). */
  left?: React.ReactNode;
  /** Main slot content. */
  children: React.ReactNode;
}

/**
 * Pane split tree projection: main slot plus optional left fixed slot with a
 * keyboard-operable resize handle (separator semantics from
 * react-resizable-panels). Ratios clamp to the native 0.05–0.95 contract.
 */
export function WorkingArea({ left, children }: WorkingAreaProps) {
  if (!left) {
    return (
      <div className={styles.workingArea} data-testid="working-area">
        {children}
      </div>
    );
  }
  return (
    <Group orientation="horizontal" className={styles.workingArea}>
      <Panel defaultSize="24%" minSize="5%" maxSize="60%">
        {left}
      </Panel>
      <Separator
        style={{ width: "var(--clay-dimension-border-hairline, 1px)" }}
      />
      <Panel minSize="40%">{children}</Panel>
    </Group>
  );
}
