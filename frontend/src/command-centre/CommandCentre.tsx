import { useMemo, useState } from "react";

import { ClayList, ClayModal, ClayText, ClayTextField } from "../components";
import type { WorkspaceController } from "../shell/workspace-controller";

import styles from "./command-centre.module.css";

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

  return (
    <ClayModal title={menu.prompt} open onClose={() => workspace.menuCancel()}>
      <div className={styles.surface} data-testid="command-centre">
        <ClayTextField
          label={secretPrompt || menu.origin !== "centered" ? menu.prompt : "Search"}
          value={draft}
          onChange={(query) => {
            setDraft(query);
            workspace.menuQuery(query);
          }}
          autoFocus
          onKeyDown={(event) => {
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
          }}
        />
        {items.length > 0 ? (
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
        ) : (
          <div className={styles.empty} role="status">
            <ClayText variant="body" muted>
              {empty}
            </ClayText>
          </div>
        )}
        <output className={styles.status} aria-live="polite">
          {menu.items.length === 1
            ? "1 result"
            : `${menu.items.length} results`}
        </output>
        <ClayText variant="caption" muted>
          {menu.prompt.startsWith("Browse")
            ? "Enter opens. Alt+Enter opens a folder as workspace. Escape closes."
            : "Enter runs the selected command. Escape closes."}
        </ClayText>
      </div>
    </ClayModal>
  );
}
