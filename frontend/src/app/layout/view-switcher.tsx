// The tab's view switcher: tab chrome, never inside a view (DESIGN.md §12).
// It is the design system's `seg` family — one selected item, hairline pill
// container — consumed here because it belongs to the titlebar, not to a
// catalog primitive.
//
// Rules (approved `start.html` / `shell.html` / `workspace.html`):
// - an uncommitted tab (nothing picked) shows both items disabled, with the
//   reason in the tooltip: the launcher is the surface up;
// - a tab with either half shows both items enabled — the missing half's view
//   is its own prompt, and picking one later never starts a new tab;
// - the active view is marked by the selected item alone (one signal per
//   surface; the accent bar/underline pattern is retired, DESIGN.md §14.13).

import { recipeAttributes } from "../../components/recipe-attributes";
import type { TabView } from "../../shell/tab-store";

import styles from "./shell.module.css";

export interface ViewSwitcherProps {
  view: TabView;
  /** The active tab has a folder / an agent. */
  hasWorkspace: boolean;
  hasAgent: boolean;
  onSelect: (view: TabView) => void;
}

/** Tab tooltips keep the app's literal chord spelling (`Ctrl+1`), matching
 *  the status bar's hints rather than the prototype's glyphs. */
const ITEMS: Array<{
  view: TabView;
  label: string;
  chord: string;
  /** Why the item is inert while the tab has picked nothing (approved
   *  `start.html`: `Pick a workspace first` / `Pick an agent first`). */
  reason: string;
}> = [
  {
    view: "workspace",
    label: "Workspace",
    chord: "Ctrl+1",
    reason: "Pick a workspace first",
  },
  {
    view: "agent",
    label: "Agent",
    chord: "Ctrl+2",
    reason: "Pick an agent first",
  },
];

export function ViewSwitcher({
  view,
  hasWorkspace,
  hasAgent,
  onSelect,
}: ViewSwitcherProps) {
  const uncommitted = !hasWorkspace && !hasAgent;
  return (
    <span
      className={styles.viewSwitch}
      role="tablist"
      aria-label="Tab view"
      data-viewswitch={uncommitted ? "empty" : view}
      {...recipeAttributes("seg", "root")}
    >
      {ITEMS.map((item) => {
        const disabled = uncommitted;
        const picked = item.view === "workspace" ? hasWorkspace : hasAgent;
        const title = disabled
          ? item.reason
          : picked
            ? `${item.label} view (${item.chord})`
            : `${item.label} view (${item.chord}) — nothing picked yet`;
        return (
          <button
            key={item.view}
            type="button"
            role="tab"
            className={styles.viewSwitchItem}
            aria-selected={view === item.view && !uncommitted}
            disabled={disabled}
            title={title}
            data-active={view === item.view && !uncommitted ? "true" : "false"}
            {...recipeAttributes("seg", "item")}
            onClick={() => onSelect(item.view)}
          >
            {item.label}
          </button>
        );
      })}
    </span>
  );
}
