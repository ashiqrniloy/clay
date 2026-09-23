import { useEffect, useState, type KeyboardEventHandler } from "react";
import type { TransientMenuSnapshotDto } from "../bridge/types";
import { ClayIcon, ClayKbd, ClayText, ClayTextField } from "../components";
import { recipeAttributes } from "../components/recipe-attributes";

import styles from "./command-centre.module.css";

/**
 * The composer's `/` palette (plan 124; plan 125 made it the shell's *only*
 * transient surface): the command catalogue, the path browser and every picker
 * stage, drawn as the *menu of the field below it* — the lane's composer form
 * is the positioning context, so the sheet spans that box and rises from its
 * own edge 6px above it (DESIGN.md §5/§7/§11/§12).
 *
 * One sheet, one vocabulary from the server (`mode`): the catalogue keeps the
 * `/` sigil, its scope chips and its chord chips; a *stage* draws the session's
 * prompt as a micro-label, keeps the field's echo (the field is its filter) and
 * states its own keys in the foot. The window-centred sheet is retired, so a
 * picker no longer swaps this component out for a second one (§14.4).
 *
 * It owns **no input** with one deliberate exception: a stage whose value is a
 * credential renders a *shielded* field inside the sheet (`type="password"`,
 * §13), because the composer's draft is persisted layout state and must never
 * hold a secret. That stage's field replaces the composer's — and the
 * credential itself travels through the session's query and the host's
 * credential path alone (§13.11).
 *
 * Rows are `list` rows and the selection is the server session's: ↑↓ move it
 * from the field, `↵` runs it, a click picks it directly. The scope segment and
 * the per-row chords come from the *rows*, so a chip exists exactly when a row
 * carries one and nothing is derived from a command id here (DESIGN.md §12).
 */
export interface CommandPaletteProps {
  /** The palette session's snapshot (`origin === "commandPalette"`). */
  menu: TransientMenuSnapshotDto;
  /** The field's text after the `/` sigil for the catalogue — echoed live,
   *  never routed back: the field itself is the query (the session filters
   *  server-side). */
  query: string;
  /** The selected scope chip (`"all"` = every row). */
  scope: PaletteScope;
  /** Run the row at `index`: the session's selection moves there first. */
  onPick: (index: number) => void;
  /** Select a scope chip (the shell sends it as the session's filter). */
  onScope: (scope: PaletteScope) => void;
  /** The shielded stage's value (the only stage that owns an input). */
  onSecret?: (value: string) => void;
  /** Key routing for the shielded field: it is the focused control of that
   *  stage, so the field's ↑↓/`Esc`/`↵` belong to it there. */
  onKeyDown?: KeyboardEventHandler<HTMLInputElement | HTMLTextAreaElement>;
}

/** The palette's scope chips (DESIGN.md §12): `all` plus the three scopes the
 *  server's closed vocabulary can tag a row with. Order is the design's. */
export const PALETTE_SCOPES = ["all", "session", "shell", "files"] as const;

export type PaletteScope = (typeof PALETTE_SCOPES)[number];

/** The server's presentation vocabulary (protocol v32). `catalogue` is also
 *  spelled by an absent mode, so a session from an older daemon is the surface
 *  plan 124 shipped rather than a stage it cannot draw. */
export const PALETTE_MODES = [
  "catalogue",
  "path",
  "picker",
  "secret",
  "url",
  "oauth",
] as const;

export type PaletteMode = (typeof PALETTE_MODES)[number];

/** The row chord a session list declares for its secondary action (`Alt+↵`
 *  deletes the selected session). The server writes it on those rows alone
 *  (`agent_picker.rs`), so the sheet's foot can state the verb it enables. */
export const SECONDARY_ACTION_BINDING = "Alt+↵";

/** What `↵` does at a stage, in the design's own words (`choose` for a picker
 *  list, `resume` on a session list, `store` the credential, `save` the base
 *  URL, `run` the device flow). */
const STAGE_VERBS: Record<
  Exclude<PaletteMode, "catalogue" | "path">,
  string
> = {
  picker: "choose",
  secret: "store",
  url: "save",
  oauth: "run",
};

const SCOPE_LABELS: Record<PaletteScope, string> = {
  all: "All",
  session: "Session",
  shell: "Shell",
  files: "Files",
};

/** The chip's wire value: `All` is the absence of a scope. */
export function scopeWireValue(scope: PaletteScope): string | null {
  return scope === "all" ? null : scope;
}

/** The mode a snapshot declares. An unknown or absent spelling is drawn as the
 *  catalogue only when it *is* the catalogue: anything else falls back to the
 *  plain list stage, which claims no sigil and no shield. */
export function paletteModeOf(menu: TransientMenuSnapshotDto): PaletteMode {
  const mode = menu.mode ?? "catalogue";
  return (PALETTE_MODES as readonly string[]).includes(mode)
    ? (mode as PaletteMode)
    : "picker";
}

