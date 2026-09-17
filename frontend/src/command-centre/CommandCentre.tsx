import { useMemo, useState, type KeyboardEvent } from "react";

import {
  ClayIcon,
  ClayKbd,
  ClayList,
  ClayModal,
  ClayText,
} from "../components";
import type { WorkspaceController } from "../shell/workspace-controller";

import styles from "./command-centre.module.css";

/**
 * The window's transient menus that own their input and their surface: the
 * centred sheet (`centered`: pickers, package menus — a head with its field,
 * the scrolling results, and a foot of key hints) and the narrower
 * `contextMenu`/`menuBar` popover. Menu sessions take the popover surface and
 * show their prompt as the head's micro-label; the sheet's prompt is its
 * accessible name.
 *
 * Plan 124: the composer's `/` palette is **not** here. Its query is the lane's
 * field and its anchor is that field's box, so the lane draws it
 * (`CommandPalette`) — this component keeps the sessions that are window
 * surfaces of their own (the `commandPalette` origin is filtered out before it
 * is mounted, `WorkspacePanes`).
 */
export function CommandCentre({
  workspace,
}: {
  workspace: WorkspaceController;
}) {
  const menu = workspace.active()?.menu ?? null;
  const stageKey = `${String(menu?.sessionId ?? "")}:${menu?.prompt ?? ""}`;
  const [stage, setStage] = useState(stageKey);
  const [draft, setDraft] = useState(menu?.query ?? "");
  // Secret snapshots mask query as bullets. Binding that string would clobber
  // the typed key, so draft only resets on a new session/stage.
  if (stage !== stageKey) {
    setStage(stageKey);
    setDraft(menu?.query ?? "");
  }
  const items = useMemo(
    () =>
      (menu?.items ?? []).map((item) => ({
        id: item.id,
        title: item.label,
        detail: item.detail ?? undefined,
      })),
    [menu?.items],
  );
  if (!menu) return null;

  const selected = menu.items[menu.selectedIndex]?.id ?? null;
  const empty =
    typeof menu.status === "object"
      ? menu.status.empty.message
      : menu.items.length === 0
        ? "No results"
        : null;
  const flushAndActivate = (secondary = false) => {
    workspace.menuQuery(draft);
    workspace.menuActivate(secondary);
  };
  const secretPrompt = /api key|hidden|base url/i.test(menu.prompt);
  const palette = menu.origin === "centered";
  const count =
    menu.items.length === 1 ? "1 result" : `${menu.items.length} results`;
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Backspace" && draft.length === 0) {
      event.preventDefault();
      workspace.menuBackspace();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      workspace.menuMove(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      workspace.menuMove(-1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      flushAndActivate(event.altKey);
    }
  };

  return (
    <ClayModal
      title={menu.prompt}
      open
      onClose={() => workspace.menuCancel()}
      flush
    >
      <div
        className={
          palette ? styles.surface : `${styles.surface} ${styles.menu}`
        }
        data-testid="command-centre"
      >
        <div className={styles.head}>
          <span className={styles.searchIcon} aria-hidden="true">
            <ClayIcon name="session.search" />
          </span>
          {/* The server's prompt names the session ("Command Centre",
              "Browse workspace", "Session actions", "API key (hidden)"): it is
              the head's micro-label and the field's name, so the visible label
              and the accessible name never disagree. */}
          <span className={styles.prompt}>{menu.prompt}</span>
          <input
            className={styles.input}
            aria-label={menu.prompt}
            placeholder={
              secretPrompt
                ? undefined
                : palette
                  ? "Type a command or a file path"
                  : "Type to filter"
            }
            value={draft}
            onChange={(event) => {
              setDraft(event.target.value);
              workspace.menuQuery(event.target.value);
            }}
            onKeyDown={onKeyDown}
            autoFocus
            autoComplete="off"
            spellCheck={false}
          />
        </div>
        {items.length > 0 ? (
          <div className={styles.results}>
            <ClayList
              ariaLabel={`${menu.prompt} results`}
              items={items}
              selectedId={selected}
              onSelect={(id) => {
                const index = menu.items.findIndex((item) => item.id === id);
                if (index >= 0) workspace.menuMove(index - menu.selectedIndex);
              }}
              onAction={() => flushAndActivate(false)}
            />
          </div>
        ) : (
          <div className={styles.empty} role="status">
            <ClayText variant="body" muted>
              {empty}
            </ClayText>
            <span className={styles.emptyHint}>
              <ClayKbd>Esc</ClayKbd> clears and closes
            </span>
          </div>
        )}
        <div className={styles.foot}>
          <span className={styles.hint}>
            <span className={styles.hintKeys} aria-hidden="true">
              <ClayKbd>↑</ClayKbd>
              <ClayKbd>↓</ClayKbd>
            </span>
            navigate
          </span>
          <span className={styles.hint}>
            <ClayKbd>Enter</ClayKbd> run
          </span>
          <span className={styles.hint}>
            <ClayKbd>Esc</ClayKbd> close
          </span>
          <span className={styles.spacer} />
          <output className={styles.count} aria-live="polite">
            {count}
          </output>
        </div>
      </div>
    </ClayModal>
  );
}
