import type { TransientMenuSnapshotDto } from "../bridge/types";
import { ClayIcon, ClayKbd, ClayText } from "../components";
import { recipeAttributes } from "../components/recipe-attributes";

import styles from "./command-centre.module.css";

/**
 * The composer's `/` palette (plan 124): the command catalogue and its path
 * mode, drawn as the *menu of the field below it* — the lane's composer form is
 * the positioning context, so the sheet spans that box and rises from its own
 * edge 6px above it (DESIGN.md §5/§7/§11/§12).
 *
 * It owns **no input**: the lane's field is the query, so the head only echoes
 * what the field holds (after the `/` sigil) and the sheet draws no well and no
 * focus ring of its own — the boundary in focus stays the composer box's
 * (§9/§14.4). Rows are `list` rows and the selection is the server session's:
 * ↑↓ move it from the field, `↵` runs it, a click picks it directly.
 *
 * The scope segment and the per-row chords come from the *rows*: the server
 * tags each item with a scope from its closed vocabulary and states its
 * `bindings`, so a chip exists exactly when a row carries one and nothing is
 * derived from a command id here (DESIGN.md §12). The chip is a filter of the
 * session, not a local view: selecting one sends it to the server with the
 * query, so the session keeps selecting over the rows it shows.
 */
export interface CommandPaletteProps {
  /** The palette session's snapshot (`origin === "commandPalette"`). */
  menu: TransientMenuSnapshotDto;
  /** The field's text after the `/` sigil — echoed live, never routed back:
   *  the field itself is the query (the session filters server-side). */
  query: string;
  /** The selected scope chip (`"all"` = every row). */
  scope: PaletteScope;
  /** Run the row at `index`: the session's selection moves there first. */
  onPick: (index: number) => void;
  /** Select a scope chip (the shell sends it as the session's filter). */
  onScope: (scope: PaletteScope) => void;
}

/** The palette's scope chips (DESIGN.md §12): `all` plus the three scopes the
 *  server's closed vocabulary can tag a row with. Order is the design's. */
export const PALETTE_SCOPES = ["all", "session", "shell", "files"] as const;

export type PaletteScope = (typeof PALETTE_SCOPES)[number];

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

export function CommandPalette({
  menu,
  query,
  scope,
  onPick,
  onScope,
}: CommandPaletteProps) {
  const items = menu.items;
  const selected = menu.selectedIndex;
  const empty =
    typeof menu.status === "object"
      ? menu.status.empty.message
      : items.length === 0
        ? "No results"
        : null;
  const count = items.length === 1 ? "1 result" : `${items.length} results`;
  // The segment exists when the session's rows are scoped at all (the `All`
  // chip is always the way back to the whole catalogue), or when a chip other
  // than `All` is selected and a query has emptied the list under it.
  const segmented = scope !== "all" || items.some((item) => item.scope);
  return (
    <section
      className={styles.palette}
      role="dialog"
      aria-label={menu.prompt}
      data-testid="command-palette"
    >
      {/* The head is the field's own echo: the icon is decoration and the `/`
          sigil is drawn by CSS, so a screen reader hears the dialog's name and
          the field below it, not this line twice. */}
      <div className={styles.palHead}>
        <span className={styles.searchIcon} aria-hidden="true">
          <ClayIcon name="session.search" />
        </span>
        <span
          className={styles.palQuery}
          data-empty={query.length === 0 ? "true" : "false"}
        >
          {query.length > 0 ? query : "type a command"}
        </span>
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
      {items.length > 0 ? (
        <div
          className={styles.palList}
          role="listbox"
          aria-label={menu.prompt}
          {...recipeAttributes("list", "root")}
        >
          {items.map((item, index) => (
            <button
              key={item.id}
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
            <ClayKbd>Esc</ClayKbd> dismisses the palette and returns the
            composer
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
        <span className={styles.hint}>
          <ClayKbd>↵</ClayKbd> run
        </span>
        <span className={styles.hint}>
          <ClayKbd>Esc</ClayKbd> close
        </span>
      </footer>
    </section>
  );
}