export function CommandPalette({
  menu,
  query,
  scope,
  onPick,
  onScope,
  onSecret,
  onKeyDown,
}: CommandPaletteProps) {
  const items = menu.items;
  const selected = menu.selectedIndex;
  const mode = paletteModeOf(menu);
  const catalogue = mode === "catalogue";
  // One stage claims an input of its own: the shielded credential. Its echo is
  // the server's bullet mask, so the sheet shows neither text nor a placeholder
  // for it — the shield is what carries the value (§12/§13).
  const shielded = mode === "secret";
  // Non-catalogue sessions name themselves with the prompt line; a stage with
  // an empty filter shows that line alone (the artifact's rule).
  const showPrompt = !catalogue;
  // The shielded stage's echo is the server's bullet mask, which is not a
  // value to read back: only the shield shows what is typed (§12/§13).
  const showEcho = !shielded && (catalogue || query.length > 0);
  const empty =
    typeof menu.status === "object"
      ? menu.status.empty.message
      : items.length === 0
        ? "No results"
        : null;
  const count = items.length === 1 ? "1 result" : `${items.length} results`;
  const secondary = items.some((item) =>
    item.bindings?.includes(SECONDARY_ACTION_BINDING),
  );
  // The segment exists when the session's rows are scoped at all (the `All`
  // chip is always the way back to the whole catalogue), or when a chip other
  // than `All` is selected and a query has emptied the list under it. A stage's
  // rows are not scoped, so it draws no chip it cannot fill.
  const segmented =
    catalogue && (scope !== "all" || items.some((item) => item.scope));

  // The shield's value is local to the stage: the sheet is remounted by its
  // caller when the session ends, and a stage change inside one session clears
  // it here — a credential outlives neither (§13).
  const [secret, setSecret] = useState("");
  useEffect(() => {
    setSecret("");
  }, [menu.sessionId, mode]);

  return (
    <section
      className={styles.palette}
      role="dialog"
      aria-label={menu.prompt}
      data-mode={mode}
      data-testid="command-palette"
    >
      {/* The head is the field's own echo: the icon is decoration and the `/`
          sigil is drawn by CSS, so a screen reader hears the dialog's name and
          the field below it, not this line twice. A stage names itself with the
          prompt instead — the stage's question, which is also the shield's own
          accessible name. */}
      <div className={styles.palHead}>
        <span className={styles.searchIcon} aria-hidden="true">
          <ClayIcon name="session.search" />
        </span>
        {showPrompt && <span className={styles.prompt}>{menu.prompt}</span>}
        {showEcho && (
          <span
            className={styles.palQuery}
            data-empty={query.length === 0 ? "true" : "false"}
          >
            {query.length > 0 ? query : "type a command"}
          </span>
        )}
        {segmented && (
          <span
            className={styles.palSeg}
            role="group"
            aria-label="Scope"
            {...recipeAttributes("seg", "root")}
          >
            {PALETTE_SCOPES.map((candidate) => (
              <button
                key={candidate}
                type="button"
                className={styles.palSegItem}
                aria-pressed={scope === candidate}
                data-active={scope === candidate ? "true" : "false"}
                data-scope={candidate}
                {...recipeAttributes("seg", "item")}
                onClick={() => onScope(candidate)}
              >
                {SCOPE_LABELS[candidate]}
              </button>
            ))}
          </span>
        )}
      </div>
      {shielded && (
        <div className={styles.stageField}>
          <ClayTextField
            label={menu.prompt}
            labelHidden
            type="password"
            autoFocus
            value={secret}
            onChange={(value) => {
              setSecret(value);
              onSecret?.(value);
            }}
            onKeyDown={onKeyDown}
          />
        </div>
      )}
      {items.length > 0 ? (
        <div
          className={styles.palList}
          role="listbox"
          aria-label={menu.prompt}
          aria-activedescendant={
            items[selected] ? `pal-option-${selected}` : undefined
          }
          {...recipeAttributes("list", "root")}
        >
          {items.map((item, index) => (
            <button
              key={item.id}
              id={`pal-option-${index}`}
              type="button"
              role="option"
              aria-selected={index === selected}
              data-selected={index === selected ? "true" : undefined}
              className={styles.palRow}
              onClick={() => onPick(index)}
              {...recipeAttributes("list", "row")}
            >
              <span className={styles.palTitle}>{item.label}</span>
              {/* The server's one detail line: routing and provenance (the
                  chords are chips of their own now). */}
              {item.detail && (
                <span
                  className={styles.palDetail}
                  {...recipeAttributes("list", "rowDetail")}
                >
                  {item.detail}
                </span>
              )}
              {item.bindings?.length ? (
                <span className={styles.palKeys}>
                  {item.bindings.map((binding) => (
                    <span key={binding} className={styles.palChord}>
                      {/* One chip per stroke, the status bar's own spelling
                          (`Ctrl+X Ctrl+P` → `Ctrl+X`, `Ctrl+P`). */}
                      {binding.split(" ").map((stroke) => (
                        <ClayKbd key={stroke}>{stroke}</ClayKbd>
                      ))}
                    </span>
                  ))}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      ) : (
        <div className={styles.empty} role="status">
          <ClayText variant="body" muted>
            {empty}
          </ClayText>
          <span className={styles.emptyHint}>
            <ClayKbd>Esc</ClayKbd>{" "}
            {catalogue || mode === "path"
              ? "dismisses the palette and returns the composer"
              : "goes back a stage"}
          </span>
        </div>
      )}
      <footer className={styles.foot}>
        <output className={styles.count} aria-live="polite">
          {count}
        </output>
        <span className={styles.spacer} />
        <span className={styles.hint}>
          <ClayKbd>↑↓</ClayKbd> navigate
        </span>
        {catalogue || mode === "path" ? (
          <>
            <span className={styles.hint}>
              <ClayKbd>↵</ClayKbd> run
            </span>
            <span className={styles.hint}>
              <ClayKbd>Esc</ClayKbd> close
            </span>
          </>
        ) : (
          <>
            <span className={styles.hint}>
              <ClayKbd>↵</ClayKbd> {secondary ? "resume" : STAGE_VERBS[mode]}
            </span>
            <span className={styles.hint}>
              <ClayKbd>Esc</ClayKbd> back
            </span>
            <span className={styles.hint}>
              <ClayKbd>Alt+←</ClayKbd> back
            </span>
            {secondary && (
              <span className={styles.hint}>
                <ClayKbd>{SECONDARY_ACTION_BINDING}</ClayKbd> delete
              </span>
            )}
          </>
        )}
      </footer>
    </section>
  );
}
