// Approval strip (plan 119 SC-4): the Allow/Deny alert for a suspended
// durable run. Presentation only — the decision payload and its optimistic
// clear stay in the panel, which owns the run's command lane.
import { useEffect, useRef } from "react";

import { ClayButton, ClayText } from "../components";
import type { AgentSnapshot } from "../agent/state";
import styles from "./coding-agent.module.css";

/** The store's own pending-approval shape — one derivation, no mirror. */
export type PendingApprovalView = NonNullable<AgentSnapshot["pendingApproval"]>;

export function ApprovalStrip({
  approval,
  onResolve,
}: {
  approval: PendingApprovalView;
  onResolve: (outcome: "allow_once" | "reject_once") => void;
}) {
  const stripRef = useRef<HTMLDivElement | null>(null);

  // F3 (plan 119 review): `alertdialog` promises the user is taken to the
  // decision. Move focus to the first action while the run is suspended, and
  // hand it back to whatever had it when the strip goes away — announcing a
  // dialog that the keyboard never enters is the misuse this replaces.
  useEffect(() => {
    const previous = document.activeElement;
    stripRef.current?.querySelector("button")?.focus();
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) {
        previous.focus();
      }
    };
  }, []);

  return (
    <div
      ref={stripRef}
      className={styles.approvalStrip}
      role="alertdialog"
      aria-label="Tool approval"
    >
      <ClayText variant="caption">
        Tool “{approval.toolName}” needs approval
      </ClayText>
      <ClayButton onPress={() => onResolve("allow_once")}>Allow</ClayButton>
      <ClayButton onPress={() => onResolve("reject_once")}>Deny</ClayButton>
    </div>
  );
}
