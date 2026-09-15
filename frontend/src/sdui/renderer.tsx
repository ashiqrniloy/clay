import { Fragment, type ReactNode } from "react";

import {
  ClayButton,
  ClayIcon,
  ClayIconButton,
  ClayList,
  ClayText,
  recipeAttributes,
} from "../components";
import { sduiActionPayload, type IntentSender } from "./actions";
import { detached } from "../lib/detached";
import type { SduiActionIntent } from "./types";
import type { SduiState } from "./state";

import styles from "./renderer.module.css";

// File-browser visibility affordance: the server's toggle buttons keep honest
// labels ("Show/Hide file browser"), so the renderer recognizes the pair by
// label and paints them as the `>`/`<` indicator instead of a text button.
const FILE_BROWSER_TOGGLE_LABELS = new Set([
  "Show file browser",
  "Hide file browser",
]);

export function SduiRenderer({
  state,
  send,
  editorSlot,
}: {
  state: SduiState;
  send: IntentSender;
  editorSlot: ReactNode;
}) {
  const dispatch = (intent: SduiActionIntent) => {
    detached(send(sduiActionPayload(state.version, intent)));
  };
  const render = (id: number, ancestors = new Set<number>()): ReactNode => {
    if (ancestors.has(id)) return null;
    const node = state.nodes.get(id);
    if (!node) return null;
    const next = new Set(ancestors).add(id);
    // A region node may name the host dimension token it is sized from
    // (`dimension.sidebar.default`). The attribute is data; the rule that reads
    // it and the token it resolves to are host-owned CSS, so a tree can neither
    // invent geometry nor pick an arbitrary size.
    const sizeToken = node.size ?? undefined;
    const children = (ids: number[]) =>
      ids.map((child) => (
        <Fragment key={child}>{render(child, next)}</Fragment>
      ));
    const kind = node.kind;
    if ("panel" in kind) {
      return (
        <aside
          className={styles.panel}
          aria-labelledby={`sdui-${id}-title`}
          {...recipeAttributes("panel", "root")}
        >
          <ClayText id={`sdui-${id}-title`} variant="title">
            {kind.panel.title}
          </ClayText>
          {children(kind.panel.children)}
        </aside>
      );
    }
    if ("label" in kind) {
      return (
        <ClayText variant="body">
          {kind.label.icon && <ClayIcon name={kind.label.icon} />}
          {kind.label.text}
        </ClayText>
      );
    }
    if ("button" in kind) {
      // A labeled icon action (the file browser's hide/show indicator) is
      // an icon button: no text glyph, the `>`/`<` tells the direction.
      if (FILE_BROWSER_TOGGLE_LABELS.has(kind.button.label)) {
        return (
          <span
            className={
              kind.button.label.startsWith("Show")
                ? styles.expandIndicator
                : styles.collapseIndicator
            }
          >
            <ClayIconButton
              icon="disclosure.right"
              label={kind.button.label}
              shortcut="Ctrl+B"
              variant="muted"
              onPress={() => dispatch(kind.button.action)}
            />
          </span>
        );
      }
      return (
        <ClayButton onPress={() => dispatch(kind.button.action)}>
          {kind.button.icon && <ClayIcon name={kind.button.icon} />}
          {kind.button.label}
        </ClayButton>
      );
    }
    if ("list" in kind) {
      return (
        <ClayList
          ariaLabel="Server-driven items"
          filter={
            kind.list.filter
              ? {
                  placeholder: kind.list.filter.placeholder,
                  shortcut: kind.list.filter.shortcut ?? undefined,
                }
              : null
          }
          items={kind.list.items.map((item) => ({
            id: item.id,
            title: item.label,
            detail: item.detail ?? undefined,
            icon: item.icon ?? undefined,
            disabled: item.action == null,
          }))}
          onAction={(itemId) => {
            const action = kind.list.items.find(
              (item) => item.id === itemId,
            )?.action;
            if (action) dispatch(action);
          }}
        />
      );
    }
    if ("editorView" in kind) return editorSlot;
    if ("flex" in kind) {
      const direction =
        kind.flex.direction === "row"
          ? styles.row
          : kind.flex.direction === "column"
            ? styles.column
            : styles.flexDefault;
      // The sidebar title line keeps the workspace name and its hide
      // indicator on one row instead of stacking them.
      const isSidebarHead =
        kind.flex.direction === "row" &&
        kind.flex.children.some((child) => {
          const node = state.nodes.get(child);
          return (
            node != null &&
            "button" in node.kind &&
            FILE_BROWSER_TOGGLE_LABELS.has(node.kind.button.label)
          );
        });
      return (
        <div
          className={`${direction} ${isSidebarHead ? styles.sidebarHead : ""}`.trim()}
          {...recipeAttributes("flex", "root")}
        >
          {children(kind.flex.children)}
        </div>
      );
    }
    return (
      <div
        className={styles.stack}
        data-clay-size={sizeToken}
        {...recipeAttributes("stack", "root")}
      >
        {children(kind.stack.children)}
      </div>
    );
  };

  return <>{render(state.rootId)}</>;
}
